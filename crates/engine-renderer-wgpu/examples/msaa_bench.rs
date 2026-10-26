//! Encode and completion cost of optional 4× MSAA.
//!
//! `encode_ns` is [`SceneRenderer::render`](engine_renderer_wgpu::SceneRenderer::render) after the
//! previous sample's completion wait, so the queue is idle. It records command buffers and submits
//! them. It does not map a readback.
//!
//! `completion_ns` is the following `device.poll(wait_indefinitely)`. That waits until submitted
//! work finishes. It includes GPU execution and driver synchronization. It is not a timestamp
//! query inside the scene pass, the resolve, the load expansion, or the depth diagnostic.
//!
//! `extract_ns` is [`extract_scene`](engine_renderer::extract_scene) only. Sample count does not
//! change it.
//!
//! The first table is the stage-1 scene: one direct orthographic camera, 64 cutout sprites, and 4
//! shared cuboids at 1280×720. Its color target is `RENDER_ATTACHMENT` only. Later rows add
//! sprite-heavy, mesh-heavy, split, offscreen, internal-resolution, load, and depth workloads.
//! A load row's color target also has `TEXTURE_BINDING`. Image bytes are `width * height * 4 * 4`
//! per retained 4× image: a size calculation, not a driver measurement. Refuses a debug build.

use std::time::Instant;

use engine_assets::{AssetManager, ColorSpace, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::{
    extract_scene, Camera, CameraClear, CameraOutput, CameraPresentation, CameraProjection, Mesh, MsaaSamples,
    RenderTargetStore, Sprite, ViewportRect,
};
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const WARMUP: usize = 4;
const SAMPLES: usize = 11;

fn main() {
    if cfg!(debug_assertions) {
        eprintln!(
            "msaa_bench: refusing a debug build; use cargo run --release -p engine-renderer-wgpu --example msaa_bench"
        );
        std::process::exit(2);
    }
    let gpu = GpuContext::headless().unwrap_or_else(|error| {
        eprintln!("msaa_bench: {error}");
        std::process::exit(1);
    });
    let info = gpu.adapter.get_info();
    let pixels = u64::from(WIDTH) * u64::from(HEIGHT);
    println!(
        "msaa_bench adapter={} backend={:?} viewport={WIDTH}x{HEIGHT} sprites=64 meshes=4",
        info.name, info.backend
    );
    println!(
        "one-sample surface depth is {pixels} * 4 bytes. 4× color is {} bytes and 4× depth is the same. The 4× path does not also keep the one-sample surface depth.",
        pixels * 16
    );
    println!(
        "completion_ns waits for the device queue. It is not a GPU pass timestamp. encode_ns is CPU recording and submit."
    );
    println!(
        "run samples encode_median_ns encode_p95_ns completion_median_ns completion_p95_ns world_draws resolve_draws pipeline_creates image_creates color_bytes depth_bytes"
    );

    let mut assets = AssetManager::in_memory();
    let white = assets.insert("white", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();

    let mut world = mixed_scene();
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::new(WIDTH as f32, HEIGHT as f32)).unwrap();

    for run in 1..=2 {
        measure(&gpu, &assets, &scene, run, MsaaSamples::One);
        measure(&gpu, &assets, &scene, run, MsaaSamples::Four);
    }

    println!(
        "workload run samples extract_median_ns extract_p95_ns encode_median_ns encode_p95_ns completion_median_ns completion_p95_ns world_draws resolve_draws expand_draws depth_draws present_draws pipeline_creates image_creates timed_image_creates color_bytes depth_bytes image_count"
    );
    let mut workloads = [
        ("sprites", sprite_scene(), false),
        ("meshes", mesh_scene(), false),
        ("mixed", mixed_scene(), false),
        ("split", split_scene(), false),
        ("offscreen", offscreen_scene(), false),
        ("internal", internal_scene(), false),
        ("load", load_scene(), true),
        ("depth", depth_scene(), false),
    ];
    for run in 1..=2 {
        for (name, world, sampleable) in &mut workloads {
            propagate_transforms(world);
            let (extract_median, extract_p95) = time_extract(world, &assets);
            let extracted = extract_scene(world, &assets, Vec2::new(WIDTH as f32, HEIGHT as f32)).unwrap();
            let timing = WorkloadRun {
                name,
                run,
                extract_median,
                extract_p95,
                samples: MsaaSamples::One,
                sampleable: *sampleable,
            };
            measure_workload(&gpu, &assets, &extracted, &timing);
            let timing = WorkloadRun { samples: MsaaSamples::Four, ..timing };
            measure_workload(&gpu, &assets, &extracted, &timing);
        }
    }
    switch_samples(&gpu, &assets, &scene);
}

fn measure(
    gpu: &GpuContext,
    assets: &AssetManager,
    scene: &engine_renderer::RenderScene,
    run: u32,
    samples: MsaaSamples,
) {
    let mut renderer = SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm);
    let target = color_target(gpu, WIDTH, HEIGHT, wgpu::TextureUsages::RENDER_ATTACHMENT);
    renderer.set_msaa(gpu, samples).unwrap();
    renderer.render(gpu, scene, assets, &target.1, (WIDTH, HEIGHT));
    let _ = wait(gpu);
    for _ in 0..WARMUP {
        renderer.render(gpu, scene, assets, &target.1, (WIDTH, HEIGHT));
        let _ = wait(gpu);
    }
    let mut encode = Vec::with_capacity(SAMPLES);
    let mut completion = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        renderer.render(gpu, scene, assets, &target.1, (WIDTH, HEIGHT));
        encode.push(started.elapsed().as_nanos() as u64);
        let started = Instant::now();
        if wait(gpu) {
            completion.push(started.elapsed().as_nanos() as u64);
        }
    }
    let stats = renderer.stats();
    let resources = renderer.resource_stats();
    let usage = renderer.msaa_images();
    let label = match samples {
        MsaaSamples::One => "1x",
        MsaaSamples::Four => "4x",
    };
    println!(
        "{run} {label} {SAMPLES} {} {} {} {} {} {} {} {} {} {}",
        median(&encode),
        p95(&encode),
        median(&completion),
        p95(&completion),
        stats.world_draw_calls,
        stats.resolve_draws,
        resources.msaa_pipeline_creates,
        resources.msaa_image_creates,
        usage.color_bytes,
        usage.depth_bytes,
    );
}

