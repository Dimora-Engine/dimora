use std::collections::{HashMap, HashSet};

use bytemuck::Pod;
use engine_assets::{
    validate_texture_dimensions, AddressMode, AssetHandle, AssetId, AssetManager, BaseLevelUpload, ColorSpace,
    MeshData, MipFilter, SamplerDesc, TexelRect, Texture, TextureDimensionLimits, TextureEdit, TextureFilter,
    TexturePixelEdits,
};
use engine_renderer::{RenderScene, SpriteLayer};
use wgpu::util::DeviceExt;

use crate::mip::{self, MipGenerator};

/// LOD clamp for a texture whose sampling view contains only the base level.
///
/// This is wgpu 30.0.1's default `lod_max_clamp`. Clamping to `0..0` makes minification use the
/// magnification filter: the clamped LOD is what selects min versus mag, and Metal turns nearest
/// mip filtering plus `0..0` into a non-mipmapped sampler that ignores `minFilter`. The texture
/// view still contains only level 0, so a wider clamp does not sample a mip that was not allocated.
const SINGLE_LEVEL_LOD_MAX: f32 = 32.0;

use crate::gpu_types::{GpuGlobals, GpuMeshInstance, GpuSpriteInstance};
use crate::{GpuContext, RenderError};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADER_COMMON: &str = include_str!("shaders/common.wgsl");

/// Per-frame counters, handy for HUDs and for asserting batching in tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub draw_calls: u32,
    pub sprite_instances: u32,
    pub mesh_instances: u32,
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
    fn upload<T: Pod>(&mut self, gpu: &GpuContext, data: &[T]) {
        let bytes: &[u8] = bytemuck::cast_slice(data);
        if bytes.is_empty() {
            return;
        }
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.buffer = Self::alloc(&gpu.device, self.label, self.capacity);
        }
        gpu.queue.write_buffer(&self.buffer, 0, bytes);
    }
}

/// Draws [`RenderScene`]s into any texture view of a given color format.
///
/// Pass order in one render pass: opaque meshes, cutout sprites/tiles (both write depth), then
/// alpha-blended sprites back-to-front (depth-tested, no depth write). Real depth testing is what
/// lets sprites and 3D geometry interleave correctly.
pub struct SceneRenderer {
    format: wgpu::TextureFormat,
    globals: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
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
    mesh_pipeline: wgpu::RenderPipeline,
    textures: HashMap<AssetId, GpuTexture>,
    white: GpuTexture,
    missing: GpuTexture,
    meshes: HashMap<AssetId, GpuMesh>,
    cutout_instances: InstanceBuffer,
    blend_instances: InstanceBuffer,
    mesh_instances: InstanceBuffer,
    depth: Option<(wgpu::TextureView, (u32, u32))>,
    stats: FrameStats,
    /// Number of `render` calls so far; the clock for idle eviction.
    frame: u64,
    cache_max_idle_frames: u32,
    /// Tighter than the device limit when set. `None` uses the device limit on both axes.
    dimension_limit_override: Option<TextureDimensionLimits>,
    /// Asset revision already refused, so a later frame does not hash or allocate it again.
    refused_uploads: HashMap<AssetId, u32>,
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

        let sprite_module = shader(device, "sprite", include_str!("shaders/sprite.wgsl"));
        let mesh_module = shader(device, "mesh", include_str!("shaders/mesh.wgsl"));

        let instance_attrs_sprite = wgpu::vertex_attr_array![
            0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
            4 => Float32x4, 5 => Float32x4, 6 => Uint32x4
        ];
        let sprite_instance_layout = wgpu::VertexBufferLayout {
            array_stride: core::mem::size_of::<GpuSpriteInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attrs_sprite,
        };
        let sprite_pipeline = |label, entry, depth_write: bool, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &sprite_module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(sprite_instance_layout.clone())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &sprite_module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(depth_state(depth_write)),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let sprite_cutout = sprite_pipeline("sprite cutout", "fs_cutout", true, None);
        let sprite_blend = sprite_pipeline("sprite blend", "fs_blend", false, Some(wgpu::BlendState::ALPHA_BLENDING));

