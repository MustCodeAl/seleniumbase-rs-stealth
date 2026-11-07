//! The fetch loop of the browser scraper: load, detect, decide, act, and load again.

use std::time::{Duration, Instant};

use super::{BrowserFetch, BrowserScraper};
use crate::challenge::detect::{ChallengeKind, Detection};
use crate::challenge::mitigation::{AttemptContext, FetchKind, GiveUpReason, Mitigation};
use crate::egress::redact::RedactedUrl;
use crate::error::ScrapeError;
use crate::jar::store::site_of;
use crate::ledger::events::ScrapeEvent;
use crate::ledger::state::SiteRecord;
use crate::runtime::cdp::{self, NavResponse, PageSnapshot};
use seleniumbase_rs::config::proxy_list::ProxyOutcome;
use seleniumbase_rs::utils::urls::normalize_page_url;

/// Why a URL is refused without echoing it.
const INVALID_URL: &str = "the URL is not valid";

/// Why a URL with no host is refused.
const NO_HOST: &str = "the URL has no host";

/// A page to fetch: the URL the browser is given, the site it belongs to, and its redacted form.
pub(super) struct Target {
    pub(super) url: String,
    pub(super) site: String,
    redacted: RedactedUrl,
}

impl Target {
    /// Reads `raw` the way [`Page::goto`](seleniumbase_rs::sb_cdp::Page::goto) does, so a bare host gets
    /// `https://`, and finds the registrable domain of its host.
    pub(super) fn parse(raw: &str) -> Result<Self, ScrapeError> {
        let refused = |reason: &str| ScrapeError::Navigation {
            url: RedactedUrl::new(raw),
            reason: reason.to_owned(),
        };
        let url = normalize_page_url(raw).map_err(|_error| refused(INVALID_URL))?;
        let parsed = url::Url::parse(&url).map_err(|_error| refused(INVALID_URL))?;
        let host = parsed.host_str().ok_or_else(|| refused(NO_HOST))?;
        let site = site_of(host).unwrap_or_else(|| host.to_ascii_lowercase());
        Ok(Self {
            redacted: RedactedUrl::new(&url),
            url,
            site,
        })
    }
}

/// What to do after an action was carried out.
pub(super) enum Step {
    /// The page is real; return it.
    Finish,
    /// Load the page again.
    Again,
}

/// What a loop knows of a challenge it solved.
struct Solved {
    kind: ChallengeKind,
    since: Instant,
}

impl BrowserScraper {
    /// Fetches `url` through the browser, solving challenges on the way.
    ///
    /// A site in cooldown is not requested at all. The state of the site (successes, challenges,
    /// cooldown) is read before the first load and saved after the last, whether the fetch worked
    /// or not.
    ///
    /// # Errors
    ///
    /// - [`ScrapeError::CoolingDown`] when the site is to be left alone, before or because of this
    ///   fetch;
    /// - [`ScrapeError::Unsolved`] when a challenge was still there after the attempts allowed;
    /// - [`ScrapeError::GaveUp`] when the policy stopped for another reason;
    /// - [`ScrapeError::Navigation`] when the browser could not load the page;
    /// - [`ScrapeError::Ledger`], [`ScrapeError::Geo`] and [`ScrapeError::Browser`] when the store,
    ///   a locale or the browser fails.
    #[tracing::instrument(skip_all, fields(url = %RedactedUrl::new(url)))]
    pub async fn fetch(&mut self, url: &str) -> Result<BrowserFetch, ScrapeError> {
        let target = Target::parse(url)?;
        if let Some(limits) = &self.wiring.limits {
            limits.acquire(&target.site).await;
        }
        let mut state = self
            .wiring
            .store
            .get(&target.site)
            .await?
            .unwrap_or_else(|| SiteRecord::new(&target.site));
        let now = self.now();
        if let Some(until) = state.cooldown_until.filter(|until| now < *until) {
            return Err(ScrapeError::CoolingDown {
                site: target.site,
                until,
            });
        }
        self.proxy.site = Some(target.site.clone());
        let outcome = self.attempts(&target, &mut state).await;
        let saved = self.wiring.store.put(state).await;
        let fetched = outcome?;
        saved?;
        Ok(fetched)
    }

