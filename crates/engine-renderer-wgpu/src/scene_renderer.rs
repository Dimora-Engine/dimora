use std::collections::{HashMap, HashSet};

use bytemuck::{Pod, Zeroable};
use engine_assets::{
    validate_texture_dimensions, AddressMode, AssetHandle, AssetId, AssetManager, BaseLevelUpload, ColorSpace,
    MeshData, MipFilter, SamplerDesc, TexelRect, Texture, TextureDimensionLimits, TextureEdit, TextureFilter,
    TexturePixelEdits,
};
use engine_math::Mat4;
use engine_renderer::{
    select_msaa, CameraClear, CameraOutput, CameraPass, CameraPresentation, DebugSegment, MeshDraw, MeshInstance,
    MsaaError, MsaaSamples, MsaaSkip, PresentationFit, RenderScene, RenderTargetDesc, RenderTargetId, ScaledImage,
    SpriteBatch, SpriteLayer, SpritePipelineKind, TransparentBatch, DEPTH_VIEW_EMPTY, DEPTH_VIEW_MIN_SPAN,
    DEPTH_VIEW_UNSUPPORTED,
};
use wgpu::util::DeviceExt;

use crate::mip::{self, MipGenerator};

/// LOD clamp for a texture whose sampling view contains only the base level.
///
/// This is wgpu 30.0.1's default `lod_max_clamp`. Clamping to `0..0` makes minification use the
/// magnification filter: the clamped LOD is what selects min versus mag, and Metal turns nearest
/// mip filtering plus `0..0` into a non-mipmapped sampler that ignores `minFilter`. The texture
/// view still contains only level 0, so a wider clamp does not sample a mip that was not allocated.
const SINGLE_LEVEL_LOD_MAX: f32 = 32.0;

use crate::gpu_types::{GpuDebugVertex, GpuGlobals, GpuMeshInstance, GpuSpriteInstance};
use crate::{GpuContext, RenderError};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Per-frame counters, handy for HUDs and for asserting batching in tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Every `draw` and `draw_indexed`, including viewport color clears, depth-view draws,
    /// 4× resolves, and load expansions. A one-sample frame has none of the last two, so its total
    /// matches the world and clear draws.
    pub draw_calls: u32,
    /// Scene draws only. A viewport clear is not one of these.
    pub world_draw_calls: u32,
    /// Viewport color fills for later cameras. The first camera uses the attachment clear and
    /// does not add one.
    pub clear_draw_calls: u32,
    /// Active camera passes submitted this frame.
    pub camera_passes: u32,
    /// Passes whose output is the surface.
    pub surface_passes: u32,
    /// Passes whose output is an offscreen target.
    pub offscreen_passes: u32,
    pub sprite_instances: u32,
    pub mesh_instances: u32,
    /// Instances passed to the draw that submitted the most instances. This is the range given to
    /// `draw` or `draw_indexed`, not a count of CPU batch objects. Viewport clears are not included.
    pub largest_draw_instances: u32,
    /// Sum of the instance ranges passed to world draw calls. An indexed mesh draw counts
    /// instances, not indices. A missing GPU mesh that is skipped is not included. A sprite drawn
    /// by two cameras counts twice. A debug line draw counts its vertices, two per segment, and
    /// counts again for each camera that draws the batch. Viewport clears are not included.
    /// A depth-view draw is not included.
    pub submitted_instances: u32,
    /// One per camera whose presentation is [`CameraPresentation::Depth`]. Not a world draw and
    /// not a viewport clear. Zero when every camera is shaded, so a shaded frame's
    /// [`Self::draw_calls`] stays the world and clear total.
    pub depth_view_draws: u32,
    /// Nearest blits of an internal camera image. Not a world draw and not a viewport clear.
    /// Zero when no camera uses an internal resolution that fits in its viewport.
    pub presentation_draws: u32,
    /// Shader resolves of a 4× color image into one camera viewport. Not a world draw.
    /// Zero when the renderer is at one sample, and zero for a depth presentation.
    pub resolve_draws: u32,
    /// Copies of an already-resolved destination into the four color samples of a later
    /// [`CameraClear::Load`] viewport. Not a world draw. Zero at one sample, for a first writer,
    /// for [`CameraClear::Color`], and for a depth presentation.
    pub expand_draws: u32,
    /// Cameras that were not drawn because their 4× images were not allocated. Zero at one sample.
    /// The selected count is not changed, and the camera is not drawn at one sample.
    pub msaa_skipped: u32,
}

/// GPU submission counts for one camera pass. Summed into [`FrameStats`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassFrameStats {
    pub world_draw_calls: u32,
    pub clear_draw_calls: u32,
    pub sprite_instances: u32,
    pub mesh_instances: u32,
    pub submitted_instances: u32,
    pub depth_view_draws: u32,
    pub presentation_draws: u32,
    pub resolve_draws: u32,
    pub expand_draws: u32,
}

/// Frames a cached texture or mesh may go unused before it is evicted, unless configured otherwise
/// (see [`SceneRenderer::set_cache_max_idle_frames`]). About two seconds at 60 frames per second.
pub const DEFAULT_CACHE_MAX_IDLE_FRAMES: u32 = 120;

/// How many assets currently have a GPU copy in the renderer's caches. The built-in placeholder
/// textures are not counted, so an idle renderer reports zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub textures: usize,
    pub meshes: usize,
}

/// Lifetime counts for GPU image storage, uploads, samplers, and bind groups.
///
/// Placeholder images created with the renderer are included. A later frame that only changes a
/// sampler increases [`Self::bind_group_creates`] and leaves [`Self::texture_creates`] and
/// [`Self::texture_writes`] unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuResourceStats {
    pub texture_creates: u64,
    pub texture_writes: u64,
    pub sampler_creates: u64,
    pub bind_group_creates: u64,
    /// Pixel buffers hashed because an asset revision changed. Unchanged revisions are not hashed.
    pub pixel_fingerprints: u64,
    /// Times anisotropy above 1 was forced to 1 because `DownlevelFlags::ANISOTROPIC_FILTERING` is absent.
    /// Incremented once per renderer, when the fallback is first logged.
    pub anisotropy_capability_fallbacks: u64,
    /// Times a sampler with anisotropy below 1 was forced to 1 before GPU creation.
    /// Incremented once per renderer, when the fallback is first logged.
    pub invalid_anisotropy_fallbacks: u64,
    /// Chains filled by the private blit. A one-level texture does not increment this.
    /// Sampler-only changes do not increment it. A partial base upload that rebuilds the whole
    /// chain increments it once. That rebuild is not a partial mip update.
    pub mip_generations: u64,
    /// Times a non-power-of-two `Mipmaps::Generate` texture was kept at one level because
    /// `DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` is absent. Once per asset name.
    pub npot_mip_fallbacks: u64,
    /// Whole base-level `write_texture` calls. The two placeholder images are included.
    pub full_base_uploads: u64,
    /// Rectangle `write_texture` calls. One per selected region, not per asset revision.
    pub partial_base_uploads: u64,
    /// Logical source bytes submitted to [`Self::full_base_uploads`] and [`Self::partial_base_uploads`].
    /// This is not measured GPU memory bandwidth, and a mip-chain rebuild is not added.
    pub base_upload_bytes: u64,
    /// Times an instance buffer was replaced because the frame's bytes did not fit. The buffer
    /// created with the renderer is not counted. This is not a GPU timing measurement.
    pub instance_buffer_grows: u64,
    /// Offscreen color images created for a [`RenderTargetId`] that had no GPU image. This is not
    /// a CPU pixel upload and does not increment [`Self::texture_creates`] or [`Self::texture_writes`].
    pub render_target_creates: u64,
    /// Offscreen color and depth images replaced because the description's size or generation changed.
    /// The previous image is dropped. wgpu keeps it alive until earlier submissions finish.
    pub render_target_reallocs: u64,
    /// Private internal-resolution color images created. The matching depth image is not a second count.
    pub internal_target_creates: u64,
    /// Private internal images replaced because that camera's internal size changed.
    pub internal_target_reallocs: u64,
    /// Private internal images dropped because the camera no longer presents one this frame.
    pub internal_target_drops: u64,
    /// Times the 4× scene pipelines were created. The first successful [`MsaaSamples::Four`]
    /// increments this once. A later `Four` does not create them again. Switching to
    /// [`MsaaSamples::One`] keeps the pipelines and does not increment this.
    pub msaa_pipeline_creates: u64,
    /// 4× color images created. The matching depth image is not a second count. A surface image,
    /// an offscreen target, and an internal-resolution image each count once.
    pub msaa_image_creates: u64,
    /// 4× color and depth images replaced because that destination's size changed.
    pub msaa_image_reallocs: u64,
    /// 4× images dropped because the renderer returned to one sample, a destination was removed
    /// or resized, or an internal camera stopped scaling.
    pub msaa_image_drops: u64,
}

/// 4× color and depth bytes retained for every live destination.
///
/// Bytes are `width * height * 4 * 4` per image: four bytes per sample and four samples, summed
/// across the surface, offscreen targets, and internal-resolution cameras. This is a size
/// calculation, not a measured allocation from the driver. [`Self::width`] and [`Self::height`]
/// describe the surface image when one is retained, and are zero when only a private image is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MsaaImageUsage {
    pub width: u32,
    pub height: u32,
    pub color_bytes: u64,
    pub depth_bytes: u64,
    /// Retained 4× color images. Each has a matching depth image that is not a second count.
    pub image_count: u32,
}

/// Private internal-resolution images retained after the latest frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InternalTargetUsage {
    pub cameras: u32,
    /// `width * height * 4` summed across retained color images.
    pub color_bytes: u64,
    /// `width * height * 4` summed across the matching `Depth32Float` images.
    pub depth_bytes: u64,
}

/// Effective GPU sampling and storage for one cached asset texture.
///
/// This is the configuration passed to `Device::create_sampler` and the image that was allocated,
/// after CPU normalization, the device anisotropy fallback, and the LOD clamp for the allocated chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CachedTextureInfo {
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
    pub mip_level_count: u32,
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    /// `false` when the CPU mip filter is absent or nearest. Both share one GPU sampler.
    pub mip_filter_linear: bool,
    pub address_u: AddressMode,
    pub address_v: AddressMode,
    pub anisotropy: u8,
    pub lod_min: f32,
    pub lod_max: f32,
    pub pixel_fingerprint: u64,
}

/// Identity of one GPU sampler after normalization, device fallback, and LOD clamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SamplerKey {
    min_linear: bool,
    mag_linear: bool,
    mip_linear: bool,
    address_u: AddressMode,
    address_v: AddressMode,
    anisotropy: u8,
    lod_min_bits: u32,
    lod_max_bits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct EffectiveSampling {
    key: SamplerKey,
    min_filter: TextureFilter,
    mag_filter: TextureFilter,
    mip_linear: bool,
    address_u: AddressMode,
    address_v: AddressMode,
    anisotropy: u8,
    lod_min: f32,
    lod_max: f32,
    /// Mip levels in the view the scene samples. `1` keeps the base level even when storage holds a chain.
    view_levels: u32,
    /// Anisotropy above 1 was requested and the device flag is absent.
    capability_fallback: bool,
    /// The CPU descriptor had anisotropy below 1.
    invalid_anisotropy: bool,
}

struct GpuTexture {
    revision: u32,
    /// Value of `SceneRenderer::frame` when a frame last drew with this texture.
    last_used: u64,
    fingerprint: u64,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    mip_level_count: u32,
    sampling: EffectiveSampling,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
}

struct GpuMesh {
    revision: u32,
    last_used: u64,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

/// One depth image. The attachment view is bound by the scene pass. The sample view is bound by
/// the depth presentation pass. Those are different views of one image so the attachment is not
/// also a binding in the same pass.
///
/// `sample_bind_group` is created the first time a camera presents this image as depth. A shaded
/// camera does not create it. The texture is always `RENDER_ATTACHMENT | TEXTURE_BINDING`, so
/// turning the presentation on does not allocate a second depth image.
struct DepthImage {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    attachment: wgpu::TextureView,
    sample: wgpu::TextureView,
    sample_bind_group: Option<wgpu::BindGroup>,
    size: (u32, u32),
}

/// One offscreen color image plus the depth image cleared for every pass that writes it.
///
/// Sprites sample the color view. The depth image is sampled only by a depth presentation pass
/// for a camera that writes this target.
struct GpuRenderTarget {
    generation: u64,
    width: u32,
    height: u32,
    color: wgpu::Texture,
    view: wgpu::TextureView,
    depth: DepthImage,
    bind_group: wgpu::BindGroup,
    /// Multisample color and depth for cameras that write this target at 4×. The public `color`
    /// image stays one sample. Absent at one sample, and until the first 4× camera prepares it.
    msaa: Option<MsaaImages>,
}

/// Private color and depth for one camera's internal resolution. Not a [`RenderTargetId`].
struct InternalTarget {
    width: u32,
    height: u32,
    color: wgpu::Texture,
    view: wgpu::TextureView,
    depth: DepthImage,
    present_bind_group: wgpu::BindGroup,
    /// Multisample color and depth at this internal size. The present blit samples `view`.
    msaa: Option<MsaaImages>,
}

/// Growable vertex buffer rewritten every frame.
struct InstanceBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    label: &'static str,
}

impl InstanceBuffer {
    fn new(device: &wgpu::Device, label: &'static str) -> Self {
        let capacity = 4096;
        Self { buffer: Self::alloc(device, label, capacity), capacity, label }
    }
    fn alloc(device: &wgpu::Device, label: &'static str, capacity: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    /// Writes `data` into the buffer. Returns whether the buffer had to grow. An empty slice does
    /// not grow and does not write.
    fn upload<T: Pod>(&mut self, gpu: &GpuContext, data: &[T]) -> bool {
        let bytes: &[u8] = bytemuck::cast_slice(data);
        if bytes.is_empty() {
            return false;
        }
        let mut grew = false;
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.buffer = Self::alloc(&gpu.device, self.label, self.capacity);
            grew = true;
        }
        gpu.queue.write_buffer(&self.buffer, 0, bytes);
        grew
    }
}

