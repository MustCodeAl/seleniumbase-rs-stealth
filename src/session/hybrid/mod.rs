//! `HybridSession`: the dual-mode session that moves between plain HTTP and a real browser.
//!
//! A session presents one [`Identity`] (a fingerprint, the measured browser it belongs to,
//! the cookies it earned and the egress they were earned through) in one of two modes:
//!
//! - [`FetchKind::Http`]: an [`HttpSession`] sends the requests. It is fast and cheap, and its TLS
//!   `ClientHello`, HTTP/2 start and headers are those of the measured Chrome.
//! - [`FetchKind::Browser`]: a real Chrome sends them, behind the [`MitmProxy`], which ends the
//!   browser's TLS and replays each request with the same measured `ClientHello` and HTTP/2 start.
//!   Both modes therefore look alike on the wire, and a clearance earned in one is valid in the
//!   other.
//!
//! # The loop
//!
//! A session starts in the mode its [`EscalationPolicy`] prefers (HTTP unless told otherwise) and
//! [`fetch`](HybridSession::fetch) asks the policies three questions:
//!
//! 1. Before the request, [`EscalationPolicy::before_fetch`]: does the identity hold a clearance for
//!    the site that is about to end, or must it hold one and does not, or does the site's history
//!    say plain requests fail? Then the session goes to the browser first.
//! 2. After the answer, [`MitigationPolicy::decide`]: a real page ends the loop; a challenge met in
//!    HTTP mode sends the session to the browser ([`Escalation::Escalate`]); in the browser the
//!    challenge is solved and the cookies move into the identity; a block or a rate limit starts a
//!    cooldown, rotates the egress or the profile, or stops the fetch.
//! 3. After a solve, [`EscalationPolicy::after_fetch`]: a browser that earned a clearance hands the
//!    identity back to HTTP ([`Escalation::Demote`]), which carries it on from there.
//!
//! Every change of mode emits [`SessionEvent::FetchKindChanged`] with its reason. The browser is closed
//! when the session goes back to HTTP and is started again, with the same identity and egress,
//! when it is needed. The MITM proxy lives from the first time a browser is needed until the
//! session is closed, so its port and its certificate stay the same across browsers.
//!
//! ```no_run
//! use seleniumbase_rs_stealth::session::HybridSession;
//! use seleniumbase_rs_stealth::network::proxy::geo::CountryCode;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let mut session = HybridSession::builder()
//!     .geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::try_new("DE")?)])?
//!     .build()
//!     .await?;
//! let page = session.fetch("https://example.com/").await?;
//! println!("{} bytes, {:?} mode", page.body.len(), page.fetch_kind);
//! session.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Egress and identity
//!
//! Both legs share one egress: the HTTP session connects through it and the MITM proxy sends the
//! browser's requests through it. A clearance belongs to the address that earned it, so changing
//! the egress ([`rotate_proxy`](HybridSession::rotate_proxy)) drops the cookies and the
//! clearances, and an identity can only be restored through the egress it names
//! ([`HybridSessionBuilder::identity`], [`HybridSession::import_identity`]).
//!
//! # Lifetime
//!
//! A launched browser lives until the session leaves the browser mode or is closed. There is no
//! idle timeout and no deadline in the session: it can sit unused for as long as the process lives
//! and be used again.
//!
//! # What it does not do
//!
//! A matching TLS and HTTP/2 start is one signal among many, and a session does not make
//! automation permitted by a site's terms. The measured Chrome and the installed Chrome must have
//! the same major version: a browser that is another version is refused before it loads a page.
//!
//! [`EscalationPolicy`]: crate::session::passport::policy::EscalationPolicy
//! [`EscalationPolicy::before_fetch`]: crate::session::passport::policy::EscalationPolicy::before_fetch
//! [`EscalationPolicy::after_fetch`]: crate::session::passport::policy::EscalationPolicy::after_fetch
//! [`MitigationPolicy::decide`]: crate::session::challenge::mitigation::MitigationPolicy::decide
//! [`Escalation::Escalate`]: crate::session::passport::policy::Escalation::Escalate
//! [`Escalation::Demote`]: crate::session::passport::policy::Escalation::Demote

mod browser;
mod builder;
mod error;
mod fetch;
mod rotate;
mod transition;

use std::fmt;
use std::sync::Arc;

pub use self::builder::{DEFAULT_ATTEMPTS, HybridSessionBuilder, MITM_SHUTDOWN_GRACE};
pub use self::error::HybridError;
pub use self::fetch::HybridFetch;
pub use super::http::Clock;

use self::browser::BrowserLeg;
use super::http::{HttpRules, HttpSession, HttpSessionOptions};
use crate::identity::fingerprint::OsType;
use crate::network::mitm::{MitmOptions, MitmProxy};
use crate::network::proxy::geo::GeoResolver;
use crate::network::proxy::pool::ProxyPool;
use crate::network::proxy::route::{ProxyKey, ProxyRoute};
use crate::session::cdp_session::Launcher;
use crate::session::challenge::mitigation::FetchKind;
use crate::session::ledger::events::{EventSink, SessionEvent};
use crate::session::ledger::state::Ledger;
use crate::session::pace::SiteLimits;
use crate::session::passport::identity::Identity;
use seleniumbase_rs::sb_cdp::{LaunchOptions, Page};