    /// Reads the page now open, solves the challenge on it if there is one, and waits for the
    /// browser to show the clearance. Returns what the page is afterwards.
    ///
    /// The status and headers used for detection are those of the navigation that led to the page
    /// ([`fetch`](Self::fetch) loads it). A page that is not a challenge is returned as it is.
    ///
    /// A challenge with a widget is attempted with [`cdp::solve_widget`], after the pointer has
    /// arrived on the page like a person's. The core's solver presses a widget again when the page
    /// shows no result, up to three times; a widget that never clears is a challenge that stands,
    /// not an error. A script challenge needs no click: it is waited for. The wait ends when the
    /// vendor's clearance cookie holds a new value or, for a vendor without one, when the page
    /// stops being a challenge, and is bounded by the verify timeout of the policy's `solve` options,
    /// which the solver also uses, for each press, beside its other limits.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when the browser cannot be read or refuses the input.
    #[tracing::instrument(skip_all)]
    pub async fn solve_challenge(&mut self) -> Result<Detection, ScrapeError> {
        let before = self.read_current().await?;
        if before.detection.kind == ChallengeKind::Clear {
            return Ok(before.detection);
        }
        let vendor = before.detection.evidence.vendor;
        let stale = cdp::clearance_value(&self.page, vendor).await?;
        let has_widget = before.detection.evidence.widget.is_some();
        if has_widget || before.detection.kind == ChallengeKind::Interactive {
            cdp::human_warm_up(&self.page, self.persona.seed).await;
            cdp::solve_widget(&self.page, &self.wiring.policy.solve).await?;
        }
        cdp::wait_for_clearance(&self.page, vendor, stale.as_deref(), self.clearance_wait())
            .await?;
        Ok(self.read_current().await?.detection)
    }

    /// How long the browser is given to show a solved challenge: the verify timeout of the
    /// policy's solve options, as core's `SolveOptions::verify_timeout_allowed` reads it.
    pub(super) fn clearance_wait(&self) -> Duration {
        self.wiring.policy.solve.verify_timeout_allowed()
    }

    /// The page now open, judged with the answer that led to it.
    async fn read_current(&self) -> Result<PageSnapshot, ScrapeError> {
        let response = match &self.last {
            Some(response) => response.clone(),
            None => NavResponse {
                url: self.page.url().await?,
                status: 0,
                headers: Vec::new(),
            },
        };
        cdp::detect_page(&self.page, &response).await
    }

    /// Loads, decides and acts until the page is real or the policy stops.
    async fn attempts(
        &mut self,
        target: &Target,
        state: &mut SiteRecord,
    ) -> Result<BrowserFetch, ScrapeError> {
        let mut solved: Option<Solved> = None;
        let mut attempt = 0_u32;
        loop {
            attempt = attempt.saturating_add(1);
            let (response, snapshot) = self.load_once(target, state).await?;
            let action = self.decide(target, state, &snapshot.detection, attempt);
            if action == Mitigation::Solve && solved.is_none() {
                solved = Some(Solved {
                    kind: snapshot.detection.kind,
                    since: Instant::now(),
                });
            }
            match self.act(action, target, state, &snapshot, attempt).await? {
                Step::Finish => {
                    return Ok(self.finished(target, response, snapshot, attempt, solved));
                }
                Step::Again => {}
            }
        }
    }

    /// One load: navigates, reads the page, and records what happened.
    async fn load_once(
        &mut self,
        target: &Target,
        state: &mut SiteRecord,
    ) -> Result<(NavResponse, PageSnapshot), ScrapeError> {
        let started = Instant::now();
        let response = match cdp::goto_with_response(&self.page, &target.url).await {
            Ok(response) => response,
            Err(error) => {
                self.report(ProxyOutcome::ConnectFailed);
                return Err(error);
            }
        };
        let snapshot = cdp::detect_page(&self.page, &response).await?;
        let detection = &snapshot.detection;
        state.record(detection, FetchKind::Browser, self.now());
        state.last_proxy = self.current_key();
        self.emit(&ScrapeEvent::Request {
            url: target.redacted.clone(),
            status: response.status,
            fetch_kind: FetchKind::Browser,
            elapsed_ms: elapsed_ms(started),
        });
        if detection.kind != ChallengeKind::Clear {
            self.emit(&ScrapeEvent::ChallengeDetected {
                url: target.redacted.clone(),
                kind: detection.kind,
                vendor: detection.evidence.vendor,
                fetch_kind: FetchKind::Browser,
            });
        }
        self.report(outcome_of(detection.kind));
        self.last = Some(response.clone());
        Ok((response, snapshot))
    }

    /// Tells the pool how the proxy in use fared.
    fn report(&self, outcome: ProxyOutcome) {
        if let (Some(pool), Some(current)) = (&self.proxy.pool, &self.proxy.current) {
            pool.report(current.proxy.key(), outcome, self.now());
        }
    }

