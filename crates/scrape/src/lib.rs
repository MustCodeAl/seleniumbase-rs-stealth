//! Stealth scraping without TLS spoofing: one pure module per concern, and the runtime that does the I/O.
//!
//! The main types are re-exported here, so a caller does not need the module paths:
//! `BrowserScraper` and its builder, `ScrapeError`, the egress types (`ProxyRoute`, `ProxyKey`,
//! `ProxyPool`, `ProxySwitch`), the identity (`Identity`, `EscalationPolicy`), the cookie
//! jar (`CookieJar`), the challenge types (`Detection`, `ChallengeKind`, `MitigationPolicy`, ...)
//! and the events with their sinks (`ScrapeEvent`, `EventSink` and the three sinks that ship).
//! The pure modules stay as they were; the I/O sits under `runtime`.
//!
//! The scrape layer is experimental, like the rest of the stealth layer: its types may change
//! between releases.

#![forbid(unsafe_code)]

pub mod challenge;
pub mod egress;
pub mod error;
pub mod jar;
pub mod ledger;
pub mod passport;
pub mod runtime;
mod sync;

pub use challenge::detect::{ChallengeKind, Detection, ChallengeEvidence, ResponseView, ProtectionVendor};
pub use challenge::mitigation::{Mitigation, AttemptContext, GiveUpReason, MitigationPolicy, FetchKind};
pub use egress::geo::{CountryCode, GeoResolver, LocaleExt, StaticGeoResolver};
pub use egress::pool::{ProxyPool, RotationStrategy};
pub use egress::redact::{RedactedUrl, Secret};
pub use egress::route::{ProxyKey, ProxyRoute};
pub use error::ScrapeError;
pub use jar::store::CookieJar;
pub use ledger::events::{CollectingEventSink, EventSink, NoopEventSink, ScrapeEvent};
pub use ledger::state::{SiteRecord, InMemoryLedger, Ledger};
pub use passport::identity::{WireMeasurementRef, IdentityError, Identity};
pub use passport::policy::{EscalationPolicy, Escalation};
pub use runtime::browser_scraper::{
    BrowserFetch, BrowserLauncher, BrowserScraper, BrowserScraperBuilder, ChromeLauncher,
};
pub use runtime::proxy_switch::{ContextProxySwitch, ProxySwitch};
pub use runtime::json_store::JsonLedger;
pub use runtime::tracing_sink::TracingEventSink;