/// Draws [`RenderScene`]s into any texture view of a given color format.
///
/// Pass order in one render pass: depth-writing meshes, opaque sprites, mask sprites, then
/// cutout sprites and tiles. Blended sprites and blended meshes follow in one back-to-front
/// sequence. A pipeline change does not pull one primitive ahead of the other. Depth testing is
/// what lets opaque and mask geometry occlude a later blend. When the frame has debug segments,
/// one line-list draw follows that sequence: reverse-Z depth test, no depth write, unlit vertex
/// color. An empty debug list does not add that draw.
///
/// A camera whose [`CameraPresentation`] is [`CameraPresentation::Depth`] stores that depth and
/// then draws one viewport triangle that samples it. The triangle replaces the camera rectangle.
/// It does not run when the camera is shaded. The scene color pass still runs first.
pub struct SceneRenderer {
    format: wgpu::TextureFormat,
    globals: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    /// Color-only viewport fill. Later cameras with [`CameraClear::Color`] draw this inside the
    /// scissor. It does not write depth and does not blend.
    clear_pipeline: wgpu::RenderPipeline,
    /// Same fill as [`Self::clear_pipeline`] with no depth state, for a presentation pass that
    /// does not attach destination depth.
    color_fill_pipeline: wgpu::RenderPipeline,
    clear_bind_group: wgpu::BindGroup,
    clear_uniform: wgpu::Buffer,
    present_pipeline: wgpu::RenderPipeline,
    present_layout: wgpu::BindGroupLayout,
    present_uniform: wgpu::Buffer,
    /// Fullscreen depth presentation. No depth state: its pass has no depth attachment.
    /// Created with the renderer. A shaded frame does not bind it.
    depth_view_pipeline: wgpu::RenderPipeline,
    depth_view_layout: wgpu::BindGroupLayout,
    depth_view_uniform: wgpu::Buffer,
    texture_layout: wgpu::BindGroupLayout,
    /// Shared samplers keyed by the effective GPU descriptor, not by entity or asset.
    samplers: HashMap<SamplerKey, wgpu::Sampler>,
    /// `DownlevelFlags::ANISOTROPIC_FILTERING` queried once from the adapter.
    anisotropic_filtering: bool,
    /// `DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` queried once from the adapter.
    npot_mipmaps: bool,
    anisotropy_capability_logged: bool,
    invalid_anisotropy_logged: bool,
    /// Formats already checked for blit support.
    blit_support: HashMap<wgpu::TextureFormat, bool>,
    /// Asset names already logged for the non-power-of-two single-level fallback.
    npot_fallback_names: HashSet<String>,
    mip_generator: Option<MipGenerator>,
    resource_stats: GpuResourceStats,
    sprite_cutout: wgpu::RenderPipeline,
    sprite_blend: wgpu::RenderPipeline,
    sprite_opaque: wgpu::RenderPipeline,
    sprite_opaque_double: wgpu::RenderPipeline,
    sprite_mask: wgpu::RenderPipeline,
    sprite_mask_double: wgpu::RenderPipeline,
    sprite_blend_culled: wgpu::RenderPipeline,
    mesh_solid: wgpu::RenderPipeline,
    mesh_solid_double: wgpu::RenderPipeline,
    mesh_mask: wgpu::RenderPipeline,
    mesh_mask_double: wgpu::RenderPipeline,
    mesh_blend: wgpu::RenderPipeline,
    mesh_blend_double: wgpu::RenderPipeline,
    /// One-pixel unlit lines. Group 0 only. Depth test on, depth write off, alpha blend.
    debug_pipeline: wgpu::RenderPipeline,
    textures: HashMap<AssetId, GpuTexture>,
    white: GpuTexture,
    missing: GpuTexture,
    /// Sampler used by every offscreen target. Targets have no asset sampler descriptor.
    target_sampler: wgpu::Sampler,
    render_targets: HashMap<RenderTargetId, GpuRenderTarget>,
    /// Keyed by [`CameraPass::camera_id`]. Entries exist only for cameras that scaled an image
    /// this frame. A size change replaces the entry. wgpu keeps the old image until earlier
    /// submissions finish.
    internal_targets: HashMap<u32, InternalTarget>,
    meshes: HashMap<AssetId, GpuMesh>,
    opaque_instances: InstanceBuffer,
    mask_instances: InstanceBuffer,
    cutout_instances: InstanceBuffer,
    blend_instances: InstanceBuffer,
    mesh_instances: InstanceBuffer,
    blend_mesh_instances: InstanceBuffer,
    debug_vertices: InstanceBuffer,
    /// Vertices uploaded for this frame's debug lines. Zero skips the line draw in every camera.
    debug_vertex_count: u32,
    depth: Option<DepthImage>,
    stats: FrameStats,
    pass_stats: Vec<PassFrameStats>,
    /// Number of `render` calls so far; the clock for idle eviction.
    frame: u64,
    cache_max_idle_frames: u32,
    /// Tighter than the device limit when set. `None` uses the device limit on both axes.
    dimension_limit_override: Option<TextureDimensionLimits>,
    /// Asset revision already refused, so a later frame does not hash or allocate it again.
    refused_uploads: HashMap<AssetId, u32>,
    /// Active sample count. [`MsaaSamples::One`] uses the pipelines created in [`Self::new`].
    msaa: MsaaSamples,
    /// 4× pipelines, created on the first successful [`Self::set_msaa`] of [`MsaaSamples::Four`].
    /// Retained after a switch back to one sample. Images inside it are not.
    msaa4: Option<Msaa4>,
    /// Scene, debug, and viewport-clear layouts shared by the one-sample and four-sample pipelines.
    scene_layout: wgpu::PipelineLayout,
    debug_layout: wgpu::PipelineLayout,
    clear_pipeline_layout: wgpu::PipelineLayout,
    /// `(camera id, reason)` already logged for a 4× skip. A later frame with the same pair is quiet.
    msaa_skip_warned: HashSet<(u32, MsaaSkip)>,
    /// The blank-destination warning has been logged since the last surface camera that drew at 4×.
    msaa_blank_logged: bool,
    /// Destination size of the latest [`Self::render`]. Letterbox and presentation use this when
    /// 4× is selected, because that path does not keep the one-sample surface depth image.
    surface_size: (u32, u32),
}

