//! Stage-1 4× MSAA on Apple Metal, plus capability checks that do not need one GPU.
//!
//! One sample stays the default path. Four samples are a direct shaded surface camera that
//! clears color. Load, depth presentation, offscreen targets, and internal resolution are skipped.

use engine_assets::{
    AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture, TextureDimensionLimits, Vertex,
};
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{msaa4_supported, GpuContext, MsaaImageUsage, SceneRenderer};
use engine_world::{propagate_transforms, World};

mod capture {
    #![allow(dead_code)]
    include!("../../log_capture.rs");
}

fn gpu() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(gpu) => {
            let info = gpu.adapter.get_info();
            eprintln!("GPU test adapter: {} ({:?}, {:?})", info.name, info.backend, info.device_type);
            Some(gpu)
        }
        Err(error) => {
            if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() == Some("1") {
                panic!("DIMORA_REQUIRE_GPU=1 but the GPU test adapter is unavailable: {error}");
            }
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

fn renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8Unorm)
}

fn srgb_renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8UnormSrgb)
}

fn px(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect_px(img: &[u8], width: u32, x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, width, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn partial_pixels(img: &[u8]) -> usize {
    img.chunks(4)
        .filter(|pixel| {
            let red = pixel[0];
            red > 8 && red < 247 && pixel[1] == red && pixel[2] == red && pixel[3] == 255
        })
        .count()
}

fn draw(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
    width: u32,
    height: u32,
) -> Vec<u8> {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(width as f32, height as f32)).unwrap();
    renderer.render_to_rgba(gpu, &scene, assets, width, height).unwrap()
}

fn world_with(camera: Camera) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(DebugDraw::default());
    world.spawn((camera, Transform::from_xyz(0.0, 0.0, -4.0)));
    world
}

fn white_sprite(assets: &mut AssetManager, name: &str) -> Sprite {
    let texture =
        assets.insert(name, Texture::solid(1, 1, [255, 255, 255, 255]).with_color_space(ColorSpace::Linear)).unwrap();
    let sprite_name = format!("{name}-sprite");
    let material_name = format!("{name}-mat");
    assets.insert(&sprite_name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    let material = assets.insert(&material_name, MaterialAsset::unlit(Color::WHITE)).unwrap();
    Sprite::new(&sprite_name).with_size(Vec2::splat(4.0)).with_anchor(Vec2::splat(0.5)).with_material(material)
}

fn diagonal_mesh() -> MeshData {
    let normal = Vec3::new(0.0, 0.0, -1.0);
    MeshData::new(
        vec![
            Vertex::new(Vec3::new(-1.0, -1.0, 0.0), normal, [0.0, 1.0]),
            Vertex::new(Vec3::new(0.2, -1.0, 0.0), normal, [1.0, 1.0]),
            Vertex::new(Vec3::new(-1.0, 1.0, 0.0), normal, [0.0, 0.0]),
        ],
        vec![0, 1, 2],
    )
}

fn card(z_offset: f32) -> (Transform, MeshData) {
    let normal = Vec3::new(0.0, 0.0, -1.0);
    let mesh = MeshData::new(
        vec![
            Vertex::new(Vec3::new(-0.6, -0.6, z_offset), normal, [0.0, 1.0]),
            Vertex::new(Vec3::new(0.6, -0.6, z_offset), normal, [1.0, 1.0]),
            Vertex::new(Vec3::new(0.6, 0.6, z_offset), normal, [1.0, 0.0]),
            Vertex::new(Vec3::new(-0.6, 0.6, z_offset), normal, [0.0, 0.0]),
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    (Transform::IDENTITY, mesh)
}

#[test]
fn guaranteed_features_accept_4x_without_requiring_a_hardware_resolve_on_depth() {
    let features = wgpu::Features::empty();
    for format in [wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Rgba8UnormSrgb] {
        let color = format.guaranteed_format_features(features).flags;
        let depth = wgpu::TextureFormat::Depth32Float.guaranteed_format_features(features).flags;
        assert!(msaa4_supported(color, depth), "{format:?}");
        assert!(color.sample_count_supported(4));
        assert!(depth.sample_count_supported(4));
        assert!(!depth.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE));
    }
    let x4 = wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4;
    let empty = wgpu::TextureFormatFeatureFlags::empty();
    assert!(msaa4_supported(x4, x4));
    assert!(!msaa4_supported(empty, x4));
    assert!(!msaa4_supported(x4, empty));
    assert!(!empty.sample_count_supported(4));
}

#[test]
fn one_sample_is_the_default_and_does_not_build_four_sample_pipelines() {
    let Some(gpu) = gpu() else { return };
    let renderer = renderer(&gpu);
    assert_eq!(renderer.msaa(), MsaaSamples::One);
    assert_eq!(renderer.retained_msaa_pipeline_samples(), None);
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 0);
    assert_eq!(renderer.msaa_images(), MsaaImageUsage::default());
    assert!(SceneRenderer::adapter_supports_msaa4(&gpu, wgpu::TextureFormat::Rgba8Unorm));
    assert!(SceneRenderer::adapter_supports_msaa4(&gpu, wgpu::TextureFormat::Rgba8UnormSrgb));
}

#[test]
fn four_samples_are_created_once_and_images_follow_the_selected_count() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    assert_eq!(renderer.msaa(), MsaaSamples::Four);
    assert_eq!(renderer.retained_msaa_pipeline_samples(), Some(4));
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 1);
    assert_eq!(renderer.msaa_images().color_bytes, 0);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 1);

    draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    let first = renderer.msaa_images();
    assert_eq!(first, MsaaImageUsage { width: 16, height: 8, color_bytes: 16 * 8 * 16, depth_bytes: 16 * 8 * 16 });
    assert_eq!(renderer.resource_stats().msaa_image_creates, 1);
    assert_eq!(renderer.stats().resolve_draws, 1);

    draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    assert_eq!(renderer.resource_stats().msaa_image_creates, 1);
    assert_eq!(renderer.resource_stats().msaa_image_reallocs, 0);

    draw(&gpu, &mut renderer, &mut world, &assets, 32, 8);
    assert_eq!(renderer.resource_stats().msaa_image_creates, 1);
    assert_eq!(renderer.resource_stats().msaa_image_reallocs, 1);
    assert_eq!(renderer.msaa_images().width, 32);

    renderer.set_msaa(&gpu, MsaaSamples::One).unwrap();
    assert_eq!(renderer.msaa(), MsaaSamples::One);
    assert_eq!(renderer.retained_msaa_pipeline_samples(), Some(4));
    assert_eq!(renderer.msaa_images(), MsaaImageUsage::default());
    assert_eq!(renderer.resource_stats().msaa_image_drops, 1);
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 1);

    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 1);
    draw(&gpu, &mut renderer, &mut world, &assets, 32, 8);
    assert_eq!(renderer.resource_stats().msaa_image_creates, 2);
    assert_eq!(renderer.msaa(), MsaaSamples::Four);
}

