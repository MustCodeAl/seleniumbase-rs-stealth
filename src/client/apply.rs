//! Giving a tab or launched browser an identity from a [`Fingerprint`],
//! [`ProfileFingerprint`], [`GeneratedFingerprint`], or [`StealthLayer`].
//!
//! Stealth is exposed as a first-class module rather than a user-facing plugin:
//!
//! - [`StealthPlan`] packages the document bootstrap script and safe CDP
//!   protocol overrides for a [`Fingerprint`] apart from any live connection.
//! - [`StealthLayer`] packages a full stealth identity (core [`Fingerprint`],
//!   [`ProfileFingerprint`], and selective [`StealthScriptBundle`]) for launch
//!   or direct page injection.
//! - [`LaunchOptionsFingerprintExt`] and [`BrowserConfigFingerprintExt`] layer
//!   a [`Fingerprint`] or [`StealthLayer`] onto pure CDP [`LaunchOptions`] and
//!   WebDriver [`BrowserConfig`] respectively.
//! - [`PageFingerprintExt`] and [`BaseCaseFingerprintExt`] provide per-page
//!   application, capture, and comparison helpers over
//!   `seleniumbase_rs::sb_cdp::Page` and `seleniumbase_rs::BaseCase`.

use std::fmt;
use std::sync::Arc;

use seleniumbase_rs::error::SeleniumBaseError;
use seleniumbase_rs::plugin_api::{
    async_trait, switch_policy, Capability, Category, HostScope, LaunchSettings, Metadata,
    PageHandle, Plugin, PluginError, PluginSet, Requirements, Result as PluginResult,
    WebRtcPolicy as ApiWebRtcPolicy,
};
use seleniumbase_rs::sb_cdp::{LaunchOptions, Page};
use seleniumbase_rs::{BaseCase, BrowserConfig};
use serde_json::{json, Value};

use crate::client::evasions::{launch_args, safe_cdp_overrides, session_script};
use crate::client::script::{StealthScriptBundle, StealthScriptOptions};
use crate::identity::capture::{
    capture_async_script, capture_expression, timestamp, CapturedFingerprint,
};
use crate::identity::fingerprint::{
    Fingerprint, FingerprintComparison, FingerprintGenerator, GeneratedFingerprint, OsType,
    ProfileFingerprint, WebRtcPolicy,
};

/// The override that grants permissions. It is browser-wide, so it needs the
/// context of the tab it is for and is sent separately.
const PERMISSIONS: &str = "Browser.grantPermissions";

/// The command that installs a script to run in every new document before
/// the page's own scripts.
const INIT_SCRIPT_COMMAND: &str = "Page.addScriptToEvaluateOnNewDocument";

/// Internal lifecycle hook name used to attach stealth launch/page hooks to
/// `LaunchOptions` and `BrowserConfig`.
const STEALTH_HOOK_NAME: &str = "evasion-stealth";

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

/// Complete stealth layer combining launch configuration, CDP protocol
/// overrides, and a selective [`StealthScriptBundle`].
///
/// Apply a `StealthLayer` to a launch configuration with
/// [`LaunchOptionsFingerprintExt::with_stealth`] or
/// [`BrowserConfigFingerprintExt::with_stealth`], or directly to an open page
/// via [`PageFingerprintExt::apply_stealth_layer`],
/// [`BaseCaseFingerprintExt::apply_stealth_layer`], or
/// [`StealthLayer::inject_script`].
#[derive(Clone, Debug)]
pub struct StealthLayer {
    bundle: StealthScriptBundle,
    plan: Option<StealthPlan>,
    fingerprint: Option<Fingerprint>,
    profile: Option<ProfileFingerprint>,
}

