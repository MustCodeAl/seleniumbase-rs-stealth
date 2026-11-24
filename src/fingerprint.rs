//! Unified profile fingerprint representation and advanced generator.
//!
//! This module provides:
//! 1. [`ProfileFingerprint`]: A unified JSON-serializable fingerprint model whose field
//!    names align directly with the [`Fingerprint`] API methods (`user_agent`, `platform`,
//!    `languages`, `hardware_concurrency`, `device_memory`, `webgl_vendor`, `webgl_renderer`,
//!    `canvas_noise`, `audio_noise`, `webrtc_policy`) instead of obscure masked names.
//! 2. [`FingerprintGenerator`]: An enriched generator supporting advanced parameters inspired by
//!    Scrapfly `fingerprint-generator`, Browserforge, and Apify `fingerprint-suite` (color gamut,
//!    HDR, WebGL extension suites, audio sample rates, and comprehensive font enumeration lists).
//! 3. [`GeneratedFingerprint`]: The output bundle combining the unified profile, the core
//!    [`Fingerprint`], HTTP/Client Hints headers, and hardware signatures.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use seleniumbase_rs::evasion::fingerprint::{
    AudioNoise, BrowserType, CanvasNoise, Fingerprint, FingerprintError, OsType, WebRtcPolicy,
};
use seleniumbase_rs::evasion::settings::{ColorGamut, MediaFeatures, WebGl};

/// High Dynamic Range (HDR) screen capability mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HdrMode {
    /// Standard dynamic range display (SDR).
    #[default]
    Standard,
    /// High dynamic range display (HDR).
    High,
}

impl HdrMode {
    /// Returns the CSS `@media (dynamic-range: ...)` value.
    #[must_use]
    pub const fn media_value(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::High => "high",
        }
    }
}

/// Web Audio API characteristics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioMetadata {
    /// Audio sample rate in Hz (e.g. 44100 or 48000).
    pub sample_rate: u32,
    /// Maximum channel count (typically 2 for stereo).
    pub max_channel_count: u32,
    /// Estimated output latency in seconds.
    pub output_latency: f64,
}

impl Default for AudioMetadata {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            max_channel_count: 2,
            output_latency: 0.015,
        }
    }
}

/// WebGL GPU metadata including extensions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuMetadata {
    /// Unmasked WebGL vendor string (e.g. `Google Inc. (NVIDIA)` or `Apple`).
    pub vendor: String,
    /// Unmasked WebGL renderer string (e.g. `ANGLE (NVIDIA, ...) Direct3D11` or `Apple M2`).
    pub renderer: String,
    /// Supported WebGL extensions.
    #[serde(default)]
    pub extensions: Vec<String>,
}

/// Display and screen characteristics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayMetadata {
    /// Screen width in CSS pixels.
    pub width: u32,
    /// Screen height in CSS pixels.
    pub height: u32,
    /// Available screen width (excluding taskbar/dock).
    pub avail_width: u32,
    /// Available screen height (excluding taskbar/dock).
    pub avail_height: u32,
    /// Device pixel ratio (e.g. 1.0, 1.25, 2.0).
    pub pixel_ratio: f64,
    /// Color depth (e.g. 24 or 30 bits).
    pub color_depth: u32,
    /// Display color gamut (sRGB, P3, Rec2020).
    pub color_gamut: ColorGamut,
    /// High dynamic range mode.
    pub hdr: HdrMode,
}

impl Default for DisplayMetadata {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            avail_width: 1920,
            avail_height: 1040,
            pixel_ratio: 1.0,
            color_depth: 24,
            color_gamut: ColorGamut::Srgb,
            hdr: HdrMode::Standard,
        }
    }
}

/// Helper to deserialize languages from either a JSON array of strings `["en-US", "en"]`
/// or a comma-separated string `"en-US,en"`.
fn deserialize_languages<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum LangInput {
        List(Vec<String>),
        CommaSeparated(String),
    }

    match LangInput::deserialize(deserializer)? {
        LangInput::List(list) => Ok(list),
        LangInput::CommaSeparated(s) => Ok(s
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(ToString::to_string)
            .collect()),
    }
}

fn parse_canvas_noise_str(s: &str) -> Option<CanvasNoise> {
    match s.trim().to_ascii_lowercase().as_str() {
        "natural" | "disabled" | "none" | "off" | "auto" => Some(CanvasNoise::Natural),
        "persistent" | "mask" | "deterministic" => Some(CanvasNoise::Persistent),
        "random" => Some(CanvasNoise::Random),
        "low" => Some(CanvasNoise::Low),
        _ => None,
    }
}

fn deserialize_canvas_noise_opt<'de, D>(deserializer: D) -> Result<Option<CanvasNoise>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        Typed(CanvasNoise),
        Text(String),
    }

    let opt = Option::<Helper>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(Helper::Typed(val)) => Ok(Some(val)),
        Some(Helper::Text(s)) => parse_canvas_noise_str(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown canvas_noise variant: {s}"))),
    }
}

