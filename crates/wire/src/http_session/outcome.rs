//! The outcome of a fetch in the terms the session policy acts on: is the reply a challenge, what
//! does the mitigation policy do about it, and does the session change mode.
//!
//! [`HttpSession::fetch`] sends the request and judges the reply in one call, and
//! [`HttpSession::assess`] judges a reply that [`HttpRequest::send`](super::HttpRequest::send)
//! already returned. [`HttpSession::preflight`] asks the other question, before the request:
//! whether the identity's clearance for the site is about to end, so that the session should go
//! to the browser first. The decisions are those of `scrape::challenge::mitigation` and
//! `scrape::passport::policy`; this module only fills in what an HTTP session knows.

use wreq::Method;

use super::error::HttpError;
use super::redirect::site_name;
use super::reply::HttpResponse;
use super::request::parse_url;
use super::HttpSession;
use seleniumbase_rs_scrape::challenge::detect::{ChallengeKind, Detection};
use seleniumbase_rs_scrape::challenge::mitigation::{
    AttemptContext, FetchKind, Mitigation, MitigationPolicy,
};
use seleniumbase_rs_scrape::ledger::events::ScrapeEvent;
use seleniumbase_rs_scrape::ledger::state::SiteRecord;
use seleniumbase_rs_scrape::passport::identity::Identity;
use seleniumbase_rs_scrape::passport::policy::{EscalateReason, Escalation, EscalationPolicy};

/// Where a fetch stands, as the mitigation policy needs to know it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpAttempt {
    /// The attempt at this URL, counting from one.
    pub number: u32,
    /// The challenges the site showed in a row before this reply.
    pub previous_challenges: u32,
    /// Whether another egress is available.
    pub can_rotate_proxy: bool,
    /// Whether a browser is available to take over.
    pub can_escalate: bool,
}

impl Default for HttpAttempt {
    /// The first attempt, no earlier challenge, no other egress, a browser available.
    fn default() -> Self {
        Self {
            number: 1,
            previous_challenges: 0,
            can_rotate_proxy: false,
            can_escalate: true,
        }
    }
}

/// The two policies that judge a reply.
#[derive(Clone, Copy, Debug, Default)]
pub struct HttpRules {
    /// What to do about a challenge.
    pub mitigation: MitigationPolicy,
    /// When the session changes mode.
    pub escalation: EscalationPolicy,
}

/// What a reply means for the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpVerdict {
    /// Whether the reply is a challenge, and the evidence.
    pub detection: Detection,
    /// What the mitigation policy chose.
    pub action: Mitigation,
    /// Whether the session changes mode because of it.
    pub escalation: Escalation,
}

impl HttpVerdict {
    /// Whether the reply is a real answer: it is not a challenge, a block or a rate limit.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.detection.kind == ChallengeKind::Clear
    }
}

/// A reply and the verdict on it.
#[derive(Clone, Debug)]
pub struct HttpFetch {
    /// The reply.
    pub reply: HttpResponse,
    /// What it means for the session.
    pub verdict: HttpVerdict,
}

impl HttpSession {
    /// Judges a reply: detects a challenge, asks the mitigation policy what to do, and asks the
    /// session policy whether that changes the mode. Emits `ChallengeDetected` and
    /// `ActionChosen` events.
    ///
    /// `identity` is the identity the session works for; the session is in HTTP mode, so only
    /// an escalation can come out of it.
    #[must_use]
    pub fn assess(
        &self,
        reply: &HttpResponse,
        attempt: &HttpAttempt,
        rules: &HttpRules,
        identity: &Identity,
    ) -> HttpVerdict {
        let detection = reply.detect();
        let challenged = detection.kind != ChallengeKind::Clear;
        let context = AttemptContext {
            attempt: attempt.number,
            fetch_kind: FetchKind::Http,
            consecutive_challenges: attempt
                .previous_challenges
                .saturating_add(u32::from(challenged)),
            can_rotate_proxy: attempt.can_rotate_proxy,
            can_escalate: attempt.can_escalate,
            now: self.now(),
        };
        let action = rules.mitigation.decide(&detection, &context);
        let transition =
            rules
                .escalation
                .after_fetch(FetchKind::Http, &detection, &action, identity);
        if challenged {
            self.emit(&ScrapeEvent::ChallengeDetected {
                url: reply.redacted_url(),
                kind: detection.kind,
                vendor: detection.evidence.vendor,
                fetch_kind: FetchKind::Http,
            });
            self.emit(&ScrapeEvent::ActionChosen {
                site: site_name(reply.url()),
                action,
            });
        }
        HttpVerdict {
            detection,
            action,
            escalation: transition,
        }
    }

    /// Fetches `url` with a `GET` and judges the reply; see [`assess`](Self::assess).
    ///
    /// # Errors
    ///
    /// The errors of [`HttpRequest::send`](super::HttpRequest::send).
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn fetch(
        &self,
        url: &str,
        attempt: &HttpAttempt,
        rules: &HttpRules,
        identity: &Identity,
    ) -> Result<HttpFetch, HttpError> {
        let reply = self.request(Method::GET, url).send().await?;
        let verdict = self.assess(&reply, attempt, rules, identity);
        Ok(HttpFetch { reply, verdict })
    }

    /// Asks, before a request to `url`, whether the session should go to the browser first
    /// because the identity's clearance for the site is about to end, is missing and required, or
    /// the site's history says plain requests do not work.
    ///
    /// The identity is first brought up to date with the cookies of this session. Emits
    /// `ClearanceExpiring` when that is the reason.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Url`] when `url` cannot be requested.
    pub fn preflight(
        &self,
        url: &str,
        identity: &Identity,
        state: Option<&SiteRecord>,
        rules: &HttpRules,
    ) -> Result<Escalation, HttpError> {
        let url = parse_url(url)?;
        let site = site_name(&url);
        let now = self.now();
        let current = self.identity_at(identity, now);
        let transition =
            rules
                .escalation
                .before_fetch(FetchKind::Http, &current, &site, state, now);
        if let Escalation::Escalate(EscalateReason::ClearanceExpiring { remaining_secs }) =
            transition
        {
            self.emit(&ScrapeEvent::ClearanceExpiring {
                site,
                remaining_secs,
            });
        }
        Ok(transition)
    }

    /// The identity `base` with the cookies of this session in it, and the clearances found in
    /// them. Emits `IdentityExported`.
    #[must_use]
    pub fn export_identity(&self, base: &Identity) -> Identity {
        let exported = self.identity_at(base, self.now());
        self.emit(&ScrapeEvent::IdentityExported {
            cookies: exported.cookies.len(),
        });
        exported
    }

    /// `base` with this session's cookies, its clearances refreshed at `now`.
    ///
    /// A clearance that ended by the clock since `base` recorded it stays on the list, so the
    /// policy sees an ended clearance and sends the session to the browser, instead of seeing no
    /// clearance at all. One the site removed early is gone from the jar and from the list.
    fn identity_at(&self, base: &Identity, now: u64) -> Identity {
        let mut identity = base.clone();
        identity.cookies = self.inner().jar.snapshot();
        let earlier = std::mem::take(&mut identity.clearances);
        identity.refresh_clearances(now);
        for ended in earlier.into_iter().filter(|c| c.expires_at <= now) {
            let known = identity
                .clearances
                .iter()
                .any(|c| c.site == ended.site && c.cookie == ended.cookie);
            if !known {
                identity.clearances.push(ended);
            }
        }
        identity
    }
}
