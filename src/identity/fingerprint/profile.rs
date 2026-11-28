//! Unified JSON-serializable profile fingerprint and hardware/display metadata models.
//!
//! [`ProfileFingerprint`] exposes flat, API-aligned field names (`user_agent`,
//! `platform`, `languages`, `hardware_concurrency`, `device_memory`,
//! `webgl_vendor`, `webgl_renderer`, `canvas_noise`, `audio_noise`,
//! `webrtc_policy`) and converts cleanly to and from core [`Fingerprint`].

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use super::randomize::{
    Locale, PRESET_CHROMIUM_MAJOR, chromium_hints, edge_hints, known_full_version,
};
use super::settings::DynamicRange;
use super::{
    AudioNoise, BatteryProfile, BrandVersion, BrowserType, CanvasNoise,
    ClientHints as CoreClientHints, ColorGamut, ColorScheme, ConnectionProfile, Fingerprint,
    FingerprintBuilder, FingerprintError, MediaFeatures, OsType, SpeechVoice, Spoof, WebGl,
    WebRtcPolicy,
};

/// High Dynamic Range (HDR) screen capability mode (alias of [`DynamicRange`]).
pub type HdrMode = DynamicRange;

/// CSS `prefers-reduced-motion` media feature value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReducedMotion {
    /// Standard animations (`(prefers-reduced-motion: no-preference)`).
    NoPreference,
    /// Reduced motion (`(prefers-reduced-motion: reduce)`).
    Reduce,
}

impl ReducedMotion {
    /// Returns the CSS media query token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoPreference => "no-preference",
            Self::Reduce => "reduce",
        }
    }
}

/// Screen orientation type according to the W3C Screen Orientation API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenOrientationType {
    /// Landscape primary (standard desktop).
    #[default]
    LandscapePrimary,
    /// Portrait primary (standard mobile/tablet).
    PortraitPrimary,
    /// Landscape secondary (180 degree rotation).
    LandscapeSecondary,
    /// Portrait secondary (180 degree rotation).
    PortraitSecondary,
}

impl ScreenOrientationType {
    /// Returns the standard string identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LandscapePrimary => "landscape-primary",
            Self::PortraitPrimary => "portrait-primary",
            Self::LandscapeSecondary => "landscape-secondary",
            Self::PortraitSecondary => "portrait-secondary",
        }
    }
}

impl fmt::Display for ScreenOrientationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Screen orientation metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenOrientation {
    /// Orientation type string (`landscape-primary`, `portrait-primary`, etc.).
    #[serde(rename = "type")]
    pub orientation_type: ScreenOrientationType,
    /// Clockwise orientation angle in degrees (0, 90, 180, 270).
    pub angle: u32,
}

impl Default for ScreenOrientation {
    fn default() -> Self {
        Self {
            orientation_type: ScreenOrientationType::LandscapePrimary,
            angle: 0,
        }
    }
}

/// Audio dynamics compressor parameters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioDynamicsCompressor {
    /// Decibel value above which compression begins.
    pub threshold: f32,
    /// Decibel range over which the compression transition occurs.
    pub knee: f32,
    /// Input/output change ratio.
    pub ratio: f32,
    /// Time in seconds to reduce gain by 10dB.
    pub attack: f32,
    /// Time in seconds to increase gain by 10dB.
    pub release: f32,
}

impl Default for AudioDynamicsCompressor {
    fn default() -> Self {
        Self {
            threshold: -24.0,
            knee: 30.0,
            ratio: 12.0,
            attack: 0.003,
            release: 0.25,
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
    /// Base latency in seconds.
    pub base_latency: f64,
    /// Dynamics compressor settings.
    pub compressor: AudioDynamicsCompressor,
}

impl Default for AudioMetadata {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            max_channel_count: 2,
            output_latency: 0.015,
            base_latency: 0.005,
            compressor: AudioDynamicsCompressor::default(),
        }
    }
}

