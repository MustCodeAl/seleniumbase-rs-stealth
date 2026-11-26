//! Giving a tab or launched browser an identity from a [`Fingerprint`].
//!
//! [`StealthPlan`] packages the document bootstrap script and protocol overrides
//! for a [`Fingerprint`] apart from any live connection. [`PageFingerprintExt`]
//! and [`BaseCaseFingerprintExt`] provide per-page application, capture, and
//! comparison helpers over `seleniumbase_rs::sb_cdp::Page` and
//! `seleniumbase_rs::BaseCase`. [`FingerprintPlugin`] and
//! [`LaunchOptionsFingerprintExt`] wire a [`Fingerprint`] into pure CDP
//! launches through the ADR 004 plugin host.

use std::fmt;
use std::sync::Arc;

use seleniumbase_rs::error::SeleniumBaseError;
use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, HostScope, LaunchSettings, Metadata, PageHandle, Plugin,
    PluginError, Requirements, Result as PluginResult, WebRtcPolicy as ApiWebRtcPolicy,
};
use seleniumbase_rs::sb_cdp::{LaunchOptions, Page};
use seleniumbase_rs::BaseCase;
use serde_json::{json, Value};

use crate::capture::{capture_async_script, capture_expression, timestamp, CapturedFingerprint};
use crate::evasions::{launch_args, safe_cdp_overrides, session_script};
use crate::fingerprint::{Fingerprint, FingerprintComparison, WebRtcPolicy};

/// The override that grants permissions. It is browser-wide, so it needs the
/// context of the tab it is for and is sent separately.
const PERMISSIONS: &str = "Browser.grantPermissions";

/// The command that installs a script to run in every new document before
/// the page's own scripts.
const INIT_SCRIPT_COMMAND: &str = "Page.addScriptToEvaluateOnNewDocument";

/// What it takes to give a page the identity of a [`Fingerprint`], worked out
/// once, whatever carries the commands to the browser.
#[derive(Clone)]
pub struct StealthPlan {
    /// Commands for the page's own session, in the order they are sent: the
    /// init script first, as `Page.addScriptToEvaluateOnNewDocument`.
    tab: Vec<(String, Value)>,
    /// Permissions to grant in the page's browser context, if any.
    permissions: Option<Vec<String>>,
}

impl fmt::Debug for StealthPlan {
    /// Counts instead of printing: the bootstrap script is thousands of lines.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StealthPlan")
            .field("tab_commands", &self.tab.len())
            .field("permissions", &self.permissions.as_ref().map(Vec::len))
            .finish()
    }
}

impl StealthPlan {
    /// Works out the plan for `fingerprint`: the same script and commands
    /// whether a WebSocket or an in-process channel carries them.
    #[must_use]
    pub fn new(fingerprint: &Fingerprint) -> Self {
        let mut tab = vec![
            (
                INIT_SCRIPT_COMMAND.to_owned(),
                json!({ "source": session_script(fingerprint) }),
            ),
            // Some overrides, such as the blocked URLs, only take effect once
            // the network domain is on.
            ("Network.enable".to_owned(), json!({})),
        ];
        // Without the proxy's `Proxy-Authorization` header, which would reach
        // every site the tab visits: the tab's proxy is answered through the
        // proxy's own challenge instead.
        let mut permissions = None;
        for (method, params) in safe_cdp_overrides(fingerprint) {
            if method == PERMISSIONS {
                permissions = params["permissions"].as_array().map(|names| {
                    names
                        .iter()
                        .filter_map(|name| name.as_str().map(str::to_owned))
                        .collect()
                });
            } else {
                tab.push((method, params));
            }
        }
        Self { tab, permissions }
    }

    /// The script that makes the page's own scripts read the fingerprint.
    #[must_use]
    pub fn init_script(&self) -> &str {
        self.tab
            .first()
            .and_then(|(_, params)| params["source"].as_str())
            .unwrap_or_default()
    }