/// The ports and settings that are fixed when the session is built.
struct Wiring {
    launch: LaunchOptions,
    launcher: Launcher,
    store: Arc<dyn Ledger>,
    sink: Arc<dyn EventSink>,
    geo: Arc<dyn GeoResolver>,
    rules: HttpRules,
    limits: Option<SiteLimits>,
    http: HttpSessionOptions,
    mitm: MitmOptions,
    clock: Clock,
}

/// The browser profile the session presents.
struct Persona {
    seed: u64,
    /// The Chrome major the measured TLS and HTTP/2 belong to.
    major: u32,
    os: OsType,
}

/// The egress pool and the egress in use.
struct Egress {
    pool: Option<ProxyPool>,
    size: usize,
    current: Option<ProxyRoute>,
    /// The site fetched last, so a rotation can keep a site on one egress when the pool does.
    site: Option<String>,
}

/// The transport that sends the requests now.
enum Leg {
    Http(HttpSession),
    Browser(BrowserLeg),
}

impl Leg {
    /// The mode the leg stands for.
    fn fetch_kind(&self) -> FetchKind {
        match self {
            Self::Http(_) => FetchKind::Http,
            Self::Browser(_) => FetchKind::Browser,
        }
    }

    /// Closes the browser of a browser leg. An HTTP leg has nothing to close.
    async fn close(self) {
        if let Self::Browser(leg) = self {
            leg.close().await;
        }
    }
}

/// A scraping session that moves between HTTP and a real browser. See the
/// [module documentation](self).
pub struct HybridSession {
    wiring: Wiring,
    persona: Persona,
    identity: Identity,
    leg: Leg,
    /// The proxy the browser leg runs behind, once a browser has been needed.
    mitm: Option<MitmProxy>,
    egress: Egress,
}

impl fmt::Debug for HybridSession {
    /// The mode, the seed and the egress key; never a proxy credential, a token or a cookie.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridSession")
            .field("mode", &self.leg.fetch_kind())
            .field("seed", &self.persona.seed)
            .field("egress", &self.egress_key())
            .field("egresses", &self.egress.size)
            .field("mitm", &self.mitm.is_some())
            .finish_non_exhaustive()
    }
}

impl HybridSession {
    /// Starts configuring a session.
    #[must_use]
    pub fn builder() -> HybridSessionBuilder {
        HybridSessionBuilder::new()
    }

    /// The mode the requests are sent in now.
    #[must_use]
    pub fn fetch_kind(&self) -> FetchKind {
        self.leg.fetch_kind()
    }

    /// The identity as of the last change of mode, rotation, import or restore.
    ///
    /// The cookies the current leg has gathered since are not in it;
    /// [`export_identity`](Self::export_identity) reads them.
    #[must_use]
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// The egress both legs leave through; `None` for the direct connection. It carries no
    /// credentials.
    #[must_use]
    pub fn proxy_route(&self) -> Option<&ProxyKey> {
        self.egress_key()
    }

    /// The noise seed of the profile in use.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.persona.seed
    }

    /// The HTTP session, in HTTP mode: for requests of your own, such as an API call, that carry
    /// the cookies the session has earned. `None` in browser mode.
    #[must_use]
    pub fn http_session(&self) -> Option<&HttpSession> {
        match &self.leg {
            Leg::Http(http) => Some(http),
            Leg::Browser(_) => None,
        }
    }

    /// The tab of the browser, in browser mode. `None` in HTTP mode.
    #[must_use]
    pub fn page(&self) -> Option<&Page> {
        match &self.leg {
            Leg::Browser(leg) => Some(leg.page()),
            Leg::Http(_) => None,
        }
    }

    /// Closes the browser if one is running, and stops the MITM proxy.
    ///
    /// A session that is dropped instead leaves both to their own drop.
    ///
    /// # Errors
    ///
    /// Returns [`HybridError::Scrape`] when the browser cannot be told to close. The proxy is
    /// stopped either way.
    #[tracing::instrument(skip_all)]
    pub async fn close(self) -> Result<(), HybridError> {
        let closed = match self.leg {
            Leg::Browser(leg) => leg.close_checked().await,
            Leg::Http(_) => Ok(()),
        };
        if let Some(mitm) = self.mitm {
            mitm.shutdown(MITM_SHUTDOWN_GRACE).await;
        }
        closed.map_err(HybridError::from)
    }

    /// The time now in seconds since the Unix epoch, from the session's clock.
    fn now(&self) -> u64 {
        (self.wiring.clock)()
    }

    /// Hands an event to the sink.
    fn emit(&self, event: &SessionEvent) {
        self.wiring.sink.emit(event);
    }

    /// The key of the egress in use.
    fn egress_key(&self) -> Option<&ProxyKey> {
        self.egress.current.as_ref().map(ProxyRoute::key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_send<T: Send>(_value: T) {}

    /// Never called: it only has to compile, which proves the session and the futures of its
    /// methods can be moved to another task.
    #[expect(
        dead_code,
        reason = "compiled to prove the futures are Send, never run"
    )]
    fn the_session_can_be_driven_from_a_spawned_task(mut session: HybridSession) {
        is_send(session.fetch("https://example.com/"));
        is_send(session.escalate());
        is_send(session.demote());
        is_send(session.rotate_fingerprint());
        is_send(session.rotate_proxy());
        is_send(session.export_identity());
        is_send(HybridSession::builder().build());
        is_send(session);
    }
}
