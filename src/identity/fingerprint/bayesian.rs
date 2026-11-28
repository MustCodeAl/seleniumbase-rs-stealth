//! Conditional statistical fingerprint generation, browser-accurate HTTP header ordering,
//! cross-engine conversions (CDP overrides, Chromium launch flags), and coherence auditing.
//!
//! Models realistic joint distributions across operating system, browser family, viewport
//! geometry, GPU vendor/renderer, hardware concurrency, device memory, and locale so generated
//! personas match real-world browser telemetry rather than independent uniform random picks.

use std::collections::BTreeMap;

use super::randomize::PRESET_CHROMIUM_MAJOR;
use super::{BrowserType, ColorScheme, FingerprintError, OsType};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::generator::{
    FingerprintGenerator, FingerprintOptions, GeneratedFingerprint, ScreenConstraints,
};
use super::profile::HdrMode;

/// Device form-factor filter for statistical sampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceCategory {
    /// Desktop / laptop workstations.
    Desktop,
    /// Smartphones.
    Mobile,
}

/// Request context used by [`HeaderGenerator`] to pick browser-accurate `Accept` and `Sec-Fetch-*` headers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestContext {
    /// Top-level document navigation (`Sec-Fetch-Mode: navigate`).
    #[default]
    Navigation,
    /// Same-origin `fetch()` / XHR (`Sec-Fetch-Mode: cors`).
    Fetch,
    /// Subresource load such as script/stylesheet/image (`Sec-Fetch-Mode: no-cors`).
    Subresource,
    /// Cross-origin API request (`Sec-Fetch-Site: cross-site`).
    CrossOriginApi,
}

/// Constraints for [`BayesianSampler::sample`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleConstraints {
    /// Allowed operating systems (empty = all weighted by market share).
    pub os: Vec<OsType>,
    /// Allowed browser families (empty = all compatible with chosen OS).
    pub browsers: Vec<BrowserType>,
    /// Optional device form-factor filter.
    pub device: Option<DeviceCategory>,
    /// Minimum Chromium major version.
    pub min_version: Option<u32>,
    /// Maximum Chromium major version.
    pub max_version: Option<u32>,
    /// Preferred `navigator.languages` list.
    pub locales: Option<Vec<String>>,
    /// Screen geometry constraints.
    pub screen: Option<ScreenConstraints>,
    /// Optional explicit timezone.
    pub timezone: Option<String>,
    /// Optional explicit color scheme.
    pub color_scheme: Option<ColorScheme>,
    /// When `true`, returns an error if no real-world joint configuration satisfies all filters.
    pub strict: bool,
    /// Deterministic PRNG seed.
    pub seed: Option<u64>,
}

impl SampleConstraints {
    /// Starts an empty constraint set (unconstrained realistic sampling).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restricts sampling to the given operating systems.
    #[must_use]
    pub fn os(mut self, os: impl IntoIterator<Item = OsType>) -> Self {
        self.os = os.into_iter().collect();
        self
    }

    /// Restricts sampling to the given browser families.
    #[must_use]
    pub fn browsers(mut self, browsers: impl IntoIterator<Item = BrowserType>) -> Self {
        self.browsers = browsers.into_iter().collect();
        self
    }

    /// Restricts sampling to desktop or mobile form factors.
    #[must_use]
    pub const fn device(mut self, device: DeviceCategory) -> Self {
        self.device = Some(device);
        self
    }

    /// Restricts the major browser version range.
    #[must_use]
    pub const fn version_range(mut self, min: u32, max: u32) -> Self {
        self.min_version = Some(min);
        self.max_version = Some(max);
        self
    }

    /// Sets preferred locales.
    #[must_use]
    pub fn locales(mut self, locales: Vec<String>) -> Self {
        self.locales = Some(locales);
        self
    }

    /// Sets screen constraints.
    #[must_use]
    pub const fn screen(mut self, screen: ScreenConstraints) -> Self {
        self.screen = Some(screen);
        self
    }

    /// Sets the deterministic seed.
    #[must_use]
    pub const fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Enables or disables strict constraint validation.
    #[must_use]
    pub const fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
}