    /// Every command for the page's own protocol session, in order.
    pub fn commands(&self) -> impl ExactSizeIterator<Item = (&str, &Value)> + '_ {
        self.tab
            .iter()
            .map(|(method, params)| (method.as_str(), params))
    }

    /// The commands after the init script, in order: `Network.enable`, then
    /// the overrides sorted by command name.
    pub fn overrides(&self) -> impl ExactSizeIterator<Item = (&str, &Value)> + '_ {
        self.commands().skip(1)
    }

    /// The commands for the tab's own protocol session, in order.
    #[cfg(test)]
    pub(crate) fn tab_commands(&self) -> &[(String, Value)] {
        &self.tab
    }

    /// The parameters of `Browser.grantPermissions` for a page in the browser
    /// context `context` (`None` for the default one), if the fingerprint
    /// grants any.
    #[must_use]
    pub fn permission_grant(&self, context: Option<&str>) -> Option<Value> {
        let names = self
            .permissions
            .as_ref()
            .filter(|names| !names.is_empty())?;
        let mut params = json!({ "permissions": names });
        if let Some(context) = context {
            params["browserContextId"] = json!(context);
        }
        Some(params)
    }
}

/// Extension methods for applying, capturing, and comparing [`Fingerprint`]s on
/// a pure CDP [`Page`].
pub trait PageFingerprintExt {
    /// Makes this tab match `fingerprint`, for the documents it loads from now
    /// on.
    fn apply_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Captures the fingerprint the page currently exposes to scripts.
    fn capture_fingerprint(
        &self,
    ) -> impl Future<Output = Result<CapturedFingerprint, SeleniumBaseError>> + Send;

    /// Captures the page's fingerprint and compares it against `expected`.
    fn compare_fingerprint(
        &self,
        expected: &Fingerprint,
    ) -> impl Future<Output = Result<FingerprintComparison, SeleniumBaseError>> + Send;
}

impl PageFingerprintExt for Page {
    async fn apply_fingerprint(&self, fingerprint: &Fingerprint) -> Result<(), SeleniumBaseError> {
        let plan = StealthPlan::new(fingerprint);
        for (method, params) in plan.commands() {
            self.execute(method, params.clone())
                .await
                .map_err(|error| refused(method, &error))?;
        }
        if plan.permission_grant(None).is_none() {
            return Ok(());
        }
        let info = self
            .browser()
            .execute("Target.getTargetInfo", json!({ "targetId": self.id() }))
            .await?;
        let context = info["targetInfo"]["browserContextId"]
            .as_str()
            .map(str::to_owned);
        if let Some(grant) = plan.permission_grant(context.as_deref()) {
            self.browser()
                .execute(PERMISSIONS, grant)
                .await
                .map_err(|error| refused(PERMISSIONS, &error))?;
        }
        Ok(())
    }

    async fn capture_fingerprint(&self) -> Result<CapturedFingerprint, SeleniumBaseError> {
        let readings = self.evaluate(&capture_expression()).await?;
        CapturedFingerprint::from_probe(&readings, timestamp())
    }

    async fn compare_fingerprint(
        &self,
        expected: &Fingerprint,
    ) -> Result<FingerprintComparison, SeleniumBaseError> {
        let captured = self.capture_fingerprint().await?;
        Ok(expected.compare_to(&captured.fingerprint))
    }
}

/// Extension methods for capturing and comparing [`Fingerprint`]s on a
/// WebDriver [`BaseCase`].
pub trait BaseCaseFingerprintExt {
    /// Reads what the browser shows the page it has open, as a
    /// [`CapturedFingerprint`].
    fn capture_fingerprint(
        &self,
    ) -> impl Future<Output = Result<CapturedFingerprint, SeleniumBaseError>> + Send;

    /// Captures what the page shows and compares it with `expected`.
    fn compare_fingerprint(
        &self,
        expected: &Fingerprint,
    ) -> impl Future<Output = Result<FingerprintComparison, SeleniumBaseError>> + Send;
}

impl BaseCaseFingerprintExt for BaseCase {
    async fn capture_fingerprint(&self) -> Result<CapturedFingerprint, SeleniumBaseError> {
        let readings = self.execute_async_script(&capture_async_script()).await?;
        CapturedFingerprint::from_probe(&readings, timestamp())
    }

    async fn compare_fingerprint(
        &self,
        expected: &Fingerprint,
    ) -> Result<FingerprintComparison, SeleniumBaseError> {
        let captured = self.capture_fingerprint().await?;
        Ok(expected.compare_to(&captured.fingerprint))
    }
}

