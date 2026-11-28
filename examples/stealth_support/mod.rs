//! Shared helpers for runnable examples.
#![allow(
    dead_code,
    reason = "each example uses a part of this module, so the rest looks unused to it"
)]

use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "network")]
pub mod shop;

#[cfg(feature = "network")]
use seleniumbase_rs_stealth::Browser;
#[cfg(feature = "network")]
use seleniumbase_rs_stealth::identity::fingerprint::OsType;
#[cfg(feature = "network")]
use seleniumbase_rs_stealth::network::spec::measurement;

/// The operating system this machine runs, as the persona of a real browser on it.
#[cfg(feature = "network")]
pub fn host_os() -> OsType {
    if cfg!(target_os = "macos") {
        OsType::Macos
    } else if cfg!(target_os = "windows") {
        OsType::Windows
    } else {
        OsType::Linux
    }
}

/// The seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The system of this machine and the Chrome major version that was measured on it.
#[cfg(feature = "network")]
pub fn measured_chrome() -> Result<(OsType, u32), String> {
    let os = host_os();
    match measurement::get(Browser::Chrome, os) {
        Ok(entry) => Ok((os, entry.major)),
        Err(error) => Err(format!(
            "{error}.\nThis example needs a measured system, and only Chrome 155 on macOS is \
             measured today. See \"Re-measuring when Chrome updates\" in \
             docs/tutorials/stealth_scraping.md of seleniumbase-rs."
        )),
    }
}