        let mesh_vertex_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let mesh_instance_attrs = wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Uint32x4
        ];
        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &mesh_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: core::mem::size_of::<engine_assets::Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &mesh_vertex_attrs,
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: core::mem::size_of::<GpuMeshInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &mesh_instance_attrs,
                    }),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mesh_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(true)),
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
            white,
            missing,
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
            sprite_cutout,
            sprite_blend,
            mesh_pipeline,
            textures: HashMap::new(),
            meshes: HashMap::new(),
            cutout_instances: InstanceBuffer::new(device, "cutout sprite instances"),
            blend_instances: InstanceBuffer::new(device, "blended sprite instances"),
            mesh_instances: InstanceBuffer::new(device, "mesh instances"),
            depth: None,
            stats: FrameStats::default(),
            frame: 0,
            cache_max_idle_frames: DEFAULT_CACHE_MAX_IDLE_FRAMES,
            dimension_limit_override: None,
            refused_uploads: HashMap::new(),
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Counters from the most recent [`SceneRenderer::render`].
    pub fn stats(&self) -> FrameStats {
        self.stats
    }

    /// How many textures and meshes are cached on the GPU right now.
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats { textures: self.textures.len(), meshes: self.meshes.len() }
    }

    /// Counts of GPU image, upload, sampler, and bind-group work since this renderer was created.
    pub fn resource_stats(&self) -> GpuResourceStats {
        self.resource_stats
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
    pub fn render(
        &mut self,
        gpu: &GpuContext,
        scene: &RenderScene,
        assets: &AssetManager,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&GpuGlobals::from_scene(scene)));

        self.frame += 1;
        for layer in [&scene.cutout_sprites, &scene.blended_sprites] {
            for batch in &layer.batches {
                self.ensure_texture(gpu, assets, batch.texture);
            }
        }
        for m in &scene.meshes {
            self.ensure_mesh(gpu, assets, m.mesh);
            if let Some(t) = m.texture {
                self.ensure_texture(gpu, assets, t);
            }
        }
        self.evict_unused(assets);

        self.cutout_instances.upload(gpu, &sprite_instances(&scene.cutout_sprites));
        self.blend_instances.upload(gpu, &sprite_instances(&scene.blended_sprites));
        let mesh_data: Vec<GpuMeshInstance> = scene
            .meshes
            .iter()
            .map(|m| GpuMeshInstance { model: m.model.to_cols_array(), color: m.color, flags: [m.lit as u32, 0, 0, 0] })
            .collect();
        self.mesh_instances.upload(gpu, &mesh_data);
        self.ensure_depth(gpu, size);

        let mut stats = FrameStats {
            sprite_instances: (scene.cutout_sprites.instances.len() + scene.blended_sprites.instances.len()) as u32,
            mesh_instances: scene.meshes.len() as u32,
            draw_calls: 0,
        };

        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let depth_view = &self.depth.as_ref().expect("depth created above").0;
            let c = scene.clear_color;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: c.r as f64, g: c.g as f64, b: c.b as f64, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            let [vx, vy, vw, vh] = scene.viewport_px;
            let (x, y) = (vx.min(size.0 as f32 - 1.0), vy.min(size.1 as f32 - 1.0));
            let (w, h) = (vw.min(size.0 as f32 - x).max(1.0), vh.min(size.1 as f32 - y).max(1.0));
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);

            // Opaque meshes: consecutive instances sharing (mesh, texture) merge into one draw.
            pass.set_pipeline(&self.mesh_pipeline);
            pass.set_vertex_buffer(1, self.mesh_instances.buffer.slice(..));
            let mut i = 0;
            while i < scene.meshes.len() {
                let key = (scene.meshes[i].mesh, scene.meshes[i].texture);
                let end = i + scene.meshes[i..].iter().take_while(|m| (m.mesh, m.texture) == key).count();
                if let Some(mesh) = self.meshes.get(&key.0.id()) {
                    let texture = key.1.and_then(|t| self.textures.get(&t.id())).unwrap_or(&self.white);
                    pass.set_bind_group(1, &texture.bind_group, &[]);
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, i as u32..end as u32);
                    stats.draw_calls += 1;
                }
                i = end;
            }

            for (pipeline, layer, buffer) in [
                (&self.sprite_cutout, &scene.cutout_sprites, &self.cutout_instances),
                (&self.sprite_blend, &scene.blended_sprites, &self.blend_instances),
            ] {
                if layer.batches.is_empty() {
                    continue;
                }
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, buffer.buffer.slice(..));
                for batch in &layer.batches {
                    let texture = self.textures.get(&batch.texture.id()).unwrap_or(&self.missing);
                    pass.set_bind_group(1, &texture.bind_group, &[]);
                    pass.draw(0..6, batch.range.clone());
                    stats.draw_calls += 1;
                }
            }
        }
        gpu.queue.submit([encoder.finish()]);
        self.stats = stats;
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
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

    fn ensure_depth(&mut self, gpu: &GpuContext, size: (u32, u32)) {
        if matches!(&self.depth, Some((_, s)) if *s == size) {
            return;
        }
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.depth = Some((texture.create_view(&wgpu::TextureViewDescriptor::default()), size));
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
}

fn shader(device: &wgpu::Device, label: &str, body: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(format!("{SHADER_COMMON}\n{body}").into()),
    })
}

fn depth_state(write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn sprite_instances(layer: &SpriteLayer) -> Vec<GpuSpriteInstance> {
    layer
        .instances
        .iter()
        .map(|i| GpuSpriteInstance {
            model: i.model.to_cols_array(),
            uv: i.uv,
            tint: i.tint,
            flags: [i.lit as u32, 0, 0, 0],
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
