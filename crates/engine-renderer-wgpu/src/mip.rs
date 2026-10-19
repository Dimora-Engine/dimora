//! Private mip-chain allocation and the GPU blit that fills levels after the base.
//!
//! `Mipmaps::None` stays one level. `Mipmaps::Generate` asks for
//! `floor(log2(max(width, height))) + 1` levels. A device that cannot sample a non-power-of-two
//! mip chain, or a format that cannot be filtered and written as a render attachment, keeps the
//! base level and does not run the blit.

use std::collections::HashMap;

use engine_assets::Mipmaps;

use crate::GpuContext;

const BLIT_SHADER: &str = include_str!("shaders/mip.wgsl");

/// What the device will actually store for one texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MipAllocation {
    pub levels: u32,
    /// `Mipmaps::Generate` was requested, the image is not power-of-two, and
    /// `DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` is absent.
    pub npot_fallback: bool,
    /// The format cannot be sampled with a linear filter and written as a render attachment.
    pub format_fallback: bool,
}

/// Levels to allocate before a device fallback. `Mipmaps::None` is 1.
pub(crate) fn allocate_mip_levels(
    mips: Mipmaps,
    width: u32,
    height: u32,
    npot_mipmaps: bool,
    format_can_blit: bool,
) -> MipAllocation {
    let requested = mips.level_count(width, height);
    if requested <= 1 {
        return MipAllocation { levels: 1, npot_fallback: false, format_fallback: false };
    }
    if !format_can_blit {
        return MipAllocation { levels: 1, npot_fallback: false, format_fallback: true };
    }
    let power_of_two = width.is_power_of_two() && height.is_power_of_two();
    if !power_of_two && !npot_mipmaps {
        return MipAllocation { levels: 1, npot_fallback: true, format_fallback: false };
    }
    MipAllocation { levels: requested, npot_fallback: false, format_fallback: false }
}

