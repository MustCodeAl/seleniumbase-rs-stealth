//! The launch closure, and what the scraper reads from the browser it launched.

use std::fmt;

use std::pin::Pin;
use std::sync::Arc;

use serde_json::json;

use crate::error::ScrapeError;
use seleniumbase_rs::error::SeleniumBaseError;
use seleniumbase_rs::evasion::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

/// The protocol command that names the browser product and version.
const VERSION_COMMAND: &str = "Browser.getVersion";

/// The prefix of the `Chrome/155.0.0.0` token of a user agent or product string.
const CHROME_TOKEN: &str = "Chrome/";

/// Starts a browser for a [`BrowserScraper`](super::BrowserScraper) or a stealth session: a
/// launch closure, as core's `BrowserPool::with_launcher` takes.
///
/// The scraper starts a browser when it is built and again when it rotates its fingerprint. The
/// default is core's [`Browser::launch`]; another closure can start a different build,
/// reuse a browser, or hand out a scripted one in a test.
#[derive(Clone)]
pub struct Launcher(Arc<LaunchFn>);

/// The closure a [`Launcher`] holds, its future boxed.
type LaunchFn = dyn Fn(LaunchOptions) -> Pin<Box<dyn Future<Output = Result<Browser, SeleniumBaseError>> + Send>>
    + Send
    + Sync;

impl Launcher {
    /// A launcher that calls `launch`, a closure from the launch options to a browser.
    pub fn new<F, Fut>(launch: F) -> Self
    where
        F: Fn(LaunchOptions) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Browser, SeleniumBaseError>> + Send + 'static,
    {
        Self(Arc::new(move |options| Box::pin(launch(options))))
    }

    /// Starts a browser with `options`.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when no browser can be started or controlled.
    pub async fn launch(&self, options: LaunchOptions) -> Result<Browser, ScrapeError> {
        Ok((self.0)(options).await?)
    }
}

impl Default for Launcher {
    /// Core's [`Browser::launch`].
    fn default() -> Self {
        Self::new(|options: LaunchOptions| async move { Browser::launch(options).await })
    }
}

impl fmt::Debug for Launcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Launcher")
    }
}

/// The operating system this machine runs, as the persona of a real browser on it.
pub(super) fn host_os() -> OsType {
    if cfg!(target_os = "macos") {
        OsType::Macos
    } else if cfg!(target_os = "windows") {
        OsType::Windows
    } else {
        OsType::Linux
    }
}

/// The major version of the browser, from the product it reports, when it reports one.
pub(super) async fn browser_major(browser: &Browser) -> Option<u32> {
    let version = browser.execute(VERSION_COMMAND, json!({})).await.ok()?;
    major_of(version["product"].as_str()?)
}

/// The major version in a `Chrome/155.0.8059.40`, `HeadlessChrome/155.0.8059.40` or `Chromium/...`
/// token.
pub(super) fn major_of(text: &str) -> Option<u32> {
    let (_, after) = text.split_once('/')?;
    after.split('.').next()?.parse().ok()
}

/// The major version a fingerprint claims in its user agent.
pub(super) fn claimed_major(fingerprint: &Fingerprint) -> Option<u32> {
    let agent = fingerprint.user_agent().as_str()?;
    let (_, after) = agent.split_once(CHROME_TOKEN)?;
    after.split('.').next()?.parse().ok()
}

/// A fresh persona for `os` and `seed`: claiming the running browser's major when it is known.
pub(super) fn persona_for(os: OsType, seed: u64, major: Option<u32>) -> Fingerprint {
    match major {
        Some(major) => Fingerprint::randomized_with_major(os, seed, major),
        None => Fingerprint::randomized(os, seed),
    }
}
