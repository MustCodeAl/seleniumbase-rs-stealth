//! Layered stealth module for [`seleniumbase_rs`].
//!
//! Import this crate when core's lightweight undetected mode
//! (`undetected::patcher`, `sb_cdp::human`) is not enough and you need
//! coherent browser identity, client-side JS/CDP masking, wire-level
//! TLS/HTTP2/HTTP3 impersonation, or resilient anti-bot sessions.
//!
//! Re-exports all of [`seleniumbase_rs`] alongside four strictly separated
//! layers, each shipping with sensible defaults and deep controls underneath:
//!
//! 1. [`identity`] *(always enabled, `--no-default-features`, zero BoringSSL)*:
//!    pure persona and fingerprint modeling: unified [`identity::fingerprint`]
//!    module ([`identity::Fingerprint`], `ProfileFingerprint`,
//!    `FingerprintGenerator`, `FingerprintOptions`, `BayesianSampler`,
//!    `HeaderGenerator`, `FingerprintConverter`, `audit_coherence`, `compare`,
//!    `consistency`, `schema`, plus submodules `spoof`, `policy`, `settings`,
//!    `mimic`, and `randomize`), `profile_payloads`, and probe `capture`.
//! 2. [`client`] *(feature `client`, pure Rust, zero BoringSSL)*: client-side
//!    browser masking (`StealthLayer` with `Default`, `balanced`, `strict`,
//!    `minimal`), 42-surface `StealthScriptBundle` / `StealthScriptOptions`,
//!    32 modular JS `EvasionProvider` passes, `StealthPlan` CDP overrides,
//!    engine extension traits (`LaunchOptions`, `BrowserConfig`, `Page`,
//!    `BaseCase`), and core Layer-0 re-exports.
//! 3. [`network`] *(feature `network-spec` zero BoringSSL; features `network`
//!    and `http3` opt into BoringSSL/wreq + standalone `h3d/`)*: pure wire
//!    `spec` tables (TLS `ClientHello`, JA3/JA4, H2, H3, headers, measured
//!    browsers), `proxy` pools & geo routing, wire `capture`, `WireFormat`,
//!    `MitmProxy`, `EphemeralCa`, and unified `http3` (`Http3Client`,
//!    `Http3Pool`, `capture`).
//! 4. [`session`] *(feature `session`, default, zero BoringSSL; `HttpSession`
//!    and `HybridSession` unlock with `network`)*: stateful anti-bot sessions
//!    ([`session::CdpSession`] for pure CDP, [`session::HttpSession`] for wire
//!    HTTP/2+3, and [`session::HybridSession`] for dual-mode HTTP and real Chrome
//!    behind `MitmProxy`), backed by `challenge` detection &
//!    `MitigationPolicy`, `CookieJar`, `Ledger`, and `Passport`.

#![forbid(unsafe_code)]

#[doc(inline)]
pub use seleniumbase_rs::*;

#[cfg(feature = "session")]
pub(crate) mod sync;

pub mod identity;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "network-spec")]
#[warn(
    clippy::pedantic,
    clippy::allow_attributes_without_reason,
    clippy::clone_on_ref_ptr,
    clippy::map_err_ignore,
    clippy::semicolon_outside_block,
    clippy::unused_result_ok,
    clippy::too_long_first_doc_paragraph
)]
pub mod network;

#[cfg(feature = "session")]
#[warn(
    clippy::pedantic,
    clippy::allow_attributes_without_reason,
    clippy::clone_on_ref_ptr,
    clippy::map_err_ignore,
    clippy::semicolon_outside_block,
    clippy::unused_result_ok,
    clippy::too_long_first_doc_paragraph
)]
pub mod session;