/// Complete WebGL 1 & 2 parameter tables matching empirical GPU configurations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebGlParameters {
    /// `MAX_TEXTURE_SIZE`
    pub max_texture_size: u32,
    /// `MAX_CUBE_MAP_TEXTURE_SIZE`
    pub max_cube_map_texture_size: u32,
    /// `MAX_RENDERBUFFER_SIZE`
    pub max_renderbuffer_size: u32,
    /// `MAX_VERTEX_ATTRIBS`
    pub max_vertex_attribs: u32,
    /// `MAX_VERTEX_UNIFORM_VECTORS`
    pub max_vertex_uniform_vectors: u32,
    /// `MAX_VARYING_VECTORS`
    pub max_varying_vectors: u32,
    /// `MAX_FRAGMENT_UNIFORM_VECTORS`
    pub max_fragment_uniform_vectors: u32,
    /// `MAX_VIEWPORT_DIMS`
    pub max_viewport_dims: [u32; 2],
    /// `ALIASED_LINE_WIDTH_RANGE`
    pub aliased_line_width_range: [f32; 2],
    /// `ALIASED_POINT_SIZE_RANGE`
    pub aliased_point_size_range: [f32; 2],
    /// `MAX_COMBINED_TEXTURE_IMAGE_UNITS`
    pub max_combined_texture_image_units: u32,
    /// `MAX_VERTEX_TEXTURE_IMAGE_UNITS`
    pub max_vertex_texture_image_units: u32,
    /// `MAX_TEXTURE_IMAGE_UNITS`
    pub max_texture_image_units: u32,
    /// WebGL 2 `MAX_3D_TEXTURE_SIZE`
    pub max_3d_texture_size: u32,
    /// WebGL 2 `MAX_ARRAY_TEXTURE_LAYERS`
    pub max_array_texture_layers: u32,
    /// WebGL 2 `MAX_COLOR_ATTACHMENTS`
    pub max_color_attachments: u32,
    /// WebGL 2 `MAX_DRAW_BUFFERS`
    pub max_draw_buffers: u32,
    /// WebGL 2 `MAX_UNIFORM_BUFFER_BINDINGS`
    pub max_uniform_buffer_bindings: u32,
    /// WebGL 2 `MAX_UNIFORM_BLOCK_SIZE`
    pub max_uniform_block_size: u64,
    /// WebGL 2 `UNIFORM_BUFFER_OFFSET_ALIGNMENT`
    pub uniform_buffer_offset_alignment: u32,
    /// WebGL 2 `MAX_VERTEX_OUTPUT_COMPONENTS`
    pub max_vertex_output_components: u32,
    /// WebGL 2 `MAX_FRAGMENT_INPUT_COMPONENTS`
    pub max_fragment_input_components: u32,
    /// WebGL 2 `MIN_PROGRAM_TEXEL_OFFSET`
    pub min_program_texel_offset: i32,
    /// WebGL 2 `MAX_PROGRAM_TEXEL_OFFSET`
    pub max_program_texel_offset: i32,
    /// WebGL 2 `MAX_TRANSFORM_FEEDBACK_SEPARATE_COMPONENTS`
    pub max_transform_feedback_separate_components: u32,
    /// Shading language version string.
    pub shading_language_version: String,
    /// WebGL vendor.
    pub vendor: String,
    /// WebGL renderer.
    pub renderer: String,
    /// Unmasked WebGL vendor.
    pub unmasked_vendor: String,
    /// Unmasked WebGL renderer.
    pub unmasked_renderer: String,
}

impl WebGlParameters {
    /// Generates WebGL parameters matching an OS and unmasked GPU pair.
    #[must_use]
    pub fn for_gpu(os: OsType, unmasked_vendor: &str, unmasked_renderer: &str) -> Self {
        Self::for_gpu_with_max_texture(os, unmasked_vendor, unmasked_renderer, None)
    }

    /// Generates WebGL parameters with an optional explicit `MAX_TEXTURE_SIZE` cap.
    #[must_use]
    pub fn for_gpu_with_max_texture(
        os: OsType,
        unmasked_vendor: &str,
        unmasked_renderer: &str,
        max_texture_size: Option<u32>,
    ) -> Self {
        let is_apple = matches!(os, OsType::Macos | OsType::Ios)
            || unmasked_vendor.contains("Apple")
            || unmasked_renderer.contains("Apple");
        let is_mobile = os.is_mobile()
            || unmasked_renderer.contains("Adreno")
            || unmasked_renderer.contains("Mali")
            || unmasked_renderer.contains("PowerVR");
        let max_tex = max_texture_size.unwrap_or(if os == OsType::Android { 8192 } else { 16384 });
        Self {
            max_texture_size: max_tex,
            max_cube_map_texture_size: max_tex,
            max_renderbuffer_size: max_tex,
            max_vertex_attribs: 16,
            max_vertex_uniform_vectors: if is_apple || is_mobile { 1024 } else { 4096 },
            max_varying_vectors: 30,
            max_fragment_uniform_vectors: 1024,
            max_viewport_dims: [max_tex, max_tex],
            aliased_line_width_range: [1.0, 1.0],
            aliased_point_size_range: [1.0, 1024.0],
            max_combined_texture_image_units: if is_apple || is_mobile { 32 } else { 80 },
            max_vertex_texture_image_units: 16,
            max_texture_image_units: 16,
            max_3d_texture_size: 2048,
            max_array_texture_layers: 2048,
            max_color_attachments: 8,
            max_draw_buffers: 8,
            max_uniform_buffer_bindings: 84,
            max_uniform_block_size: 65536,
            uniform_buffer_offset_alignment: 256,
            max_vertex_output_components: 64,
            max_fragment_input_components: 64,
            min_program_texel_offset: -8,
            max_program_texel_offset: 7,
            max_transform_feedback_separate_components: 4,
            shading_language_version: "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)"
                .to_string(),
            vendor: "WebKit".to_string(),
            renderer: "WebKit WebGL".to_string(),
            unmasked_vendor: unmasked_vendor.to_string(),
            unmasked_renderer: unmasked_renderer.to_string(),
        }
    }
}

/// WebGL GPU metadata including extensions and parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GpuMetadata {
    /// Unmasked WebGL vendor string (e.g. `Google Inc. (NVIDIA)` or `Apple`).
    pub vendor: String,
    /// Unmasked WebGL renderer string (e.g. `ANGLE (NVIDIA, ...) Direct3D11` or `Apple M2`).
    pub renderer: String,
    /// Supported WebGL extensions.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Full WebGL 1 & 2 parameter tables.
    pub parameters: WebGlParameters,
}

