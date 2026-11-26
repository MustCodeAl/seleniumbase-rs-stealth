//! One event of every kind, built from a URL that carries every secret redaction must remove.

use std::time::Duration;

use super::SessionEvent;
use crate::network::proxy::redact::RedactedUrl;
use crate::session::challenge::detect::{ChallengeKind, ProtectionVendor};
use crate::session::challenge::mitigation::{FetchKind, Mitigation};
use crate::session::passport::identity::fixtures::proxy;
use crate::session::passport::policy::{EscalateReason, Escalation};

/// A URL with userinfo, a sensitive query value and a fragment, each marked with a word the
/// tests look for.
pub(crate) const SECRET_URL: &str =
    "https://alice:s3cr3t-pass@example.com/path?page=2&token=SECRET-TOKEN#frag-secret";

/// The words in [`SECRET_URL`] that must never reach an event, a log line or an error.
pub(crate) const SECRETS: [&str; 4] = ["alice", "s3cr3t-pass", "SECRET-TOKEN", "frag-secret"];

fn url() -> RedactedUrl {
    RedactedUrl::new(SECRET_URL)
}

/// One event of every variant, in the order of the enum.
pub(crate) fn every_event() -> Vec<SessionEvent> {
    vec![
        SessionEvent::Request {
            url: url(),
            status: 200,
            fetch_kind: FetchKind::Http,
            elapsed_ms: 31,
        },
        SessionEvent::ChallengeDetected {
            url: url(),
            kind: ChallengeKind::JsChallenge,
            vendor: Some(ProtectionVendor::Cloudflare),
            fetch_kind: FetchKind::Http,
        },
        SessionEvent::ChallengeSolved {
            url: url(),
            kind: ChallengeKind::Interactive,
            attempts: 2,
            elapsed_ms: 4_500,
        },
        SessionEvent::ActionChosen {
            site: "example.com".to_owned(),
            action: Mitigation::Retry {
                after: Duration::from_millis(1_500),
            },
        },
        SessionEvent::FetchKindChanged {
            from: FetchKind::Http,
            to: FetchKind::Browser,
            reason: Escalation::Escalate(EscalateReason::Challenge(ChallengeKind::JsChallenge)),
        },
        SessionEvent::ProxyRotated {
            from: Some(proxy("p1.example")),
            to: None,
        },
        SessionEvent::FingerprintRotated {
            from_seed: 7,
            to_seed: 8,
        },
        SessionEvent::ClearanceExpiring {
            site: "example.com".to_owned(),
            remaining_secs: 90,
        },
        SessionEvent::CooldownStarted {
            site: "example.com".to_owned(),
            until: 1_800_000_900,
        },
        SessionEvent::IdentityExported { cookies: 3 },
        SessionEvent::IdentityRestored { cookies: 3 },
    ]
}