/// Deterministic splitmix64 PRNG for repeatable weighted sampling.
#[derive(Clone, Debug)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn pick_weighted<'a, T>(&mut self, items: &'a [(T, u32)]) -> Option<&'a T> {
        let total: u64 = items.iter().map(|(_, w)| u64::from(*w)).sum();
        if total == 0 {
            return None;
        }
        let mut target = self.next_u64() % total;
        for (item, weight) in items {
            let w = u64::from(*weight);
            if target < w {
                return Some(item);
            }
            target -= w;
        }
        items.last().map(|(item, _)| item)
    }
}

/// Conditional statistical fingerprint sampler.
#[derive(Clone, Debug, Default)]
pub struct BayesianSampler;

impl BayesianSampler {
    /// Samples a coherent [`GeneratedFingerprint`] from conditional real-world distributions.
    ///
    /// # Errors
    ///
    /// Returns [`FingerprintError::Incoherent`] when `constraints.strict` is `true` and
    /// the requested OS / device / browser / version combination has no valid real-world match.
    #[allow(
        clippy::too_many_lines,
        reason = "samples OS, browser, viewport, and hardware jointly in one coherent pass"
    )]
    pub fn sample(
        constraints: &SampleConstraints,
    ) -> Result<GeneratedFingerprint, FingerprintError> {
        let seed = constraints.seed.unwrap_or(0xBA7E_51A4_0000_0001);
        let mut rng = Rng::new(seed);

        if let (Some(min_v), Some(max_v)) = (constraints.min_version, constraints.max_version)
            && min_v > max_v
        {
            return Err(FingerprintError::Incoherent {
                setting: "version_range",
                reason: "min_version cannot exceed max_version".to_owned(),
            });
        }

        // 1. Sample OS conditioned on device category + explicit filter.
        let os_weights: [(OsType, u32); 5] = [
            (OsType::Windows, 52),
            (OsType::Macos, 24),
            (OsType::Linux, 8),
            (OsType::Android, 10),
            (OsType::Ios, 6),
        ];
        let filtered_os: Vec<(OsType, u32)> = os_weights
            .into_iter()
            .filter(|(os, _)| {
                let device_ok = match constraints.device {
                    Some(DeviceCategory::Desktop) => !os.is_mobile(),
                    Some(DeviceCategory::Mobile) => os.is_mobile(),
                    None => true,
                };
                let list_ok = constraints.os.is_empty() || constraints.os.contains(os);
                device_ok && list_ok
            })
            .collect();

        let os = if let Some(&picked) = rng.pick_weighted(&filtered_os) {
            picked
        } else if constraints.strict {
            return Err(FingerprintError::Incoherent {
                setting: "os",
                reason: "no operating system matches the requested OS and device filters"
                    .to_owned(),
            });
        } else {
            OsType::Windows
        };

        // 2. Sample browser conditioned on OS.
        let browser_candidates: Vec<(BrowserType, u32)> = match os {
            OsType::Ios => vec![(BrowserType::MobileSafari, 100)],
            OsType::Android => vec![(BrowserType::Chromium, 92), (BrowserType::Firefox, 8)],
            OsType::Macos => vec![(BrowserType::Chromium, 85), (BrowserType::Firefox, 15)],
            OsType::Windows | OsType::Linux => {
                vec![(BrowserType::Chromium, 88), (BrowserType::Firefox, 12)]
            }
        };
        let filtered_browsers: Vec<(BrowserType, u32)> = browser_candidates
            .into_iter()
            .filter(|(b, _)| constraints.browsers.is_empty() || constraints.browsers.contains(b))
            .collect();
        let browser = if let Some(&picked) = rng.pick_weighted(&filtered_browsers) {
            picked
        } else if constraints.strict {
            return Err(FingerprintError::Incoherent {
                setting: "browsers",
                reason: format!("no requested browser is coherent with OS {os:?}"),
            });
        } else if os == OsType::Ios {
            BrowserType::MobileSafari
        } else {
            BrowserType::Chromium
        };

        // 3. Sample viewport (W, H, DPR) conditioned on OS.
        let screen_table: &[((u32, u32, f64), u32)] = match os {
            OsType::Macos => &[
                ((1440, 900, 2.0), 35),
                ((1512, 982, 2.0), 30),
                ((1728, 1117, 2.0), 20),
                ((2560, 1440, 2.0), 15),
            ],
            OsType::Android => &[
                ((412, 915, 2.625), 45),
                ((393, 873, 2.75), 35),
                ((360, 800, 3.0), 20),
            ],
            OsType::Ios => &[
                ((390, 844, 3.0), 40),
                ((393, 852, 3.0), 35),
                ((430, 932, 3.0), 25),
            ],
            OsType::Windows | OsType::Linux => &[
                ((1920, 1080, 1.0), 50),
                ((2560, 1440, 1.0), 22),
                ((1536, 864, 1.25), 16),
                ((1366, 768, 1.0), 12),
            ],
        };
        let &(w, h, dpr) = rng
            .pick_weighted(screen_table)
            .unwrap_or(&(1920, 1080, 1.0));

        // 4. Sample hardware (cores, memory GiB) conditioned on OS.
        let hw_table: &[((u32, u32), u32)] = match os {
            OsType::Macos => &[((8, 8), 35), ((10, 16), 35), ((12, 16), 20), ((16, 32), 10)],
            OsType::Android => &[((8, 8), 55), ((8, 6), 30), ((8, 12), 15)],
            OsType::Ios => &[((6, 4), 60), ((6, 6), 40)],
            OsType::Windows | OsType::Linux => &[
                ((8, 8), 35),
                ((12, 16), 30),
                ((16, 16), 20),
                ((4, 8), 10),
                ((24, 32), 5),
            ],
        };
        let &(cores, mem_gb) = rng.pick_weighted(hw_table).unwrap_or(&(8, 8));

        let min_v = constraints.min_version.unwrap_or(PRESET_CHROMIUM_MAJOR);
        let max_v = constraints
            .max_version
            .unwrap_or(min_v.max(PRESET_CHROMIUM_MAJOR));

        let mut builder = FingerprintOptions::builder()
            .os(os)
            .browser(browser)
            .min_version(min_v)
            .max_version(max_v)
            .window_size(w, h)
            .hardware_concurrency(cores)
            .device_memory(mem_gb)
            .seed(seed);

        let mut screen_cfg = constraints.screen.unwrap_or_default();
        if screen_cfg.device_pixel_ratio.is_none() {
            screen_cfg.device_pixel_ratio = Some(dpr);
        }
        builder = builder.screen(screen_cfg);

        if os == OsType::Macos {
            builder = builder.hdr_mode(HdrMode::High);
        }
        if let Some(locales) = &constraints.locales {
            builder = builder.locales(locales.clone());
        }
        if let Some(tz) = &constraints.timezone {
            builder = builder.timezone(tz.clone());
        }
        if let Some(scheme) = constraints.color_scheme {
            builder = builder.color_scheme(scheme);
        }

        let opts = builder.build()?;
        Ok(FingerprintGenerator::generate_with(&opts))
    }

    /// Generates `count` distinct, deterministically seeded personas satisfying `constraints`.
    ///
    /// # Errors
    ///
    /// Propagates any strict validation error from [`Self::sample`].
    pub fn sample_batch(
        constraints: &SampleConstraints,
        count: usize,
    ) -> Result<Vec<GeneratedFingerprint>, FingerprintError> {
        let base_seed = constraints.seed.unwrap_or(0xBA7E_51A4_0000_0001);
        let mut out = Vec::with_capacity(count);
        for idx in 0..count {
            let mut step = constraints.clone();
            step.seed = Some(base_seed.wrapping_add(u64::try_from(idx).unwrap_or(0)));
            out.push(Self::sample(&step)?);
        }
        Ok(out)
    }

    /// Samples a coherent [`GeneratedFingerprint`] along with ordered HTTP headers for `context`.
    ///
    /// # Errors
    ///
    /// Propagates any validation error from [`Self::sample`].
    pub fn sample_with_headers(
        constraints: &SampleConstraints,
        context: RequestContext,
    ) -> Result<(GeneratedFingerprint, Vec<(String, String)>), FingerprintError> {
        let fp = Self::sample(constraints)?;
        let headers = HeaderGenerator::generate(&fp, context, None);
        Ok((fp, headers))
    }
}

