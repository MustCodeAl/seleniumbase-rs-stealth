//! Seeded fingerprint generator with correlated display, GPU, WebGPU, audio,
//! Client Hints, media device, battery, network, and font profiles.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::profile::{
    AudioDynamicsCompressor, AudioMetadata, BatteryMetadata, ClientHintsHighEntropy,
    DisplayMetadata, GpuMetadata, HdrMode, MediaDeviceInfo, MediaDeviceMetadata, NetworkMetadata,
    ProfileFingerprint, ReducedMotion, ScreenOrientation, ScreenOrientationType, WebGlParameters,
    WebGpuMetadata,
};
use super::randomize::{Locale, PRESET_CHROMIUM_MAJOR};
#[cfg(test)]
use super::randomize::{edge_hints, known_full_version};
use super::settings::DynamicRange;
use super::{
    AudioNoise, BrandVersion, BrowserType, CanvasNoise, ColorGamut, ColorScheme, ConnectionProfile,
    Fingerprint, FingerprintError, Geolocation, MediaFeatures, OsType, SpeechVoice, WebGl,
    WebRtcPolicy,
};

// ---------------------------------------------------------------------------
// Standard WebGL extension sets
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

static MOBILE_WEBGL_EXTENSIONS: [&str; 19] = [
    "ANGLE_instanced_arrays",
    "EXT_blend_minmax",
    "EXT_color_buffer_half_float",
    "EXT_float_blend",
    "EXT_frag_depth",
    "EXT_shader_texture_lod",
    "EXT_sRGB",
    "EXT_texture_filter_anisotropic",
    "OES_element_index_uint",
    "OES_fbo_render_mipmap",
    "OES_standard_derivatives",
    "OES_texture_float",
    "OES_texture_float_linear",
    "OES_texture_half_float",
    "OES_texture_half_float_linear",
    "OES_vertex_array_object",
    "WEBGL_compressed_texture_etc",
    "WEBGL_debug_renderer_info",
    "WEBGL_lose_context",
];

static ANDROID_FONTS: [&str; 8] = [
    "Roboto",
    "Noto Sans",
    "Noto Serif",
    "Droid Sans Mono",
    "Noto Color Emoji",
    "Cutive Mono",
    "Dancing Script",
    "Coming Soon",
];

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

/// Advanced Fingerprint Generator with empirical distributions.
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

/// Screen dimension and display constraints for [`FingerprintOptions`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenConstraints {
    /// Minimum screen width in CSS pixels.
    pub min_width: Option<u32>,
    /// Maximum screen width in CSS pixels.
    pub max_width: Option<u32>,
    /// Device pixel ratio override.
    pub device_pixel_ratio: Option<f64>,
    /// High dynamic range mode override.
    pub hdr_mode: Option<HdrMode>,
    /// Color depth in bits per pixel.
    pub color_depth: Option<u32>,
}

impl ScreenConstraints {
    /// Creates empty screen constraints.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            min_width: None,
            max_width: None,
            device_pixel_ratio: None,
            hdr_mode: None,
            color_depth: None,
        }
    }

    /// Sets the minimum screen width in CSS pixels.
    #[must_use]
    pub const fn min_width(mut self, width: u32) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Sets the maximum screen width in CSS pixels.
    #[must_use]
    pub const fn max_width(mut self, width: u32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Sets the device pixel ratio override.
    #[must_use]
    pub const fn device_pixel_ratio(mut self, dpr: f64) -> Self {
        self.device_pixel_ratio = Some(dpr);
        self
    }

    /// Sets the HDR mode override.
    #[must_use]
    pub const fn hdr_mode(mut self, mode: HdrMode) -> Self {
        self.hdr_mode = Some(mode);
        self
    }

    /// Sets the color depth in bits per pixel.
    #[must_use]
    pub const fn color_depth(mut self, depth: u32) -> Self {
        self.color_depth = Some(depth);
        self
    }
}

/// Network connection override for [`FingerprintOptions`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionOverride {
    /// Effective connection type (`"4g"`, `"3g"`, `"2g"`, `"slow-2g"`).
    pub effective_type: String,
    /// Round-trip time in milliseconds.
    pub rtt_ms: u32,
    /// Downlink bandwidth in Mbps.
    pub downlink_mbps: f64,
    /// Whether `saveData` is enabled.
    pub save_data: bool,
}

impl ConnectionOverride {
    /// Creates a new connection override.
    #[must_use]
    pub fn new(effective_type: impl Into<String>, rtt_ms: u32, downlink_mbps: f64) -> Self {
        Self {
            effective_type: effective_type.into(),
            rtt_ms,
            downlink_mbps,
            save_data: false,
        }
    }

    /// Sets the `saveData` flag.
    #[must_use]
    pub const fn save_data(mut self, enabled: bool) -> Self {
        self.save_data = enabled;
        self
    }
}

/// Explicit `navigator.getBattery()` override for [`FingerprintOptions`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatteryOverride {
    /// Whether the battery is currently charging.
    pub charging: bool,
    /// Battery charge level in `0.0..=1.0`.
    pub level: f64,
    /// Seconds until fully charged (`0.0` when full, `f64::INFINITY` when discharging).
    pub charging_time: f64,
    /// Seconds until discharged (`f64::INFINITY` when charging).
    pub discharging_time: f64,
}

impl BatteryOverride {
    /// Creates a battery override with coherent charging/discharging times.
    #[must_use]
    pub fn new(charging: bool, level: f64) -> Self {
        let clamped = level.clamp(0.0, 1.0);
        Self {
            charging,
            level: clamped,
            charging_time: if charging {
                if clamped >= 0.999 { 0.0 } else { 1800.0 }
            } else {
                f64::INFINITY
            },
            discharging_time: if charging {
                f64::INFINITY
            } else {
                (clamped * 18_000.0).max(600.0)
            },
        }
    }
}