impl SceneRenderer {
    pub fn new(gpu: &GpuContext, format: wgpu::TextureFormat) -> Self {
        let device = &gpu.device;

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: core::mem::size_of::<GpuGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let debug_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("debug line layout"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let clear_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport clear"),
            size: core::mem::size_of::<ClearUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let clear_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport clear layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let clear_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport clear"),
            layout: &clear_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: clear_uniform.as_entire_binding() }],
        });
        let clear_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewport clear layout"),
            bind_group_layouts: &[Some(&clear_layout)],
            immediate_size: 0,
        });
        let draw = crate::draw_pipelines::make_draw_pipelines(
            device,
            format,
            1,
            &layout,
            &debug_layout,
            &clear_pipeline_layout,
        );
        let clear_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport clear"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/viewport_clear.wgsl").into()),
        });
        let color_fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("letterbox"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("letterbox layout"),
                bind_group_layouts: &[Some(&clear_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &clear_shader,
                entry_point: Some("vs_clear"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &clear_shader,
                entry_point: Some("fs_clear"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let present_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("internal present"),
            size: core::mem::size_of::<PresentUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("internal present layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("internal present"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/present.wgsl").into()),
        });
        let present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("internal present"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("internal present layout"),
                bind_group_layouts: &[Some(&present_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &present_shader,
                entry_point: Some("vs_present"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &present_shader,
                entry_point: Some("fs_present"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let depth_view_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("depth view"),
            size: core::mem::size_of::<DepthViewUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let depth_view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("depth view layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let depth_view_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("depth view"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/depth_view.wgsl").into()),
        });
        let depth_view_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("depth view"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("depth view layout"),
                bind_group_layouts: &[Some(&depth_view_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &depth_view_shader,
                entry_point: Some("vs_depth"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &depth_view_shader,
                entry_point: Some("fs_depth"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let downlevel = gpu.adapter.get_downlevel_capabilities().flags;
        let anisotropic_filtering = downlevel.contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING);
        let npot_mipmaps = downlevel.contains(wgpu::DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES);
        let mut resource_stats = GpuResourceStats::default();
        let mut samplers = HashMap::new();
        let default_sampling = effective_sampling(SamplerDesc::default(), anisotropic_filtering, 1);
        let default_sampler = device.create_sampler(&sampler_descriptor(&default_sampling));
        resource_stats.sampler_creates = 1;
        samplers.insert(default_sampling.key, default_sampler.clone());
        let white = upload_image(
            gpu,
            &texture_layout,
            &default_sampler,
            ImageUpload {
                sampling: default_sampling,
                width: 1,
                height: 1,
                rgba: &[255, 255, 255, 255],
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                mip_levels: 1,
                label: "white",
                fingerprint: pixel_fingerprint(&[255, 255, 255, 255]),
            },
            &mut resource_stats,
        );
        let missing = upload_image(
            gpu,
            &texture_layout,
            &default_sampler,
            ImageUpload {
                sampling: default_sampling,
                width: 1,
                height: 1,
                rgba: &[255, 0, 255, 255],
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                mip_levels: 1,
                label: "missing",
                fingerprint: pixel_fingerprint(&[255, 0, 255, 255]),
            },
            &mut resource_stats,
        );

        Self {
            format,
            globals,
            globals_bind_group,
            clear_pipeline: draw.clear,
            color_fill_pipeline,
            clear_bind_group,
            clear_uniform,
            present_pipeline,
            present_layout,
            present_uniform,
            depth_view_pipeline,
            depth_view_layout,
            depth_view_uniform,
            white,
            missing,
            target_sampler: default_sampler,
            render_targets: HashMap::new(),
            internal_targets: HashMap::new(),
            texture_layout,
            samplers,
            anisotropic_filtering,
            npot_mipmaps,
            anisotropy_capability_logged: false,
            invalid_anisotropy_logged: false,
            blit_support: HashMap::new(),
            npot_fallback_names: HashSet::new(),
            mip_generator: None,
            resource_stats,
            sprite_cutout: draw.sprite_cutout,
            sprite_blend: draw.sprite_blend,
            sprite_opaque: draw.sprite_opaque,
            sprite_opaque_double: draw.sprite_opaque_double,
            sprite_mask: draw.sprite_mask,
            sprite_mask_double: draw.sprite_mask_double,
            sprite_blend_culled: draw.sprite_blend_culled,
            mesh_solid: draw.mesh_solid,
            mesh_solid_double: draw.mesh_solid_double,
            mesh_mask: draw.mesh_mask,
            mesh_mask_double: draw.mesh_mask_double,
            mesh_blend: draw.mesh_blend,
            mesh_blend_double: draw.mesh_blend_double,
            debug_pipeline: draw.debug,
            textures: HashMap::new(),
            meshes: HashMap::new(),
            opaque_instances: InstanceBuffer::new(device, "opaque sprite instances"),
            mask_instances: InstanceBuffer::new(device, "mask sprite instances"),
            cutout_instances: InstanceBuffer::new(device, "cutout sprite instances"),
            blend_instances: InstanceBuffer::new(device, "blended sprite instances"),
            mesh_instances: InstanceBuffer::new(device, "mesh instances"),
            blend_mesh_instances: InstanceBuffer::new(device, "blended mesh instances"),
            debug_vertices: InstanceBuffer::new(device, "debug line vertices"),
            debug_vertex_count: 0,
            depth: None,
            stats: FrameStats::default(),
            pass_stats: Vec::new(),
            frame: 0,
            cache_max_idle_frames: DEFAULT_CACHE_MAX_IDLE_FRAMES,
            dimension_limit_override: None,
            refused_uploads: HashMap::new(),
            msaa: MsaaSamples::One,
            msaa4: None,
            scene_layout: layout,
            debug_layout,
            clear_pipeline_layout,
            msaa_skip_warned: HashSet::new(),
            msaa_blank_logged: false,
            surface_size: (1, 1),
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Sample count in effect. [`MsaaSamples::One`] until [`Self::set_msaa`] accepts
    /// [`MsaaSamples::Four`].
    pub fn msaa(&self) -> MsaaSamples {
        self.msaa
    }

    /// Sample count of the retained 4× pipelines.
    ///
    /// `None` until the first successful [`MsaaSamples::Four`]. The pipelines stay after a switch
    /// back to [`MsaaSamples::One`], so this can be [`Some`]`(4)` while [`Self::msaa`] is
    /// [`MsaaSamples::One`]. Images do not follow that rule: [`Self::msaa_images`] is empty at
    /// one sample.
    pub fn retained_msaa_pipeline_samples(&self) -> Option<u32> {
        self.msaa4.as_ref().map(|extra| extra.draw.sample_count)
    }

    /// 4× color and depth bytes retained right now.
    ///
    /// The surface pair, each offscreen target's pair, and each internal camera's pair are
    /// included. [`MsaaImageUsage::width`] and [`MsaaImageUsage::height`] are the surface image.
    pub fn msaa_images(&self) -> MsaaImageUsage {
        let mut usage = MsaaImageUsage::default();
        let mut add = |images: &MsaaImages, surface: bool| {
            let pixels = u64::from(images.width) * u64::from(images.height);
            usage.color_bytes += pixels * 4 * 4;
            usage.depth_bytes += pixels * 4 * 4;
            usage.image_count += 1;
            if surface {
                usage.width = images.width;
                usage.height = images.height;
            }
        };
        if let Some(images) = self.msaa4.as_ref().and_then(|extra| extra.images.as_ref()) {
            add(images, true);
        }
        for target in self.render_targets.values() {
            if let Some(images) = target.msaa.as_ref() {
                add(images, false);
            }
        }
        for internal in self.internal_targets.values() {
            if let Some(images) = internal.msaa.as_ref() {
                add(images, false);
            }
        }
        usage
    }

    /// Whether `format` and `Depth32Float` both support 4 samples on `gpu`'s adapter.
    ///
    /// Hardware resolve is not required. Stage 1 averages samples in a viewport-scoped shader.
    pub fn adapter_supports_msaa4(gpu: &GpuContext, format: wgpu::TextureFormat) -> bool {
        let color = gpu.adapter.get_texture_format_features(format).flags;
        let depth = gpu.adapter.get_texture_format_features(DEPTH_FORMAT).flags;
        msaa4_supported(color, depth)
    }

    /// Selects the sample count.
    ///
    /// [`MsaaSamples::One`] always succeeds. [`MsaaSamples::Four`] checks the adapter and, on
    /// success, builds the 4× pipelines once. A rejected request leaves [`Self::msaa`] unchanged
    /// and does not build those pipelines. Returning to one sample drops 4× images and keeps the
    /// pipelines. `wgpu`'s `create_texture` does not report an out-of-memory `Result`; a size the
    /// device limits reject is handled when a frame tries to allocate, without changing this count.
    pub fn set_msaa(&mut self, gpu: &GpuContext, samples: MsaaSamples) -> Result<(), MsaaError> {
        let color = gpu.adapter.get_texture_format_features(self.format).flags.sample_count_supported(4);
        let depth = gpu.adapter.get_texture_format_features(DEPTH_FORMAT).flags.sample_count_supported(4);
        let next = select_msaa(self.msaa, samples, color, depth)?;
        if next == MsaaSamples::Four {
            if self.msaa4.is_none() {
                self.msaa4 = Some(Msaa4::create(
                    gpu,
                    self.format,
                    &self.scene_layout,
                    &self.debug_layout,
                    &self.clear_pipeline_layout,
                ));
                self.resource_stats.msaa_pipeline_creates += 1;
            }
            // The one-sample surface depth is not attached while 4× is selected.
            self.depth = None;
        } else {
            self.drop_msaa_images();
        }
        self.msaa = next;
        Ok(())
    }

    /// Counters from the most recent [`SceneRenderer::render`].
    pub fn stats(&self) -> FrameStats {
        self.stats
    }

    /// Per-camera counters from the most recent [`SceneRenderer::render`], in draw order.
    pub fn pass_stats(&self) -> &[PassFrameStats] {
        &self.pass_stats
    }

    /// How many textures and meshes are cached on the GPU right now.
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats { textures: self.textures.len(), meshes: self.meshes.len() }
    }

    /// Counts of GPU image, upload, sampler, and bind-group work since this renderer was created.
    pub fn resource_stats(&self) -> GpuResourceStats {
        self.resource_stats
    }

    /// Bytes reserved by the per-frame instance and debug-vertex buffers.
    ///
    /// Each buffer starts at 4,096 bytes and grows to the next power of two when a frame does not
    /// fit. Empty uploads do not shrink it. This is reserved GPU buffer size, not a copy of the
    /// CPU `Vec` capacity and not measured bandwidth.
    pub fn instance_buffer_capacity_bytes(&self) -> u64 {
        self.opaque_instances.capacity
            + self.mask_instances.capacity
            + self.cutout_instances.capacity
            + self.blend_instances.capacity
            + self.mesh_instances.capacity
            + self.blend_mesh_instances.capacity
            + self.debug_vertices.capacity
    }

    /// Samplers currently retained. Shared entries stay while any cached image or placeholder uses them.
    pub fn sampler_cache_len(&self) -> usize {
        self.samplers.len()
    }

    /// Whether the adapter reported `DownlevelFlags::ANISOTROPIC_FILTERING` when this renderer was created.
    pub fn anisotropic_filtering_supported(&self) -> bool {
        self.anisotropic_filtering
    }

    /// Whether the adapter reported `DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` at creation.
    pub fn npot_mipmapped_textures_supported(&self) -> bool {
        self.npot_mipmaps
    }

    /// Effective storage and sampling for a cached asset texture. Placeholders are not included.
    pub fn cached_texture(&self, id: AssetId) -> Option<CachedTextureInfo> {
        let texture = self.textures.get(&id)?;
        Some(CachedTextureInfo {
            width: texture.width,
            height: texture.height,
            color_space: color_space_of(texture.format),
            mip_level_count: texture.mip_level_count,
            min_filter: texture.sampling.min_filter,
            mag_filter: texture.sampling.mag_filter,
            mip_filter_linear: texture.sampling.mip_linear,
            address_u: texture.sampling.address_u,
            address_v: texture.sampling.address_v,
            anisotropy: texture.sampling.anisotropy,
            lod_min: texture.sampling.lod_min,
            lod_max: texture.sampling.lod_max,
            pixel_fingerprint: texture.fingerprint,
        })
    }

    /// A cached texture or mesh that no frame has drawn with for this many consecutive frames is
    /// evicted and re-uploaded if it is needed again. Values below 1 are treated as 1; use
    /// `u32::MAX` to evict only assets that were removed from the manager.
    pub fn set_cache_max_idle_frames(&mut self, frames: u32) {
        self.cache_max_idle_frames = frames.max(1);
    }

    pub fn cache_max_idle_frames(&self) -> u32 {
        self.cache_max_idle_frames
    }

    /// Limits texture allocation to the smaller of the device `max_texture_dimension_2d` and `limits`.
    ///
    /// `None` uses the device limit for both axes. A size past the limit is not allocated. The two
    /// placeholder images are already created and are not checked again.
    pub fn set_texture_dimension_limits(&mut self, limits: Option<TextureDimensionLimits>) {
        self.dimension_limit_override = limits;
    }

    /// Tightly packed RGBA8 of one allocated mip on a cached asset texture.
    ///
    /// Placeholders are not returned. `level` must be inside the allocated chain.
    pub fn read_cached_mip(&self, gpu: &GpuContext, id: AssetId, level: u32) -> Option<Vec<u8>> {
        let cached = self.textures.get(&id)?;
        if level >= cached.mip_level_count {
            return None;
        }
        let (width, height) = mip::mip_extent(cached.width, cached.height, level);
        read_texture_level(gpu, &cached.texture, level, width, height)
    }

    /// Records and submits one frame into `target`.
    ///
    /// Each camera pass is uploaded and submitted before the next pass writes the shared globals
    /// or instance buffers. `Queue::write_buffer` copies into a staging belt that is applied on
    /// the next submit, so two writes to the same buffer before one submit would leave every draw
    /// in that submit reading the later camera. Submitting between cameras keeps camera A's
    /// matrices and instances alive until those draws have been recorded against that copy.
    ///
    /// The first pass that writes each destination clears that whole color image to that camera's
    /// clear color, whether the policy is [`CameraClear::Color`] or [`CameraClear::Load`]. A
    /// destination has no defined previous image. Later `Color` cameras on that same destination
    /// fill only their viewport. Later `Load` cameras do not. An offscreen target does not receive
    /// a surface camera's clear. Every pass clears its own depth image to 0.0. A shaded camera
    /// discards that depth and does not run a depth-view pass. A depth presentation stores it
    /// and replaces that camera's viewport in a second pass with no depth attachment. Surface
    /// depth is not reused for a target of a different size. One full-surface shaded camera is
    /// still a single pass with no viewport-clear draw and no depth-view draw.
    ///
    /// Offscreen images are allocated or resized from [`RenderScene::targets`] before any pass.
    /// A resize drops the previous image. Pixels in the replacement are undefined until a camera
    /// writes that target. Unchanged targets keep their image, view, and bind group.
    pub fn render(
        &mut self,
        gpu: &GpuContext,
        scene: &RenderScene,
        assets: &AssetManager,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        self.frame += 1;
        self.sync_render_targets(gpu, &scene.targets);
        for camera in &scene.passes {
            for layer in [&camera.opaque_sprites, &camera.mask_sprites, &camera.cutout_sprites, &camera.blended_sprites]
            {
                for batch in &layer.batches {
                    if batch.unavailable_target || batch.render_target.is_some() {
                        continue;
                    }
                    self.ensure_texture(gpu, assets, batch.texture);
                }
            }
            for mesh in camera.meshes.iter().chain(camera.blended_meshes.iter()) {
                self.ensure_mesh(gpu, assets, mesh.mesh);
                if let Some(texture) = mesh.texture {
                    self.ensure_texture(gpu, assets, texture);
                }
            }
        }
        self.evict_unused(assets);
        self.surface_size = (size.0.max(1), size.1.max(1));
        if self.msaa == MsaaSamples::One {
            self.ensure_depth(gpu, self.surface_size);
        }
        // One upload for every camera. Later cameras do not rewrite this buffer, so a second
        // submit still reads the same vertices. An empty list does not write and does not draw.
        let debug_vertices = collect_debug_vertices(&scene.debug_lines);
        self.debug_vertex_count = debug_vertices.len() as u32;
        if !debug_vertices.is_empty() {
            self.resource_stats.instance_buffer_grows += u64::from(self.debug_vertices.upload(gpu, &debug_vertices));
        }

        let mut stats = FrameStats { camera_passes: scene.passes.len() as u32, ..FrameStats::default() };
        let mut pass_stats = Vec::with_capacity(scene.passes.len());
        let mut opened = HashSet::new();
        let mut color_surface_failed = false;
        let mut load_surface_failed = false;
        for camera in &scene.passes {
            if self.msaa == MsaaSamples::Four {
                match self.prepare_four_sample(gpu, camera) {
                    FourPrep::Unavailable => {
                        self.note_direct_surface_failure(camera, &mut color_surface_failed, &mut load_surface_failed);
                        self.warn_msaa_skip(camera.camera_id, MsaaSkip::ImagesUnavailable);
                        stats.msaa_skipped += 1;
                        let mut one = PassFrameStats::default();
                        count_pass_instances(camera, &mut stats, &mut one);
                        pass_stats.push(one);
                        continue;
                    }
                    FourPrep::Absent => {
                        if let CameraOutput::Target(id) = camera.output {
                            log::warn!(
                                "render target {id} has no GPU image; camera {} was not drawn",
                                camera.camera_id
                            );
                        }
                        match camera.output {
                            CameraOutput::Surface => stats.surface_passes += 1,
                            CameraOutput::Target(_) => stats.offscreen_passes += 1,
                        }
                        let _first = opened.insert(camera.output);
                        let mut one = PassFrameStats::default();
                        count_pass_instances(camera, &mut stats, &mut one);
                        pass_stats.push(one);
                        continue;
                    }
                    FourPrep::Letterbox => {
                        match camera.output {
                            CameraOutput::Surface => stats.surface_passes += 1,
                            CameraOutput::Target(_) => stats.offscreen_passes += 1,
                        }
                        let first = opened.insert(camera.output);
                        let mut one = self.submit_letterbox(gpu, camera, first, target, &mut stats);
                        count_pass_instances(camera, &mut stats, &mut one);
                        pass_stats.push(one);
                        continue;
                    }
                    FourPrep::Ready => {}
                }
            }
            match camera.output {
                CameraOutput::Surface => stats.surface_passes += 1,
                CameraOutput::Target(_) => stats.offscreen_passes += 1,
            }
            let first = opened.insert(camera.output);
            let mut one = if self.msaa == MsaaSamples::Four {
                self.submit_msaa_camera(gpu, scene, camera, first, target, &mut stats)
            } else {
                self.submit_camera(gpu, scene, camera, first, target, &mut stats)
            };
            count_pass_instances(camera, &mut stats, &mut one);
            pass_stats.push(one);
        }
        if self.msaa == MsaaSamples::Four {
            let surface_written = opened.iter().any(|output| matches!(output, CameraOutput::Surface));
            if surface_written {
                self.msaa_blank_logged = false;
            } else if color_surface_failed && !load_surface_failed {
                self.clear_undefined_surface(gpu, target);
            }
        }
        self.retire_internal_targets(&scene.passes);
        self.stats = stats;
        self.pass_stats = pass_stats;
    }

    /// Uploads this camera's GPU data, records its pass, and submits before the caller may
    /// overwrite those buffers.
    fn submit_camera(
        &mut self,
        gpu: &GpuContext,
        scene: &RenderScene,
        camera: &CameraPass,
        first: bool,
        target: &wgpu::TextureView,
        stats: &mut FrameStats,
    ) -> PassFrameStats {
        self.upload_camera(gpu, scene, camera);

        if matches!(camera.view.fit, PresentationFit::Rejected(_)) {
            return PassFrameStats::default();
        }
        if matches!(camera.view.fit, PresentationFit::Letterbox { .. }) {
            return self.submit_letterbox(gpu, camera, first, target, stats);
        }
        let scaled = match camera.view.fit {
            PresentationFit::Scaled(image) => Some(image),
            _ => None,
        };
        if let Some(image) = scaled {
            if !self.ensure_internal(gpu, camera.camera_id, image.internal_width, image.internal_height) {
                log::warn!(
                    "camera {} internal resolution {}x{} was not allocated; the viewport is filled with the letterbox color",
                    camera.camera_id,
                    image.internal_width,
                    image.internal_height
                );
                return self.submit_letterbox(gpu, camera, first, target, stats);
            }
        }

        if let CameraOutput::Target(id) = camera.output {
            if !self.render_targets.contains_key(&id) {
                log::warn!("render target {id} has no GPU image; camera {} was not drawn", camera.camera_id);
                return PassFrameStats::default();
            }
        }

        // A later Color camera paints only its rectangle on this destination. The attachment
        // clear covers the whole image, so it is used once, for the first camera that writes it.
        // Alpha matches that clear: 1.
        let fill_viewport = scaled.is_none() && !first && camera.clear == CameraClear::Color;
        if fill_viewport {
            let uniform =
                ClearUniform { color: [camera.clear_color.r, camera.clear_color.g, camera.clear_color.b, 1.0] };
            gpu.queue.write_buffer(&self.clear_uniform, 0, bytemuck::bytes_of(&uniform));
        }
        if let Some(image) = scaled {
            let plan = self.present_plan(camera, false, first);
            if plan.fill_draw {
                self.write_clear(gpu, [camera.letterbox_color.r, camera.letterbox_color.g, camera.letterbox_color.b]);
            }
            self.write_present(gpu, image);
        }
        // Depth presentation samples the depth this camera is about to store. The uniform is
        // written before the encoder so this submit sees it. The next camera has its own submit.
        let visualize = camera.presentation == CameraPresentation::Depth;
        if visualize {
            self.prepare_depth_view(gpu, camera);
        }

        let mut one = PassFrameStats::default();
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("camera") });
        {
            let (color_view, depth_view, size, scene_viewport, scene_first) = if scaled.is_some() {
                let image = &self.internal_targets[&camera.camera_id];
                let size = (image.width, image.height);
                (
                    image.view.clone(),
                    image.depth.attachment.clone(),
                    size,
                    [0.0, 0.0, size.0 as f32, size.1 as f32],
                    true,
                )
            } else {
                match camera.output {
                    CameraOutput::Surface => {
                        let surface_depth = self.depth.as_ref().expect("depth created above");
                        (
                            target.clone(),
                            surface_depth.attachment.clone(),
                            surface_depth.size,
                            camera.viewport_px,
                            first,
                        )
                    }
                    CameraOutput::Target(id) => {
                        let image = self.render_targets.get(&id).expect("missing targets return before the encoder");
                        (
                            image.view.clone(),
                            image.depth.attachment.clone(),
                            (image.width, image.height),
                            camera.viewport_px,
                            first,
                        )
                    }
                }
            };
            let color_load = if scene_first {
                wgpu::LoadOp::Clear(attachment_clear(camera.clear_color.r, camera.clear_color.g, camera.clear_color.b))
            } else {
                wgpu::LoadOp::Load
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("camera"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: color_load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: if visualize { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard },
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let (x, y, w, h) = clamp_viewport(scene_viewport, size);
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
            if fill_viewport {
                pass.set_pipeline(&self.clear_pipeline);
                pass.set_bind_group(0, &self.clear_bind_group, &[]);
                pass.draw(0..3, 0..1);
                record_clear(stats, &mut one);
            }

            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_vertex_buffer(1, self.mesh_instances.buffer.slice(..));
            let mut i = 0;
            while i < camera.meshes.len() {
                let end = MeshInstance::run_end(&camera.meshes, i);
                let draw = camera.meshes[i];
                if let Some(mesh) = self.meshes.get(&draw.mesh.id()) {
                    pass.set_pipeline(self.mesh_pipeline(draw.draw));
                    let texture = draw.texture.and_then(|t| self.textures.get(&t.id())).unwrap_or(&self.white);
                    pass.set_bind_group(1, &texture.bind_group, &[]);
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, i as u32..end as u32);
                    record_world(stats, &mut one, (end - i) as u32);
                }
                i = end;
            }
            self.draw_sprites(&mut pass, &camera.opaque_sprites, &self.opaque_instances, stats, &mut one);
            self.draw_sprites(&mut pass, &camera.mask_sprites, &self.mask_instances, stats, &mut one);
            self.draw_sprites(&mut pass, &camera.cutout_sprites, &self.cutout_instances, stats, &mut one);
            self.draw_transparent(&mut pass, camera, stats, &mut one);
            // After transparent draws. Opaque, mask, and cutout wrote depth, so a farther line
            // is occluded. Blended draws did not write depth, so a line can paint over them.
            // No depth write: a line does not hide a later camera. Not an X-ray overlay.
            if self.debug_vertex_count > 0 {
                pass.set_pipeline(&self.debug_pipeline);
                pass.set_bind_group(0, &self.globals_bind_group, &[]);
                pass.set_vertex_buffer(0, self.debug_vertices.buffer.slice(..));
                pass.draw(0..self.debug_vertex_count, 0..1);
                record_world(stats, &mut one, self.debug_vertex_count);
            }
        }
        if visualize {
            self.draw_depth_view(&mut encoder, camera, target, stats, &mut one);
        }
        if scaled.is_some() {
            self.record_present(&mut encoder, camera, target, first, stats, &mut one);
        }
        gpu.queue.submit([encoder.finish()]);
        one
    }

    /// Writes this camera's inverse projection and creates the depth sample group once.
    ///
    /// The group is not created for a shaded camera. Resizing the image drops it with the image.
    fn prepare_depth_view(&mut self, gpu: &GpuContext, camera: &CameraPass) {
        if let PresentationFit::Scaled(image) = camera.view.fit {
            let viewport = [0.0, 0.0, image.internal_width as f32, image.internal_height as f32];
            let uniform = DepthViewUniform::from_projection(camera.view.proj, viewport);
            gpu.queue.write_buffer(&self.depth_view_uniform, 0, bytemuck::bytes_of(&uniform));
            if self.internal_targets[&camera.camera_id].depth.sample_bind_group.is_some() {
                return;
            }
            let sample = self.internal_targets[&camera.camera_id].depth.sample.clone();
            let group =
                depth_sample_bind_group(&gpu.device, &self.depth_view_layout, &sample, &self.depth_view_uniform);
            self.internal_targets.get_mut(&camera.camera_id).expect("internal image").depth.sample_bind_group =
                Some(group);
            self.resource_stats.bind_group_creates += 1;
            return;
        }
        let size = match camera.output {
            CameraOutput::Surface => self.depth.as_ref().expect("depth created above").size,
            CameraOutput::Target(id) => {
                let image = &self.render_targets[&id];
                (image.width, image.height)
            }
        };
        let (x, y, w, h) = clamp_viewport(camera.viewport_px, size);
        let uniform = DepthViewUniform::from_projection(camera.view.proj, [x, y, w, h]);
        gpu.queue.write_buffer(&self.depth_view_uniform, 0, bytemuck::bytes_of(&uniform));
        let missing = match camera.output {
            CameraOutput::Surface => self.depth.as_ref().expect("depth created above").sample_bind_group.is_none(),
            CameraOutput::Target(id) => self.render_targets[&id].depth.sample_bind_group.is_none(),
        };
        if !missing {
            return;
        }
        let group = match camera.output {
            CameraOutput::Surface => depth_sample_bind_group(
                &gpu.device,
                &self.depth_view_layout,
                &self.depth.as_ref().expect("depth created above").sample,
                &self.depth_view_uniform,
            ),
            CameraOutput::Target(id) => depth_sample_bind_group(
                &gpu.device,
                &self.depth_view_layout,
                &self.render_targets[&id].depth.sample,
                &self.depth_view_uniform,
            ),
        };
        match camera.output {
            CameraOutput::Surface => {
                self.depth.as_mut().expect("depth created above").sample_bind_group = Some(group);
            }
            CameraOutput::Target(id) => {
                self.render_targets.get_mut(&id).expect("target").depth.sample_bind_group = Some(group);
            }
        }
        self.resource_stats.bind_group_creates += 1;
    }

    /// Second pass for one depth presentation. It has no depth attachment, so it can sample the
    /// depth the scene pass stored. The viewport and scissor stay this camera's rectangle.
    /// A later camera has not cleared that depth yet: this runs before the submit below.
    fn draw_depth_view(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera: &CameraPass,
        surface: &wgpu::TextureView,
        stats: &mut FrameStats,
        one: &mut PassFrameStats,
    ) {
        if let PresentationFit::Scaled(image) = camera.view.fit {
            let (sample, color_view, size) = {
                let internal = &self.internal_targets[&camera.camera_id];
                (
                    internal.depth.sample_bind_group.clone().expect("internal depth sample"),
                    internal.view.clone(),
                    (image.internal_width, image.internal_height),
                )
            };
            let (x, y, w, h) = (0.0, 0.0, size.0 as f32, size.1 as f32);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth view"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.depth_view_pipeline);
            pass.set_bind_group(0, &sample, &[]);
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_scissor_rect(0, 0, size.0, size.1);
            pass.draw(0..3, 0..1);
            record_depth_view(stats, one);
            return;
        }
        let (sample, color_view, size) = match camera.output {
            CameraOutput::Surface => {
                let depth = self.depth.as_ref().expect("depth created above");
                (depth.sample_bind_group.as_ref().expect("depth sample"), surface, depth.size)
            }
            CameraOutput::Target(id) => {
                let image = &self.render_targets[&id];
                (
                    image.depth.sample_bind_group.as_ref().expect("depth sample"),
                    &image.view,
                    (image.width, image.height),
                )
            }
        };
        let (x, y, w, h) = clamp_viewport(camera.viewport_px, size);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("depth view"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.depth_view_pipeline);
        pass.set_bind_group(0, sample, &[]);
        pass.set_viewport(x, y, w, h, 0.0, 1.0);
        pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
        pass.draw(0..3, 0..1);
        record_depth_view(stats, one);
    }

    /// Letterbox fill only. The scene was not rendered. The destination viewport is defined.
    fn submit_letterbox(
        &mut self,
        gpu: &GpuContext,
        camera: &CameraPass,
        first: bool,
        surface: &wgpu::TextureView,
        stats: &mut FrameStats,
    ) -> PassFrameStats {
        if let CameraOutput::Target(id) = camera.output {
            if !self.render_targets.contains_key(&id) {
                log::warn!("render target {id} has no GPU image; camera {} was not drawn", camera.camera_id);
                return PassFrameStats::default();
            }
        }
        let plan = self.present_plan(camera, true, first);
        if plan.fill_draw {
            self.write_clear(gpu, [camera.letterbox_color.r, camera.letterbox_color.g, camera.letterbox_color.b]);
        }
        let mut one = PassFrameStats::default();
        let mut encoder =
            gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("letterbox") });
        self.record_present(&mut encoder, camera, surface, first, stats, &mut one);
        gpu.queue.submit([encoder.finish()]);
        one
    }

    fn record_present(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera: &CameraPass,
        surface: &wgpu::TextureView,
        first: bool,
        stats: &mut FrameStats,
        one: &mut PassFrameStats,
    ) {
        let letterbox_only = matches!(camera.view.fit, PresentationFit::Letterbox { .. });
        let plan = self.present_plan(camera, letterbox_only, first);
        let (destination, dest_size) = self.destination_view(camera, surface);
        let (x, y, w, h) = clamp_viewport(camera.viewport_px, dest_size);
        let load = if !first {
            wgpu::LoadOp::Load
        } else if plan.clear_with_letterbox {
            wgpu::LoadOp::Clear(attachment_clear(
                camera.letterbox_color.r,
                camera.letterbox_color.g,
                camera.letterbox_color.b,
            ))
        } else {
            wgpu::LoadOp::Clear(attachment_clear(camera.clear_color.r, camera.clear_color.g, camera.clear_color.b))
        };
        let present = match camera.view.fit {
            PresentationFit::Scaled(_) => Some(self.internal_targets[&camera.camera_id].present_bind_group.clone()),
            _ => None,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &destination,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if plan.fill_draw {
            pass.set_pipeline(&self.color_fill_pipeline);
            pass.set_bind_group(0, &self.clear_bind_group, &[]);
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
            pass.draw(0..3, 0..1);
            record_clear(stats, one);
        }
        if let Some(group) = present.as_ref() {
            let PresentationFit::Scaled(image) = camera.view.fit else { return };
            let (dx, dy, dw, dh) = clamp_viewport(
                [image.display_origin.x, image.display_origin.y, image.display_size.x, image.display_size.y],
                dest_size,
            );
            pass.set_pipeline(&self.present_pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.set_viewport(dx, dy, dw, dh, 0.0, 1.0);
            pass.set_scissor_rect(dx as u32, dy as u32, dw as u32, dh as u32);
            pass.draw(0..3, 0..1);
            record_presentation(stats, one);
        }
    }

    fn present_plan(&self, camera: &CameraPass, letterbox_only: bool, first: bool) -> PresentPlan {
        let size = self.output_size(camera);
        let (x, y, w, h) = clamp_viewport(camera.viewport_px, size);
        let covers = x <= 0.0 && y <= 0.0 && w + 0.5 >= size.0 as f32 && h + 0.5 >= size.1 as f32;
        let bars = if letterbox_only {
            true
        } else if let PresentationFit::Scaled(image) = camera.view.fit {
            image.display_size.x + 0.5 < w || image.display_size.y + 0.5 < h
        } else {
            false
        };
        PresentPlan { fill_draw: bars && !(first && covers), clear_with_letterbox: first && covers && bars }
    }

    fn output_size(&self, camera: &CameraPass) -> (u32, u32) {
        match camera.output {
            CameraOutput::Surface => self.surface_size,
            CameraOutput::Target(id) => {
                let image = &self.render_targets[&id];
                (image.width, image.height)
            }
        }
    }

    fn destination_view(&self, camera: &CameraPass, surface: &wgpu::TextureView) -> (wgpu::TextureView, (u32, u32)) {
        match camera.output {
            CameraOutput::Surface => (surface.clone(), self.output_size(camera)),
            CameraOutput::Target(id) => {
                let image = &self.render_targets[&id];
                (image.view.clone(), (image.width, image.height))
            }
        }
    }

    fn write_clear(&self, gpu: &GpuContext, color: [f32; 3]) {
        let uniform = ClearUniform { color: [color[0], color[1], color[2], 1.0] };
        gpu.queue.write_buffer(&self.clear_uniform, 0, bytemuck::bytes_of(&uniform));
    }

    fn write_present(&self, gpu: &GpuContext, image: ScaledImage) {
        let uniform =
            PresentUniform { origin: [image.display_origin.x, image.display_origin.y], scale: image.scale, _pad: 0 };
        gpu.queue.write_buffer(&self.present_uniform, 0, bytemuck::bytes_of(&uniform));
    }

    /// Allocates or reuses this camera's private image. `false` when the device cannot allocate it.
    fn ensure_internal(&mut self, gpu: &GpuContext, camera_id: u32, width: u32, height: u32) -> bool {
        if let Err(error) = validate_texture_dimensions(width, height, self.dimension_limits(gpu)) {
            log::warn!("internal resolution {width}x{height} was not allocated: {error}");
            self.internal_targets.remove(&camera_id);
            return false;
        }
        if let Some(current) = self.internal_targets.get(&camera_id) {
            if current.width == width && current.height == height {
                return true;
            }
        }
        let replacing = self.internal_targets.contains_key(&camera_id);
        if replacing && self.internal_targets.get(&camera_id).and_then(|image| image.msaa.as_ref()).is_some() {
            self.resource_stats.msaa_image_drops += 1;
        }
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let color = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("internal color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = DepthImage::allocate(&gpu.device, (width, height), "internal depth");
        let present_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("internal present"),
            layout: &self.present_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: self.present_uniform.as_entire_binding() },
            ],
        });
        self.resource_stats.bind_group_creates += 1;
        if replacing {
            self.resource_stats.internal_target_reallocs += 1;
        } else {
            self.resource_stats.internal_target_creates += 1;
        }
        self.internal_targets
            .insert(camera_id, InternalTarget { width, height, color, view, depth, present_bind_group, msaa: None });
        true
    }

    fn retire_internal_targets(&mut self, passes: &[CameraPass]) {
        let live: HashSet<u32> = passes
            .iter()
            .filter(|pass| matches!(pass.view.fit, PresentationFit::Scaled(_)))
            .map(|pass| pass.camera_id)
            .collect();
        let msaa_drops =
            self.internal_targets.iter().filter(|(id, image)| !live.contains(id) && image.msaa.is_some()).count()
                as u64;
        let before = self.internal_targets.len();
        self.internal_targets.retain(|id, _| live.contains(id));
        self.resource_stats.internal_target_drops += (before - self.internal_targets.len()) as u64;
        self.resource_stats.msaa_image_drops += msaa_drops;
    }

    /// Color and depth bytes of private internal images retained for the cameras that scaled
    /// during the latest [`Self::render`].
    pub fn internal_targets(&self) -> InternalTargetUsage {
        let mut color_bytes = 0u64;
        let mut depth_bytes = 0u64;
        for image in self.internal_targets.values() {
            let pixels = u64::from(image.width) * u64::from(image.height);
            color_bytes += pixels * 4;
            depth_bytes += pixels * 4;
        }
        InternalTargetUsage { cameras: self.internal_targets.len() as u32, color_bytes, depth_bytes }
    }

    fn sprite_bind_group<'a>(&'a self, batch: &SpriteBatch) -> &'a wgpu::BindGroup {
        if batch.unavailable_target {
            return &self.missing.bind_group;
        }
        if let Some(id) = batch.render_target {
            return self.render_targets.get(&id).map(|target| &target.bind_group).unwrap_or(&self.missing.bind_group);
        }
        &self.textures.get(&batch.texture.id()).unwrap_or(&self.missing).bind_group
    }

    fn mesh_pipeline(&self, draw: MeshDraw) -> &wgpu::RenderPipeline {
        if self.msaa == MsaaSamples::Four {
            let draw_pipes = &self.msaa4.as_ref().expect("4× pipelines exist while Four is selected").draw;
            return match draw {
                MeshDraw::Solid { double_sided: false } => &draw_pipes.mesh_solid,
                MeshDraw::Solid { double_sided: true } => &draw_pipes.mesh_solid_double,
                MeshDraw::Mask { double_sided: false } => &draw_pipes.mesh_mask,
                MeshDraw::Mask { double_sided: true } => &draw_pipes.mesh_mask_double,
                MeshDraw::Blend { double_sided: false } => &draw_pipes.mesh_blend,
                MeshDraw::Blend { double_sided: true } => &draw_pipes.mesh_blend_double,
            };
        }
        match draw {
            MeshDraw::Solid { double_sided: false } => &self.mesh_solid,
            MeshDraw::Solid { double_sided: true } => &self.mesh_solid_double,
            MeshDraw::Mask { double_sided: false } => &self.mesh_mask,
            MeshDraw::Mask { double_sided: true } => &self.mesh_mask_double,
            MeshDraw::Blend { double_sided: false } => &self.mesh_blend,
            MeshDraw::Blend { double_sided: true } => &self.mesh_blend_double,
        }
    }

    /// Walks the combined blend sequence. Each item rebinds the pipeline and buffers it needs.
    /// The walk does not sort again, and it does not draw every sprite before every mesh.
    fn draw_transparent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        camera: &'a CameraPass,
        stats: &mut FrameStats,
        one: &mut PassFrameStats,
    ) {
        for batch in &camera.transparent {
            match batch {
                TransparentBatch::Sprites(batch) => {
                    pass.set_pipeline(self.sprite_pipeline(batch.pipeline));
                    pass.set_bind_group(1, self.sprite_bind_group(batch), &[]);
                    pass.set_vertex_buffer(0, self.blend_instances.buffer.slice(..));
                    pass.draw(0..6, batch.range.clone());
                    record_world(stats, one, batch.range.end - batch.range.start);
                }
                TransparentBatch::Meshes { mesh, texture, draw, range } => {
                    let Some(gpu_mesh) = self.meshes.get(&mesh.id()) else { continue };
                    pass.set_pipeline(self.mesh_pipeline(*draw));
                    let bound = texture.and_then(|handle| self.textures.get(&handle.id())).unwrap_or(&self.white);
                    pass.set_bind_group(1, &bound.bind_group, &[]);
                    pass.set_vertex_buffer(0, gpu_mesh.vertices.slice(..));
                    pass.set_vertex_buffer(1, self.blend_mesh_instances.buffer.slice(..));
                    pass.set_index_buffer(gpu_mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..gpu_mesh.index_count, 0, range.clone());
                    record_world(stats, one, range.end - range.start);
                }
            }
        }
    }

    fn sprite_pipeline(&self, kind: SpritePipelineKind) -> &wgpu::RenderPipeline {
        if self.msaa == MsaaSamples::Four {
            let draw_pipes = &self.msaa4.as_ref().expect("4× pipelines exist while Four is selected").draw;
            return match kind {
                SpritePipelineKind::LegacyCutout => &draw_pipes.sprite_cutout,
                SpritePipelineKind::LegacyBlend | SpritePipelineKind::BlendDouble => &draw_pipes.sprite_blend,
                SpritePipelineKind::OpaqueCulled => &draw_pipes.sprite_opaque,
                SpritePipelineKind::OpaqueDouble => &draw_pipes.sprite_opaque_double,
                SpritePipelineKind::MaskCulled => &draw_pipes.sprite_mask,
                SpritePipelineKind::MaskDouble => &draw_pipes.sprite_mask_double,
                SpritePipelineKind::BlendCulled => &draw_pipes.sprite_blend_culled,
            };
        }
        match kind {
            SpritePipelineKind::LegacyCutout => &self.sprite_cutout,
            SpritePipelineKind::LegacyBlend | SpritePipelineKind::BlendDouble => &self.sprite_blend,
            SpritePipelineKind::OpaqueCulled => &self.sprite_opaque,
            SpritePipelineKind::OpaqueDouble => &self.sprite_opaque_double,
            SpritePipelineKind::MaskCulled => &self.sprite_mask,
            SpritePipelineKind::MaskDouble => &self.sprite_mask_double,
            SpritePipelineKind::BlendCulled => &self.sprite_blend_culled,
        }
    }

    fn draw_sprites(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        layer: &SpriteLayer,
        instances: &InstanceBuffer,
        stats: &mut FrameStats,
        one: &mut PassFrameStats,
    ) {
        if layer.batches.is_empty() {
            return;
        }
        pass.set_vertex_buffer(0, instances.buffer.slice(..));
        for batch in &layer.batches {
            pass.set_pipeline(self.sprite_pipeline(batch.pipeline));
            pass.set_bind_group(1, self.sprite_bind_group(batch), &[]);
            pass.draw(0..6, batch.range.clone());
            record_world(stats, one, batch.range.end - batch.range.start);
        }
    }

    /// Tightly packed RGBA8 of one camera's private internal image after [`Self::render`].
    ///
    /// This is a test and diagnostic read. Sprites cannot sample this image, and it is not a
    /// [`RenderTargetId`]. `None` when that camera did not scale an image this frame.
    pub fn read_internal(&self, gpu: &GpuContext, camera_id: u32) -> Option<Vec<u8>> {
        let image = self.internal_targets.get(&camera_id)?;
        read_texture_level(gpu, &image.color, 0, image.width, image.height)
    }

    /// Tightly packed RGBA8 of one offscreen target after [`Self::render`].
    ///
    /// This is a test and diagnostic read. Camera-to-texture sampling does not call it and does
    /// not upload the pixels into a texture asset. `None` when `id` has no GPU image.
    pub fn read_render_target(&self, gpu: &GpuContext, id: RenderTargetId) -> Option<Vec<u8>> {
        let target = self.render_targets.get(&id)?;
        read_texture_level(gpu, &target.color, 0, target.width, target.height)
    }

    /// Renders to an offscreen RGBA8 image and returns tightly packed pixels (sRGB encoded when
    /// this renderer's format is an sRGB format). Blocks until the GPU is done.
    pub fn render_to_rgba(
        &mut self,
        gpu: &GpuContext,
        scene: &RenderScene,
        assets: &AssetManager,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, RenderError> {
        assert!(
            matches!(self.format, wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Rgba8Unorm),
            "render_to_rgba needs an RGBA8 renderer format, got {:?}",
            self.format
        );
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.render(gpu, scene, assets, &view, (width, height));

        let unpadded = width * 4;
        let padded = unpadded.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            size,
        );
        gpu.queue.submit([encoder.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| RenderError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| RenderError::Readback(e.to_string()))?
            .map_err(|e| RenderError::Readback(e.to_string()))?;

        let data = readback.slice(..).get_mapped_range().map_err(|e| RenderError::Readback(e.to_string()))?;
        let mut out = Vec::with_capacity((unpadded * height) as usize);
        for row in data.chunks(padded as usize).take(height as usize) {
            out.extend_from_slice(&row[..unpadded as usize]);
        }
        drop(data);
        readback.unmap();
        Ok(out)
    }

    /// Allocates, resizes, or drops offscreen images so they match `targets` before any pass.
    ///
    /// An unchanged id, size, and generation keeps the color image, depth image, view, and bind
    /// group. A changed generation replaces both images. Dropped images stay alive inside wgpu
    /// until submissions that already reference them finish. This does not copy old pixels and
    /// does not count as a texture upload.
    fn sync_render_targets(&mut self, gpu: &GpuContext, targets: &[RenderTargetDesc]) {
        let live: HashSet<RenderTargetId> = targets.iter().map(|target| target.id).collect();
        let msaa_drops =
            self.render_targets.iter().filter(|(id, image)| !live.contains(id) && image.msaa.is_some()).count() as u64;
        self.render_targets.retain(|id, _| live.contains(id));
        self.resource_stats.msaa_image_drops += msaa_drops;
        for desc in targets {
            if let Err(error) = validate_texture_dimensions(desc.width, desc.height, self.dimension_limits(gpu)) {
                log::warn!("render target {} was not allocated: {error}", desc.id);
                self.render_targets.remove(&desc.id);
                continue;
            }
            let replace = match self.render_targets.get(&desc.id) {
                Some(current) => {
                    current.generation != desc.generation
                        || current.width != desc.width
                        || current.height != desc.height
                }
                None => false,
            };
            if self.render_targets.contains_key(&desc.id) && !replace {
                continue;
            }
            self.allocate_render_target(gpu, desc, replace);
        }
    }

    fn allocate_render_target(&mut self, gpu: &GpuContext, desc: &RenderTargetDesc, replace: bool) {
        if replace {
            self.resource_stats.render_target_reallocs += 1;
            if self.render_targets.get(&desc.id).and_then(|image| image.msaa.as_ref()).is_some() {
                self.resource_stats.msaa_image_drops += 1;
            }
        } else {
            self.resource_stats.render_target_creates += 1;
        }
        let size = wgpu::Extent3d { width: desc.width, height: desc.height, depth_or_array_layers: 1 };
        let color = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render target"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = DepthImage::allocate(&gpu.device, (desc.width, desc.height), "render target depth");
        let bind_group =
            texture_bind_group(&gpu.device, &self.texture_layout, &view, &self.target_sampler, "render target");
        self.resource_stats.bind_group_creates += 1;
        self.render_targets.insert(
            desc.id,
            GpuRenderTarget {
                generation: desc.generation,
                width: desc.width,
                height: desc.height,
                color,
                view,
                depth,
                bind_group,
                msaa: None,
            },
        );
    }

    fn ensure_depth(&mut self, gpu: &GpuContext, size: (u32, u32)) {
        if matches!(&self.depth, Some(depth) if depth.size == size) {
            return;
        }
        self.depth = Some(DepthImage::allocate(&gpu.device, size, "depth"));
    }

    /// Drops GPU copies whose asset is gone from the manager and copies no frame has used for
    /// `cache_max_idle_frames` frames. Runs after this frame's uploads and before drawing, so
    /// everything the frame needs is already marked as used, and a removed asset can never be
    /// drawn from a stale copy.
    fn evict_unused(&mut self, assets: &AssetManager) {
        let (frame, max_idle) = (self.frame, self.cache_max_idle_frames as u64);
        let keep = |last_used: u64, exists: bool| exists && frame - last_used < max_idle;
        self.textures.retain(|id, t| keep(t.last_used, assets.get(AssetHandle::<Texture>::from_id(*id)).is_some()));
        self.meshes.retain(|id, m| keep(m.last_used, assets.get(AssetHandle::<MeshData>::from_id(*id)).is_some()));
        self.refused_uploads.retain(|id, _| assets.get(AssetHandle::<Texture>::from_id(*id)).is_some());
        let mut live = HashSet::new();
        live.insert(self.white.sampling.key);
        live.insert(self.missing.sampling.key);
        for texture in self.textures.values() {
            live.insert(texture.sampling.key);
        }
        self.samplers.retain(|key, _| live.contains(key));
    }

    /// Ensures a GPU image for `handle` and marks it used this frame.
    ///
    /// An unchanged asset revision does no work and does not hash pixels. A new revision is
    /// inspected: matching dimensions, format, and mip allocation keep the image.
    /// [`TexturePixelEdits::base_upload`] then chooses no upload, the recorded rectangles, or the
    /// whole base level. A partial or full base upload rebuilds an allocated mip chain. A sampler
    /// change only rebuilds the bind group and, when the sampled level range changes, the view.
    /// Storage that no longer matches is replaced. A size the device cannot allocate is not created.
    /// Missing assets get no entry; sprite draws then use the magenta placeholder.
    fn ensure_texture(&mut self, gpu: &GpuContext, assets: &AssetManager, handle: AssetHandle<Texture>) {
        let revision = assets.revision(handle);
        let id = handle.id();
        if revision == 0 {
            self.refused_uploads.remove(&id);
            return;
        }
        if let Some(cached) = self.textures.get_mut(&id) {
            if cached.revision == revision {
                cached.last_used = self.frame;
                return;
            }
        }
        if self.refused_uploads.get(&id) == Some(&revision) {
            if let Some(cached) = self.textures.get_mut(&id) {
                cached.last_used = self.frame;
            }
            return;
        }
        let Some(tex) = assets.get(handle) else { return };
        let (width, height) = (tex.width, tex.height);
        if let Err(error) = validate_texture_dimensions(width, height, self.dimension_limits(gpu)) {
            self.textures.remove(&id);
            self.refuse_texture_upload(assets, handle, revision, &error.to_string());
            return;
        }
        if width == 0 || height == 0 {
            self.textures.remove(&id);
            self.refuse_texture_upload(assets, handle, revision, "width and height must be at least 1");
            return;
        }
        let format = gpu_format(tex.color_space);
        let can_blit = self.format_can_blit(gpu, format);
        let allocation = mip::allocate_mip_levels(tex.mips, width, height, self.npot_mipmaps, can_blit);
        if allocation.npot_fallback {
            let name = assets.name_of(handle).unwrap_or("unnamed texture").to_string();
            self.note_npot_fallback(&name);
        }
        let sampling = effective_sampling(tex.sampler, self.anisotropic_filtering, allocation.levels);
        self.note_fallbacks(&sampling);
        let fingerprint = pixel_fingerprint(&tex.rgba);
        self.resource_stats.pixel_fingerprints += 1;
        let sampler = self.cached_sampler(gpu, &sampling);
        let mut rebuild_mips = false;
        let mut in_place_refused = false;
        if let Some(cached) = self.textures.get_mut(&id) {
            let storage_matches = cached.width == width
                && cached.height == height
                && cached.format == format
                && cached.mip_level_count == allocation.levels;
            if storage_matches {
                match apply_base_upload(
                    &gpu.queue,
                    cached,
                    BasePixels {
                        rgba: &tex.rgba,
                        width,
                        height,
                        fingerprint,
                        edits: tex.pixel_edits(),
                        levels: allocation.levels,
                    },
                    &mut self.resource_stats,
                ) {
                    Some(rebuild) => {
                        if cached.sampling.view_levels != sampling.view_levels {
                            cached.view = sampling_view(&cached.texture, sampling.view_levels);
                        }
                        if cached.sampling.key != sampling.key {
                            cached.bind_group = texture_bind_group(
                                &gpu.device,
                                &self.texture_layout,
                                &cached.view,
                                &sampler,
                                "asset texture",
                            );
                            self.resource_stats.bind_group_creates += 1;
                        }
                        cached.sampling = sampling;
                        cached.revision = revision;
                        cached.last_used = self.frame;
                        rebuild_mips = rebuild;
                    }
                    None => in_place_refused = true,
                }
            }
        }
        if in_place_refused {
            self.refuse_texture_upload(
                assets,
                handle,
                revision,
                "base-level bytes do not match the texture dimensions",
            );
            return;
        }
        if self.textures.get(&id).is_some_and(|cached| cached.revision == revision) {
            self.refused_uploads.remove(&id);
            if rebuild_mips {
                let texture = self.textures.get(&id).expect("revision was just stored").texture.clone();
                self.generate_mips(gpu, &texture, format, width, height, allocation.levels);
            }
            return;
        }
        if !base_bytes_match(&tex.rgba, width, height) {
            self.textures.remove(&id);
            self.refuse_texture_upload(
                assets,
                handle,
                revision,
                "base-level bytes do not match the texture dimensions",
            );
            return;
        }
        let mut gpu_tex = upload_image(
            gpu,
            &self.texture_layout,
            &sampler,
            ImageUpload {
                sampling,
                width,
                height,
                rgba: &tex.rgba,
                format,
                mip_levels: allocation.levels,
                label: "asset texture",
                fingerprint,
            },
            &mut self.resource_stats,
        );
        gpu_tex.revision = revision;
        gpu_tex.last_used = self.frame;
        let texture = gpu_tex.texture.clone();
        self.textures.insert(id, gpu_tex);
        self.refused_uploads.remove(&id);
        if allocation.levels > 1 {
            self.generate_mips(gpu, &texture, format, width, height, allocation.levels);
        }
    }

    fn dimension_limits(&self, gpu: &GpuContext) -> TextureDimensionLimits {
        let device_limit = gpu.device.limits().max_texture_dimension_2d;
        let mut limits = TextureDimensionLimits { max_width: device_limit, max_height: device_limit };
        if let Some(tighter) = self.dimension_limit_override {
            limits.max_width = limits.max_width.min(tighter.max_width);
            limits.max_height = limits.max_height.min(tighter.max_height);
        }
        limits
    }

    fn refuse_texture_upload(
        &mut self,
        assets: &AssetManager,
        handle: AssetHandle<Texture>,
        revision: u32,
        reason: &str,
    ) {
        let id = handle.id();
        self.refused_uploads.insert(id, revision);
        match assets.name_of(handle) {
            Some(name) => log::warn!("texture '{name}' was not uploaded: {reason}"),
            None => log::warn!("texture {:016x} was not uploaded: {reason}", id.0),
        }
    }

    fn format_can_blit(&mut self, gpu: &GpuContext, format: wgpu::TextureFormat) -> bool {
        if let Some(supported) = self.blit_support.get(&format) {
            return *supported;
        }
        let supported = mip::format_supports_mip_blit(&gpu.adapter, format);
        self.blit_support.insert(format, supported);
        if !supported {
            log::warn!(
                "texture format {format:?} cannot be linearly filtered and written as a render attachment; mip generation uses one level"
            );
        }
        supported
    }

    fn note_npot_fallback(&mut self, name: &str) {
        if !self.npot_fallback_names.insert(name.to_string()) {
            return;
        }
        log::warn!(
            "texture '{name}' is not a power of two and DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES is absent; using one mip level"
        );
        self.resource_stats.npot_mip_fallbacks += 1;
    }

    fn generate_mips(
        &mut self,
        gpu: &GpuContext,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        levels: u32,
    ) {
        if levels <= 1 {
            return;
        }
        if self.mip_generator.is_none() {
            self.mip_generator = Some(MipGenerator::new(&gpu.device));
            self.resource_stats.sampler_creates += 1;
        }
        let generator = self.mip_generator.as_mut().expect("mip generator was just created");
        generator.generate(gpu, texture, format, width, height, levels);
        self.resource_stats.mip_generations += 1;
    }

    fn note_fallbacks(&mut self, sampling: &EffectiveSampling) {
        if sampling.capability_fallback && !self.anisotropy_capability_logged {
            log::warn!(
                "DownlevelFlags::ANISOTROPIC_FILTERING is absent; anisotropy above 1 is sampled as 1 and the other sampler settings are unchanged"
            );
            self.anisotropy_capability_logged = true;
            self.resource_stats.anisotropy_capability_fallbacks += 1;
        }
        if sampling.invalid_anisotropy && !self.invalid_anisotropy_logged {
            log::warn!("sampler anisotropy below 1 is invalid; sampling with anisotropy 1 and the requested filters");
            self.invalid_anisotropy_logged = true;
            self.resource_stats.invalid_anisotropy_fallbacks += 1;
        }
    }

    fn cached_sampler(&mut self, gpu: &GpuContext, sampling: &EffectiveSampling) -> wgpu::Sampler {
        if let Some(sampler) = self.samplers.get(&sampling.key) {
            return sampler.clone();
        }
        let sampler = gpu.device.create_sampler(&sampler_descriptor(sampling));
        self.resource_stats.sampler_creates += 1;
        self.samplers.insert(sampling.key, sampler.clone());
        sampler
    }

    fn ensure_mesh(&mut self, gpu: &GpuContext, assets: &AssetManager, handle: AssetHandle<MeshData>) {
        let revision = assets.revision(handle);
        if revision == 0 {
            return;
        }
        if let Some(cached) = self.meshes.get_mut(&handle.id()) {
            if cached.revision == revision {
                cached.last_used = self.frame;
                return;
            }
        }
        let Some(mesh) = assets.get(handle) else { return };
        let vertices = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh vertices"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes.insert(
            handle.id(),
            GpuMesh { revision, last_used: self.frame, vertices, indices, index_count: mesh.indices.len() as u32 },
        );
    }

    fn upload_camera(&mut self, gpu: &GpuContext, scene: &RenderScene, camera: &CameraPass) {
        self.resource_stats.instance_buffer_grows +=
            u64::from(self.opaque_instances.upload(gpu, &sprite_instances(&camera.opaque_sprites)));
        self.resource_stats.instance_buffer_grows +=
            u64::from(self.mask_instances.upload(gpu, &sprite_instances(&camera.mask_sprites)));
        self.resource_stats.instance_buffer_grows +=
            u64::from(self.cutout_instances.upload(gpu, &sprite_instances(&camera.cutout_sprites)));
        self.resource_stats.instance_buffer_grows +=
            u64::from(self.blend_instances.upload(gpu, &sprite_instances(&camera.blended_sprites)));
        let mesh_data: Vec<GpuMeshInstance> = camera
            .meshes
            .iter()
            .map(|mesh| GpuMeshInstance {
                model: mesh.model.to_cols_array(),
                color: mesh.color,
                flags: [mesh.lit as u32, mesh.cutoff.to_bits(), 0, 0],
            })
            .collect();
        self.resource_stats.instance_buffer_grows += u64::from(self.mesh_instances.upload(gpu, &mesh_data));
        let blend_mesh_data: Vec<GpuMeshInstance> = camera
            .blended_meshes
            .iter()
            .map(|mesh| GpuMeshInstance {
                model: mesh.model.to_cols_array(),
                color: mesh.color,
                flags: [mesh.lit as u32, mesh.cutoff.to_bits(), 0, 0],
            })
            .collect();
        self.resource_stats.instance_buffer_grows += u64::from(self.blend_mesh_instances.upload(gpu, &blend_mesh_data));
        let globals =
            GpuGlobals::from_camera(&camera.view, scene.ambient, scene.fog, scene.coplanar_thickness, &camera.lights);
        gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
    }

    /// One camera at 4×. Multisample color and depth match the image that camera renders: the
    /// surface, an offscreen target, or the internal-resolution image. A shaded camera resolves
    /// that viewport into the one-sample destination. A depth presentation writes the nearest
    /// reverse-Z sample instead of resolving color. A later [`CameraClear::Load`] camera expands
    /// the resolved destination into the multisample viewport before the scene pass.
    fn submit_msaa_camera(
        &mut self,
        gpu: &GpuContext,
        scene: &RenderScene,
        camera: &CameraPass,
        first: bool,
        target: &wgpu::TextureView,
        stats: &mut FrameStats,
    ) -> PassFrameStats {
        self.upload_camera(gpu, scene, camera);
        let scaled = match camera.view.fit {
            PresentationFit::Scaled(image) => Some(image),
            _ => None,
        };
        let visualize = camera.presentation == CameraPresentation::Depth;
        // The private internal image is cleared every frame. `first` still belongs to the public
        // destination, which the present pass uses.
        let scene_first = scaled.is_some() || first;
        let expand = !scene_first && !visualize && camera.clear == CameraClear::Load;
        let fill_viewport = !scene_first && !visualize && camera.clear == CameraClear::Color;
        if fill_viewport {
            let uniform =
                ClearUniform { color: [camera.clear_color.r, camera.clear_color.g, camera.clear_color.b, 1.0] };
            gpu.queue.write_buffer(&self.clear_uniform, 0, bytemuck::bytes_of(&uniform));
        }
        if let Some(image) = scaled {
            let plan = self.present_plan(camera, false, first);
            if plan.fill_draw {
                self.write_clear(gpu, [camera.letterbox_color.r, camera.letterbox_color.g, camera.letterbox_color.b]);
            }
            self.write_present(gpu, image);
        }
        if visualize {
            self.prepare_msaa_depth_view(gpu, camera);
        }

        let (expand_pipeline, expand_layout, resolve_pipeline, msaa_clear, msaa_debug, depth_pipeline) = {
            let extra = self.msaa4.as_ref().expect("4× pipelines exist while Four is selected");
            (
                extra.expand_pipeline.clone(),
                extra.expand_layout.clone(),
                extra.resolve_pipeline.clone(),
                extra.draw.clear.clone(),
                extra.draw.debug.clone(),
                extra.depth_view_pipeline.clone(),
            )
        };
        let attachments = self.msaa_attachments(camera, target);
        let expand_group = if expand {
            Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("msaa expand"),
                layout: &expand_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&attachments.destination),
                }],
            }))
        } else {
            None
        };
        if expand_group.is_some() {
            // The destination view is new for a headless frame and typically for a window frame,
            // so this group is not retained. Image-create counters do not include it.
            self.resource_stats.bind_group_creates += 1;
        }
        let depth_group = if visualize { Some(self.msaa_depth_group(camera)) } else { None };

        let mut one = PassFrameStats::default();
        let mut encoder =
            gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("msaa camera") });
        let (x, y, w, h) = clamp_viewport(attachments.viewport, attachments.size);
        if let Some(group) = expand_group.as_ref() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("msaa expand"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &attachments.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&expand_pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
            pass.draw(0..3, 0..1);
            record_expand(stats, &mut one);
        }
        {
            let color_load = if scene_first {
                wgpu::LoadOp::Clear(attachment_clear(camera.clear_color.r, camera.clear_color.g, camera.clear_color.b))
            } else {
                wgpu::LoadOp::Load
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("msaa camera"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &attachments.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: color_load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &attachments.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: if visualize { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard },
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
            if fill_viewport {
                pass.set_pipeline(&msaa_clear);
                pass.set_bind_group(0, &self.clear_bind_group, &[]);
                pass.draw(0..3, 0..1);
                record_clear(stats, &mut one);
            }
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_vertex_buffer(1, self.mesh_instances.buffer.slice(..));
            let mut i = 0;
            while i < camera.meshes.len() {
                let end = MeshInstance::run_end(&camera.meshes, i);
                let draw = camera.meshes[i];
                if let Some(mesh) = self.meshes.get(&draw.mesh.id()) {
                    pass.set_pipeline(self.mesh_pipeline(draw.draw));
                    let texture = draw.texture.and_then(|t| self.textures.get(&t.id())).unwrap_or(&self.white);
                    pass.set_bind_group(1, &texture.bind_group, &[]);
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, i as u32..end as u32);
                    record_world(stats, &mut one, (end - i) as u32);
                }
                i = end;
            }
            self.draw_sprites(&mut pass, &camera.opaque_sprites, &self.opaque_instances, stats, &mut one);
            self.draw_sprites(&mut pass, &camera.mask_sprites, &self.mask_instances, stats, &mut one);
            self.draw_sprites(&mut pass, &camera.cutout_sprites, &self.cutout_instances, stats, &mut one);
            self.draw_transparent(&mut pass, camera, stats, &mut one);
            if self.debug_vertex_count > 0 {
                pass.set_pipeline(&msaa_debug);
                pass.set_bind_group(0, &self.globals_bind_group, &[]);
                pass.set_vertex_buffer(0, self.debug_vertices.buffer.slice(..));
                pass.draw(0..self.debug_vertex_count, 0..1);
                record_world(stats, &mut one, self.debug_vertex_count);
            }
        }
        let destination_load = if scene_first {
            wgpu::LoadOp::Clear(attachment_clear(camera.clear_color.r, camera.clear_color.g, camera.clear_color.b))
        } else {
            wgpu::LoadOp::Load
        };
        if let Some(group) = depth_group.as_ref() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("msaa depth view"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &attachments.destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: destination_load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&depth_pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
            pass.draw(0..3, 0..1);
            record_depth_view(stats, &mut one);
        } else {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("msaa resolve"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &attachments.destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: destination_load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&resolve_pipeline);
            pass.set_bind_group(0, &attachments.resolve_group, &[]);
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
            pass.draw(0..3, 0..1);
            record_resolve(stats, &mut one);
        }
        if scaled.is_some() {
            self.record_present(&mut encoder, camera, target, first, stats, &mut one);
        }
        gpu.queue.submit([encoder.finish()]);
        one
    }

    /// Prepares the 4× images for one camera. A letterbox camera allocates none. A failed direct
    /// surface camera is [`FourPrep::Unavailable`] and does not become the destination's first writer.
    fn prepare_four_sample(&mut self, gpu: &GpuContext, camera: &CameraPass) -> FourPrep {
        if matches!(camera.view.fit, PresentationFit::Letterbox { .. }) {
            return FourPrep::Letterbox;
        }
        if let PresentationFit::Scaled(image) = camera.view.fit {
            if !self.ensure_internal(gpu, camera.camera_id, image.internal_width, image.internal_height) {
                return FourPrep::Letterbox;
            }
            if !self.ensure_named_msaa(
                gpu,
                MsaaSlot::Internal(camera.camera_id),
                image.internal_width,
                image.internal_height,
            ) {
                log::warn!(
                    "camera {} 4× images were not allocated; the viewport is filled with the letterbox color and the scene was not drawn at 1×",
                    camera.camera_id
                );
                return FourPrep::Letterbox;
            }
            return FourPrep::Ready;
        }
        match camera.output {
            CameraOutput::Target(id) => {
                let Some(stored) = self.render_targets.get(&id) else {
                    return FourPrep::Absent;
                };
                let (width, height) = (stored.width, stored.height);
                if self.ensure_named_msaa(gpu, MsaaSlot::Target(id), width, height) {
                    FourPrep::Ready
                } else {
                    FourPrep::Unavailable
                }
            }
            CameraOutput::Surface => {
                if self.prepare_msaa_images(gpu, self.surface_size) {
                    FourPrep::Ready
                } else {
                    FourPrep::Unavailable
                }
            }
        }
    }

    fn note_direct_surface_failure(&self, camera: &CameraPass, color_failed: &mut bool, load_failed: &mut bool) {
        if !matches!(camera.output, CameraOutput::Surface) || !matches!(camera.view.fit, PresentationFit::Direct) {
            return;
        }
        match camera.clear {
            CameraClear::Color => *color_failed = true,
            CameraClear::Load => *load_failed = true,
        }
    }

    fn ensure_named_msaa(&mut self, gpu: &GpuContext, slot: MsaaSlot, width: u32, height: u32) -> bool {
        if self.msaa4.is_none() {
            log::warn!("4× MSAA images were not allocated: pipelines are missing");
            return false;
        }
        let (width, height) = (width.max(1), height.max(1));
        if let Err(error) = validate_texture_dimensions(width, height, self.dimension_limits(gpu)) {
            log::warn!("4× MSAA images {width}x{height} were not allocated: {error}");
            return false;
        }
        let (matches_size, replacing) = {
            let current = match slot {
                MsaaSlot::Target(id) => self.render_targets.get(&id).and_then(|image| image.msaa.as_ref()),
                MsaaSlot::Internal(id) => self.internal_targets.get(&id).and_then(|image| image.msaa.as_ref()),
            };
            (current.is_some_and(|images| images.width == width && images.height == height), current.is_some())
        };
        if matches_size {
            return true;
        }
        let layout = self.msaa4.as_ref().expect("checked above").resolve_layout.clone();
        let images = MsaaImages::allocate(gpu, self.format, width, height, &layout);
        self.resource_stats.bind_group_creates += 1;
        if replacing {
            self.resource_stats.msaa_image_reallocs += 1;
        } else {
            self.resource_stats.msaa_image_creates += 1;
        }
        match slot {
            MsaaSlot::Target(id) => {
                if let Some(stored) = self.render_targets.get_mut(&id) {
                    stored.msaa = Some(images);
                }
            }
            MsaaSlot::Internal(id) => {
                if let Some(stored) = self.internal_targets.get_mut(&id) {
                    stored.msaa = Some(images);
                }
            }
        }
        true
    }

    fn prepare_msaa_depth_view(&mut self, gpu: &GpuContext, camera: &CameraPass) {
        let viewport = if let PresentationFit::Scaled(image) = camera.view.fit {
            [0.0, 0.0, image.internal_width as f32, image.internal_height as f32]
        } else {
            let size = match camera.output {
                CameraOutput::Surface => self.surface_size,
                CameraOutput::Target(id) => {
                    let image = &self.render_targets[&id];
                    (image.width, image.height)
                }
            };
            let (x, y, w, h) = clamp_viewport(camera.viewport_px, size);
            [x, y, w, h]
        };
        let uniform = DepthViewUniform::from_projection(camera.view.proj, viewport);
        gpu.queue.write_buffer(&self.depth_view_uniform, 0, bytemuck::bytes_of(&uniform));
        if !self.msaa_depth_group_missing(camera) {
            return;
        }
        let sample = self.msaa_depth_sample(camera);
        let layout = self.msaa4.as_ref().expect("4× pipelines").depth_view_layout.clone();
        let group = depth_sample_bind_group(&gpu.device, &layout, &sample, &self.depth_view_uniform);
        self.store_msaa_depth_group(camera, group);
        self.resource_stats.bind_group_creates += 1;
    }

    fn msaa_depth_group_missing(&self, camera: &CameraPass) -> bool {
        self.msaa_images_of(camera).depth_sample_bind_group.is_none()
    }

    fn msaa_depth_sample(&self, camera: &CameraPass) -> wgpu::TextureView {
        self.msaa_images_of(camera).depth_sample.clone()
    }

    fn msaa_depth_group(&self, camera: &CameraPass) -> wgpu::BindGroup {
        self.msaa_images_of(camera).depth_sample_bind_group.clone().expect("4× depth sample group")
    }

    fn msaa_images_of(&self, camera: &CameraPass) -> &MsaaImages {
        if matches!(camera.view.fit, PresentationFit::Scaled(_)) {
            return self.internal_targets[&camera.camera_id].msaa.as_ref().expect("internal 4× images");
        }
        match camera.output {
            CameraOutput::Surface => {
                self.msaa4.as_ref().expect("4× pipelines").images.as_ref().expect("surface 4× images")
            }
            CameraOutput::Target(id) => self.render_targets[&id].msaa.as_ref().expect("target 4× images"),
        }
    }

    fn store_msaa_depth_group(&mut self, camera: &CameraPass, group: wgpu::BindGroup) {
        let images = if matches!(camera.view.fit, PresentationFit::Scaled(_)) {
            self.internal_targets.get_mut(&camera.camera_id).expect("internal image").msaa.as_mut()
        } else {
            match camera.output {
                CameraOutput::Surface => self.msaa4.as_mut().expect("4× pipelines").images.as_mut(),
                CameraOutput::Target(id) => self.render_targets.get_mut(&id).expect("target").msaa.as_mut(),
            }
        };
        images.expect("4× images").depth_sample_bind_group = Some(group);
    }

    fn msaa_attachments(&self, camera: &CameraPass, surface: &wgpu::TextureView) -> MsaaAttachments {
        if let PresentationFit::Scaled(image) = camera.view.fit {
            let internal = &self.internal_targets[&camera.camera_id];
            let msaa = internal.msaa.as_ref().expect("internal 4× images");
            let size = (image.internal_width, image.internal_height);
            return MsaaAttachments {
                color: msaa.color_attachment.clone(),
                depth: msaa.depth_attachment.clone(),
                resolve_group: msaa.resolve_group.clone(),
                destination: internal.view.clone(),
                size,
                viewport: [0.0, 0.0, size.0 as f32, size.1 as f32],
            };
        }
        match camera.output {
            CameraOutput::Surface => {
                let images = self.msaa4.as_ref().expect("4× pipelines").images.as_ref().expect("surface 4× images");
                MsaaAttachments {
                    color: images.color_attachment.clone(),
                    depth: images.depth_attachment.clone(),
                    resolve_group: images.resolve_group.clone(),
                    destination: surface.clone(),
                    size: (images.width, images.height),
                    viewport: camera.viewport_px,
                }
            }
            CameraOutput::Target(id) => {
                let stored = &self.render_targets[&id];
                let msaa = stored.msaa.as_ref().expect("target 4× images");
                MsaaAttachments {
                    color: msaa.color_attachment.clone(),
                    depth: msaa.depth_attachment.clone(),
                    resolve_group: msaa.resolve_group.clone(),
                    destination: stored.view.clone(),
                    size: (stored.width, stored.height),
                    viewport: camera.viewport_px,
                }
            }
        }
    }

    /// Allocates or reuses the 4× surface images. `false` leaves the selected count at 4× and
    /// draws nothing for this frame.
    fn prepare_msaa_images(&mut self, gpu: &GpuContext, size: (u32, u32)) -> bool {
        if self.msaa4.is_none() {
            log::warn!("4× MSAA images were not allocated: pipelines are missing");
            return false;
        }
        let (width, height) = (size.0.max(1), size.1.max(1));
        if let Err(error) = validate_texture_dimensions(width, height, self.dimension_limits(gpu)) {
            log::warn!("4× MSAA images {width}x{height} were not allocated: {error}");
            return false;
        }
        if let Some(current) = self.msaa4.as_ref().and_then(|extra| extra.images.as_ref()) {
            if current.width == width && current.height == height {
                return true;
            }
        }
        let replacing = self.msaa4.as_ref().and_then(|extra| extra.images.as_ref()).is_some();
        let images = MsaaImages::allocate(
            gpu,
            self.format,
            width,
            height,
            &self.msaa4.as_ref().expect("checked above").resolve_layout,
        );
        self.resource_stats.bind_group_creates += 1;
        if replacing {
            self.resource_stats.msaa_image_reallocs += 1;
        } else {
            self.resource_stats.msaa_image_creates += 1;
        }
        self.msaa4.as_mut().expect("checked above").images = Some(images);
        true
    }

    fn drop_msaa_images(&mut self) {
        if self.msaa4.as_mut().and_then(|extra| extra.images.take()).is_some() {
            self.resource_stats.msaa_image_drops += 1;
        }
        for target in self.render_targets.values_mut() {
            if target.msaa.take().is_some() {
                self.resource_stats.msaa_image_drops += 1;
            }
        }
        for internal in self.internal_targets.values_mut() {
            if internal.msaa.take().is_some() {
                self.resource_stats.msaa_image_drops += 1;
            }
        }
    }

    fn warn_msaa_skip(&mut self, camera_id: u32, reason: MsaaSkip) {
        if !self.msaa_skip_warned.insert((camera_id, reason)) {
            return;
        }
        log::warn!("{}", reason.message(camera_id));
    }

    /// A 4× frame whose surface cameras were all skipped. Opaque black is defined output.
    /// It is not the skipped camera's clear color, and it is not a one-sample scene.
    fn clear_undefined_surface(&mut self, gpu: &GpuContext, target: &wgpu::TextureView) {
        if !self.msaa_blank_logged {
            self.msaa_blank_logged = true;
            log::warn!(
                "4× MSAA drew no surface camera; the destination was cleared to opaque black and 1× was not used"
            );
        }
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("msaa undefined destination") });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("msaa undefined destination"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        gpu.queue.submit([encoder.finish()]);
    }
}

