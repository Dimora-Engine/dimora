//! Encode cost of the depth presentation against the same scene shaded.
//!
//! `encode_ns` is [`SceneRenderer::render`](engine_renderer_wgpu::SceneRenderer::render) after the
//! previous sample's completion wait, so the queue is idle. It records command buffers and submits
//! them. It does not map a readback.
//!
//! `completion_ns` is the following `device.poll(wait_indefinitely)`. That waits until submitted
//! work finishes. It includes GPU execution and driver synchronization. It is not a timestamp
//! query inside either pass. If the wait fails, the column is `unmeasured`.
//!
//! The scene is 64 cutout sprites and 4 shared cuboids at 1280×720. It is not the render-scale
//! catalog. Refuses a debug build.

use std::time::Instant;

use engine_assets::{AssetManager, ColorSpace, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::{extract_scene, Camera, CameraPresentation, CameraProjection, Mesh, Sprite, ViewportRect};
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const WARMUP: usize = 2;
const SAMPLES: usize = 11;

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("depth_view_bench: refusing a debug build; use cargo run --release -p engine-renderer-wgpu --example depth_view_bench");
        std::process::exit(2);
    }
    let gpu = GpuContext::headless().unwrap_or_else(|error| {
        eprintln!("depth_view_bench: {error}");
        std::process::exit(1);
    });
    let info = gpu.adapter.get_info();
    println!(
        "depth_view_bench adapter={} backend={:?} viewport={WIDTH}x{HEIGHT} sprites=64 meshes=4",
        info.name, info.backend
    );
    println!(
        "depth texture is one Depth32Float image, {WIDTH}*{HEIGHT}*4 = {} bytes, created for shaded frames too. No second copy.",
        WIDTH * HEIGHT * 4
    );
    println!(
        "label cameras presentation encode_median_ns encode_p95_ns completion_median_ns completion_p95_ns draw_calls world_draw_calls depth_view_draws bind_group_creates"
    );

    let mut assets = AssetManager::in_memory();
    let white = assets.insert("white", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();

    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    let target = color_target(&gpu);
    for cameras in [1, 7] {
        for presentation in [CameraPresentation::Shaded, CameraPresentation::Depth] {
            let mut world = scene(cameras, presentation);
            propagate_transforms(&mut world);
            let extracted = extract_scene(&world, &assets, Vec2::new(WIDTH as f32, HEIGHT as f32)).unwrap();
            renderer.render(&gpu, &extracted, &assets, &target.1, (WIDTH, HEIGHT));
            let _ = wait(&gpu);
            for _ in 0..WARMUP {
                renderer.render(&gpu, &extracted, &assets, &target.1, (WIDTH, HEIGHT));
                let _ = wait(&gpu);
            }
            let mut encode = Vec::with_capacity(SAMPLES);
            let mut completion = Vec::with_capacity(SAMPLES);
            for _ in 0..SAMPLES {
                let started = Instant::now();
                renderer.render(&gpu, &extracted, &assets, &target.1, (WIDTH, HEIGHT));
                encode.push(started.elapsed().as_nanos() as u64);
                let started = Instant::now();
                if wait(&gpu) {
                    completion.push(started.elapsed().as_nanos() as u64);
                }
            }
            let stats = renderer.stats();
            let completion_median =
                if completion.len() == SAMPLES { median(&completion).to_string() } else { "unmeasured".to_string() };
            let completion_p95 =
                if completion.len() == SAMPLES { p95(&completion).to_string() } else { "unmeasured".to_string() };
            let label = if presentation == CameraPresentation::Depth { "depth" } else { "shaded" };
            println!(
                "{label} {cameras} {label} {} {} {completion_median} {completion_p95} {} {} {} {}",
                median(&encode),
                p95(&encode),
                stats.draw_calls,
                stats.world_draw_calls,
                stats.depth_view_draws,
                renderer.resource_stats().bind_group_creates,
            );
        }
    }
}

fn scene(cameras: i32, presentation: CameraPresentation) -> World {
    let mut world = World::new();
    for index in 0..cameras {
        let width = 1.0 / cameras as f32;
        world.spawn((
            Camera {
                order: index,
                viewport: ViewportRect::new(index as f32 * width, 0.0, width, 1.0),
                projection: CameraProjection::orthographic(18.0).with_clip_range(0.5, 40.0),
                presentation,
                ..Camera::orthographic(18.0)
            },
            Transform::from_xyz(0.0, 1.0, -10.0),
        ));
    }
    for z in 0..8 {
        for x in 0..8 {
            world.spawn((
                Transform::from_xyz(x as f32 * 1.6 - 5.6, 1.0, z as f32 * 1.6),
                Sprite::new("quad").with_size(Vec2::splat(1.1)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
            ));
        }
    }
    for index in 0..4 {
        world.spawn((
            Transform::from_xyz(-3.0 + index as f32 * 2.0, 0.5, 6.0),
            Mesh::new("cube").with_material(engine_renderer::Material::color(Color::rgb(0.7, 0.4, 0.2))),
        ));
    }
    world
}

fn color_target(gpu: &GpuContext) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench color"),
        size: wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn wait(gpu: &GpuContext) -> bool {
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).is_ok()
}

fn median(values: &[u64]) -> u64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn p95(values: &[u64]) -> u64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() as f32) * 0.95).ceil() as usize - 1;
    sorted[index.min(sorted.len() - 1)]
}