/// Rich configuration options for [`FingerprintGenerator::generate_with`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FingerprintOptions {
    /// Operating system persona.
    pub os: Option<OsType>,
    /// Browser family persona.
    pub browser: Option<BrowserType>,
    /// Explicit User-Agent string override.
    pub user_agent: Option<String>,
    /// Explicit `navigator.platform` override.
    pub platform: Option<String>,
    /// Explicit screen `(width, height)` shorthand override.
    pub window_size: Option<(u32, u32)>,
    /// Additional HTTP headers merged into [`GeneratedFingerprint::headers`].
    pub custom_headers: BTreeMap<String, String>,
    /// Minimum Chromium major version.
    pub min_version: Option<u32>,
    /// Maximum Chromium major version.
    pub max_version: Option<u32>,
    /// Ordered `navigator.languages` list.
    pub locales: Option<Vec<String>>,
    /// IANA timezone identifier.
    pub timezone: Option<String>,
    /// Geolocation coordinates.
    pub geolocation: Option<Geolocation>,
    /// Screen geometry and display constraints.
    pub screen: Option<ScreenConstraints>,
    /// Explicit `(vendor, renderer)` WebGL strings.
    pub webgl_override: Option<(String, String)>,
    /// Explicit WebGL 1/2 `getParameter()` limits override.
    pub webgl_params: Option<WebGlParameters>,
    /// Explicit WebGL supported extension list override.
    pub webgl_extensions: Option<Vec<String>>,
    /// Independent deterministic seed for canvas noise.
    pub canvas_seed: Option<u64>,
    /// Independent deterministic seed for Web Audio noise.
    pub audio_seed: Option<u64>,
    /// Configurable `StorageManager.estimate().quota` in GiB.
    pub storage_quota_gb: Option<u64>,
    /// Configurable browser toolbar chrome height in CSS pixels.
    pub outer_chrome_height: Option<u32>,
    /// Configurable minimum `window.history.length` on fresh tabs.
    pub history_min_length: Option<u32>,
    /// Whether WebGPU spoofing is enabled.
    pub webgpu_enabled: bool,
    /// `navigator.hardwareConcurrency` override.
    pub hardware_concurrency: Option<u32>,
    /// `navigator.deviceMemory` override in GiB.
    pub device_memory: Option<u32>,
    /// `navigator.maxTouchPoints` override.
    pub max_touch_points: Option<u32>,
    /// `navigator.doNotTrack` override (`"1"`, `"0"`, `"unspecified"`).
    pub do_not_track: Option<String>,
    /// `navigator.pdfViewerEnabled` override.
    pub pdf_viewer_enabled: Option<bool>,
    /// Canvas noise amplitude override.
    pub canvas_noise_amplitude: Option<f64>,
    /// Audio noise amplitude override.
    pub audio_noise_amplitude: Option<f64>,
    /// Web Audio `AudioContext.sampleRate` override.
    pub audio_sample_rate: Option<u32>,
    /// Web Audio `DynamicsCompressorNode` parameter override.
    pub audio_compressor: Option<AudioDynamicsCompressor>,
    /// `DOMRect` jitter amplitude override.
    pub dom_rect_jitter: Option<f64>,
    /// Font metrics jitter amplitude override.
    pub font_metrics_jitter: Option<f64>,
    /// Explicit installed font list override.
    pub fonts: Option<Vec<String>>,
    /// Explicit `navigator.getBattery()` override.
    pub battery: Option<BatteryOverride>,
    /// Explicit `window.speechSynthesis.getVoices()` override.
    pub speech_voices: Option<Vec<SpeechVoice>>,
    /// CSS `prefers-color-scheme` override.
    pub color_scheme: Option<ColorScheme>,
    /// CSS `prefers-reduced-motion` override.
    pub reduced_motion: Option<ReducedMotion>,
    /// CSS `color-gamut` override.
    pub color_gamut: Option<ColorGamut>,
    /// CSS `dynamic-range` HDR mode override.
    pub hdr_mode: Option<HdrMode>,
    /// Deterministic sub-millisecond `performance.now()` jitter amplitude.
    pub performance_jitter_ms: Option<f64>,
    /// Whether `Worker` / `SharedWorker` navigator consistency spoofing is enabled.
    pub worker_spoofing: bool,
    /// WebRTC IP handling policy.
    pub webrtc_policy: Option<WebRtcPolicy>,
    /// `NetworkInformation` connection override.
    pub connection: Option<ConnectionOverride>,
    /// Synthetic media device counts `(cameras, microphones, speakers)`.
    pub media_devices: Option<(u32, u32, u32)>,
    /// Deterministic seed.
    pub seed: Option<u64>,
}

impl Default for FingerprintOptions {
    fn default() -> Self {
        Self {
            os: None,
            browser: None,
            user_agent: None,
            platform: None,
            window_size: None,
            custom_headers: BTreeMap::new(),
            min_version: None,
            max_version: None,
            locales: None,
            timezone: None,
            geolocation: None,
            screen: None,
            webgl_override: None,
            webgl_params: None,
            webgl_extensions: None,
            canvas_seed: None,
            audio_seed: None,
            storage_quota_gb: None,
            outer_chrome_height: None,
            history_min_length: None,
            webgpu_enabled: true,
            hardware_concurrency: None,
            device_memory: None,
            max_touch_points: None,
            do_not_track: None,
            pdf_viewer_enabled: None,
            canvas_noise_amplitude: None,
            audio_noise_amplitude: None,
            audio_sample_rate: None,
            audio_compressor: None,
            dom_rect_jitter: None,
            font_metrics_jitter: None,
            fonts: None,
            battery: None,
            speech_voices: None,
            color_scheme: None,
            reduced_motion: None,
            color_gamut: None,
            hdr_mode: None,
            performance_jitter_ms: None,
            worker_spoofing: true,
            webrtc_policy: None,
            connection: None,
            media_devices: None,
            seed: None,
        }
    }
}

impl FingerprintOptions {
    /// Starts a [`FingerprintOptionsBuilder`].
    pub fn builder() -> FingerprintOptionsBuilder {
        FingerprintOptionsBuilder::new()
    }

    /// Preset for a standard Windows desktop Chrome persona.
    #[must_use]
    pub fn windows_chrome() -> Self {
        Self {
            os: Some(OsType::Windows),
            browser: Some(BrowserType::Chromium),
            color_gamut: Some(ColorGamut::Srgb),
            ..Self::default()
        }
    }

    /// Preset for a standard macOS desktop Chrome persona with P3 display.
    #[must_use]
    pub fn macos_chrome() -> Self {
        Self {
            os: Some(OsType::Macos),
            browser: Some(BrowserType::Chromium),
            color_gamut: Some(ColorGamut::P3),
            hdr_mode: Some(HdrMode::High),
            ..Self::default()
        }
    }

    /// Preset for a standard Linux desktop Chrome persona.
    #[must_use]
    pub fn linux_chrome() -> Self {
        Self {
            os: Some(OsType::Linux),
            browser: Some(BrowserType::Chromium),
            color_gamut: Some(ColorGamut::Srgb),
            ..Self::default()
        }
    }

    /// Preset for a mobile Android Chrome persona.
    #[must_use]
    pub fn android_chrome() -> Self {
        Self {
            os: Some(OsType::Android),
            browser: Some(BrowserType::Chromium),
            max_touch_points: Some(5),
            outer_chrome_height: Some(0),
            ..Self::default()
        }
    }

    /// Preset for a mobile iOS Safari persona.
    #[must_use]
    pub fn ios_safari() -> Self {
        Self {
            os: Some(OsType::Ios),
            browser: Some(BrowserType::MobileSafari),
            max_touch_points: Some(5),
            color_gamut: Some(ColorGamut::P3),
            outer_chrome_height: Some(0),
            ..Self::default()
        }
    }
}

/// Builder for [`FingerprintOptions`].
#[derive(Clone, Debug, Default)]
#[must_use = "FingerprintOptionsBuilder does nothing until build() is called"]
pub struct FingerprintOptionsBuilder {
    inner: FingerprintOptions,
}

impl FingerprintOptionsBuilder {
    /// Creates a new builder with default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the operating system persona.
    pub fn os(mut self, os: OsType) -> Self {
        self.inner.os = Some(os);
        self
    }

