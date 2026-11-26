//! Extension traits for `seleniumbase-rs` engines.
//!
//! Provides [`StealthPageExt`] for pure CDP [`Page`] instances and
//! [`StealthBaseCaseExt`] for WebDriver [`BaseCase`] instances, allowing
//! direct application of [`StealthScriptBundle`] and [`ProfileFingerprint`].

use seleniumbase_rs::error::SeleniumBaseError;
use seleniumbase_rs::evasion::capture::CapturedFingerprint;
use seleniumbase_rs::evasion::fingerprint::{Fingerprint, FingerprintComparison};
use seleniumbase_rs::sb_cdp::Page;
use seleniumbase_rs::BaseCase;

use crate::fingerprint::{GeneratedFingerprint, ProfileFingerprint};
use crate::script::StealthScriptBundle;

/// Stealth extension methods for pure CDP [`Page`].
pub trait StealthPageExt {
    /// Injects a [`ProfileFingerprint`] into the page, installing its script
    /// on all new documents and applying CDP protocol overrides if valid.
    fn apply_profile(
        &self,
        profile: &ProfileFingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a compiled [`StealthScriptBundle`] into the page for all new documents.
    fn apply_stealth(
        &self,
        bundle: &StealthScriptBundle,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a core [`Fingerprint`]'s script and protocol overrides into the page.
    fn apply_stealth_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a full [`GeneratedFingerprint`] bundle into the page.
    fn apply_generated(
        &self,
        generated: &GeneratedFingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;
}

impl StealthPageExt for Page {
    async fn apply_profile(&self, profile: &ProfileFingerprint) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::from_profile(profile);
        if let Ok(fp) = profile.to_fingerprint() {
            self.apply_fingerprint(&fp).await?;
        }
        self.apply_stealth(&bundle).await
    }

    async fn apply_stealth(&self, bundle: &StealthScriptBundle) -> Result<(), SeleniumBaseError> {
        self.execute(
            "Page.addScriptToEvaluateOnNewDocument",
            serde_json::json!({ "source": bundle.script() }),
        )
        .await?;
        Ok(())
    }

    async fn apply_stealth_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::new(fingerprint);
        self.apply_fingerprint(fingerprint).await?;
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
}

/// Stealth extension methods for WebDriver [`BaseCase`].
pub trait StealthBaseCaseExt {
    /// Injects a [`ProfileFingerprint`]'s script into the active page session.
    fn apply_profile(
        &self,
        profile: &ProfileFingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a compiled [`StealthScriptBundle`] into the active page session.
    fn apply_stealth(
        &self,
        bundle: &StealthScriptBundle,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a core [`Fingerprint`]'s stealth script into the active page session.
    fn apply_stealth_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Injects a full [`GeneratedFingerprint`] bundle into the active page session.
    fn apply_generated(
        &self,
        generated: &GeneratedFingerprint,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Captures the active page's current fingerprint.
    fn capture_stealth_fingerprint(
        &self,
    ) -> impl Future<Output = Result<CapturedFingerprint, SeleniumBaseError>> + Send;

    /// Compares the active page's current fingerprint with `expected`.
    fn compare_stealth_fingerprint(
        &self,
        expected: &Fingerprint,
    ) -> impl Future<Output = Result<FingerprintComparison, SeleniumBaseError>> + Send;
}

impl StealthBaseCaseExt for BaseCase {
    async fn apply_profile(&self, profile: &ProfileFingerprint) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::from_profile(profile);
        self.apply_stealth(&bundle).await
    }

    async fn apply_stealth(&self, bundle: &StealthScriptBundle) -> Result<(), SeleniumBaseError> {
        self.execute_script(bundle.script()).await?;
        Ok(())
    }

    async fn apply_stealth_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::new(fingerprint);
        self.apply_stealth(&bundle).await
    }

    async fn apply_generated(
        &self,
        generated: &GeneratedFingerprint,
    ) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::from_generated(generated);
        self.apply_stealth(&bundle).await
    }

    async fn capture_stealth_fingerprint(&self) -> Result<CapturedFingerprint, SeleniumBaseError> {
        self.capture_fingerprint().await
    }

    async fn compare_stealth_fingerprint(
        &self,
        expected: &Fingerprint,
    ) -> Result<FingerprintComparison, SeleniumBaseError> {
        self.compare_fingerprint(expected).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::FingerprintGenerator;
    use seleniumbase_rs::evasion::fingerprint::OsType;

    fn assert_page_ext<T: StealthPageExt>() {}
    fn assert_base_case_ext<T: StealthBaseCaseExt>() {}

    #[test]
    fn traits_are_implemented_for_cdp_page_and_webdriver_base_case() {
        assert_page_ext::<Page>();
        assert_base_case_ext::<BaseCase>();

        let profile = ProfileFingerprint::default();
        let bundle = StealthScriptBundle::from_profile(&profile);
        assert!(bundle.script().contains("navigator"));

        let fp = Fingerprint::randomized(OsType::Windows, 999);
        let bundle_fp = StealthScriptBundle::new(&fp);
        assert!(bundle_fp.script().contains("webdriver"));

        let generated = FingerprintGenerator::new(999).os(OsType::Macos).generate();
        let bundle_gen = StealthScriptBundle::from_generated(&generated);
        assert!(bundle_gen.script().contains(&generated.gpu.renderer));
    }

    #[test]
    fn stealth_page_ext_and_base_case_ext_execute_protocol_commands_on_mocked_page() {
        let rt = seleniumbase_rs::tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async {
            let (browser, mock) = seleniumbase_rs::sb_cdp::Browser::new_mocked();
            let page = browser.default_page().await.expect("mock page");

            let generated = FingerprintGenerator::new(77).os(OsType::Windows).generate();
            let bundle = StealthScriptBundle::from_generated(&generated);

            page.apply_stealth(&bundle).await.expect("apply_stealth");
            page.apply_profile(&generated.profile)
                .await
                .expect("apply_profile");
            page.apply_stealth_fingerprint(&generated.fingerprint)
                .await
                .expect("apply_stealth_fingerprint");
            page.apply_generated(&generated)
                .await
                .expect("apply_generated");

            let injected = mock.calls_to("Page.addScriptToEvaluateOnNewDocument");
            assert!(
                injected
                    .iter()
                    .filter(|c| c.params["source"] == bundle.script())
                    .count()
                    >= 2
            );
            assert!(injected.len() >= 4);

            // Exercise StealthBaseCaseExt over a BaseCase attached to the mocked CDP page.
            let session = seleniumbase_rs::BrowserSession::attach_page(page);
            let sb = seleniumbase_rs::BaseCase::with_session(
                seleniumbase_rs::BrowserConfig::default(),
                session,
            );
            sb.apply_stealth(&bundle)
                .await
                .expect("base_case apply_stealth");
            sb.apply_profile(&generated.profile)
                .await
                .expect("base_case apply_profile");
            sb.apply_stealth_fingerprint(&generated.fingerprint)
                .await
                .expect("base_case apply_stealth_fingerprint");
            sb.apply_generated(&generated)
                .await
                .expect("base_case apply_generated");
        });
    }
}
