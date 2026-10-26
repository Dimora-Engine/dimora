//! Private frame image used when 4× output is copied onto a non-sampleable surface.
//!
//! `present_to_rgba` is the headless stand-in for that copy. `render_to_rgba` still resolves
//! straight into a sampleable target and does not allocate the window frame image.

use engine_assets::{AssetManager, ColorSpace, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Transform, Vec2};
use engine_renderer::*;
use engine_renderer_wgpu::{wgpu, GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

fn gpu() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(gpu) => Some(gpu),
        Err(error) => {
            if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() == Some("1") {
                panic!("DIMORA_REQUIRE_GPU=1 but the GPU test adapter is unavailable: {error}");
            }
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

fn px(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn pose() -> Transform {
    Transform::from_xyz(0.0, 0.0, -4.0)
}

fn solid(assets: &mut AssetManager, name: &str, rgba: [u8; 4]) -> Sprite {
    let texture = assets.insert(name, Texture::solid(1, 1, rgba).with_color_space(ColorSpace::Linear)).unwrap();
    let sprite_name = format!("{name}-sprite");
    assets.insert(&sprite_name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    Sprite::new(&sprite_name).with_anchor(Vec2::splat(0.5)).unlit()
}

fn draw_both(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
    width: u32,
    height: u32,
) -> (Vec<u8>, Vec<u8>) {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(width as f32, height as f32)).unwrap();
    let direct = renderer.render_to_rgba(gpu, &scene, assets, width, height).unwrap();
    let presented = renderer.present_to_rgba(gpu, &scene, assets, width, height).unwrap();
    (direct, presented)
}

fn empty_scene() -> RenderScene {
    RenderScene {
        passes: Vec::new(),
        targets: Vec::new(),
        ambient: [1.0, 1.0, 1.0],
        fog: [0.0; 4],
        coplanar_thickness: 0.0,
        debug_lines: Vec::new(),
        stats: ExtractStats::default(),
    }
}

fn split_world(assets: &mut AssetManager) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((
        Camera::orthographic(2.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_viewport(ViewportRect::new(0.0, 0.0, 1.0, 0.5)),
        pose(),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_clear_color(Color::rgb(0.0, 0.0, 1.0))
                .with_viewport(ViewportRect::new(0.0, 0.5, 1.0, 0.5))
        },
        Transform::from_xyz(1.5, -0.4, -7.0),
    ));
    world.spawn((
        Camera {
            order: 2,
            ..Camera::orthographic(2.0)
                .with_clear(CameraClear::Load)
                .with_viewport(ViewportRect::new(0.25, 0.25, 0.5, 0.5))
        },
        pose(),
    ));
    let veil = solid(assets, "veil", [0, 255, 0, 128]).with_size(Vec2::splat(0.7));
    world.spawn((Transform::IDENTITY, veil));
    world
}

#[test]
fn a_direct_sampleable_target_does_not_allocate_the_window_frame() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let mut world = World::new();
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()));
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &AssetManager::in_memory(), Vec2::new(16.0, 16.0)).unwrap();
    let _ = renderer.render_to_rgba(&gpu, &scene, &AssetManager::in_memory(), 16, 16).unwrap();
    assert!(renderer.surface_frame().is_none());
    assert_eq!(renderer.resource_stats().surface_frame_creates, 0);
    assert_eq!(renderer.stats().frame_present_draws, 0);
}

