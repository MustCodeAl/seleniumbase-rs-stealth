//! Layer 2 (`client`): client-side browser masking (`StealthLayer`, 42-surface
//! `StealthScriptBundle` / `StealthScriptOptions`, 32 modular JS
//! `EvasionProvider` passes, `StealthPlan` CDP protocol overrides, launch/page
//! extension traits, and core Layer-0 re-exports).
//!
//! Pure Rust, zero BoringSSL (`features = ["client"]`, included by default).
//!
//! # Quick start (good default)
//!
//! ```no_run
//! use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};
//! use seleniumbase_rs_stealth::client::{LaunchOptionsFingerprintExt, StealthLayer};
//!
//! # async fn demo() -> seleniumbase_rs_stealth::Result<()> {
//! let layer = StealthLayer::default();
//! let options = LaunchOptions::default().with_stealth(&layer)?;
//! let browser = Browser::launch(options).await?;
//! browser.close().await?;
//! # Ok(())
//! # }
//! ```

pub mod apply;
pub mod evasions;
pub mod providers;
pub mod script;
pub mod uc;

pub use seleniumbase_rs::browser::devtools as cdp;
pub use seleniumbase_rs::sb_cdp::human::{behavior, humanize};
pub use seleniumbase_rs::undetected::{dprocess, patcher, reactor};

pub use apply::{
    BaseCaseFingerprintExt, BrowserConfigFingerprintExt, LaunchOptionsFingerprintExt,
    PageFingerprintExt, PageScriptTarget, StealthLayer, StealthPlan,
};
pub use patcher::{
    BinaryFormat, BinaryMarkerReport, BinaryPatchRule, ChromeBinaryPatcher, ChromedriverPatcher,
    EnginePatch, MarkerCounts, PatchPreset, PatchedBrowserBundle, engine_spoofing_args,
    find_system_chrome, find_system_chrome_with,
};
pub use providers::{
    EvasionConfig, EvasionContext, EvasionProvider, EvasionRegistry, SESSION_OMITTED_PROVIDERS,
    default_registry, session_registry,
};
pub use script::{StealthScriptBundle, StealthScriptOptions};
