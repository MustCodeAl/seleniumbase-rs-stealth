//! Unified profile fingerprint representation and advanced generator.
//!
//! This module provides:
//! 1. [`ProfileFingerprint`]: A unified JSON-serializable fingerprint model whose field
//!    names align directly with the [`Fingerprint`] API methods (`user_agent`, `platform`,
//!    `languages`, `hardware_concurrency`, `device_memory`, `webgl_vendor`, `webgl_renderer`,
//!    `canvas_noise`, `audio_noise`, `webrtc_policy`) instead of obscure masked names.
//! 2. [`FingerprintGenerator`]: An enriched generator supporting empirical distributions
//!    from Scrapfly `fingerprint-generator`, Browserforge, and Apify `fingerprint-suite`
//!    (screen resolutions, DPR, orientation, taskbar offsets, WebGL 1 & 2 parameter tables,
//!    WebGPU adapter limits, high-entropy Client Hints, audio dynamics compressor,
//!    synthetic media devices, battery, and network metadata).
//! 3. [`GeneratedFingerprint`]: The output bundle combining the unified profile, the core
//!    [`Fingerprint`], HTTP/Client Hints headers, hardware signatures, and device models.

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
        let is_apple = matches!(os, OsType::Macos | OsType::Ios);
        Self {
            max_texture_size: 16384,
            max_cube_map_texture_size: 16384,
            max_renderbuffer_size: 16384,
            max_vertex_attribs: 16,
            max_vertex_uniform_vectors: if is_apple { 1024 } else { 4096 },
            max_varying_vectors: 30,
            max_fragment_uniform_vectors: 1024,
            max_viewport_dims: [16384, 16384],
            aliased_line_width_range: [1.0, 1.0],
            aliased_point_size_range: [1.0, 1024.0],
            max_combined_texture_image_units: if is_apple { 32 } else { 80 },
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

impl WebGpuMetadata {
    /// Generates WebGPU metadata for an OS and GPU pairing.
    #[must_use]
    pub fn for_gpu(os: OsType, vendor: &str, renderer: &str) -> Self {
        let (vendor_norm, arch, desc, features) = match os {
            OsType::Macos | OsType::Ios => (
                "apple".to_string(),
                "common-3".to_string(),
                renderer.to_string(),
                vec![
                    "depth-clip-control".to_string(),
                    "depth32float-stencil8".to_string(),
                    "texture-compression-bc".to_string(),
                    "texture-compression-etc2".to_string(),
                    "texture-compression-astc".to_string(),
                    "timestamp-query".to_string(),
                    "indirect-first-instance".to_string(),
                    "shader-f16".to_string(),
                    "rg11b10ufloat-renderable".to_string(),
                    "bgra8unorm-storage".to_string(),
                    "float32-filterable".to_string(),
                ],
            ),
            OsType::Linux => (
                "intel".to_string(),
                "gen-9".to_string(),
                "Intel(R) UHD Graphics 630".to_string(),
                vec![
                    "depth-clip-control".to_string(),
                    "depth32float-stencil8".to_string(),
                    "texture-compression-bc".to_string(),
                    "timestamp-query".to_string(),
                    "indirect-first-instance".to_string(),
                    "rg11b10ufloat-renderable".to_string(),
                    "bgra8unorm-storage".to_string(),
                    "float32-filterable".to_string(),
                ],
            ),
            _ => {
                if vendor.contains("NVIDIA") || renderer.contains("NVIDIA") {
                    (
                        "nvidia".to_string(),
                        "ampere".to_string(),
                        "NVIDIA GeForce RTX 3060".to_string(),
                        vec![
                            "depth-clip-control".to_string(),
                            "depth32float-stencil8".to_string(),
                            "texture-compression-bc".to_string(),
                            "timestamp-query".to_string(),
                            "indirect-first-instance".to_string(),
                            "shader-f16".to_string(),
                            "rg11b10ufloat-renderable".to_string(),
                            "bgra8unorm-storage".to_string(),
                            "float32-filterable".to_string(),
                        ],
                    )
                } else {
                    (
                        "intel".to_string(),
                        "gen-9".to_string(),
                        "Intel(R) UHD Graphics 630".to_string(),
                        vec![
                            "depth-clip-control".to_string(),
                            "depth32float-stencil8".to_string(),
                            "texture-compression-bc".to_string(),
                            "timestamp-query".to_string(),
                            "indirect-first-instance".to_string(),
                            "rg11b10ufloat-renderable".to_string(),
                            "float32-filterable".to_string(),
                        ],
                    )
                }
            }
        };

        Self {
            vendor: vendor_norm,
            architecture: arch,
            device: String::new(),
            description: desc,
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

/// Browser brand version for Client Hints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrandVersion {
    /// Brand name.
    pub brand: String,
    /// Version string.
    pub version: String,
}

/// High-entropy Client Hints metadata (`navigator.userAgentData.getHighEntropyValues`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHintsHighEntropy {
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
    /// Full version list of brands.
    pub full_version_list: Vec<BrandVersion>,
    /// Whether running on mobile device.
    pub mobile: bool,
    /// Whether running under WOW64 emulation.
    pub wow64: bool,
}

impl ClientHintsHighEntropy {
    /// Generates high entropy client hints matching an OS and browser major version.
    #[must_use]
    pub fn for_os(os: OsType, major: u32) -> Self {
        let (arch, bitness, model, platform, platform_version, mobile) = match os {
            OsType::Macos => (
                "arm".to_string(),
                "64".to_string(),
                String::new(),
                "macOS".to_string(),
                "14.5.0".to_string(),
                false,
            ),
            OsType::Linux => (
                "x86".to_string(),
                "64".to_string(),
                String::new(),
                "Linux".to_string(),
                "6.5.0".to_string(),
                false,
            ),
            OsType::Android => (
                "arm".to_string(),
                "64".to_string(),
                "Pixel 8".to_string(),
                "Android".to_string(),
                "14.0.0".to_string(),
                true,
            ),
            OsType::Ios => (
                "arm".to_string(),
                "64".to_string(),
                "iPhone".to_string(),
                "iOS".to_string(),
                "17.5.0".to_string(),
                true,
            ),
            _ => (
                "x86".to_string(),
                "64".to_string(),
                String::new(),
                "Windows".to_string(),
                "15.0.0".to_string(),
                false,
            ),
        };

        let full_version_list = vec![
            BrandVersion {
                brand: "Not_A Brand".to_string(),
                version: "8.0.0.0".to_string(),
            },
            BrandVersion {
                brand: "Chromium".to_string(),
                version: format!("{major}.0.7163.0"),
            },
            BrandVersion {
                brand: "Google Chrome".to_string(),
                version: format!("{major}.0.7163.0"),
            },
        ];

        Self {
            architecture: arch,
            bitness,
            model,
            platform,
            platform_version,
            full_version_list,
            mobile,
            wow64: false,
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

        let (width, height) = screen.map_or((1920, 1080), |s| (s.width, s.height));
        let avail_height = if fp.os_type() == OsType::Macos {
            height.saturating_sub(25)
        } else {
            height.saturating_sub(40)
        };

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
            screen_width: Some(width),
            screen_height: Some(height),
            avail_width: Some(width),
            avail_height: Some(avail_height),
            device_pixel_ratio: fp.pixel_ratio().value().copied(),
            color_depth: fp.color_depth().value().copied(),
            screen_orientation: Some(ScreenOrientation {
                orientation_type: if fp.os_type().is_mobile() {
                    ScreenOrientationType::PortraitPrimary
                } else {
                    ScreenOrientationType::LandscapePrimary
                },
                angle: 0,
            }),
            timezone: fp.timezone().value().map(ToString::to_string),
            client_hints: Some(ClientHintsHighEntropy::for_os(fp.os_type(), 157)),
            battery: Some(BatteryMetadata::default()),
            network: Some(NetworkMetadata::default()),
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

// ---------------------------------------------------------------------------
// Comprehensive Font Lists per OS
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

    /// Generates a coherent [`GeneratedFingerprint`] bundle with full parameters.
    #[must_use]
    pub fn generate(&self) -> GeneratedFingerprint {
        let (gpu_vendor, gpu_renderer, gpu_extensions, webgl_params) = self.generate_gpu();
        let webgpu = WebGpuMetadata::for_gpu(self.os, &gpu_vendor, &gpu_renderer);
        let display = self.generate_display();
        let audio = self.generate_audio();
        let media_devices = MediaDeviceMetadata::synthetic(self.seed, self.os);
        let battery = self.generate_battery();
        let network = Self::generate_network();
        let fonts = self.generate_fonts();
        let (user_agent, platform) = self.generate_navigator();
        let client_hints = ClientHintsHighEntropy::for_os(self.os, self.major);
        let headers = self.generate_headers(&user_agent, &client_hints);

        let profile = ProfileFingerprint {
            os: Some(self.os),
            browser: Some(self.browser),
            user_agent: Some(user_agent),
            platform: Some(platform),
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
            avail_width: Some(display.avail_width),
            avail_height: Some(display.avail_height),
            device_pixel_ratio: Some(display.pixel_ratio),
            color_depth: Some(display.color_depth),
            screen_orientation: Some(display.orientation),
            timezone: Some("America/New_York".to_owned()),
            client_hints: Some(client_hints.clone()),
            battery: Some(battery),
            network: Some(network.clone()),
        };

        // Produce a verified core Fingerprint
        let fingerprint = Fingerprint::randomized_with_major(self.os, self.seed, self.major);

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

    fn generate_headers(
        &self,
        user_agent: &str,
        client_hints: &ClientHintsHighEntropy,
    ) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        headers.insert("User-Agent".to_owned(), user_agent.to_owned());
        headers.insert("Accept-Language".to_owned(), "en-US,en;q=0.9".to_owned());
        headers.insert(
            "Sec-CH-UA".to_owned(),
            format!(
                "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"{m}\", \"Google Chrome\";v=\"{m}\"",
                m = self.major
            ),
        );
        headers.insert(
            "Sec-CH-UA-Mobile".to_owned(),
            if client_hints.mobile {
                "?1".to_owned()
            } else {
                "?0".to_owned()
            },
        );
        headers.insert(
            "Sec-CH-UA-Platform".to_owned(),
            format!("\"{}\"", client_hints.platform),
        );
        headers.insert(
            "Sec-CH-UA-Platform-Version".to_owned(),
            format!("\"{}\"", client_hints.platform_version),
        );
        headers.insert(
            "Sec-CH-UA-Arch".to_owned(),
            format!("\"{}\"", client_hints.architecture),
        );
        headers.insert(
            "Sec-CH-UA-Bitness".to_owned(),
            format!("\"{}\"", client_hints.bitness),
        );
        headers.insert(
            "Sec-CH-UA-Model".to_owned(),
            format!("\"{}\"", client_hints.model),
        );
        headers.insert(
            "Sec-CH-UA-Full-Version-List".to_owned(),
            format!(
                "\"Not_A Brand\";v=\"8.0.0.0\", \"Chromium\";v=\"{m}.0.7163.0\", \"Google Chrome\";v=\"{m}.0.7163.0\"",
                m = self.major
            ),
        );
        headers
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

    fn generate_gpu(&self) -> (String, String, Vec<String>, WebGlParameters) {
        let (vendor, renderer, extensions) = match self.os {
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
        };

        let params = WebGlParameters::for_gpu(self.os, &vendor, &renderer);
        (vendor, renderer, extensions, params)
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
                orientation: ScreenOrientation {
                    orientation_type: ScreenOrientationType::LandscapePrimary,
                    angle: 0,
                },
            },
            OsType::Android => DisplayMetadata {
                width: 412,
                height: 915,
                avail_width: 412,
                avail_height: 915,
                pixel_ratio: 2.625,
                color_depth: 24,
                color_gamut: gamut,
                hdr,
                orientation: ScreenOrientation {
                    orientation_type: ScreenOrientationType::PortraitPrimary,
                    angle: 0,
                },
            },
            OsType::Ios => DisplayMetadata {
                width: 390,
                height: 844,
                avail_width: 390,
                avail_height: 844,
                pixel_ratio: 3.0,
                color_depth: 30,
                color_gamut: gamut,
                hdr,
                orientation: ScreenOrientation {
                    orientation_type: ScreenOrientationType::PortraitPrimary,
                    angle: 0,
                },
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
                orientation: ScreenOrientation {
                    orientation_type: ScreenOrientationType::LandscapePrimary,
                    angle: 0,
                },
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
            base_latency: 0.005,
            compressor: AudioDynamicsCompressor::default(),
        }
    }

    fn generate_battery(&self) -> BatteryMetadata {
        match self.os {
            OsType::Android | OsType::Ios => BatteryMetadata {
                charging: false,
                charging_time: f64::INFINITY,
                discharging_time: 18000.0,
                level: 0.85,
            },
            _ => BatteryMetadata {
                charging: true,
                charging_time: 0.0,
                discharging_time: f64::INFINITY,
                level: 1.0,
            },
        }
    }

    fn generate_network() -> NetworkMetadata {
        NetworkMetadata {
            downlink: 10.0,
            effective_type: "4g".to_string(),
            rtt: 50,
            save_data: false,
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
                "orientation": {
                    "type": self.display.orientation.orientation_type.as_str(),
                    "angle": self.display.orientation.angle,
                },
            },
            "gpu": {
                "vendor": self.gpu.vendor,
                "renderer": self.gpu.renderer,
                "unmaskedVendor": self.gpu.vendor,
                "unmaskedRenderer": self.gpu.renderer,
                "extensions": self.gpu.extensions,
                "parameters": self.gpu.parameters,
            },
            "webgpu": {
                "vendor": self.webgpu.vendor,
                "architecture": self.webgpu.architecture,
                "description": self.webgpu.description,
                "features": self.webgpu.features,
                "limits": self.webgpu.limits,
            },
            "audio": {
                "sampleRate": self.audio.sample_rate,
                "maxChannelCount": self.audio.max_channel_count,
                "outputLatency": self.audio.output_latency,
            },
            "mediaDevices": self.media_devices.devices,
            "battery": self.battery,
            "network": self.network,
            "fonts": self.fonts,
        })
    }

    /// Serializes this bundle into a JSON structure compatible with Browserforge.
    #[must_use]
    pub fn to_browserforge_json(&self) -> serde_json::Value {
        serde_json::json!({
            "screen": {
                "width": self.display.width,
                "height": self.display.height,
                "availWidth": self.display.avail_width,
                "availHeight": self.display.avail_height,
                "colorDepth": self.display.color_depth,
                "pixelDepth": self.display.color_depth,
                "devicePixelRatio": self.display.pixel_ratio,
            },
            "navigator": {
                "userAgent": self.profile.user_agent,
                "platform": self.profile.platform,
                "languages": self.profile.languages,
                "hardwareConcurrency": self.profile.hardware_concurrency,
                "deviceMemory": self.profile.device_memory,
            },
            "headers": self.headers,
            "videoCard": {
                "vendor": self.gpu.vendor,
                "renderer": self.gpu.renderer,
            },
            "battery": self.battery,
        })
    }

    /// Serializes this bundle into a JSON structure compatible with Apify fingerprint-suite.
    #[must_use]
    pub fn to_apify_json(&self) -> serde_json::Value {
        serde_json::json!({
            "fingerprint": {
                "screen": {
                    "width": self.display.width,
                    "height": self.display.height,
                    "availWidth": self.display.avail_width,
                    "availHeight": self.display.avail_height,
                    "colorDepth": self.display.color_depth,
                    "pixelDepth": self.display.color_depth,
                    "devicePixelRatio": self.display.pixel_ratio,
                },
                "navigator": {
                    "userAgent": self.profile.user_agent,
                    "platform": self.profile.platform,
                    "languages": self.profile.languages,
                    "hardwareConcurrency": self.profile.hardware_concurrency,
                    "deviceMemory": self.profile.device_memory,
                },
                "webgl": {
                    "vendor": self.gpu.vendor,
                    "renderer": self.gpu.renderer,
                    "parameters": self.gpu.parameters,
                },
                "audio": {
                    "sampleRate": self.audio.sample_rate,
                },
                "battery": self.battery,
            },
            "headers": self.headers,
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
        assert_ne!(bundle.gpu.extensions.len(), 0);
        assert!(bundle.fonts.contains(&"Helvetica Neue".to_string()));

        let scrapfly_json = bundle.to_scrapfly_json();
        assert_eq!(scrapfly_json["os"], "macOS");
        assert_eq!(scrapfly_json["screen"]["colorGamut"], "p3");
        assert_eq!(scrapfly_json["audio"]["sampleRate"], 44100);
    }

    #[test]
    fn generator_creates_webgl_parameters_and_webgpu_limits() {
        let generator = FingerprintGenerator::new(101)
            .os(OsType::Windows)
            .major(157);
        let bundle = generator.generate();

        assert_eq!(bundle.gpu.parameters.max_texture_size, 16384);
        assert_eq!(bundle.gpu.parameters.max_viewport_dims, [16384, 16384]);
        assert_eq!(bundle.webgpu.vendor, "nvidia");
        assert_eq!(bundle.webgpu.limits.max_texture_dimension_2d, 16384);
        assert_eq!(bundle.display.avail_height, 1040);
        assert_eq!(
            bundle.display.orientation.orientation_type,
            ScreenOrientationType::LandscapePrimary
        );
        assert_eq!(bundle.media_devices.devices.len(), 3);
        assert_eq!(bundle.client_hints.platform, "Windows");
    }
}
