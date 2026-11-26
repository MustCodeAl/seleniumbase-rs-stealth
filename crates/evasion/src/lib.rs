//! Browser evasion, fingerprint spoofing, capture/comparison, and profile
//! payload modules moved from `seleniumbase-rs` core in v0.10.0.
//!
//! Core-owned modules (`patcher`, `dprocess`, `reactor`, `behavior`, `humanize`,
//! and `cdp`) are re-exported from `seleniumbase-rs` so callers migrating from
//! `seleniumbase_rs::evasion::*` keep a single canonical namespace.

pub mod capture;
pub mod evasions;
pub mod fingerprint;
pub mod identity;
pub mod js;
pub mod mimic;
pub mod policy;
pub mod profile_payloads;
pub mod providers;
pub mod randomize;
pub mod settings;
pub mod spoof;
pub mod uc;

pub use seleniumbase_rs::browser::devtools as cdp;
pub use seleniumbase_rs::sb_cdp::human::{behavior, humanize};
pub use seleniumbase_rs::undetected::{dprocess, patcher, reactor};

pub use capture::{CapturedFingerprint, ProbeInfo};
pub use fingerprint::{
    BatteryProfile, BrandVersion, BrowserType, ClientHints, CoherenceReport, ConnectionProfile,
    Contradiction, Dimension, Fingerprint, FingerprintBuilder, FingerprintComparison,
    FingerprintError, Geolocation, HumanizeConfig, OsType, ProxyConfig, QuicMode, ScreenSize,
    SettingDiff, Severity, SpeechVoice, StealthPolicy, WebGl, WebRtcAddresses, WebRtcPolicy,
};
pub use identity::{
    BaseCaseFingerprintExt, FingerprintPlugin, LaunchOptionsFingerprintExt, PageFingerprintExt,
    StealthPlan,
};
pub use mimic::Specimen;
pub use patcher::{
    engine_spoofing_args, find_system_chrome, find_system_chrome_with, BinaryFormat,
    BinaryMarkerReport, BinaryPatchRule, ChromeBinaryPatcher, ChromedriverPatcher, EnginePatch,
    MarkerCounts, PatchPreset, PatchedBrowserBundle,
};
pub use profile_payloads::ProfileParams;
pub use providers::{
    default_registry, session_registry, EvasionConfig, EvasionContext, EvasionProvider,
    EvasionRegistry, SESSION_OMITTED_PROVIDERS,
};
pub use spoof::Spoof;

/// Shorthand for a built-in [`Fingerprint`] preset (`windows`, `macos`, `linux`,
/// `android`, `ios`, `safari`, `edge`).
#[macro_export]
macro_rules! fingerprint {
    (windows) => {
        $crate::Fingerprint::windows_desktop()
    };
    (macos) => {
        $crate::Fingerprint::macos_desktop()
    };
    (linux) => {
        $crate::Fingerprint::linux_desktop()
    };
    (android) => {
        $crate::Fingerprint::android_mobile()
    };
    (ios) => {
        $crate::Fingerprint::ios_mobile_safari()
    };
    (safari) => {
        $crate::Fingerprint::ios_mobile_safari()
    };
    (edge) => {
        $crate::Fingerprint::edge_desktop()
    };
}
