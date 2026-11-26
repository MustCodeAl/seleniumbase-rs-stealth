//! Stealth Plugin for `seleniumbase-rs`.
//!
//! Provides [`StealthPlugin`], a plugin conforming to [`seleniumbase_rs::plugin_api::Plugin`]
//! and offering standalone script injection hooks for both WebDriver and pure CDP browser engines.

use std::sync::Arc;

use seleniumbase_rs::evasion::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, HostScope, LaunchSettings, Metadata, PageHandle, Plugin,
    Requirements, Result as PluginResult,
};

use crate::fingerprint::{FingerprintGenerator, GeneratedFingerprint, ProfileFingerprint};
use crate::script::{StealthScriptBundle, StealthScriptOptions};

/// Stealth plugin that configures launch settings and injects evasions on page creation.
#[derive(Clone, Debug)]
pub struct StealthPlugin {
    name: String,
    bundle: StealthScriptBundle,
    fingerprint: Option<Fingerprint>,
    profile: Option<ProfileFingerprint>,
}

impl StealthPlugin {
    /// Creates a stealth plugin from a core [`Fingerprint`].
    #[must_use]
    pub fn new(fingerprint: Fingerprint) -> Self {
        let bundle = StealthScriptBundle::new(&fingerprint);
        let profile = ProfileFingerprint::from_fingerprint(&fingerprint);
        Self {
            name: "stealth".to_string(),
            bundle,
            fingerprint: Some(fingerprint),
            profile: Some(profile),
        }
    }

    /// Creates a stealth plugin from a [`ProfileFingerprint`].
    #[must_use]
    pub fn from_profile(profile: ProfileFingerprint) -> Self {
        let bundle = StealthScriptBundle::from_profile(&profile);
        Self {
            name: "stealth".to_string(),
            bundle,
            fingerprint: None,
            profile: Some(profile),
        }
    }

    /// Creates a stealth plugin from a [`GeneratedFingerprint`].
    #[must_use]
    pub fn from_generated(generated: GeneratedFingerprint) -> Self {
        let bundle = StealthScriptBundle::from_generated(&generated);
        Self {
            name: "stealth".to_string(),
            bundle,
            fingerprint: Some(generated.fingerprint),
            profile: Some(generated.profile),
        }
    }

    /// Creates a randomized stealth plugin seeded by `seed` for Windows.
    #[must_use]
    pub fn randomized(seed: u64) -> Self {
        Self::randomized_for(OsType::Windows, seed)
    }

    /// Creates a randomized stealth plugin for `os` seeded by `seed`.
    #[must_use]
    pub fn randomized_for(os: OsType, seed: u64) -> Self {
        let generated = FingerprintGenerator::new(seed).os(os).generate();
        Self::from_generated(generated)
    }

    /// Rebuilds the plugin's [`StealthScriptBundle`] using selective [`StealthScriptOptions`].
    #[must_use]
    pub fn with_script_options(mut self, options: &StealthScriptOptions) -> Self {
        if let Some(profile) = &self.profile {
            self.bundle = StealthScriptBundle::from_profile_with(profile, options);
        } else if let Some(fp) = &self.fingerprint {
            let profile = ProfileFingerprint::from_fingerprint(fp);
            self.bundle = StealthScriptBundle::from_profile_with(&profile, options);
            self.profile = Some(profile);
        }
        self
    }

    /// Returns the plugin's unique name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the compiled stealth script bundle.
    #[must_use]
    pub fn bundle(&self) -> &StealthScriptBundle {
        &self.bundle
    }

    /// Returns a reference to the active profile, if available.
    #[must_use]
    pub fn profile(&self) -> Option<&ProfileFingerprint> {
        self.profile.as_ref()
    }

    /// Returns a reference to the core fingerprint, if available.
    #[must_use]
    pub fn fingerprint(&self) -> Option<&Fingerprint> {
        self.fingerprint.as_ref()
    }

    /// Injects the stealth script bundle into any page-like handle implementing [`PageScriptTarget`].
    ///
    /// # Errors
    ///
    /// Returns [`seleniumbase_rs::error::SeleniumBaseError`] if script evaluation fails.
    pub async fn inject_script<P: ?Sized + PageScriptTarget>(
        &self,
        page: &P,
    ) -> Result<(), seleniumbase_rs::error::SeleniumBaseError> {
        page.add_init_script(self.bundle.script()).await
    }
}

/// Target interface for injecting initialization scripts.
pub trait PageScriptTarget {
    /// Injects an initialization script into the page.
    fn add_init_script(
        &self,
        script: &str,
    ) -> impl std::future::Future<Output = Result<(), seleniumbase_rs::error::SeleniumBaseError>> + Send;
}

impl PageScriptTarget for seleniumbase_rs::sb_cdp::Page {
    async fn add_init_script(
        &self,
        script: &str,
    ) -> Result<(), seleniumbase_rs::error::SeleniumBaseError> {
        self.execute(
            "Page.addScriptToEvaluateOnNewDocument",
            serde_json::json!({ "source": script }),
        )
        .await?;
        Ok(())
    }
}

