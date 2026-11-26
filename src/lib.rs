//! Unified stealth and automation facade for `seleniumbase-rs`.
//!
//! Re-exports all of [`seleniumbase_rs`] (including `sb_cdp` and `evasion`)
//! alongside `codec` (feature `codec`), `scrape` (enabled by default), and
//! `wire` (when the `wire` feature is enabled). The core paths stay as they
//! are in `seleniumbase-rs`: `evasion::fingerprint::{CanvasNoise, Fingerprint,
//! StealthPolicy}`, `sb_cdp`, and so on.

#![forbid(unsafe_code)]

#[doc(inline)]
pub use seleniumbase_rs::*;

pub mod fingerprint;
pub use fingerprint::{
    ConnectionOverride, FingerprintGenerator, FingerprintOptions, FingerprintOptionsBuilder,
    GeneratedFingerprint, ProfileFingerprint, ScreenConstraints,
};

pub mod script;
pub use script::{StealthScriptBundle, StealthScriptOptions};

pub mod plugin;
pub use plugin::StealthPlugin;

pub mod ext;
pub use ext::{StealthBaseCaseExt, StealthPageExt};

#[cfg(feature = "codec")]
#[doc(inline)]
pub use seleniumbase_rs_codec as codec;

#[cfg(feature = "scrape")]
#[doc(inline)]
pub use seleniumbase_rs_scrape as scrape;

#[cfg(feature = "wire")]
#[doc(inline)]
pub use seleniumbase_rs_wire as wire;