/// Standard 28+ adapter limits for WebGPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebGpuLimits {
    /// `maxTextureDimension1D`
    pub max_texture_dimension_1d: u32,
    /// `maxTextureDimension2D`
    pub max_texture_dimension_2d: u32,
    /// `maxTextureDimension3D`
    pub max_texture_dimension_3d: u32,
    /// `maxTextureArrayLayers`
    pub max_texture_array_layers: u32,
    /// `maxBindGroups`
    pub max_bind_groups: u32,
    /// `maxBindGroupsPlusVertexBuffers`
    pub max_bind_groups_plus_vertex_buffers: u32,
    /// `maxBindingsPerBindGroup`
    pub max_bindings_per_bind_group: u32,
    /// `maxDynamicUniformBuffersPerPipelineLayout`
    pub max_dynamic_uniform_buffers_per_pipeline_layout: u32,
    /// `maxDynamicStorageBuffersPerPipelineLayout`
    pub max_dynamic_storage_buffers_per_pipeline_layout: u32,
    /// `maxSampledTexturesPerShaderStage`
    pub max_sampled_textures_per_shader_stage: u32,
    /// `maxSamplersPerShaderStage`
    pub max_samplers_per_shader_stage: u32,
    /// `maxStorageBuffersPerShaderStage`
    pub max_storage_buffers_per_shader_stage: u32,
    /// `maxStorageTexturesPerShaderStage`
    pub max_storage_textures_per_shader_stage: u32,
    /// `maxUniformBuffersPerShaderStage`
    pub max_uniform_buffers_per_shader_stage: u32,
    /// `maxUniformBufferBindingSize`
    pub max_uniform_buffer_binding_size: u64,
    /// `maxStorageBufferBindingSize`
    pub max_storage_buffer_binding_size: u64,
    /// `minUniformBufferOffsetAlignment`
    pub min_uniform_buffer_offset_alignment: u32,
    /// `minStorageBufferOffsetAlignment`
    pub min_storage_buffer_offset_alignment: u32,
    /// `maxVertexBuffers`
    pub max_vertex_buffers: u32,
    /// `maxBufferSize`
    pub max_buffer_size: u64,
    /// `maxVertexAttributes`
    pub max_vertex_attributes: u32,
    /// `maxVertexBufferArrayStride`
    pub max_vertex_buffer_array_stride: u32,
    /// `maxInterStageShaderComponents`
    pub max_inter_stage_shader_components: u32,
    /// `maxInterStageShaderVariables`
    pub max_inter_stage_shader_variables: u32,
    /// `maxColorAttachments`
    pub max_color_attachments: u32,
    /// `maxColorAttachmentBytesPerSample`
    pub max_color_attachment_bytes_per_sample: u32,
    /// `maxComputeWorkgroupStorageSize`
    pub max_compute_workgroup_storage_size: u32,
    /// `maxComputeInvocationsPerWorkgroup`
    pub max_compute_invocations_per_workgroup: u32,
    /// `maxComputeWorkgroupSizeX`
    pub max_compute_workgroup_size_x: u32,
    /// `maxComputeWorkgroupSizeY`
    pub max_compute_workgroup_size_y: u32,
    /// `maxComputeWorkgroupSizeZ`
    pub max_compute_workgroup_size_z: u32,
    /// `maxComputeWorkgroupsPerDimension`
    pub max_compute_workgroups_per_dimension: u32,
}

impl Default for WebGpuLimits {
    fn default() -> Self {
        Self {
            max_texture_dimension_1d: 16384,
            max_texture_dimension_2d: 16384,
            max_texture_dimension_3d: 2048,
            max_texture_array_layers: 2048,
            max_bind_groups: 4,
            max_bind_groups_plus_vertex_buffers: 24,
            max_bindings_per_bind_group: 1000,
            max_dynamic_uniform_buffers_per_pipeline_layout: 8,
            max_dynamic_storage_buffers_per_pipeline_layout: 4,
            max_sampled_textures_per_shader_stage: 16,
            max_samplers_per_shader_stage: 16,
            max_storage_buffers_per_shader_stage: 8,
            max_storage_textures_per_shader_stage: 4,
            max_uniform_buffers_per_shader_stage: 12,
            max_uniform_buffer_binding_size: 65536,
            max_storage_buffer_binding_size: 134_217_728,
            min_uniform_buffer_offset_alignment: 256,
            min_storage_buffer_offset_alignment: 256,
            max_vertex_buffers: 8,
            max_buffer_size: 268_435_456,
            max_vertex_attributes: 16,
            max_vertex_buffer_array_stride: 2048,
            max_inter_stage_shader_components: 60,
            max_inter_stage_shader_variables: 16,
            max_color_attachments: 8,
            max_color_attachment_bytes_per_sample: 32,
            max_compute_workgroup_storage_size: 32768,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroup_size_x: 256,
            max_compute_workgroup_size_y: 256,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,
        }
    }
}

/// WebGPU metadata including `GPUAdapter` details and full limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebGpuMetadata {
    /// Adapter vendor (e.g. `nvidia`, `apple`, `intel`).
    pub vendor: String,
    /// Adapter architecture (e.g. `ampere`, `common-3`, `gen-9`).
    pub architecture: String,
    /// Device identifier string.
    pub device: String,
    /// Adapter description.
    pub description: String,
    /// Enabled WebGPU features.
    pub features: Vec<String>,
    /// WebGPU adapter limits.
    pub limits: WebGpuLimits,
}