fn convert_webrtc_policy(
    policy: seleniumbase_rs::evasion::fingerprint::WebRtcPolicy,
) -> seleniumbase_rs::plugin_api::WebRtcPolicy {
    match policy {
        seleniumbase_rs::evasion::fingerprint::WebRtcPolicy::DisableNonProxiedUdp => {
            seleniumbase_rs::plugin_api::WebRtcPolicy::DisableNonProxiedUdp
        }
        seleniumbase_rs::evasion::fingerprint::WebRtcPolicy::PublicAndPrivateInterfaces => {
            seleniumbase_rs::plugin_api::WebRtcPolicy::DefaultPublicAndPrivateInterfaces
        }
        seleniumbase_rs::evasion::fingerprint::WebRtcPolicy::PublicInterfaceOnly => {
            seleniumbase_rs::plugin_api::WebRtcPolicy::DefaultPublicInterfaceOnly
        }
        _ => seleniumbase_rs::plugin_api::WebRtcPolicy::DisableNonProxiedUdp,
    }
}

#[async_trait]
impl Plugin for StealthPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Privacy)
            .capability(Capability::LaunchArgs)
            .capability(Capability::LaunchNetwork)
            .capability(Capability::PageScript(HostScope::Everywhere))
    }

    fn requirements(&self) -> Requirements {
        Requirements::empty()
    }

    fn per_launch(&self) -> Option<Arc<dyn Plugin>> {
        Some(Arc::new(self.clone()))
    }

    async fn before_launch(&self, launch: &mut LaunchSettings) -> PluginResult<()> {
        if !launch
            .args
            .iter()
            .any(|arg| arg.starts_with("--disable-blink-features=AutomationControlled"))
        {
            launch
                .args
                .push("--disable-blink-features=AutomationControlled".to_string());
        }

        if let Some(profile) = &self.profile {
            if let Some(ua) = &profile.user_agent {
                launch.args.push(format!("--user-agent={ua}"));
            }
            if let Some(first_lang) = profile
                .languages
                .first()
                .and_then(|l| l.split(';').next())
                .map(str::trim)
                .filter(|l| !l.is_empty())
            {
                launch.args.push(format!("--lang={first_lang}"));
            }
            if let (Some(w), Some(h)) = (profile.screen_width, profile.screen_height) {
                launch.args.push(format!("--window-size={w},{h}"));
            }
            if let Some(webrtc) = profile.webrtc_policy {
                launch.webrtc_policy = Some(convert_webrtc_policy(webrtc));
            }
        } else if let Some(fp) = &self.fingerprint {
            if let Some(ua) = fp.user_agent().value() {
                launch.args.push(format!("--user-agent={ua}"));
            }
            if let Some(lang) = fp.locale().value().map(String::as_str).or_else(|| {
                fp.languages()
                    .value()
                    .and_then(|s| s.split(',').next())
                    .and_then(|s| s.split(';').next())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            }) {
                launch.args.push(format!("--lang={lang}"));
            }
            if let Some(s) = fp.screen().value() {
                launch
                    .args
                    .push(format!("--window-size={},{}", s.width, s.height));
            }
            if let Some(webrtc) = fp.policy().webrtc_policy {
                launch.webrtc_policy = Some(convert_webrtc_policy(webrtc));
            }
        }

        Ok(())
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        page.add_init_script(self.bundle.script()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seleniumbase_rs::plugin_api::PluginSet;

    #[test]
    fn plugin_initializes_with_fingerprint() {
        let fp = Fingerprint::randomized(OsType::Windows, 123);
        let plugin = StealthPlugin::new(fp);

        assert_eq!(plugin.name(), "stealth");
        assert!(plugin.bundle().script().contains("navigator"));
        assert!(plugin.fingerprint().is_some());
        assert!(plugin.profile().is_some());
    }

    #[test]
    fn plugin_initializes_from_generated_profile() {
        let plugin = StealthPlugin::randomized(456);

        assert_eq!(plugin.name(), "stealth");
        assert!(plugin.bundle().script().contains("webdriver"));
    }

    #[test]
    fn plugin_before_launch_passes_capability_checks_and_sets_clean_args() {
        let plugin = StealthPlugin::randomized_for(OsType::Macos, 77);
        let meta = plugin.metadata();
        assert!(meta.capabilities.covers(&Capability::LaunchArgs));
        assert!(meta.capabilities.covers(&Capability::LaunchNetwork));
        assert!(meta
            .capabilities
            .covers(&Capability::PageScript(HostScope::Everywhere)));

        let set = PluginSet::new().with(plugin).expect("valid plugin set");
        let mut settings = LaunchSettings::default();
        let rt = seleniumbase_rs::tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(set.before_launch(&mut settings))
            .expect("before_launch passes capability checks");

        assert!(settings
            .args
            .iter()
            .any(|a| a == "--disable-blink-features=AutomationControlled"));
        assert!(settings.args.iter().any(|a| a.starts_with("--user-agent=")));
        let lang_arg = settings
            .args
            .iter()
            .find(|a| a.starts_with("--lang="))
            .expect("--lang arg present");
        assert!(!lang_arg.contains(','), "single tag only: {lang_arg}");
        assert!(!lang_arg.contains(";q="), "no q-values: {lang_arg}");
        assert!(settings.webrtc_policy.is_some());
    }
}