/// Ordered HTTP header generator matching real browser wire order per [`RequestContext`].
#[derive(Clone, Debug, Default)]
pub struct HeaderGenerator;

impl HeaderGenerator {
    /// Produces an ordered `Vec<(String, String)>` of HTTP headers matching browser wire order.
    #[must_use]
    pub fn generate(
        fp: &GeneratedFingerprint,
        context: RequestContext,
        referer: Option<&str>,
    ) -> Vec<(String, String)> {
        let mut ordered = Vec::with_capacity(14);
        let is_chromium = fp.profile.browser == Some(BrowserType::Chromium);

        if is_chromium {
            for key in ["Sec-CH-UA", "Sec-CH-UA-Mobile", "Sec-CH-UA-Platform"] {
                if let Some(val) = fp.headers.get(key) {
                    ordered.push((key.to_owned(), val.clone()));
                }
            }
        }
        if context == RequestContext::Navigation {
            ordered.push(("Upgrade-Insecure-Requests".to_owned(), "1".to_owned()));
        }
        if let Some(ua) = fp.headers.get("User-Agent") {
            ordered.push(("User-Agent".to_owned(), ua.clone()));
        }
        let accept = match context {
            RequestContext::Navigation => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"
            }
            RequestContext::Fetch | RequestContext::CrossOriginApi => {
                "application/json, text/plain, */*"
            }
            RequestContext::Subresource => "*/*",
        };
        ordered.push(("Accept".to_owned(), accept.to_owned()));