#[test]
fn the_window_copy_matches_the_sampleable_target_on_every_pixel() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let mut world = split_world(&mut assets);
    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let (direct, presented) = draw_both(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    assert_eq!(presented, direct, "the 1:1 copy changed a resolved pixel");
    assert_eq!(px(&presented, 32, 0, 0), [255, 0, 0, 255], "top-left");
    assert_eq!(px(&presented, 32, 31, 0), [255, 0, 0, 255], "top-right");
    assert_eq!(px(&presented, 32, 0, 31), [0, 0, 255, 255], "bottom-left");
    assert_eq!(px(&presented, 32, 31, 31), [0, 0, 255, 255], "bottom-right");
    assert_eq!(px(&presented, 32, 0, 15), [255, 0, 0, 255], "last row of the top viewport");
    assert_eq!(px(&presented, 32, 0, 16), [0, 0, 255, 255], "first row of the bottom viewport");
    assert_ne!(px(&presented, 32, 16, 16), [255, 0, 0, 255], "the load viewport replaced the earlier clear");
    assert_eq!(renderer.stats().frame_present_draws, 1);
    assert!(renderer.stats().expand_draws >= 1);
    assert!(renderer.stats().resolve_draws >= 2);
    let frame = renderer.surface_frame().expect("private frame");
    assert_eq!((frame.width, frame.height, frame.color_bytes), (32, 32, 32 * 32 * 4));
    let creates = renderer.resource_stats().surface_frame_creates;
    let _ = draw_both(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    assert_eq!(renderer.resource_stats().surface_frame_creates, creates, "a steady frame allocates again");
}

#[test]
fn an_srgb_window_copy_matches_the_sampleable_target() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let mut world = split_world(&mut assets);
    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8UnormSrgb);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let (direct, presented) = draw_both(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    assert_eq!(presented, direct, "sRGB present round trip changed a pixel");
    assert_eq!(px(&presented, 32, 0, 0), px(&presented, 32, 31, 0));
    assert_ne!(px(&presented, 32, 0, 0), px(&presented, 32, 0, 31));
}

#[test]
fn one_sample_presentation_draws_directly_and_matches_four_after_a_round_trip() {
    let Some(gpu) = gpu() else { return };
    let mut world = World::new();
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(0.2, 0.4, 0.8)), pose()));
    let assets = AssetManager::in_memory();
    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::new(16.0, 16.0)).unwrap();
    let one = renderer.present_to_rgba(&gpu, &scene, &assets, 16, 16).unwrap();
    assert!(renderer.surface_frame().is_none());
    assert_eq!(renderer.stats().frame_present_draws, 0);
    assert_eq!(renderer.stats().resolve_draws, 0);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let four = renderer.present_to_rgba(&gpu, &scene, &assets, 16, 16).unwrap();
    assert_eq!(four, one, "a fully covered clear changed between 1× and the 4× copy");
    assert_eq!(renderer.stats().frame_present_draws, 1);
    let pipelines = renderer.resource_stats().msaa_pipeline_creates;
    renderer.set_msaa(&gpu, MsaaSamples::One).unwrap();
    assert!(renderer.surface_frame().is_none());
    assert_eq!(renderer.msaa_images().image_count, 0);
    assert!(renderer.resource_stats().surface_frame_drops >= 1);
    let restored = renderer.present_to_rgba(&gpu, &scene, &assets, 16, 16).unwrap();
    assert_eq!(restored, one);
    assert_eq!(renderer.stats().frame_present_draws, 0);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, pipelines);
    let again = renderer.present_to_rgba(&gpu, &scene, &assets, 16, 16).unwrap();
    assert_eq!(again, one);
    assert!(renderer.surface_frame().is_some());
}

#[test]
fn a_later_frame_and_an_empty_frame_do_not_keep_the_previous_color() {
    let Some(gpu) = gpu() else { return };
    let assets = AssetManager::in_memory();
    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let mut red = World::new();
    red.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()));
    propagate_transforms(&mut red);
    let red_scene = extract_scene(&red, &assets, Vec2::new(8.0, 8.0)).unwrap();
    let first = renderer.present_to_rgba(&gpu, &red_scene, &assets, 8, 8).unwrap();
    assert_eq!(px(&first, 8, 0, 0), [255, 0, 0, 255]);
    let mut blue = World::new();
    blue.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)), pose()));
    propagate_transforms(&mut blue);
    let blue_scene = extract_scene(&blue, &assets, Vec2::new(8.0, 8.0)).unwrap();
    let second = renderer.present_to_rgba(&gpu, &blue_scene, &assets, 8, 8).unwrap();
    assert_eq!(px(&second, 8, 7, 7), [0, 0, 255, 255]);
    let cleared = renderer.present_to_rgba(&gpu, &empty_scene(), &assets, 8, 8).unwrap();
    assert_eq!(px(&cleared, 8, 3, 3), [0, 0, 0, 255], "an empty frame kept the previous private image");
    assert_eq!(renderer.stats().frame_present_draws, 1);
    assert_eq!(renderer.stats().surface_passes, 0);
}