impl StealthLayer {
    /// Creates a stealth layer from a core [`Fingerprint`].
    #[must_use]
    pub fn new(fingerprint: Fingerprint) -> Self {
        let bundle = StealthScriptBundle::new(&fingerprint);
        let plan = StealthPlan::new(&fingerprint);
        let profile = ProfileFingerprint::from_fingerprint(&fingerprint);
        Self {
            bundle,
            plan: Some(plan),
            fingerprint: Some(fingerprint),
            profile: Some(profile),
        }
    }

    /// Creates a stealth layer from a [`ProfileFingerprint`].
    #[must_use]
    pub fn from_profile(profile: ProfileFingerprint) -> Self {
        let bundle = StealthScriptBundle::from_profile(&profile);
        Self {
            bundle,
            plan: None,
            fingerprint: None,
            profile: Some(profile),
        }
    }

    /// Creates a stealth layer from a [`GeneratedFingerprint`].
    #[must_use]
    pub fn from_generated(generated: GeneratedFingerprint) -> Self {
        let bundle = StealthScriptBundle::from_generated(&generated);
        let plan = StealthPlan::new(&generated.fingerprint);
        Self {
            bundle,
            plan: Some(plan),
            fingerprint: Some(generated.fingerprint),
            profile: Some(generated.profile),
        }
    }

    /// Creates a randomized Windows stealth layer seeded by `seed`.
    #[must_use]
    pub fn randomized(seed: u64) -> Self {
        Self::randomized_for(OsType::Windows, seed)
    }

    /// Creates a randomized stealth layer for `os` seeded by `seed`.
    #[must_use]
    pub fn randomized_for(os: OsType, seed: u64) -> Self {
        let generated = FingerprintGenerator::new(seed).os(os).generate();
        Self::from_generated(generated)
    }

    /// Creates a balanced stealth layer for `os` seeded by `seed` (all core,
    /// worker, console, mouse, WebRTC, Intl, and media guards enabled while
    /// keeping pixel/audio/SVG output visually lossless).
    #[must_use]
    pub fn balanced(os: OsType, seed: u64) -> Self {
        Self::randomized_for(os, seed).with_script_options(&StealthScriptOptions::balanced())
    }

    /// Creates a strict stealth layer for `os` seeded by `seed` with all 42
    /// script evasion surfaces enabled.
    #[must_use]
    pub fn strict(os: OsType, seed: u64) -> Self {
        Self::randomized_for(os, seed).with_script_options(&StealthScriptOptions::strict())
    }

    /// Creates a minimal stealth layer for `os` seeded by `seed` covering core
    /// webdriver, runtime, and permissions masking.
    #[must_use]
    pub fn minimal(os: OsType, seed: u64) -> Self {
        Self::randomized_for(os, seed).with_script_options(&StealthScriptOptions::minimal())
    }

    /// Rebuilds this layer's [`StealthScriptBundle`] using selective
    /// [`StealthScriptOptions`].
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

    /// Returns the compiled stealth script bundle.
    #[must_use]
    pub const fn bundle(&self) -> &StealthScriptBundle {
        &self.bundle
    }

    /// Returns the precomputed [`StealthPlan`] for the core fingerprint, if present.
    #[must_use]
    pub const fn plan(&self) -> Option<&StealthPlan> {
        self.plan.as_ref()
    }

    /// Returns the active [`ProfileFingerprint`], if available.
    #[must_use]
    pub const fn profile(&self) -> Option<&ProfileFingerprint> {
        self.profile.as_ref()
    }

    /// Returns the core [`Fingerprint`], if available.
    #[must_use]
    pub const fn fingerprint(&self) -> Option<&Fingerprint> {
        self.fingerprint.as_ref()
    }

    /// Returns the WebRTC IP handling policy configured by this layer, if any.
    #[must_use]
    pub fn webrtc_policy(&self) -> Option<WebRtcPolicy> {
        self.profile
            .as_ref()
            .and_then(|profile| profile.webrtc_policy)
            .or_else(|| {
                self.fingerprint
                    .as_ref()
                    .and_then(|fp| fp.policy().webrtc_policy)
            })
    }