    /// Sets the browser family persona.
    pub fn browser(mut self, browser: BrowserType) -> Self {
        self.inner.browser = Some(browser);
        self
    }

    /// Sets an explicit `User-Agent` string override.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.inner.user_agent = Some(ua.into());
        self
    }

    /// Sets an explicit `navigator.platform` override.
    pub fn platform(mut self, platform: impl Into<String>) -> Self {
        self.inner.platform = Some(platform.into());
        self
    }

    /// Sets an explicit screen `(width, height)` shorthand override.
    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.inner.window_size = Some((width, height));
        self
    }

    /// Adds a custom HTTP header merged into [`GeneratedFingerprint::headers`].
    pub fn custom_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.custom_headers.insert(name.into(), value.into());
        self
    }

    /// Sets the minimum Chromium major version.
    pub fn min_version(mut self, version: u32) -> Self {
        self.inner.min_version = Some(version);
        self
    }

    /// Sets the maximum Chromium major version.
    pub fn max_version(mut self, version: u32) -> Self {
        self.inner.max_version = Some(version);
        self
    }

    /// Sets the ordered `navigator.languages` list.
    pub fn locales(mut self, locales: Vec<String>) -> Self {
        self.inner.locales = Some(locales);
        self
    }

    /// Sets the IANA timezone identifier.
    pub fn timezone(mut self, timezone: impl Into<String>) -> Self {
        self.inner.timezone = Some(timezone.into());
        self
    }

    /// Sets the geolocation coordinates.
    pub fn geolocation(mut self, geolocation: Geolocation) -> Self {
        self.inner.geolocation = Some(geolocation);
        self
    }

    /// Sets the screen geometry and display constraints.
    pub fn screen(mut self, screen: ScreenConstraints) -> Self {
        self.inner.screen = Some(screen);
        self
    }

    /// Sets explicit WebGL `(vendor, renderer)` strings.
    pub fn webgl_override(
        mut self,
        vendor: impl Into<String>,
        renderer: impl Into<String>,
    ) -> Self {
        self.inner.webgl_override = Some((vendor.into(), renderer.into()));
        self
    }

    /// Enables or disables WebGPU spoofing.
    pub fn webgpu_enabled(mut self, enabled: bool) -> Self {
        self.inner.webgpu_enabled = enabled;
        self
    }

    /// Sets `navigator.hardwareConcurrency`.
    pub fn hardware_concurrency(mut self, cores: u32) -> Self {
        self.inner.hardware_concurrency = Some(cores);
        self
    }

    /// Sets `navigator.deviceMemory` in GiB.
    pub fn device_memory(mut self, memory_gb: u32) -> Self {
        self.inner.device_memory = Some(memory_gb);
        self
    }

    /// Sets the canvas noise amplitude.
    pub fn canvas_noise_amplitude(mut self, amplitude: f64) -> Self {
        self.inner.canvas_noise_amplitude = Some(amplitude);
        self
    }

    /// Sets the audio noise amplitude.
    pub fn audio_noise_amplitude(mut self, amplitude: f64) -> Self {
        self.inner.audio_noise_amplitude = Some(amplitude);
        self
    }

    /// Sets the Web Audio sample rate in Hz.
    pub fn audio_sample_rate(mut self, rate: u32) -> Self {
        self.inner.audio_sample_rate = Some(rate);
        self
    }

    /// Sets the `DOMRect` jitter amplitude.
    pub fn dom_rect_jitter(mut self, jitter: f64) -> Self {
        self.inner.dom_rect_jitter = Some(jitter);
        self
    }

    /// Sets the font metrics jitter amplitude.
    pub fn font_metrics_jitter(mut self, jitter: f64) -> Self {
        self.inner.font_metrics_jitter = Some(jitter);
        self
    }

    /// Sets the WebRTC IP handling policy.
    pub fn webrtc_policy(mut self, policy: WebRtcPolicy) -> Self {
        self.inner.webrtc_policy = Some(policy);
        self
    }

    /// Sets the `NetworkInformation` connection override.
    pub fn connection(mut self, connection: ConnectionOverride) -> Self {
        self.inner.connection = Some(connection);
        self
    }

    /// Sets synthetic media device counts `(cameras, microphones, speakers)`.
    pub fn media_devices(mut self, cameras: u32, microphones: u32, speakers: u32) -> Self {
        self.inner.media_devices = Some((cameras, microphones, speakers));
        self
    }

    /// Sets the deterministic generation seed.
    pub fn seed(mut self, seed: u64) -> Self {
        self.inner.seed = Some(seed);
        self
    }

    /// Sets explicit installed font names.
    pub fn fonts(mut self, fonts: Vec<String>) -> Self {
        self.inner.fonts = Some(fonts);
        self
    }

    /// Sets explicit `navigator.getBattery()` parameters.
    pub fn battery(mut self, battery: BatteryOverride) -> Self {
        self.inner.battery = Some(battery);
        self
    }

    /// Sets explicit `window.speechSynthesis.getVoices()` entries.
    pub fn speech_voices(mut self, voices: Vec<SpeechVoice>) -> Self {
        self.inner.speech_voices = Some(voices);
        self
    }

    /// Sets `navigator.maxTouchPoints`.
    pub fn max_touch_points(mut self, points: u32) -> Self {
        self.inner.max_touch_points = Some(points);
        self
    }

    /// Sets `navigator.doNotTrack` (`"1"`, `"0"`, or `"unspecified"`).
    pub fn do_not_track(mut self, value: impl Into<String>) -> Self {
        self.inner.do_not_track = Some(value.into());
        self
    }

    /// Sets `navigator.pdfViewerEnabled`.
    pub fn pdf_viewer_enabled(mut self, enabled: bool) -> Self {
        self.inner.pdf_viewer_enabled = Some(enabled);
        self
    }

    /// Sets CSS `prefers-color-scheme`.
    pub fn color_scheme(mut self, scheme: ColorScheme) -> Self {
        self.inner.color_scheme = Some(scheme);
        self
    }

    /// Sets CSS `prefers-reduced-motion`.
    pub fn reduced_motion(mut self, motion: ReducedMotion) -> Self {
        self.inner.reduced_motion = Some(motion);
        self
    }

    /// Sets CSS `color-gamut`.
    pub fn color_gamut(mut self, gamut: ColorGamut) -> Self {
        self.inner.color_gamut = Some(gamut);
        self
    }

    /// Sets CSS `dynamic-range` HDR mode.
    pub fn hdr_mode(mut self, hdr: HdrMode) -> Self {
        self.inner.hdr_mode = Some(hdr);
        self
    }

    /// Sets deterministic sub-millisecond `performance.now()` jitter amplitude in ms.
    pub fn performance_jitter_ms(mut self, jitter_ms: f64) -> Self {
        self.inner.performance_jitter_ms = Some(jitter_ms);
        self
    }

    /// Enables or disables `Worker` / `SharedWorker` navigator consistency spoofing.
    pub fn worker_spoofing(mut self, enabled: bool) -> Self {
        self.inner.worker_spoofing = enabled;
        self
    }

    /// Sets explicit WebGL 1/2 parameter limits.
    pub fn webgl_params(mut self, params: WebGlParameters) -> Self {
        self.inner.webgl_params = Some(params);
        self
    }

    /// Sets explicit Web Audio `DynamicsCompressorNode` parameters.
    pub fn audio_compressor(mut self, compressor: AudioDynamicsCompressor) -> Self {
        self.inner.audio_compressor = Some(compressor);
        self
    }

    /// Sets explicit WebGL supported extension names.
    pub fn webgl_extensions(mut self, extensions: Vec<String>) -> Self {
        self.inner.webgl_extensions = Some(extensions);
        self
    }

    /// Sets an independent deterministic seed for canvas noise.
    pub fn canvas_seed(mut self, seed: u64) -> Self {
        self.inner.canvas_seed = Some(seed);
        self
    }

    /// Sets an independent deterministic seed for Web Audio noise.
    pub fn audio_seed(mut self, seed: u64) -> Self {
        self.inner.audio_seed = Some(seed);
        self
    }

    /// Sets `StorageManager.estimate().quota` in GiB.
    pub fn storage_quota_gb(mut self, gb: u64) -> Self {
        self.inner.storage_quota_gb = Some(gb);
        self
    }

    /// Sets the browser toolbar chrome height offset in CSS pixels.
    pub fn outer_chrome_height(mut self, px: u32) -> Self {
        self.inner.outer_chrome_height = Some(px);
        self
    }

    /// Sets the minimum `window.history.length` reported on fresh tabs.
    pub fn history_min_length(mut self, min_len: u32) -> Self {
        self.inner.history_min_length = Some(min_len);
        self
    }

    /// Validates constraints and builds [`FingerprintOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`FingerprintError`] if `min_version > max_version`, `screen.min_width > screen.max_width`,
    /// `battery.level` is outside `0.0..=1.0`, or `performance_jitter_ms` is negative/non-finite.
    pub fn build(self) -> Result<FingerprintOptions, FingerprintError> {
        if let (Some(min_v), Some(max_v)) = (self.inner.min_version, self.inner.max_version)
            && min_v > max_v
        {
            return Err(FingerprintError::Incoherent {
                setting: "min_version",
                reason: "min_version cannot exceed max_version".to_owned(),
            });
        }
        if let Some(screen) = self.inner.screen
            && let (Some(min_w), Some(max_w)) = (screen.min_width, screen.max_width)
            && min_w > max_w
        {
            return Err(FingerprintError::Incoherent {
                setting: "screen",
                reason: "screen min_width cannot exceed max_width".to_owned(),
            });
        }
        if let Some(bat) = self.inner.battery
            && (!bat.level.is_finite() || !(0.0..=1.0).contains(&bat.level))
        {
            return Err(FingerprintError::Incoherent {
                setting: "battery",
                reason: "battery level must be finite and within 0.0..=1.0".to_owned(),
            });
        }
        if let Some(jitter) = self.inner.performance_jitter_ms
            && (!jitter.is_finite() || jitter < 0.0)
        {
            return Err(FingerprintError::Incoherent {
                setting: "performance_jitter_ms",
                reason: "performance_jitter_ms must be finite and non-negative".to_owned(),
            });
        }
        Ok(self.inner)
    }
}