        let (site, mode, dest, user) = match context {
            RequestContext::Navigation => (
                if referer.is_some() {
                    "cross-site"
                } else {
                    "none"
                },
                "navigate",
                "document",
                Some("?1"),
            ),
            RequestContext::Fetch => ("same-origin", "cors", "empty", None),
            RequestContext::Subresource => ("same-origin", "no-cors", "script", None),
            RequestContext::CrossOriginApi => ("cross-site", "cors", "empty", None),
        };
        ordered.push(("Sec-Fetch-Site".to_owned(), site.to_owned()));
        ordered.push(("Sec-Fetch-Mode".to_owned(), mode.to_owned()));
        if let Some(u) = user {
            ordered.push(("Sec-Fetch-User".to_owned(), u.to_owned()));
        }
        ordered.push(("Sec-Fetch-Dest".to_owned(), dest.to_owned()));

        if let Some(r) = referer {
            ordered.push(("Referer".to_owned(), r.to_owned()));
        }
        let is_safari = matches!(fp.profile.browser, Some(BrowserType::MobileSafari));
        ordered.push((
            "Accept-Encoding".to_owned(),
            if is_safari {
                "gzip, deflate, br".to_owned()
            } else {
                "gzip, deflate, br, zstd".to_owned()
            },
        ));
        if let Some(al) = fp.headers.get("Accept-Language") {
            ordered.push(("Accept-Language".to_owned(), al.clone()));
        }
        if is_chromium {
            let prio = match context {
                RequestContext::Navigation => "u=0, i",
                RequestContext::Fetch | RequestContext::CrossOriginApi => "u=1, i",
                RequestContext::Subresource => "u=2",
            };
            ordered.push(("Priority".to_owned(), prio.to_owned()));
        }
        ordered
    }
}

/// Cross-engine conversions from a [`GeneratedFingerprint`] to CDP commands and Chromium CLI flags.
#[derive(Clone, Debug, Default)]
pub struct FingerprintConverter;