fn record_world(stats: &mut FrameStats, pass: &mut PassFrameStats, instances: u32) {
    stats.draw_calls += 1;
    stats.world_draw_calls += 1;
    stats.submitted_instances += instances;
    pass.world_draw_calls += 1;
    pass.submitted_instances += instances;
    if instances > stats.largest_draw_instances {
        stats.largest_draw_instances = instances;
    }
}

fn record_clear(stats: &mut FrameStats, pass: &mut PassFrameStats) {
    stats.draw_calls += 1;
    stats.clear_draw_calls += 1;
    pass.clear_draw_calls += 1;
}

fn record_presentation(stats: &mut FrameStats, pass: &mut PassFrameStats) {
    stats.draw_calls += 1;
    stats.presentation_draws += 1;
    pass.presentation_draws += 1;
}

fn record_depth_view(stats: &mut FrameStats, pass: &mut PassFrameStats) {
    stats.draw_calls += 1;
    stats.depth_view_draws += 1;
    pass.depth_view_draws += 1;
}

fn record_resolve(stats: &mut FrameStats, pass: &mut PassFrameStats) {
    stats.draw_calls += 1;
    stats.resolve_draws += 1;
    pass.resolve_draws += 1;
}

fn record_expand(stats: &mut FrameStats, pass: &mut PassFrameStats) {
    stats.draw_calls += 1;
    stats.expand_draws += 1;
    pass.expand_draws += 1;
}

