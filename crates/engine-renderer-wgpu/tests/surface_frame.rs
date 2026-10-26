//! Private frame image used when 4× output is copied onto a non-sampleable surface.
//!
//! `present_to_rgba` is the headless stand-in for that copy. `render_to_rgba` still resolves
//! straight into a sampleable target and does not allocate the window frame image.

use engine_assets::{AssetManager, ColorSpace, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Transform, Vec2};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
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