impl FingerprintConverter {
    /// Emits ready-to-execute CDP `(method, params)` tuples for `Emulation.*` and `Network.*`.
    #[must_use]
    pub fn to_cdp_commands(fp: &GeneratedFingerprint) -> Vec<(String, Value)> {
        let mut cmds = Vec::new();
        if let Some(ua) = &fp.profile.user_agent {
            let mut params = json!({
                "userAgent": ua,
                "acceptLanguage": fp.profile.languages.join(","),
                "platform": fp.profile.platform.as_deref().unwrap_or("Win32"),
            });
            if let Some(ch) = &fp.profile.client_hints {
                params["userAgentMetadata"] = json!({
                    "brands": ch.brands,
                    "fullVersionList": ch.full_version_list,
                    "fullVersion": ch.ua_full_version,
                    "platform": ch.platform,
                    "platformVersion": ch.platform_version,
                    "architecture": ch.architecture,
                    "model": ch.model,
                    "mobile": ch.mobile,
                    "bitness": ch.bitness,
                    "wow64": ch.wow64,
                });
            }
            cmds.push(("Emulation.setUserAgentOverride".to_owned(), params));
        }

        cmds.push((
            "Emulation.setDeviceMetricsOverride".to_owned(),
            json!({
                "width": fp.display.width,
                "height": fp.display.height,
                "deviceScaleFactor": fp.display.pixel_ratio,
                "mobile": fp.profile.os.is_some_and(|os| os.is_mobile()),
            }),
        ));

        if let Some(tz) = &fp.profile.timezone {
            cmds.push((
                "Emulation.setTimezoneOverride".to_owned(),
                json!({ "timezoneId": tz }),
            ));
        }

        if let Some(lang) = fp.profile.languages.first() {
            cmds.push((
                "Emulation.setLocaleOverride".to_owned(),
                json!({ "locale": lang }),
            ));
        }

        if !fp.headers.is_empty() {
            let extra: BTreeMap<&String, &String> = fp
                .headers
                .iter()
                .filter(|(k, _)| !k.eq_ignore_ascii_case("user-agent"))
                .collect();
            if !extra.is_empty() {
                cmds.push((
                    "Network.setExtraHTTPHeaders".to_owned(),
                    json!({ "headers": extra }),
                ));
            }
        }
        cmds
    }

    /// Emits coherent Chromium command-line launch flags matching `fp`.
    #[must_use]
    pub fn to_chromium_launch_args(fp: &GeneratedFingerprint) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(ua) = &fp.profile.user_agent {
            args.push(format!("--user-agent={ua}"));
        }
        args.push(format!(
            "--window-size={},{}",
            fp.display.width, fp.display.height
        ));
        args.push(format!(
            "--force-device-scale-factor={}",
            fp.display.pixel_ratio
        ));
        if let Some(lang) = fp.profile.languages.first() {
            args.push(format!("--lang={lang}"));
        }
        args
    }

    /// Emits a Playwright-compatible browser context options JSON object for `fp`.
    #[must_use]
    pub fn to_playwright_context_options(fp: &GeneratedFingerprint) -> Value {
        let extra_headers: BTreeMap<&String, &String> = fp
            .headers
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("user-agent"))
            .collect();
        let color_scheme = match fp.profile.color_scheme {
            Some(ColorScheme::Dark) => "dark",
            _ => "light",
        };
        let is_mobile = fp.profile.os.is_some_and(|os| os.is_mobile());
        json!({
            "userAgent": fp.profile.user_agent,
            "viewport": {
                "width": fp.display.width,
                "height": fp.display.height,
            },
            "deviceScaleFactor": fp.display.pixel_ratio,
            "isMobile": is_mobile,
            "hasTouch": is_mobile || fp.profile.max_touch_points.is_some_and(|t| t > 0),
            "locale": fp.profile.languages.first(),
            "timezoneId": fp.profile.timezone,
            "colorScheme": color_scheme,
            "extraHTTPHeaders": extra_headers,
        })
    }

    /// Serializes the generated fingerprint metadata to pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns [`serde_json::Error`] if serialization fails.
    pub fn to_json_pretty(fp: &GeneratedFingerprint) -> Result<String, serde_json::Error> {
        let payload = json!({
            "profile": fp.profile,
            "headers": fp.headers,
            "client_hints": fp.client_hints,
            "audio": fp.audio,
            "gpu": fp.gpu,
            "webgpu": fp.webgpu,
            "display": fp.display,
            "media_devices": fp.media_devices,
            "battery": fp.battery,
            "network": fp.network,
            "fonts": fp.fonts,
        });
        serde_json::to_string_pretty(&payload)
    }
}