fn count_pass_instances(camera: &CameraPass, stats: &mut FrameStats, one: &mut PassFrameStats) {
    one.sprite_instances = (camera.opaque_sprites.instances.len()
        + camera.mask_sprites.instances.len()
        + camera.cutout_sprites.instances.len()
        + camera.blended_sprites.instances.len()) as u32;
    one.mesh_instances = (camera.meshes.len() + camera.blended_meshes.len()) as u32;
    stats.sprite_instances += one.sprite_instances;
    stats.mesh_instances += one.mesh_instances;
}

/// Both formats must support 4 samples. `MULTISAMPLE_RESOLVE` is not required: the resolve is a shader.
pub fn msaa4_supported(color: wgpu::TextureFormatFeatureFlags, depth: wgpu::TextureFormatFeatureFlags) -> bool {
    color.sample_count_supported(4) && depth.sample_count_supported(4)
}

fn attachment_clear(r: f32, g: f32, b: f32) -> wgpu::Color {
    wgpu::Color { r: r as f64, g: g as f64, b: b as f64, a: 1.0 }
}

fn clamp_viewport(viewport_px: [f32; 4], size: (u32, u32)) -> (f32, f32, f32, f32) {
    let [vx, vy, vw, vh] = viewport_px;
    let (x, y) = (vx.min(size.0 as f32 - 1.0), vy.min(size.1 as f32 - 1.0));
    let (w, h) = (vw.min(size.0 as f32 - x).max(1.0), vh.min(size.1 as f32 - y).max(1.0));
    (x, y, w, h)
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ClearUniform {
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PresentUniform {
    origin: [f32; 2],
    scale: u32,
    _pad: u32,
}

struct PresentPlan {
    fill_draw: bool,
    clear_with_letterbox: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DepthViewUniform {
    inv_proj: [f32; 16],
    viewport: [f32; 4],
    empty_color: [f32; 4],
    unsupported_color: [f32; 4],
    mode: u32,
    min_span: f32,
    _pad: [u32; 2],
}

impl DepthViewUniform {
    fn from_projection(proj: Mat4, viewport: [f32; 4]) -> Self {
        let (inv_proj, mode) = match proj.inverse() {
            Some(inverse) => (inverse.to_cols_array(), 1),
            None => ([0.0; 16], 0),
        };
        Self {
            inv_proj,
            viewport,
            empty_color: DEPTH_VIEW_EMPTY,
            unsupported_color: DEPTH_VIEW_UNSUPPORTED,
            mode,
            min_span: DEPTH_VIEW_MIN_SPAN,
            _pad: [0; 2],
        }
    }
}

impl DepthImage {
    fn allocate(device: &wgpu::Device, size: (u32, u32), label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let attachment = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sample = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, attachment, sample, sample_bind_group: None, size }
    }
}

fn depth_sample_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("depth view"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: uniform.as_entire_binding() },
        ],
    })
}

