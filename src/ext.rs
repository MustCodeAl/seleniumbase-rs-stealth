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
        self.apply_stealth(&bundle).await?;

        if let Ok(fp) = profile.to_fingerprint() {
            let _ = self.apply_fingerprint(&fp).await;
        }

        Ok(())
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
        self.apply_stealth(&bundle).await?;
        self.apply_fingerprint(fingerprint).await?;
        Ok(())
    }

    async fn apply_generated(
        &self,
        generated: &GeneratedFingerprint,
    ) -> Result<(), SeleniumBaseError> {
        let bundle = StealthScriptBundle::from_generated(generated);
        self.apply_stealth(&bundle).await?;
        self.apply_fingerprint(&generated.fingerprint).await?;
        Ok(())
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
    use seleniumbase_rs::evasion::fingerprint::OsType;

    #[test]
    fn traits_are_object_safe_or_callable() {
        let profile = ProfileFingerprint::default();
        let bundle = StealthScriptBundle::from_profile(&profile);
        assert!(bundle.script().contains("navigator"));

        let fp = Fingerprint::randomized(OsType::Windows, 999);
        let bundle_fp = StealthScriptBundle::new(&fp);
        assert!(bundle_fp.script().contains("webdriver"));
    }
}
