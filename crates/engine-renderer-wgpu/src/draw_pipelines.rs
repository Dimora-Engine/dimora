//! Scene pipelines at one sample count.
//!
//! The one-sample renderer builds these in [`SceneRenderer::new`]. Four-sample copies are built
//! the first time 4× is selected. Alpha-to-coverage stays off at both counts. A pipeline from
//! one count is never bound to an attachment of the other.

use crate::gpu_types::{GpuDebugVertex, GpuMeshInstance, GpuSpriteInstance};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADER_COMMON: &str = include_str!("shaders/common.wgsl");

/// Pipelines whose sample count must match the color and depth attachments they draw into.
pub(crate) struct DrawPipelines {
    pub sprite_cutout: wgpu::RenderPipeline,
    pub sprite_blend: wgpu::RenderPipeline,
    pub sprite_opaque: wgpu::RenderPipeline,
    pub sprite_opaque_double: wgpu::RenderPipeline,
    pub sprite_mask: wgpu::RenderPipeline,
    pub sprite_mask_double: wgpu::RenderPipeline,
    pub sprite_blend_culled: wgpu::RenderPipeline,
    pub mesh_solid: wgpu::RenderPipeline,
    pub mesh_solid_double: wgpu::RenderPipeline,
    pub mesh_mask: wgpu::RenderPipeline,
    pub mesh_mask_double: wgpu::RenderPipeline,
    pub mesh_blend: wgpu::RenderPipeline,
    pub mesh_blend_double: wgpu::RenderPipeline,
    pub debug: wgpu::RenderPipeline,
    pub clear: wgpu::RenderPipeline,
    pub sample_count: u32,
}

pub(crate) fn make_draw_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    sample_count: u32,
    scene_layout: &wgpu::PipelineLayout,
    debug_layout: &wgpu::PipelineLayout,
    clear_pipeline_layout: &wgpu::PipelineLayout,
) -> DrawPipelines {
    let multisample = if sample_count == 1 {
        wgpu::MultisampleState::default()
    } else {
        wgpu::MultisampleState { count: sample_count, mask: !0, alpha_to_coverage_enabled: false }
    };
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
    let sprite_pipeline = |label,
                           entry,
                           depth_write: bool,
                           blend: Option<wgpu::BlendState>,
                           cull: Option<wgpu::Face>| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(scene_layout),
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
            primitive: wgpu::PrimitiveState { cull_mode: cull, front_face: wgpu::FrontFace::Ccw, ..Default::default() },
            depth_stencil: Some(depth_state(depth_write)),
            multisample,
            multiview_mask: None,
            cache: None,
        })
    };
    let sprite_cutout = sprite_pipeline("sprite cutout", "fs_cutout", true, None, None);
    let sprite_blend = sprite_pipeline("sprite blend", "fs_blend", false, Some(wgpu::BlendState::ALPHA_BLENDING), None);
    let sprite_opaque = sprite_pipeline("sprite opaque", "fs_opaque", true, None, Some(wgpu::Face::Back));
    let sprite_opaque_double = sprite_pipeline("sprite opaque double", "fs_opaque", true, None, None);
    let sprite_mask = sprite_pipeline("sprite mask", "fs_mask", true, None, Some(wgpu::Face::Back));
    let sprite_mask_double = sprite_pipeline("sprite mask double", "fs_mask", true, None, None);
    let sprite_blend_culled = sprite_pipeline(
        "sprite blend culled",
        "fs_blend",
        false,
        Some(wgpu::BlendState::ALPHA_BLENDING),
        Some(wgpu::Face::Back),
    );

    let mesh_vertex_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
    let mesh_instance_attrs = wgpu::vertex_attr_array![
        3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Uint32x4
    ];
    let mesh_vertex_layout = wgpu::VertexBufferLayout {
        array_stride: core::mem::size_of::<engine_assets::Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &mesh_vertex_attrs,
    };
    let mesh_instance_layout = wgpu::VertexBufferLayout {
        array_stride: core::mem::size_of::<GpuMeshInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &mesh_instance_attrs,
    };
    let mesh_pipeline = |label, entry, depth_write: bool, blend: Option<wgpu::BlendState>, cull: Option<wgpu::Face>| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(scene_layout),
            vertex: wgpu::VertexState {
                module: &mesh_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(mesh_vertex_layout.clone()), Some(mesh_instance_layout.clone())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mesh_module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: cull, front_face: wgpu::FrontFace::Ccw, ..Default::default() },
            depth_stencil: Some(depth_state(depth_write)),
            multisample,
            multiview_mask: None,
            cache: None,
        })
    };
    let mesh_solid = mesh_pipeline("mesh", "fs_main", true, None, Some(wgpu::Face::Back));
    let mesh_solid_double = mesh_pipeline("mesh double", "fs_main", true, None, None);
    let mesh_mask = mesh_pipeline("mesh mask", "fs_mask", true, None, Some(wgpu::Face::Back));
    let mesh_mask_double = mesh_pipeline("mesh mask double", "fs_mask", true, None, None);
    let mesh_blend =
        mesh_pipeline("mesh blend", "fs_blend", false, Some(wgpu::BlendState::ALPHA_BLENDING), Some(wgpu::Face::Back));
    let mesh_blend_double =
        mesh_pipeline("mesh blend double", "fs_blend", false, Some(wgpu::BlendState::ALPHA_BLENDING), None);

    // Own shader module: `shader()` prepends common.wgsl, which binds a texture this
    // pipeline does not have. Line width is not part of wgpu 30's primitive state, so the
    // contract is one pixel. Alpha blend lets a segment's alpha show. Depth write stays off.
    let debug_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("debug line"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/debug_line.wgsl").into()),
    });
    let debug_attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];
    let debug_vertex_layout = wgpu::VertexBufferLayout {
        array_stride: core::mem::size_of::<GpuDebugVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &debug_attrs,
    };
    let debug = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("debug line"),
        layout: Some(debug_layout),
        vertex: wgpu::VertexState {
            module: &debug_shader,
            entry_point: Some("vs_debug"),
            compilation_options: Default::default(),
            buffers: &[Some(debug_vertex_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: &debug_shader,
            entry_point: Some("fs_debug"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::LineList, ..Default::default() },
        depth_stencil: Some(depth_state(false)),
        multisample,
        multiview_mask: None,
        cache: None,
    });

    let clear_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("viewport clear"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/viewport_clear.wgsl").into()),
    });
    let clear = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viewport clear"),
        layout: Some(clear_pipeline_layout),
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
        // The pass has a depth attachment. This draw must not change it: the attachment clear
        // already set 0.0, and a color fill is not a depth writer.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample,
        multiview_mask: None,
        cache: None,
    });

    DrawPipelines {
        sprite_cutout,
        sprite_blend,
        sprite_opaque,
        sprite_opaque_double,
        sprite_mask,
        sprite_mask_double,
        sprite_blend_culled,
        mesh_solid,
        mesh_solid_double,
        mesh_mask,
        mesh_mask_double,
        mesh_blend,
        mesh_blend_double,
        debug,
        clear,
        sample_count,
    }
}

fn depth_state(write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(wgpu::CompareFunction::Greater),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn shader(device: &wgpu::Device, label: &str, body: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(format!("{SHADER_COMMON}\n{body}").into()),
    })
}
