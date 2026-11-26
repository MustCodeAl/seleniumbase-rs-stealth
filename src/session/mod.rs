//! Layer 4 (`session`): stateful anti-bot sessions, challenge detection &
//! mitigation, cookie jar, per-site ledger, and passport persistence.
//!
//! - [`CdpSession`] *(default, zero BoringSSL)*: real Chrome over CDP with
//!   challenge solving, geo-aligned fingerprint/proxy rotation, and cooldowns.
//! - [`HttpSession`] *(feature `network`, plus `http3` for `Alt-Svc` H3)*:
//!   plain HTTP client continuing a browser's session with measured TLS/H2/H3.
//! - [`HybridSession`] *(feature `network`)*: dual-mode session automatically
//!   escalating between [`HttpSession`] and real Chrome behind `MitmProxy`.
//!
//! Pure domain modules (`challenge`, `jar`, `ledger`, `passport`) perform no
//! I/O or clock reads (verified by `tests/session_domain_purity.rs`); I/O
//! adapters live under [`io`].

pub mod cdp;
pub mod cdp_session;
pub mod challenge;
pub mod error;
#[cfg(feature = "network")]
pub mod http;
#[cfg(feature = "network")]
pub mod hybrid;
pub mod io;
pub mod jar;
pub mod ledger;
pub mod passport;

pub use cdp_session::{CdpFetch, CdpSession, CdpSessionBuilder, Launcher};
pub use challenge::detect::{
    ChallengeEvidence, ChallengeKind, Detection, ProtectionVendor, ResponseView,
};
pub use challenge::mitigation::{
    AttemptContext, FetchKind, GiveUpReason, Mitigation, MitigationPolicy,
};
pub use error::SessionError;
#[cfg(feature = "network")]
pub use http::{
    HttpAttempt, HttpError, HttpFetch, HttpRequest, HttpResponse, HttpRules, HttpSession,
    HttpSessionOptions, HttpVerdict, Method, ResponseHeaders,
};
#[cfg(feature = "network")]
pub use hybrid::{HybridError, HybridFetch, HybridSession, HybridSessionBuilder};
pub use io::json_store::JsonLedger;
pub use io::tracing_sink::TracingEventSink;
pub use jar::store::CookieJar;
pub use ledger::events::{CollectingEventSink, EventSink, NoopEventSink, SessionEvent};
pub use ledger::state::{InMemoryLedger, Ledger, SiteRecord};
pub use passport::identity::{Identity, IdentityError, WireMeasurementRef};
pub use passport::policy::{Escalation, EscalationPolicy};