/// ADR 004 plugin that applies a [`Fingerprint`]'s launch arguments and per-tab
/// [`StealthPlan`] overrides to every browser launch and new tab.
#[derive(Clone, Debug)]
pub struct FingerprintPlugin {
    fingerprint: Fingerprint,
    plan: StealthPlan,
}

impl FingerprintPlugin {
    /// Creates a plugin that applies `fingerprint` on launch and on every new page.
    #[must_use]
    pub fn new(fingerprint: Fingerprint) -> Self {
        let plan = StealthPlan::new(&fingerprint);
        Self { fingerprint, plan }
    }

    /// Returns the fingerprint applied by this plugin.
    #[must_use]
    pub const fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// Returns the precomputed [`StealthPlan`] for this fingerprint.
    #[must_use]
    pub const fn plan(&self) -> &StealthPlan {
        &self.plan
    }
}

fn to_api_webrtc_policy(policy: WebRtcPolicy) -> ApiWebRtcPolicy {
    match policy {
        WebRtcPolicy::DisableNonProxiedUdp => ApiWebRtcPolicy::DisableNonProxiedUdp,
        WebRtcPolicy::PublicAndPrivateInterfaces => {
            ApiWebRtcPolicy::DefaultPublicAndPrivateInterfaces
        }
        WebRtcPolicy::PublicInterfaceOnly => ApiWebRtcPolicy::DefaultPublicInterfaceOnly,
    }
}

#[async_trait]
impl Plugin for FingerprintPlugin {
    fn name(&self) -> &str {
        "evasion-fingerprint"
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
        for arg in launch_args(&self.fingerprint) {
            if !launch.args.contains(&arg) {
                launch.args.push(arg);
            }
        }
        if let Some(policy) = self.fingerprint.policy().webrtc_policy
            && launch.webrtc_policy.is_none()
        {
            launch.webrtc_policy = Some(to_api_webrtc_policy(policy));
        }
        Ok(())
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        let init = self.plan.init_script();
        if !init.is_empty() {
            page.add_init_script(init).await?;
        }
        for (method, params) in self.plan.overrides() {
            page.send(method, params.clone()).await.map_err(|error| {
                PluginError::other(format!("applying fingerprint override {method}: {error}"))
            })?;
        }
        Ok(())
    }
}

/// Extension method to layer a [`Fingerprint`] onto [`LaunchOptions`].
///
/// Explicit caller settings already present on `self` (such as `user_agent`,
/// `lang`, `timezone`, `geolocation`, and `platform`) take precedence over the
/// fingerprint's defaults, matching the pre-v0.10 launch layer rules.
pub trait LaunchOptionsFingerprintExt {
    /// Returns a copy of these launch options layered with `fingerprint`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if attaching the fingerprint plugin or
    /// rebuilding the options fails.
    fn with_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<LaunchOptions, SeleniumBaseError>;
}

impl LaunchOptionsFingerprintExt for LaunchOptions {
    fn with_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<LaunchOptions, SeleniumBaseError> {
        let mut builder = self.clone().into_builder();

        if self.user_agent().is_none()
            && let Some(ua) = fingerprint.user_agent().as_str()
        {
            builder = builder.user_agent(ua);
        }
        if self.lang().is_none()
            && let Some(lang) = fingerprint.locale().as_str()
        {
            builder = builder.lang(lang);
        }
        if self.timezone().is_none()
            && let Some(tz) = fingerprint.timezone().as_str()
        {
            builder = builder.timezone(tz);
        }
        if self.geolocation().is_none()
            && let Some(geo) = fingerprint.geolocation().value()
        {
            builder = builder.geolocation(geo.latitude, geo.longitude, geo.accuracy);
        }
        if self.platform().is_none()
            && let Some(platform) = fingerprint.platform().as_str()
        {
            builder = builder.platform(platform);
        }
        if let Some(policy) = fingerprint.policy().webrtc_policy {
            builder = builder.webrtc_policy(policy.into());
        }

        // Preserve any existing plugins while layering FingerprintPlugin.
        let existing = self.plugins();
        let plugins = if existing.names().contains(&"evasion-fingerprint") {
            existing.clone()
        } else {
            existing
                .clone()
                .with(FingerprintPlugin::new(fingerprint.clone()))
                .map_err(|error| {
                    SeleniumBaseError::plugin("evasion-fingerprint", error.to_string())
                })?
        };
        builder = builder.plugins(plugins);

        builder.build()
    }
}

