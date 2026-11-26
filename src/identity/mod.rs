//! Layer 1 (`identity`): pure browser persona & fingerprint data, synthesis,
//! Bayesian sampling, coherence auditing, recorded specimens, profile payloads,
//! and probe capture.
//!
//! Always enabled (`default-features = false`, pure Rust, zero JS/CDP
//! injection, zero BoringSSL).

pub mod capture;
pub mod fingerprint;
pub mod js;
pub mod mimic;
pub mod policy;
pub mod profile_payloads;
pub mod randomize;
pub mod settings;
pub mod spoof;

pub use capture::{CapturedFingerprint, ProbeInfo};
pub use fingerprint::{
    audit_coherence, AudioDynamicsCompressor, AudioMetadata, BatteryMetadata, BatteryOverride,
    BatteryProfile, BayesianSampler, BrandVersion, BrowserType, ClientHints,
    ClientHintsHighEntropy, CoherenceAudit, CoherenceReport, ColorGamut, ColorScheme,
    ConnectionOverride, ConnectionProfile, Contradiction, DeviceCategory, Dimension,
    DisplayMetadata, Fingerprint, FingerprintBuilder, FingerprintComparison, FingerprintConverter,
    FingerprintError, FingerprintGenerator, FingerprintOptions, FingerprintOptionsBuilder,
    GeneratedFingerprint, Geolocation, GpuMetadata, HdrMode, HeaderGenerator, HumanizeConfig,
    MediaDeviceInfo, MediaDeviceMetadata, NetworkMetadata, OsType, ProfileFingerprint, ProxyConfig,
    QuicMode, ReducedMotion, RequestContext, SampleConstraints, ScreenConstraints,
    ScreenOrientation, ScreenOrientationType, ScreenSize, SettingDiff, Severity, SpeechVoice,
    StealthPolicy, WebGl, WebGlParameters, WebGpuLimits, WebGpuMetadata, WebRtcAddresses,
    WebRtcPolicy,
};
pub use mimic::Specimen;
pub use profile_payloads::ProfileParams;
pub use spoof::Spoof;

/// Shorthand for a built-in [`Fingerprint`] preset (`windows`, `macos`, `linux`,
/// `android`, `ios`, `safari`, `edge`).
#[macro_export]
macro_rules! fingerprint {
    (windows) => {
        $crate::identity::Fingerprint::windows_desktop()
    };
    (macos) => {
        $crate::identity::Fingerprint::macos_desktop()
    };
    (linux) => {
        $crate::identity::Fingerprint::linux_desktop()
    };
    (android) => {
        $crate::identity::Fingerprint::android_mobile()
    };
    (ios) => {
        $crate::identity::Fingerprint::ios_mobile_safari()
    };
    (safari) => {
        $crate::identity::Fingerprint::ios_mobile_safari()
    };
    (edge) => {
        $crate::identity::Fingerprint::edge_desktop()
    };
}
