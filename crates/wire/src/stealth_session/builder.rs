//! The builder of a `StealthSession`, and how a session starts.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::pace::SiteLimits;
use super::rotate::{check_identity, persona_fingerprint};
use super::transition::open_leg;
use super::{Clock, Egress, StealthError, StealthSession, Wiring};
use crate::http_session::{HttpRules, HttpSessionOptions};
use crate::mitm::MitmOptions;
use seleniumbase_rs::browser::config::Browser as BrowserName;
use seleniumbase_rs::config::proxy_list::RotationStrategy;
use seleniumbase_rs::error::SeleniumBaseError;
use seleniumbase_rs::evasion::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_codec::measurement;
use seleniumbase_rs_scrape::challenge::mitigation::{
    MitigationPolicy, DEFAULT_MAX_CHALLENGE_ATTEMPTS,
};
use seleniumbase_rs_scrape::egress::geo::{CountryCode, GeoResolver, StaticGeoResolver};
use seleniumbase_rs_scrape::egress::pool::ProxyPool;
use seleniumbase_rs_scrape::egress::route::ProxyRoute;
use seleniumbase_rs_scrape::ledger::events::EventSink;
use seleniumbase_rs_scrape::ledger::state::{InMemoryLedger, Ledger};
use seleniumbase_rs_scrape::passport::identity::{Identity, WireMeasurementRef};
use seleniumbase_rs_scrape::passport::policy::EscalationPolicy;
use seleniumbase_rs_scrape::runtime::browser_scraper::Launcher;
use seleniumbase_rs_scrape::runtime::cdp::ProxyLocale;
use seleniumbase_rs_scrape::runtime::tracing_sink::TracingEventSink;

/// How many loads of one URL a session fetch makes before it gives up, unless told otherwise.
///
/// One load meets a challenge over HTTP, one in the browser solves it, and one more over HTTP
/// uses the clearance. A browser-only scraper needs [`DEFAULT_MAX_CHALLENGE_ATTEMPTS`].
pub const DEFAULT_ATTEMPTS: u32 = DEFAULT_MAX_CHALLENGE_ATTEMPTS + 1;

/// How long the MITM proxy is given to finish its requests when the session is closed.
pub const MITM_SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Configures a [`StealthSession`]; see [`StealthSession::builder`].
///
/// Every setting has a default: Chrome with the platform's launch options, no proxy, a state kept
/// in memory, events written to `tracing`, HTTP as the preferred mode, [`DEFAULT_ATTEMPTS`]
/// attempts per fetch, and a profile drawn from a random seed.
pub struct StealthSessionBuilder {
    launch: LaunchOptions,
    launcher: Option<Launcher>,
    egresses: Vec<ProxyRoute>,
    rotation: RotationStrategy,
    attempts: Option<u32>,
    mitigation: Option<MitigationPolicy>,
    policy: EscalationPolicy,
    store: Option<Arc<dyn Ledger>>,
    sink: Option<Arc<dyn EventSink>>,
    geo: Option<Arc<dyn GeoResolver>>,
    limits: Option<(usize, Duration)>,
    seed: Option<u64>,
    fingerprint: Option<Fingerprint>,
    restore: Option<Identity>,
    http: HttpSessionOptions,
    mitm: MitmOptions,

    clock: Option<Clock>,
}

impl fmt::Debug for StealthSessionBuilder {
    /// Counts the proxies instead of printing them, and leaves the identity out.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StealthSessionBuilder")
            .field("egresses", &self.egresses.len())
            .field("rotation", &self.rotation)
            .field("policy", &self.policy)
            .field("seed", &self.seed)
            .field("restore", &self.restore.is_some())
            .finish_non_exhaustive()
    }
}

impl StealthSessionBuilder {
    pub(super) fn new() -> Self {
        Self {
            launch: LaunchOptions::default(),
            launcher: None,
            egresses: Vec::new(),
            rotation: RotationStrategy::RoundRobin,
            attempts: None,
            mitigation: None,
            policy: EscalationPolicy::default(),
            store: None,
            sink: None,
            geo: None,
            limits: None,
            seed: None,
            fingerprint: None,
            restore: None,
            http: HttpSessionOptions::new(),
            mitm: MitmOptions::default(),

            clock: None,
        }
    }

    /// The launch options of the browser, as built. The session adds its fingerprint to a copy of them,
    /// so a setting made here (a user agent, say) wins over the fingerprint's. The proxy is the
    /// MITM proxy's, whatever is set here.
    #[must_use]
    pub fn launch_options(mut self, launch: LaunchOptions) -> Self {
        self.launch = launch;
        self
    }