#[test]
fn resizing_the_presented_frame_replaces_only_that_image() {
    let Some(gpu) = gpu() else { return };
    let assets = AssetManager::in_memory();
    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let mut world = World::new();
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()));
    propagate_transforms(&mut world);
    let small = extract_scene(&world, &assets, Vec2::new(16.0, 16.0)).unwrap();
    let _ = renderer.present_to_rgba(&gpu, &small, &assets, 16, 16).unwrap();
    let creates = renderer.resource_stats().surface_frame_creates;
    let msaa_creates = renderer.resource_stats().msaa_image_creates;
    let mut world = World::new();
    world.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(0.0, 1.0, 0.0)), pose()));
    propagate_transforms(&mut world);
    let large = extract_scene(&world, &assets, Vec2::new(20.0, 10.0)).unwrap();
    let image = renderer.present_to_rgba(&gpu, &large, &assets, 20, 10).unwrap();
    assert_eq!(image.len(), 20 * 10 * 4);
    assert_eq!(px(&image, 20, 19, 9), [0, 255, 0, 255]);
    assert_eq!(renderer.resource_stats().surface_frame_creates, creates);
    assert_eq!(renderer.resource_stats().surface_frame_reallocs, 1);
    assert!(renderer.resource_stats().msaa_image_reallocs >= 1);
    assert_eq!(renderer.resource_stats().msaa_image_creates, msaa_creates);
    let frame = renderer.surface_frame().unwrap();
    assert_eq!((frame.width, frame.height), (20, 10));
}

#[test]
fn an_internal_camera_and_an_offscreen_target_survive_the_window_copy() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let marker = solid(&mut assets, "marker", [0, 255, 0, 255]).with_size(Vec2::splat(0.4)).cutout();
    let mut world = World::new();
    world.insert_resource(RenderTargetStore::new());
    let target = world.resource_mut::<RenderTargetStore>().create(16, 16).unwrap();
    world.spawn((
        Camera::orthographic(2.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)).with_output(CameraOutput::Target(target)),
        pose(),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(8, 8)
        },
        pose(),
    ));
    world.spawn((Transform::IDENTITY, marker));
    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::new(32.0, 32.0)).unwrap();
    let direct = renderer.render_to_rgba(&gpu, &scene, &assets, 32, 32).unwrap();
    let direct_target = renderer.read_render_target(&gpu, target).unwrap();
    let presented = renderer.present_to_rgba(&gpu, &scene, &assets, 32, 32).unwrap();
    let presented_target = renderer.read_render_target(&gpu, target).unwrap();
    assert_eq!(presented, direct);
    assert_eq!(presented_target, direct_target);
    assert_eq!(px(&presented_target, 16, 8, 8), [0, 255, 0, 255]);
    assert_eq!(renderer.stats().presentation_draws, 1, "the internal blit is not the window copy");
    assert_eq!(renderer.stats().frame_present_draws, 1);
}

#[test]
fn a_reused_entity_index_does_not_keep_the_old_internal_image() {
    let Some(gpu) = gpu() else { return };
    let assets = AssetManager::in_memory();
    let mut renderer = SceneRenderer::new(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let mut world = World::new();
    let first = world.spawn((
        Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(16, 16),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    let first_id = first.id();
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::new(32.0, 32.0)).unwrap();
    let _ = renderer.render_to_rgba(&gpu, &scene, &assets, 32, 32).unwrap();
    let old = renderer.read_internal(&gpu, first_id).unwrap();
    assert_eq!(old.len(), 16 * 16 * 4);
    assert_eq!(px(&old, 16, 0, 0), [255, 0, 0, 255]);
    world.despawn(first).unwrap();
    let cleanup = world.spawn((Camera::orthographic(2.0).with_clear_color(Color::BLACK), pose()));
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::new(32.0, 32.0)).unwrap();
    let _ = renderer.render_to_rgba(&gpu, &scene, &assets, 32, 32).unwrap();
    assert_eq!(renderer.internal_targets().cameras, 0);
    assert!(renderer.resource_stats().internal_target_drops >= 1);
    assert!(renderer.resource_stats().msaa_image_drops >= 1);
    world.despawn(cleanup).unwrap();
    let replacement = world.spawn((
        Camera::orthographic(2.0)
            .with_clear_color(Color::rgb(0.0, 0.0, 1.0))
            .with_internal_resolution(8, 10)
            .with_presentation(CameraPresentation::Depth),
        Transform::from_xyz(2.0, -1.0, -9.0),
    ));
    assert_eq!(replacement.id(), first_id, "hecs did not reuse the despawned index");
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::new(32.0, 32.0)).unwrap();
    let _ = renderer.render_to_rgba(&gpu, &scene, &assets, 32, 32).unwrap();
    let image = renderer.read_internal(&gpu, replacement.id()).unwrap();
    assert_eq!(image.len(), 8 * 10 * 4);
    let empty = [
        (DEPTH_VIEW_EMPTY[0] * 255.0).round() as u8,
        (DEPTH_VIEW_EMPTY[1] * 255.0).round() as u8,
        (DEPTH_VIEW_EMPTY[2] * 255.0).round() as u8,
        (DEPTH_VIEW_EMPTY[3] * 255.0).round() as u8,
    ];
    assert_eq!(px(&image, 8, 0, 0), empty, "the replacement showed the previous red clear");
    assert_ne!(px(&image, 8, 0, 0), [255, 0, 0, 255]);
    let usage = renderer.msaa_images();
    let internal_bytes = 8 * 10 * 4 * 4;
    assert!(usage.color_bytes >= internal_bytes);
    assert_eq!(renderer.internal_targets().color_bytes, 8 * 10 * 4);
}