struct Msaa4 {
    draw: crate::draw_pipelines::DrawPipelines,
    resolve_pipeline: wgpu::RenderPipeline,
    resolve_layout: wgpu::BindGroupLayout,
    expand_pipeline: wgpu::RenderPipeline,
    expand_layout: wgpu::BindGroupLayout,
    depth_view_pipeline: wgpu::RenderPipeline,
    depth_view_layout: wgpu::BindGroupLayout,
    images: Option<MsaaImages>,
}

struct MsaaImages {
    width: u32,
    height: u32,
    #[allow(dead_code)]
    color: wgpu::Texture,
    color_attachment: wgpu::TextureView,
    #[allow(dead_code)]
    depth: wgpu::Texture,
    depth_attachment: wgpu::TextureView,
    /// Second view of `depth`, sampled by the 4× depth presentation. Not bound in the scene pass.
    depth_sample: wgpu::TextureView,
    depth_sample_bind_group: Option<wgpu::BindGroup>,
    resolve_group: wgpu::BindGroup,
}

struct MsaaAttachments {
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve_group: wgpu::BindGroup,
    /// One-sample image the resolve or depth presentation writes. The expand pass samples it.
    destination: wgpu::TextureView,
    size: (u32, u32),
    viewport: [f32; 4],
}