    /// Computes deduplicated Chromium launch arguments for this stealth layer.
    #[must_use]
    pub fn launch_args(&self) -> Vec<String> {
        fn push_unique(args: &mut Vec<String>, arg: String) {
            if !args.contains(&arg) {
                args.push(arg);
            }
        }

        let mut args = vec!["--disable-blink-features=AutomationControlled".to_owned()];

        if let Some(fp) = &self.fingerprint {
            for arg in launch_args(fp) {
                push_unique(&mut args, arg);
            }
        }

        if let Some(profile) = &self.profile {
            if let Some(ua) = &profile.user_agent
                && !args.iter().any(|a| a.starts_with("--user-agent="))
            {
                push_unique(&mut args, format!("--user-agent={ua}"));
            }
            if let Some(first_lang) = profile
                .languages
                .first()
                .and_then(|l| l.split(';').next())
                .map(str::trim)
                .filter(|l| !l.is_empty())
                && !args.iter().any(|a| a.starts_with("--lang="))
            {
                push_unique(&mut args, format!("--lang={first_lang}"));
            }
            if let (Some(w), Some(h)) = (profile.screen_width, profile.screen_height)
                && !args.iter().any(|a| a.starts_with("--window-size="))
            {
                push_unique(&mut args, format!("--window-size={w},{h}"));
            }
        } else if let Some(fp) = &self.fingerprint {
            if let Some(ua) = fp.user_agent().value()
                && !args.iter().any(|a| a.starts_with("--user-agent="))
            {
                push_unique(&mut args, format!("--user-agent={ua}"));
            }
            if let Some(lang) = fp.locale().value().map(String::as_str).or_else(|| {
                fp.languages()
                    .value()
                    .and_then(|s| s.split(',').next())
                    .and_then(|s| s.split(';').next())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            }) && !args.iter().any(|a| a.starts_with("--lang="))
            {
                push_unique(&mut args, format!("--lang={lang}"));
            }
            if let Some(s) = fp.screen().value()
                && !args.iter().any(|a| a.starts_with("--window-size="))
            {
                push_unique(&mut args, format!("--window-size={},{}", s.width, s.height));
            }
        }

        args
    }

    /// Injects this layer's compiled script bundle into any page-like handle
    /// implementing [`PageScriptTarget`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if script registration fails.
    pub async fn inject_script<P: ?Sized + PageScriptTarget>(
        &self,
        page: &P,
    ) -> Result<(), SeleniumBaseError> {
        page.add_init_script(self.bundle.script()).await
    }
}

impl Default for StealthLayer {
    /// Returns a balanced stealth layer matching the host desktop OS.
    fn default() -> Self {
        let os = if cfg!(target_os = "macos") {
            OsType::Macos
        } else if cfg!(target_os = "linux") {
            OsType::Linux
        } else {
            OsType::Windows
        };
        let fp = match os {
            OsType::Macos => Fingerprint::macos_desktop(),
            OsType::Linux => Fingerprint::linux_desktop(),
            _ => Fingerprint::windows_desktop(),
        };
        Self::new(fp).with_script_options(&StealthScriptOptions::balanced())
    }
}