fn detect_gpu_maker(os: OsType, vendor: &str, renderer: &str) -> &'static str {
    let lower_r = renderer.to_ascii_lowercase();
    let lower_v = vendor.to_ascii_lowercase();
    for hay in [&lower_r, &lower_v] {
        if hay.contains("apple") {
            return "apple";
        }
        if hay.contains("nvidia") || hay.contains("geforce") {
            return "nvidia";
        }
        if hay.contains("radeon") || hay.contains("amd") || hay.contains("ati ") {
            return "amd";
        }
        if hay.contains("intel") || hay.contains("iris") || hay.contains("uhd graphics") {
            return "intel";
        }
        if hay.contains("adreno") || hay.contains("qualcomm") {
            return "qualcomm";
        }
        if hay.contains("mali") || hay.contains("arm") {
            return "arm";
        }
        if hay.contains("powervr") {
            return "imgtec";
        }
    }
    match os {
        OsType::Macos | OsType::Ios => "apple",
        OsType::Android => "qualcomm",
        OsType::Linux => "intel",
        _ => "nvidia",
    }
}

fn detect_gpu_architecture(maker: &str, renderer: &str) -> &'static str {
    let lower = renderer
        .to_ascii_lowercase()
        .replace("(r)", "")
        .replace("(tm)", "");
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    match maker {
        "apple" => "metal-3",
        "nvidia" => {
            if has(&["rtx 40"]) {
                "lovelace"
            } else if has(&["rtx 30"]) {
                "ampere"
            } else if has(&["rtx 20", "gtx 16"]) {
                "turing"
            } else if has(&["gtx 10"]) {
                "pascal"
            } else {
                ""
            }
        }
        "amd" => {
            if has(&["rx 7"]) {
                "rdna-3"
            } else if has(&["rx 6"]) {
                "rdna-2"
            } else if has(&["rx 5"]) {
                "rdna-1"
            } else if has(&["vega"]) {
                "gcn-5"
            } else {
                ""
            }
        }
        "intel" => {
            if has(&["arc"]) {
                "gen-12-hp"
            } else if has(&["iris xe", "uhd graphics 7"]) {
                "gen-12-lp"
            } else if has(&["uhd graphics 6", "hd graphics 6", "iris plus"]) {
                "gen-9"
            } else {
                ""
            }
        }
        "qualcomm" => {
            if has(&["adreno 7", "adreno (tm) 7"]) {
                "adreno-7xx"
            } else if has(&["adreno 6", "adreno (tm) 6"]) {
                "adreno-6xx"
            } else {
                ""
            }
        }
        "arm" => {
            if has(&["mali-g7", "immortalis"]) {
                "valhall"
            } else if has(&["mali-g5", "mali-g3"]) {
                "bifrost"
            } else {
                ""
            }
        }
        _ => "",
    }
}

impl WebGpuMetadata {
    /// Generates WebGPU metadata for an OS and GPU pairing.
    #[must_use]
    pub fn for_gpu(os: OsType, vendor: &str, renderer: &str) -> Self {
        let maker = detect_gpu_maker(os, vendor, renderer);
        let arch = detect_gpu_architecture(maker, renderer);
        let mut features = vec![
            "depth-clip-control".to_string(),
            "depth32float-stencil8".to_string(),
            "timestamp-query".to_string(),
            "indirect-first-instance".to_string(),
            "rg11b10ufloat-renderable".to_string(),
            "float32-filterable".to_string(),
        ];
        if !matches!(os, OsType::Android) {
            features.push("texture-compression-bc".to_string());
            features.push("bgra8unorm-storage".to_string());
        }
        if matches!(maker, "apple" | "qualcomm" | "arm") || os.is_mobile() {
            features.push("texture-compression-etc2".to_string());
            features.push("texture-compression-astc".to_string());
        }
        if matches!(maker, "apple" | "nvidia" | "amd") {
            features.push("shader-f16".to_string());
        }

        Self {
            vendor: maker.to_string(),
            architecture: arch.to_string(),
            device: String::new(),
            description: renderer.to_string(),
            features,
            limits: WebGpuLimits::default(),
        }
    }
}

/// Synthetic media device information.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaDeviceInfo {
    /// Deterministic device UUID.
    pub device_id: String,
    /// Device kind (`audioinput`, `audiooutput`, `videoinput`).
    pub kind: String,
    /// Descriptive human-readable label.
    pub label: String,
    /// Deterministic group UUID.
    pub group_id: String,
}