enum FourPrep {
    Ready,
    Letterbox,
    Absent,
    Unavailable,
}

#[derive(Clone, Copy)]
enum MsaaSlot {
    Target(RenderTargetId),
    Internal(u32),
}

impl Msaa4 {
    fn create(
        gpu: &GpuContext,
        format: wgpu::TextureFormat,
        scene_layout: &wgpu::PipelineLayout,
        debug_layout: &wgpu::PipelineLayout,
        clear_pipeline_layout: &wgpu::PipelineLayout,
    ) -> Self {
        let draw = crate::draw_pipelines::make_draw_pipelines(
            &gpu.device,
            format,
            4,
            scene_layout,
            debug_layout,
            clear_pipeline_layout,
        );
        let resolve_layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("msaa resolve layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: true,
                },
                count: None,
            }],
        });
        let shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("msaa resolve"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/msaa_resolve.wgsl").into()),
        });
        let resolve_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("msaa resolve"),
            layout: Some(&gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("msaa resolve layout"),
                bind_group_layouts: &[Some(&resolve_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_resolve"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_resolve"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let expand_layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("msaa expand layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let expand_shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("msaa expand"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/msaa_expand.wgsl").into()),
        });
        let samples = wgpu::MultisampleState { count: 4, mask: !0, alpha_to_coverage_enabled: false };
        let expand_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("msaa expand"),
            layout: Some(&gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("msaa expand layout"),
                bind_group_layouts: &[Some(&expand_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &expand_shader,
                entry_point: Some("vs_expand"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &expand_shader,
                entry_point: Some("fs_expand"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: samples,
            multiview_mask: None,
            cache: None,
        });
        let depth_view_layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("msaa depth view layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: true,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let depth_shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("msaa depth view"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/depth_view_msaa.wgsl").into()),
        });
        let depth_view_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("msaa depth view"),
            layout: Some(&gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("msaa depth view layout"),
                bind_group_layouts: &[Some(&depth_view_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &depth_shader,
                entry_point: Some("vs_depth"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &depth_shader,
                entry_point: Some("fs_depth"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            draw,
            resolve_pipeline,
            resolve_layout,
            expand_pipeline,
            expand_layout,
            depth_view_pipeline,
            depth_view_layout,
            images: None,
        }
    }
}

impl MsaaImages {
    fn allocate(
        gpu: &GpuContext,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        resolve_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let color = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("msaa color"),
            size,
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_attachment = color.create_view(&wgpu::TextureViewDescriptor::default());
        let color_sample = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("msaa depth"),
            size,
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_attachment = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_sample = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let resolve_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("msaa resolve"),
            layout: resolve_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&color_sample),
            }],
        });
        Self {
            width,
            height,
            color,
            color_attachment,
            depth,
            depth_attachment,
            depth_sample,
            depth_sample_bind_group: None,
            resolve_group,
        }
    }
}

fn collect_debug_vertices(segments: &[DebugSegment]) -> Vec<GpuDebugVertex> {
    let mut vertices = Vec::with_capacity(segments.len() * 2);
    for segment in segments {
        if !finite_debug_segment(segment) {
            continue;
        }
        vertices.push(GpuDebugVertex { position: segment.start.extend(1.0).to_array(), color: segment.color });
        vertices.push(GpuDebugVertex { position: segment.end.extend(1.0).to_array(), color: segment.color });
    }
    vertices
}

fn finite_debug_segment(segment: &DebugSegment) -> bool {
    let finite_point = |x: f32, y: f32, z: f32| x.is_finite() && y.is_finite() && z.is_finite();
    finite_point(segment.start.x, segment.start.y, segment.start.z)
        && finite_point(segment.end.x, segment.end.y, segment.end.z)
        && segment.color.iter().all(|channel| channel.is_finite())
        && segment.start.distance_squared(segment.end) > 0.0
}

fn sprite_instances(layer: &SpriteLayer) -> Vec<GpuSpriteInstance> {
    layer
        .instances
        .iter()
        .map(|i| GpuSpriteInstance {
            model: i.model.to_cols_array(),
            uv: i.uv,
            tint: i.tint,
            flags: [
                i.lit as u32,
                i.shading.cutoff_bits(),
                u32::from(i.coplanar_level.is_some()),
                u32::from(i.coplanar_level.unwrap_or(0)),
            ],
        })
        .collect()
}

struct ImageUpload<'a> {
    sampling: EffectiveSampling,
    width: u32,
    height: u32,
    rgba: &'a [u8],
    format: wgpu::TextureFormat,
    mip_levels: u32,
    label: &'a str,
    fingerprint: u64,
}

fn upload_image(
    gpu: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    image: ImageUpload<'_>,
    stats: &mut GpuResourceStats,
) -> GpuTexture {
    let size = wgpu::Extent3d { width: image.width, height: image.height, depth_or_array_layers: 1 };
    let mut usage =
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC;
    if image.mip_levels > 1 {
        usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(image.label),
        size,
        mip_level_count: image.mip_levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: image.format,
        usage,
        view_formats: &[],
    });
    stats.texture_creates += 1;
    write_base_level(&gpu.queue, &texture, image.width, image.height, image.rgba);
    note_full_upload(stats, image.rgba.len() as u64);
    let view = sampling_view(&texture, image.sampling.view_levels);
    let bind_group = texture_bind_group(&gpu.device, layout, &view, sampler, image.label);
    stats.bind_group_creates += 1;
    GpuTexture {
        revision: 0,
        last_used: 0,
        fingerprint: image.fingerprint,
        width: image.width,
        height: image.height,
        format: image.format,
        mip_level_count: image.mip_levels,
        sampling: image.sampling,
        view,
        bind_group,
        texture,
    }
}

fn note_full_upload(stats: &mut GpuResourceStats, bytes: u64) {
    stats.texture_writes += 1;
    stats.full_base_uploads += 1;
    stats.base_upload_bytes += bytes;
}

fn note_partial_upload(stats: &mut GpuResourceStats, bytes: u64) {
    stats.texture_writes += 1;
    stats.partial_base_uploads += 1;
    stats.base_upload_bytes += bytes;
}