/// The error for a command the browser refused while applying an identity.
fn refused(method: &str, error: &SeleniumBaseError) -> SeleniumBaseError {
    SeleniumBaseError::cdp_driver(format!(
        "applying the fingerprint failed at {method}: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evasions::{raw_overrides, CREDENTIAL_HEADER_METHOD};
    use crate::fingerprint::{OsType, ProxyConfig, Spoof};
    use crate::providers::{session_registry, EvasionContext};

    fn methods(identity: &StealthPlan) -> Vec<&str> {
        identity
            .tab_commands()
            .iter()
            .map(|(m, _)| m.as_str())
            .collect()
    }

    #[test]
    fn the_script_comes_first_then_the_network_domain_then_the_overrides() {
        let identity = StealthPlan::new(&Fingerprint::randomized(OsType::Windows, 7));
        let methods = methods(&identity);

        assert_eq!(methods[0], "Page.addScriptToEvaluateOnNewDocument");
        assert_eq!(methods[1], "Network.enable");
        assert!(methods.contains(&"Network.setUserAgentOverride"));
        assert!(methods.contains(&"Emulation.setTimezoneOverride"));
        assert!(methods.contains(&"Emulation.setLocaleOverride"));
    }

    #[test]
    fn the_script_is_the_sessions_and_leaves_plugins_and_connection_to_the_browser() {
        let fingerprint = Fingerprint::randomized(OsType::Windows, 7);
        let identity = StealthPlan::new(&fingerprint);

        let source = identity.tab_commands()[0].1["source"].as_str().unwrap();
        assert_eq!(
            source,
            session_registry(&fingerprint).bootstrap(&EvasionContext::new(&fingerprint))
        );
        let owned =
            regex::Regex::new(r"\b(?:plugins|mimeTypes|pdfViewerEnabled|connection)\b").unwrap();
        assert_eq!(owned.find(source), None, "a name Chrome answers for itself");
    }

    #[test]
    fn proxy_credentials_are_never_included_in_stealth_plan() {
        let proxy = ProxyConfig {
            r#type: "http".to_owned(),
            host: "proxy.example.com".to_owned(),
            port: 8080,
            username: Some("alice".to_owned()),
            password: Some("secret".to_owned()),
            save_traffic: false,
        };
        let fp = Fingerprint::builder()
            .proxy(Spoof::Value(proxy))
            .build()
            .expect("valid fingerprint");
        assert!(raw_overrides(&fp)
            .iter()
            .any(|(m, _)| m == CREDENTIAL_HEADER_METHOD));
        let plan = StealthPlan::new(&fp);
        assert!(!methods(&plan).contains(&CREDENTIAL_HEADER_METHOD));
    }

    #[test]
    fn with_fingerprint_preserves_caller_overrides() {
        let fp = Fingerprint::randomized(OsType::Windows, 42);
        let base = LaunchOptions::builder()
            .user_agent("Custom/1.0")
            .lang("fr-FR")
            .build()
            .expect("valid base options");
        let layered = base.with_fingerprint(&fp).expect("layers fingerprint");
        assert_eq!(layered.user_agent(), Some("Custom/1.0"));
        assert_eq!(layered.lang(), Some("fr-FR"));
        assert_eq!(layered.timezone(), fp.timezone().as_str());
        assert!(layered.plugins().names().contains(&"evasion-fingerprint"));
    }

    #[test]
    fn with_fingerprint_applies_fingerprint_defaults_when_unset() {
        let fp = Fingerprint::randomized(OsType::Macos, 13);
        let layered = LaunchOptions::default()
            .with_fingerprint(&fp)
            .expect("layers fingerprint");
        assert_eq!(layered.user_agent(), fp.user_agent().as_str());
        assert_eq!(layered.lang(), fp.locale().as_str());
        assert_eq!(layered.timezone(), fp.timezone().as_str());
    }
}