/// Media device enumeration metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaDeviceMetadata {
    /// Enumerated audio and video devices.
    pub devices: Vec<MediaDeviceInfo>,
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "deterministic bit shifting and truncation for UUID v4 fields"
)]
fn deterministic_uuid(seed: u64, domain: &str, index: u32) -> String {
    let mut h1 = 0xcbf2_9ce4_8422_2325_u64 ^ seed;
    for &b in domain.as_bytes() {
        h1 = (h1 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h1 = (h1 ^ u64::from(index)).wrapping_mul(0x0000_0100_0000_01b3);

    let mut h2 = h1.rotate_left(31).wrapping_mul(0x517c_c1b7_2722_0a95);
    h1 = (h1 ^ (h2 >> 32)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    h2 = (h2 ^ (h1 >> 32)).wrapping_mul(0x85eb_ca6b_cce6_543f);

    let p1 = (h1 >> 32) as u32;
    let p2 = (h1 >> 16) as u16;
    let p3 = ((h1 as u16) & 0x0fff) | 0x4000;
    let p4 = (((h2 >> 48) as u16) & 0x3fff) | 0x8000;
    let p5 = h2 & 0x0000_ffff_ffff_ffff;

    format!("{p1:08x}-{p2:04x}-{p3:04x}-{p4:04x}-{p5:012x}")
}

impl MediaDeviceMetadata {
    /// Generates synthetic devices with deterministic UUIDs.
    #[must_use]
    pub fn synthetic(seed: u64, os: OsType) -> Self {
        let (audio_in_lbl, audio_out_lbl, video_in_lbl) = match os {
            OsType::Macos => (
                "Built-in Microphone",
                "Built-in Output",
                "FaceTime HD Camera",
            ),
            OsType::Linux => (
                "PulseAudio Default Audio Capture",
                "PulseAudio Default Audio Playback",
                "USB Video Device",
            ),
            OsType::Android | OsType::Ios => ("Built-in Microphone", "Built-in Speaker", "Camera"),
            _ => (
                "Default - Microphone (Realtek(R) Audio)",
                "Default - Speakers (Realtek(R) Audio)",
                "Integrated Camera (04f2:b6d9)",
            ),
        };

        let group_audio = deterministic_uuid(seed, "audio_group", 0);
        let group_video = deterministic_uuid(seed, "video_group", 1);

        let devices = vec![
            MediaDeviceInfo {
                device_id: deterministic_uuid(seed, "audioinput", 0),
                kind: "audioinput".to_string(),
                label: audio_in_lbl.to_string(),
                group_id: group_audio.clone(),
            },
            MediaDeviceInfo {
                device_id: deterministic_uuid(seed, "audiooutput", 1),
                kind: "audiooutput".to_string(),
                label: audio_out_lbl.to_string(),
                group_id: group_audio,
            },
            MediaDeviceInfo {
                device_id: deterministic_uuid(seed, "videoinput", 2),
                kind: "videoinput".to_string(),
                label: video_in_lbl.to_string(),
                group_id: group_video,
            },
        ];

        Self { devices }
    }
}

/// Battery status metadata (`navigator.getBattery`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatteryMetadata {
    /// Whether battery is currently charging.
    pub charging: bool,
    /// Seconds until full charge.
    pub charging_time: f64,
    /// Seconds until full discharge.
    pub discharging_time: f64,
    /// Battery level between 0.0 and 1.0.
    pub level: f64,
}

impl Default for BatteryMetadata {
    fn default() -> Self {
        Self {
            charging: true,
            charging_time: 0.0,
            discharging_time: f64::INFINITY,
            level: 1.0,
        }
    }
}

/// Network connection metadata (`navigator.connection`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkMetadata {
    /// Effective bandwidth estimate in megabits per second.
    pub downlink: f64,
    /// Effective connection type (`4g`, `3g`, etc.).
    pub effective_type: String,
    /// Estimated round-trip time in milliseconds.
    pub rtt: u32,
    /// Reduced data usage preference.
    pub save_data: bool,
}

impl Default for NetworkMetadata {
    fn default() -> Self {
        Self {
            downlink: 10.0,
            effective_type: "4g".to_string(),
            rtt: 50,
            save_data: false,
        }
    }
}

/// High-entropy Client Hints metadata (`navigator.userAgentData.getHighEntropyValues`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHintsHighEntropy {
    /// Low-entropy brand list (`navigator.userAgentData.brands` / `Sec-CH-UA`).
    #[serde(default)]
    pub brands: Vec<BrandVersion>,
    /// CPU architecture (`x86`, `arm`).
    pub architecture: String,
    /// Platform bitness (`64`, `32`).
    pub bitness: String,
    /// Device model string.
    pub model: String,
    /// Platform name (`Windows`, `macOS`, `Linux`, `Android`).
    pub platform: String,
    /// Platform version (e.g. `15.0.0` for Windows 11).
    pub platform_version: String,
    /// Full browser version string (`uaFullVersion`).
    #[serde(default)]
    pub ua_full_version: String,
    /// Full version list of brands.
    pub full_version_list: Vec<BrandVersion>,
    /// Whether running on mobile device.
    pub mobile: bool,
    /// Whether running under WOW64 emulation.
    pub wow64: bool,
    /// Form factors (`Desktop`, `Mobile`).
    #[serde(default)]
    pub form_factors: Vec<String>,
}

impl ClientHintsHighEntropy {
    /// Converts a core [`CoreClientHints`] structure into [`ClientHintsHighEntropy`].
    #[must_use]
    pub fn from_core(ch: &CoreClientHints) -> Self {
        let form_factors = if ch.form_factors.is_empty() {
            vec![if ch.mobile { "Mobile" } else { "Desktop" }.to_owned()]
        } else {
            ch.form_factors.clone()
        };
        Self {
            brands: ch.brands.clone(),
            architecture: ch.architecture.clone(),
            bitness: ch.bitness.clone(),
            model: ch.model.clone(),
            platform: ch.platform.clone(),
            platform_version: ch.platform_version.clone(),
            ua_full_version: ch.ua_full_version.clone(),
            full_version_list: ch.full_version_list.clone(),
            mobile: ch.mobile,
            wow64: ch.wow64,
            form_factors,
        }
    }

