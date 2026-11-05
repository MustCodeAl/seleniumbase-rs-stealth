//! The `TracingEventSink`, which writes redacted scraper events to `tracing`.
//!
//! Each event becomes one `tracing` event on the [`TARGET`] with an `event` field naming it and
//! one field per value. A [`ScrapeEvent`] holds only redacted URLs and egress keys, so nothing
//! secret is formatted here. The sink does no I/O of its own, so it does not block; the
//! subscriber decides where the line goes.

use std::fmt;

use crate::egress::route::ProxyKey;
use crate::ledger::events::{EventSink, ScrapeEvent};

#[cfg(test)]
mod tests;

/// The `tracing` target of every event this sink writes.
pub const TARGET: &str = "seleniumbase_rs_scrape";

/// An event sink that writes to `tracing`.
///
/// Requests and decisions are logged at `DEBUG`, challenges, changes of fetch_kind, egress or profile and
/// identity moves at `INFO`, and a cooldown at `WARN`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TracingEventSink;

/// An egress for a log field: `direct` or `scheme://host:port`.
struct Shown<'a>(&'a Option<ProxyKey>);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(key) => write!(f, "{key}"),
            None => f.write_str("direct"),
        }
    }
}

impl EventSink for TracingEventSink {
    fn emit(&self, event: &ScrapeEvent) {
        let name = event.name();
        match event {
            ScrapeEvent::Request {
                url,
                status,
                fetch_kind,
                elapsed_ms,
            } => {
                tracing::debug!(target: TARGET, event = name, %url, status, ?fetch_kind, elapsed_ms);
            }
            ScrapeEvent::ChallengeDetected {
                url,
                kind,
                vendor,
                fetch_kind,
            } => {
                tracing::info!(target: TARGET, event = name, %url, ?kind, ?vendor, ?fetch_kind);
            }
            ScrapeEvent::ChallengeSolved {
                url,
                kind,
                attempts,
                elapsed_ms,
            } => {
                tracing::info!(target: TARGET, event = name, %url, ?kind, attempts, elapsed_ms);
            }
            ScrapeEvent::ActionChosen { site, action } => {
                tracing::debug!(target: TARGET, event = name, %site, ?action);
            }
            ScrapeEvent::FetchKindChanged { from, to, reason } => {
                tracing::info!(target: TARGET, event = name, ?from, ?to, ?reason);
            }
            ScrapeEvent::ProxyRotated { from, to } => {
                tracing::info!(target: TARGET, event = name, from = %Shown(from), to = %Shown(to));
            }
            ScrapeEvent::FingerprintRotated { from_seed, to_seed } => {
                tracing::info!(target: TARGET, event = name, from_seed, to_seed);
            }
            ScrapeEvent::ClearanceExpiring {
                site,
                remaining_secs,
            } => {
                tracing::info!(target: TARGET, event = name, %site, remaining_secs);
            }
            ScrapeEvent::CooldownStarted { site, until } => {
                tracing::warn!(target: TARGET, event = name, %site, until);
            }
            ScrapeEvent::IdentityExported { cookies }
            | ScrapeEvent::IdentityRestored { cookies } => {
                tracing::info!(target: TARGET, event = name, cookies);
            }
        }
    }
}