/// IEC 61966-2-1 encode, the inverse of `engine_core::srgb_to_linear`, rounded to 8 bits.
fn srgb_unorm(linear: f32) -> u8 {
    let encoded = if linear <= 0.0031308 { linear * 12.92 } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Physical bytes of a `Bgra8UnormSrgb` texel.
///
/// The shader writes logical RGBA. `copy_texture_to_buffer` returns the stored order B, G, R, A.
fn stored_bgra(linear_r: f32, linear_g: f32, linear_b: f32) -> [u8; 4] {
    [srgb_unorm(linear_b), srgb_unorm(linear_g), srgb_unorm(linear_r), 255]
}

fn bgra_supported(gpu: &GpuContext) -> Result<(), String> {
    let format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let color = gpu.adapter.get_texture_format_features(format);
    let depth = gpu.adapter.get_texture_format_features(wgpu::TextureFormat::Depth32Float);
    let frame = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
    let readback = frame | wgpu::TextureUsages::COPY_SRC;
    if !color.allowed_usages.contains(readback) {
        return Err(format!(
            "Bgra8UnormSrgb cannot be a sampleable copied color attachment; allowed usages are {:?}",
            color.allowed_usages
        ));
    }
    if !color.flags.sample_count_supported(4) {
        return Err(format!("Bgra8UnormSrgb does not support 4 samples; flags are {:?}", color.flags));
    }
    if !depth.flags.sample_count_supported(4) {
        return Err("Depth32Float does not support 4 samples".into());
    }
    Ok(())
}

fn bgra_target(gpu: &GpuContext, width: u32, height: u32, usage: wgpu::TextureUsages) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bgra present"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage,
        view_formats: &[],
    })
}

fn read_color(gpu: &GpuContext, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
    let row_bytes = width * 4;
    let padded = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bgra readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("bgra read") });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
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
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("bgra readback poll");
    rx.recv().expect("bgra map callback").expect("bgra map");
    let data = buffer.slice(..).get_mapped_range().expect("bgra mapped range");
    let mut out = Vec::with_capacity((row_bytes as usize) * height as usize);
    for row in data.chunks(padded as usize).take(height as usize) {
        out.extend_from_slice(&row[..row_bytes as usize]);
    }
    drop(data);
    buffer.unmap();
    out
}