    /// Converts this structure into a core [`CoreClientHints`] value.
    #[must_use]
    pub fn to_core(&self) -> CoreClientHints {
        let brands = if self.brands.is_empty() {
            self.full_version_list
                .iter()
                .map(|b| {
                    let major = b.version.split('.').next().unwrap_or(&b.version);
                    BrandVersion::new(b.brand.clone(), major)
                })
                .collect()
        } else {
            self.brands.clone()
        };
        let ua_full_version = if self.ua_full_version.is_empty() {
            self.full_version_list
                .iter()
                .find(|b| b.brand == "Google Chrome" || b.brand == "Microsoft Edge")
                .or_else(|| {
                    self.full_version_list
                        .iter()
                        .find(|b| b.brand == "Chromium")
                })
                .map_or_else(String::new, |b| b.version.clone())
        } else {
            self.ua_full_version.clone()
        };
        CoreClientHints {
            brands,
            full_version_list: self.full_version_list.clone(),
            platform: self.platform.clone(),
            platform_version: self.platform_version.clone(),
            architecture: self.architecture.clone(),
            bitness: self.bitness.clone(),
            model: self.model.clone(),
            ua_full_version,
            mobile: self.mobile,
            wow64: self.wow64,
            form_factors: self.form_factors.clone(),
        }
    }

    /// Generates high-entropy Client Hints matching an OS and Chromium major version.
    #[must_use]
    pub fn for_os(os: OsType, major: u32) -> Self {
        Self::for_browser(os, false, major)
    }

    /// Generates high-entropy Client Hints for Chromium or Microsoft Edge.
    #[must_use]
    pub fn for_browser(os: OsType, is_edge: bool, major: u32) -> Self {
        let (arch, bitness, model, platform, platform_version, mobile) = match os {
            OsType::Macos => ("arm", "64", "", "macOS", "27.0.1", false),
            OsType::Linux => ("x86", "64", "", "Linux", "6.8.0", false),
            OsType::Android => ("", "", "Pixel 8", "Android", "14.0.0", true),
            OsType::Ios => ("arm", "64", "iPhone", "iOS", "26.0", true),
            _ => ("x86", "64", "", "Windows", "15.0.0", false),
        };

        let mut hints = if is_edge {
            edge_hints(major)
        } else {
            chromium_hints(major)
        };
        if !is_edge
            && hints.ua_full_version.ends_with(".0.0.0")
            && let Some(known) = known_full_version(major)
        {
            known.full_version.clone_into(&mut hints.ua_full_version);
        }

        let form_factors = vec![if mobile { "Mobile" } else { "Desktop" }.to_owned()];

        Self {
            brands: hints.brands,
            architecture: arch.to_owned(),
            bitness: bitness.to_owned(),
            model: model.to_owned(),
            platform: platform.to_owned(),
            platform_version: platform_version.to_owned(),
            ua_full_version: hints.ua_full_version,
            full_version_list: hints.full_version_list,
            mobile,
            wow64: false,
            form_factors,
        }
    }

    /// Placeholder metadata for iOS/Safari personas (which do not expose `navigator.userAgentData`).
    #[must_use]
    pub fn for_safari(os: OsType) -> Self {
        Self {
            brands: Vec::new(),
            architecture: "arm".to_owned(),
            bitness: "64".to_owned(),
            model: if os == OsType::Ios {
                "iPhone".to_owned()
            } else {
                String::new()
            },
            platform: if os == OsType::Ios {
                "iOS".to_owned()
            } else {
                "macOS".to_owned()
            },
            platform_version: if os == OsType::Ios {
                "26.0".to_owned()
            } else {
                "27.0.1".to_owned()
            },
            ua_full_version: String::new(),
            full_version_list: Vec::new(),
            mobile: os.is_mobile(),
            wow64: false,
            form_factors: vec![if os.is_mobile() { "Mobile" } else { "Desktop" }.to_owned()],
        }
    }
}

/// Display and screen characteristics with taskbar offsets and DPR.
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
    /// Screen orientation.
    pub orientation: ScreenOrientation,
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
            orientation: ScreenOrientation::default(),
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

    /// Available screen width excluding dock/taskbar.
    #[serde(default)]
    pub avail_width: Option<u32>,

    /// Available screen height excluding dock/taskbar.
    #[serde(default)]
    pub avail_height: Option<u32>,

    /// Screen device pixel ratio.
    #[serde(default)]
    pub device_pixel_ratio: Option<f64>,

    /// Screen color depth.
    #[serde(default)]
    pub color_depth: Option<u32>,

    /// Screen orientation.
    #[serde(default)]
    pub screen_orientation: Option<ScreenOrientation>,

    /// IANA time zone identifier (e.g. `America/New_York`, `Europe/Berlin`).
    #[serde(default)]
    pub timezone: Option<String>,

    /// High entropy Client Hints.
    #[serde(default)]
    pub client_hints: Option<ClientHintsHighEntropy>,

    /// Battery status.
    #[serde(default)]
    pub battery: Option<BatteryMetadata>,

    /// Network connection info.
    #[serde(default)]
    pub network: Option<NetworkMetadata>,

    /// Deterministic seed used for noise and identity generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,

    /// Optional canvas noise amplitude override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_noise_amplitude: Option<f64>,

    /// Optional audio noise amplitude override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_noise_amplitude: Option<f64>,

    /// Optional `DOMRect` jitter amplitude override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dom_rect_jitter: Option<f64>,

    /// Optional font metrics jitter amplitude override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_metrics_jitter: Option<f64>,

    /// Whether WebGPU spoofing is enabled.
    #[serde(default = "default_true")]
    pub webgpu_enabled: bool,

    /// `navigator.maxTouchPoints` override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_touch_points: Option<u32>,

    /// `navigator.doNotTrack` override (`"1"`, `"0"`, `"unspecified"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub do_not_track: Option<String>,

    /// `navigator.pdfViewerEnabled` override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf_viewer_enabled: Option<bool>,

    /// CSS `prefers-color-scheme` media feature override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<ColorScheme>,

    /// CSS `prefers-reduced-motion` media feature override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<ReducedMotion>,

    /// Synthetic `window.speechSynthesis.getVoices()` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub speech_voices: Vec<SpeechVoice>,

    /// Deterministic sub-millisecond jitter amplitude for `performance.now()`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_jitter_ms: Option<f64>,

    /// Whether `Worker` / `SharedWorker` navigator consistency spoofing is enabled.
    #[serde(default = "default_true")]
    pub worker_spoofing: bool,

    /// Optional independent seed for 2D/Offscreen canvas noise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_seed: Option<u64>,

    /// Optional independent seed for Web Audio noise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_seed: Option<u64>,

    /// Optional `StorageManager.estimate().quota` in GiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_quota_gb: Option<u64>,

    /// Optional browser chrome height offset in CSS pixels (`outerHeight - innerHeight` & `MouseEvent.screenY`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outer_chrome_height: Option<u32>,

    /// Optional minimum `window.history.length` reported on fresh tabs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_min_length: Option<u32>,
}