    /// Starts the browser with `launch`, a closure from the launch options to a browser (as core's
    /// `BrowserPool::with_launcher` takes), instead of core's `Browser::launch`.
    #[must_use]
    pub fn launcher<F, Fut>(mut self, launch: F) -> Self
    where
        F: Fn(LaunchOptions) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Browser, SeleniumBaseError>> + Send + 'static,
    {
        self.launcher = Some(Launcher::new(launch));
        self
    }

    /// Adds proxies, each with the country it exits in. The country decides the language and the
    /// time zone the session presents while it leaves through that proxy.
    ///
    /// # Errors
    ///
    /// Returns [`StealthError::Scrape`] for a spec that is not `scheme://[user:password@]host:port`
    /// with a scheme of http, https, socks5 or socks5h. The error never echoes the credentials.
    pub fn geo_proxies<I, S>(mut self, proxies: I) -> Result<Self, StealthError>
    where
        I: IntoIterator<Item = (S, CountryCode)>,
        S: AsRef<str>,
    {
        for (spec, country) in proxies {
            self.egresses
                .push(ProxyRoute::parse(spec.as_ref(), Some(country))?);
        }
        Ok(self)
    }

    /// How the pool picks a proxy when one is needed, at the start and at each rotation: core's
    /// [`RotationStrategy`], round robin unless set here.
    #[must_use]
    pub fn proxy_rotation(mut self, rotation: RotationStrategy) -> Self {
        self.rotation = rotation;
        self
    }

    /// How many loads of one URL a fetch may make before it gives up. Zero is read as one.
    #[must_use]
    pub fn max_challenge_attempts(mut self, attempts: u32) -> Self {
        self.attempts = Some(attempts);
        self
    }

    /// The whole mitigation policy: when to rotate, how long to cool down. The attempts set by
    /// [`max_challenge_attempts`](Self::max_challenge_attempts) still apply on top;
    /// without either, the attempts are [`DEFAULT_ATTEMPTS`].
    #[must_use]
    pub fn mitigation(mut self, policy: MitigationPolicy) -> Self {
        self.mitigation = Some(policy);
        self
    }

