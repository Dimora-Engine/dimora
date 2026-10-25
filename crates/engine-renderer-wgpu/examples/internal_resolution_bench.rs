//! Extract and encode cost of integer-resolution presentation against the same scene drawn directly.
//!
//! `extract_ns` is `extract_scene`. It does not submit GPU work.
//!
//! `encode_ns` is [`SceneRenderer::render`](engine_renderer_wgpu::SceneRenderer::render) after the
//! previous sample's completion wait, so the queue is idle. It records command buffers and submits
//! them. It does not map a readback.
//!
//! `completion_ns` is the following `device.poll(wait_indefinitely)`. That waits until submitted
//! work finishes. It includes GPU execution and driver synchronization. It is not a timestamp
//! query inside the scene pass or the presentation blit.
//!
//! The scene is 64 cutout sprites and 4 shared cuboids at 1280×720, the depth-view bench scene.
//! A scaled row sets every camera to 320×180. A direct row leaves `internal_resolution` unset.
//! Refuses a debug build.

use std::time::Instant;

use engine_assets::{AssetManager, ColorSpace, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::{extract_scene, Camera, CameraProjection, Mesh, RenderScene, Sprite, ViewportRect};
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const INTERNAL: (u32, u32) = (320, 180);
const WARMUP: usize = 4;
const SAMPLES: usize = 11;

fn main() {
    if cfg!(debug_assertions) {
        eprintln!(
            "internal_resolution_bench: refusing a debug build; use cargo run --release -p engine-renderer-wgpu --example internal_resolution_bench"
        );
        std::process::exit(2);
    }
    let gpu = GpuContext::headless().unwrap_or_else(|error| {
        eprintln!("internal_resolution_bench: {error}");
        std::process::exit(1);
    });
    let info = gpu.adapter.get_info();
    println!(
        "internal_resolution_bench adapter={} backend={:?} viewport={WIDTH}x{HEIGHT} sprites=64 meshes=4 internal={}x{}",
        info.name, info.backend, INTERNAL.0, INTERNAL.1
    );
    println!(
        "one 320x180 color image is {} bytes and one Depth32Float image is the same. Two cameras retain both.",
        INTERNAL.0 * INTERNAL.1 * 4
    );
    println!(
        "run mode cameras extract_median_ns extract_p95_ns encode_median_ns encode_p95_ns completion_median_ns completion_p95_ns world_draws presentation_draws creates reallocs drops color_bytes depth_bytes"
    );

    let mut assets = AssetManager::in_memory();
    let white = assets.insert("white", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();

    for run in 1..=2 {
        for cameras in [1, 2] {
            for internal in [false, true] {
                measure(&gpu, &assets, run, cameras, internal);
            }
        }
    }
    lifetime(&gpu, &assets);
}

fn measure(gpu: &GpuContext, assets: &AssetManager, run: u32, cameras: i32, internal: bool) {
    let mut world = scene(cameras, internal);
    propagate_transforms(&mut world);
    let target_size = Vec2::new(WIDTH as f32, HEIGHT as f32);
    for _ in 0..WARMUP {
        let _ = extract_scene(&world, assets, target_size).unwrap();
    }
    let mut extract = Vec::with_capacity(SAMPLES);
    let mut scene = extract_scene(&world, assets, target_size).unwrap();
    for _ in 0..SAMPLES {
        let started = Instant::now();
        scene = extract_scene(&world, assets, target_size).unwrap();
        extract.push(started.elapsed().as_nanos() as u64);
    }

    let mut renderer = SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm);
    let target = color_target(gpu);
    renderer.render(gpu, &scene, assets, &target.1, (WIDTH, HEIGHT));
    let _ = wait(gpu);
    for _ in 0..WARMUP {
        renderer.render(gpu, &scene, assets, &target.1, (WIDTH, HEIGHT));
        let _ = wait(gpu);
    }
    let mut encode = Vec::with_capacity(SAMPLES);
    let mut completion = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        renderer.render(gpu, &scene, assets, &target.1, (WIDTH, HEIGHT));
        encode.push(started.elapsed().as_nanos() as u64);
        let started = Instant::now();
        if wait(gpu) {
            completion.push(started.elapsed().as_nanos() as u64);
        }
    }
    let stats = renderer.stats();
    let resources = renderer.resource_stats();
    let usage = renderer.internal_targets();
    let mode = if internal { "internal" } else { "direct" };
    let completion_median =
        if completion.len() == SAMPLES { median(&completion).to_string() } else { "unmeasured".to_string() };
    let completion_p95 =
        if completion.len() == SAMPLES { p95(&completion).to_string() } else { "unmeasured".to_string() };
    println!(
        "{run} {mode} {cameras} {} {} {} {} {completion_median} {completion_p95} {} {} {} {} {} {} {}",
        median(&extract),
        p95(&extract),
        median(&encode),
        p95(&encode),
        stats.world_draw_calls,
        stats.presentation_draws,
        resources.internal_target_creates,
        resources.internal_target_reallocs,
        resources.internal_target_drops,
        usage.color_bytes,
        usage.depth_bytes,
    );
    let _ = scene;
}

fn lifetime(gpu: &GpuContext, assets: &AssetManager) {
    println!("step creates reallocs drops cameras color_bytes depth_bytes world_draws presentation_draws");
    let mut renderer = SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm);
    let target = color_target(gpu);
    let mut world = scene(1, true);
    propagate_transforms(&mut world);
    let mut scene = extract(&mut world, assets);
    render_step(gpu, &mut renderer, assets, &target.1, &scene, "320x180");
    render_step(gpu, &mut renderer, assets, &target.1, &scene, "320x180-again");
    let mut query = world.query::<&mut Camera>();
    query.iter().next().unwrap().1.internal_resolution = Some((160, 90));
    drop(query);
    scene = extract(&mut world, assets);
    render_step(gpu, &mut renderer, assets, &target.1, &scene, "160x90");
    let mut query = world.query::<&mut Camera>();
    query.iter().next().unwrap().1.internal_resolution = None;
    drop(query);
    scene = extract(&mut world, assets);
    render_step(gpu, &mut renderer, assets, &target.1, &scene, "direct");
}

fn render_step(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    assets: &AssetManager,
    target: &wgpu::TextureView,
    scene: &RenderScene,
    label: &str,
) {
    renderer.render(gpu, scene, assets, target, (WIDTH, HEIGHT));
    let _ = wait(gpu);
    let stats = renderer.stats();
    let resources = renderer.resource_stats();
    let usage = renderer.internal_targets();
    println!(
        "{label} {} {} {} {} {} {} {} {}",
        resources.internal_target_creates,
        resources.internal_target_reallocs,
        resources.internal_target_drops,
        usage.cameras,
        usage.color_bytes,
        usage.depth_bytes,
        stats.world_draw_calls,
        stats.presentation_draws,
    );
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, Vec2::new(WIDTH as f32, HEIGHT as f32)).unwrap()
}

fn scene(cameras: i32, internal: bool) -> World {
    let mut world = World::new();
    for index in 0..cameras {
        let width = 1.0 / cameras as f32;
        let mut camera = Camera {
            order: index,
            viewport: ViewportRect::new(index as f32 * width, 0.0, width, 1.0),
            projection: CameraProjection::orthographic(18.0).with_clip_range(0.5, 40.0),
            ..Camera::orthographic(18.0)
        };
        if internal {
            camera = camera.with_internal_resolution(INTERNAL.0, INTERNAL.1);
        }
        world.spawn((camera, Transform::from_xyz(0.0, 1.0, -10.0)));
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
