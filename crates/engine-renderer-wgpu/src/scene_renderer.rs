use std::collections::{HashMap, HashSet};

use bytemuck::Pod;
use engine_assets::{
    AddressMode, AssetHandle, AssetId, AssetManager, ColorSpace, MeshData, MipFilter, SamplerDesc, Texture,
    TextureFilter,
};
use engine_renderer::{RenderScene, SpriteLayer};
use wgpu::util::DeviceExt;

/// M5-2B uploads one mip level for every texture, including `Mipmaps::Generate`.
/// Generating that chain is M5-2C.
const ALLOCATED_MIP_LEVELS: u32 = 1;

/// LOD clamp for a texture that has only the base level.
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
}

/// Effective GPU sampling and storage for one cached asset texture.
///
/// This is the configuration passed to `Device::create_sampler` and the image that was allocated,
/// after CPU normalization, the device anisotropy fallback, and the one-level LOD clamp.
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
    anisotropy_capability_logged: bool,
    invalid_anisotropy_logged: bool,
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

        let anisotropic_filtering =
            gpu.adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING);
        let mut resource_stats = GpuResourceStats::default();
        let mut samplers = HashMap::new();
        let default_sampling = effective_sampling(SamplerDesc::default(), anisotropic_filtering);
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
                label: "white",
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
                label: "missing",
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
            anisotropy_capability_logged: false,
            invalid_anisotropy_logged: false,
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
    /// inspected: matching dimensions, format, mip allocation, and pixel fingerprint keep the
    /// image, and a sampler change only rebuilds the bind group. Storage that no longer matches
    /// is replaced. Missing assets get no entry; sprite draws then use the magenta placeholder.
    fn ensure_texture(&mut self, gpu: &GpuContext, assets: &AssetManager, handle: AssetHandle<Texture>) {
        let revision = assets.revision(handle);
        if revision == 0 {
            return;
        }
        if let Some(cached) = self.textures.get_mut(&handle.id()) {
            if cached.revision == revision {
                cached.last_used = self.frame;
                return;
            }
        }
        let Some(tex) = assets.get(handle) else { return };
        let sampling = effective_sampling(tex.sampler, self.anisotropic_filtering);
        self.note_fallbacks(&sampling);
        let format = gpu_format(tex.color_space);
        let fingerprint = pixel_fingerprint(&tex.rgba);
        self.resource_stats.pixel_fingerprints += 1;
        let (width, height) = (tex.width, tex.height);
        let sampler = self.cached_sampler(gpu, &sampling);
        let id = handle.id();
        if let Some(cached) = self.textures.get_mut(&id) {
            let storage_matches = cached.width == width
                && cached.height == height
                && cached.format == format
                && cached.mip_level_count == ALLOCATED_MIP_LEVELS;
            if storage_matches {
                if cached.fingerprint != fingerprint {
                    write_base_level(&gpu.queue, &cached.texture, width, height, &tex.rgba);
                    cached.fingerprint = fingerprint;
                    self.resource_stats.texture_writes += 1;
                }
                if cached.sampling.key != sampling.key {
                    cached.bind_group =
                        texture_bind_group(&gpu.device, &self.texture_layout, &cached.view, &sampler, "asset texture");
                    self.resource_stats.bind_group_creates += 1;
                }
                cached.sampling = sampling;
                cached.revision = revision;
                cached.last_used = self.frame;
                return;
            }
        }
        let mut gpu_tex = upload_image(
            gpu,
            &self.texture_layout,
            &sampler,
            ImageUpload { sampling, width, height, rgba: &tex.rgba, format, label: "asset texture" },
            &mut self.resource_stats,
        );
        gpu_tex.revision = revision;
        gpu_tex.last_used = self.frame;
        gpu_tex.fingerprint = fingerprint;
        self.textures.insert(id, gpu_tex);
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
    label: &'a str,
}

fn upload_image(
    gpu: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    image: ImageUpload<'_>,
    stats: &mut GpuResourceStats,
) -> GpuTexture {
    let size = wgpu::Extent3d { width: image.width, height: image.height, depth_or_array_layers: 1 };
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(image.label),
        size,
        mip_level_count: ALLOCATED_MIP_LEVELS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: image.format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    stats.texture_creates += 1;
    write_base_level(&gpu.queue, &texture, image.width, image.height, image.rgba);
    stats.texture_writes += 1;
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = texture_bind_group(&gpu.device, layout, &view, sampler, image.label);
    stats.bind_group_creates += 1;
    GpuTexture {
        revision: 0,
        last_used: 0,
        fingerprint: pixel_fingerprint(image.rgba),
        width: image.width,
        height: image.height,
        format: image.format,
        mip_level_count: ALLOCATED_MIP_LEVELS,
        sampling: image.sampling,
        view,
        bind_group,
        texture,
    }
}

