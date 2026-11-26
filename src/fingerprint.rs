//! Unified profile fingerprint representation and advanced generator.
//!
//! This module provides:
//! 1. [`ProfileFingerprint`]: A unified JSON-serializable fingerprint model whose field
//!    names align directly with the [`Fingerprint`] API methods (`user_agent`, `platform`,
//!    `languages`, `hardware_concurrency`, `device_memory`, `webgl_vendor`, `webgl_renderer`,
//!    `canvas_noise`, `audio_noise`, `webrtc_policy`) with zero legacy aliases.
//! 2. [`FingerprintGenerator`]: A seeded generator producing correlated display geometry,
//!    color gamut, HDR, GPU/WebGL 1 & 2 parameter tables, WebGPU adapter limits and info,
//!    high-entropy Client Hints with GREASE brands and known Chromium builds, Web Audio
//!    metadata, synthetic media devices, battery, and network profiles.
//! 3. [`GeneratedFingerprint`]: The output bundle combining the unified profile, the core
//!    [`Fingerprint`], HTTP/Client Hints headers, hardware signatures, and device models.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

pub use seleniumbase_rs::evasion::fingerprint::BrandVersion;
use seleniumbase_rs::evasion::fingerprint::{
    AudioNoise, BatteryProfile, BrowserType, CanvasNoise, ClientHints as CoreClientHints,
    ConnectionProfile, Fingerprint, FingerprintBuilder, FingerprintError, OsType, Spoof,
    WebRtcPolicy,
};
use seleniumbase_rs::evasion::randomize::{
    chromium_hints, edge_hints, known_full_version, Locale, PRESET_CHROMIUM_MAJOR,
};
use seleniumbase_rs::evasion::settings::{
    ColorGamut, DynamicRange, Geolocation, MediaFeatures, WebGl,
};

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

impl fmt::Display for HdrMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.media_value())
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

/// Synthetic `window.speechSynthesis.getVoices()` entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechVoice {
    /// Voice display name (e.g. `"Microsoft David - English (United States)"`).
    pub name: String,
    /// BCP 47 language tag (e.g. `"en-US"`).
    pub lang: String,
    /// Voice URI (defaults to `name`).
    pub voice_uri: String,
    /// Whether the voice is a local OS voice.
    pub local_service: bool,
    /// Whether this voice is the default voice.
    pub default: bool,
}

impl SpeechVoice {
    /// Creates a local speech synthesis voice with `voice_uri` equal to `name`.
    #[must_use]
    pub fn new(name: impl Into<String>, lang: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            voice_uri: name.clone(),
            name,
            lang: lang.into(),
            local_service: true,
            default: false,
        }
    }

    /// Returns a realistic OS-matched default voice list.
    #[must_use]
    pub fn defaults_for(os: OsType) -> Vec<Self> {
        let mut voices = match os {
            OsType::Macos | OsType::Ios => vec![
                Self::new("Samantha", "en-US"),
                Self::new("Alex", "en-US"),
                Self::new("Victoria", "en-US"),
                Self::new("Daniel", "en-GB"),
            ],
            OsType::Windows => vec![
                Self::new("Microsoft David - English (United States)", "en-US"),
                Self::new("Microsoft Zira - English (United States)", "en-US"),
                Self::new("Microsoft Mark - English (United States)", "en-US"),
                Self::new("Google US English", "en-US"),
            ],
            OsType::Linux | OsType::Android | _ => vec![
                Self::new("Google US English", "en-US"),
                Self::new("Google UK English Female", "en-GB"),
                Self::new("Google UK English Male", "en-GB"),
            ],
        };
        if let Some(first) = voices.first_mut() {
            first.default = true;
        }
        voices
    }
}

/// CSS `prefers-color-scheme` media feature value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorScheme {
    /// Light theme (`(prefers-color-scheme: light)`).
    Light,
    /// Dark theme (`(prefers-color-scheme: dark)`).
    Dark,
    /// No explicit preference (`(prefers-color-scheme: no-preference)`).
    NoPreference,
}

impl ColorScheme {
    /// Returns the CSS media query token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::NoPreference => "no-preference",
        }
    }
}

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
                if clamped >= 0.999 {
                    0.0
                } else {
                    1800.0
                }
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
            min_version: None,
            max_version: None,
            locales: None,
            timezone: None,
            geolocation: None,
            screen: None,
            webgl_override: None,
            webgl_params: None,
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

        let (gpu_vendor, gpu_renderer, gpu_extensions, mut webgl_params) =
            Self::build_gpu(&base_fp);
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

        let user_agent = profile.user_agent.clone().unwrap_or_default();
        let client_hints = profile
            .client_hints
            .clone()
            .unwrap_or_else(|| ClientHintsHighEntropy::for_safari(os));
        let headers = Self::generate_headers(
            &base_fp,
            &user_agent,
            profile.client_hints.as_ref(),
            &profile.languages,
        );

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
            assert!(win_bundle
                .profile
                .languages
                .iter()
                .all(|l| !l.contains(";q=")));
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
            assert!(bundle
                .client_hints
                .full_version_list
                .iter()
                .any(|b| b.brand == "Google Chrome" && b.version == expected_full));
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
        assert!(!android
            .gpu
            .extensions
            .iter()
            .any(|e| e.contains("s3tc") || e.contains("bptc") || e.contains("rgtc")));
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