pub(crate) fn format_supports_mip_blit(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> bool {
    let features = adapter.get_texture_format_features(format);
    let usage =
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT;
    features.allowed_usages.contains(usage) && features.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
}

/// Width and height of mip `level`. Level 0 is the base image.
pub(crate) fn mip_extent(width: u32, height: u32, level: u32) -> (u32, u32) {
    ((width >> level).max(1), (height >> level).max(1))
}

/// One blit pipeline per texture format, plus one linear clamp sampler reused for every asset.
pub(crate) struct MipGenerator {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl MipGenerator {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mip blit layout"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mip blit pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mip blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        // Both min and mag are linear. The source view is a single mip, so the 2x2 average does
        // not depend on whether the hardware treats the footprint as minification.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mip blit sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            ..Default::default()
        });
        Self { layout, pipeline_layout, shader, sampler, pipelines: HashMap::new() }
    }

    /// Fills levels `1..levels` from the previous level. The base level is already uploaded.
    ///
    /// Each pass reads one mip view and writes the next. The two views are different subresources.
    pub(crate) fn generate(
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
        self.ensure_pipeline(&gpu.device, format);
        for level in 1..levels {
            let source = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mip blit source"),
                base_mip_level: level - 1,
                mip_level_count: Some(1),
                dimension: Some(wgpu::TextureViewDimension::D2),
                ..Default::default()
            });
            let dest = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mip blit dest"),
                base_mip_level: level,
                mip_level_count: Some(1),
                dimension: Some(wgpu::TextureViewDimension::D2),
                ..Default::default()
            });
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mip blit bind group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&source) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            let mut encoder =
                gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("mip blit") });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mip blit pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dest,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                let (dest_w, dest_h) = mip_extent(width, height, level);
                pass.set_pipeline(&self.pipelines[&format]);
                pass.set_bind_group(0, &bind, &[]);
                pass.set_viewport(0.0, 0.0, dest_w as f32, dest_h as f32, 0.0, 1.0);
                pass.draw(0..3, 0..1);
            }
            gpu.queue.submit([encoder.finish()]);
        }
    }

    fn ensure_pipeline(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.pipelines.contains_key(&format) {
            return;
        }
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mip blit"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.insert(format, pipeline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_one_level_and_generate_follows_the_larger_side() {
        assert_eq!(allocate_mip_levels(Mipmaps::None, 64, 32, false, true).levels, 1);
        let pot = allocate_mip_levels(Mipmaps::Generate, 8, 8, false, true);
        assert_eq!(pot.levels, 4);
        assert!(!pot.npot_fallback);
        assert_eq!(allocate_mip_levels(Mipmaps::Generate, 1, 1, false, true).levels, 1);
        assert_eq!(allocate_mip_levels(Mipmaps::Generate, 3, 5, true, true).levels, 3);
        assert_eq!(allocate_mip_levels(Mipmaps::Generate, 1, 16, true, true).levels, 5);
        assert_eq!(allocate_mip_levels(Mipmaps::Generate, 16, 1, true, true).levels, 5);
    }

    #[test]
    fn npot_without_the_downlevel_flag_keeps_one_level() {
        let fallback = allocate_mip_levels(Mipmaps::Generate, 6, 4, false, true);
        assert_eq!(fallback.levels, 1);
        assert!(fallback.npot_fallback);
        assert!(!fallback.format_fallback);

        let supported = allocate_mip_levels(Mipmaps::Generate, 6, 4, true, true);
        assert_eq!(supported.levels, 3);
        assert!(!supported.npot_fallback);

        let square_pot = allocate_mip_levels(Mipmaps::Generate, 8, 4, false, true);
        assert_eq!(square_pot.levels, 4);
        assert!(!square_pot.npot_fallback);
    }

    #[test]
    fn a_format_that_cannot_blit_keeps_one_level() {
        let fallback = allocate_mip_levels(Mipmaps::Generate, 8, 8, true, false);
        assert_eq!(fallback.levels, 1);
        assert!(fallback.format_fallback);
        assert!(!fallback.npot_fallback);
    }

    #[test]
    fn mip_extent_halves_and_stops_at_one() {
        assert_eq!(mip_extent(5, 3, 0), (5, 3));
        assert_eq!(mip_extent(5, 3, 1), (2, 1));
        assert_eq!(mip_extent(5, 3, 2), (1, 1));
        assert_eq!(mip_extent(1, 8, 3), (1, 1));
    }

    fn gpu_or_skip() -> Option<GpuContext> {
        match GpuContext::headless() {
            Ok(gpu) => Some(gpu),
            Err(error) => {
                if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() == Some("1") {
                    panic!("mip generator GPU test required a device: {error}");
                }
                eprintln!("skipping mip generator GPU test: {error}");
                None
            }
        }
    }

    fn upload_base(gpu: &GpuContext, texture: &wgpu::Texture, width: u32, height: u32, rgba: &[u8]) {
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
    }

    fn read_mip(gpu: &GpuContext, texture: &wgpu::Texture, level: u32, width: u32, height: u32) -> Vec<u8> {
        let padded = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mip readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("mip read") });
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
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let data = buffer.slice(..).get_mapped_range().unwrap();
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for row in data.chunks(padded as usize).take(height as usize) {
            out.extend_from_slice(&row[..(width * 4) as usize]);
        }
        drop(data);
        buffer.unmap();
        out
    }

    fn checker_texture(
        gpu: &GpuContext,
        format: wgpu::TextureFormat,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> wgpu::Texture {
        let levels = Mipmaps::Generate.level_count(width, height);
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mip contents"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        upload_base(gpu, &texture, width, height, rgba);
        let mut generator = MipGenerator::new(&gpu.device);
        generator.generate(gpu, &texture, format, width, height, levels);
        texture
    }

    #[test]
    fn generated_srgb_and_linear_levels_average_black_and_white_in_linear_light() {
        let Some(gpu) = gpu_or_skip() else { return };
        let info = gpu.adapter.get_info();
        let flags = gpu.adapter.get_downlevel_capabilities().flags;
        eprintln!(
            "mip contents adapter: {} ({:?}, {:?}); NON_POWER_OF_TWO_MIPMAPPED_TEXTURES={}",
            info.name,
            info.backend,
            info.device_type,
            flags.contains(wgpu::DownlevelFlags::NON_POWER_OF_TWO_MIPMAPPED_TEXTURES)
        );
        for format in [wgpu::TextureFormat::Rgba8UnormSrgb, wgpu::TextureFormat::Rgba8Unorm] {
            assert!(format_supports_mip_blit(&gpu.adapter, format), "{format:?} cannot blit");
        }

        // 2x2 checker. The 1x1 mip is the linear average of black and white.
        let mut checker = Vec::new();
        for y in 0..2 {
            for x in 0..2 {
                let on = (x + y) % 2 == 0;
                checker.extend_from_slice(if on { &[0, 0, 0, 255] } else { &[255, 255, 255, 255] });
            }
        }
        let srgb = checker_texture(&gpu, wgpu::TextureFormat::Rgba8UnormSrgb, &checker, 2, 2);
        let linear = checker_texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, &checker, 2, 2);
        let srgb_mip = read_mip(&gpu, &srgb, 1, 1, 1);
        let linear_mip = read_mip(&gpu, &linear, 1, 1, 1);
        eprintln!("sRGB 1x1 mip bytes {srgb_mip:?}; linear 1x1 mip bytes {linear_mip:?}");
        // Encoded sRGB of linear 0.5 is about byte 188. A byte average of 0 and 255 would be 127.
        assert!(
            (180..=195).contains(&srgb_mip[0])
                && (180..=195).contains(&srgb_mip[1])
                && (180..=195).contains(&srgb_mip[2]),
            "sRGB mip stored encoded linear 0.5, got {srgb_mip:?}"
        );
        assert_eq!(srgb_mip[3], 255, "opaque checker alpha stays opaque: {srgb_mip:?}");
        assert!(
            (120..=136).contains(&linear_mip[0])
                && (120..=136).contains(&linear_mip[1])
                && (120..=136).contains(&linear_mip[2]),
            "linear mip stored about byte 128, got {linear_mip:?}"
        );
        assert_ne!(srgb_mip[0], linear_mip[0], "sRGB encoding and linear bytes must differ");

        // Straight alpha: RGB stays white while alpha averages. Premultiplied RGB would fall too.
        let mut alpha = Vec::new();
        for y in 0..2 {
            for x in 0..2 {
                let opaque = (x + y) % 2 == 0;
                alpha.extend_from_slice(if opaque { &[255, 255, 255, 255] } else { &[255, 255, 255, 0] });
            }
        }
        let straight = checker_texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, &alpha, 2, 2);
        let averaged = read_mip(&gpu, &straight, 1, 1, 1);
        assert!(averaged[0] > 240 && averaged[1] > 240 && averaged[2] > 240, "straight RGB stays white: {averaged:?}");
        assert!((110..=145).contains(&averaged[3]), "straight alpha averages: {averaged:?}");

        // Orientation: left red, right blue must survive the 4x2 -> 2x1 step.
        let mut horizontal = vec![0u8; 4 * 2 * 4];
        for y in 0..2 {
            for x in 0..4 {
                let px = if x < 2 { [255, 0, 0, 255] } else { [0, 0, 255, 255] };
                let i = (y * 4 + x) * 4;
                horizontal[i..i + 4].copy_from_slice(&px);
            }
        }
        let oriented = checker_texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, &horizontal, 4, 2);
        let row = read_mip(&gpu, &oriented, 1, 2, 1);
        assert!(row[0] > 200 && row[2] < 40, "left mip texel stays red: {row:?}");
        assert!(row[4] < 40 && row[6] > 200, "right mip texel stays blue: {row:?}");

        let mut vertical = vec![0u8; 2 * 4 * 4];
        for y in 0..4 {
            for x in 0..2 {
                let px = if y < 2 { [255, 0, 0, 255] } else { [0, 0, 255, 255] };
                let i = (y * 2 + x) * 4;
                vertical[i..i + 4].copy_from_slice(&px);
            }
        }
        let oriented = checker_texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, &vertical, 2, 4);
        let column = read_mip(&gpu, &oriented, 1, 1, 2);
        assert!(column[0] > 200 && column[2] < 40, "top mip texel stays red: {column:?}");
        assert!(column[4] < 40 && column[6] > 200, "bottom mip texel stays blue: {column:?}");
    }
}