    /// When the session changes mode: the mode it prefers, whether HTTP needs a clearance, how
    /// close to its end a clearance may get, and whether a solve sends the session back to HTTP.
    #[must_use]
    pub fn escalation_policy(mut self, policy: EscalationPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Where what is learned about each site is kept between fetches.
    #[must_use]
    pub fn ledger(mut self, store: Arc<dyn Ledger>) -> Self {
        self.store = Some(store);
        self
    }

    /// Where the session reports what it does. The HTTP session and the MITM proxy report to it
    /// as well.
    #[must_use]
    pub fn event_sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    /// How the country, time zone and language of an egress are found.
    #[must_use]
    pub fn geo_resolver(mut self, geo: Arc<dyn GeoResolver>) -> Self {
        self.geo = Some(geo);
        self
    }

    /// At most `calls` fetches of one site in `window`, in either mode. The limit is per site,
    /// not global.
    #[must_use]
    pub fn rate_limit(mut self, calls: usize, window: Duration) -> Self {
        self.limits = Some((calls, window));
        self
    }

    /// The seed of the first profile. The same seed gives the same profile, and each rotation
    /// derives the next seed from the last, so a run can be repeated.
    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// The first profile itself, instead of one drawn from the seed. It must claim the measured
    /// Chrome. A rotation replaces it with a generated profile of the same operating system.
    /// The rotations derive from [`seed`](Self::seed), not from the seed of this fingerprint.
    #[doc(alias = "profile")]
    #[must_use]
    pub fn fingerprint(mut self, fingerprint: Fingerprint) -> Self {
        self.fingerprint = Some(fingerprint);
        self
    }

    /// Starts from a saved identity: its fingerprint, its cookies and its clearances. It takes
    /// the place of [`fingerprint`](Self::fingerprint) and [`seed`](Self::seed).
    ///
    /// The identity must belong to one of the proxies added, or to the direct connection when
    /// none was: [`build`](Self::build) refuses an identity of another egress.
    #[must_use]
    pub fn identity(mut self, identity: Identity) -> Self {
        self.restore = Some(identity);
        self
    }

    /// The limits and plumbing of the HTTP leg: timeouts, body size, redirects, the trust store
    /// and the resolver entries. The clock and the event sink of the session replace the ones in
    /// these options.
    #[must_use]
    pub fn http_session_options(mut self, options: HttpSessionOptions) -> Self {
        self.http = options;
        self
    }

    /// The limits and custom inputs of the MITM proxy that carries the browser leg.
    ///
    /// Without an egress the proxy refuses loopback, private and link-local destinations, so a
    /// page cannot use it to reach the machine's own services. A site on the local network, or a
    /// loopback test server, needs
    /// [`MitmOptions::allow_private_destinations`](crate::mitm::MitmOptions::allow_private_destinations).
    #[doc(alias = "mitm")]
    #[must_use]
    pub fn mitm_options(mut self, options: MitmOptions) -> Self {
        self.mitm = options;
        self
    }

    /// The clock the session reads, in seconds since the Unix epoch. The default is the system
    /// clock.
    #[must_use]
    pub fn clock(mut self, clock: Clock) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Builds the session and starts its first leg: an HTTP session, or a browser behind the MITM
    /// proxy when the policy prefers the browser.
    ///
    /// # Errors
    ///
    /// - [`StealthError::WireFormat`] when this machine's system has no measured Chrome, or the
    ///   profile given claims another Chrome version than the measured one;
    /// - [`StealthError::Scrape`] when the country of an egress has no locale, a restored identity
    ///   belongs to another egress or is not valid, or no browser can be started;
    /// - [`StealthError::Http`] when the options or the egress are refused by the HTTP client;
    /// - [`StealthError::Mitm`] when the proxy cannot start.
    #[tracing::instrument(skip_all)]
    pub async fn build(self) -> Result<StealthSession, StealthError> {
        let (wiring, start) = self.into_parts();
        let now = (wiring.clock)();
        let pool = (!start.egresses.is_empty())
            .then(|| ProxyPool::from_routes(start.egresses.clone(), start.rotation))
            .transpose()?;
        let current = first_egress(&start, pool.as_ref(), now)?;
        let identity = start_identity(&wiring, &start, current.as_ref(), now).await?;
        let mode = wiring.rules.escalation.prefer;
        let mut mitm = None;
        let leg = open_leg(&wiring, &mut mitm, mode, &identity, current.as_ref()).await?;
        Ok(StealthSession {
            persona: super::Persona {
                // A fingerprint without random settings, such as a preset, has a seed derived
                // from its user agent, the same in every session; lending it would make every
                // session on that preset draw the same next profile on a rotation. Only an
                // identity that is restored brings its own seed.
                seed: if start.restore.is_some() {
                    identity.fingerprint.seed()
                } else {
                    start.seed
                },
                major: identity.wire_measurement.major,
                os: identity.wire_measurement.os,
            },
            wiring,
            identity,
            leg,
            mitm,
            egress: Egress {
                pool,
                size: start.egresses.len(),
                current,
                site: None,
            },
        })
    }

    /// Fills the defaults and splits the builder into the fixed wiring and the start settings.
    fn into_parts(self) -> (Wiring, Start) {
        let mut mitigation = self.mitigation.unwrap_or_else(|| MitigationPolicy {
            max_challenge_attempts: DEFAULT_ATTEMPTS,
            ..MitigationPolicy::default()
        });
        if let Some(attempts) = self.attempts {
            mitigation.max_challenge_attempts = attempts;
        }
        let clock = self.clock.unwrap_or_else(system_clock);
        let sink = self.sink.unwrap_or_else(|| Arc::new(TracingEventSink));
        let wiring = Wiring {
            launch: self.launch,
            launcher: self.launcher.unwrap_or_default(),
            store: self
                .store
                .unwrap_or_else(|| Arc::new(InMemoryLedger::new())),
            geo: self.geo.unwrap_or_else(|| Arc::new(StaticGeoResolver)),
            rules: HttpRules {
                mitigation,
                escalation: self.policy,
            },
            limits: self
                .limits
                .map(|(calls, window)| SiteLimits::new(calls, window)),
            http: self
                .http
                .clock(Arc::clone(&clock))
                .event_sink(Arc::clone(&sink)),
            mitm: self.mitm,

            sink,
            clock,
        };
        let start = Start {
            egresses: self.egresses,
            rotation: self.rotation,
            fingerprint: self.fingerprint,
            restore: self.restore,
            seed: self.seed.unwrap_or_else(random_seed),
        };
        (wiring, start)
    }
}

/// The settings that only matter while the session starts.
struct Start {
    egresses: Vec<ProxyRoute>,
    rotation: RotationStrategy,
    fingerprint: Option<Fingerprint>,
    restore: Option<Identity>,
    seed: u64,
}

/// The egress the session starts through: the one a restored identity belongs to, else the pool's
/// first pick. A restored identity of another egress is refused.
fn first_egress(
    start: &Start,
    pool: Option<&ProxyPool>,
    now: u64,
) -> Result<Option<ProxyRoute>, StealthError> {
    let Some(identity) = &start.restore else {
        return Ok(pool.and_then(|pool| pool.pick(None, now)));
    };
    identity.validate()?;
    let named = identity.proxy.as_ref().and_then(|key| {
        start.egresses.iter().find(|egress| {
            let held = egress.key();
            held.scheme == key.scheme && held.host == key.host && held.port == key.port
        })
    });
    let found = named.cloned();
    // An identity that names no egress that is held is checked against the first one: it belongs
    // to the direct connection or to a proxy that is not in this pool, and the check says so.
    let against = found.as_ref().or_else(|| start.egresses.first());
    check_identity(identity, against.map(ProxyRoute::key))?;
    Ok(found)
}

/// The identity the session starts with: the one restored, or one made for this machine that
/// claims the measured Chrome, in the locale of the first egress.
async fn start_identity(
    wiring: &Wiring,
    start: &Start,
    current: Option<&ProxyRoute>,
    now: u64,
) -> Result<Identity, StealthError> {
    if let Some(identity) = &start.restore {
        return Ok(identity.clone());
    }
    let mut fingerprint = if let Some(fingerprint) = &start.fingerprint {
        fingerprint.clone()
    } else {
        let major = measured_major(host_os())?;
        persona_fingerprint(host_os(), start.seed, major, None)?
    };
    if let Some(egress) = current {
        fingerprint = ProxyLocale::resolve(&*wiring.geo, egress)
            .await?
            .localize(&fingerprint)?;
    }
    let os = fingerprint.os_type();
    let emulation = WireMeasurementRef {
        kind: BrowserName::Chrome,
        major: measured_major(os)?,
        os,
    };
    Ok(Identity::new(
        fingerprint,
        emulation,
        current.map(|egress| egress.key().clone()),
        now,
    )?)
}

/// The Chrome major the measured TLS and HTTP/2 of `os` belong to.
fn measured_major(os: OsType) -> Result<u32, StealthError> {
    Ok(measurement::get(BrowserName::Chrome, os)
        .map_err(crate::client::WireFormatError::from)?
        .major)
}

/// The operating system this machine runs, as the persona of a real browser on it.
fn host_os() -> OsType {
    if cfg!(target_os = "macos") {
        OsType::Macos
    } else if cfg!(target_os = "windows") {
        OsType::Windows
    } else {
        OsType::Linux
    }
}

/// The system clock, in seconds since the Unix epoch.
fn system_clock() -> Clock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    })
}