impl FingerprintGenerator {
    /// Generates a coherent [`GeneratedFingerprint`] from rich [`FingerprintOptions`].
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "applies all FingerprintOptions overrides coherently in one pass"
    )]
    pub fn generate_with(options: &FingerprintOptions) -> GeneratedFingerprint {
        let seed = options.seed.unwrap_or(0x5EED_C0DE_0000_0001);
        let os = options.os.unwrap_or(OsType::Windows);
        let min_v = options.min_version.unwrap_or(PRESET_CHROMIUM_MAJOR);
        let max_v = options
            .max_version
            .unwrap_or(min_v.max(PRESET_CHROMIUM_MAJOR));
        let major = if min_v <= max_v {
            let span = u64::from(max_v - min_v) + 1;
            min_v + u32::try_from(seed % span).unwrap_or(0)
        } else {
            min_v
        };

        let mut generator = Self::new(seed).os(os).major(major);
        if let Some(rate) = options.audio_sample_rate {
            generator = generator.audio_sample_rate(rate);
        }
        if let Some(policy) = options.webrtc_policy {
            generator = generator.webrtc_policy(policy);
        }
        if let Some(fonts) = &options.fonts {
            generator = generator.fonts(fonts.clone());
        }
        if let Some(gamut) = options.color_gamut {
            generator = generator.color_gamut(gamut);
        }
        if let Some(hdr) = options
            .hdr_mode
            .or_else(|| options.screen.and_then(|s| s.hdr_mode))
        {
            generator = generator.hdr(hdr == HdrMode::High);
        }

        let mut base_fp = generator.build_core_fingerprint();
        let mut builder = base_fp.to_builder();
        if let Some(browser) = options.browser {
            builder = builder.browser_type(browser);
        }
        if let Some(ua) = &options.user_agent {
            builder = builder.user_agent(ua.as_str());
        }
        if let Some(platform) = &options.platform {
            builder = builder.platform(platform.as_str());
        }
        if let Some((w, h)) = options.window_size {
            builder = builder.screen(w, h);
            let lost = if os.is_mobile() { 0 } else { 40 };
            builder = builder.avail_screen((w, h.saturating_sub(lost)));
        }
        if let Some(locales) = &options.locales
            && !locales.is_empty()
        {
            builder = builder.languages(locales.join(",").as_str());
        }
        if let Some(tz) = &options.timezone {
            builder = builder.timezone(tz.as_str());
        }
        if let Some(geo) = options.geolocation {
            builder = builder.geolocation(geo);
        }
        if let Some(cores) = options.hardware_concurrency {
            builder = builder.hardware_concurrency(cores);
        }
        if let Some(mem) = options.device_memory {
            builder = builder.device_memory(f64::from(mem));
        }
        if let Some((vendor, renderer)) = &options.webgl_override {
            builder = builder.webgl(WebGl::new(vendor.as_str(), renderer.as_str()));
        }
        if let Some(screen) = options.screen {
            if let Some(dpr) = screen.device_pixel_ratio {
                builder = builder.pixel_ratio(dpr);
            }
            if let Some(depth) = screen.color_depth {
                builder = builder.color_depth(depth);
            }
            if screen.min_width.is_some() || screen.max_width.is_some() {
                let current_w = base_fp.screen().value().map_or(1920, |s| s.width);
                let current_h = base_fp.screen().value().map_or(1080, |s| s.height);
                let min_w = screen.min_width.unwrap_or(0);
                let max_w = screen.max_width.unwrap_or(u32::MAX);
                let clamped_w = current_w.clamp(min_w, max_w);
                if clamped_w != current_w {
                    let scaled_h = (u64::from(current_h) * u64::from(clamped_w)
                        / u64::from(current_w.max(1)))
                    .max(200);
                    let h = u32::try_from(scaled_h).unwrap_or(current_h);
                    builder = builder.screen(clamped_w, h);
                    let lost = if os.is_mobile() { 0 } else { 40 };
                    builder = builder.avail_screen((clamped_w, h.saturating_sub(lost)));
                }
            }
        }
        if let Some(conn) = &options.connection {
            builder = builder.connection(ConnectionProfile {
                effective_type: conn.effective_type.clone(),
                rtt: conn.rtt_ms,
                downlink: conn.downlink_mbps,
                save_data: conn.save_data,
            });
        }
        base_fp = builder.build().unwrap_or(base_fp);

        let (gpu_vendor, gpu_renderer, mut gpu_extensions, mut webgl_params) =
            Self::build_gpu(&base_fp);
        if let Some(ref exts) = options.webgl_extensions {
            gpu_extensions.clone_from(exts);
        }
        if let Some(ref params_override) = options.webgl_params {
            webgl_params = params_override.clone();
        }
        let webgpu = WebGpuMetadata::for_gpu(os, &gpu_vendor, &gpu_renderer);
        let display = Self::build_display(&base_fp);
        let mut audio = generator.build_audio(&base_fp);
        if let Some(comp) = options.audio_compressor {
            audio.compressor = comp;
        }
        let media_devices = if let Some((cams, mics, spks)) = options.media_devices {
            let mut devices = Vec::with_capacity((cams + mics + spks) as usize);
            let mut push_kind = |kind: &str, count: u32, prefix: &str| {
                for i in 0..count {
                    devices.push(MediaDeviceInfo {
                        device_id: format!("{seed:016x}-{prefix}-{i}"),
                        kind: kind.to_owned(),
                        label: format!("{prefix} {}", i + 1),
                        group_id: format!("{seed:016x}-grp-{prefix}"),
                    });
                }
            };
            push_kind("videoinput", cams, "Camera");
            push_kind("audioinput", mics, "Microphone");
            push_kind("audiooutput", spks, "Speaker");
            MediaDeviceMetadata { devices }
        } else {
            MediaDeviceMetadata::synthetic(seed, os)
        };
        let battery = if let Some(bat) = options.battery {
            BatteryMetadata {
                charging: bat.charging,
                charging_time: bat.charging_time,
                discharging_time: bat.discharging_time,
                level: bat.level,
            }
        } else {
            Self::build_battery(&base_fp)
        };
        let network = Self::build_network(&base_fp);
        let fonts = options
            .fonts
            .clone()
            .unwrap_or_else(|| generator.build_fonts(&base_fp));

        let mut profile = ProfileFingerprint::from_fingerprint(&base_fp);
        profile.webgl_extensions.clone_from(&gpu_extensions);
        profile.fonts.clone_from(&fonts);
        profile.battery = Some(battery);
        profile.canvas_noise_amplitude = options.canvas_noise_amplitude;
        profile.audio_noise_amplitude = options.audio_noise_amplitude;
        profile.dom_rect_jitter = options.dom_rect_jitter;
        profile.font_metrics_jitter = options.font_metrics_jitter;
        profile.webgpu_enabled = options.webgpu_enabled;
        if let Some(points) = options.max_touch_points {
            profile.max_touch_points = Some(points);
        }
        if let Some(dnt) = &options.do_not_track {
            profile.do_not_track = Some(dnt.clone());
        }
        if let Some(pdf) = options.pdf_viewer_enabled {
            profile.pdf_viewer_enabled = Some(pdf);
        }
        if let Some(scheme) = options.color_scheme {
            profile.color_scheme = Some(scheme);
        }
        if let Some(motion) = options.reduced_motion {
            profile.reduced_motion = Some(motion);
        }
        if let Some(gamut) = options.color_gamut {
            profile.color_gamut = Some(gamut);
        }
        if let Some(hdr) = options.hdr_mode {
            profile.hdr = Some(hdr == HdrMode::High);
        }
        if let Some(voices) = &options.speech_voices {
            profile.speech_voices.clone_from(voices);
        }
        profile.performance_jitter_ms = options.performance_jitter_ms;
        profile.worker_spoofing = options.worker_spoofing;
        profile.canvas_seed = options.canvas_seed;
        profile.audio_seed = options.audio_seed;
        profile.storage_quota_gb = options.storage_quota_gb;
        profile.outer_chrome_height = options.outer_chrome_height;
        profile.history_min_length = options.history_min_length;

        let user_agent = profile.user_agent.clone().unwrap_or_default();
        let client_hints = profile
            .client_hints
            .clone()
            .unwrap_or_else(|| ClientHintsHighEntropy::for_safari(os));
        let mut headers = Self::generate_headers(
            &base_fp,
            &user_agent,
            profile.client_hints.as_ref(),
            &profile.languages,
        );
        for (k, v) in &options.custom_headers {
            headers.insert(k.clone(), v.clone());
        }

        GeneratedFingerprint {
            profile,
            fingerprint: base_fp,
            headers,
            client_hints,
            audio,
            gpu: GpuMetadata {
                vendor: gpu_vendor,
                renderer: gpu_renderer,
                extensions: gpu_extensions,
                parameters: webgl_params,
            },
            webgpu,
            display,
            media_devices,
            battery,
            network,
            fonts,
        }
    }
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

    /// Generates a coherent [`GeneratedFingerprint`] bundle with full parameters.
    #[must_use]
    pub fn generate(&self) -> GeneratedFingerprint {
        let fingerprint = self.build_core_fingerprint();
        let os = fingerprint.os_type();

        let (gpu_vendor, gpu_renderer, gpu_extensions, webgl_params) =
            Self::build_gpu(&fingerprint);
        let webgpu = WebGpuMetadata::for_gpu(os, &gpu_vendor, &gpu_renderer);
        let display = Self::build_display(&fingerprint);
        let audio = self.build_audio(&fingerprint);
        let media_devices = MediaDeviceMetadata::synthetic(self.seed, os);
        let battery = Self::build_battery(&fingerprint);
        let network = Self::build_network(&fingerprint);
        let fonts = self.build_fonts(&fingerprint);

        let mut profile = ProfileFingerprint::from_fingerprint(&fingerprint);
        profile.webgl_extensions.clone_from(&gpu_extensions);
        if profile.fonts.is_empty() {
            profile.fonts.clone_from(&fonts);
        }

        let user_agent = profile.user_agent.clone().unwrap_or_default();
        let client_hints = profile
            .client_hints
            .clone()
            .unwrap_or_else(|| ClientHintsHighEntropy::for_safari(os));
        let headers = Self::generate_headers(
            &fingerprint,
            &user_agent,
            profile.client_hints.as_ref(),
            &profile.languages,
        );

        GeneratedFingerprint {
            profile,
            fingerprint,
            headers,
            client_hints,
            audio,
            gpu: GpuMetadata {
                vendor: gpu_vendor,
                renderer: gpu_renderer,
                extensions: gpu_extensions,
                parameters: webgl_params,
            },
            webgpu,
            display,
            media_devices,
            battery,
            network,
            fonts,
        }
    }

    fn build_core_fingerprint(&self) -> Fingerprint {
        let base_fp = Fingerprint::randomized_with_major(self.os, self.seed, self.major);
        let default_gamut = if matches!(self.os, OsType::Macos | OsType::Ios) {
            ColorGamut::P3
        } else {
            ColorGamut::Srgb
        };
        let base_mf = base_fp
            .media_features()
            .value()
            .copied()
            .unwrap_or_else(|| MediaFeatures::new(default_gamut));
        let gamut = self.color_gamut.unwrap_or(base_mf.color_gamut);
        let hdr_enabled = self
            .hdr
            .unwrap_or(matches!(self.os, OsType::Macos | OsType::Ios));
        let mut updated_mf = base_mf;
        updated_mf.color_gamut = gamut;
        updated_mf.dynamic_range = if hdr_enabled {
            DynamicRange::High
        } else {
            DynamicRange::Standard
        };

        let mut builder = base_fp
            .to_builder()
            .canvas_noise(self.canvas_noise)
            .audio_noise(self.audio_noise)
            .webrtc_policy(self.webrtc_policy)
            .media_features(updated_mf);

        if base_fp.avail_screen().is_natural()
            && let Some(screen) = base_fp.screen().value()
        {
            let lost = if self.os.is_mobile() { 0 } else { 40 };
            builder = builder.avail_screen((screen.width, screen.height.saturating_sub(lost)));
        }
        if let Some(rate) = self.audio_sample_rate {
            builder = builder.audio_sample_rate(rate);
        }
        if let Some(fonts) = &self.fonts {
            builder = builder.fonts(fonts.clone());
        }

        builder.build().unwrap_or(base_fp)
    }

    fn build_gpu(fp: &Fingerprint) -> (String, String, Vec<String>, WebGlParameters) {
        let os = fp.os_type();
        let webgl = fp.webgl().value();
        let (fallback_vendor, fallback_renderer) = match os {
            OsType::Macos => (
                "Google Inc. (Apple)",
                "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)",
            ),
            OsType::Linux => (
                "Google Inc. (Intel)",
                "ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6)",
            ),
            OsType::Android => ("Qualcomm", "Adreno (TM) 740"),
            OsType::Ios => ("Apple Inc.", "Apple GPU"),
            _ => (
                "Google Inc. (NVIDIA)",
                "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)",
            ),
        };

        let vendor = webgl
            .and_then(|gl| gl.vendor.clone())
            .unwrap_or_else(|| fallback_vendor.to_owned());
        let renderer = webgl
            .and_then(|gl| gl.renderer.clone())
            .unwrap_or_else(|| fallback_renderer.to_owned());

        let is_mobile = os.is_mobile()
            || renderer.contains("Adreno")
            || renderer.contains("Mali")
            || renderer.contains("PowerVR");
        let extensions =
            if let Some(exts) = webgl.map(|gl| &gl.extensions).filter(|e| !e.is_empty()) {
                exts.clone()
            } else if is_mobile {
                MOBILE_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect()
            } else if os == OsType::Macos {
                APPLE_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect()
            } else {
                COMMON_WEBGL_EXTENSIONS
                    .iter()
                    .map(|&s| s.to_owned())
                    .collect()
            };

        let max_tex = webgl.and_then(|gl| gl.max_texture_size);
        let params = WebGlParameters::for_gpu_with_max_texture(os, &vendor, &renderer, max_tex);
        (vendor, renderer, extensions, params)
    }

    fn build_display(fp: &Fingerprint) -> DisplayMetadata {
        let os = fp.os_type();
        let (width, height) = fp
            .screen()
            .value()
            .map_or_else(|| os.default_screen(), |s| (s.width, s.height));
        let (avail_width, avail_height) = fp.avail_screen().value().map_or_else(
            || {
                let lost = if os.is_mobile() { 0 } else { 40 };
                (width, height.saturating_sub(lost))
            },
            |a| (a.width, a.height),
        );
        let pixel_ratio = fp
            .pixel_ratio()
            .value()
            .copied()
            .unwrap_or(if os.is_mobile() { 3.0 } else { 1.0 });
        let color_depth = fp.color_depth().value().copied().unwrap_or(24);
        let mf = fp.media_features().value();
        let color_gamut = mf.map_or(
            if matches!(os, OsType::Macos | OsType::Ios) {
                ColorGamut::P3
            } else {
                ColorGamut::Srgb
            },
            |m| m.color_gamut,
        );
        let hdr = if mf.is_some_and(|m| matches!(m.dynamic_range, DynamicRange::High)) {
            HdrMode::High
        } else {
            HdrMode::Standard
        };
        let orientation_type = if height > width || os.is_mobile() {
            ScreenOrientationType::PortraitPrimary
        } else {
            ScreenOrientationType::LandscapePrimary
        };

        DisplayMetadata {
            width,
            height,
            avail_width,
            avail_height,
            pixel_ratio,
            color_depth,
            color_gamut,
            hdr,
            orientation: ScreenOrientation {
                orientation_type,
                angle: 0,
            },
        }
    }

    fn build_audio(&self, fp: &Fingerprint) -> AudioMetadata {
        let rate = self
            .audio_sample_rate
            .or_else(|| fp.audio_sample_rate())
            .unwrap_or(if matches!(fp.os_type(), OsType::Linux | OsType::Android) {
                44100
            } else {
                48000
            });

        AudioMetadata {
            sample_rate: rate,
            max_channel_count: 2,
            output_latency: 0.012,
            base_latency: 0.005,
            compressor: AudioDynamicsCompressor::default(),
        }
    }

    fn build_battery(fp: &Fingerprint) -> BatteryMetadata {
        if let Some(b) = fp.battery().value() {
            BatteryMetadata {
                charging: b.charging,
                charging_time: b.charging_time,
                discharging_time: b.discharging_time,
                level: b.level,
            }
        } else {
            BatteryMetadata::default()
        }
    }

    fn build_network(fp: &Fingerprint) -> NetworkMetadata {
        if let Some(c) = fp.connection().value() {
            NetworkMetadata {
                downlink: c.downlink,
                effective_type: c.effective_type.clone(),
                rtt: c.rtt,
                save_data: c.save_data,
            }
        } else {
            NetworkMetadata::default()
        }
    }

    fn build_fonts(&self, fp: &Fingerprint) -> Vec<String> {
        if let Some(custom) = &self.fonts {
            return custom.clone();
        }
        if let Some(fp_fonts) = fp.fonts().value().filter(|f| !f.is_empty()) {
            return fp_fonts.clone();
        }
        match fp.os_type() {
            OsType::Windows => WINDOWS_FONTS.iter().map(|&f| f.to_owned()).collect(),
            OsType::Macos | OsType::Ios => MACOS_FONTS.iter().map(|&f| f.to_owned()).collect(),
            OsType::Android => ANDROID_FONTS.iter().map(|&f| f.to_owned()).collect(),
            _ => LINUX_FONTS.iter().map(|&f| f.to_owned()).collect(),
        }
    }

    fn format_brand_list_header(brands: &[BrandVersion]) -> String {
        brands
            .iter()
            .map(|entry| {
                let escaped = entry.brand.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{escaped}\";v=\"{}\"", entry.version)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn generate_headers(
        fp: &Fingerprint,
        user_agent: &str,
        client_hints: Option<&ClientHintsHighEntropy>,
        languages: &[String],
    ) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        headers.insert("User-Agent".to_owned(), user_agent.to_owned());

        let accept_lang = fp
            .accept_languages()
            .as_str()
            .map(ToOwned::to_owned)
            .or_else(|| Locale::accept_language_list(languages))
            .unwrap_or_else(|| "en-US,en;q=0.9".to_owned());
        headers.insert("Accept-Language".to_owned(), accept_lang);

        if let Some(ch) = client_hints {
            if !ch.brands.is_empty() {
                headers.insert(
                    "Sec-CH-UA".to_owned(),
                    Self::format_brand_list_header(&ch.brands),
                );
            }
            headers.insert(
                "Sec-CH-UA-Mobile".to_owned(),
                if ch.mobile {
                    "?1".to_owned()
                } else {
                    "?0".to_owned()
                },
            );
            headers.insert(
                "Sec-CH-UA-Platform".to_owned(),
                format!("\"{}\"", ch.platform),
            );
            headers.insert(
                "Sec-CH-UA-Platform-Version".to_owned(),
                format!("\"{}\"", ch.platform_version),
            );
            headers.insert(
                "Sec-CH-UA-Arch".to_owned(),
                format!("\"{}\"", ch.architecture),
            );
            headers.insert(
                "Sec-CH-UA-Bitness".to_owned(),
                format!("\"{}\"", ch.bitness),
            );
            headers.insert("Sec-CH-UA-Model".to_owned(), format!("\"{}\"", ch.model));
            if !ch.full_version_list.is_empty() {
                headers.insert(
                    "Sec-CH-UA-Full-Version-List".to_owned(),
                    Self::format_brand_list_header(&ch.full_version_list),
                );
            }
        }
        headers
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
    /// High-entropy Client Hints.
    pub client_hints: ClientHintsHighEntropy,
    /// Web Audio characteristics.
    pub audio: AudioMetadata,
    /// WebGL and GPU characteristics.
    pub gpu: GpuMetadata,
    /// WebGPU adapter metadata and limits.
    pub webgpu: WebGpuMetadata,
    /// Screen and display characteristics.
    pub display: DisplayMetadata,
    /// Synthetic audio and video media devices.
    pub media_devices: MediaDeviceMetadata,
    /// Battery metadata.
    pub battery: BatteryMetadata,
    /// Network connection metadata.
    pub network: NetworkMetadata,
    /// Enumerated font list.
    pub fonts: Vec<String>,
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
    fn generator_uses_correlated_core_fingerprint_fields_without_hardcoded_gpu_or_display() {
        let mut windows_renderers = std::collections::HashSet::new();
        let mut mac_renderers = std::collections::HashSet::new();
        for seed in 1..20 {
            let win_bundle = FingerprintGenerator::new(seed)
                .os(OsType::Windows)
                .major(157)
                .generate();
            assert_eq!(win_bundle.profile.os, Some(OsType::Windows));
            assert_eq!(
                win_bundle.profile.webgl_renderer.as_deref(),
                win_bundle.fingerprint.webgl_renderer()
            );
            assert_eq!(
                win_bundle.gpu.renderer,
                win_bundle.fingerprint.webgl_renderer().unwrap()
            );
            assert_eq!(
                win_bundle.profile.hardware_concurrency,
                win_bundle
                    .fingerprint
                    .hardware_concurrency()
                    .value()
                    .copied()
            );
            assert_eq!(
                win_bundle.profile.device_memory,
                win_bundle.fingerprint.device_memory().value().copied()
            );
            assert_eq!(
                win_bundle.profile.timezone.as_deref(),
                win_bundle.fingerprint.timezone().as_str()
            );
            let screen = win_bundle.fingerprint.screen().value().unwrap();
            let avail = win_bundle.fingerprint.avail_screen().value().unwrap();
            assert_eq!(win_bundle.display.width, screen.width);
            assert_eq!(win_bundle.display.height, screen.height);
            assert_eq!(win_bundle.display.avail_width, avail.width);
            assert_eq!(win_bundle.display.avail_height, avail.height);
            assert!(
                win_bundle
                    .profile
                    .languages
                    .iter()
                    .all(|l| !l.contains(";q="))
            );
            assert!(win_bundle.fingerprint.validate().is_coherent());
            windows_renderers.insert(win_bundle.gpu.renderer);

            let mac_bundle = FingerprintGenerator::new(seed)
                .os(OsType::Macos)
                .major(156)
                .generate();
            assert_eq!(mac_bundle.profile.os, Some(OsType::Macos));
            assert_eq!(mac_bundle.display.color_gamut, ColorGamut::P3);
            assert_eq!(mac_bundle.webgpu.vendor, "apple");
            assert_eq!(mac_bundle.webgpu.architecture, "metal-3");
            assert_eq!(
                mac_bundle.gpu.renderer,
                mac_bundle.fingerprint.webgl_renderer().unwrap()
            );
            assert!(mac_bundle.fonts.contains(&"Helvetica Neue".to_string()));
            mac_renderers.insert(mac_bundle.gpu.renderer);
        }
        assert!(windows_renderers.len() > 1, "GPU must vary by seed");
        assert!(mac_renderers.len() > 1, "Mac GPU must vary by seed");
    }

    #[test]
    fn generator_creates_genuine_grease_and_known_chromium_builds() {
        for (major, expected_full, expected_grease) in [
            (155, "155.0.8059.40", "Not(A:Brand"),
            (156, "156.0.8078.35", "Not:A-Brand"),
            (157, "157.0.8103.39", "Not-A.Brand"),
        ] {
            let bundle = FingerprintGenerator::new(101)
                .os(OsType::Windows)
                .major(major)
                .generate();
            assert_eq!(
                known_full_version(major).map(|k| k.full_version),
                Some(expected_full)
            );
            assert_eq!(bundle.client_hints.ua_full_version, expected_full);
            assert!(
                bundle
                    .client_hints
                    .brands
                    .iter()
                    .any(|b| b.brand == expected_grease),
                "expected GREASE brand {expected_grease} in {:?}",
                bundle.client_hints.brands
            );
            assert!(
                bundle
                    .client_hints
                    .full_version_list
                    .iter()
                    .any(|b| b.brand == "Google Chrome" && b.version == expected_full)
            );
            let sec_ch_ua = bundle.headers.get("Sec-CH-UA").expect("Sec-CH-UA");
            assert!(sec_ch_ua.contains(expected_grease), "{sec_ch_ua}");
            let full_list = bundle
                .headers
                .get("Sec-CH-UA-Full-Version-List")
                .expect("Sec-CH-UA-Full-Version-List");
            assert!(full_list.contains(expected_full), "{full_list}");
            assert!(
                !full_list.contains("7163"),
                "no fake 7163 build: {full_list}"
            );
        }

        let edge = edge_hints(157);
        assert!(edge.brands.iter().any(|b| b.brand == "Microsoft Edge"));
        assert!(!edge.brands.iter().any(|b| b.brand == "Google Chrome"));
    }

    #[test]
    fn from_fingerprint_reads_actual_client_hints_version_avail_screen_and_media_features() {
        let fp = Fingerprint::builder()
            .os_type(OsType::Macos)
            .core_version(155)
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36")
            .screen(1512, 982)
            .avail_screen((1512, 940))
            .audio_sample_rate(44100)
            .media_features(MediaFeatures {
                color_gamut: ColorGamut::P3,
                dynamic_range: DynamicRange::High,
                ..MediaFeatures::new(ColorGamut::P3)
            })
            .build()
            .expect("valid fingerprint");

        let profile = ProfileFingerprint::from_fingerprint(&fp);
        assert_eq!(profile.avail_width, Some(1512));
        assert_eq!(profile.avail_height, Some(940));
        assert_eq!(profile.color_gamut, Some(ColorGamut::P3));
        assert_eq!(profile.hdr, Some(true));
        assert_eq!(profile.audio_sample_rate, Some(44100));
        let ch = profile
            .client_hints
            .as_ref()
            .expect("Chromium client hints");
        assert_eq!(ch.ua_full_version, "155.0.8059.40");
        assert!(ch.brands.iter().any(|b| b.brand == "Not(A:Brand"));

        let ios_fp = Fingerprint::randomized(OsType::Ios, 7);
        let ios_profile = ProfileFingerprint::from_fingerprint(&ios_fp);
        assert_eq!(ios_profile.browser, Some(BrowserType::MobileSafari));
        assert!(ios_profile.client_hints.is_none());
        assert!(ios_profile.battery.is_none());
        assert!(ios_profile.network.is_none());
        assert!(ios_profile.device_memory.is_none());
    }

    #[test]
    fn generator_creates_webgl_parameters_and_webgpu_limits() {
        let generator = FingerprintGenerator::new(101)
            .os(OsType::Windows)
            .major(157);
        let bundle = generator.generate();

        assert_eq!(bundle.gpu.parameters.max_texture_size, 16384);
        assert_eq!(bundle.gpu.parameters.max_viewport_dims, [16384, 16384]);
        assert_eq!(bundle.gpu.parameters.max_array_texture_layers, 2048);
        assert_eq!(bundle.gpu.parameters.max_uniform_block_size, 65536);
        assert_eq!(bundle.gpu.parameters.max_uniform_buffer_bindings, 84);
        assert_eq!(bundle.webgpu.limits.max_texture_dimension_2d, 16384);
        assert_eq!(
            bundle.display.avail_height,
            bundle.display.height.saturating_sub(40)
        );
        assert_eq!(
            bundle.display.orientation.orientation_type,
            ScreenOrientationType::LandscapePrimary
        );
        assert_eq!(bundle.media_devices.devices.len(), 3);
        assert_eq!(bundle.client_hints.platform, "Windows");
        assert_ne!(bundle.webgpu.vendor, "");

        let android = FingerprintGenerator::new(77)
            .os(OsType::Android)
            .major(157)
            .generate();
        assert!(
            !android
                .gpu
                .extensions
                .iter()
                .any(|e| e.contains("s3tc") || e.contains("bptc") || e.contains("rgtc"))
        );
        assert!(android.client_hints.mobile);
        assert_eq!(android.display.avail_height, android.display.height);

        let linux = FingerprintGenerator::new(55)
            .os(OsType::Linux)
            .major(157)
            .generate();
        assert_eq!(linux.audio.sample_rate, 44100);
        assert!(linux.fingerprint.validate().is_coherent());
    }

    #[test]
    fn profile_to_fingerprint_roundtrip_preserves_metadata() {
        let generated = FingerprintGenerator::new(33)
            .os(OsType::Macos)
            .major(155)
            .hdr(true)
            .audio_sample_rate(48000)
            .generate();
        let rebuilt = generated
            .profile
            .to_fingerprint()
            .expect("roundtrip fingerprint builds");
        assert!(rebuilt.validate().is_coherent());
        assert_eq!(rebuilt.seed(), generated.fingerprint.seed());
        assert_eq!(rebuilt.audio_sample_rate(), Some(48000));
        assert_eq!(
            rebuilt.avail_screen().value(),
            generated.fingerprint.avail_screen().value()
        );
        assert_eq!(
            rebuilt.media_features().value().map(|m| m.dynamic_range),
            Some(DynamicRange::High)
        );
        assert_eq!(
            rebuilt
                .client_hints()
                .value()
                .map(|c| c.ua_full_version.as_str()),
            Some("155.0.8059.40")
        );
    }

    #[test]
    fn fingerprint_options_builder_validates_constraints_and_is_deterministic() {
        let invalid_ver = FingerprintOptions::builder()
            .min_version(157)
            .max_version(154)
            .build();
        assert!(invalid_ver.is_err());

        let invalid_screen = FingerprintOptions::builder()
            .screen(ScreenConstraints::new().min_width(1920).max_width(1280))
            .build();
        assert!(invalid_screen.is_err());

        let opts = FingerprintOptions::builder()
            .os(OsType::Macos)
            .browser(BrowserType::Chromium)
            .min_version(154)
            .max_version(156)
            .locales(vec!["en-GB".to_owned(), "en".to_owned()])
            .timezone("Europe/London")
            .screen(
                ScreenConstraints::new()
                    .min_width(1280)
                    .max_width(1440)
                    .device_pixel_ratio(2.0)
                    .hdr_mode(HdrMode::High)
                    .color_depth(30),
            )
            .webgl_override("CustomVendor", "CustomRenderer")
            .webgpu_enabled(false)
            .hardware_concurrency(12)
            .device_memory(16)
            .canvas_noise_amplitude(0.02)
            .audio_noise_amplitude(0.001)
            .audio_sample_rate(48000)
            .dom_rect_jitter(0.0005)
            .font_metrics_jitter(0.0002)
            .webrtc_policy(WebRtcPolicy::DisableNonProxiedUdp)
            .connection(ConnectionOverride::new("4g", 50, 10.0).save_data(true))
            .media_devices(1, 2, 1)
            .seed(4242)
            .build()
            .expect("valid options");

        let a = FingerprintGenerator::generate_with(&opts);
        let b = FingerprintGenerator::generate_with(&opts);
        assert_eq!(a.profile, b.profile);
        assert_eq!(a.profile.timezone.as_deref(), Some("Europe/London"));
        assert_eq!(a.profile.webgl_vendor.as_deref(), Some("CustomVendor"));
        assert_eq!(a.profile.webgl_renderer.as_deref(), Some("CustomRenderer"));
        assert_eq!(a.profile.hardware_concurrency, Some(12));
        assert_eq!(a.profile.device_memory, Some(16.0));
        assert!(!a.profile.webgpu_enabled);
        assert_eq!(a.profile.canvas_noise_amplitude, Some(0.02));
        assert_eq!(a.media_devices.devices.len(), 4);
    }
}