fn parse_audio_noise_str(s: &str) -> Option<AudioNoise> {
    match s.trim().to_ascii_lowercase().as_str() {
        "natural" | "disabled" | "none" | "off" | "auto" => Some(AudioNoise::Natural),
        "persistent" | "mask" | "deterministic" => Some(AudioNoise::Persistent),
        "random" => Some(AudioNoise::Random),
        _ => None,
    }
}

fn deserialize_audio_noise_opt<'de, D>(deserializer: D) -> Result<Option<AudioNoise>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        Typed(AudioNoise),
        Text(String),
    }

    let opt = Option::<Helper>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(Helper::Typed(val)) => Ok(Some(val)),
        Some(Helper::Text(s)) => parse_audio_noise_str(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown audio_noise variant: {s}"))),
    }
}

fn parse_webrtc_policy_str(s: &str) -> Option<WebRtcPolicy> {
    match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "disable_non_proxied_udp" | "disablenonproxiedudp" | "disabled" | "disable" => {
            Some(WebRtcPolicy::DisableNonProxiedUdp)
        }
        "public_and_private_interfaces" | "publicandprivateinterfaces" | "public_and_private" => {
            Some(WebRtcPolicy::PublicAndPrivateInterfaces)
        }
        "public_interface_only" | "publicinterfaceonly" | "public_only" => {
            Some(WebRtcPolicy::PublicInterfaceOnly)
        }
        _ => None,
    }
}

fn deserialize_webrtc_policy_opt<'de, D>(deserializer: D) -> Result<Option<WebRtcPolicy>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        Typed(WebRtcPolicy),
        Text(String),
    }

    let opt = Option::<Helper>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(Helper::Typed(val)) => Ok(Some(val)),
        Some(Helper::Text(s)) => parse_webrtc_policy_str(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown webrtc_policy variant: {s}"))),
    }
}

fn parse_os_type_str(s: &str) -> Option<OsType> {
    match s.trim().to_ascii_lowercase().as_str() {
        "windows" | "win" => Some(OsType::Windows),
        "macos" | "mac" | "darwin" | "osx" => Some(OsType::Macos),
        "linux" => Some(OsType::Linux),
        "android" => Some(OsType::Android),
        "ios" | "ipados" | "iphone" | "ipad" => Some(OsType::Ios),
        _ => None,
    }
}

fn deserialize_os_type_opt<'de, D>(deserializer: D) -> Result<Option<OsType>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        Typed(OsType),
        Text(String),
    }

    let opt = Option::<Helper>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(Helper::Typed(val)) => Ok(Some(val)),
        Some(Helper::Text(s)) => parse_os_type_str(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown os variant: {s}"))),
    }
}

fn parse_browser_type_str(s: &str) -> Option<BrowserType> {
    match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "chromium" | "chrome" => Some(BrowserType::Chromium),
        "firefox" | "gecko" => Some(BrowserType::Firefox),
        "mobile_safari" | "mobilesafari" | "safari" | "webkit" => Some(BrowserType::MobileSafari),
        _ => None,
    }
}

fn deserialize_browser_type_opt<'de, D>(deserializer: D) -> Result<Option<BrowserType>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        Typed(BrowserType),
        Text(String),
    }

    let opt = Option::<Helper>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(Helper::Typed(val)) => Ok(Some(val)),
        Some(Helper::Text(s)) => parse_browser_type_str(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown browser variant: {s}"))),
    }
}

fn parse_color_gamut_str(s: &str) -> Option<ColorGamut> {
    match s.trim().to_ascii_lowercase().replace('-', "").as_str() {
        "srgb" => Some(ColorGamut::Srgb),
        "p3" | "displayp3" => Some(ColorGamut::P3),
        "rec2020" => Some(ColorGamut::Rec2020),
        _ => None,
    }
}

fn deserialize_color_gamut_opt<'de, D>(deserializer: D) -> Result<Option<ColorGamut>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        Typed(ColorGamut),
        Text(String),
    }

    let opt = Option::<Helper>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(Helper::Typed(val)) => Ok(Some(val)),
        Some(Helper::Text(s)) => parse_color_gamut_str(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown color_gamut variant: {s}"))),
    }
}