fn write_base_level(queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32, rgba: &[u8]) {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
        size,
    );
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

/// CPU normalization, then the device anisotropy fallback, then the single-level LOD clamp.
///
/// One allocated mip level means the texture view has no other level to sample. The LOD upper bound
/// stays at [`SINGLE_LEVEL_LOD_MAX`] so a minified footprint still uses `min_filter`. `Mipmaps::Generate`
/// does not allocate that chain until M5-2C. A missing mip filter and nearest mip filtering produce
/// the same key. `create_sampler` is not retried: invalid anisotropy is corrected here, and a later
/// HAL failure is a device error.
fn effective_sampling(sampler: SamplerDesc, anisotropic_filtering: bool) -> EffectiveSampling {
    let (sampler, invalid_anisotropy) = match sampler.normalized() {
        Ok(sampler) => (sampler, false),
        Err(_) => (SamplerDesc { anisotropy: 1, ..sampler }, true),
    };
    let capability_fallback = sampler.anisotropy > 1 && !anisotropic_filtering;
    let anisotropy = if capability_fallback { 1 } else { sampler.anisotropy };
    let mip_linear = sampler.mip_filter == Some(MipFilter::Linear);
    let lod_min: f32 = 0.0;
    let lod_max: f32 = SINGLE_LEVEL_LOD_MAX;
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
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in rgba {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^ (rgba.len() as u64)
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
        let sampling = effective_sampling(linear_aniso(8), false);
        assert!(sampling.capability_fallback);
        assert_eq!(sampling.anisotropy, 1);
        assert_eq!(sampling.min_filter, TextureFilter::Linear);
        assert_eq!(sampling.mag_filter, TextureFilter::Linear);
        assert!(sampling.mip_linear);
        assert_eq!(sampler_descriptor(&sampling).anisotropy_clamp, 1);
    }

    #[test]
    fn supported_anisotropy_reaches_the_descriptor() {
        let sampling = effective_sampling(linear_aniso(8), true);
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
        );
        assert_eq!(nearest.anisotropy, 1);
        assert_eq!(nearest.min_filter, TextureFilter::Nearest);
        assert_eq!(nearest.mag_filter, TextureFilter::Linear);

        let odd = effective_sampling(linear_aniso(3), true);
        assert_eq!(odd.anisotropy, 3);
        assert_eq!(sampler_descriptor(&odd).anisotropy_clamp, 3);
    }

    #[test]
    fn anisotropy_below_one_is_corrected_before_sampler_creation() {
        let sampling = effective_sampling(SamplerDesc { anisotropy: 0, ..SamplerDesc::default() }, true);
        assert!(sampling.invalid_anisotropy);
        assert_eq!(sampling.anisotropy, 1);
        assert_eq!(sampling.min_filter, TextureFilter::Nearest);
        assert_eq!(sampler_descriptor(&sampling).anisotropy_clamp, 1);
    }

    #[test]
    fn one_mip_level_clamps_lod_and_collapses_nearest_mip_filters() {
        let base = effective_sampling(SamplerDesc::default(), true);
        let nearest =
            effective_sampling(SamplerDesc { mip_filter: Some(MipFilter::Nearest), ..SamplerDesc::default() }, true);
        let linear =
            effective_sampling(SamplerDesc { mip_filter: Some(MipFilter::Linear), ..SamplerDesc::default() }, true);
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
        );
        let descriptor = sampler_descriptor(&sampling);
        assert_eq!(descriptor.min_filter, wgpu::FilterMode::Linear);
        assert_eq!(descriptor.mag_filter, wgpu::FilterMode::Nearest);
        assert_eq!(descriptor.address_mode_u, wgpu::AddressMode::Repeat);
        assert_eq!(descriptor.address_mode_v, wgpu::AddressMode::MirrorRepeat);
        assert_eq!(descriptor.address_mode_w, wgpu::AddressMode::ClampToEdge);
    }
}
