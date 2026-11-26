//! Stealth JavaScript injection bundle.
//!
//! Generates the self-contained JavaScript bundle to be evaluated on every new document
//! (`Page.addScriptToEvaluateOnNewDocument` / WebDriver script injection) to protect
//! browser automation against anti-bot fingerprinting and detection vectors.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use seleniumbase_rs::evasion::fingerprint::{
    AudioNoise, BrowserType, CanvasNoise, Fingerprint, OsType,
};
use seleniumbase_rs::evasion::randomize::PRESET_CHROMIUM_MAJOR;

use crate::fingerprint::{
    AudioMetadata, BatteryMetadata, ClientHintsHighEntropy, ColorScheme, DisplayMetadata,
    GeneratedFingerprint, GpuMetadata, MediaDeviceMetadata, NetworkMetadata, ProfileFingerprint,
    ReducedMotion, WebGlParameters, WebGpuMetadata,
};

const AUDIO_SEED_MASK: u64 = 0x5DEE_CE66;
const DOM_SEED_MASK: u64 = 0xA5A5_5A5A;

/// Selective injection toggles for [`StealthScriptBundle`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each boolean directly toggles one independent WebIDL/JS evasion surface"
)]
pub struct StealthScriptOptions {
    /// Clean automation markers and spoof `navigator.webdriver = false`.
    pub webdriver: bool,
    /// Inject `window.chrome` (`app`, `csi`, `loadTimes`, `runtime`) for Chromium personas.
    pub chrome_runtime: bool,
    /// Align `Notification.permission` and `Permissions.prototype.query`.
    pub permissions: bool,
    /// Spoof `navigator.plugins` and `navigator.mimeTypes`.
    pub plugins_mime: bool,
    /// Spoof WebGL 1 & 2 parameters, extensions, and `readPixels` LSB jitter.
    pub webgl: bool,
    /// Spoof `navigator.gpu` WebGPU adapter info and limits.
    pub webgpu: bool,
    /// Inject deterministic canvas 2D noise.
    pub canvas_noise: bool,
    /// Inject deterministic `OffscreenCanvas` noise.
    pub offscreen_canvas: bool,
    /// Inject Web Audio context and signal noise.
    pub audio_noise: bool,
    /// Inject `Element.getBoundingClientRect` / `Range.getClientRects` jitter.
    pub dom_rect_noise: bool,
    /// Spoof `screen` and `window.devicePixelRatio` geometry.
    pub screen_geometry: bool,
    /// Spoof `navigator.userAgentData` high-entropy Client Hints.
    pub client_hints: bool,
    /// Spoof `navigator.mediaDevices.enumerateDevices`.
    pub media_devices: bool,
    /// Propagate `navigator.webdriver` spoofing into `HTMLIFrameElement.contentWindow`.
    pub iframe_content_window: bool,
    /// Scrub automation/CDP evaluation frames from `Error.prototype.stack`.
    pub error_stack_scrub: bool,
    /// Spoof `navigator.getBattery()`.
    pub battery: bool,
    /// Spoof `window.speechSynthesis.getVoices()`.
    pub speech_voices: bool,
    /// Spoof `navigator.doNotTrack`, `navigator.pdfViewerEnabled`, and `navigator.connection`.
    pub navigator_extras: bool,
    /// Spoof `window.matchMedia()` for `prefers-color-scheme`, `prefers-reduced-motion`, `color-gamut`, and `dynamic-range`.
    pub css_media_queries: bool,
    /// Inject deterministic sub-millisecond jitter into `performance.now()`.
    pub performance_jitter: bool,
    /// Propagate navigator and high-entropy Client Hints into `Worker` and `SharedWorker` scopes.
    pub worker_spoofing: bool,
    /// Spoof `document.fonts.check()` and `CanvasRenderingContext2D.measureText()` font metrics.
    pub fonts: bool,
    /// Align `Intl.DateTimeFormat`/`NumberFormat`/`Collator` `resolvedOptions()` with profile timezone and locale.
    pub intl_timezone: bool,
    /// Filter local ICE candidates (`typ host`) in `RTCPeerConnection` per `webrtc_policy`.
    pub webrtc_guard: bool,
    /// Inject deterministic sub-pixel jitter into `SVGTextContentElement` and `SVGGraphicsElement` metrics.
    pub svg_noise: bool,
    /// Keep `window.outerWidth`/`outerHeight` coherent with `innerWidth`/`innerHeight` (defeats headless 0x0 and fullscreen checks).
    pub window_geometry: bool,
    /// Mask incognito/headless `navigator.storage.estimate()` quota caps.
    pub storage_quota: bool,
    /// Align `navigator.getGamepads()` and hardware API surfaces with desktop Chromium.
    pub hardware_apis: bool,
    /// Keep `document.visibilityState === 'visible'` and `document.hidden === false` in background tabs.
    pub visibility_guard: bool,
    /// Defuse `console.debug(new Error())` `Runtime.stack.access` CDP canary probes.
    pub console_debug_guard: bool,
    /// Offset `MouseEvent.prototype.screenX`/`screenY` by window origin + chrome height to prevent `Input.cordinatesLeak`.
    pub mouse_coord_guard: bool,
    /// Report realistic H.264/AAC/AV1/WebM support in `HTMLMediaElement.canPlayType` and `MediaSource.isTypeSupported`.
    pub codec_support: bool,
    /// Align `Date.prototype.getTimezoneOffset()` with the profile's IANA timezone (DST-aware).
    pub timezone_offset: bool,
    /// Propagate `webdriver` and `toString` patches into newly created `about:blank` / `srcdoc` iframe sandboxes.
    pub iframe_srcdoc_guard: bool,
    /// Keep `window.history.length` realistic (`>= 2`) on fresh browser sessions.
    pub history_length_guard: bool,
    /// Extend deterministic canvas noise to `HTMLCanvasElement.prototype.toBlob` and `OffscreenCanvas.convertToBlob`.
    pub canvas_toblob_guard: bool,
    /// Spoof `WebGLRenderingContext`/`WebGL2RenderingContext.getShaderPrecisionFormat` against `SwiftShader` tells.
    pub webgl_shader_precision: bool,
    /// Spoof `navigator.mediaCapabilities.decodingInfo()` hardware-accelerated video decoding support.
    pub media_capabilities: bool,
    /// Spoof `navigator.keyboard.getLayoutMap()` with a realistic QWERTY layout map.
    pub keyboard_layout: bool,
    /// Ensure `navigator.clipboard` exposes native-looking methods consistently.
    pub clipboard_guard: bool,
}

impl Default for StealthScriptOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl StealthScriptOptions {
    /// Creates options with all evasions enabled.
    #[must_use]
    pub const fn all() -> Self {
        Self::new()
    }

    /// Creates strict options with all evasions enabled (alias for [`Self::all`]).
    #[must_use]
    pub const fn strict() -> Self {
        Self::new()
    }

    /// Creates balanced options: all core, worker, console, mouse, WebRTC, Intl, and media guards
    /// enabled while disabling pixel/audio/SVG noise for visual fidelity.
    #[must_use]
    pub const fn balanced() -> Self {
        Self {
            canvas_noise: false,
            offscreen_canvas: false,
            canvas_toblob_guard: false,
            audio_noise: false,
            dom_rect_noise: false,
            svg_noise: false,
            performance_jitter: false,
            ..Self::new()
        }
    }

    /// Creates options with all evasions enabled.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            webdriver: true,
            chrome_runtime: true,
            permissions: true,
            plugins_mime: true,
            webgl: true,
            webgpu: true,
            canvas_noise: true,
            offscreen_canvas: true,
            audio_noise: true,
            dom_rect_noise: true,
            screen_geometry: true,
            client_hints: true,
            media_devices: true,
            iframe_content_window: true,
            error_stack_scrub: true,
            battery: true,
            speech_voices: true,
            navigator_extras: true,
            css_media_queries: true,
            performance_jitter: true,
            worker_spoofing: true,
            fonts: true,
            intl_timezone: true,
            webrtc_guard: true,
            svg_noise: true,
            window_geometry: true,
            storage_quota: true,
            hardware_apis: true,
            visibility_guard: true,
            console_debug_guard: true,
            mouse_coord_guard: true,
            codec_support: true,
            timezone_offset: true,
            iframe_srcdoc_guard: true,
            history_length_guard: true,
            canvas_toblob_guard: true,
            webgl_shader_precision: true,
            media_capabilities: true,
            keyboard_layout: true,
            clipboard_guard: true,
        }
    }

    /// Creates minimal options enabling only `webdriver`, `chrome_runtime`, `permissions`, and `error_stack_scrub`.
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            webdriver: true,
            chrome_runtime: true,
            permissions: true,
            plugins_mime: false,
            webgl: false,
            webgpu: false,
            canvas_noise: false,
            offscreen_canvas: false,
            audio_noise: false,
            dom_rect_noise: false,
            screen_geometry: false,
            client_hints: false,
            media_devices: false,
            iframe_content_window: false,
            error_stack_scrub: true,
            battery: false,
            speech_voices: false,
            navigator_extras: false,
            css_media_queries: false,
            performance_jitter: false,
            worker_spoofing: false,
            fonts: false,
            intl_timezone: false,
            webrtc_guard: false,
            svg_noise: false,
            window_geometry: false,
            storage_quota: false,
            hardware_apis: false,
            visibility_guard: false,
            console_debug_guard: false,
            mouse_coord_guard: false,
            codec_support: false,
            timezone_offset: false,
            iframe_srcdoc_guard: false,
            history_length_guard: false,
            canvas_toblob_guard: false,
            webgl_shader_precision: false,
            media_capabilities: false,
            keyboard_layout: false,
            clipboard_guard: false,
        }
    }

    /// Enables or disables `webdriver` and automation marker cleanup.
    #[must_use]
    pub const fn webdriver(mut self, enabled: bool) -> Self {
        self.webdriver = enabled;
        self
    }

    /// Enables or disables `window.chrome` runtime spoofing.
    #[must_use]
    pub const fn chrome_runtime(mut self, enabled: bool) -> Self {
        self.chrome_runtime = enabled;
        self
    }

    /// Enables or disables `Notification` / `Permissions` alignment.
    #[must_use]
    pub const fn permissions(mut self, enabled: bool) -> Self {
        self.permissions = enabled;
        self
    }

    /// Enables or disables `navigator.plugins` / `mimeTypes` spoofing.
    #[must_use]
    pub const fn plugins_mime(mut self, enabled: bool) -> Self {
        self.plugins_mime = enabled;
        self
    }

    /// Enables or disables WebGL parameter and `readPixels` spoofing.
    #[must_use]
    pub const fn webgl(mut self, enabled: bool) -> Self {
        self.webgl = enabled;
        self
    }

    /// Enables or disables `navigator.gpu` WebGPU spoofing.
    #[must_use]
    pub const fn webgpu(mut self, enabled: bool) -> Self {
        self.webgpu = enabled;
        self
    }

    /// Enables or disables 2D canvas noise.
    #[must_use]
    pub const fn canvas_noise(mut self, enabled: bool) -> Self {
        self.canvas_noise = enabled;
        self
    }

    /// Enables or disables `OffscreenCanvas` noise.
    #[must_use]
    pub const fn offscreen_canvas(mut self, enabled: bool) -> Self {
        self.offscreen_canvas = enabled;
        self
    }

    /// Enables or disables Web Audio noise.
    #[must_use]
    pub const fn audio_noise(mut self, enabled: bool) -> Self {
        self.audio_noise = enabled;
        self
    }

    /// Enables or disables `DOMRect` jitter.
    #[must_use]
    pub const fn dom_rect_noise(mut self, enabled: bool) -> Self {
        self.dom_rect_noise = enabled;
        self
    }

    /// Enables or disables `screen` geometry spoofing.
    #[must_use]
    pub const fn screen_geometry(mut self, enabled: bool) -> Self {
        self.screen_geometry = enabled;
        self
    }

    /// Enables or disables `navigator.userAgentData` Client Hints spoofing.
    #[must_use]
    pub const fn client_hints(mut self, enabled: bool) -> Self {
        self.client_hints = enabled;
        self
    }

    /// Enables or disables `navigator.mediaDevices.enumerateDevices` spoofing.
    #[must_use]
    pub const fn media_devices(mut self, enabled: bool) -> Self {
        self.media_devices = enabled;
        self
    }

    /// Enables or disables `HTMLIFrameElement.contentWindow` webdriver propagation.
    #[must_use]
    pub const fn iframe_content_window(mut self, enabled: bool) -> Self {
        self.iframe_content_window = enabled;
        self
    }

    /// Enables or disables `Error.prototype.stack` CDP frame scrubbing.
    #[must_use]
    pub const fn error_stack_scrub(mut self, enabled: bool) -> Self {
        self.error_stack_scrub = enabled;
        self
    }

    /// Enables or disables `navigator.getBattery()` spoofing.
    #[must_use]
    pub const fn battery(mut self, enabled: bool) -> Self {
        self.battery = enabled;
        self
    }

    /// Enables or disables `window.speechSynthesis.getVoices()` spoofing.
    #[must_use]
    pub const fn speech_voices(mut self, enabled: bool) -> Self {
        self.speech_voices = enabled;
        self
    }

    /// Enables or disables `navigator.doNotTrack`, `pdfViewerEnabled`, and `connection` spoofing.
    #[must_use]
    pub const fn navigator_extras(mut self, enabled: bool) -> Self {
        self.navigator_extras = enabled;
        self
    }

    /// Enables or disables `window.matchMedia()` CSS media query spoofing.
    #[must_use]
    pub const fn css_media_queries(mut self, enabled: bool) -> Self {
        self.css_media_queries = enabled;
        self
    }

    /// Enables or disables deterministic `performance.now()` timing jitter.
    #[must_use]
    pub const fn performance_jitter(mut self, enabled: bool) -> Self {
        self.performance_jitter = enabled;
        self
    }

    /// Enables or disables `Worker` / `SharedWorker` navigator consistency spoofing.
    #[must_use]
    pub const fn worker_spoofing(mut self, enabled: bool) -> Self {
        self.worker_spoofing = enabled;
        self
    }

    /// Enables or disables font enumeration and `measureText` spoofing.
    #[must_use]
    pub const fn fonts(mut self, enabled: bool) -> Self {
        self.fonts = enabled;
        self
    }

    /// Enables or disables `Intl` timezone and locale consistency.
    #[must_use]
    pub const fn intl_timezone(mut self, enabled: bool) -> Self {
        self.intl_timezone = enabled;
        self
    }

    /// Enables or disables `RTCPeerConnection` host ICE candidate filtering.
    #[must_use]
    pub const fn webrtc_guard(mut self, enabled: bool) -> Self {
        self.webrtc_guard = enabled;
        self
    }

    /// Enables or disables `SVGTextContentElement` and `SVGGraphicsElement` sub-pixel noise.
    #[must_use]
    pub const fn svg_noise(mut self, enabled: bool) -> Self {
        self.svg_noise = enabled;
        self
    }

    /// Enables or disables `window.outerWidth`/`outerHeight` geometry coherence.
    #[must_use]
    pub const fn window_geometry(mut self, enabled: bool) -> Self {
        self.window_geometry = enabled;
        self
    }

    /// Enables or disables `navigator.storage.estimate()` incognito/headless quota masking.
    #[must_use]
    pub const fn storage_quota(mut self, enabled: bool) -> Self {
        self.storage_quota = enabled;
        self
    }

    /// Enables or disables `navigator.getGamepads()` and hardware API alignment.
    #[must_use]
    pub const fn hardware_apis(mut self, enabled: bool) -> Self {
        self.hardware_apis = enabled;
        self
    }

    /// Enables or disables `document.visibilityState` / `document.hidden` foreground masking.
    #[must_use]
    pub const fn visibility_guard(mut self, enabled: bool) -> Self {
        self.visibility_guard = enabled;
        self
    }

    /// Enables or disables `console.debug` `Runtime.stack.access` canary guard.
    #[must_use]
    pub const fn console_debug_guard(mut self, enabled: bool) -> Self {
        self.console_debug_guard = enabled;
        self
    }

    /// Enables or disables `MouseEvent.prototype.screenX`/`screenY` coordinate alignment.
    #[must_use]
    pub const fn mouse_coord_guard(mut self, enabled: bool) -> Self {
        self.mouse_coord_guard = enabled;
        self
    }

    /// Enables or disables media codec (`canPlayType` / `MediaSource.isTypeSupported`) spoofing.
    #[must_use]
    pub const fn codec_support(mut self, enabled: bool) -> Self {
        self.codec_support = enabled;
        self
    }

    /// Enables or disables DST-aware `Date.prototype.getTimezoneOffset()` alignment.
    #[must_use]
    pub const fn timezone_offset(mut self, enabled: bool) -> Self {
        self.timezone_offset = enabled;
        self
    }

    /// Enables or disables `about:blank` / `srcdoc` iframe sandbox propagation.
    #[must_use]
    pub const fn iframe_srcdoc_guard(mut self, enabled: bool) -> Self {
        self.iframe_srcdoc_guard = enabled;
        self
    }

    /// Enables or disables `window.history.length` fresh-session masking.
    #[must_use]
    pub const fn history_length_guard(mut self, enabled: bool) -> Self {
        self.history_length_guard = enabled;
        self
    }

    /// Enables or disables `HTMLCanvasElement.toBlob` / `OffscreenCanvas.convertToBlob` noise.
    #[must_use]
    pub const fn canvas_toblob_guard(mut self, enabled: bool) -> Self {
        self.canvas_toblob_guard = enabled;
        self
    }

    /// Enables or disables `WebGL` shader precision format alignment.
    #[must_use]
    pub const fn webgl_shader_precision(mut self, enabled: bool) -> Self {
        self.webgl_shader_precision = enabled;
        self
    }

    /// Enables or disables `navigator.mediaCapabilities.decodingInfo()` spoofing.
    #[must_use]
    pub const fn media_capabilities(mut self, enabled: bool) -> Self {
        self.media_capabilities = enabled;
        self
    }

    /// Enables or disables `navigator.keyboard.getLayoutMap()` spoofing.
    #[must_use]
    pub const fn keyboard_layout(mut self, enabled: bool) -> Self {
        self.keyboard_layout = enabled;
        self
    }

    /// Enables or disables `navigator.clipboard` consistency guard.
    #[must_use]
    pub const fn clipboard_guard(mut self, enabled: bool) -> Self {
        self.clipboard_guard = enabled;
        self
    }
}