/// Target interface for installing initialization scripts on a page.
pub trait PageScriptTarget {
    /// Installs an initialization script that runs on every new document.
    fn add_init_script(
        &self,
        script: &str,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;
}

impl PageScriptTarget for Page {
    async fn add_init_script(&self, script: &str) -> Result<(), SeleniumBaseError> {
        self.execute(INIT_SCRIPT_COMMAND, json!({ "source": script }))
            .await?;
        Ok(())
    }
}

/// Extension methods for applying, capturing, and comparing fingerprints and
/// stealth scripts on a pure CDP [`Page`].
pub trait PageFingerprintExt {
    /// Makes this tab match `fingerprint`, for the documents it loads from now
    /// on.
    fn apply_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a [`ProfileFingerprint`] into the page, applying core CDP
    /// overrides if valid and installing its [`StealthScriptBundle`] on all new
    /// documents.
    fn apply_profile(
        &self,
        profile: &ProfileFingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a full [`GeneratedFingerprint`] bundle into the page.
    fn apply_generated(
        &self,
        generated: &GeneratedFingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Installs a compiled [`StealthScriptBundle`] to run on every new document.
    fn apply_stealth(
        &self,
        bundle: &StealthScriptBundle,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Applies a full [`StealthLayer`] (core CDP overrides when present, plus
    /// the layer's [`StealthScriptBundle`]).
    fn apply_stealth_layer(
        &self,
        layer: &StealthLayer,
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

    async fn apply_profile(&self, profile: &ProfileFingerprint) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::from_profile(profile);
        if let Ok(fp) = profile.to_fingerprint() {
            self.apply_fingerprint(&fp).await?;
        }
        self.apply_stealth(&bundle).await
    }

    async fn apply_generated(
        &self,
        generated: &GeneratedFingerprint,
    ) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::from_generated(generated);
        self.apply_fingerprint(&generated.fingerprint).await?;
        self.apply_stealth(&bundle).await
    }

    async fn apply_stealth(&self, bundle: &StealthScriptBundle) -> Result<(), SeleniumBaseError> {
        <Self as PageScriptTarget>::add_init_script(self, bundle.script()).await
    }

    async fn apply_stealth_layer(&self, layer: &StealthLayer) -> Result<(), SeleniumBaseError> {
        if let Some(fp) = layer.fingerprint() {
            self.apply_fingerprint(fp).await?;
        }
        self.apply_stealth(layer.bundle()).await
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

/// Extension methods for applying, capturing, and comparing fingerprints and
/// stealth scripts on a WebDriver [`BaseCase`].
pub trait BaseCaseFingerprintExt {
    /// Injects a core [`Fingerprint`]'s stealth script into the active page session.
    fn apply_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a [`ProfileFingerprint`]'s stealth script into the active page session.
    fn apply_profile(
        &self,
        profile: &ProfileFingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a full [`GeneratedFingerprint`] bundle into the active page session.
    fn apply_generated(
        &self,
        generated: &GeneratedFingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a compiled [`StealthScriptBundle`] into the active page session.
    fn apply_stealth(
        &self,
        bundle: &StealthScriptBundle,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a [`StealthLayer`]'s script bundle into the active page session.
    fn apply_stealth_layer(
        &self,
        layer: &StealthLayer,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

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
    async fn apply_fingerprint(&self, fingerprint: &Fingerprint) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::new(fingerprint);
        self.apply_stealth(&bundle).await
    }

    async fn apply_profile(&self, profile: &ProfileFingerprint) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::from_profile(profile);
        self.apply_stealth(&bundle).await
    }

    async fn apply_generated(
        &self,
        generated: &GeneratedFingerprint,
    ) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::from_generated(generated);
        self.apply_stealth(&bundle).await
    }

    async fn apply_stealth(&self, bundle: &StealthScriptBundle) -> Result<(), SeleniumBaseError> {
        self.execute_script(bundle.script()).await?;
        Ok(())
    }

    async fn apply_stealth_layer(&self, layer: &StealthLayer) -> Result<(), SeleniumBaseError> {
        self.apply_stealth(layer.bundle()).await
    }

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

fn to_api_webrtc_policy(policy: WebRtcPolicy) -> ApiWebRtcPolicy {
    match policy {
        WebRtcPolicy::DisableNonProxiedUdp => ApiWebRtcPolicy::DisableNonProxiedUdp,
        WebRtcPolicy::PublicAndPrivateInterfaces => {
            ApiWebRtcPolicy::DefaultPublicAndPrivateInterfaces
        }
        WebRtcPolicy::PublicInterfaceOnly => ApiWebRtcPolicy::DefaultPublicInterfaceOnly,
    }
}

/// Internal lifecycle adapter wiring stealth launch arguments and per-page
/// script/CDP injection through core's extension host without exposing stealth
/// publicly as a plugin.
#[derive(Clone, Debug)]
enum HookPayload {
    Fingerprint {
        fingerprint: Box<Fingerprint>,
        plan: StealthPlan,
    },
    Layer(Box<StealthLayer>),
}

#[derive(Clone, Debug)]
pub(crate) struct StealthLaunchHook {
    payload: HookPayload,
}

impl StealthLaunchHook {
    pub(crate) fn from_fingerprint(fingerprint: Fingerprint) -> Self {
        let plan = StealthPlan::new(&fingerprint);
        Self {
            payload: HookPayload::Fingerprint {
                fingerprint: Box::new(fingerprint),
                plan,
            },
        }
    }

    pub(crate) fn from_layer(layer: StealthLayer) -> Self {
        Self {
            payload: HookPayload::Layer(Box::new(layer)),
        }
    }
}

#[async_trait]
impl Plugin for StealthLaunchHook {
    fn name(&self) -> &str {
        STEALTH_HOOK_NAME
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
        match &self.payload {
            HookPayload::Fingerprint { fingerprint, .. } => {
                for arg in launch_args(fingerprint) {
                    if switch_policy::refusal(&arg).is_none() && !launch.args.contains(&arg) {
                        launch.args.push(arg);
                    }
                }
                if let Some(policy) = fingerprint.policy().webrtc_policy
                    && launch.webrtc_policy.is_none()
                {
                    launch.webrtc_policy = Some(to_api_webrtc_policy(policy));
                }
            }
            HookPayload::Layer(layer) => {
                for arg in layer.launch_args() {
                    if switch_policy::refusal(&arg).is_none() && !launch.args.contains(&arg) {
                        launch.args.push(arg);
                    }
                }
                if let Some(policy) = layer.webrtc_policy()
                    && launch.webrtc_policy.is_none()
                {
                    launch.webrtc_policy = Some(to_api_webrtc_policy(policy));
                }
            }
        }
        Ok(())
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        match &self.payload {
            HookPayload::Fingerprint { plan, .. } => {
                let init = plan.init_script();
                if !init.is_empty() {
                    page.add_init_script(init).await?;
                }
                for (method, params) in plan.overrides() {
                    page.send(method, params.clone()).await.map_err(|error| {
                        PluginError::other(format!(
                            "applying fingerprint override {method}: {error}"
                        ))
                    })?;
                }
            }
            HookPayload::Layer(layer) => {
                let script = layer.bundle().script();
                if !script.is_empty() {
                    page.add_init_script(script).await?;
                }
                if let Some(plan) = layer.plan() {
                    for (method, params) in plan.overrides() {
                        page.send(method, params.clone()).await.map_err(|error| {
                            PluginError::other(format!(
                                "applying stealth override {method}: {error}"
                            ))
                        })?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn attach_hook(
    existing: &PluginSet,
    hook: StealthLaunchHook,
) -> Result<PluginSet, SeleniumBaseError> {
    if existing.names().contains(&STEALTH_HOOK_NAME) {
        return Ok(existing.clone());
    }
    existing
        .clone()
        .with(hook)
        .map_err(|error| SeleniumBaseError::plugin(STEALTH_HOOK_NAME, error.to_string()))
}

/// Internal helper used by [`crate::identity::profile_payloads::ProfileParams`] to attach
/// a fingerprint hook to an already-configured [`BrowserConfig`].
pub(crate) fn attach_fingerprint_to_browser_config(
    config: &mut BrowserConfig,
    fingerprint: Fingerprint,
) {
    if let Ok(hooks) = attach_hook(
        config.plugins.set(),
        StealthLaunchHook::from_fingerprint(fingerprint),
    ) {
        config.plugins = hooks.into();
    }
}

/// Extension methods to layer a [`Fingerprint`] or [`StealthLayer`] onto pure
/// CDP [`LaunchOptions`].
///
/// Explicit caller settings already present on `self` (such as `user_agent`,
/// `lang`, `timezone`, `geolocation`, and `platform`) take precedence over the
/// stealth defaults.
pub trait LaunchOptionsFingerprintExt {
    /// Returns a copy of these launch options layered with `fingerprint`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if attaching the stealth launch hook or
    /// rebuilding the options fails.
    fn with_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<LaunchOptions, SeleniumBaseError>;

    /// Returns a copy of these launch options layered with a full [`StealthLayer`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if attaching the stealth launch hook or
    /// rebuilding the options fails.
    fn with_stealth(&self, layer: &StealthLayer) -> Result<LaunchOptions, SeleniumBaseError>;
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

        let hooks = attach_hook(
            self.plugins(),
            StealthLaunchHook::from_fingerprint(fingerprint.clone()),
        )?;
        builder = builder.plugins(hooks);

        builder.build()
    }

    fn with_stealth(&self, layer: &StealthLayer) -> Result<LaunchOptions, SeleniumBaseError> {
        let mut builder = self.clone().into_builder();

        if let Some(fp) = layer.fingerprint() {
            if self.user_agent().is_none()
                && let Some(ua) = fp.user_agent().as_str()
            {
                builder = builder.user_agent(ua);
            }
            if self.lang().is_none()
                && let Some(lang) = fp.locale().as_str()
            {
                builder = builder.lang(lang);
            }
            if self.timezone().is_none()
                && let Some(tz) = fp.timezone().as_str()
            {
                builder = builder.timezone(tz);
            }
            if self.geolocation().is_none()
                && let Some(geo) = fp.geolocation().value()
            {
                builder = builder.geolocation(geo.latitude, geo.longitude, geo.accuracy);
            }
            if self.platform().is_none()
                && let Some(platform) = fp.platform().as_str()
            {
                builder = builder.platform(platform);
            }
        } else if let Some(profile) = layer.profile() {
            if self.user_agent().is_none()
                && let Some(ua) = profile.user_agent.as_deref()
            {
                builder = builder.user_agent(ua);
            }
            if self.lang().is_none()
                && let Some(lang) = profile
                    .languages
                    .first()
                    .and_then(|l| l.split(';').next())
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
            {
                builder = builder.lang(lang);
            }
            if self.timezone().is_none()
                && let Some(tz) = profile.timezone.as_deref()
            {
                builder = builder.timezone(tz);
            }
            if self.platform().is_none()
                && let Some(platform) = profile.platform.as_deref()
            {
                builder = builder.platform(platform);
            }
        }

        if let Some(policy) = layer.webrtc_policy() {
            builder = builder.webrtc_policy(policy.into());
        }

        let hooks = attach_hook(self.plugins(), StealthLaunchHook::from_layer(layer.clone()))?;
        builder = builder.plugins(hooks);

        builder.build()
    }
}

/// Extension methods to layer a [`Fingerprint`] or [`StealthLayer`] onto a
/// WebDriver / UC [`BrowserConfig`].
pub trait BrowserConfigFingerprintExt {
    /// Returns a copy of this browser configuration layered with `fingerprint`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if attaching the stealth launch hook fails.
    fn with_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<BrowserConfig, SeleniumBaseError>;

    /// Returns a copy of this browser configuration layered with `layer`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if attaching the stealth launch hook fails.
    fn with_stealth(&self, layer: &StealthLayer) -> Result<BrowserConfig, SeleniumBaseError>;
}

impl BrowserConfigFingerprintExt for BrowserConfig {
    fn with_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<BrowserConfig, SeleniumBaseError> {
        let mut config = self.clone();
        if config.browser.user_agent.is_none()
            && let Some(ua) = fingerprint.user_agent().value()
        {
            config.browser.user_agent = Some(ua.clone());
        }
        if config.browser.lang.is_none()
            && let Some(lang) = fingerprint.locale().value()
        {
            config.browser.lang = Some(lang.clone());
        }
        let hooks = attach_hook(
            config.plugins.set(),
            StealthLaunchHook::from_fingerprint(fingerprint.clone()),
        )?;
        config.plugins = hooks.into();
        Ok(config)
    }

    fn with_stealth(&self, layer: &StealthLayer) -> Result<BrowserConfig, SeleniumBaseError> {
        let mut config = self.clone();
        if config.browser.user_agent.is_none() {
            config.browser.user_agent =
                layer
                    .profile()
                    .and_then(|p| p.user_agent.clone())
                    .or_else(|| {
                        layer
                            .fingerprint()
                            .and_then(|fp| fp.user_agent().value().cloned())
                    });
        }
        if config.browser.lang.is_none() {
            config.browser.lang = layer
                .fingerprint()
                .and_then(|fp| fp.locale().value().cloned())
                .or_else(|| {
                    layer.profile().and_then(|p| {
                        p.languages
                            .first()
                            .and_then(|l| l.split(';').next())
                            .map(str::trim)
                            .filter(|l| !l.is_empty())
                            .map(str::to_owned)
                    })
                });
        }
        let hooks = attach_hook(
            config.plugins.set(),
            StealthLaunchHook::from_layer(layer.clone()),
        )?;
        config.plugins = hooks.into();
        Ok(config)
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
    use crate::client::evasions::{raw_overrides, CREDENTIAL_HEADER_METHOD};
    use crate::client::providers::{session_registry, EvasionContext};
    use crate::identity::fingerprint::{ProxyConfig, Spoof};

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
        assert!(layered.plugins().names().contains(&STEALTH_HOOK_NAME));
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

    #[test]
    fn stealth_layer_initializes_from_fingerprint_profile_and_generated() {
        let fp = Fingerprint::randomized(OsType::Windows, 123);
        let layer = StealthLayer::new(fp);
        assert!(layer.bundle().script().contains("navigator"));
        assert!(layer.fingerprint().is_some());
        assert!(layer.profile().is_some());
        assert!(layer.plan().is_some());

        let rand_layer = StealthLayer::randomized(456);
        assert!(rand_layer.bundle().script().contains("webdriver"));

        let layered_opts = LaunchOptions::default()
            .with_stealth(&rand_layer)
            .expect("layers stealth");
        assert!(layered_opts.plugins().names().contains(&STEALTH_HOOK_NAME));
        assert!(layered_opts.user_agent().is_some());

        let layered_cfg = BrowserConfig::default()
            .with_stealth(&rand_layer)
            .expect("layers browser config");
        assert!(layered_cfg.browser.user_agent.is_some());
    }

    #[test]
    fn stealth_launch_hook_passes_capability_checks_and_sets_clean_args() {
        let layer = StealthLayer::randomized_for(OsType::Macos, 77);
        let hook = StealthLaunchHook::from_layer(layer);
        let meta = hook.metadata();
        assert!(meta.capabilities.covers(&Capability::LaunchArgs));
        assert!(meta.capabilities.covers(&Capability::LaunchNetwork));
        assert!(meta
            .capabilities
            .covers(&Capability::PageScript(HostScope::Everywhere)));

        let set = PluginSet::new().with(hook).expect("valid hook set");
        let mut settings = LaunchSettings::default();
        let rt = tokio::runtime::Builder::new_current_thread()
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

    #[test]
    fn stealth_layer_from_profile_script_options_and_webrtc_mapping() {
        struct RecordingTarget(std::sync::Mutex<Vec<String>>);
        impl PageScriptTarget for RecordingTarget {
            fn add_init_script(
                &self,
                script: &str,
            ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send {
                self.0.lock().unwrap().push(script.to_owned());
                std::future::ready(Ok(()))
            }
        }

        let profile = ProfileFingerprint {
            user_agent: Some("CustomUA/1.0".to_owned()),
            languages: vec!["fr-FR;q=0.9".to_owned()],
            screen_width: Some(1280),
            screen_height: Some(720),
            webrtc_policy: Some(WebRtcPolicy::PublicInterfaceOnly),
            ..ProfileFingerprint::default()
        };

        let layer = StealthLayer::from_profile(profile)
            .with_script_options(&StealthScriptOptions::minimal());
        assert!(layer.fingerprint().is_none());
        assert!(layer.profile().is_some());

        let hook = StealthLaunchHook::from_layer(layer.clone());
        assert_eq!(hook.requirements(), Requirements::empty());
        assert!(hook.per_launch().is_some());

        let mut launch = LaunchSettings::default();
        launch
            .args
            .push("--disable-blink-features=AutomationControlled".to_owned());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(hook.before_launch(&mut launch))
            .expect("before_launch");
        assert_eq!(
            launch
                .args
                .iter()
                .filter(|a| *a == "--disable-blink-features=AutomationControlled")
                .count(),
            1
        );
        assert!(launch.args.iter().any(|a| a == "--user-agent=CustomUA/1.0"));
        assert!(launch.args.iter().any(|a| a == "--lang=fr-FR"));
        assert!(launch.args.iter().any(|a| a == "--window-size=1280,720"));
        assert_eq!(
            launch.webrtc_policy,
            Some(ApiWebRtcPolicy::DefaultPublicInterfaceOnly)
        );

        assert_eq!(
            to_api_webrtc_policy(WebRtcPolicy::DisableNonProxiedUdp),
            ApiWebRtcPolicy::DisableNonProxiedUdp
        );
        assert_eq!(
            to_api_webrtc_policy(WebRtcPolicy::PublicAndPrivateInterfaces),
            ApiWebRtcPolicy::DefaultPublicAndPrivateInterfaces
        );

        let target = RecordingTarget(std::sync::Mutex::new(Vec::new()));
        rt.block_on(layer.inject_script(&target))
            .expect("inject_script");
        assert_eq!(target.0.lock().unwrap().len(), 1);
    }

    #[test]
    fn page_and_base_case_ext_execute_protocol_commands_on_mocked_page() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async {
            let (browser, mock) = seleniumbase_rs::sb_cdp::Browser::new_mocked();
            let page = browser.default_page().await.expect("mock page");

            let generated = FingerprintGenerator::new(77).os(OsType::Windows).generate();
            let bundle = StealthScriptBundle::from_generated(&generated);
            let layer = StealthLayer::from_generated(generated.clone());

            page.apply_stealth(&bundle).await.expect("apply_stealth");
            page.apply_profile(&generated.profile)
                .await
                .expect("apply_profile");
            page.apply_fingerprint(&generated.fingerprint)
                .await
                .expect("apply_fingerprint");
            page.apply_generated(&generated)
                .await
                .expect("apply_generated");
            page.apply_stealth_layer(&layer)
                .await
                .expect("apply_stealth_layer");

            let injected = mock.calls_to("Page.addScriptToEvaluateOnNewDocument");
            assert!(
                injected
                    .iter()
                    .filter(|c| c.params["source"] == bundle.script())
                    .count()
                    >= 3
            );
            assert!(injected.len() >= 5);

            let session = seleniumbase_rs::BrowserSession::attach_page(page);
            let sb = BaseCase::with_session(BrowserConfig::default(), session);
            sb.apply_stealth(&bundle)
                .await
                .expect("base_case apply_stealth");
            sb.apply_profile(&generated.profile)
                .await
                .expect("base_case apply_profile");
            sb.apply_fingerprint(&generated.fingerprint)
                .await
                .expect("base_case apply_fingerprint");
            sb.apply_generated(&generated)
                .await
                .expect("base_case apply_generated");
            sb.apply_stealth_layer(&layer)
                .await
                .expect("base_case apply_stealth_layer");
        });
    }
}