const fn default_true() -> bool {
    true
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
            avail_width: Some(1920),
            avail_height: Some(1040),
            device_pixel_ratio: Some(1.0),
            color_depth: Some(24),
            screen_orientation: Some(ScreenOrientation::default()),
            timezone: Some("America/New_York".to_owned()),
            client_hints: None,
            battery: Some(BatteryMetadata::default()),
            network: Some(NetworkMetadata::default()),
            seed: None,
            canvas_noise_amplitude: None,
            audio_noise_amplitude: None,
            dom_rect_jitter: None,
            font_metrics_jitter: None,
            webgpu_enabled: true,
            max_touch_points: None,
            do_not_track: None,
            pdf_viewer_enabled: None,
            color_scheme: None,
            reduced_motion: None,
            speech_voices: Vec::new(),
            performance_jitter_ms: None,
            worker_spoofing: true,
            canvas_seed: None,
            audio_seed: None,
            storage_quota_gb: None,
            outer_chrome_height: None,
            history_min_length: None,
        }
    }
}

fn parse_languages_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|part| part.split(';').next().unwrap_or(part).trim())
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn chrome_major_from_ua(ua: &str) -> Option<u32> {
    let (_, rest) = ua.split_once("Chrome/")?;
    rest.split('.').next()?.parse().ok()
}

