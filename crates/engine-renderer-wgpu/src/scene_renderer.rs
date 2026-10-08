use std::collections::HashMap;

use bytemuck::Pod;
use engine_assets::{AssetHandle, AssetId, AssetManager, MeshData, Texture, TextureFilter};
use engine_renderer::{RenderScene, SpriteLayer};
use wgpu::util::DeviceExt;

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

struct GpuTexture {
    revision: u32,
    bind_group: wgpu::BindGroup,
    _texture: wgpu::Texture,
}

struct GpuMesh {
    revision: u32,
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
    samplers: [wgpu::Sampler; 2],
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

        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let samplers = [sampler(wgpu::FilterMode::Nearest), sampler(wgpu::FilterMode::Linear)];

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

        Self {
            format,
            globals,
            globals_bind_group,
            white: placeholder(gpu, &texture_layout, &samplers[0], [255; 4], 1, "white"),
            missing: placeholder(gpu, &texture_layout, &samplers[0], [255, 0, 255, 255], 1, "missing"),
            texture_layout,
            samplers,
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
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Counters from the most recent [`SceneRenderer::render`].
    pub fn stats(&self) -> FrameStats {
        self.stats
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

    /// Uploads (or re-uploads after a revision bump) the GPU copy of a texture asset. Assets that
    /// are not loaded get no entry, and draws fall back to a magenta placeholder.
    fn ensure_texture(&mut self, gpu: &GpuContext, assets: &AssetManager, handle: AssetHandle<Texture>) {
        let revision = assets.revision(handle);
        if revision == 0 || matches!(self.textures.get(&handle.id()), Some(t) if t.revision == revision) {
            return;
        }
        let Some(tex) = assets.get(handle) else { return };
        let sampler = &self.samplers[(tex.filter == TextureFilter::Linear) as usize];
        let mut gpu_tex =
            upload_texture(gpu, &self.texture_layout, sampler, tex.width, tex.height, &tex.rgba, "asset texture");
        gpu_tex.revision = revision;
        self.textures.insert(handle.id(), gpu_tex);
    }

    fn ensure_mesh(&mut self, gpu: &GpuContext, assets: &AssetManager, handle: AssetHandle<MeshData>) {
        let revision = assets.revision(handle);
        if revision == 0 || matches!(self.meshes.get(&handle.id()), Some(m) if m.revision == revision) {
            return;
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
        self.meshes
            .insert(handle.id(), GpuMesh { revision, vertices, indices, index_count: mesh.indices.len() as u32 });
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

fn placeholder(
    gpu: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    rgba: [u8; 4],
    size: u32,
    label: &str,
) -> GpuTexture {
    upload_texture(gpu, layout, sampler, size, size, &rgba.repeat((size * size) as usize), label)
}

fn upload_texture(
    gpu: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    rgba: &[u8],
    label: &str,
) -> GpuTexture {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    });
    GpuTexture { revision: 0, bind_group, _texture: texture }
}