    /// Asks the mitigation policy what to do, and reports the answer.
    fn decide(
        &self,
        target: &Target,
        state: &SiteRecord,
        detection: &Detection,
        attempt: u32,
    ) -> Mitigation {
        let context = AttemptContext {
            attempt,
            fetch_kind: FetchKind::Browser,
            consecutive_challenges: state.consecutive_challenges,
            can_rotate_proxy: self.proxy.size > 1,
            can_escalate: false,
            now: self.now(),
        };
        let action = self.wiring.policy.decide(detection, &context);
        self.emit(&ScrapeEvent::ActionChosen {
            site: target.site.clone(),
            action,
        });
        action
    }

    /// Carries out `action`.
    pub(super) async fn act(
        &mut self,
        action: Mitigation,
        target: &Target,
        state: &mut SiteRecord,
        snapshot: &PageSnapshot,
        attempt: u32,
    ) -> Result<Step, ScrapeError> {
        match action {
            Mitigation::Proceed => return Ok(Step::Finish),
            Mitigation::Solve => {
                self.solve_challenge().await?;
            }
            Mitigation::Retry { after } => tokio::time::sleep(after).await,
            Mitigation::RotateProxy => {
                self.rotate_or_give_up(target, &snapshot.detection, attempt)
                    .await?;
            }
            Mitigation::RotateFingerprint => self.rotate_fingerprint().await?,
            Mitigation::Cooldown { until } => return Err(self.cool_down(target, state, until)),
            Mitigation::GiveUp(reason) => {
                return Err(give_up(target, &snapshot.detection, reason, attempt));
            }
            // An action this loop does not know ends it: nothing is retried blindly.
            _ => {
                return Err(give_up(
                    target,
                    &snapshot.detection,
                    GiveUpReason::AttemptsExhausted,
                    attempt,
                ));
            }
        }
        Ok(Step::Again)
    }

    /// Rotates the proxy; with none left to rotate to, the fetch gives up.
    async fn rotate_or_give_up(
        &mut self,
        target: &Target,
        detection: &Detection,
        attempt: u32,
    ) -> Result<(), ScrapeError> {
        match self.rotate_proxy().await {
            Err(ScrapeError::NoProxyLeft) => Err(give_up(
                target,
                detection,
                GiveUpReason::NoEgressLeft,
                attempt,
            )),
            other => other,
        }
    }

    /// Leaves the site alone until `until`, and says so.
    fn cool_down(&self, target: &Target, state: &mut SiteRecord, until: u64) -> ScrapeError {
        state.start_cooldown(until);
        self.emit(&ScrapeEvent::CooldownStarted {
            site: target.site.clone(),
            until,
        });
        ScrapeError::CoolingDown {
            site: target.site.clone(),
            until,
        }
    }

    /// The result of a fetch that ended on a real page.
    fn finished(
        &self,
        target: &Target,
        response: NavResponse,
        snapshot: PageSnapshot,
        attempts: u32,
        solved: Option<Solved>,
    ) -> BrowserFetch {
        if let Some(solved) = &solved {
            self.emit(&ScrapeEvent::ChallengeSolved {
                url: target.redacted.clone(),
                kind: solved.kind,
                attempts,
                elapsed_ms: elapsed_ms(solved.since),
            });
        }
        BrowserFetch {
            url: response.url,
            status: response.status,
            headers: response.headers,
            body: snapshot.body,
            detection: snapshot.detection,
            attempts,
            solved: solved.map(|solved| solved.kind),
            proxy: self.current_key(),
        }
    }
}

/// How the proxy fared, by what the site answered.
fn outcome_of(kind: ChallengeKind) -> ProxyOutcome {
    match kind {
        ChallengeKind::Clear => ProxyOutcome::Success,
        ChallengeKind::Blocked => ProxyOutcome::Blocked,
        _ => ProxyOutcome::Challenged,
    }
}

/// The error for a policy that stopped: an unpassed challenge is [`ScrapeError::Unsolved`].
pub(super) fn give_up(
    target: &Target,
    detection: &Detection,
    reason: GiveUpReason,
    attempts: u32,
) -> ScrapeError {
    let passable = matches!(
        detection.kind,
        ChallengeKind::JsChallenge | ChallengeKind::Interactive
    );
    if reason == GiveUpReason::AttemptsExhausted && passable {
        return ScrapeError::Unsolved {
            url: target.redacted.clone(),
            kind: detection.kind,
            attempts,
        };
    }
    ScrapeError::GaveUp {
        url: target.redacted.clone(),
        reason,
    }
}

/// Milliseconds since `started`, saturating.
fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