fn extract_client_hints(fp: &Fingerprint) -> Option<ClientHintsHighEntropy> {
    if fp.client_hints().is_off() || fp.browser_type() != BrowserType::Chromium {
        return None;
    }
    if let Some(ch) = fp.client_hints().value() {
        return Some(ClientHintsHighEntropy::from_core(ch));
    }
    let ua = fp.user_agent().as_str();
    let major = fp
        .core_version()
        .or_else(|| ua.and_then(chrome_major_from_ua))
        .unwrap_or(PRESET_CHROMIUM_MAJOR);
    let is_edge = ua.is_some_and(|u| u.contains("Edg/"));
    Some(ClientHintsHighEntropy::for_browser(
        fp.os_type(),
        is_edge,
        major,
    ))
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
            builder = builder.max_touch_points(if os.is_mobile() { 5 } else { 0 });
        }
        if let Some(browser) = self.browser {
            builder = builder.browser_type(browser);
        }
        if let Some(seed) = self.seed {
            builder = builder.seed(seed);
        }
        if let Some(ua) = &self.user_agent {
            builder = builder.user_agent(ua.clone());
            if let Some(major) = chrome_major_from_ua(ua) {
                builder = builder.core_version(major);
            }
        }
        if let Some(platform) = &self.platform {
            builder = builder.platform(platform.clone());
        }
        if !self.languages.is_empty() {
            if let Some(accept_lang) = Locale::accept_language_list(&self.languages) {
                builder = builder
                    .languages(accept_lang.clone())
                    .accept_languages(accept_lang);
            } else {
                builder = builder.languages(self.languages.join(","));
            }
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
        if let (Some(aw), Some(ah)) = (self.avail_width, self.avail_height) {
            builder = builder.avail_screen((aw, ah));
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
        if let Some(gl) = self.to_webgl() {
            builder = builder.webgl(gl);
        }

        self.apply_spoof_overrides(builder).build()
    }

    fn to_webgl(&self) -> Option<WebGl> {
        let mut webgl = match (&self.webgl_vendor, &self.webgl_renderer) {
            (Some(vendor), Some(renderer)) => Some(WebGl::new(vendor.clone(), renderer.clone())),
            (None, Some(renderer)) => Some(WebGl::renderer_only(renderer.clone())),
            (Some(vendor), None) => Some(WebGl::vendor_only(vendor.clone())),
            (None, None) => None,
        };
        if let Some(ref mut gl) = webgl
            && !self.webgl_extensions.is_empty()
        {
            gl.extensions.clone_from(&self.webgl_extensions);
        }
        webgl
    }

    fn apply_spoof_overrides(&self, mut builder: FingerprintBuilder) -> FingerprintBuilder {
        if !self.fonts.is_empty() {
            builder = builder.fonts(self.fonts.clone());
        }
        if let Some(rate) = self.audio_sample_rate {
            builder = builder.audio_sample_rate(rate);
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
        if self.color_gamut.is_some() || self.hdr.is_some() {
            let mut mf = MediaFeatures::new(self.color_gamut.unwrap_or(ColorGamut::Srgb));
            if self.hdr == Some(true) {
                mf.dynamic_range = DynamicRange::High;
            }
            builder = builder.media_features(mf);
        }
        let safari_like =
            self.browser == Some(BrowserType::MobileSafari) || self.os == Some(OsType::Ios);
        if let Some(ch) = &self.client_hints {
            builder = builder.client_hints(ch.to_core());
        } else if safari_like {
            builder = builder.client_hints(Spoof::Off);
        }
        if let Some(b) = &self.battery {
            builder = builder.battery(BatteryProfile {
                charging: b.charging,
                level: b.level,
                charging_time: b.charging_time,
                discharging_time: b.discharging_time,
            });
        } else if safari_like {
            builder = builder.battery(Spoof::Off);
        }
        if let Some(n) = &self.network {
            builder = builder.connection(ConnectionProfile {
                effective_type: n.effective_type.clone(),
                rtt: n.rtt,
                downlink: n.downlink,
                save_data: n.save_data,
            });
        } else if safari_like {
            builder = builder.connection(Spoof::Off);
        }
        builder
    }

    /// Creates a [`ProfileFingerprint`] extracted from a built [`Fingerprint`].
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "maps all core Fingerprint fields into a complete ProfileFingerprint"
    )]
    pub fn from_fingerprint(fp: &Fingerprint) -> Self {
        let os = fp.os_type();
        let screen = fp.screen().value();
        let avail_screen = fp.avail_screen().value();
        let webgl = fp.webgl().value();
        let media_features = fp.media_features().value();

        let mut languages = fp
            .languages()
            .value()
            .or_else(|| fp.accept_languages().value())
            .map_or_else(Vec::new, |s| parse_languages_list(s));
        if languages.is_empty()
            && let Some(locale) = fp.locale().value()
        {
            languages.push(locale.clone());
        }

        let (width, height) = screen.map_or_else(|| os.default_screen(), |s| (s.width, s.height));
        let (avail_width, avail_height) = avail_screen.map_or_else(
            || {
                let lost = if os.is_mobile() { 0 } else { 40 };
                (width, height.saturating_sub(lost))
            },
            |a| (a.width, a.height),
        );

        let battery = if fp.battery().is_off() {
            None
        } else if let Some(b) = fp.battery().value() {
            Some(BatteryMetadata {
                charging: b.charging,
                charging_time: b.charging_time,
                discharging_time: b.discharging_time,
                level: b.level,
            })
        } else {
            Some(BatteryMetadata::default())
        };

        let network = if fp.connection().is_off() {
            None
        } else if let Some(c) = fp.connection().value() {
            Some(NetworkMetadata {
                downlink: c.downlink,
                effective_type: c.effective_type.clone(),
                rtt: c.rtt,
                save_data: c.save_data,
            })
        } else {
            Some(NetworkMetadata::default())
        };

        Self {
            os: Some(os),
            browser: Some(fp.browser_type()),
            user_agent: fp.user_agent().value().map(ToString::to_string),
            platform: fp.platform().value().map(ToString::to_string),
            languages,
            hardware_concurrency: fp.hardware_concurrency().value().copied(),
            device_memory: fp.device_memory().value().copied(),
            webgl_vendor: webgl.and_then(|w| w.vendor.clone()),
            webgl_renderer: webgl.and_then(|w| w.renderer.clone()),
            webgl_extensions: webgl.map_or_else(Vec::new, |w| w.extensions.clone()),
            canvas_noise: Some(fp.policy().canvas_noise),
            audio_noise: Some(fp.policy().audio_noise),
            webrtc_policy: fp.policy().webrtc_policy,
            color_gamut: media_features.map(|m| m.color_gamut),
            hdr: media_features
                .map(|m| matches!(m.dynamic_range, DynamicRange::High))
                .or(Some(false)),
            audio_sample_rate: fp.audio_sample_rate().or(Some(
                if matches!(os, OsType::Linux | OsType::Android) {
                    44100
                } else {
                    48000
                },
            )),
            fonts: fp.fonts().value().cloned().unwrap_or_default(),
            screen_width: Some(width),
            screen_height: Some(height),
            avail_width: Some(avail_width),
            avail_height: Some(avail_height),
            device_pixel_ratio: fp.pixel_ratio().value().copied(),
            color_depth: fp.color_depth().value().copied(),
            screen_orientation: Some(ScreenOrientation {
                orientation_type: if height > width || os.is_mobile() {
                    ScreenOrientationType::PortraitPrimary
                } else {
                    ScreenOrientationType::LandscapePrimary
                },
                angle: 0,
            }),
            timezone: fp.timezone().value().map(ToString::to_string),
            client_hints: extract_client_hints(fp),
            battery,
            network,
            seed: Some(fp.seed()),
            canvas_noise_amplitude: None,
            audio_noise_amplitude: None,
            dom_rect_jitter: None,
            font_metrics_jitter: None,
            webgpu_enabled: true,
            max_touch_points: fp.max_touch_points().value().copied(),
            do_not_track: None,
            pdf_viewer_enabled: Some(true),
            color_scheme: None,
            reduced_motion: None,
            speech_voices: SpeechVoice::defaults_for(os),
            performance_jitter_ms: None,
            worker_spoofing: true,
            canvas_seed: None,
            audio_seed: None,
            storage_quota_gb: None,
            outer_chrome_height: None,
            history_min_length: None,
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
