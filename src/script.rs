//! Stealth JavaScript injection bundle.
//!
//! Generates the self-contained JavaScript bundle to be evaluated on every new document
//! (`Page.addScriptToEvaluateOnNewDocument` / WebDriver script injection) to protect
//! browser automation against anti-bot fingerprinting and detection vectors.

use seleniumbase_rs::evasion::fingerprint::Fingerprint;

use crate::fingerprint::{
    AudioMetadata, BatteryMetadata, ClientHintsHighEntropy, DisplayMetadata, GeneratedFingerprint,
    GpuMetadata, MediaDeviceMetadata, NetworkMetadata, ProfileFingerprint, WebGpuMetadata,
};

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
        let script = generate_stealth_js(
            &generated.profile,
            Some(&generated.gpu),
            Some(&generated.webgpu),
            Some(&generated.display),
            Some(&generated.client_hints),
            Some(&generated.audio),
            Some(&generated.media_devices),
            Some(&generated.battery),
            Some(&generated.network),
        );
        Self { script }
    }

    /// Creates a stealth bundle from a [`ProfileFingerprint`].
    #[must_use]
    pub fn from_profile(profile: &ProfileFingerprint) -> Self {
        let script = generate_stealth_js(profile, None, None, None, None, None, None, None, None);
        Self { script }
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

/// Generates the complete stealth JavaScript injection string.
#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::similar_names,
    reason = "comprehensive browser stealth script generation with WebGL uniform parameters"
)]
fn generate_stealth_js(
    profile: &ProfileFingerprint,
    gpu: Option<&GpuMetadata>,
    webgpu: Option<&WebGpuMetadata>,
    display: Option<&DisplayMetadata>,
    client_hints: Option<&ClientHintsHighEntropy>,
    audio: Option<&AudioMetadata>,
    media_devices: Option<&MediaDeviceMetadata>,
    battery: Option<&BatteryMetadata>,
    network: Option<&NetworkMetadata>,
) -> String {
    let languages_json = serde_json::to_string(&profile.languages)
        .unwrap_or_else(|_| "[\"en-US\", \"en\"]".to_string());
    let primary_lang = profile.languages.first().map_or("en-US", String::as_str);
    let platform = profile.platform.as_deref().unwrap_or("Win32");
    let hw_concurrency = profile.hardware_concurrency.unwrap_or(8);
    let dev_memory = profile.device_memory.unwrap_or(8.0);

    let screen_w = display.map_or_else(|| profile.screen_width.unwrap_or(1920), |d| d.width);
    let screen_h = display.map_or_else(|| profile.screen_height.unwrap_or(1080), |d| d.height);
    let avail_w = display.map_or_else(
        || profile.avail_width.unwrap_or(screen_w),
        |d| d.avail_width,
    );
    let avail_h = display.map_or_else(
        || profile.avail_height.unwrap_or(screen_h.saturating_sub(40)),
        |d| d.avail_height,
    );
    let pixel_ratio = display.map_or_else(
        || profile.device_pixel_ratio.unwrap_or(1.0),
        |d| d.pixel_ratio,
    );
    let color_depth = display.map_or_else(|| profile.color_depth.unwrap_or(24), |d| d.color_depth);
    let orientation_type = display
        .map(|d| d.orientation.orientation_type.as_str())
        .or_else(|| {
            profile
                .screen_orientation
                .map(|o| o.orientation_type.as_str())
        })
        .unwrap_or("landscape-primary");
    let orientation_angle = display
        .map(|d| d.orientation.angle)
        .or_else(|| profile.screen_orientation.map(|o| o.angle))
        .unwrap_or(0);

    let unmasked_vendor = gpu.map_or_else(
        || {
            profile
                .webgl_vendor
                .as_deref()
                .unwrap_or("Google Inc. (NVIDIA)")
        },
        |g| &g.vendor,
    );
    let unmasked_renderer = gpu.map_or_else(
        || {
            profile.webgl_renderer.as_deref().unwrap_or(
                "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)",
            )
        },
        |g| &g.renderer,
    );

    let extensions_json = gpu.map_or_else(
        || serde_json::to_string(&profile.webgl_extensions).unwrap_or_else(|_| "[]".to_string()),
        |g| serde_json::to_string(&g.extensions).unwrap_or_else(|_| "[]".to_string()),
    );

    let (
        max_texture_size,
        max_viewport_dims_json,
        max_vertex_attribs,
        max_v_uniform,
        max_varying,
        max_f_uniform,
    ) = if let Some(g) = gpu {
        (
            g.parameters.max_texture_size,
            serde_json::to_string(&g.parameters.max_viewport_dims)
                .unwrap_or_else(|_| "[16384, 16384]".to_string()),
            g.parameters.max_vertex_attribs,
            g.parameters.max_vertex_uniform_vectors,
            g.parameters.max_varying_vectors,
            g.parameters.max_fragment_uniform_vectors,
        )
    } else {
        (16384, "[16384, 16384]".to_string(), 16, 1024, 30, 1024)
    };

    let webgpu_vendor = webgpu.map_or("nvidia", |w| &w.vendor);
    let webgpu_arch = webgpu.map_or("ampere", |w| &w.architecture);
    let webgpu_desc = webgpu.map_or(unmasked_renderer, |w| &w.description);
    let webgpu_features_json = webgpu.map_or_else(
        || "[\"depth-clip-control\", \"depth32float-stencil8\", \"texture-compression-bc\", \"timestamp-query\"]".to_string(),
        |w| serde_json::to_string(&w.features).unwrap_or_else(|_| "[]".to_string()),
    );
    let webgpu_limits_json = webgpu.map_or_else(
        || {
            serde_json::json!({
                "maxTextureDimension1D": 16384,
                "maxTextureDimension2D": 16384,
                "maxTextureDimension3D": 2048,
                "maxTextureArrayLayers": 2048,
                "maxBindGroups": 4,
                "maxBindGroupsPlusVertexBuffers": 24,
                "maxBindingsPerBindGroup": 1000,
                "maxDynamicUniformBuffersPerPipelineLayout": 8,
                "maxDynamicStorageBuffersPerPipelineLayout": 4,
                "maxSampledTexturesPerShaderStage": 16,
                "maxSamplersPerShaderStage": 16,
                "maxStorageBuffersPerShaderStage": 8,
                "maxStorageTexturesPerShaderStage": 4,
                "maxUniformBuffersPerShaderStage": 12,
                "maxUniformBufferBindingSize": 65536,
                "maxStorageBufferBindingSize": 134_217_728,
                "minUniformBufferOffsetAlignment": 256,
                "minStorageBufferOffsetAlignment": 256,
                "maxVertexBuffers": 8,
                "maxBufferSize": 268_435_456,
                "maxVertexAttributes": 16,
                "maxVertexBufferArrayStride": 2048,
                "maxInterStageShaderComponents": 60,
                "maxInterStageShaderVariables": 16,
                "maxColorAttachments": 8,
                "maxColorAttachmentBytesPerSample": 32,
                "maxComputeWorkgroupStorageSize": 32768,
                "maxComputeInvocationsPerWorkgroup": 256,
                "maxComputeWorkgroupSizeX": 256,
                "maxComputeWorkgroupSizeY": 256,
                "maxComputeWorkgroupSizeZ": 64,
                "maxComputeWorkgroupsPerDimension": 65535,
            })
            .to_string()
        },
        |w| serde_json::to_string(&w.limits).unwrap_or_else(|_| "{}".to_string()),
    );

    let ch_arch = client_hints.map_or("x86", |ch| &ch.architecture);
    let ch_bitness = client_hints.map_or("64", |ch| &ch.bitness);
    let ch_model = client_hints.map_or("", |ch| &ch.model);
    let ch_platform = client_hints.map_or("Windows", |ch| &ch.platform);
    let ch_platform_v = client_hints.map_or("15.0.0", |ch| &ch.platform_version);
    let ch_mobile = client_hints.is_some_and(|ch| ch.mobile);
    let ch_full_versions_json = client_hints.map_or_else(
        || {
            serde_json::json!([
                { "brand": "Not_A Brand", "version": "8.0.0.0" },
                { "brand": "Chromium", "version": "157.0.7163.0" },
                { "brand": "Google Chrome", "version": "157.0.7163.0" }
            ])
            .to_string()
        },
        |ch| serde_json::to_string(&ch.full_version_list).unwrap_or_else(|_| "[]".to_string()),
    );

    let audio_sample_rate = audio.map_or_else(
        || profile.audio_sample_rate.unwrap_or(48000),
        |a| a.sample_rate,
    );
    let audio_channels = audio.map_or(2, |a| a.max_channel_count);
    let audio_latency = audio.map_or(0.015, |a| a.output_latency);
    let audio_base_latency = audio.map_or(0.005, |a| a.base_latency);
    let (comp_thresh, comp_knee, comp_ratio, comp_attack, comp_release) =
        audio.map_or((-24.0, 30.0, 12.0, 0.003, 0.25), |a| {
            (
                a.compressor.threshold,
                a.compressor.knee,
                a.compressor.ratio,
                a.compressor.attack,
                a.compressor.release,
            )
        });

    let media_devices_json = media_devices.map_or_else(
        || "[]".to_string(),
        |m| serde_json::to_string(&m.devices).unwrap_or_else(|_| "[]".to_string()),
    );

    let (bat_charging, bat_ch_time, bat_dis_time, bat_level) = battery
        .map_or((true, 0.0, f64::INFINITY, 1.0), |b| {
            (b.charging, b.charging_time, b.discharging_time, b.level)
        });
    let bat_dis_time_js = if bat_dis_time.is_infinite() {
        "Infinity".to_string()
    } else {
        bat_dis_time.to_string()
    };

    let (net_downlink, net_type, net_rtt, net_save_data) = network
        .map_or((10.0, "4g", 50, false), |n| {
            (n.downlink, n.effective_type.as_str(), n.rtt, n.save_data)
        });

    format!(
        r"(function() {{
  'use strict';

  // 1. Clean automation markers and webdriver flags
  try {{
    delete window.__webdriver_evaluate;
    delete document.__webdriver_evaluate;
    delete window.__selenium_evaluate;
    delete document.__selenium_evaluate;
    delete window.__webdriver_script_function;
    delete document.__webdriver_script_function;
    delete window.__webdriver_script_fn;
    delete document.__webdriver_script_fn;
    delete window.$cdc_asdjflasutopfhvcZLmcfl_;
    delete document.$cdc_asdjflasutopfhvcZLmcfl_;

    for (const key of Object.getOwnPropertyNames(window)) {{
      if (key.startsWith('cdc_') || key.startsWith('$cdc_')) {{
        delete window[key];
      }}
    }}
  }} catch (_) {{}}

  try {{
    Object.defineProperty(Object.getPrototypeOf(navigator), 'webdriver', {{
      get: () => undefined,
      enumerable: true,
      configurable: true
    }});
  }} catch (_) {{}}

  // 2. Languages
  try {{
    const languages = {languages_json};
    const primary = {primary_lang:?};
    Object.defineProperty(navigator, 'languages', {{
      get: () => languages,
      enumerable: true,
      configurable: true
    }});
    Object.defineProperty(navigator, 'language', {{
      get: () => primary,
      enumerable: true,
      configurable: true
    }});
  }} catch (_) {{}}

  // 3. Platform & Hardware
  try {{
    Object.defineProperty(navigator, 'platform', {{
      get: () => {platform:?},
      enumerable: true,
      configurable: true
    }});
    Object.defineProperty(navigator, 'hardwareConcurrency', {{
      get: () => {hw_concurrency},
      enumerable: true,
      configurable: true
    }});
    Object.defineProperty(navigator, 'deviceMemory', {{
      get: () => {dev_memory},
      enumerable: true,
      configurable: true
    }});
  }} catch (_) {{}}

  // 4. Client Hints High Entropy
  try {{
    if (navigator.userAgentData) {{
      const highEntropy = {{
        architecture: {ch_arch:?},
        bitness: {ch_bitness:?},
        model: {ch_model:?},
        platform: {ch_platform:?},
        platformVersion: {ch_platform_v:?},
        fullVersionList: {ch_full_versions_json},
        mobile: {ch_mobile},
        wow64: false
      }};

      const origGetHighEntropyValues = navigator.userAgentData.getHighEntropyValues;
      navigator.userAgentData.getHighEntropyValues = function(hints) {{
        return Promise.resolve(highEntropy);
      }};
    }}
  }} catch (_) {{}}

  // 5. Screen & Viewport with Taskbar & Orientation
  try {{
    Object.defineProperty(screen, 'width', {{ get: () => {screen_w}, configurable: true }});
    Object.defineProperty(screen, 'height', {{ get: () => {screen_h}, configurable: true }});
    Object.defineProperty(screen, 'availWidth', {{ get: () => {avail_w}, configurable: true }});
    Object.defineProperty(screen, 'availHeight', {{ get: () => {avail_h}, configurable: true }});
    Object.defineProperty(screen, 'colorDepth', {{ get: () => {color_depth}, configurable: true }});
    Object.defineProperty(screen, 'pixelDepth', {{ get: () => {color_depth}, configurable: true }});
    Object.defineProperty(window, 'devicePixelRatio', {{ get: () => {pixel_ratio}, configurable: true }});

    const orientationObj = {{
      type: {orientation_type:?},
      angle: {orientation_angle},
      onchange: null,
      addEventListener: () => {{}},
      removeEventListener: () => {{}},
      dispatchEvent: () => false,
      lock: () => Promise.resolve(),
      unlock: () => {{}}
    }};
    Object.defineProperty(screen, 'orientation', {{
      get: () => orientationObj,
      configurable: true
    }});
  }} catch (_) {{}}

  // 6. WebGL 1 & 2 Parameters & Extensions
  try {{
    const unmaskedVendor = {unmasked_vendor:?};
    const unmaskedRenderer = {unmasked_renderer:?};
    const extensions = {extensions_json};
    const maxViewportDims = new Int32Array({max_viewport_dims_json});

    const patchWebGL = (proto) => {{
      if (!proto) return;
      const origGetParameter = proto.getParameter;
      proto.getParameter = function(param) {{
        switch (param) {{
          case 37445: return unmaskedVendor; // UNMASKED_VENDOR_WEBGL
          case 37446: return unmaskedRenderer; // UNMASKED_RENDERER_WEBGL
          case 7936: return 'WebKit'; // VENDOR
          case 7937: return 'WebKit WebGL'; // RENDERER
          case 35724: return 'WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)'; // SHADING_LANGUAGE_VERSION
          case 3379: return {max_texture_size}; // MAX_TEXTURE_SIZE
          case 3386: return maxViewportDims; // MAX_VIEWPORT_DIMS
          case 34921: return {max_vertex_attribs}; // MAX_VERTEX_ATTRIBS
          case 36347: return {max_v_uniform}; // MAX_VERTEX_UNIFORM_VECTORS
          case 36348: return {max_varying}; // MAX_VARYING_VECTORS
          case 36349: return {max_f_uniform}; // MAX_FRAGMENT_UNIFORM_VECTORS
          default: return origGetParameter.apply(this, arguments);
        }}
      }};

      proto.getSupportedExtensions = function() {{
        return extensions;
      }};
    }};

    if (window.WebGLRenderingContext) patchWebGL(WebGLRenderingContext.prototype);
    if (window.WebGL2RenderingContext) patchWebGL(WebGL2RenderingContext.prototype);
  }} catch (_) {{}}

  // 7. WebGPU Adapter & Limits
  try {{
    if (navigator.gpu) {{
      const webgpuLimits = {webgpu_limits_json};
      const webgpuFeatures = new Set({webgpu_features_json});
      const adapterInfo = {{
        vendor: {webgpu_vendor:?},
        architecture: {webgpu_arch:?},
        device: '',
        description: {webgpu_desc:?}
      }};

      const mockAdapter = {{
        features: webgpuFeatures,
        limits: webgpuLimits,
        isFallbackAdapter: false,
        requestAdapterInfo: () => Promise.resolve(adapterInfo)
      }};

      navigator.gpu.requestAdapter = function() {{
        return Promise.resolve(mockAdapter);
      }};
    }}
  }} catch (_) {{}}

  // 8. Web Audio API & Compressor
  try {{
    const patchAudioContext = (AudioCtx) => {{
      if (!AudioCtx) return;
      Object.defineProperty(AudioCtx.prototype, 'sampleRate', {{
        get: () => {audio_sample_rate},
        configurable: true
      }});
      Object.defineProperty(AudioCtx.prototype, 'baseLatency', {{
        get: () => {audio_base_latency},
        configurable: true
      }});
      Object.defineProperty(AudioCtx.prototype, 'outputLatency', {{
        get: () => {audio_latency},
        configurable: true
      }});
      if (AudioCtx.prototype.destination) {{
        try {{
          Object.defineProperty(AudioCtx.prototype.destination, 'maxChannelCount', {{
            get: () => {audio_channels},
            configurable: true
          }});
        }} catch (_) {{}}
      }}

      const origCreateCompressor = AudioCtx.prototype.createDynamicsCompressor;
      AudioCtx.prototype.createDynamicsCompressor = function() {{
        const node = origCreateCompressor.apply(this, arguments);
        try {{
          node.threshold.value = {comp_thresh};
          node.knee.value = {comp_knee};
          node.ratio.value = {comp_ratio};
          node.attack.value = {comp_attack};
          node.release.value = {comp_release};
        }} catch (_) {{}}
        return node;
      }};
    }};

    if (window.AudioContext) patchAudioContext(AudioContext);
    if (window.webkitAudioContext) patchAudioContext(webkitAudioContext);
    if (window.OfflineAudioContext) patchAudioContext(OfflineAudioContext);
  }} catch (_) {{}}

  // 9. Media Devices
  try {{
    if (navigator.mediaDevices) {{
      const syntheticDevices = {media_devices_json};
      navigator.mediaDevices.enumerateDevices = function() {{
        return Promise.resolve(syntheticDevices.map(d => ({{
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
      }};
    }}
  }} catch (_) {{}}

  // 10. Battery
  try {{
    if (navigator.getBattery) {{
      const batteryInfo = {{
        charging: {bat_charging},
        chargingTime: {bat_ch_time},
        dischargingTime: {bat_dis_time_js},
        level: {bat_level},
        addEventListener: () => {{}},
        removeEventListener: () => {{}},
        dispatchEvent: () => false,
        onchargingchange: null,
        onchargingtimechange: null,
        ondischargingtimechange: null,
        onlevelchange: null
      }};
      navigator.getBattery = () => Promise.resolve(batteryInfo);
    }}
  }} catch (_) {{}}

  // 11. Network Connection
  try {{
    if (navigator.connection) {{
      Object.defineProperty(navigator.connection, 'downlink', {{ get: () => {net_downlink}, configurable: true }});
      Object.defineProperty(navigator.connection, 'effectiveType', {{ get: () => {net_type:?}, configurable: true }});
      Object.defineProperty(navigator.connection, 'rtt', {{ get: () => {net_rtt}, configurable: true }});
      Object.defineProperty(navigator.connection, 'saveData', {{ get: () => {net_save_data}, configurable: true }});
    }}
  }} catch (_) {{}}

  // 12. Window Chrome & Permissions
  try {{
    if (!window.chrome) {{
      window.chrome = {{
        app: {{
          isInstalled: false,
          InstallState: {{ DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' }},
          RunningState: {{ CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' }}
        }},
        csi: () => {{}},
        loadTimes: () => ({{
          requestTime: Date.now() / 1000 - 0.5,
          startLoadTime: Date.now() / 1000 - 0.45,
          commitLoadTime: Date.now() / 1000 - 0.4,
          finishDocumentLoadTime: Date.now() / 1000 - 0.2,
          firstPaintTime: Date.now() / 1000 - 0.35,
          finishLoadTime: Date.now() / 1000 - 0.1,
          wasFetchedViaSpdy: true,
          wasNpnNegotiated: true,
          npnNegotiatedProtocol: 'h2',
          wasAlternateProtocolAvailable: false,
          connectionInfo: 'h2'
        }}),
        runtime: {{
          OnInstalledReason: {{ CHROME_UPDATE: 'chrome_update', INSTALL: 'install', SHARED_MODULE_UPDATE: 'shared_module_update', UPDATE: 'update' }},
          OnRestartRequiredReason: {{ APP_UPDATE: 'app_update', OS_UPDATE: 'os_update', PERIODIC: 'periodic' }},
          PlatformArch: {{ ARM: 'arm', ARM64: 'arm64', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' }},
          PlatformNaclArch: {{ ARM: 'arm', MIPS: 'mips', MIPS64: 'mips64', X86_32: 'x86-32', X86_64: 'x86-64' }},
          PlatformOs: {{ ANDROID: 'android', CROS: 'cros', LINUX: 'linux', MAC: 'mac', OPENBSD: 'openbsd', WIN: 'win' }},
          RequestUpdateCheckStatus: {{ NO_UPDATE: 'no_update', THROTTLED: 'throttled', UPDATE_AVAILABLE: 'update_available' }}
        }}
      }};
    }}
  }} catch (_) {{}}

  try {{
    if (navigator.permissions && navigator.permissions.query) {{
      const origQuery = navigator.permissions.query;
      navigator.permissions.query = function(param) {{
        if (param && param.name === 'notifications') {{
          return Promise.resolve({{ state: 'prompt', onchange: null }});
        }}
        return origQuery.apply(this, arguments);
      }};
    }}
  }} catch (_) {{}}
}})();
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::FingerprintGenerator;
    use seleniumbase_rs::evasion::fingerprint::OsType;

    #[test]
    fn bundle_generates_valid_stealth_script_from_fingerprint() {
        let fp = Fingerprint::randomized(OsType::Windows, 42);
        let bundle = StealthScriptBundle::new(&fp);
        let script = bundle.script();

        assert!(
            script.contains("Object.defineProperty(Object.getPrototypeOf(navigator), 'webdriver'")
        );
        assert!(script.contains("languages"));
        assert!(script.contains("hardwareConcurrency"));
        assert!(script.contains("WebGL"));
        assert!(script.contains("window.chrome"));
    }

    #[test]
    fn bundle_generates_advanced_spoofing_from_generated_bundle() {
        let generator = FingerprintGenerator::new(88).os(OsType::Macos).major(157);
        let generated = generator.generate();
        let bundle = StealthScriptBundle::from_generated(&generated);
        let script = bundle.script();

        assert!(script.contains("Apple M2"));
        assert!(script.contains("webgpuLimits"));
        assert!(script.contains("navigator.mediaDevices.enumerateDevices"));
        assert!(script.contains("sampleRate"));
        assert!(script.contains("createDynamicsCompressor"));
        assert!(script.contains("getHighEntropyValues"));
    }
}