fn mixed_scene() -> World {
    let mut world = World::new();
    world.spawn((
        Camera {
            projection: CameraProjection::orthographic(18.0).with_clip_range(0.5, 40.0),
            ..Camera::orthographic(18.0)
        },
        Transform::from_xyz(0.0, 1.0, -10.0),
    ));
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

struct WorkloadRun<'a> {
    name: &'a str,
    run: u32,
    extract_median: u64,
    extract_p95: u64,
    samples: MsaaSamples,
    sampleable: bool,
}

fn measure_workload(
    gpu: &GpuContext,
    assets: &AssetManager,
    scene: &engine_renderer::RenderScene,
    timing: &WorkloadRun<'_>,
) {
    let mut renderer = SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm);
    let usage = if timing.sampleable {
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
    } else {
        wgpu::TextureUsages::RENDER_ATTACHMENT
    };
    let target = color_target(gpu, WIDTH, HEIGHT, usage);
    renderer.set_msaa(gpu, timing.samples).unwrap();
    renderer.render(gpu, scene, assets, &target.1, (WIDTH, HEIGHT));
    let _ = wait(gpu);
    for _ in 0..WARMUP {
        renderer.render(gpu, scene, assets, &target.1, (WIDTH, HEIGHT));
        let _ = wait(gpu);
    }
    let images_before = renderer.resource_stats().msaa_image_creates;
    let mut encode = Vec::with_capacity(SAMPLES);
    let mut completion = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        renderer.render(gpu, scene, assets, &target.1, (WIDTH, HEIGHT));
        encode.push(started.elapsed().as_nanos() as u64);
        let started = Instant::now();
        if wait(gpu) {
            completion.push(started.elapsed().as_nanos() as u64);
        }
    }
    let stats = renderer.stats();
    let resources = renderer.resource_stats();
    let images = renderer.msaa_images();
    let label = match timing.samples {
        MsaaSamples::One => "1x",
        MsaaSamples::Four => "4x",
    };
    println!(
        "{} {} {label} {SAMPLES} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
        timing.name,
        timing.run,
        timing.extract_median,
        timing.extract_p95,
        median(&encode),
        p95(&encode),
        median(&completion),
        p95(&completion),
        stats.world_draw_calls,
        stats.resolve_draws,
        stats.expand_draws,
        stats.depth_view_draws,
        stats.presentation_draws,
        resources.msaa_pipeline_creates,
        resources.msaa_image_creates,
        resources.msaa_image_creates - images_before,
        images.color_bytes,
        images.depth_bytes,
        images.image_count,
    );
}

fn time_extract(world: &World, assets: &AssetManager) -> (u64, u64) {
    let size = Vec2::new(WIDTH as f32, HEIGHT as f32);
    for _ in 0..WARMUP {
        let _ = extract_scene(world, assets, size).unwrap();
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let _ = extract_scene(world, assets, size).unwrap();
        samples.push(started.elapsed().as_nanos() as u64);
    }
    (median(&samples), p95(&samples))
}

fn camera() -> Camera {
    Camera { projection: CameraProjection::orthographic(18.0).with_clip_range(0.5, 40.0), ..Camera::orthographic(18.0) }
}