/// Coherence audit report for a [`GeneratedFingerprint`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoherenceAudit {
    /// Score from `0` (incoherent) to `100` (fully coherent).
    pub score: u8,
    /// Specific cross-layer contradictions detected.
    pub issues: Vec<String>,
}

impl CoherenceAudit {
    /// Returns `true` when zero contradictions were found.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Audits cross-layer coherence across UA, platform, Client Hints, GPU renderer, and viewport.
#[must_use]
pub fn audit_coherence(fp: &GeneratedFingerprint) -> CoherenceAudit {
    let mut issues = Vec::new();
    let ua = fp.profile.user_agent.as_deref().unwrap_or_default();
    let platform = fp.profile.platform.as_deref().unwrap_or_default();
    let renderer = fp.gpu.renderer.as_str();

    if ua.contains("Windows") && !platform.is_empty() && !platform.contains("Win") {
        issues.push(format!(
            "Windows User-Agent paired with non-Windows platform '{platform}'"
        ));
    }
    if ua.contains("Macintosh") && !platform.is_empty() && !platform.contains("Mac") {
        issues.push(format!(
            "macOS User-Agent paired with non-Mac platform '{platform}'"
        ));
    }
    if ua.contains("Windows") && renderer.contains("Apple") {
        issues.push(format!(
            "Windows User-Agent paired with Apple GPU renderer '{renderer}'"
        ));
    }
    if ua.contains("Macintosh") && renderer.contains("Direct3D") {
        issues.push(format!(
            "macOS User-Agent paired with Direct3D renderer '{renderer}'"
        ));
    }
    if let Some(ch) = &fp.profile.client_hints {
        if ua.contains("Windows") && !ch.platform.eq_ignore_ascii_case("Windows") {
            issues.push(format!(
                "Windows UA conflicts with Sec-CH-UA-Platform '{}'",
                ch.platform
            ));
        }
        if ua.contains("Macintosh") && !ch.platform.eq_ignore_ascii_case("macOS") {
            issues.push(format!(
                "macOS UA conflicts with Sec-CH-UA-Platform '{}'",
                ch.platform
            ));
        }
    }
    if fp.display.width == 0 || fp.display.height == 0 {
        issues.push("display dimensions cannot be zero".to_owned());
    }

    let penalty = u8::try_from(issues.len().saturating_mul(20)).unwrap_or(100);
    CoherenceAudit {
        score: 100u8.saturating_sub(penalty),
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayesian_sampler_produces_coherent_desktop_and_mobile_personas() {
        let desktop = BayesianSampler::sample(
            &SampleConstraints::new()
                .device(DeviceCategory::Desktop)
                .seed(1234),
        )
        .expect("desktop sample");
        assert!(!desktop.profile.os.unwrap().is_mobile());
        let report = audit_coherence(&desktop);
        assert!(report.is_clean(), "issues: {:?}", report.issues);
        assert_eq!(report.score, 100);

        let mobile = BayesianSampler::sample(
            &SampleConstraints::new()
                .device(DeviceCategory::Mobile)
                .seed(5678),
        )
        .expect("mobile sample");
        assert!(mobile.profile.os.unwrap().is_mobile());
        assert!(audit_coherence(&mobile).is_clean());
    }

    #[test]
    fn strict_sampler_rejects_impossible_os_device_combination() {
        let err = BayesianSampler::sample(
            &SampleConstraints::new()
                .os([OsType::Windows])
                .device(DeviceCategory::Mobile)
                .strict(true),
        )
        .expect_err("Windows cannot be mobile in strict mode");
        assert!(matches!(err, FingerprintError::Incoherent { .. }));
    }

    #[test]
    fn header_generator_preserves_browser_wire_order_for_navigation_and_fetch() {
        let fp = BayesianSampler::sample(
            &SampleConstraints::new()
                .os([OsType::Windows])
                .browsers([BrowserType::Chromium])
                .seed(42),
        )
        .expect("windows chrome");
        let nav = HeaderGenerator::generate(&fp, RequestContext::Navigation, None);
        let names: Vec<&str> = nav.iter().map(|(k, _)| k.as_str()).collect();
        let ua_idx = names.iter().position(|n| *n == "User-Agent").unwrap();
        let accept_idx = names.iter().position(|n| *n == "Accept").unwrap();
        let fetch_idx = names.iter().position(|n| *n == "Sec-Fetch-Site").unwrap();
        assert!(ua_idx < accept_idx && accept_idx < fetch_idx);

        let api = HeaderGenerator::generate(
            &fp,
            RequestContext::CrossOriginApi,
            Some("https://example.com/"),
        );
        assert!(
            api.iter()
                .any(|(k, v)| k == "Sec-Fetch-Site" && v == "cross-site")
        );
        assert!(
            api.iter()
                .any(|(k, v)| k == "Referer" && v == "https://example.com/")
        );

        // Navigation with Referer must not emit contradictory `Sec-Fetch-Site: none`.
        let referred_nav = HeaderGenerator::generate(
            &fp,
            RequestContext::Navigation,
            Some("https://example.com/"),
        );
        assert!(
            referred_nav
                .iter()
                .any(|(k, v)| k == "Sec-Fetch-Site" && v == "cross-site")
        );

        // Safari/iOS must not advertise `zstd` in Accept-Encoding.
        let ios = BayesianSampler::sample(&SampleConstraints::new().os([OsType::Ios]).seed(9))
            .expect("ios safari");
        let ios_headers = HeaderGenerator::generate(&ios, RequestContext::Navigation, None);
        assert!(
            ios_headers
                .iter()
                .any(|(k, v)| k == "Accept-Encoding" && v == "gzip, deflate, br")
        );
        let pw_ios = FingerprintConverter::to_playwright_context_options(&ios);
        assert_eq!(pw_ios["isMobile"], true);
        assert_eq!(pw_ios["hasTouch"], true);
    }

    #[test]
    fn converter_emits_cdp_commands_and_launch_args_and_auditor_catches_contradictions() {
        let mut fp = BayesianSampler::sample(&SampleConstraints::new().os([OsType::Macos]).seed(7))
            .expect("macos sample");
        let cdp = FingerprintConverter::to_cdp_commands(&fp);
        assert!(
            cdp.iter()
                .any(|(m, _)| m == "Emulation.setUserAgentOverride")
        );
        assert!(
            cdp.iter()
                .any(|(m, _)| m == "Emulation.setDeviceMetricsOverride")
        );
        let args = FingerprintConverter::to_chromium_launch_args(&fp);
        assert!(args.iter().any(|a| a.starts_with("--window-size=")));

        // Inject a deliberate contradiction: macOS UA + Direct3D renderer.
        fp.gpu.renderer = "ANGLE (NVIDIA Direct3D11)".to_owned();
        let report = audit_coherence(&fp);
        assert!(!report.is_clean());
        assert!(report.score < 100);
    }

    #[test]
    fn sample_with_headers_and_playwright_and_json_converters_work() {
        let constraints = SampleConstraints::new()
            .os([OsType::Windows])
            .browsers([BrowserType::Chromium])
            .seed(99);
        let (fp, headers) =
            BayesianSampler::sample_with_headers(&constraints, RequestContext::Navigation)
                .expect("sample with headers");
        assert!(headers.iter().any(|(k, _)| k == "User-Agent"));
        assert!(headers.iter().any(|(k, _)| k == "Sec-Fetch-Mode"));

        let pw = FingerprintConverter::to_playwright_context_options(&fp);
        assert!(pw.get("userAgent").is_some());
        assert_eq!(pw["viewport"]["width"], json!(fp.display.width));
        assert_eq!(pw["viewport"]["height"], json!(fp.display.height));
        assert_eq!(pw["deviceScaleFactor"], json!(fp.display.pixel_ratio));
        assert!(pw.get("locale").is_some());
        assert!(pw.get("timezoneId").is_some());
        assert!(pw.get("colorScheme").is_some());
        assert!(pw.get("extraHTTPHeaders").is_some());

        let pretty = FingerprintConverter::to_json_pretty(&fp).expect("pretty json");
        let parsed: Value = serde_json::from_str(&pretty).expect("valid json");
        assert!(parsed.get("profile").is_some());
        assert!(parsed.get("display").is_some());
    }

    #[test]
    fn sampler_covers_batch_os_variants_version_validation_and_coherence_checks() {
        // Invalid version_range errors even without strict mode.
        let bad_ver = BayesianSampler::sample(&SampleConstraints::new().version_range(150, 140))
            .expect_err("min > max must fail");
        assert!(matches!(bad_ver, FingerprintError::Incoherent { .. }));

        // Strict browser incompatibility with iOS fails cleanly; non-strict falls back.
        let ios_chrome = SampleConstraints::new()
            .os([OsType::Ios])
            .browsers([BrowserType::Chromium])
            .strict(true);
        assert!(BayesianSampler::sample(&ios_chrome).is_err());

        let ios_fallback = BayesianSampler::sample(&ios_chrome.strict(false)).expect("fallback");
        assert_eq!(
            ios_fallback.profile.browser,
            Some(BrowserType::MobileSafari)
        );

        // Non-strict impossible OS+device falls back to Windows.
        let win_mobile_fallback = BayesianSampler::sample(
            &SampleConstraints::new()
                .os([OsType::Windows])
                .device(DeviceCategory::Mobile)
                .strict(false),
        )
        .expect("non-strict fallback");
        assert_eq!(win_mobile_fallback.profile.os, Some(OsType::Windows));

        // Batch + explicit locales, screen, timezone, color_scheme, and Linux/Android sampling.
        let constraints = SampleConstraints::new()
            .os([OsType::Linux, OsType::Android])
            .locales(vec!["de-DE".to_owned()])
            .screen(ScreenConstraints::new().min_width(1000).max_width(2000))
            .seed(101);
        let mut with_tz = constraints.clone();
        with_tz.timezone = Some("Europe/Berlin".to_owned());
        with_tz.color_scheme = Some(ColorScheme::Dark);
        let batch = BayesianSampler::sample_batch(&with_tz, 4).expect("batch");
        assert_eq!(batch.len(), 4);
        assert_eq!(batch[0].profile.timezone.as_deref(), Some("Europe/Berlin"));
        assert_eq!(batch[0].profile.color_scheme, Some(ColorScheme::Dark));

        // Fetch and Subresource contexts emit expected Sec-Fetch headers.
        let fetch_hdrs = HeaderGenerator::generate(&batch[0], RequestContext::Fetch, None);
        assert!(
            fetch_hdrs
                .iter()
                .any(|(k, v)| k == "Sec-Fetch-Mode" && v == "cors")
        );
        let sub_hdrs = HeaderGenerator::generate(&batch[0], RequestContext::Subresource, None);
        assert!(
            sub_hdrs
                .iter()
                .any(|(k, v)| k == "Sec-Fetch-Dest" && v == "script")
        );

        // Audit contradictions: Windows UA + MacIntel platform + Apple GPU + macOS Client Hints.
        let mut bad_win = BayesianSampler::sample(
            &SampleConstraints::new()
                .os([OsType::Windows])
                .browsers([BrowserType::Chromium])
                .seed(11),
        )
        .expect("win");
        bad_win.profile.platform = Some("MacIntel".to_owned());
        bad_win.gpu.renderer = "Apple M3".to_owned();
        if let Some(ch) = bad_win.profile.client_hints.as_mut() {
            ch.platform = "macOS".to_owned();
        }
        let report = audit_coherence(&bad_win);
        assert!(report.issues.len() >= 3);
        assert!(report.score < 50);

        // macOS UA + Win32 platform + Windows Client Hints.
        let mut bad_mac =
            BayesianSampler::sample(&SampleConstraints::new().os([OsType::Macos]).seed(12))
                .expect("mac");
        bad_mac.profile.platform = Some("Win32".to_owned());
        if let Some(ch) = bad_mac.profile.client_hints.as_mut() {
            ch.platform = "Windows".to_owned();
        }
        let mac_report = audit_coherence(&bad_mac);
        assert!(mac_report.issues.len() >= 2);
    }
}