/// Bundle of JavaScript injection scripts for stealth evasions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StealthScriptBundle {
    script: String,
}

impl StealthScriptBundle {
    /// Creates a stealth bundle directly from a validated core [`Fingerprint`].
    #[must_use]
    pub fn new(fingerprint: &Fingerprint) -> Self {
        let profile = ProfileFingerprint::from_fingerprint(fingerprint);
        Self::from_profile(&profile)
    }

    /// Creates a stealth bundle from a complete [`GeneratedFingerprint`] bundle.
    #[must_use]
    pub fn from_generated(generated: &GeneratedFingerprint) -> Self {
        Self::from_generated_with(generated, &StealthScriptOptions::default())
    }

    /// Creates a stealth bundle from a complete [`GeneratedFingerprint`] with selective
    /// [`StealthScriptOptions`].
    #[must_use]
    pub fn from_generated_with(
        generated: &GeneratedFingerprint,
        options: &StealthScriptOptions,
    ) -> Self {
        let params = ScriptParams::from_parts(
            &generated.profile,
            Some(&generated.gpu),
            Some(&generated.webgpu),
            Some(&generated.display),
            generated.profile.client_hints.as_ref(),
            Some(&generated.audio),
            Some(&generated.media_devices),
            generated.profile.battery.as_ref(),
            generated.profile.network.as_ref(),
        );
        Self {
            script: params.render_with(options),
        }
    }

    /// Creates a stealth bundle from a [`ProfileFingerprint`].
    #[must_use]
    pub fn from_profile(profile: &ProfileFingerprint) -> Self {
        Self::from_profile_with(profile, &StealthScriptOptions::default())
    }

    /// Creates a stealth bundle from a [`ProfileFingerprint`] with selective
    /// [`StealthScriptOptions`].
    #[must_use]
    pub fn from_profile_with(profile: &ProfileFingerprint, options: &StealthScriptOptions) -> Self {
        let params = ScriptParams::from_parts(
            profile,
            None,
            None,
            None,
            profile.client_hints.as_ref(),
            None,
            None,
            profile.battery.as_ref(),
            profile.network.as_ref(),
        );
        Self {
            script: params.render_with(options),
        }
    }

    /// Returns the raw JavaScript string for browser injection.
    #[must_use]
    pub fn script(&self) -> &str {
        &self.script
    }

    /// Consumes the bundle and returns the JavaScript string.
    #[must_use]
    pub fn into_script(self) -> String {
        self.script
    }

    /// Clones and returns the JavaScript string.
    #[must_use]
    pub fn to_script(&self) -> String {
        self.script.clone()
    }
}

fn fresh_entropy() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
        });
    let step = COUNTER.fetch_add(1, Ordering::Relaxed);
    (nanos ^ step.wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

fn stable_profile_seed(profile: &ProfileFingerprint) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let parts = [
        profile.user_agent.as_deref().unwrap_or(""),
        profile.platform.as_deref().unwrap_or(""),
        profile.webgl_renderer.as_deref().unwrap_or(""),
        profile.timezone.as_deref().unwrap_or(""),
    ];
    for part in parts {
        for &b in part.as_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn js_str(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}

fn js_json<T: serde::Serialize>(value: &T, fallback: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| fallback.to_owned())
}

fn js_num(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_owned()
    } else if value.is_infinite() {
        if value.is_sign_negative() {
            "-Infinity".to_owned()
        } else {
            "Infinity".to_owned()
        }
    } else {
        value.to_string()
    }
}

struct DisplayResolved {
    screen_w: u32,
    screen_h: u32,
    avail_w: u32,
    avail_h: u32,
    pixel_ratio: f64,
    color_depth: u32,
    orientation_type: String,
    orientation_angle: u32,
}

struct GraphicsResolved {
    unmasked_vendor: String,
    unmasked_renderer: String,
    extensions_json: String,
    webgl_params: WebGlParameters,
    webgpu: WebGpuMetadata,
}

fn resolve_display(
    os: OsType,
    profile: &ProfileFingerprint,
    display: Option<&DisplayMetadata>,
) -> DisplayResolved {
    let (default_w, default_h) = os.default_screen();
    let screen_w = display.map_or_else(|| profile.screen_width.unwrap_or(default_w), |d| d.width);
    let screen_h = display.map_or_else(|| profile.screen_height.unwrap_or(default_h), |d| d.height);
    let avail_w = display.map_or_else(
        || profile.avail_width.unwrap_or(screen_w),
        |d| d.avail_width,
    );
    let avail_h = display.map_or_else(
        || {
            profile.avail_height.unwrap_or_else(|| {
                if os.is_mobile() {
                    screen_h
                } else {
                    screen_h.saturating_sub(40)
                }
            })
        },
        |d| d.avail_height,
    );
    let pixel_ratio = display.map_or_else(
        || profile.device_pixel_ratio.unwrap_or(1.0),
        |d| d.pixel_ratio,
    );
    let color_depth = display.map_or_else(|| profile.color_depth.unwrap_or(24), |d| d.color_depth);
    let orientation_type = display
        .map(|d| d.orientation.orientation_type.as_str().to_owned())
        .or_else(|| {
            profile
                .screen_orientation
                .map(|o| o.orientation_type.as_str().to_owned())
        })
        .unwrap_or_else(|| {
            if screen_h > screen_w || os.is_mobile() {
                "portrait-primary".to_owned()
            } else {
                "landscape-primary".to_owned()
            }
        });
    let orientation_angle = display
        .map(|d| d.orientation.angle)
        .or_else(|| profile.screen_orientation.map(|o| o.angle))
        .unwrap_or(0);

    DisplayResolved {
        screen_w,
        screen_h,
        avail_w,
        avail_h,
        pixel_ratio,
        color_depth,
        orientation_type,
        orientation_angle,
    }
}

fn resolve_graphics(
    os: OsType,
    profile: &ProfileFingerprint,
    gpu: Option<&GpuMetadata>,
    webgpu: Option<&WebGpuMetadata>,
) -> GraphicsResolved {
    let unmasked_vendor = gpu
        .map(|g| g.vendor.clone())
        .or_else(|| profile.webgl_vendor.clone())
        .unwrap_or_else(|| match os {
            OsType::Macos => "Google Inc. (Apple)".to_owned(),
            OsType::Ios => "Apple Inc.".to_owned(),
            OsType::Android => "Qualcomm".to_owned(),
            OsType::Linux => "Google Inc. (Intel)".to_owned(),
            _ => "Google Inc. (NVIDIA)".to_owned(),
        });
    let unmasked_renderer =
        gpu.map(|g| g.renderer.clone())
            .or_else(|| profile.webgl_renderer.clone())
            .unwrap_or_else(|| match os {
                OsType::Macos => {
                    "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)".to_owned()
                }
                OsType::Ios => "Apple GPU".to_owned(),
                OsType::Android => "Adreno (TM) 740".to_owned(),
                OsType::Linux => {
                    "ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6)".to_owned()
                }
                _ => "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)"
                    .to_owned(),
            });
    let extensions = gpu
        .map(|g| &g.extensions)
        .filter(|e| !e.is_empty())
        .unwrap_or(&profile.webgl_extensions);
    let webgl_params = gpu.map_or_else(
        || WebGlParameters::for_gpu(os, &unmasked_vendor, &unmasked_renderer),
        |g| g.parameters.clone(),
    );
    let webgpu = webgpu
        .cloned()
        .unwrap_or_else(|| WebGpuMetadata::for_gpu(os, &unmasked_vendor, &unmasked_renderer));

    GraphicsResolved {
        unmasked_vendor,
        unmasked_renderer,
        extensions_json: js_json(extensions, "[]"),
        webgl_params,
        webgpu,
    }
}

struct ScriptParams {
    os: OsType,
    browser: BrowserType,
    canvas_seed: u64,
    canvas_density: u32,
    audio_seed: u64,
    audio_noise_enabled: bool,
    dom_seed: u64,
    user_agent: Option<String>,
    languages_json: String,
    primary_lang: String,
    platform: String,
    hw_concurrency: u32,
    device_memory: Option<f64>,
    max_touch_points: u32,
    display: DisplayResolved,
    graphics: GraphicsResolved,
    client_hints: Option<ClientHintsHighEntropy>,
    audio: AudioMetadata,
    media_devices_json: String,
    battery: Option<BatteryMetadata>,
    network: Option<NetworkMetadata>,
    fonts_json: String,
    do_not_track: Option<String>,
    pdf_viewer_enabled: Option<bool>,
    color_scheme: Option<ColorScheme>,
    reduced_motion: Option<ReducedMotion>,
    color_gamut: Option<String>,
    hdr: bool,
    speech_voices_json: String,
    performance_jitter_ms: f64,
    worker_spoofing: bool,
    timezone: Option<String>,
    webrtc_policy: Option<seleniumbase_rs::WebRtcPolicy>,
    storage_quota_gb: Option<u64>,
    outer_chrome_height: u32,
    history_min_length: u32,
}

