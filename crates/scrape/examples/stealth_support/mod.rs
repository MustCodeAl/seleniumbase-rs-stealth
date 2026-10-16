//! What the stealth examples share. This folder is a module of theirs, not an example: Cargo only
//! builds `examples/*.rs` and `examples/*/main.rs`, and this folder has neither a `main.rs` nor
//! an entry in `Cargo.toml`.
//!
//! Every example declares it with `mod stealth_support;` and uses the part it needs. The shop
//! that plays a protected site needs the TLS server of the mimic crate, so it lives in that
//! crate's examples.

use std::time::{SystemTime, UNIX_EPOCH};

/// The seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