#[test]
fn an_unsupported_size_does_not_fall_back_to_one_sample() {
    let Some(gpu) = gpu() else { return };
    capture::begin();
    let mut renderer = renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    renderer.set_texture_dimension_limits(Some(TextureDimensionLimits { max_width: 1, max_height: 1 }));
    let mut assets = AssetManager::in_memory();
    let sprite = white_sprite(&mut assets, "white");
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)));
    world.spawn((Transform::IDENTITY, sprite));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    assert_eq!(renderer.msaa(), MsaaSamples::Four);
    assert_eq!(renderer.resource_stats().msaa_image_creates, 0);
    assert_eq!(renderer.stats().world_draw_calls, 0);
    assert_eq!(renderer.stats().resolve_draws, 0);
    assert_eq!(renderer.stats().msaa_skipped, 1);
    expect_px(&image, 16, 8, 8, [0, 0, 0, 255], 0, "defined black, not the sprite and not 1× red");
    let messages = capture::messages();
    assert!(messages.iter().any(|message| message.contains("not allocated")));
    assert!(messages.iter().any(|message| message.contains("not replaced with 1×")));
}

#[test]
fn four_samples_cover_a_diagonal_edge_that_one_sample_leaves_binary() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    assets.insert("tri", diagonal_mesh()).unwrap();
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    world.spawn((Transform::IDENTITY, Mesh::new("tri").with_material(Material::unlit(Color::WHITE))));
    let mut one = renderer(&gpu);
    let mut four = renderer(&gpu);
    four.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let low = draw(&gpu, &mut one, &mut world, &assets, 64, 64);
    let high = draw(&gpu, &mut four, &mut world, &assets, 64, 64);
    let low_partial = partial_pixels(&low);
    let high_partial = partial_pixels(&high);
    let differ = low.chunks(4).zip(high.chunks(4)).filter(|(left, right)| left != right).count();
    let odd: Vec<[u8; 4]> = high
        .chunks(4)
        .filter(|pixel| pixel[0] != 0 && pixel[0] != 255)
        .take(6)
        .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
        .collect();
    assert!(low.chunks(4).any(|pixel| pixel[0] == 255), "the triangle is visible at 1×");
    assert_eq!(low_partial, 0, "1× edge pixels stay on or off");
    assert!(
        high_partial > 8,
        "4× edge has intermediate coverage, counted {high_partial}, differ {differ}, odd {odd:?}, resolves {}, white1 {}",
        four.stats().resolve_draws,
        low.chunks(4).filter(|pixel| pixel[0] == 255).count()
    );
    assert_eq!(four.stats().resolve_draws, 1);
    assert_eq!(one.stats().resolve_draws, 0);
    assert_eq!(one.msaa(), MsaaSamples::One);
}

