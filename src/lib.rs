//! Unified stealth and automation facade for `seleniumbase-rs`.
//!
//! Re-exports all of [`seleniumbase_rs`] (including `sb_cdp` and `stealth`)
//! alongside [`codec`], [`scrape`], and (when the `mimic` feature is enabled)
//! [`mimic`].

#![forbid(unsafe_code)]

#[doc(inline)]
pub use seleniumbase_rs::*;

#[doc(inline)]
pub use seleniumbase_rs::stealth::*;

#[cfg(feature = "codec")]
#[doc(inline)]
pub use seleniumbase_rs_codec as codec;

#[cfg(feature = "scrape")]
#[doc(inline)]
pub use seleniumbase_rs_scrape as scrape;

#[cfg(feature = "mimic")]
#[doc(inline)]
pub use seleniumbase_rs_mimic as mimic;