/// Unified profile fingerprint matching the `Fingerprint` API methods.
///
/// This struct uses the exact, idiomatic names from the [`Fingerprint`] API
/// (`user_agent`, `platform`, `languages`, `hardware_concurrency`, `device_memory`,
/// `webgl_vendor`, `webgl_renderer`, `canvas_noise`, `audio_noise`, `webrtc_policy`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileFingerprint {
    /// Operating system persona.
    #[serde(default, deserialize_with = "deserialize_os_type_opt")]
    pub os: Option<OsType>,

    /// Browser persona.
    #[serde(default, deserialize_with = "deserialize_browser_type_opt")]
    pub browser: Option<BrowserType>,

    /// `navigator.userAgent` and the `User-Agent` HTTP header.
    #[serde(default)]
    pub user_agent: Option<String>,

    /// `navigator.platform`.
    #[serde(default)]
    pub platform: Option<String>,

    /// `navigator.languages`. Accepts both JSON arrays `["en-US", "en"]` and
    /// comma-separated strings `"en-US,en"`.
    #[serde(default, deserialize_with = "deserialize_languages")]
    pub languages: Vec<String>,

    /// `navigator.hardwareConcurrency` (logical CPU core count).
    #[serde(default)]
    pub hardware_concurrency: Option<u32>,

    /// `navigator.deviceMemory` (RAM in GB).
    #[serde(default)]
    pub device_memory: Option<f64>,

    /// WebGL vendor string.
    #[serde(default)]
    pub webgl_vendor: Option<String>,

    /// WebGL renderer string.
    #[serde(default)]
    pub webgl_renderer: Option<String>,

    /// WebGL supported extensions.
    #[serde(default)]
    pub webgl_extensions: Vec<String>,

    /// Canvas noise policy.
    #[serde(default, deserialize_with = "deserialize_canvas_noise_opt")]
    pub canvas_noise: Option<CanvasNoise>,

    /// Audio context noise policy.
    #[serde(default, deserialize_with = "deserialize_audio_noise_opt")]
    pub audio_noise: Option<AudioNoise>,

    /// WebRTC IP handling policy.
    #[serde(default, deserialize_with = "deserialize_webrtc_policy_opt")]
    pub webrtc_policy: Option<WebRtcPolicy>,

    /// Screen color gamut (`srgb`, `p3`, `rec2020`).
    #[serde(default, deserialize_with = "deserialize_color_gamut_opt")]
    pub color_gamut: Option<ColorGamut>,

    /// Whether display supports High Dynamic Range.
    #[serde(default)]
    pub hdr: Option<bool>,

    /// Web Audio sample rate in Hz.
    #[serde(default)]
    pub audio_sample_rate: Option<u32>,

    /// Installed / enumerated font list.
    #[serde(default)]
    pub fonts: Vec<String>,

    /// Screen viewport width in CSS pixels.
    #[serde(default)]
    pub screen_width: Option<u32>,

    /// Screen viewport height in CSS pixels.
    #[serde(default)]
    pub screen_height: Option<u32>,

    /// Screen device pixel ratio.
    #[serde(default)]
    pub device_pixel_ratio: Option<f64>,

    /// Screen color depth.
    #[serde(default)]
    pub color_depth: Option<u32>,

    /// IANA time zone identifier (e.g. `America/New_York`, `Europe/Berlin`).
    #[serde(default)]
    pub timezone: Option<String>,
}

impl Default for ProfileFingerprint {
    fn default() -> Self {
        Self {
            os: Some(OsType::Windows),
            browser: Some(BrowserType::Chromium),
            user_agent: None,
            platform: None,
            languages: vec!["en-US".to_owned(), "en".to_owned()],
            hardware_concurrency: Some(8),
            device_memory: Some(8.0),
            webgl_vendor: None,
            webgl_renderer: None,
            webgl_extensions: Vec::new(),
            canvas_noise: Some(CanvasNoise::Persistent),
            audio_noise: Some(AudioNoise::Persistent),
            webrtc_policy: Some(WebRtcPolicy::DisableNonProxiedUdp),
            color_gamut: Some(ColorGamut::Srgb),
            hdr: Some(false),
            audio_sample_rate: Some(48000),
            fonts: Vec::new(),
            screen_width: Some(1920),
            screen_height: Some(1080),
            device_pixel_ratio: Some(1.0),
            color_depth: Some(24),
            timezone: Some("America/New_York".to_owned()),
        }
    }
}

impl ProfileFingerprint {
    /// Builds an idiomatic [`Fingerprint`] from this profile.
    ///
    /// # Errors
    ///
    /// Returns [`FingerprintError`] if the settings contain contradictory values
    /// or cannot be validated.
    pub fn to_fingerprint(&self) -> Result<Fingerprint, FingerprintError> {
        let mut builder = Fingerprint::builder();

        if let Some(os) = self.os {
            builder = builder.os_type(os);
        }
        if let Some(browser) = self.browser {
            builder = builder.browser_type(browser);
        }
        if let Some(ua) = &self.user_agent {
            builder = builder.user_agent(ua.clone());
        }
        if let Some(platform) = &self.platform {
            builder = builder.platform(platform.clone());
        }
        if !self.languages.is_empty() {
            builder = builder.languages(self.languages.join(","));
            if let Some(first) = self.languages.first() {
                builder = builder.locale(first.clone());
            }
        }
        if let Some(hw) = self.hardware_concurrency {
            builder = builder.hardware_concurrency(hw);
        }
        if let Some(mem) = self.device_memory {
            builder = builder.device_memory(mem);
        }
        if let (Some(w), Some(h)) = (self.screen_width, self.screen_height) {
            builder = builder.screen(w, h);
        }
        if let Some(dpr) = self.device_pixel_ratio {
            builder = builder.pixel_ratio(dpr);
        }
        if let Some(cd) = self.color_depth {
            builder = builder.color_depth(cd);
        }
        if let Some(tz) = &self.timezone {
            builder = builder.timezone(tz.clone());
        }
        if let (Some(vendor), Some(renderer)) = (&self.webgl_vendor, &self.webgl_renderer) {
            builder = builder.webgl(WebGl::new(vendor.clone(), renderer.clone()));
        } else if let Some(renderer) = &self.webgl_renderer {
            builder = builder.webgl(WebGl::renderer_only(renderer.clone()));
        } else if let Some(vendor) = &self.webgl_vendor {
            builder = builder.webgl(WebGl::vendor_only(vendor.clone()));
        }

        if !self.fonts.is_empty() {
            builder = builder.fonts(self.fonts.clone());
        }

        if let Some(canvas) = self.canvas_noise {
            builder = builder.canvas_noise(canvas);
        }
        if let Some(audio) = self.audio_noise {
            builder = builder.audio_noise(audio);
        }
        if let Some(webrtc) = self.webrtc_policy {
            builder = builder.webrtc_policy(webrtc);
        }
        if let Some(gamut) = self.color_gamut {
            builder = builder.media_features(MediaFeatures::new(gamut));
        }

        builder.build()
    }