#[test]
fn occlusion_and_reverse_z_still_prefer_the_nearer_surface() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let (near_xf, near_mesh) = card(0.0);
    let (far_xf, far_mesh) = card(1.0);
    assets.insert("near", near_mesh).unwrap();
    assets.insert("far", far_mesh).unwrap();
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    world.spawn((far_xf, Mesh::new("far").with_material(Material::unlit(Color::rgb(0.0, 1.0, 0.0)))));
    world.spawn((near_xf, Mesh::new("near").with_material(Material::unlit(Color::rgb(1.0, 0.0, 0.0)))));
    let mut renderer = renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    expect_px(&image, 32, 16, 16, [255, 0, 0, 255], 1, "near red occludes far green");
}

#[test]
fn a_fully_covered_blend_matches_one_sample_and_cutout_discard_stays_binary() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let texture =
        assets.insert("tex", Texture::solid(1, 1, [255, 255, 0, 128]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    let blend = assets.insert("blend", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)));
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("sprite").with_size(Vec2::splat(4.0)).with_anchor(Vec2::splat(0.5)).with_material(blend),
    ));
    let mut one = renderer(&gpu);
    let mut four = renderer(&gpu);
    four.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let low = draw(&gpu, &mut one, &mut world, &assets, 16, 16);
    let high = draw(&gpu, &mut four, &mut world, &assets, 16, 16);
    expect_px(&high, 16, 8, 8, px(&low, 16, 8, 8), 1, "blended center matches 1×");

    let clear_tex =
        assets.insert("clear-tex", Texture::solid(1, 1, [0, 255, 0, 0]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("clear-sprite", SpriteAsset::whole_texture(clear_tex, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.5).unwrap()))
        .unwrap();
    let mut masked = world_with(Camera::orthographic(2.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)));
    masked.spawn((
        Transform::IDENTITY,
        Sprite::new("clear-sprite").with_size(Vec2::splat(4.0)).with_anchor(Vec2::splat(0.5)).with_material(mask),
    ));
    let image = draw(&gpu, &mut four, &mut masked, &assets, 16, 16);
    expect_px(&image, 16, 8, 8, [0, 0, 255, 255], 1, "mask discard keeps the clear color");
    assert_eq!(partial_pixels(&image), 0, "discard does not sprinkle coverage gray");
}

#[test]
fn debug_lines_draw_at_four_samples() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let assets = AssetManager::in_memory();
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    world.resource_mut::<DebugDraw>().line(Vec3::new(-0.8, 0.0, 0.0), Vec3::new(0.8, 0.0, 0.0), Color::WHITE);
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 32);
    assert!(image.chunks(4).any(|pixel| pixel[0] > 0), "the line is visible, including partial coverage");
    assert_eq!(renderer.stats().resolve_draws, 1);
    assert!(renderer.stats().world_draw_calls >= 1);
}

#[test]
fn srgb_output_is_one_encode() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = srgb_renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let mut assets = AssetManager::in_memory();
    let sprite = white_sprite(&mut assets, "srgb");
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    let material = assets.insert("color", MaterialAsset::unlit(Color::rgb(0.5, 0.2, 0.8))).unwrap();
    world.spawn((Transform::IDENTITY, sprite.with_material(material)));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    let once = [srgb_channel(0.5), srgb_channel(0.2), srgb_channel(0.8), 255];
    let twice = [
        srgb_channel(once[0] as f32 / 255.0),
        srgb_channel(once[1] as f32 / 255.0),
        srgb_channel(once[2] as f32 / 255.0),
        255,
    ];
    expect_px(&image, 16, 8, 8, once, 1, "one sRGB encode");
    let pixel = px(&image, 16, 8, 8);
    let gap = pixel.iter().zip(twice).map(|(got, want)| (*got as i16 - want as i16).abs()).max().unwrap();
    assert!(gap > 8, "a second encode is {twice:?}, pixel {pixel:?}");
}

#[test]
fn the_resolve_writes_only_the_camera_viewport() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let mut assets = AssetManager::in_memory();
    let sprite = white_sprite(&mut assets, "half");
    let mut world = world_with(
        Camera::orthographic(2.0)
            .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
            .with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
    );
    world.spawn((Transform::IDENTITY, sprite));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 16);
    expect_px(&image, 32, 4, 8, [255, 255, 255, 255], 1, "left viewport is the sprite");
    expect_px(&image, 32, 24, 8, [0, 255, 0, 255], 0, "right of the viewport stays the clear");
    expect_px(&image, 32, 16, 0, [0, 255, 0, 255], 0, "first pixel past the viewport stays the clear");
}

