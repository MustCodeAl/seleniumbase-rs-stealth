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

#[doc(inline)]
pub use seleniumbase_rs_evasion as evasion;

#[doc(inline)]
pub use seleniumbase_rs_evasion::{
    BaseCaseFingerprintExt, FingerprintPlugin, LaunchOptionsFingerprintExt, PageFingerprintExt,
    StealthPlan,
};

pub mod fingerprint;
pub use fingerprint::{
    BatteryOverride, ColorScheme, ConnectionOverride, FingerprintGenerator, FingerprintOptions,
    FingerprintOptionsBuilder, GeneratedFingerprint, ProfileFingerprint, ReducedMotion,
    ScreenConstraints, SpeechVoice,
};

pub mod bayesian;
pub use bayesian::{
    audit_coherence, BayesianSampler, CoherenceReport, DeviceCategory, FingerprintConverter,
    HeaderGenerator, RequestContext, SampleConstraints,
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