    /// Creates a [`ProfileFingerprint`] extracted from a built [`Fingerprint`].
    #[must_use]
    pub fn from_fingerprint(fp: &Fingerprint) -> Self {
        let screen = fp.screen().value();
        let webgl = fp.webgl().value();
        let languages = fp.languages().value().map_or_else(Vec::new, |s| {
            s.split(',')
                .map(str::trim)
                .map(ToString::to_string)
                .collect()
        });

        Self {
            os: Some(fp.os_type()),
            browser: Some(fp.browser_type()),
            user_agent: fp.user_agent().value().map(ToString::to_string),
            platform: fp.platform().value().map(ToString::to_string),
            languages,
            hardware_concurrency: fp.hardware_concurrency().value().copied(),
            device_memory: fp.device_memory().value().copied(),
            webgl_vendor: webgl.and_then(|w| w.vendor.clone()),
            webgl_renderer: webgl.and_then(|w| w.renderer.clone()),
            webgl_extensions: Vec::new(),
            canvas_noise: Some(fp.policy().canvas_noise),
            audio_noise: Some(fp.policy().audio_noise),
            webrtc_policy: fp.policy().webrtc_policy,
            color_gamut: fp.media_features().value().map(|m| m.color_gamut),
            hdr: Some(false),
            audio_sample_rate: Some(
                if fp.os_type().is_mobile() || fp.os_type() == OsType::Macos {
                    44100
                } else {
                    48000
                },
            ),
            fonts: fp.fonts().value().cloned().unwrap_or_default(),
            screen_width: screen.map(|s| s.width),
            screen_height: screen.map(|s| s.height),
            device_pixel_ratio: fp.pixel_ratio().value().copied(),
            color_depth: fp.color_depth().value().copied(),
            timezone: fp.timezone().value().map(ToString::to_string),
        }
    }