#[test]
fn two_surface_viewports_keep_their_own_pixels() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let assets = AssetManager::in_memory();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.spawn((
        Camera::orthographic(2.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_clear_color(Color::rgb(0.0, 0.0, 1.0))
                .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
        },
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 32, 16);
    expect_px(&image, 32, 4, 8, [255, 0, 0, 255], 0, "left camera");
    expect_px(&image, 32, 24, 8, [0, 0, 255, 255], 0, "right camera replaced only its viewport");
    assert_eq!(renderer.stats().resolve_draws, 2);
    assert_eq!(renderer.stats().msaa_skipped, 0);
}

#[test]
fn a_later_frame_does_not_keep_the_previous_clear() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let assets = AssetManager::in_memory();
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)));
    let red = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    expect_px(&red, 8, 1, 1, [255, 0, 0, 255], 0, "first clear");
    {
        let mut cameras = world.query::<&mut Camera>();
        let (_, camera) = cameras.iter().next().unwrap();
        *camera = Camera::orthographic(2.0).with_clear_color(Color::rgb(0.0, 1.0, 0.0));
    }
    let green = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    assert!(green.chunks(4).all(|pixel| pixel == [0, 255, 0, 255]), "no red remains");
}

#[test]
fn switching_sample_counts_does_not_reuse_the_other_count_for_a_covered_sprite() {
    let Some(gpu) = gpu() else { return };
    let mut assets = AssetManager::in_memory();
    let sprite = white_sprite(&mut assets, "cover");
    let mut world = world_with(Camera::orthographic(2.0).with_clear_color(Color::BLACK));
    world.spawn((
        Transform::IDENTITY,
        sprite.with_material(assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap()),
    ));
    let mut dedicated = renderer(&gpu);
    let low = draw(&gpu, &mut dedicated, &mut world, &assets, 16, 16);
    let mut renderer = renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let _high = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    renderer.set_msaa(&gpu, MsaaSamples::One).unwrap();
    let again = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    expect_px(&again, 16, 8, 8, px(&low, 16, 8, 8), 0, "back at 1× the center matches a 1× renderer");
    assert_eq!(renderer.stats().resolve_draws, 0);
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 1);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let high = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    expect_px(&high, 16, 8, 8, [255, 0, 0, 255], 1, "4× center is the sprite");
    assert_eq!(renderer.stats().resolve_draws, 1);
    assert_eq!(renderer.resource_stats().msaa_pipeline_creates, 1);
}

#[test]
fn unsupported_cameras_are_skipped_and_a_later_direct_camera_still_draws() {
    let Some(gpu) = gpu() else { return };
    capture::begin();
    let mut renderer = renderer(&gpu);
    renderer.set_msaa(&gpu, MsaaSamples::Four).unwrap();
    let mut assets = AssetManager::in_memory();
    let sprite = white_sprite(&mut assets, "kept");
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world.insert_resource(RenderTargetStore::new());
    let target = world.resource_mut::<RenderTargetStore>().create(8, 8).unwrap();
    world.spawn((
        Camera::orthographic(2.0).with_clear(CameraClear::Load).with_clear_color(Color::rgb(0.0, 1.0, 0.0)),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((
        Camera {
            order: 1,
            ..Camera::orthographic(2.0)
                .with_presentation(CameraPresentation::Depth)
                .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
        },
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((
        Camera {
            order: 2,
            ..Camera::orthographic(2.0)
                .with_output(CameraOutput::Target(target))
                .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
        },
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((
        Camera {
            order: 3,
            ..Camera::orthographic(2.0).with_internal_resolution(4, 4).with_clear_color(Color::rgb(0.0, 1.0, 0.0))
        },
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((
        Camera { order: 4, ..Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)) },
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((
        Transform::IDENTITY,
        sprite.with_material(assets.insert("white-mat", MaterialAsset::unlit(Color::WHITE)).unwrap()),
    ));
    let image = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    assert_eq!(renderer.msaa(), MsaaSamples::Four);
    assert_eq!(renderer.stats().msaa_skipped, 4);
    assert_eq!(renderer.stats().resolve_draws, 1);
    expect_px(&image, 16, 8, 8, [255, 255, 255, 255], 1, "the direct camera still draws");
    let skips =
        || capture::messages().into_iter().filter(|message| message.contains("was not drawn")).collect::<Vec<_>>();
    let first = skips();
    assert!(first.iter().any(|message| message.contains("CameraClear::Load")), "{first:?}");
    assert!(first.iter().any(|message| message.contains("CameraPresentation::Depth")));
    assert!(first.iter().any(|message| message.contains("offscreen")));
    assert!(first.iter().any(|message| message.contains("internal resolution")));
    draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    assert_eq!(skips().len(), first.len(), "the same skips are not logged twice");
}

fn srgb_channel(linear: f32) -> u8 {
    let encoded = if linear <= 0.0031308 { 12.92 * linear } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}