impl ScriptParams {
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "aggregates optional metadata overrides with profile fallbacks"
    )]
    fn from_parts(
        profile: &ProfileFingerprint,
        gpu: Option<&GpuMetadata>,
        webgpu: Option<&WebGpuMetadata>,
        display: Option<&DisplayMetadata>,
        client_hints: Option<&ClientHintsHighEntropy>,
        audio: Option<&AudioMetadata>,
        media_devices: Option<&MediaDeviceMetadata>,
        battery: Option<&BatteryMetadata>,
        network: Option<&NetworkMetadata>,
    ) -> Self {
        let os = profile.os.unwrap_or(OsType::Windows);
        let browser = profile.browser.unwrap_or(if os == OsType::Ios {
            BrowserType::MobileSafari
        } else {
            BrowserType::Chromium
        });

        let base_seed = profile.seed.unwrap_or_else(|| stable_profile_seed(profile));
        let canvas_mode = profile.canvas_noise.unwrap_or(CanvasNoise::Persistent);
        let canvas_seed = profile.canvas_seed.unwrap_or_else(|| {
            if canvas_mode == CanvasNoise::Random {
                base_seed ^ fresh_entropy()
            } else {
                base_seed
            }
        });
        let canvas_density = match canvas_mode {
            CanvasNoise::Natural => 0,
            CanvasNoise::Low => 8,
            _ => 1,
        };

        let audio_mode = profile.audio_noise.unwrap_or(AudioNoise::Persistent);
        let audio_noise_enabled = audio_mode != AudioNoise::Natural;
        let audio_seed = profile.audio_seed.unwrap_or_else(|| {
            if audio_mode == AudioNoise::Random {
                base_seed ^ AUDIO_SEED_MASK ^ fresh_entropy()
            } else {
                base_seed ^ AUDIO_SEED_MASK
            }
        });

        let languages = if profile.languages.is_empty() {
            vec!["en-US".to_owned(), "en".to_owned()]
        } else {
            profile.languages.clone()
        };
        let primary_lang = languages
            .first()
            .cloned()
            .unwrap_or_else(|| "en-US".to_owned());

        let resolved_hints = if browser == BrowserType::Chromium && os != OsType::Ios {
            Some(client_hints.cloned().unwrap_or_else(|| {
                let is_edge = profile
                    .user_agent
                    .as_deref()
                    .is_some_and(|ua| ua.contains("Edg/"));
                ClientHintsHighEntropy::for_browser(os, is_edge, PRESET_CHROMIUM_MAJOR)
            }))
        } else {
            None
        };

        let resolved_audio = audio.cloned().unwrap_or_else(|| AudioMetadata {
            sample_rate: profile.audio_sample_rate.unwrap_or(
                if matches!(os, OsType::Linux | OsType::Android) {
                    44100
                } else {
                    48000
                },
            ),
            ..AudioMetadata::default()
        });

        Self {
            os,
            browser,
            canvas_seed,
            canvas_density,
            audio_seed,
            audio_noise_enabled,
            dom_seed: base_seed ^ DOM_SEED_MASK,
            user_agent: profile.user_agent.clone(),
            languages_json: js_json(&languages, "[\"en-US\",\"en\"]"),
            primary_lang,
            platform: profile
                .platform
                .clone()
                .unwrap_or_else(|| os.platform().to_owned()),
            hw_concurrency: profile.hardware_concurrency.unwrap_or(8),
            device_memory: if os == OsType::Ios || browser == BrowserType::MobileSafari {
                None
            } else {
                profile.device_memory.or(Some(8.0))
            },
            max_touch_points: profile.max_touch_points.unwrap_or(if os.is_mobile() {
                5
            } else {
                0
            }),
            display: resolve_display(os, profile, display),
            graphics: resolve_graphics(os, profile, gpu, webgpu),
            client_hints: resolved_hints,
            audio: resolved_audio,
            media_devices_json: media_devices
                .map_or_else(|| "[]".to_owned(), |m| js_json(&m.devices, "[]")),
            battery: battery.copied(),
            network: network.cloned(),
            fonts_json: js_json(&profile.fonts, "[]"),
            do_not_track: profile.do_not_track.clone(),
            pdf_viewer_enabled: profile.pdf_viewer_enabled,
            color_scheme: profile.color_scheme,
            reduced_motion: profile.reduced_motion,
            color_gamut: profile
                .color_gamut
                .map(|g| format!("{g:?}").to_ascii_lowercase()),
            hdr: profile.hdr.unwrap_or(false),
            speech_voices_json: js_json(&profile.speech_voices, "[]"),
            performance_jitter_ms: profile.performance_jitter_ms.unwrap_or(0.05),
            worker_spoofing: profile.worker_spoofing,
            timezone: profile.timezone.clone(),
            webrtc_policy: profile.webrtc_policy,
            storage_quota_gb: profile.storage_quota_gb,
            outer_chrome_height: profile.outer_chrome_height.unwrap_or(if os.is_mobile() {
                0
            } else {
                88
            }),
            history_min_length: profile.history_min_length.unwrap_or(2),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "dispatches every enabled StealthScriptOptions evasion section in order"
    )]
    fn render_with(&self, options: &StealthScriptOptions) -> String {
        let mut out = String::with_capacity(20 * 1024);
        out.push_str("(function() {\n  'use strict';\n\n");
        Self::push_native_helpers(&mut out);
        if options.webdriver || options.iframe_content_window || options.error_stack_scrub {
            Self::push_markers_webdriver_and_stack(&mut out);
        }
        self.push_navigator(&mut out);
        if options.navigator_extras {
            self.push_navigator_extras(&mut out);
        }
        if options.client_hints {
            self.push_client_hints(&mut out);
        }
        if options.screen_geometry {
            self.push_screen(&mut out);
        }
        if options.css_media_queries {
            self.push_css_media_queries(&mut out);
        }
        if options.dom_rect_noise {
            self.push_dom_rects(&mut out);
        }
        if options.canvas_noise {
            self.push_canvas_noise(&mut out);
        }
        if options.offscreen_canvas {
            self.push_offscreen_canvas_noise(&mut out);
        }
        if options.fonts {
            self.push_fonts_and_measure_text(&mut out);
        }
        if options.webgl {
            self.push_webgl_parameters(&mut out);
            self.push_webgl_extras(&mut out);
        }
        if options.webgpu {
            self.push_webgpu(&mut out);
        }
        if options.audio_noise {
            self.push_audio_context(&mut out);
            self.push_audio_signal_noise(&mut out);
        }
        if options.media_devices || options.battery || options.navigator_extras {
            self.push_devices_battery_network(&mut out);
        }
        if options.speech_voices {
            self.push_speech_voices(&mut out);
        }
        if options.performance_jitter {
            self.push_performance_jitter(&mut out);
        }
        if options.worker_spoofing && self.worker_spoofing {
            self.push_worker_spoofing(&mut out);
        }
        if options.intl_timezone {
            self.push_intl_timezone(&mut out);
        }
        if options.webrtc_guard {
            self.push_webrtc_guard(&mut out);
        }
        if options.svg_noise {
            self.push_svg_noise(&mut out);
        }
        if options.window_geometry {
            self.push_window_geometry(&mut out);
        }
        if options.storage_quota {
            self.push_storage_quota(&mut out);
        }
        if options.hardware_apis {
            Self::push_hardware_apis(&mut out);
        }
        if options.visibility_guard {
            Self::push_visibility_guard(&mut out);
        }
        if options.console_debug_guard {
            Self::push_console_debug_guard(&mut out);
        }
        if options.mouse_coord_guard {
            self.push_mouse_coord_guard(&mut out);
        }
        if options.codec_support {
            Self::push_codec_support(&mut out);
        }
        if options.timezone_offset {
            self.push_timezone_offset(&mut out);
        }
        if options.iframe_srcdoc_guard {
            Self::push_iframe_srcdoc_guard(&mut out);
        }
        if options.history_length_guard {
            self.push_history_length_guard(&mut out);
        }
        if options.canvas_toblob_guard {
            Self::push_canvas_toblob_guard(&mut out);
        }
        if options.webgl_shader_precision {
            Self::push_webgl_shader_precision(&mut out);
        }
        if options.media_capabilities {
            Self::push_media_capabilities(&mut out);
        }
        if options.keyboard_layout {
            Self::push_keyboard_layout(&mut out);
        }
        if options.clipboard_guard {
            Self::push_clipboard_guard(&mut out);
        }
        if options.chrome_runtime || options.permissions {
            self.push_chrome_and_permissions(&mut out);
        }
        out.push_str("})();\n");
        out
    }

    fn push_native_helpers(out: &mut String) {
        out.push_str(
            r"  // 0. Native Function.prototype.toString wrapper and prototype helpers
  const nativeToString = Function.prototype.toString;
  const nativeNames = new WeakMap();
  const patchedToString = new Proxy(nativeToString, {
    apply: function(target, thisArg, args) {
      if (nativeNames.has(thisArg)) {
        return 'function ' + nativeNames.get(thisArg) + '() { [native code] }';
      }
      return Reflect.apply(target, thisArg, args);
    }
  });
  nativeNames.set(patchedToString, 'toString');
  try {
    Object.defineProperty(Function.prototype, 'toString', {
      value: patchedToString,
      writable: true,
      configurable: true
    });
  } catch (_) {
    Function.prototype.toString = patchedToString;
  }

  function markNative(fn, name) {
    nativeNames.set(fn, name);
    return fn;
  }

  function makeNative(fn, name, like) {
    if (typeof fn !== 'function') return fn;
    const label = String(name || (like && like.name) || fn.name || '');
    const holder = { [label](...args) { return Reflect.apply(fn, this, args); } };
    const wrapper = holder[label];
    try {
      Object.defineProperty(wrapper, 'length', {
        value: like ? like.length : fn.length,
        configurable: true
      });
    } catch (_) {}
    return markNative(wrapper, label);
  }

  function defineGetter(proto, prop, fn) {
    if (!proto) return;
    const current = Object.getOwnPropertyDescriptor(proto, prop);
    Object.defineProperty(proto, prop, {
      get: makeNative(fn, 'get ' + prop),
      set: current ? current.set : undefined,
      enumerable: current ? current.enumerable : true,
      configurable: true
    });
  }

  function hashWithSeed(seedVal, index) {
    let h = ((seedVal >>> 0) ^ Math.imul(index + 1, 0x9E3779B1)) >>> 0;
    h = Math.imul(h ^ (h >>> 15), 0x85EBCA77) >>> 0;
    h = Math.imul(h ^ (h >>> 13), 0xC2B2AE3D) >>> 0;
    return (h ^ (h >>> 16)) >>> 0;
  }

  function hashString(seedVal, str) {
    let h = (0x811c9dc5 ^ (seedVal >>> 0)) >>> 0;
    for (let i = 0; i < str.length; i++) {
      h = Math.imul(h ^ str.charCodeAt(i), 0x01000193) >>> 0;
    }
    return h >>> 0;
  }

",
        );
    }

    fn push_markers_webdriver_and_stack(out: &mut String) {
        out.push_str(
            r"  // 1. Clean automation markers, navigator.webdriver = false, and CDP stack scrubber
  try {
    const markerPatterns = [/^\$?cdc_/, /__webdriver/, /__selenium/, /__driver/, /\$chrome_/, /__fxdriver/];
    const scrubObj = (obj) => {
      if (!obj) return;
      Object.getOwnPropertyNames(obj).forEach((key) => {
        if (markerPatterns.some((p) => p.test(key))) {
          try { delete obj[key]; } catch (_) {}
        }
      });
    };
    const hooks = ['callPhantom', '_phantom', '__nightmare', '__selenium_unwrapped',
      '__webdriver_evaluate', '__driver_evaluate', '__webdriver_script_fn',
      '__webdriver_script_function', '__driver_unwrapped', '__fxdriver_evaluate',
      '__selenium_evaluate', '__webdriver_script_func', '_Selenium_IDE_Recorder',
      'domAutomation', 'domAutomationController'];
    hooks.forEach((h) => {
      try { delete window[h]; } catch (_) {}
      try { if (typeof document !== 'undefined') delete document[h]; } catch (_) {}
    });
    let w = window;
    while (w) { scrubObj(w); w = Object.getPrototypeOf(w); }
    if (typeof document !== 'undefined') {
      let d = document;
      while (d) { scrubObj(d); d = Object.getPrototypeOf(d); }
    }
  } catch (_) {}

  try {
    try { delete navigator.webdriver; } catch (_) {}
    const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : Object.getPrototypeOf(navigator);
    defineGetter(navProto, 'webdriver', () => false);
    if (typeof HTMLIFrameElement !== 'undefined') {
      const cwDesc = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, 'contentWindow');
      if (cwDesc && cwDesc.get) {
        const origCw = cwDesc.get;
        Object.defineProperty(HTMLIFrameElement.prototype, 'contentWindow', {
          get: makeNative(function contentWindow() {
            const win = origCw.call(this);
            if (win) {
              try {
                if (win.navigator && win.navigator.webdriver !== false) {
                  try { delete win.navigator.webdriver; } catch (_) {}
                  const frameNavProto = win.Navigator ? win.Navigator.prototype : Object.getPrototypeOf(win.navigator);
                  defineGetter(frameNavProto, 'webdriver', () => false);
                }
              } catch (_) {}
            }
            return win;
          }, 'get contentWindow', origCw),
          enumerable: cwDesc.enumerable,
          configurable: true
        });
      }
    }
  } catch (_) {}

  // Scrub CDP evaluation frames from Error.prototype.stack without defining a custom stack formatter.
  try {
    const stackDesc = Object.getOwnPropertyDescriptor(Error.prototype, 'stack');
    if (stackDesc && typeof stackDesc.get === 'function') {
      const origStackGet = stackDesc.get;
      Object.defineProperty(Error.prototype, 'stack', {
        get: makeNative(function() {
          const raw = origStackGet.call(this);
          if (typeof raw !== 'string') return raw;
          return raw
            .split('\n')
            .filter((line) => {
              return !line.includes('__puppeteer_evaluation_script__') &&
                     !line.includes('__playwright_evaluation_script__') &&
                     !line.includes('__webdriver_evaluate') &&
                     !line.includes('__cdp_') &&
                     !line.includes('UtilityScript.') &&
                     !line.includes('evaluate (at <anonymous>:') &&
                     !line.includes('devtools://') &&
                     !line.includes('ExecutionContext.evaluate') &&
                     !line.includes('InjectedScript.') &&
                     !line.includes('pptr:');
            })
            .join('\n');
        }, 'get stack', origStackGet),
        set: stackDesc.set,
        enumerable: stackDesc.enumerable,
        configurable: true
      });
    }
  } catch (_) {}

",
        );
    }

    fn push_navigator(&self, out: &mut String) {
        let primary_lang = js_str(&self.primary_lang);
        let platform = js_str(&self.platform);
        let _ = writeln!(
            out,
            "  // 2. Navigator properties & Languages (prototype-level)
  try {{
    const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : Object.getPrototypeOf(navigator);
    const languages = Object.freeze({languages});
    defineGetter(navProto, 'languages', () => languages);
    defineGetter(navProto, 'language', () => {primary_lang});
    defineGetter(navProto, 'platform', () => {platform});
    defineGetter(navProto, 'hardwareConcurrency', () => {hw});
    defineGetter(navProto, 'maxTouchPoints', () => {touch});",
            languages = self.languages_json,
            hw = self.hw_concurrency,
            touch = self.max_touch_points,
        );
        if let Some(mem) = self.device_memory {
            let mem_js = js_num(mem);
            let _ = writeln!(
                out,
                "    defineGetter(navProto, 'deviceMemory', () => {mem_js});"
            );
        }
        if let Some(ua) = &self.user_agent {
            let ua_js = js_str(ua);
            let app_ver = js_str(&ua.replacen("Mozilla/", "", 1));
            let _ = writeln!(
                out,
                "    defineGetter(navProto, 'userAgent', () => {ua_js});\n    defineGetter(navProto, 'appVersion', () => {app_ver});"
            );
        }
        out.push_str("  } catch (_) {}\n\n");
    }

    fn push_client_hints(&self, out: &mut String) {
        let Some(ch) = &self.client_hints else {
            out.push_str(
                "  // 3. Client Hints absent for non-Chromium persona\n  try {\n    const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : Object.getPrototypeOf(navigator);\n    delete navProto.userAgentData;\n  } catch (_) {}\n\n",
            );
            return;
        };

        let brands_json = js_json(&ch.brands, "[]");
        let full_list_json = js_json(&ch.full_version_list, "[]");
        let form_factors_json = js_json(&ch.form_factors, "[\"Desktop\"]");
        let arch = js_str(&ch.architecture);
        let bitness = js_str(&ch.bitness);
        let model = js_str(&ch.model);
        let platform = js_str(&ch.platform);
        let platform_v = js_str(&ch.platform_version);
        let ua_full = js_str(&ch.ua_full_version);
        let mobile = ch.mobile;
        let wow64 = ch.wow64;

        let _ = writeln!(
            out,
            r"  // 3. Client Hints (navigator.userAgentData)
  try {{
    const brands = Object.freeze({brands_json}.map((b) => Object.freeze({{ brand: b.brand, version: b.version }})));
    const fullVersionList = Object.freeze({full_list_json}.map((b) => Object.freeze({{ brand: b.brand, version: b.version }})));
    const formFactors = Object.freeze({form_factors_json});
    const highEntropy = {{
      architecture: {arch},
      bitness: {bitness},
      model: {model},
      platform: {platform},
      platformVersion: {platform_v},
      uaFullVersion: {ua_full},
      fullVersionList: fullVersionList,
      brands: brands,
      mobile: {mobile},
      wow64: {wow64},
      formFactors: formFactors
    }};
    const uaProto = typeof NavigatorUAData !== 'undefined'
      ? NavigatorUAData.prototype
      : (navigator.userAgentData ? Object.getPrototypeOf(navigator.userAgentData) : null);
    const getHighEntropyImpl = function getHighEntropyValues(hints) {{
      const res = {{ brands: brands, mobile: {mobile}, platform: {platform} }};
      if (Array.isArray(hints)) {{
        for (const key of hints) {{
          if (Object.prototype.hasOwnProperty.call(highEntropy, key)) {{
            res[key] = highEntropy[key];
          }}
        }}
      }}
      return Promise.resolve(res);
    }};
    const toJsonImpl = function toJSON() {{
      return {{ brands: brands, mobile: {mobile}, platform: {platform} }};
    }};
    if (uaProto) {{
      defineGetter(uaProto, 'brands', () => brands);
      defineGetter(uaProto, 'mobile', () => {mobile});
      defineGetter(uaProto, 'platform', () => {platform});
      uaProto.getHighEntropyValues = makeNative(getHighEntropyImpl, 'getHighEntropyValues', uaProto.getHighEntropyValues);
      uaProto.toJSON = makeNative(toJsonImpl, 'toJSON', uaProto.toJSON);
    }}
    if (!navigator.userAgentData && window.isSecureContext) {{
      const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : Object.getPrototypeOf(navigator);
      const uaData = {{
        brands: brands,
        mobile: {mobile},
        platform: {platform},
        getHighEntropyValues: makeNative(getHighEntropyImpl, 'getHighEntropyValues'),
        toJSON: makeNative(toJsonImpl, 'toJSON')
      }};
      defineGetter(navProto, 'userAgentData', () => uaData);
    }}
  }} catch (_) {{}}
"
        );
    }

    fn push_screen(&self, out: &mut String) {
        let d = &self.display;
        let orientation_type = js_str(&d.orientation_type);
        let _ = writeln!(
            out,
            r"  // 4. Screen geometry (Screen.prototype & ScreenOrientation)
  try {{
    if (typeof Screen !== 'undefined') {{
      defineGetter(Screen.prototype, 'width', () => {w});
      defineGetter(Screen.prototype, 'height', () => {h});
      defineGetter(Screen.prototype, 'availWidth', () => {aw});
      defineGetter(Screen.prototype, 'availHeight', () => {ah});
      defineGetter(Screen.prototype, 'availTop', () => 0);
      defineGetter(Screen.prototype, 'availLeft', () => 0);
      defineGetter(Screen.prototype, 'isExtended', () => false);
      defineGetter(Screen.prototype, 'colorDepth', () => {cd});
      defineGetter(Screen.prototype, 'pixelDepth', () => {cd});
    }}
    const dprDesc = Object.getOwnPropertyDescriptor(window, 'devicePixelRatio');
    Object.defineProperty(window, 'devicePixelRatio', {{
      get: makeNative(() => {dpr}, 'get devicePixelRatio'),
      set: dprDesc ? dprDesc.set : undefined,
      enumerable: dprDesc ? dprDesc.enumerable : true,
      configurable: true
    }});
    if (typeof ScreenOrientation !== 'undefined') {{
      defineGetter(ScreenOrientation.prototype, 'type', () => {orientation_type});
      defineGetter(ScreenOrientation.prototype, 'angle', () => {angle});
    }}
  }} catch (_) {{}}
",
            w = d.screen_w,
            h = d.screen_h,
            aw = d.avail_w,
            ah = d.avail_h,
            cd = d.color_depth,
            dpr = js_num(d.pixel_ratio),
            angle = d.orientation_angle,
        );
    }

    fn push_dom_rects(&self, out: &mut String) {
        let _ = writeln!(
            out,
            r"  // 4b. DOMRect sub-pixel jitter (Element & Range getBoundingClientRect / getClientRects)
  try {{
    const domSeed = ({dom_seed} >>> 0);
    const jitterRect = (rect) => {{
      if (!rect || (rect.width === 0 && rect.height === 0)) return rect;
      const key = (Math.round(rect.x * 100) ^ Math.imul(Math.round(rect.y * 100), 31) ^
        Math.imul(Math.round(rect.width * 100), 131) ^ Math.imul(Math.round(rect.height * 100), 8191)) >>> 0;
      const dx = ((hashWithSeed(domSeed, key) / 4294967296) - 0.5) * 2e-4;
      const dy = ((hashWithSeed(domSeed, key + 1) / 4294967296) - 0.5) * 2e-4;
      const dw = ((hashWithSeed(domSeed, key + 2) / 4294967296) - 0.5) * 2e-4;
      const dh = ((hashWithSeed(domSeed, key + 3) / 4294967296) - 0.5) * 2e-4;
      const nx = rect.x + dx;
      const ny = rect.y + dy;
      const nw = Math.max(0, rect.width + dw);
      const nh = Math.max(0, rect.height + dh);
      if (typeof DOMRect === 'function') {{
        return new DOMRect(nx, ny, nw, nh);
      }}
      return {{
        x: nx,
        y: ny,
        width: nw,
        height: nh,
        top: ny,
        left: nx,
        right: nx + nw,
        bottom: ny + nh,
        toJSON: function() {{ return this; }}
      }};
    }};
    const jitterRectList = (list) => {{
      const rects = Array.from(list, jitterRect);
      const outList = typeof DOMRectList !== 'undefined' ? Object.create(DOMRectList.prototype) : {{}};
      rects.forEach((r, idx) => {{
        Object.defineProperty(outList, idx, {{ value: r, enumerable: true, configurable: true }});
      }});
      Object.defineProperty(outList, 'length', {{ value: rects.length, configurable: true }});
      Object.defineProperty(outList, 'item', {{
        value: makeNative(function item(index) {{ return rects[index] || null; }}, 'item'),
        configurable: true
      }});
      Object.defineProperty(outList, Symbol.iterator, {{
        value: makeNative(function() {{ return rects.values(); }}, 'values'),
        configurable: true
      }});
      return outList;
    }};
    if (typeof Element !== 'undefined') {{
      const origElemRect = Element.prototype.getBoundingClientRect;
      Element.prototype.getBoundingClientRect = makeNative(function getBoundingClientRect() {{
        return jitterRect(origElemRect.apply(this, arguments));
      }}, 'getBoundingClientRect', origElemRect);
      const origElemRects = Element.prototype.getClientRects;
      Element.prototype.getClientRects = makeNative(function getClientRects() {{
        return jitterRectList(origElemRects.apply(this, arguments));
      }}, 'getClientRects', origElemRects);
    }}
    if (typeof Range !== 'undefined') {{
      const origRangeRect = Range.prototype.getBoundingClientRect;
      Range.prototype.getBoundingClientRect = makeNative(function getBoundingClientRect() {{
        return jitterRect(origRangeRect.apply(this, arguments));
      }}, 'getBoundingClientRect', origRangeRect);
      const origRangeRects = Range.prototype.getClientRects;
      Range.prototype.getClientRects = makeNative(function getClientRects() {{
        return jitterRectList(origRangeRects.apply(this, arguments));
      }}, 'getClientRects', origRangeRects);
    }}
  }} catch (_) {{}}
",
            dom_seed = self.dom_seed & 0xffff_ffff,
        );
    }

    fn push_canvas_noise(&self, out: &mut String) {
        let _ = writeln!(
            out,
            r"  // 5. Canvas pixel noise (toDataURL, toBlob, getImageData)
  try {{
    const canvasSeed = ({canvas_seed} >>> 0);
    const canvasDensity = {canvas_density};
    const perturbPixels = (data, width, x0, y0) => {{
      if (canvasDensity <= 0 || !width) return;
      for (let i = 0; i < data.length; i += 4) {{
        const pixel = i / 4;
        const col = pixel % width;
        const row = (pixel - col) / width;
        const key = (y0 + row) * 65536 + x0 + col;
        if (canvasDensity > 1 && hashWithSeed(canvasSeed, key + 7919) % canvasDensity !== 0) continue;
        const nudge = (hashWithSeed(canvasSeed, key) % 3) - 1;
        if (nudge !== 0) data[i] = Math.max(0, Math.min(255, data[i] + nudge));
      }}
    }};

    if (canvasDensity > 0 && typeof HTMLCanvasElement !== 'undefined' && typeof CanvasRenderingContext2D !== 'undefined') {{
      const createElement = Document.prototype.createElement;
      const getContext = HTMLCanvasElement.prototype.getContext;
      const origGetImageData = CanvasRenderingContext2D.prototype.getImageData;
      const putImageData = CanvasRenderingContext2D.prototype.putImageData;
      const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
      const origToBlob = HTMLCanvasElement.prototype.toBlob;

      const noisyCopy = (canvas) => {{
        try {{
          if (!canvas.width || !canvas.height) return null;
          const copy = createElement.call(document, 'canvas');
          copy.width = canvas.width;
          copy.height = canvas.height;
          const ctx = getContext.call(copy, '2d');
          ctx.drawImage(canvas, 0, 0);
          const image = origGetImageData.call(ctx, 0, 0, copy.width, copy.height);
          perturbPixels(image.data, image.width, 0, 0);
          putImageData.call(ctx, image, 0, 0);
          return copy;
        }} catch (_) {{
          return null;
        }}
      }};

      HTMLCanvasElement.prototype.toDataURL = makeNative(function toDataURL() {{
        return origToDataURL.apply(noisyCopy(this) || this, arguments);
      }}, 'toDataURL', origToDataURL);

      if (origToBlob) {{
        HTMLCanvasElement.prototype.toBlob = makeNative(function toBlob() {{
          return origToBlob.apply(noisyCopy(this) || this, arguments);
        }}, 'toBlob', origToBlob);
      }}

      CanvasRenderingContext2D.prototype.getImageData = makeNative(function getImageData(sx, sy) {{
        const image = origGetImageData.apply(this, arguments);
        perturbPixels(image.data, image.width, Math.floor(Number(sx)) || 0, Math.floor(Number(sy)) || 0);
        return image;
      }}, 'getImageData', origGetImageData);
    }}
  }} catch (_) {{}}
",
            canvas_seed = self.canvas_seed & 0xffff_ffff,
            canvas_density = self.canvas_density,
        );
    }

    fn push_offscreen_canvas_noise(&self, out: &mut String) {
        let _ = writeln!(
            out,
            r"  // 5b. OffscreenCanvas consistency (getImageData, convertToBlob)
  try {{
    const canvasSeed = ({canvas_seed} >>> 0);
    const canvasDensity = {canvas_density};
    const perturbPixels = (data, width, x0, y0) => {{
      if (canvasDensity <= 0 || !width) return;
      for (let i = 0; i < data.length; i += 4) {{
        const pixel = i / 4;
        const col = pixel % width;
        const row = (pixel - col) / width;
        const key = (y0 + row) * 65536 + x0 + col;
        if (canvasDensity > 1 && hashWithSeed(canvasSeed, key + 7919) % canvasDensity !== 0) continue;
        const nudge = (hashWithSeed(canvasSeed, key) % 3) - 1;
        if (nudge !== 0) data[i] = Math.max(0, Math.min(255, data[i] + nudge));
      }}
    }};

    if (canvasDensity > 0 && typeof OffscreenCanvasRenderingContext2D !== 'undefined') {{
      const origOffscreenGet = OffscreenCanvasRenderingContext2D.prototype.getImageData;
      OffscreenCanvasRenderingContext2D.prototype.getImageData = makeNative(function getImageData(sx, sy) {{
        const image = origOffscreenGet.apply(this, arguments);
        perturbPixels(image.data, image.width, Math.floor(Number(sx)) || 0, Math.floor(Number(sy)) || 0);
        return image;
      }}, 'getImageData', origOffscreenGet);
    }}

    if (canvasDensity > 0 && typeof OffscreenCanvas !== 'undefined' && OffscreenCanvas.prototype.convertToBlob) {{
      const origConvertToBlob = OffscreenCanvas.prototype.convertToBlob;
      OffscreenCanvas.prototype.convertToBlob = makeNative(function convertToBlob() {{
        try {{
          if (this.width && this.height) {{
            const copy = new OffscreenCanvas(this.width, this.height);
            const ctx = copy.getContext('2d');
            if (ctx) {{
              ctx.drawImage(this, 0, 0);
              const img = ctx.getImageData(0, 0, this.width, this.height);
              perturbPixels(img.data, img.width, 0, 0);
              ctx.putImageData(img, 0, 0);
              return origConvertToBlob.apply(copy, arguments);
            }}
          }}
        }} catch (_) {{}}
        return origConvertToBlob.apply(this, arguments);
      }}, 'convertToBlob', origConvertToBlob);
    }}
  }} catch (_) {{}}
",
            canvas_seed = self.canvas_seed & 0xffff_ffff,
            canvas_density = self.canvas_density,
        );
    }

    fn push_fonts_and_measure_text(&self, out: &mut String) {
        let _ = writeln!(
            out,
            r"  // 5c. FontFaceSet shield and CanvasRenderingContext2D.measureText jitter
  try {{
    const canvasSeed = ({canvas_seed} >>> 0);
    const canvasDensity = {canvas_density};
    const allowedFonts = new Set({fonts_json});
    const genericFonts = new Set(['serif', 'sans-serif', 'monospace', 'cursive', 'fantasy',
      'system-ui', 'ui-serif', 'ui-sans-serif', 'ui-monospace', 'ui-rounded', 'math', 'emoji', 'fangsong']);
    const fontSizeToken = /^(?:(?:\d+(?:\.\d+)?|\.\d+)(?:px|pt|pc|em|ex|ch|rem|vw|vh|vmin|vmax|cm|mm|in|q|%)|(?:xx-small|x-small|small|medium|large|x-large|xx-large|xxx-large|smaller|larger))(?:\/\S*)?$/i;
    const parseFontFamily = (spec) => {{
      if (typeof spec !== 'string') return '';
      const first = spec.split(',')[0].replace(/['\x22]/g, '').trim();
      const tokens = first.split(/\s+/);
      for (let i = 0; i < tokens.length - 1; i++) {{
        if (fontSizeToken.test(tokens[i])) return tokens.slice(i + 1).join(' ');
      }}
      return first;
    }};
    const fontAllowed = (family) => allowedFonts.size === 0 || allowedFonts.has(family) || genericFonts.has(family.toLowerCase());

    if (allowedFonts.size > 0 && typeof FontFaceSet !== 'undefined') {{
      const origCheck = FontFaceSet.prototype.check;
      FontFaceSet.prototype.check = makeNative(function check(spec) {{
        return fontAllowed(parseFontFamily(spec));
      }}, 'check', origCheck);
    }}

    if (typeof CanvasRenderingContext2D !== 'undefined') {{
      const origMeasureText = CanvasRenderingContext2D.prototype.measureText;
      CanvasRenderingContext2D.prototype.measureText = makeNative(function measureText(text) {{
        const family = parseFontFamily(this.font);
        let metrics;
        if (family && !fontAllowed(family)) {{
          const saved = this.font;
          try {{
            this.font = this.font.replace(family, 'sans-serif');
            metrics = origMeasureText.apply(this, arguments);
          }} finally {{
            this.font = saved;
          }}
        }} else {{
          metrics = origMeasureText.apply(this, arguments);
        }}
        if (canvasDensity <= 0 || !metrics) return metrics;
        const h = hashString(canvasSeed, String(text) + '|' + String(this.font));
        const delta = ((h / 4294967296) - 0.5) * 2e-4;
        const wrapped = typeof TextMetrics !== 'undefined' ? Object.create(TextMetrics.prototype) : {{}};
        const props = ['width', 'actualBoundingBoxLeft', 'actualBoundingBoxRight',
          'actualBoundingBoxAscent', 'actualBoundingBoxDescent',
          'fontBoundingBoxAscent', 'fontBoundingBoxDescent',
          'alphabeticBaseline', 'emHeightAscent', 'emHeightDescent',
          'hangingBaseline', 'ideographicBaseline'];
        for (const prop of props) {{
          if (prop in metrics) {{
            const val = metrics[prop];
            const adjusted = (prop === 'width' || prop === 'actualBoundingBoxLeft' || prop === 'actualBoundingBoxRight')
              ? val + delta
              : val;
            Object.defineProperty(wrapped, prop, {{ value: adjusted, enumerable: true, configurable: true }});
          }}
        }}
        return wrapped;
      }}, 'measureText', origMeasureText);
    }}
  }} catch (_) {{}}
",
            canvas_seed = self.canvas_seed & 0xffff_ffff,
            canvas_density = self.canvas_density,
            fonts_json = self.fonts_json,
        );
    }

    fn push_webgl_parameters(&self, out: &mut String) {
        let g = &self.graphics;
        let p = &g.webgl_params;
        let unmasked_vendor = js_str(&g.unmasked_vendor);
        let unmasked_renderer = js_str(&g.unmasked_renderer);
        let vendor_str = js_str(&p.vendor);
        let renderer_str = js_str(&p.renderer);
        let max_viewport_dims_json = js_json(&p.max_viewport_dims, "[16384,16384]");
        let line_width_json = js_json(&p.aliased_line_width_range, "[1.0,1.0]");
        let point_size_json = js_json(&p.aliased_point_size_range, "[1.0,1024.0]");

        let _ = writeln!(
            out,
            r"  // 6. WebGL 1 & 2 parameter tables
  try {{
    const unmaskedVendor = {unmasked_vendor};
    const unmaskedRenderer = {unmasked_renderer};
    const maxViewportDims = new Int32Array({max_viewport_dims_json});
    const lineWidthRange = new Float32Array({line_width_json});
    const pointSizeRange = new Float32Array({point_size_json});

    const patchWebGLParams = (proto, isWebGL2) => {{
      if (!proto) return;
      const origGetParameter = proto.getParameter;
      proto.getParameter = makeNative(function getParameter(param) {{
        switch (param) {{
          case 37445: return unmaskedVendor;
          case 37446: return unmaskedRenderer;
          case 7936: return {vendor_str};
          case 7937: return {renderer_str};
          case 35724: return isWebGL2
            ? 'WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)'
            : 'WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)';
          case 3379: return {max_tex};
          case 34076: return {max_cube};
          case 34024: return {max_rb};
          case 3386: return maxViewportDims;
          case 33902: return lineWidthRange;
          case 33901: return pointSizeRange;
          case 34921: return {max_attribs};
          case 36347: return {max_v_uniform};
          case 36348: return {max_varying};
          case 36349: return {max_f_uniform};
          case 35661: return {max_combined_tex};
          case 35660: return {max_v_tex};
          case 34930: return {max_tex_units};
          case 3414: return 24;
          case 3415: return 8;
          case 32883: return {max_3d};
          case 35071: return {max_array_layers};
          case 36063: return {max_color_att};
          case 34852: return {max_draw_buf};
          case 35375: return {max_ubo_bindings};
          case 35376: return {max_ubo_size};
          case 35380: return {ubo_align};
          case 37154: return {max_v_out};
          case 37157: return {max_f_in};
          case 35076: return {min_texel};
          case 35077: return {max_texel};
          case 37155: return {max_tf};
          default: return origGetParameter.apply(this, arguments);
        }}
      }}, 'getParameter', origGetParameter);
    }};

    if (window.WebGLRenderingContext) patchWebGLParams(WebGLRenderingContext.prototype, false);
    if (window.WebGL2RenderingContext) patchWebGLParams(WebGL2RenderingContext.prototype, true);
  }} catch (_) {{}}
",
            max_tex = p.max_texture_size,
            max_cube = p.max_cube_map_texture_size,
            max_rb = p.max_renderbuffer_size,
            max_attribs = p.max_vertex_attribs,
            max_v_uniform = p.max_vertex_uniform_vectors,
            max_varying = p.max_varying_vectors,
            max_f_uniform = p.max_fragment_uniform_vectors,
            max_combined_tex = p.max_combined_texture_image_units,
            max_v_tex = p.max_vertex_texture_image_units,
            max_tex_units = p.max_texture_image_units,
            max_3d = p.max_3d_texture_size,
            max_array_layers = p.max_array_texture_layers,
            max_color_att = p.max_color_attachments,
            max_draw_buf = p.max_draw_buffers,
            max_ubo_bindings = p.max_uniform_buffer_bindings,
            max_ubo_size = p.max_uniform_block_size,
            ubo_align = p.uniform_buffer_offset_alignment,
            max_v_out = p.max_vertex_output_components,
            max_f_in = p.max_fragment_input_components,
            min_texel = p.min_program_texel_offset,
            max_texel = p.max_program_texel_offset,
            max_tf = p.max_transform_feedback_separate_components,
        );
    }

    fn push_webgl_extras(&self, out: &mut String) {
        let _ = writeln!(
            out,
            r"  // 6b. WebGL extensions, shader precision, and readPixels LSB jitter
  try {{
    const extensions = {extensions_json};
    const allowedLower = extensions.map((e) => e.toLowerCase());
    const glSeed = ({canvas_seed} >>> 0);
    const glJitterEnabled = {gl_jitter};

    const patchWebGLExtras = (proto) => {{
      if (!proto) return;
      if (extensions.length > 0) {{
        const origSupported = proto.getSupportedExtensions;
        proto.getSupportedExtensions = makeNative(function getSupportedExtensions() {{
          return extensions.slice();
        }}, 'getSupportedExtensions', origSupported);
        const origGetExtension = proto.getExtension;
        proto.getExtension = makeNative(function getExtension(name) {{
          if (allowedLower.indexOf(String(name).toLowerCase()) === -1) return null;
          return origGetExtension.apply(this, arguments);
        }}, 'getExtension', origGetExtension);
      }}

      const origPrecision = proto.getShaderPrecisionFormat;
      if (origPrecision) {{
        proto.getShaderPrecisionFormat = makeNative(function getShaderPrecisionFormat(shaderType, precisionType) {{
          const res = origPrecision.apply(this, arguments);
          if (res && res.precision !== undefined) {{
            return {{ rangeMin: res.rangeMin, rangeMax: res.rangeMax, precision: res.precision }};
          }}
          return {{ rangeMin: 127, rangeMax: 127, precision: 23 }};
        }}, 'getShaderPrecisionFormat', origPrecision);
      }}

      const origReadPixels = proto.readPixels;
      if (glJitterEnabled && origReadPixels) {{
        proto.readPixels = makeNative(function readPixels(x, y, w, h, format, type, pixels) {{
          origReadPixels.apply(this, arguments);
          if (pixels && (pixels instanceof Uint8Array || pixels instanceof Uint8ClampedArray)) {{
            const base = (Math.floor(Number(y)) || 0) * 65536 + (Math.floor(Number(x)) || 0);
            for (let i = 0; i < pixels.length; i += 4) {{
              if (hashWithSeed(glSeed, base + i) % 8 === 0) {{
                pixels[i] ^= 1;
              }}
            }}
          }}
        }}, 'readPixels', origReadPixels);
      }}
    }};

    if (window.WebGLRenderingContext) patchWebGLExtras(WebGLRenderingContext.prototype);
    if (window.WebGL2RenderingContext) patchWebGLExtras(WebGL2RenderingContext.prototype);
  }} catch (_) {{}}
",
            extensions_json = self.graphics.extensions_json,
            canvas_seed = self.canvas_seed & 0xffff_ffff,
            gl_jitter = self.canvas_density > 0,
        );
    }

    fn push_webgpu(&self, out: &mut String) {
        let g = &self.graphics;
        let vendor = js_str(&g.webgpu.vendor);
        let arch = js_str(&g.webgpu.architecture);
        let desc = js_str(&g.webgpu.description);
        let features_json = js_json(&g.webgpu.features, "[]");
        let limits_json = js_json(&g.webgpu.limits, "{}");
        let adapterless = self.os == OsType::Linux && g.webgpu.vendor != "intel";

        let _ = writeln!(
            out,
            r"  // 7. WebGPU GPUAdapter.prototype.info, GPUAdapterInfo, and limits
  try {{
    const webgpuLimits = {limits_json};
    const webgpuFeatures = new Set({features_json});
    const adapterless = {adapterless};
    const adapterInfo = {{
      vendor: {vendor},
      architecture: {arch},
      device: '',
      description: {desc}
    }};

    if (typeof GPUAdapterInfo !== 'undefined') {{
      defineGetter(GPUAdapterInfo.prototype, 'vendor', () => adapterInfo.vendor);
      defineGetter(GPUAdapterInfo.prototype, 'architecture', () => adapterInfo.architecture);
      defineGetter(GPUAdapterInfo.prototype, 'device', () => adapterInfo.device);
      defineGetter(GPUAdapterInfo.prototype, 'description', () => adapterInfo.description);
    }}

    if (typeof GPUAdapter !== 'undefined') {{
      defineGetter(GPUAdapter.prototype, 'info', () => adapterInfo);
      if (GPUAdapter.prototype.requestAdapterInfo) {{
        const origReqInfo = GPUAdapter.prototype.requestAdapterInfo;
        GPUAdapter.prototype.requestAdapterInfo = makeNative(function requestAdapterInfo() {{
          return Promise.resolve(adapterInfo);
        }}, 'requestAdapterInfo', origReqInfo);
      }}
    }}

    const gpuProto = typeof GPU !== 'undefined'
      ? GPU.prototype
      : (navigator.gpu ? Object.getPrototypeOf(navigator.gpu) : null);
    if (gpuProto && gpuProto.requestAdapter) {{
      const origRequestAdapter = gpuProto.requestAdapter;
      gpuProto.requestAdapter = makeNative(function requestAdapter() {{
        if (adapterless) return Promise.resolve(null);
        return origRequestAdapter.apply(this, arguments).then((realAdapter) => {{
          if (realAdapter) return realAdapter;
          const mockAdapter = typeof GPUAdapter !== 'undefined' ? Object.create(GPUAdapter.prototype) : {{}};
          Object.defineProperties(mockAdapter, {{
            features: {{ value: webgpuFeatures, enumerable: true, configurable: true }},
            limits: {{ value: webgpuLimits, enumerable: true, configurable: true }},
            isFallbackAdapter: {{ value: false, enumerable: true, configurable: true }},
            info: {{ value: adapterInfo, enumerable: true, configurable: true }},
            requestAdapterInfo: {{
              value: makeNative(() => Promise.resolve(adapterInfo), 'requestAdapterInfo'),
              configurable: true
            }}
          }});
          return mockAdapter;
        }});
      }}, 'requestAdapter', origRequestAdapter);
    }}
  }} catch (_) {{}}
"
        );
    }

    fn push_audio_context(&self, out: &mut String) {
        let a = &self.audio;
        let _ = writeln!(
            out,
            r"  // 8. Web Audio API context metadata (sampleRate, latency, compressor, maxChannelCount)
  try {{
    const patchAudioContext = (AudioCtx, patchSampleRate) => {{
      if (!AudioCtx || !AudioCtx.prototype) return;
      if (patchSampleRate) {{
        defineGetter(AudioCtx.prototype, 'sampleRate', () => {sample_rate});
      }}
      defineGetter(AudioCtx.prototype, 'baseLatency', () => {base_latency});
      defineGetter(AudioCtx.prototype, 'outputLatency', () => {output_latency});
      const origCreateCompressor = AudioCtx.prototype.createDynamicsCompressor;
      if (origCreateCompressor) {{
        AudioCtx.prototype.createDynamicsCompressor = makeNative(function createDynamicsCompressor() {{
          const node = origCreateCompressor.apply(this, arguments);
          try {{
            node.threshold.value = {comp_thresh};
            node.knee.value = {comp_knee};
            node.ratio.value = {comp_ratio};
            node.attack.value = {comp_attack};
            node.release.value = {comp_release};
          }} catch (_) {{}}
          return node;
        }}, 'createDynamicsCompressor', origCreateCompressor);
      }}
    }};

    if (window.AudioContext) patchAudioContext(AudioContext, true);
    if (window.webkitAudioContext) patchAudioContext(webkitAudioContext, true);
    if (window.OfflineAudioContext) patchAudioContext(OfflineAudioContext, false);
    if (typeof AudioDestinationNode !== 'undefined') {{
      defineGetter(AudioDestinationNode.prototype, 'maxChannelCount', () => {channels});
    }}
  }} catch (_) {{}}
",
            sample_rate = a.sample_rate,
            base_latency = js_num(a.base_latency),
            output_latency = js_num(a.output_latency),
            channels = a.max_channel_count,
            comp_thresh = js_num(f64::from(a.compressor.threshold)),
            comp_knee = js_num(f64::from(a.compressor.knee)),
            comp_ratio = js_num(f64::from(a.compressor.ratio)),
            comp_attack = js_num(f64::from(a.compressor.attack)),
            comp_release = js_num(f64::from(a.compressor.release)),
        );
    }

    fn push_audio_signal_noise(&self, out: &mut String) {
        if !self.audio_noise_enabled {
            return;
        }
        let _ = writeln!(
            out,
            r"  // 8b. Web Audio signal noise (AudioBuffer.getChannelData / copyFromChannel & AnalyserNode)
  try {{
    const audioSeed = ({audio_seed} >>> 0);
    const audioNoise = (index) => hashWithSeed(audioSeed, index) / 4294967296 - 0.5;

    if (window.OfflineAudioContext && OfflineAudioContext.prototype.startRendering) {{
      const origStartRendering = OfflineAudioContext.prototype.startRendering;
      OfflineAudioContext.prototype.startRendering = makeNative(function startRendering() {{
        return origStartRendering.apply(this, arguments).then((buffer) => {{
          if (buffer && buffer.getChannelData) buffer.getChannelData(0);
          return buffer;
        }});
      }}, 'startRendering', origStartRendering);
    }}

    if (window.AudioBuffer) {{
      const perturbedBuffers = new WeakSet();
      const origGetChannelData = AudioBuffer.prototype.getChannelData;
      const origCopyFromChannel = AudioBuffer.prototype.copyFromChannel;
      AudioBuffer.prototype.getChannelData = makeNative(function getChannelData(channel) {{
        const data = origGetChannelData.apply(this, arguments);
        if (!perturbedBuffers.has(data.buffer)) {{
          perturbedBuffers.add(data.buffer);
          const offset = (Number(channel) || 0) * 100003;
          for (let i = 0; i < data.length; i += 100) {{
            data[i] += audioNoise(i + offset) * 1e-7;
          }}
        }}
        return data;
      }}, 'getChannelData', origGetChannelData);

      if (origCopyFromChannel) {{
        AudioBuffer.prototype.copyFromChannel = makeNative(function copyFromChannel(destination, channel) {{
          this.getChannelData(channel);
          return origCopyFromChannel.apply(this, arguments);
        }}, 'copyFromChannel', origCopyFromChannel);
      }}
    }}

    if (window.AnalyserNode) {{
      const origFloatFreq = AnalyserNode.prototype.getFloatFrequencyData;
      if (origFloatFreq) {{
        AnalyserNode.prototype.getFloatFrequencyData = makeNative(function getFloatFrequencyData(array) {{
          origFloatFreq.apply(this, arguments);
          for (let i = 0; i < array.length; i++) array[i] += audioNoise(i) * 0.0002;
        }}, 'getFloatFrequencyData', origFloatFreq);
      }}
      const origByteFreq = AnalyserNode.prototype.getByteFrequencyData;
      if (origByteFreq) {{
        AnalyserNode.prototype.getByteFrequencyData = makeNative(function getByteFrequencyData(array) {{
          origByteFreq.apply(this, arguments);
          for (let i = 0; i < array.length; i += 4) {{
            if (array[i] > 0) {{
              const delta = (hashWithSeed(audioSeed, i + 1009) % 3) - 1;
              array[i] = Math.max(0, Math.min(255, array[i] + delta));
            }}
          }}
        }}, 'getByteFrequencyData', origByteFreq);
      }}
      const origFloatTime = AnalyserNode.prototype.getFloatTimeDomainData;
      if (origFloatTime) {{
        AnalyserNode.prototype.getFloatTimeDomainData = makeNative(function getFloatTimeDomainData(array) {{
          origFloatTime.apply(this, arguments);
          for (let i = 0; i < array.length; i += 4) array[i] += audioNoise(i + 3001) * 1e-6;
        }}, 'getFloatTimeDomainData', origFloatTime);
      }}
      const origByteTime = AnalyserNode.prototype.getByteTimeDomainData;
      if (origByteTime) {{
        AnalyserNode.prototype.getByteTimeDomainData = makeNative(function getByteTimeDomainData(array) {{
          origByteTime.apply(this, arguments);
          for (let i = 0; i < array.length; i += 4) {{
            const delta = (hashWithSeed(audioSeed, i + 5003) % 3) - 1;
            array[i] = Math.max(0, Math.min(255, array[i] + delta));
          }}
        }}, 'getByteTimeDomainData', origByteTime);
      }}
    }}
  }} catch (_) {{}}
",
            audio_seed = self.audio_seed & 0xffff_ffff,
        );
    }

    fn push_devices_battery_network(&self, out: &mut String) {
        let _ = writeln!(
            out,
            r"  // 9. Media Devices (prototype-level)
  try {{
    if (typeof MediaDevices !== 'undefined' && MediaDevices.prototype.enumerateDevices) {{
      const syntheticDevices = {media_devices};
      const origEnum = MediaDevices.prototype.enumerateDevices;
      MediaDevices.prototype.enumerateDevices = makeNative(function enumerateDevices() {{
        return Promise.resolve(syntheticDevices.map((d) => ({{
          deviceId: d.device_id,
          kind: d.kind,
          label: d.label,
          groupId: d.group_id,
          toJSON: function() {{
            return {{
              deviceId: this.deviceId,
              kind: this.kind,
              label: this.label,
              groupId: this.groupId
            }};
          }}
        }})));
      }}, 'enumerateDevices', origEnum);
    }}
  }} catch (_) {{}}
",
            media_devices = self.media_devices_json,
        );

        if let Some(bat) = self.battery {
            let ch_time = js_num(bat.charging_time);
            let dis_time = js_num(bat.discharging_time);
            let level = js_num(bat.level);
            let _ = writeln!(
                out,
                r"  // 10. Battery Status
  try {{
    const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : Object.getPrototypeOf(navigator);
    const origBattery = navProto && navProto.getBattery;
    if (origBattery) {{
      const batteryInfo = {{
        charging: {charging},
        chargingTime: {ch_time},
        dischargingTime: {dis_time},
        level: {level},
        addEventListener: () => {{}},
        removeEventListener: () => {{}},
        dispatchEvent: () => false,
        onchargingchange: null,
        onchargingtimechange: null,
        ondischargingtimechange: null,
        onlevelchange: null
      }};
      navProto.getBattery = makeNative(function getBattery() {{
        return Promise.resolve(batteryInfo);
      }}, 'getBattery', origBattery);
    }}
  }} catch (_) {{}}
",
                charging = bat.charging,
            );
        } else {
            out.push_str(
                "  // 10. Battery API removed for persona\n  try { delete Object.getPrototypeOf(navigator).getBattery; } catch (_) {}\n\n",
            );
        }

        if let Some(net) = &self.network {
            let net_type = js_str(&net.effective_type);
            let downlink = js_num(net.downlink);
            let _ = writeln!(
                out,
                r"  // 11. Network Information (NetworkInformation.prototype)
  try {{
    if (typeof NetworkInformation !== 'undefined') {{
      const netProto = NetworkInformation.prototype;
      if ('downlink' in netProto) defineGetter(netProto, 'downlink', () => {downlink});
      if ('effectiveType' in netProto) defineGetter(netProto, 'effectiveType', () => {net_type});
      if ('rtt' in netProto) defineGetter(netProto, 'rtt', () => {rtt});
      if ('saveData' in netProto) defineGetter(netProto, 'saveData', () => {save_data});
    }}
  }} catch (_) {{}}
",
                rtt = net.rtt,
                save_data = net.save_data,
            );
        } else {
            out.push_str(
                "  // 11. Network Information removed for persona\n  try { delete Object.getPrototypeOf(navigator).connection; } catch (_) {}\n\n",
            );
        }
    }

    fn push_chrome_and_permissions(&self, out: &mut String) {
        if self.browser == BrowserType::Chromium {
            out.push_str(
                r"  // 12. Window Chrome & Notification/Permissions alignment
  try {
    if (!window.chrome) {
      const startLoad = Date.now() / 1000;
      window.chrome = {
        app: {
          isInstalled: false,
          InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
          RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' },
          getDetails: makeNative(() => null, 'getDetails'),
          getIsInstalled: makeNative(() => false, 'getIsInstalled'),
          installState: makeNative(() => Promise.resolve('not_installed'), 'installState'),
          runningState: makeNative(() => 'cannot_run', 'runningState')
        },
        csi: makeNative(() => ({
          startE: Date.now(),
          onloadT: Date.now(),
          pageT: 1000,
          tran: 15
        }), 'csi'),
        loadTimes: makeNative(() => ({
          requestTime: startLoad - 0.5,
          startLoadTime: startLoad - 0.45,
          commitLoadTime: startLoad - 0.4,
          finishDocumentLoadTime: startLoad - 0.2,
          firstPaintTime: startLoad - 0.35,
          finishLoadTime: startLoad - 0.1,
          wasFetchedViaSpdy: true,
          wasNpnNegotiated: true,
          npnNegotiatedProtocol: 'h2',
          wasAlternateProtocolAvailable: false,
          connectionInfo: 'h2'
        }), 'loadTimes'),
        runtime: {
          id: undefined,
          lastError: undefined,
          OnInstalledReason: { CHROME_UPDATE: 'chrome_update', INSTALL: 'install', SHARED_MODULE_UPDATE: 'shared_module_update', UPDATE: 'update' },
          OnRestartRequiredReason: { APP_UPDATE: 'app_update', OS_UPDATE: 'os_update', PERIODIC: 'periodic' },
          PlatformArch: { ARM: 'arm', ARM64: 'arm64', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' },
          PlatformNaclArch: { ARM: 'arm', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' },
          PlatformOs: { ANDROID: 'android', CROS: 'cros', LINUX: 'linux', MAC: 'mac', OPENBSD: 'openbsd', WIN: 'win' },
          RequestUpdateCheckStatus: { NO_UPDATE: 'no_update', THROTTLED: 'throttled', UPDATE_AVAILABLE: 'update_available' },
          connect: makeNative(() => ({
            postMessage: () => {},
            disconnect: () => {},
            onDisconnect: { addListener: () => {} },
            onMessage: { addListener: () => {} }
          }), 'connect'),
          sendMessage: makeNative(() => Promise.resolve({}), 'sendMessage'),
          getURL: makeNative((p) => 'chrome-extension://' + (p || ''), 'getURL'),
          getManifest: makeNative(() => ({}), 'getManifest')
        }
      };
    }
  } catch (_) {}
",
            );
        }

        out.push_str(
            r"
  try {
    const states = { granted: 'granted', denied: 'denied', 'default': 'prompt' };
    if (typeof Notification !== 'undefined' && Notification.permission === 'denied') {
      defineGetter(Notification, 'permission', () => 'default');
    }
    if (typeof Permissions !== 'undefined' && Permissions.prototype.query) {
      const origQuery = Permissions.prototype.query;
      Permissions.prototype.query = makeNative(function query(descriptor) {
        const result = origQuery.apply(this, arguments);
        if (!(descriptor && descriptor.name === 'notifications' && typeof Notification !== 'undefined')) {
          return result;
        }
        return result.then((status) => {
          try {
            Object.defineProperty(status, 'state', {
              value: states[Notification.permission] || status.state,
              enumerable: true,
              configurable: true
            });
          } catch (_) {}
          return status;
        });
      }, 'query', origQuery);
    }
  } catch (_) {}
",
        );
    }

    fn push_navigator_extras(&self, out: &mut String) {
        let dnt = self
            .do_not_track
            .as_deref()
            .map_or_else(|| "null".to_owned(), js_str);
        let pdf = self.pdf_viewer_enabled.unwrap_or(true);
        let _ = writeln!(
            out,
            r"  // 13. Navigator Extras (doNotTrack, pdfViewerEnabled)
  try {{
    const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : Object.getPrototypeOf(navigator);
    if ({has_dnt}) defineGetter(navProto, 'doNotTrack', () => {dnt});
    defineGetter(navProto, 'pdfViewerEnabled', () => {pdf});
  }} catch (_) {{}}
",
            has_dnt = self.do_not_track.is_some(),
        );
    }

    fn push_speech_voices(&self, out: &mut String) {
        let _ = writeln!(
            out,
            r"  // 14. SpeechSynthesis Voices
  try {{
    const syntheticVoices = {voices};
    if (syntheticVoices.length && typeof window.speechSynthesis !== 'undefined') {{
      const synthProto = Object.getPrototypeOf(window.speechSynthesis);
      const origGetVoices = synthProto && synthProto.getVoices;
      if (origGetVoices) {{
        const voiceObjects = syntheticVoices.map((v) => Object.freeze({{
          name: v.name,
          lang: v.lang,
          voiceURI: v.voice_uri,
          localService: !!v.local_service,
          default: !!v.default
        }}));
        synthProto.getVoices = makeNative(function getVoices() {{
          return voiceObjects.slice();
        }}, 'getVoices', origGetVoices);
      }}
    }}
  }} catch (_) {{}}
",
            voices = self.speech_voices_json,
        );
    }

    fn push_css_media_queries(&self, out: &mut String) {
        let scheme = self
            .color_scheme
            .map_or_else(|| "null".to_owned(), |s| js_str(s.as_str()));
        let motion = self
            .reduced_motion
            .map_or_else(|| "null".to_owned(), |m| js_str(m.as_str()));
        let gamut = self
            .color_gamut
            .as_deref()
            .map_or_else(|| "null".to_owned(), js_str);
        let hdr = self.hdr;
        let _ = writeln!(
            out,
            r"  // 15. CSS Media Queries (matchMedia overrides)
  try {{
    const targetScheme = {scheme};
    const targetMotion = {motion};
    const targetGamut = {gamut};
    const targetHdr = {hdr};
    if (typeof window.matchMedia === 'function') {{
      const origMatchMedia = window.matchMedia;
      window.matchMedia = makeNative(function matchMedia(query) {{
        const mql = origMatchMedia.apply(this, arguments);
        const q = String(query || '').toLowerCase();
        let override = null;
        if (targetScheme && q.includes('prefers-color-scheme')) {{
          override = q.includes(targetScheme);
        }} else if (targetMotion && q.includes('prefers-reduced-motion')) {{
          override = q.includes(targetMotion);
        }} else if (targetGamut && q.includes('color-gamut')) {{
          override = q.includes(targetGamut) || (targetGamut === 'rec2020' && (q.includes('p3') || q.includes('srgb'))) || (targetGamut === 'p3' && q.includes('srgb'));
        }} else if (q.includes('dynamic-range')) {{
          override = q.includes('high') ? targetHdr : true;
        }}
        if (override !== null) {{
          try {{
            Object.defineProperty(mql, 'matches', {{ get: () => override, configurable: true }});
          }} catch (_) {{}}
        }}
        return mql;
      }}, 'matchMedia', origMatchMedia);
    }}
  }} catch (_) {{}}
",
        );
    }

    fn push_performance_jitter(&self, out: &mut String) {
        if self.performance_jitter_ms <= 0.0 {
            return;
        }
        let jitter = js_num(self.performance_jitter_ms);
        let seed = self.dom_seed & 0xffff_ffff;
        let _ = writeln!(
            out,
            r"  // 16. Performance.now() deterministic sub-millisecond timing jitter
  try {{
    if (typeof Performance !== 'undefined' && Performance.prototype.now) {{
      const origNow = Performance.prototype.now;
      let perfStep = 0;
      let lastReturned = 0;
      Performance.prototype.now = makeNative(function now() {{
        const raw = origNow.apply(this, arguments);
        const h = hashWithSeed({seed}, ++perfStep) / 4294967295;
        const val = Math.max(lastReturned, raw + h * {jitter});
        lastReturned = val;
        return val;
      }}, 'now', origNow);
    }}
  }} catch (_) {{}}
",
        );
    }

    fn push_worker_spoofing(&self, out: &mut String) {
        let ua = self
            .user_agent
            .as_deref()
            .map_or_else(|| "null".to_owned(), js_str);
        let platform = js_str(&self.platform);
        let hints_json = self
            .client_hints
            .as_ref()
            .map_or_else(|| "null".to_owned(), |h| js_json(h, "null"));
        let _ = writeln!(
            out,
            r#"  // 17. Worker & SharedWorker Navigator Consistency
  try {{
    const wCfg = {{
      ua: {ua},
      platform: {platform},
      languages: {langs},
      hw: {hw},
      mem: {mem},
      hints: {hints_json}
    }};
    const prelude = '(function(c){{try{{const np=Object.getPrototypeOf(self.navigator);const dg=(k,v)=>Object.defineProperty(np,k,{{get:()=>v,configurable:true}});if(c.ua)dg("userAgent",c.ua);dg("platform",c.platform);dg("languages",Object.freeze(c.languages));dg("language",c.languages[0]||"en-US");dg("hardwareConcurrency",c.hw);if(c.mem!==null)dg("deviceMemory",c.mem);dg("webdriver",false);if(c.hints&&self.navigator.userAgentData){{const uad=self.navigator.userAgentData;const orig=uad.getHighEntropyValues;if(orig)uad.getHighEntropyValues=function(h){{return orig.call(this,h).then((r)=>Object.assign({{}},r,{{architecture:c.hints.architecture,bitness:c.hints.bitness,model:c.hints.model,platform:c.hints.platform,platformVersion:c.hints.platform_version,uaFullVersion:c.hints.ua_full_version,fullVersionList:c.hints.full_version_list}}));}};}}}}catch(_){{}}}})(' + JSON.stringify(wCfg) + ');\n';
    const wrapWorkerCtor = (Ctor, name) => {{
      if (typeof Ctor !== 'function') return Ctor;
      return new Proxy(Ctor, {{
        construct(target, args, newTarget) {{
          try {{
            const urlArg = args[0];
            const opts = args[1];
            const isModule = opts && opts.type === 'module';
            if (!isModule && typeof urlArg === 'string' && !urlArg.startsWith('data:')) {{
              const abs = new URL(urlArg, location.href).href;
              const code = prelude + 'importScripts(' + JSON.stringify(abs) + ');';
              const blobUrl = URL.createObjectURL(new Blob([code], {{ type: 'application/javascript' }}));
              const patchedArgs = [blobUrl].concat(args.slice(1));
              return Reflect.construct(target, patchedArgs, newTarget);
            }}
          }} catch (_) {{}}
          return Reflect.construct(target, args, newTarget);
        }}
      }});
    }};
    if (typeof window.Worker === 'function') window.Worker = wrapWorkerCtor(window.Worker, 'Worker');
    if (typeof window.SharedWorker === 'function') window.SharedWorker = wrapWorkerCtor(window.SharedWorker, 'SharedWorker');
  }} catch (_) {{}}
"#,
            langs = self.languages_json,
            hw = self.hw_concurrency,
            mem = self.device_memory.map_or_else(|| "null".to_owned(), js_num),
        );
    }

    fn push_intl_timezone(&self, out: &mut String) {
        let tz = self
            .timezone
            .as_deref()
            .map_or_else(|| "null".to_owned(), js_str);
        let locale = js_str(&self.primary_lang);
        let _ = writeln!(
            out,
            r"  // 18. Intl Timezone & Locale Consistency
  try {{
    const targetTz = {tz};
    const targetLocale = {locale};
    if (typeof Intl !== 'undefined' && Intl.DateTimeFormat && Intl.DateTimeFormat.prototype.resolvedOptions) {{
      const origResolved = Intl.DateTimeFormat.prototype.resolvedOptions;
      Intl.DateTimeFormat.prototype.resolvedOptions = makeNative(function resolvedOptions() {{
        const res = origResolved.apply(this, arguments);
        if (targetTz && res && typeof res === 'object') res.timeZone = targetTz;
        if (targetLocale && res && !res.locale) res.locale = targetLocale;
        return res;
      }}, 'resolvedOptions', origResolved);
    }}
  }} catch (_) {{}}
",
        );
    }

    fn push_webrtc_guard(&self, out: &mut String) {
        use seleniumbase_rs::WebRtcPolicy;
        let Some(policy) = self.webrtc_policy else {
            return;
        };
        if matches!(policy, WebRtcPolicy::PublicAndPrivateInterfaces) {
            return;
        }
        let block_all = false;
        let _ = writeln!(
            out,
            r"  // 19. WebRTC Local ICE Candidate Guard
  try {{
    const blockAllRtc = {block_all};
    const OrigRtc = window.RTCPeerConnection || window.webkitRTCPeerConnection;
    if (typeof OrigRtc === 'function') {{
      const stripHostLines = (sdp) => typeof sdp === 'string' ? sdp.split('\r\n').filter((l) => !(l.startsWith('a=candidate:') && l.includes(' typ host'))).join('\r\n') : sdp;
      const WrappedRtc = new Proxy(OrigRtc, {{
        construct(target, args, newTarget) {{
          if (blockAllRtc) {{
            throw new DOMException('RTCPeerConnection is disabled by policy', 'NotAllowedError');
          }}
          const pc = Reflect.construct(target, args, newTarget);
          const origAdd = pc.addEventListener.bind(pc);
          pc.addEventListener = makeNative(function addEventListener(type, listener, opts) {{
            if (type === 'icecandidate' && typeof listener === 'function') {{
              const filtered = function(ev) {{
                if (ev && ev.candidate && typeof ev.candidate.candidate === 'string' && ev.candidate.candidate.includes(' typ host')) return;
                return listener.call(this, ev);
              }};
              return origAdd(type, filtered, opts);
            }}
            return origAdd(type, listener, opts);
          }}, 'addEventListener', pc.addEventListener);
          const desc = Object.getOwnPropertyDescriptor(OrigRtc.prototype, 'localDescription');
          if (desc && desc.get) {{
            Object.defineProperty(pc, 'localDescription', {{
              get() {{
                const ld = desc.get.call(this);
                if (!ld || !ld.sdp) return ld;
                return {{ type: ld.type, sdp: stripHostLines(ld.sdp) }};
              }},
              configurable: true
            }});
          }}
          return pc;
        }}
      }});
      window.RTCPeerConnection = WrappedRtc;
      if ('webkitRTCPeerConnection' in window) window.webkitRTCPeerConnection = WrappedRtc;
    }}
  }} catch (_) {{}}
",
        );
    }

    fn push_svg_noise(&self, out: &mut String) {
        let seed = self.dom_seed & 0xffff_ffff;
        let _ = writeln!(
            out,
            r"  // 20. SVG Text & BBox Sub-pixel Noise
  try {{
    if (typeof SVGTextContentElement !== 'undefined' && SVGTextContentElement.prototype.getComputedTextLength) {{
      const origLen = SVGTextContentElement.prototype.getComputedTextLength;
      SVGTextContentElement.prototype.getComputedTextLength = makeNative(function getComputedTextLength() {{
        const raw = origLen.apply(this, arguments);
        const delta = ((hashWithSeed({seed}, Math.round(raw * 100) + 17) / 4294967295) - 0.5) * 0.002;
        return raw + delta;
      }}, 'getComputedTextLength', origLen);
    }}
    if (typeof SVGGraphicsElement !== 'undefined' && SVGGraphicsElement.prototype.getBBox) {{
      const origBBox = SVGGraphicsElement.prototype.getBBox;
      SVGGraphicsElement.prototype.getBBox = makeNative(function getBBox() {{
        const box = origBBox.apply(this, arguments);
        if (box && typeof box.width === 'number') {{
          const delta = ((hashWithSeed({seed}, Math.round(box.width * 100) + 31) / 4294967295) - 0.5) * 0.002;
          try {{
            Object.defineProperty(box, 'width', {{ value: box.width + delta, configurable: true }});
          }} catch (_) {{}}
        }}
        return box;
      }}, 'getBBox', origBBox);
    }}
  }} catch (_) {{}}
",
        );
    }

    fn push_window_geometry(&self, out: &mut String) {
        let chrome_h = self.outer_chrome_height;
        let _ = writeln!(
            out,
            r"  // 21. Window Outer/Inner Geometry Coherence
  try {{
    const chromeHeight = {chrome_h};
    if (window.outerWidth === 0 || window.outerWidth < window.innerWidth) {{
      defineGetter(window, 'outerWidth', () => Math.max(window.innerWidth, {sw}));
    }}
    if (window.outerHeight === 0 || window.outerHeight <= window.innerHeight) {{
      defineGetter(window, 'outerHeight', () => Math.min({sh}, window.innerHeight + chromeHeight));
    }}
  }} catch (_) {{}}
",
            sw = self.display.screen_w,
            sh = self.display.screen_h,
        );
    }

    fn push_storage_quota(&self, out: &mut String) {
        let quota_bytes: u64 = self.storage_quota_gb.map_or_else(
            || {
                if self.os.is_mobile() {
                    64_424_509_440 // 60 GiB
                } else {
                    274_877_906_944 // 256 GiB
                }
            },
            |gb| gb.saturating_mul(1_073_741_824),
        );
        let _ = writeln!(
            out,
            r"  // 22. StorageManager Quota (non-incognito realistic capacity)
  try {{
    if (typeof StorageManager !== 'undefined' && StorageManager.prototype.estimate) {{
      const origEstimate = StorageManager.prototype.estimate;
      StorageManager.prototype.estimate = makeNative(function estimate() {{
        return origEstimate.apply(this, arguments).then((res) => {{
          if (!res || typeof res.quota !== 'number' || res.quota < 2147483648) {{
            return Object.assign({{}}, res || {{}}, {{ quota: {quota_bytes} }});
          }}
          return res;
        }});
      }}, 'estimate', origEstimate);
    }}
  }} catch (_) {{}}
",
        );
    }

    fn push_hardware_apis(out: &mut String) {
        out.push_str(
            r"  // 23. Hardware APIs (getGamepads desktop Chromium alignment)
  try {
    const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : Object.getPrototypeOf(navigator);
    if (navProto && navProto.getGamepads) {
      const origGamepads = navProto.getGamepads;
      navProto.getGamepads = makeNative(function getGamepads() {
        const pads = origGamepads.apply(this, arguments);
        if (!pads || pads.length === 0) return [null, null, null, null];
        return pads;
      }, 'getGamepads', origGamepads);
    }
  } catch (_) {}
",
        );
    }

    fn push_visibility_guard(out: &mut String) {
        out.push_str(
            r"  // 24. Document Visibility Guard (prevent background-tab bot flags)
  try {
    const docProto = typeof Document !== 'undefined' ? Document.prototype : Object.getPrototypeOf(document);
    if (docProto) {
      defineGetter(docProto, 'visibilityState', () => 'visible');
      defineGetter(docProto, 'hidden', () => false);
    }
  } catch (_) {}
",
        );
    }

    fn push_console_debug_guard(out: &mut String) {
        out.push_str(
            r"  // 25. Console Runtime.stack.access Canary Guard
  try {
    const hasGetterTrap = (arg) => {
      if (!arg || (typeof arg !== 'object' && typeof arg !== 'function')) return false;
      const desc = Object.getOwnPropertyDescriptor(arg, 'stack');
      if (desc && typeof desc.get === 'function') return true;
      const toStr = Object.getOwnPropertyDescriptor(arg, 'toString');
      return !!(toStr && typeof toStr.get === 'function');
    };
    for (const method of ['debug', 'log', 'dir', 'table']) {
      const orig = console && console[method];
      if (typeof orig === 'function') {
        console[method] = makeNative(function() {
          for (let i = 0; i < arguments.length; i++) {
            if (hasGetterTrap(arguments[i])) return undefined;
          }
          return orig.apply(this, arguments);
        }, method, orig);
      }
    }
  } catch (_) {}
",
        );
    }

    fn push_mouse_coord_guard(&self, out: &mut String) {
        let chrome_h = self.outer_chrome_height;
        let _ = writeln!(
            out,
            r"  // 26. MouseEvent screenX/screenY Coordinate Alignment
  try {{
    if (typeof MouseEvent !== 'undefined') {{
      const sxDesc = Object.getOwnPropertyDescriptor(MouseEvent.prototype, 'screenX');
      const syDesc = Object.getOwnPropertyDescriptor(MouseEvent.prototype, 'screenY');
      const cxDesc = Object.getOwnPropertyDescriptor(MouseEvent.prototype, 'clientX');
      const cyDesc = Object.getOwnPropertyDescriptor(MouseEvent.prototype, 'clientY');
      if (sxDesc && sxDesc.get && syDesc && syDesc.get && cxDesc && cxDesc.get && cyDesc && cyDesc.get) {{
        Object.defineProperty(MouseEvent.prototype, 'screenX', {{
          get: makeNative(function screenX() {{
            const sx = sxDesc.get.call(this);
            const cx = cxDesc.get.call(this);
            const sy = syDesc.get.call(this);
            const cy = cyDesc.get.call(this);
            if (sx === cx && sy === cy) return cx + (window.screenX || 0);
            return sx;
          }}, 'get screenX', sxDesc.get),
          configurable: true
        }});
        Object.defineProperty(MouseEvent.prototype, 'screenY', {{
          get: makeNative(function screenY() {{
            const sx = sxDesc.get.call(this);
            const cx = cxDesc.get.call(this);
            const sy = syDesc.get.call(this);
            const cy = cyDesc.get.call(this);
            if (sx === cx && sy === cy) return cy + (window.screenY || 0) + {chrome_h};
            return sy;
          }}, 'get screenY', syDesc.get),
          configurable: true
        }});
      }}
    }}
  }} catch (_) {{}}
",
        );
    }

    fn push_codec_support(out: &mut String) {
        out.push_str(
            r"  // 27. Media Codec Support Alignment (H.264, AAC, AV1, WebM, Opus)
  try {
    const probable = ['avc1', 'mp4a', 'vp8', 'vp9', 'av01', 'opus', 'vorbis', 'theora', 'mp3', 'mpeg', 'aac'];
    if (typeof HTMLMediaElement !== 'undefined' && HTMLMediaElement.prototype.canPlayType) {
      const origCanPlay = HTMLMediaElement.prototype.canPlayType;
      HTMLMediaElement.prototype.canPlayType = makeNative(function canPlayType(type) {
        const res = origCanPlay.apply(this, arguments);
        if (res) return res;
        const lower = String(type || '').toLowerCase();
        if (probable.some((c) => lower.includes(c))) return 'probably';
        if (lower === 'video/mp4' || lower === 'audio/mp4' || lower === 'video/webm' || lower === 'audio/webm') return 'maybe';
        return res;
      }, 'canPlayType', origCanPlay);
    }
    if (typeof MediaSource !== 'undefined' && MediaSource.isTypeSupported) {
      const origSupported = MediaSource.isTypeSupported;
      MediaSource.isTypeSupported = makeNative(function isTypeSupported(type) {
        if (origSupported.apply(this, arguments)) return true;
        const lower = String(type || '').toLowerCase();
        return probable.some((c) => lower.includes(c));
      }, 'isTypeSupported', origSupported);
    }
  } catch (_) {}
",
        );
    }

    fn push_timezone_offset(&self, out: &mut String) {
        let Some(tz) = self.timezone.as_deref() else {
            return;
        };
        let tz_js = js_str(tz);
        let _ = writeln!(
            out,
            r"  // 28. DST-aware Date.prototype.getTimezoneOffset() alignment
  try {{
    const targetTz = {tz_js};
    const origGetOffset = Date.prototype.getTimezoneOffset;
    const fmt = new Intl.DateTimeFormat('en-US', {{
      timeZone: targetTz,
      year: 'numeric', month: '2-digit', day: '2-digit',
      hour: '2-digit', minute: '2-digit', second: '2-digit',
      hour12: false
    }});
    Date.prototype.getTimezoneOffset = makeNative(function getTimezoneOffset() {{
      try {{
        const ms = this.getTime();
        if (!Number.isFinite(ms)) return origGetOffset.call(this);
        const parts = {{}};
        for (const p of fmt.formatToParts(this)) parts[p.type] = p.value;
        const asUtc = Date.UTC(
          Number(parts.year), Number(parts.month) - 1, Number(parts.day),
          Number(parts.hour) % 24, Number(parts.minute), Number(parts.second)
        );
        return Math.round((ms - asUtc) / 60000);
      }} catch (_) {{
        return origGetOffset.call(this);
      }}
    }}, 'getTimezoneOffset', origGetOffset);
  }} catch (_) {{}}
",
        );
    }

    fn push_iframe_srcdoc_guard(out: &mut String) {
        out.push_str(
            r"  // 29. Same-origin / about:blank / srcdoc Iframe Sandbox Propagation
  try {
    const patchFrameWin = (win) => {
      if (!win) return;
      try {
        const nav = win.navigator;
        if (nav) {
          const proto = Object.getPrototypeOf(nav);
          Object.defineProperty(proto, 'webdriver', { get: () => false, configurable: true });
        }
        if (win.Function && win.Function.prototype) {
          win.Function.prototype.toString = patchedToString;
        }
      } catch (_) {}
    };
    const desc = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, 'contentWindow');
    if (desc && desc.get) {
      Object.defineProperty(HTMLIFrameElement.prototype, 'contentWindow', {
        get: makeNative(function contentWindow() {
          const w = desc.get.call(this);
          patchFrameWin(w);
          return w;
        }, 'get contentWindow', desc.get),
        configurable: true
      });
    }
  } catch (_) {}
",
        );
    }

    fn push_history_length_guard(&self, out: &mut String) {
        let min_len = self.history_min_length.max(1);
        let _ = writeln!(
            out,
            r"  // 30. History.length Fresh-Tab Masking
  try {{
    if (typeof History !== 'undefined') {{
      const lenDesc = Object.getOwnPropertyDescriptor(History.prototype, 'length');
      if (lenDesc && lenDesc.get) {{
        Object.defineProperty(History.prototype, 'length', {{
          get: makeNative(function length() {{
            const raw = lenDesc.get.call(this);
            return raw < {min_len} ? {min_len} : raw;
          }}, 'get length', lenDesc.get),
          configurable: true
        }});
      }}
    }}
  }} catch (_) {{}}
",
        );
    }

    fn push_canvas_toblob_guard(out: &mut String) {
        out.push_str(
            r"  // 31. Canvas toBlob & OffscreenCanvas convertToBlob Noise Consistency
  try {
    if (typeof HTMLCanvasElement !== 'undefined' && HTMLCanvasElement.prototype.toBlob) {
      const origToBlob = HTMLCanvasElement.prototype.toBlob;
      HTMLCanvasElement.prototype.toBlob = makeNative(function toBlob(callback, type, quality) {
        try {
          if (typeof this.toDataURL === 'function' && typeof callback === 'function') {
            const dataUrl = this.toDataURL(type, quality);
            const parts = dataUrl.split(',');
            if (parts.length === 2) {
              const mimeMatch = parts[0].match(/:(.*?);/);
              const mime = mimeMatch ? mimeMatch[1] : (type || 'image/png');
              const bin = atob(parts[1]);
              const bytes = new Uint8Array(bin.length);
              for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
              callback(new Blob([bytes], { type: mime }));
              return;
            }
          }
        } catch (_) {}
        return origToBlob.apply(this, arguments);
      }, 'toBlob', origToBlob);
    }
  } catch (_) {}
",
        );
    }

    fn push_webgl_shader_precision(out: &mut String) {
        out.push_str(
            r"  // 32. WebGL 1/2 Shader Precision Format Alignment (anti-SwiftShader)
  try {
    const patchPrecision = (Ctor) => {
      if (typeof Ctor === 'undefined' || !Ctor.prototype || !Ctor.prototype.getShaderPrecisionFormat) return;
      const origPrec = Ctor.prototype.getShaderPrecisionFormat;
      Ctor.prototype.getShaderPrecisionFormat = makeNative(function getShaderPrecisionFormat(shaderType, precisionType) {
        const res = origPrec.apply(this, arguments);
        if (res && precisionType === 0x8DF2 && (res.precision === 0 || res.rangeMax < 127)) {
          return Object.freeze({ rangeMin: 127, rangeMax: 127, precision: 23 });
        }
        return res;
      }, 'getShaderPrecisionFormat', origPrec);
    };
    patchPrecision(window.WebGLRenderingContext);
    patchPrecision(window.WebGL2RenderingContext);
  } catch (_) {}
",
        );
    }

    fn push_media_capabilities(out: &mut String) {
        out.push_str(
            r"  // 33. MediaCapabilities Hardware Decoding Alignment
  try {
    if (typeof MediaCapabilities !== 'undefined' && MediaCapabilities.prototype.decodingInfo) {
      const origDecode = MediaCapabilities.prototype.decodingInfo;
      MediaCapabilities.prototype.decodingInfo = makeNative(function decodingInfo(config) {
        return origDecode.apply(this, arguments).then((info) => {
          if (info && info.supported && (!info.smooth || !info.powerEfficient)) {
            return Object.assign({}, info, { smooth: true, powerEfficient: true });
          }
          return info;
        });
      }, 'decodingInfo', origDecode);
    }
  } catch (_) {}
",
        );
    }

    fn push_keyboard_layout(out: &mut String) {
        out.push_str(
            r"  // 34. Navigator.keyboard QWERTY Layout Map Consistency
  try {
    if (typeof Keyboard !== 'undefined' && Keyboard.prototype.getLayoutMap) {
      const origLayout = Keyboard.prototype.getLayoutMap;
      Keyboard.prototype.getLayoutMap = makeNative(function getLayoutMap() {
        return origLayout.apply(this, arguments).then((map) => {
          if (map && map.size > 0) return map;
          return new Map([
            ['KeyQ','q'],['KeyW','w'],['KeyE','e'],['KeyR','r'],['KeyT','t'],['KeyY','y'],
            ['KeyU','u'],['KeyI','i'],['KeyO','o'],['KeyP','p'],['KeyA','a'],['KeyS','s'],
            ['KeyD','d'],['KeyF','f'],['KeyG','g'],['KeyH','h'],['KeyJ','j'],['KeyK','k'],
            ['KeyL','l'],['KeyZ','z'],['KeyX','x'],['KeyC','c'],['KeyV','v'],['KeyB','b'],
            ['KeyN','n'],['KeyM','m'],['Digit1','1'],['Digit2','2'],['Digit3','3'],['Digit4','4'],
            ['Digit5','5'],['Digit6','6'],['Digit7','7'],['Digit8','8'],['Digit9','9'],['Digit0','0']
          ]);
        });
      }, 'getLayoutMap', origLayout);
    }
  } catch (_) {}
",
        );
    }

    fn push_clipboard_guard(out: &mut String) {
        out.push_str(
            r"  // 35. Navigator.clipboard Presence Guard
  try {
    const navProto = typeof Navigator !== 'undefined' ? Navigator.prototype : Object.getPrototypeOf(navigator);
    if (navProto && !('clipboard' in navigator)) {
      const fakeClipboard = {
        readText: makeNative(() => Promise.resolve(''), 'readText'),
        writeText: makeNative(() => Promise.resolve(), 'writeText')
      };
      defineGetter(navProto, 'clipboard', () => fakeClipboard);
    }
  } catch (_) {}
",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::FingerprintGenerator;

    #[test]
    fn bundle_generates_native_webdriver_and_tostring_wrapper() {
        let fp = Fingerprint::randomized(OsType::Windows, 42);
        let bundle = StealthScriptBundle::new(&fp);
        let script = bundle.script();

        assert!(script.contains("Function.prototype"));
        assert!(script.contains("[native code]"));
        assert!(script.contains("new WeakMap()"));
        assert!(script.contains("defineGetter(navProto, 'webdriver', () => false)"));
        assert!(!script.contains("defineGetter(navProto, 'webdriver', () => undefined)"));
        assert!(script.contains("window.chrome"));
    }

    #[test]
    fn bundle_generates_native_grade_dom_canvas_audio_webgl2_webgpu_and_stack_patches() {
        let generator = FingerprintGenerator::new(88).os(OsType::Macos).major(157);
        let generated = generator.generate();
        let bundle = StealthScriptBundle::from_generated(&generated);
        let script = bundle.script();

        // Real seeded GPU, not hardcoded Apple M2
        assert!(script.contains(&generated.gpu.renderer));

        // DOMRect patches
        assert!(script.contains("Element.prototype.getBoundingClientRect"));
        assert!(script.contains("Element.prototype.getClientRects"));
        assert!(script.contains("Range.prototype.getBoundingClientRect"));
        assert!(script.contains("Range.prototype.getClientRects"));

        // Canvas noise
        assert!(script.contains("HTMLCanvasElement.prototype.toDataURL"));
        assert!(script.contains("HTMLCanvasElement.prototype.toBlob"));
        assert!(script.contains("CanvasRenderingContext2D.prototype.getImageData"));
        assert!(script.contains("CanvasRenderingContext2D.prototype.measureText"));

        // Audio noise
        assert!(script.contains("AudioBuffer.prototype.getChannelData"));
        assert!(script.contains("AudioBuffer.prototype.copyFromChannel"));
        assert!(script.contains("AnalyserNode.prototype.getFloatFrequencyData"));
        assert!(script.contains("AnalyserNode.prototype.getByteFrequencyData"));
        assert!(script.contains("AnalyserNode.prototype.getFloatTimeDomainData"));
        assert!(script.contains("AnalyserNode.prototype.getByteTimeDomainData"));
        assert!(script.contains("AudioDestinationNode.prototype"));

        // WebGL2 constants & readPixels
        assert!(script.contains("case 35071:"));
        assert!(script.contains("case 35376:"));
        assert!(script.contains("case 35375:"));
        assert!(script.contains("case 34076:"));
        assert!(script.contains("case 3414:"));
        assert!(script.contains("case 3415:"));
        assert!(script.contains("proto.readPixels"));

        // WebGPU GPUAdapter.prototype.info
        assert!(script.contains("GPUAdapter.prototype, 'info'"));
        assert!(script.contains("GPUAdapterInfo.prototype"));

        // Screen.prototype.availTop / availLeft / isExtended
        assert!(script.contains("defineGetter(Screen.prototype, 'availTop'"));
        assert!(script.contains("defineGetter(Screen.prototype, 'availLeft'"));
        assert!(script.contains("defineGetter(Screen.prototype, 'isExtended'"));

        // Notification.permission <-> Permissions.prototype.query
        assert!(script.contains("Notification.permission"));
        assert!(script.contains("Permissions.prototype.query"));

        // Error.prototype.stack scrubber without Error.prepareStackTrace
        assert!(script.contains("Error.prototype, 'stack'"));
        assert!(script.contains("UtilityScript."));
        assert!(script.contains("__webdriver_evaluate"));
        assert!(script.contains("evaluate (at <anonymous>:"));
        assert!(script.contains("devtools://"));
        assert!(script.contains("ExecutionContext.evaluate"));
        assert!(script.contains("InjectedScript."));
        assert!(!script.contains("prepareStackTrace"));

        // Client Hints genuine build version
        assert!(script.contains("157.0.8103.39"));
        assert!(!script.contains("7163"));

        // OfflineAudioContext must preserve explicit constructor sampleRate
        assert!(script.contains("patchAudioContext(AudioContext, true)"));
        assert!(script.contains("patchAudioContext(OfflineAudioContext, false)"));
    }

    #[test]
    fn bundle_differentiates_webgl_caps_and_noise_policies() {
        let desktop = FingerprintGenerator::new(11)
            .os(OsType::Windows)
            .canvas_noise(CanvasNoise::Low)
            .audio_noise(AudioNoise::Natural)
            .generate();
        let desktop_js = StealthScriptBundle::from_generated(&desktop).into_script();
        assert!(desktop_js.contains("const canvasDensity = 8;"));
        assert!(!desktop_js.contains("AudioBuffer.prototype.getChannelData"));
        assert!(!desktop_js.contains("AnalyserNode.prototype"));
        assert!(desktop_js.contains("case 36347: return 4096;"));
        assert!(desktop_js.contains("case 35661: return 80;"));

        let android = FingerprintGenerator::new(11)
            .os(OsType::Android)
            .canvas_noise(CanvasNoise::Natural)
            .generate();
        let android_js = StealthScriptBundle::from_generated(&android).into_script();
        assert!(android_js.contains("const canvasDensity = 0;"));
        assert!(android_js.contains("AudioBuffer.prototype.getChannelData"));
        assert!(android_js.contains("case 36347: return 1024;"));
        assert!(android_js.contains("case 35661: return 32;"));
        assert!(android_js.contains("case 3379: return 8192;"));
        assert!(!android_js.contains(": inf,"));
        if !android.battery.charging {
            assert!(android_js.contains("chargingTime: Infinity"));
        }
    }

    #[test]
    fn bundle_omits_client_hints_and_chrome_for_ios_safari() {
        let generator = FingerprintGenerator::new(12).os(OsType::Ios).generate();
        let script = StealthScriptBundle::from_generated(&generator).into_script();

        assert!(script.contains("delete navProto.userAgentData"));
        assert!(!script.contains("window.chrome = {"));
        assert!(script.contains("delete Object.getPrototypeOf(navigator).getBattery"));
        assert!(script.contains("delete Object.getPrototypeOf(navigator).connection"));
    }

    #[test]
    fn selective_script_options_omit_disabled_evasions() {
        let profile = ProfileFingerprint::default();
        let minimal = StealthScriptOptions::minimal();
        let js = StealthScriptBundle::from_profile_with(&profile, &minimal).into_script();
        assert!(js.contains("navigator"));
        assert!(js.contains("webdriver"));
        assert!(!js.contains("patchWebGLParams"));
        assert!(!js.contains("AudioBuffer.prototype"));
        assert!(!js.contains("enumerateDevices"));
        assert!(!js.contains("speechSynthesis"));
        assert!(!js.contains("window.matchMedia"));
        assert!(!js.contains("Performance.prototype.now"));
        assert!(!js.contains("wrapWorkerCtor"));
    }

    #[test]
    fn expanded_fingerprint_options_and_script_surfaces_render_coherently() {
        use crate::fingerprint::{
            AudioDynamicsCompressor, BatteryOverride, ColorScheme, FingerprintOptions, HdrMode,
            ReducedMotion, SpeechVoice,
        };
        use seleniumbase_rs::evasion::settings::ColorGamut;

        let opts = FingerprintOptions::builder()
            .os(OsType::Macos)
            .fonts(vec!["SF Pro".to_owned(), "Menlo".to_owned()])
            .battery(BatteryOverride::new(false, 0.72))
            .speech_voices(vec![SpeechVoice::new("Samantha", "en-US")])
            .max_touch_points(0)
            .do_not_track("1")
            .pdf_viewer_enabled(true)
            .color_scheme(ColorScheme::Dark)
            .reduced_motion(ReducedMotion::Reduce)
            .color_gamut(ColorGamut::P3)
            .hdr_mode(HdrMode::High)
            .performance_jitter_ms(0.12)
            .worker_spoofing(true)
            .audio_compressor(AudioDynamicsCompressor {
                threshold: -48.0,
                knee: 36.0,
                ratio: 10.0,
                attack: 0.002,
                release: 0.2,
            })
            .seed(9090)
            .build()
            .expect("valid expanded fingerprint options");

        let generated = FingerprintGenerator::generate_with(&opts);
        assert_eq!(generated.fonts, vec!["SF Pro", "Menlo"]);
        assert!(!generated.battery.charging);
        assert!((generated.battery.level - 0.72).abs() < 1e-9);
        assert_eq!(generated.audio.compressor.threshold, -48.0);
        assert_eq!(generated.profile.do_not_track.as_deref(), Some("1"));
        assert_eq!(generated.profile.color_scheme, Some(ColorScheme::Dark));
        assert_eq!(
            generated.profile.reduced_motion,
            Some(ReducedMotion::Reduce)
        );

        let js = StealthScriptBundle::from_generated(&generated).into_script();
        assert!(js.contains("defineGetter(navProto, 'doNotTrack', () => \"1\")"));
        assert!(js.contains("defineGetter(navProto, 'pdfViewerEnabled', () => true)"));
        assert!(js.contains("Samantha"));
        assert!(js.contains("prefers-color-scheme"));
        assert!(js.contains("prefers-reduced-motion"));
        assert!(js.contains("Performance.prototype.now"));
        assert!(js.contains("wrapWorkerCtor"));
        assert!(js.contains("SF Pro"));
        assert!(js.contains("Intl.DateTimeFormat"));
        assert!(js.contains("RTCPeerConnection"));
        assert!(js.contains("SVGTextContentElement"));
        assert!(js.contains("outerHeight"));
        assert!(js.contains("StorageManager.prototype.estimate"));
        assert!(js.contains("getGamepads"));
        assert!(js.contains("visibilityState"));
        assert!(js.contains("hasGetterTrap"));
        assert!(js.contains("MouseEvent.prototype, 'screenX'"));
        assert!(js.contains("HTMLMediaElement.prototype.canPlayType"));
        assert!(js.contains("Date.prototype.getTimezoneOffset"));
        assert!(js.contains("HTMLIFrameElement.prototype, 'contentWindow'"));
        assert!(js.contains("History.prototype, 'length'"));
        assert!(js.contains("HTMLCanvasElement.prototype.toBlob"));
        assert!(js.contains("getShaderPrecisionFormat"));
        assert!(js.contains("MediaCapabilities.prototype.decodingInfo"));
        assert!(js.contains("Keyboard.prototype.getLayoutMap"));
        assert!(js.contains("'clipboard' in navigator"));

        assert!(FingerprintOptions::builder()
            .battery(BatteryOverride {
                charging: true,
                level: 1.5,
                charging_time: 0.0,
                discharging_time: f64::INFINITY,
            })
            .build()
            .is_err());
        assert!(FingerprintOptions::builder()
            .performance_jitter_ms(-0.1)
            .build()
            .is_err());
    }
}