    /// Serializes this profile to formatted JSON.
    ///
    /// # Errors
    ///
    /// Returns [`serde_json::Error`] if serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes a profile from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`serde_json::Error`] if parsing fails.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ---------------------------------------------------------------------------
// Standard WebGL extension sets from Scrapfly fpgen and Apify fingerprint-suite
// ---------------------------------------------------------------------------

static COMMON_WEBGL_EXTENSIONS: [&str; 24] = [
    "ANGLE_instanced_arrays",
    "EXT_blend_minmax",
    "EXT_color_buffer_half_float",
    "EXT_float_blend",
    "EXT_frag_depth",
    "EXT_shader_texture_lod",
    "EXT_sRGB",
    "KHR_parallel_shader_compile",
    "OES_element_index_uint",
    "OES_fbo_render_mipmap",
    "OES_standard_derivatives",
    "OES_texture_float",
    "OES_texture_float_linear",
    "OES_texture_half_float",
    "OES_texture_half_float_linear",
    "OES_vertex_array_object",
    "WEBGL_color_buffer_float",
    "WEBGL_compressed_texture_s3tc",
    "WEBGL_compressed_texture_s3tc_srgb",
    "WEBGL_debug_renderer_info",
    "WEBGL_debug_shaders",
    "WEBGL_depth_texture",
    "WEBGL_draw_buffers",
    "WEBGL_lose_context",
];

static APPLE_WEBGL_EXTENSIONS: [&str; 23] = [
    "ANGLE_instanced_arrays",
    "EXT_blend_minmax",
    "EXT_color_buffer_half_float",
    "EXT_clip_control",
    "EXT_depth_clamp",
    "EXT_float_blend",
    "EXT_frag_depth",
    "EXT_polygon_offset_clamp",
    "EXT_shader_texture_lod",
    "EXT_texture_compression_bptc",
    "EXT_texture_compression_rgtc",
    "EXT_texture_filter_anisotropic",
    "KHR_parallel_shader_compile",
    "OES_element_index_uint",
    "OES_fbo_render_mipmap",
    "OES_standard_derivatives",
    "OES_texture_float",
    "OES_texture_float_linear",
    "OES_texture_half_float",
    "OES_vertex_array_object",
    "WEBGL_color_buffer_float",
    "WEBGL_debug_renderer_info",
    "WEBGL_lose_context",
];

// ---------------------------------------------------------------------------
// Comprehensive Font Lists per OS (matching Apify / Browserforge / Scrapfly)
// ---------------------------------------------------------------------------

static WINDOWS_FONTS: [&str; 52] = [
    "Arial",
    "Arial Black",
    "Bahnschrift",
    "Calibri",
    "Cambria",
    "Cambria Math",
    "Candara",
    "Comic Sans MS",
    "Consolas",
    "Constantia",
    "Corbel",
    "Courier New",
    "Ebrima",
    "Franklin Gothic Medium",
    "Gabriola",
    "Gadugi",
    "Georgia",
    "Impact",
    "Ink Free",
    "Javanese Text",
    "Leelawadee UI",
    "Lucida Console",
    "Lucida Sans Unicode",
    "Malgun Gothic",
    "Marlett",
    "Microsoft Himalaya",
    "Microsoft JhengHei",
    "Microsoft New Tai Lue",
    "Microsoft PhagsPa",
    "Microsoft Sans Serif",
    "Microsoft Tai Le",
    "Microsoft YaHei",
    "Microsoft Yi Baiti",
    "Mongolian Baiti",
    "MS Gothic",
    "MS PGothic",
    "MV Boli",
    "Myanmar Text",
    "Nirmala UI",
    "Palatino Linotype",
    "Segoe Print",
    "Segoe Script",
    "Segoe UI",
    "Segoe UI Emoji",
    "Segoe UI Historic",
    "Segoe UI Symbol",
    "SimSun",
    "Sitka Small",
    "Sylfaen",
    "Symbol",
    "Tahoma",
    "Times New Roman",
];

static MACOS_FONTS: [&str; 48] = [
    "American Typewriter",
    "Andale Mono",
    "Apple Braille",
    "Apple Chancery",
    "Apple Color Emoji",
    "Apple SD Gothic Neo",
    "Arial",
    "Arial Black",
    "Arial Hebrew",
    "Arial Narrow",
    "Arial Rounded MT Bold",
    "Avenir",
    "Avenir Next",
    "Avenir Next Condensed",
    "Baskerville",
    "Big Caslon",
    "Bodoni 72",
    "Bradley Hand",
    "Brush Script MT",
    "Chalkboard",
    "Chalkboard SE",
    "Chalkduster",
    "Charter",
    "Cochin",
    "Comic Sans MS",
    "Copperplate",
    "Courier",
    "Courier New",
    "Didot",
    "DIN Alternate",
    "DIN Condensed",
    "Futura",
    "Geneva",
    "Georgia",
    "Gill Sans",
    "Helvetica",
    "Helvetica Neue",
    "Herculanum",
    "Hoefler Text",
    "Impact",
    "Lucida Grande",
    "Marker Felt",
    "Menlo",
    "Monaco",
    "Noteworthy",
    "Optima",
    "Palatino",
    "Papyrus",
];

static LINUX_FONTS: [&str; 24] = [
    "Cantarell",
    "DejaVu Sans",
    "DejaVu Sans Mono",
    "DejaVu Serif",
    "Droid Sans",
    "FreeMono",
    "FreeSans",
    "FreeSerif",
    "Liberation Mono",
    "Liberation Sans",
    "Liberation Serif",
    "Noto Color Emoji",
    "Noto Mono",
    "Noto Sans",
    "Noto Sans CJK SC",
    "Noto Serif",
    "Open Sans",
    "Roboto",
    "Ubuntu",
    "Ubuntu Condensed",
    "Ubuntu Mono",
    "URW Bookman",
    "URW Gothic",
    "Nimbus Roman",
];

/// Advanced Fingerprint Generator.
///
/// Implements realistic, coherent browser profile generation based on empirical distributions
/// from Scrapfly `fingerprint-generator`, Browserforge, and Apify `fingerprint-suite`.
#[derive(Clone, Debug)]
pub struct FingerprintGenerator {
    seed: u64,
    os: OsType,
    browser: BrowserType,
    major: u32,
    color_gamut: Option<ColorGamut>,
    hdr: Option<bool>,
    audio_sample_rate: Option<u32>,
    fonts: Option<Vec<String>>,
    canvas_noise: CanvasNoise,
    audio_noise: AudioNoise,
    webrtc_policy: WebRtcPolicy,
}

impl FingerprintGenerator {
    /// Creates a new generator seeded by `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            os: OsType::Windows,
            browser: BrowserType::Chromium,
            major: 157,
            color_gamut: None,
            hdr: None,
            audio_sample_rate: None,
            fonts: None,
            canvas_noise: CanvasNoise::Persistent,
            audio_noise: AudioNoise::Persistent,
            webrtc_policy: WebRtcPolicy::DisableNonProxiedUdp,
        }
    }

    /// Sets the operating system persona.
    #[must_use]
    pub const fn os(mut self, os: OsType) -> Self {
        self.os = os;
        self
    }

    /// Sets the browser persona.
    #[must_use]
    pub const fn browser(mut self, browser: BrowserType) -> Self {
        self.browser = browser;
        self
    }

    /// Sets the Chromium major version to claim.
    #[must_use]
    pub const fn major(mut self, major: u32) -> Self {
        self.major = major;
        self
    }

    /// Sets display color gamut (`srgb`, `p3`, `rec2020`).
    #[must_use]
    pub const fn color_gamut(mut self, gamut: ColorGamut) -> Self {
        self.color_gamut = Some(gamut);
        self
    }

    /// Sets High Dynamic Range capability.
    #[must_use]
    pub const fn hdr(mut self, hdr: bool) -> Self {
        self.hdr = Some(hdr);
        self
    }

    /// Sets audio sample rate in Hz.
    #[must_use]
    pub const fn audio_sample_rate(mut self, rate: u32) -> Self {
        self.audio_sample_rate = Some(rate);
        self
    }

    /// Overrides the font enumeration list.
    #[must_use]
    pub fn fonts(mut self, fonts: Vec<String>) -> Self {
        self.fonts = Some(fonts);
        self
    }

    /// Sets canvas noise spoofing mode.
    #[must_use]
    pub const fn canvas_noise(mut self, noise: CanvasNoise) -> Self {
        self.canvas_noise = noise;
        self
    }

    /// Sets audio context noise spoofing mode.
    #[must_use]
    pub const fn audio_noise(mut self, noise: AudioNoise) -> Self {
        self.audio_noise = noise;
        self
    }

    /// Sets WebRTC IP handling policy.
    #[must_use]
    pub const fn webrtc_policy(mut self, policy: WebRtcPolicy) -> Self {
        self.webrtc_policy = policy;
        self
    }

    /// Generates a coherent [`GeneratedFingerprint`] bundle.
    #[must_use]
    pub fn generate(&self) -> GeneratedFingerprint {
        let (gpu_vendor, gpu_renderer, gpu_extensions) = self.generate_gpu();
        let display = self.generate_display();
        let audio = self.generate_audio();
        let fonts = self.generate_fonts();
        let (user_agent, platform) = self.generate_navigator();

        let profile = ProfileFingerprint {
            os: Some(self.os),
            browser: Some(self.browser),
            user_agent: Some(user_agent.clone()),
            platform: Some(platform.clone()),
            languages: vec!["en-US".to_owned(), "en".to_owned()],
            hardware_concurrency: Some(match self.os {
                OsType::Windows => 12,
                _ => 8,
            }),
            device_memory: Some(8.0),
            webgl_vendor: Some(gpu_vendor.clone()),
            webgl_renderer: Some(gpu_renderer.clone()),
            webgl_extensions: gpu_extensions.clone(),
            canvas_noise: Some(self.canvas_noise),
            audio_noise: Some(self.audio_noise),
            webrtc_policy: Some(self.webrtc_policy),
            color_gamut: Some(display.color_gamut),
            hdr: Some(matches!(display.hdr, HdrMode::High)),
            audio_sample_rate: Some(audio.sample_rate),
            fonts: fonts.clone(),
            screen_width: Some(display.width),
            screen_height: Some(display.height),
            device_pixel_ratio: Some(display.pixel_ratio),
            color_depth: Some(display.color_depth),
            timezone: Some("America/New_York".to_owned()),
        };

        // Produce a verified core Fingerprint
        let fingerprint = Fingerprint::randomized_with_major(self.os, self.seed, self.major);

        let mut headers = BTreeMap::new();
        headers.insert("User-Agent".to_owned(), user_agent);
        headers.insert("Accept-Language".to_owned(), "en-US,en;q=0.9".to_owned());
        headers.insert(
            "Sec-CH-UA".to_owned(),
            format!(
                "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"{m}\", \"Google Chrome\";v=\"{m}\"",
                m = self.major
            ),
        );
        headers.insert("Sec-CH-UA-Mobile".to_owned(), "?0".to_owned());
        headers.insert(
            "Sec-CH-UA-Platform".to_owned(),
            format!(
                "\"{}\"",
                match self.os {
                    OsType::Macos => "macOS",
                    OsType::Linux => "Linux",
                    OsType::Android => "Android",
                    OsType::Ios => "iOS",
                    _ => "Windows",
                }
            ),
        );

        GeneratedFingerprint {
            profile,
            fingerprint,
            headers,
            audio,
            gpu: GpuMetadata {
                vendor: gpu_vendor,
                renderer: gpu_renderer,
                extensions: gpu_extensions,
            },
            display,
            fonts,
        }
    }

    fn generate_navigator(&self) -> (String, String) {
        match self.os {
            OsType::Macos => (
                format!(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36",
                    self.major
                ),
                "MacIntel".to_owned(),
            ),
            OsType::Linux => (
                format!(
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36",
                    self.major
                ),
                "Linux x86_64".to_owned(),
            ),
            OsType::Android => (
                format!(
                    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Mobile Safari/537.36",
                    self.major
                ),
                "Linux armv8l".to_owned(),
            ),
            OsType::Ios => (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1".to_string(),
                "iPhone".to_owned(),
            ),
            _ => (
                format!(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36",
                    self.major
                ),
                "Win32".to_owned(),
            ),
        }
    }

    fn generate_gpu(&self) -> (String, String, Vec<String>) {
        match self.os {
            OsType::Macos => (
                "Apple".to_owned(),
                "Apple M2".to_owned(),
                APPLE_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect(),
            ),
            OsType::Windows => (
                "Google Inc. (NVIDIA)".to_owned(),
                "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)"
                    .to_owned(),
                COMMON_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect(),
            ),
            OsType::Linux => (
                "Google Inc. (Intel)".to_owned(),
                "Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6".to_owned(),
                COMMON_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect(),
            ),
            OsType::Android => (
                "Google Inc. (Qualcomm)".to_owned(),
                "Adreno (TM) 730".to_owned(),
                COMMON_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect(),
            ),
            OsType::Ios => (
                "Apple".to_owned(),
                "Apple GPU".to_owned(),
                APPLE_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect(),
            ),
            _ => (
                "Google Inc. (NVIDIA)".to_owned(),
                "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)"
                    .to_owned(),
                COMMON_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect(),
            ),
        }
    }

    fn generate_display(&self) -> DisplayMetadata {
        let is_apple = matches!(self.os, OsType::Macos | OsType::Ios);
        let gamut = self.color_gamut.unwrap_or(if is_apple {
            ColorGamut::P3
        } else {
            ColorGamut::Srgb
        });
        let hdr = self.hdr.map_or(
            if is_apple {
                HdrMode::High
            } else {
                HdrMode::Standard
            },
            |h| if h { HdrMode::High } else { HdrMode::Standard },
        );

        match self.os {
            OsType::Macos => DisplayMetadata {
                width: 1440,
                height: 900,
                avail_width: 1440,
                avail_height: 875,
                pixel_ratio: 2.0,
                color_depth: 30,
                color_gamut: gamut,
                hdr,
            },
            _ => DisplayMetadata {
                width: 1920,
                height: 1080,
                avail_width: 1920,
                avail_height: 1040,
                pixel_ratio: 1.0,
                color_depth: 24,
                color_gamut: gamut,
                hdr,
            },
        }
    }

    fn generate_audio(&self) -> AudioMetadata {
        let rate = self.audio_sample_rate.unwrap_or(
            if matches!(self.os, OsType::Macos | OsType::Ios | OsType::Android) {
                44100
            } else {
                48000
            },
        );

        AudioMetadata {
            sample_rate: rate,
            max_channel_count: 2,
            output_latency: 0.012,
        }
    }

    fn generate_fonts(&self) -> Vec<String> {
        if let Some(custom) = &self.fonts {
            return custom.clone();
        }

        match self.os {
            OsType::Windows => WINDOWS_FONTS.iter().map(|&f| f.to_owned()).collect(),
            OsType::Macos => MACOS_FONTS.iter().map(|&f| f.to_owned()).collect(),
            _ => LINUX_FONTS.iter().map(|&f| f.to_owned()).collect(),
        }
    }
}