/// A seed from the operating system's randomness.
fn random_seed() -> u64 {
    RandomState::new().build_hasher().finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use seleniumbase_rs_scrape::challenge::mitigation::FetchKind;

    #[test]
    fn a_fetch_that_needs_a_browser_gets_one_more_attempt_than_a_browser_alone() {
        assert_eq!(DEFAULT_ATTEMPTS, DEFAULT_MAX_CHALLENGE_ATTEMPTS + 1);
    }

    #[test]
    fn the_session_prefers_http_and_the_browser_is_not_started_unasked() {
        let builder = StealthSessionBuilder::new();
        assert_eq!(builder.policy.prefer, FetchKind::Http);
        assert!(builder.restore.is_none());
    }

    #[test]
    fn the_builder_shows_counts_and_no_proxy() {
        let builder = StealthSessionBuilder::new()
            .geo_proxies([(
                "http://alice:s3cret@proxy.example:8080",
                CountryCode::new("DE").expect("a country"),
            )])
            .expect("a proxy");
        let shown = format!("{builder:?}");
        assert!(shown.contains("egresses: 1"), "{shown}");
        for secret in ["alice", "s3cret"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
    }

    #[test]
    fn a_proxy_spec_that_is_not_valid_is_refused_without_echoing_it() {
        let error = StealthSessionBuilder::new()
            .geo_proxies([(
                "ftp://alice:s3cret@proxy.example:21",
                CountryCode::new("DE").expect("a country"),
            )])
            .expect_err("a scheme that is not a proxy");
        let shown = format!("{error} | {error:?}");
        assert!(!shown.contains("s3cret"), "{shown}");
    }

    #[test]
    fn the_host_system_has_a_measured_chrome_or_is_refused_by_name() {
        match measured_major(OsType::Macos) {
            Ok(major) => assert!(major > 0),
            Err(error) => panic!("macOS is measured on this repository: {error}"),
        }
        assert!(matches!(
            measured_major(OsType::Windows),
            Err(StealthError::WireFormat(_))
        ));
    }
}