fn pose() -> Transform {
    Transform::from_xyz(0.0, 1.0, -10.0)
}

fn sprite_scene() -> World {
    let mut world = World::new();
    world.spawn((camera(), pose()));
    for z in 0..16 {
        for x in 0..16 {
            world.spawn((
                Transform::from_xyz((x as f32 - 7.5) * 0.9, 1.0, z as f32 * 0.7),
                Sprite::new("quad").with_size(Vec2::splat(0.7)).with_anchor(Vec2::splat(0.5)).cutout().unlit(),
            ));
        }
    }
    world
}

fn mesh_scene() -> World {
    let mut world = World::new();
    world.spawn((camera(), pose()));
    for z in 0..8 {
        for x in 0..8 {
            world.spawn((
                Transform::from_xyz((x as f32 - 3.5) * 1.5, 0.5, z as f32 * 1.2),
                Mesh::new("cube").with_material(engine_renderer::Material::color(Color::rgb(0.7, 0.4, 0.2))),
            ));
        }
    }
    world
}

fn edit_camera(world: &mut World, edit: impl FnOnce(&mut Camera)) {
    let entity = world.query::<&Camera>().iter().next().unwrap().0;
    let mut camera = world.component_mut::<Camera>(entity).unwrap();
    edit(&mut camera);
}

fn split_scene() -> World {
    let mut world = mixed_scene();
    edit_camera(&mut world, |camera| camera.viewport = ViewportRect::new(0.0, 0.0, 0.5, 1.0));
    world.spawn((Camera { order: 1, viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0), ..camera() }, pose()));
    world
}

fn offscreen_scene() -> World {
    let mut world = mixed_scene();
    world.insert_resource(RenderTargetStore::new());
    let id = world.resource_mut::<RenderTargetStore>().create(WIDTH, HEIGHT).unwrap();
    edit_camera(&mut world, |camera| camera.output = CameraOutput::Target(id));
    world
}

fn internal_scene() -> World {
    let mut world = mixed_scene();
    edit_camera(&mut world, |camera| camera.internal_resolution = Some((320, 180)));
    world
}

fn load_scene() -> World {
    let mut world = mixed_scene();
    world.spawn((Camera { order: 1, clear: CameraClear::Load, ..camera() }, pose()));
    world
}

fn depth_scene() -> World {
    let mut world = mixed_scene();
    edit_camera(&mut world, |camera| camera.presentation = CameraPresentation::Depth);
    world
}

fn switch_samples(gpu: &GpuContext, assets: &AssetManager, scene: &engine_renderer::RenderScene) {
    let full = color_target(gpu, WIDTH, HEIGHT, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let half = color_target(gpu, WIDTH / 2, HEIGHT / 2, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let mut renderer = SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm);
    renderer.render(gpu, scene, assets, &full.1, (WIDTH, HEIGHT));
    let _ = wait(gpu);
    renderer.set_msaa(gpu, MsaaSamples::Four).unwrap();
    renderer.render(gpu, scene, assets, &full.1, (WIDTH, HEIGHT));
    let _ = wait(gpu);
    let created = renderer.resource_stats().msaa_image_creates;
    let pipelines = renderer.resource_stats().msaa_pipeline_creates;
    renderer.render(gpu, scene, assets, &full.1, (WIDTH, HEIGHT));
    let _ = wait(gpu);
    let steady = renderer.resource_stats().msaa_image_creates - created;
    renderer.render(gpu, scene, assets, &half.1, (WIDTH / 2, HEIGHT / 2));
    let _ = wait(gpu);
    let reallocs = renderer.resource_stats().msaa_image_reallocs;
    let drops_before = renderer.resource_stats().msaa_image_drops;
    renderer.set_msaa(gpu, MsaaSamples::One).unwrap();
    let released = renderer.msaa_images().image_count;
    let drops = renderer.resource_stats().msaa_image_drops - drops_before;
    renderer.set_msaa(gpu, MsaaSamples::Four).unwrap();
    let pipelines_kept = renderer.resource_stats().msaa_pipeline_creates;
    renderer.render(gpu, scene, assets, &full.1, (WIDTH, HEIGHT));
    let _ = wait(gpu);
    let restored = renderer.msaa_images();
    println!(
        "switch pipelines={pipelines} pipelines_after_return={pipelines_kept} first_image_creates={created} steady_extra_creates={steady} reallocs_on_resize={reallocs} drops_on_return={drops} images_after_return={released} restored_images={} restored_color_bytes={} restored_depth_bytes={}",
        restored.image_count, restored.color_bytes, restored.depth_bytes
    );
}

fn color_target(
    gpu: &GpuContext,
    width: u32,
    height: u32,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench color"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage,
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