/// The complete generated fingerprint bundle.
#[derive(Clone, Debug)]
pub struct GeneratedFingerprint {
    /// Unified profile representation with clean API keyword names.
    pub profile: ProfileFingerprint,
    /// Validated [`Fingerprint`] instance ready for browser injection.
    pub fingerprint: Fingerprint,
    /// Matching HTTP client request headers.
    pub headers: BTreeMap<String, String>,
    /// Web Audio characteristics.
    pub audio: AudioMetadata,
    /// WebGL and GPU characteristics.
    pub gpu: GpuMetadata,
    /// Screen and display characteristics.
    pub display: DisplayMetadata,
    /// Enumerated font list.
    pub fonts: Vec<String>,
}

impl GeneratedFingerprint {
    /// Serializes this bundle into a JSON structure compatible with Scrapfly's
    /// `fingerprint-generator` output format.
    #[must_use]
    pub fn to_scrapfly_json(&self) -> serde_json::Value {
        serde_json::json!({
            "browser": match self.profile.browser.unwrap_or_default() {
                BrowserType::Firefox => "Firefox",
                BrowserType::MobileSafari => "Safari",
                _ => "Chrome",
            },
            "os": match self.profile.os.unwrap_or_default() {
                OsType::Macos => "macOS",
                OsType::Linux => "Linux",
                OsType::Android => "Android",
                OsType::Ios => "iOS",
                _ => "Windows",
            },
            "headers": self.headers,
            "navigator": {
                "userAgent": self.profile.user_agent,
                "platform": self.profile.platform,
                "languages": self.profile.languages,
                "language": self.profile.languages.first().cloned().unwrap_or_default(),
                "hardwareConcurrency": self.profile.hardware_concurrency,
                "deviceMemory": self.profile.device_memory,
                "maxTouchPoints": if self.profile.os.is_some_and(|os| os.is_mobile()) { 5 } else { 0 },
            },
            "screen": {
                "width": self.display.width,
                "height": self.display.height,
                "availWidth": self.display.avail_width,
                "availHeight": self.display.avail_height,
                "colorDepth": self.display.color_depth,
                "pixelRatio": self.display.pixel_ratio,
                "colorGamut": match self.display.color_gamut {
                    ColorGamut::Srgb => "srgb",
                    ColorGamut::P3 => "p3",
                    ColorGamut::Rec2020 => "rec2020",
                },
                "hdr": self.display.hdr.media_value(),
            },
            "gpu": {
                "vendor": self.gpu.vendor,
                "renderer": self.gpu.renderer,
                "unmaskedVendor": self.gpu.vendor,
                "unmaskedRenderer": self.gpu.renderer,
                "extensions": self.gpu.extensions,
            },
            "audio": {
                "sampleRate": self.audio.sample_rate,
                "maxChannelCount": self.audio.max_channel_count,
                "outputLatency": self.audio.output_latency,
            },
            "fonts": self.fonts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_fingerprint_deserializes_from_unified_keywords() {
        let json_data = r#"{
            "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/157.0.0.0",
            "platform": "Win32",
            "languages": ["en-US", "en"],
            "hardware_concurrency": 16,
            "device_memory": 16.0,
            "webgl_vendor": "Google Inc. (NVIDIA)",
            "webgl_renderer": "ANGLE (NVIDIA RTX 4070)",
            "canvas_noise": "random",
            "audio_noise": "persistent",
            "webrtc_policy": "disable_non_proxied_udp",
            "color_gamut": "srgb",
            "hdr": false,
            "audio_sample_rate": 48000,
            "fonts": ["Arial", "Segoe UI"],
            "screen_width": 2560,
            "screen_height": 1440,
            "device_pixel_ratio": 1.0,
            "timezone": "America/Chicago"
        }"#;

        let profile: ProfileFingerprint = serde_json::from_str(json_data).expect("valid profile");
        assert_eq!(
            profile.user_agent.as_deref(),
            Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/157.0.0.0")
        );
        assert_eq!(profile.languages, vec!["en-US", "en"]);
        assert_eq!(profile.hardware_concurrency, Some(16));
        assert_eq!(profile.device_memory, Some(16.0));
        assert_eq!(
            profile.webgl_vendor.as_deref(),
            Some("Google Inc. (NVIDIA)")
        );
        assert_eq!(
            profile.webgl_renderer.as_deref(),
            Some("ANGLE (NVIDIA RTX 4070)")
        );
        assert_eq!(profile.canvas_noise, Some(CanvasNoise::Random));
        assert_eq!(profile.audio_noise, Some(AudioNoise::Persistent));
        assert_eq!(
            profile.webrtc_policy,
            Some(WebRtcPolicy::DisableNonProxiedUdp)
        );
        assert_eq!(profile.color_gamut, Some(ColorGamut::Srgb));
        assert_eq!(profile.audio_sample_rate, Some(48000));
        assert_eq!(profile.screen_width, Some(2560));
        assert_eq!(profile.screen_height, Some(1440));
    }

    #[test]
    fn profile_fingerprint_deserializes_from_fingerprint_api_names() {
        let json_data = r#"{
            "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
            "webgl_vendor": "Apple",
            "webgl_renderer": "Apple M1",
            "audio_noise": "persistent",
            "webrtc_policy": "disable_non_proxied_udp",
            "canvas_noise": "persistent",
            "languages": ["de-DE", "de"]
        }"#;

        let profile: ProfileFingerprint = serde_json::from_str(json_data).expect("valid profile");
        assert_eq!(
            profile.user_agent.as_deref(),
            Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)")
        );
        assert_eq!(profile.webgl_vendor.as_deref(), Some("Apple"));
        assert_eq!(profile.webgl_renderer.as_deref(), Some("Apple M1"));
        assert_eq!(profile.audio_noise, Some(AudioNoise::Persistent));
        assert_eq!(
            profile.webrtc_policy,
            Some(WebRtcPolicy::DisableNonProxiedUdp)
        );
        assert_eq!(profile.canvas_noise, Some(CanvasNoise::Persistent));
        assert_eq!(profile.languages, vec!["de-DE", "de"]);
    }

    #[test]
    fn generator_creates_coherent_scrapfly_output() {
        let generator = FingerprintGenerator::new(42).os(OsType::Macos).major(157);
        let bundle = generator.generate();

        assert_eq!(bundle.profile.os, Some(OsType::Macos));
        assert_eq!(bundle.display.color_gamut, ColorGamut::P3);
        assert_eq!(bundle.audio.sample_rate, 44100);
        assert!(!bundle.gpu.extensions.is_empty());
        assert!(bundle.fonts.contains(&"Helvetica Neue".to_string()));

        let scrapfly_json = bundle.to_scrapfly_json();
        assert_eq!(scrapfly_json["os"], "macOS");
        assert_eq!(scrapfly_json["screen"]["colorGamut"], "p3");
        assert_eq!(scrapfly_json["audio"]["sampleRate"], 44100);
    }
}