#[test]
fn a_bgra_srgb_private_frame_keeps_channel_order_and_encoding() {
    let Some(gpu) = gpu() else { return };
    if let Err(reason) = bgra_supported(&gpu) {
        if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() == Some("1") {
            panic!("Bgra8UnormSrgb private-frame test cannot run: {reason}");
        }
        eprintln!("skipping BGRA test: {reason}");
        return;
    }

    let width = 16u32;
    let height = 16u32;
    let read_usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
    assert!(
        !read_usage.contains(wgpu::TextureUsages::TEXTURE_BINDING),
        "the stand-in surface must not be shader-readable"
    );

    let mut one = SceneRenderer::new(&gpu, wgpu::TextureFormat::Bgra8UnormSrgb);
    let mut four = SceneRenderer::new(&gpu, wgpu::TextureFormat::Bgra8UnormSrgb);
    four.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let assets = AssetManager::in_memory();
    let mut mid = World::new();
    mid.spawn((Camera::orthographic(2.0).with_clear_color(Color::rgb(0.5, 0.25, 0.75)), pose()));
    propagate_transforms(&mut mid);
    let scene = extract_scene(&mid, &assets, Vec2::new(width as f32, height as f32)).unwrap();
    let one_target = bgra_target(&gpu, width, height, read_usage);
    let one_view = one_target.create_view(&wgpu::TextureViewDescriptor::default());
    one.render(&gpu, &scene, &assets, &one_view, (width, height));
    let direct = read_color(&gpu, &one_target, width, height);
    assert!(one.surface_frame().is_none());
    assert_eq!(one.stats().frame_present_draws, 0);
    assert_eq!(one.stats().resolve_draws, 0);

    let four_target = bgra_target(&gpu, width, height, read_usage);
    let four_view = four_target.create_view(&wgpu::TextureViewDescriptor::default());
    four.present_to_surface(&gpu, &scene, &assets, &four_view, (width, height)).unwrap();
    let presented = read_color(&gpu, &four_target, width, height);
    assert_eq!(four.stats().frame_present_draws, 1);
    assert_eq!(four.stats().resolve_draws, 1);
    assert_eq!(four.stats().expand_draws, 0);
    let frame = four.surface_frame().expect("private frame");
    assert_eq!((frame.width, frame.height), (width, height));

    let expected = stored_bgra(0.5, 0.25, 0.75);
    let linear_storage =
        [(0.75_f32 * 255.0).round() as u8, (0.25_f32 * 255.0).round() as u8, (0.5_f32 * 255.0).round() as u8, 255];
    let swapped = [expected[2], expected[1], expected[0], expected[3]];
    for (label, image) in [("one-sample", &direct), ("four-sample copy", &presented)] {
        let first = px(image, width, 0, 0);
        let last = px(image, width, width - 1, height - 1);
        assert_eq!(first, expected, "{label} first pixel {first:?} is not sRGB BGRA {expected:?}");
        assert_eq!(first, last, "{label} did not copy the last pixel");
        assert_ne!(first, linear_storage, "{label} stored linear unorm instead of sRGB");
        assert_ne!(first, swapped, "{label} stored RGBA byte order instead of BGRA");
        assert_eq!(first[3], 255, "{label} changed opaque alpha");
    }
    assert_eq!(
        px(&presented, width, 0, 0),
        px(&direct, width, 0, 0),
        "4× private-frame copy left the 1× BGRA reference"
    );

    let mut assets = AssetManager::in_memory();
    let mut world = split_world(&mut assets);
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::new(width as f32, height as f32)).unwrap();
    let direct_target = bgra_target(&gpu, width, height, read_usage);
    let direct_view = direct_target.create_view(&wgpu::TextureViewDescriptor::default());
    one.render(&gpu, &scene, &assets, &direct_view, (width, height));
    let direct = read_color(&gpu, &direct_target, width, height);
    let copy_target = bgra_target(&gpu, width, height, read_usage);
    let copy_view = copy_target.create_view(&wgpu::TextureViewDescriptor::default());
    four.present_to_surface(&gpu, &scene, &assets, &copy_view, (width, height)).unwrap();
    let presented = read_color(&gpu, &copy_target, width, height);
    let red = stored_bgra(1.0, 0.0, 0.0);
    let blue = stored_bgra(0.0, 0.0, 1.0);
    assert_eq!(px(&presented, width, 0, 0), red, "top-left is not stored red");
    assert_eq!(px(&presented, width, width - 1, 0), red, "top-right is not stored red");
    assert_eq!(px(&presented, width, 0, height - 1), blue, "bottom-left is not stored blue");
    assert_eq!(px(&presented, width, width - 1, height - 1), blue, "bottom-right is not stored blue");
    assert_eq!(px(&direct, width, 0, 0), red);
    assert_eq!(px(&direct, width, 0, height - 1), blue);
    let covered = px(&presented, width, 8, 8);
    assert_ne!(covered, red, "the blended sprite did not cover the center");
    assert_eq!(covered[3], 255, "blended coverage cleared destination alpha");
    assert!(covered[1] > 0, "the green sprite did not land in the physical green byte");
    assert_eq!(covered, px(&direct, width, 8, 8), "blended center left the 1× reference");
    assert_eq!(four.stats().frame_present_draws, 1);
    assert!(four.stats().expand_draws >= 1);
}
