//! Unified stealth and automation facade for `seleniumbase-rs`.
//!
//! Re-exports all of [`seleniumbase_rs`] (including `sb_cdp` and `evasion`)
//! alongside `codec` (feature `codec`), `scrape` (enabled by default), and
//! `mimic` (when the `mimic` feature is enabled). Use
//! `fingerprint::{CanvasNoise, Fingerprint, StealthPolicy}` for the core
//! fingerprint types through this facade.

#![forbid(unsafe_code)]

#[doc(inline)]
pub use seleniumbase_rs::*;

#[doc(inline)]
pub use seleniumbase_rs::evasion::*;

#[cfg(feature = "codec")]
#[doc(inline)]
pub use seleniumbase_rs_codec as codec;

#[cfg(feature = "scrape")]
#[doc(inline)]
pub use seleniumbase_rs_scrape as scrape;

#[cfg(feature = "mimic")]
#[doc(inline)]
pub use seleniumbase_rs_mimic as mimic;