fn base_bytes_match(rgba: &[u8], width: u32, height: u32) -> bool {
    match u64::from(width).checked_mul(u64::from(height)).and_then(|texels| texels.checked_mul(4)) {
        Some(len) => rgba.len() as u64 == len,
        None => false,
    }
}

struct BasePixels<'a> {
    rgba: &'a [u8],
    width: u32,
    height: u32,
    fingerprint: u64,
    edits: &'a TexturePixelEdits,
    levels: u32,
}

/// `Some(rebuild_mips)` when the GPU base level now matches `pixels.fingerprint`.
/// `None` when the bytes could not be validated, in which case nothing was submitted.
fn apply_base_upload(
    queue: &wgpu::Queue,
    cached: &mut GpuTexture,
    pixels: BasePixels<'_>,
    stats: &mut GpuResourceStats,
) -> Option<bool> {
    match pixels.edits.base_upload(Some(cached.fingerprint), pixels.fingerprint) {
        BaseLevelUpload::Unchanged => Some(false),
        BaseLevelUpload::Partial(edits) if !edits.is_empty() => {
            if let Some(packed) = pack_regions(pixels.rgba, pixels.width, pixels.height, edits) {
                let mut submitted = true;
                for (rect, bytes) in &packed {
                    if !write_region(queue, &cached.texture, *rect, bytes) {
                        submitted = false;
                        break;
                    }
                    note_partial_upload(stats, bytes.len() as u64);
                }
                if submitted {
                    cached.fingerprint = pixels.fingerprint;
                    return Some(pixels.levels > 1);
                }
            }
            submit_full_base(queue, cached, &pixels, stats)
        }
        BaseLevelUpload::Partial(_) | BaseLevelUpload::Full => submit_full_base(queue, cached, &pixels, stats),
    }
}

fn submit_full_base(
    queue: &wgpu::Queue,
    cached: &mut GpuTexture,
    pixels: &BasePixels<'_>,
    stats: &mut GpuResourceStats,
) -> Option<bool> {
    if !write_base_level(queue, &cached.texture, pixels.width, pixels.height, pixels.rgba) {
        return None;
    }
    note_full_upload(stats, pixels.rgba.len() as u64);
    cached.fingerprint = pixels.fingerprint;
    Some(pixels.levels > 1)
}

fn write_base_level(queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32, rgba: &[u8]) -> bool {
    if !base_bytes_match(rgba, width, height) {
        return false;
    }
    let Some(row_bytes) = width.checked_mul(4) else { return false };
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row_bytes), rows_per_image: Some(height) },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    true
}

fn write_region(queue: &wgpu::Queue, texture: &wgpu::Texture, rect: TexelRect, rgba: &[u8]) -> bool {
    let Some(row_bytes) = rect.width.checked_mul(4) else { return false };
    let Some(expected) = u64::from(row_bytes).checked_mul(u64::from(rect.height)) else { return false };
    if rgba.len() as u64 != expected {
        return false;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: rect.x, y: rect.y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row_bytes), rows_per_image: Some(rect.height) },
        wgpu::Extent3d { width: rect.width, height: rect.height, depth_or_array_layers: 1 },
    );
    true
}

fn pack_regions(rgba: &[u8], width: u32, height: u32, edits: &[TextureEdit]) -> Option<Vec<(TexelRect, Vec<u8>)>> {
    let mut packed = Vec::with_capacity(edits.len());
    for edit in edits {
        packed.push((edit.rect, pack_rect(rgba, width, height, edit.rect)?));
    }
    Some(packed)
}

fn pack_rect(rgba: &[u8], texture_width: u32, texture_height: u32, rect: TexelRect) -> Option<Vec<u8>> {
    if rect.width == 0 || rect.height == 0 {
        return None;
    }
    let x_end = rect.x.checked_add(rect.width)?;
    let y_end = rect.y.checked_add(rect.height)?;
    if x_end > texture_width || y_end > texture_height {
        return None;
    }
    let row_bytes = u64::from(rect.width).checked_mul(4)?;
    let row_bytes_u32 = u32::try_from(row_bytes).ok()?;
    let total = row_bytes.checked_mul(u64::from(rect.height))?;
    let stride = u64::from(texture_width).checked_mul(4)?;
    let total_usize = usize::try_from(total).ok()?;
    let mut out = vec![0u8; total_usize];
    for row in 0..rect.height {
        let y = u64::from(rect.y) + u64::from(row);
        let start = y.checked_mul(stride)?.checked_add(u64::from(rect.x).checked_mul(4)?)?;
        let start = usize::try_from(start).ok()?;
        let end = start.checked_add(row_bytes_u32 as usize)?;
        let src = rgba.get(start..end)?;
        let dest = (u64::from(row) * row_bytes) as usize;
        out[dest..dest + src.len()].copy_from_slice(src);
    }
    Some(out)
}

fn read_texture_level(
    gpu: &GpuContext,
    texture: &wgpu::Texture,
    level: u32,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let row_bytes = width.checked_mul(4)?;
    let padded = row_bytes.checked_next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)?;
    let size = u64::from(padded).checked_mul(u64::from(height))?;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("texture readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder =
        gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("texture read") });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: level,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    rx.recv().ok()?.ok()?;
    let data = buffer.slice(..).get_mapped_range().ok()?;
    let mut out = Vec::with_capacity((row_bytes as usize).saturating_mul(height as usize));
    for row in data.chunks(padded as usize).take(height as usize) {
        let Some(texels) = row.get(..row_bytes as usize) else {
            drop(data);
            buffer.unmap();
            return None;
        };
        out.extend_from_slice(texels);
    }
    drop(data);
    buffer.unmap();
    Some(out)
}

fn texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn sampling_view(texture: &wgpu::Texture, view_levels: u32) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("texture sample"),
        base_mip_level: 0,
        mip_level_count: Some(view_levels.max(1)),
        dimension: Some(wgpu::TextureViewDimension::D2),
        ..Default::default()
    })
}

fn gpu_format(color_space: ColorSpace) -> wgpu::TextureFormat {
    match color_space {
        ColorSpace::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        ColorSpace::Linear => wgpu::TextureFormat::Rgba8Unorm,
    }
}

fn color_space_of(format: wgpu::TextureFormat) -> ColorSpace {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => ColorSpace::Linear,
        _ => ColorSpace::Srgb,
    }
}

/// CPU normalization, then the device anisotropy fallback, then the LOD clamp for `allocated_levels`.
///
/// A chain is sampled only when the descriptor requests a mip filter and more than one level was
/// allocated. Otherwise the sampling view is the base level and the LOD upper bound stays at
/// [`SINGLE_LEVEL_LOD_MAX`], so minification still uses `min_filter`. Clamping that case to `0..0`
/// selects the magnification filter on Metal. `create_sampler` is not retried: invalid anisotropy is
/// corrected here, and a later HAL failure is a device error.
fn effective_sampling(sampler: SamplerDesc, anisotropic_filtering: bool, allocated_levels: u32) -> EffectiveSampling {
    let (sampler, invalid_anisotropy) = match sampler.normalized() {
        Ok(sampler) => (sampler, false),
        Err(_) => (SamplerDesc { anisotropy: 1, ..sampler }, true),
    };
    let capability_fallback = sampler.anisotropy > 1 && !anisotropic_filtering;
    let anisotropy = if capability_fallback { 1 } else { sampler.anisotropy };
    let mip_linear = sampler.mip_filter == Some(MipFilter::Linear);
    let sample_chain = sampler.mip_filter.is_some() && allocated_levels > 1;
    let view_levels = if sample_chain { allocated_levels } else { 1 };
    let lod_min: f32 = 0.0;
    let lod_max: f32 = if sample_chain { (allocated_levels - 1) as f32 } else { SINGLE_LEVEL_LOD_MAX };
    EffectiveSampling {
        key: SamplerKey {
            min_linear: sampler.min_filter == TextureFilter::Linear,
            mag_linear: sampler.mag_filter == TextureFilter::Linear,
            mip_linear,
            address_u: sampler.address_u,
            address_v: sampler.address_v,
            anisotropy,
            lod_min_bits: lod_min.to_bits(),
            lod_max_bits: lod_max.to_bits(),
        },
        min_filter: sampler.min_filter,
        mag_filter: sampler.mag_filter,
        mip_linear,
        address_u: sampler.address_u,
        address_v: sampler.address_v,
        anisotropy,
        lod_min,
        lod_max,
        view_levels,
        capability_fallback,
        invalid_anisotropy,
    }
}

fn sampler_descriptor(sampling: &EffectiveSampling) -> wgpu::SamplerDescriptor<'static> {
    wgpu::SamplerDescriptor {
        label: Some("sampler"),
        address_mode_u: wgpu_address(sampling.address_u),
        address_mode_v: wgpu_address(sampling.address_v),
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu_filter(sampling.mag_filter),
        min_filter: wgpu_filter(sampling.min_filter),
        mipmap_filter: if sampling.mip_linear {
            wgpu::MipmapFilterMode::Linear
        } else {
            wgpu::MipmapFilterMode::Nearest
        },
        lod_min_clamp: sampling.lod_min,
        lod_max_clamp: sampling.lod_max,
        compare: None,
        anisotropy_clamp: u16::from(sampling.anisotropy),
        border_color: None,
    }
}

fn wgpu_filter(filter: TextureFilter) -> wgpu::FilterMode {
    match filter {
        TextureFilter::Nearest => wgpu::FilterMode::Nearest,
        TextureFilter::Linear => wgpu::FilterMode::Linear,
    }
}

fn wgpu_address(mode: AddressMode) -> wgpu::AddressMode {
    match mode {
        AddressMode::Clamp => wgpu::AddressMode::ClampToEdge,
        AddressMode::Repeat => wgpu::AddressMode::Repeat,
        AddressMode::Mirror => wgpu::AddressMode::MirrorRepeat,
    }
}

fn pixel_fingerprint(rgba: &[u8]) -> u64 {
    engine_assets::rgba_fingerprint(rgba)
}

#[cfg(test)]
mod sampling_tests {
    use super::*;

    fn linear_aniso(anisotropy: u8) -> SamplerDesc {
        SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Linear,
            mip_filter: Some(MipFilter::Linear),
            anisotropy,
            ..SamplerDesc::default()
        }
    }

    #[test]
    fn unsupported_anisotropy_becomes_one_and_keeps_filters() {
        let sampling = effective_sampling(linear_aniso(8), false, 1);
        assert!(sampling.capability_fallback);
        assert_eq!(sampling.anisotropy, 1);
        assert_eq!(sampling.min_filter, TextureFilter::Linear);
        assert_eq!(sampling.mag_filter, TextureFilter::Linear);
        assert!(sampling.mip_linear);
        assert_eq!(sampler_descriptor(&sampling).anisotropy_clamp, 1);
    }

    #[test]
    fn supported_anisotropy_reaches_the_descriptor() {
        let sampling = effective_sampling(linear_aniso(8), true, 1);
        assert!(!sampling.capability_fallback);
        assert_eq!(sampling.anisotropy, 8);
        assert_eq!(sampler_descriptor(&sampling).anisotropy_clamp, 8);
    }

    #[test]
    fn nearest_is_not_rewritten_and_non_power_of_two_is_kept() {
        let nearest = effective_sampling(
            SamplerDesc {
                min_filter: TextureFilter::Nearest,
                mag_filter: TextureFilter::Linear,
                mip_filter: Some(MipFilter::Linear),
                anisotropy: 8,
                ..SamplerDesc::default()
            },
            true,
            1,
        );
        assert_eq!(nearest.anisotropy, 1);
        assert_eq!(nearest.min_filter, TextureFilter::Nearest);
        assert_eq!(nearest.mag_filter, TextureFilter::Linear);

        let odd = effective_sampling(linear_aniso(3), true, 1);
        assert_eq!(odd.anisotropy, 3);
        assert_eq!(sampler_descriptor(&odd).anisotropy_clamp, 3);
    }

    #[test]
    fn anisotropy_below_one_is_corrected_before_sampler_creation() {
        let sampling = effective_sampling(SamplerDesc { anisotropy: 0, ..SamplerDesc::default() }, true, 1);
        assert!(sampling.invalid_anisotropy);
        assert_eq!(sampling.anisotropy, 1);
        assert_eq!(sampling.min_filter, TextureFilter::Nearest);
        assert_eq!(sampler_descriptor(&sampling).anisotropy_clamp, 1);
    }

    #[test]
    fn one_mip_level_clamps_lod_and_collapses_nearest_mip_filters() {
        let base = effective_sampling(SamplerDesc::default(), true, 1);
        let nearest =
            effective_sampling(SamplerDesc { mip_filter: Some(MipFilter::Nearest), ..SamplerDesc::default() }, true, 1);
        let linear =
            effective_sampling(SamplerDesc { mip_filter: Some(MipFilter::Linear), ..SamplerDesc::default() }, true, 1);
        assert_eq!(base.lod_min, 0.0);
        assert_eq!(base.lod_max, SINGLE_LEVEL_LOD_MAX);
        assert_eq!(base.key, nearest.key);
        assert_ne!(base.key, linear.key);
        let descriptor = sampler_descriptor(&linear);
        assert_eq!(descriptor.lod_min_clamp, 0.0);
        assert_eq!(descriptor.lod_max_clamp, SINGLE_LEVEL_LOD_MAX);
        assert_eq!(descriptor.mipmap_filter, wgpu::MipmapFilterMode::Linear);
        assert_eq!(descriptor.min_filter, wgpu::FilterMode::Nearest);
        assert_eq!(descriptor.mag_filter, wgpu::FilterMode::Nearest);
    }

    #[test]
    fn address_modes_and_split_filters_stay_independent() {
        let sampling = effective_sampling(
            SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Nearest,
                address_u: AddressMode::Repeat,
                address_v: AddressMode::Mirror,
                ..SamplerDesc::default()
            },
            true,
            1,
        );
        let descriptor = sampler_descriptor(&sampling);
        assert_eq!(descriptor.min_filter, wgpu::FilterMode::Linear);
        assert_eq!(descriptor.mag_filter, wgpu::FilterMode::Nearest);
        assert_eq!(descriptor.address_mode_u, wgpu::AddressMode::Repeat);
        assert_eq!(descriptor.address_mode_v, wgpu::AddressMode::MirrorRepeat);
        assert_eq!(descriptor.address_mode_w, wgpu::AddressMode::ClampToEdge);
    }

    #[test]
    fn allocated_levels_set_lod_max_only_when_a_mip_filter_is_requested() {
        let stored = effective_sampling(SamplerDesc::default(), true, 5);
        assert_eq!(stored.view_levels, 1);
        assert_eq!(stored.lod_max, SINGLE_LEVEL_LOD_MAX);
        assert!(!stored.mip_linear);

        let nearest =
            effective_sampling(SamplerDesc { mip_filter: Some(MipFilter::Nearest), ..SamplerDesc::default() }, true, 5);
        let linear =
            effective_sampling(SamplerDesc { mip_filter: Some(MipFilter::Linear), ..SamplerDesc::default() }, true, 5);
        assert_eq!(nearest.view_levels, 5);
        assert_eq!(linear.view_levels, 5);
        assert_eq!(nearest.lod_max, 4.0);
        assert_eq!(linear.lod_max, 4.0);
        assert!(!nearest.mip_linear);
        assert!(linear.mip_linear);
        assert_ne!(stored.key, nearest.key);
        assert_ne!(nearest.key, linear.key);
        assert_eq!(sampler_descriptor(&nearest).lod_max_clamp, 4.0);
    }
}
