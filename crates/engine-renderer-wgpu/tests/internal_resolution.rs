//! Integer nearest presentation on Apple Metal, `Rgba8Unorm`.
//!
//! The internal image is private. These tests read it only through [`SceneRenderer::read_internal`].
//! Sprites sample the public destination, after the blit.

use engine_assets::{
    AlphaMode, AssetHandle, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture, Vertex,
};
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

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

fn px(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect_px(img: &[u8], width: u32, x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, width, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn draw(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
    width: u32,
    height: u32,
) -> (Vec<u8>, RenderScene) {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(width as f32, height as f32)).unwrap();
    let pixels = renderer.render_to_rgba(gpu, &scene, assets, width, height).unwrap();
    (pixels, scene)
}

fn world_with(cameras: impl IntoIterator<Item = (Camera, Transform)>) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    for (camera, transform) in cameras {
        world.spawn((camera, transform));
    }
    world
}

fn pose() -> Transform {
    Transform::from_xyz(0.0, 0.0, -4.0)
}

fn insert_sprite(assets: &mut AssetManager, name: &str, rgba: [u8; 4]) {
    let texture =
        assets.insert(&format!("{name}-tex"), Texture::solid(1, 1, rgba).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
}

fn sprite(name: &str, material: AssetHandle<MaterialAsset>, size: Vec2) -> Sprite {
    Sprite::new(name).with_size(size).with_anchor(Vec2::splat(0.5)).with_material(material).fixed()
}

fn card() -> MeshData {
    let n = Vec3::new(0.0, 0.0, -1.0);
    MeshData::new(
        vec![
            Vertex::new(Vec3::new(-1.0, -1.0, 0.0), n, [0.0, 1.0]),
            Vertex::new(Vec3::new(1.0, -1.0, 0.0), n, [1.0, 1.0]),
            Vertex::new(Vec3::new(1.0, 1.0, 0.0), n, [1.0, 0.0]),
            Vertex::new(Vec3::new(-1.0, 1.0, 0.0), n, [0.0, 0.0]),
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
}

#[test]
fn nearest_blocks_keep_internal_pixel_edges() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let green = assets.insert("green", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let mut world = world_with([(
        Camera::orthographic(2.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(4, 2),
        pose(),
    )]);
    world.spawn((Transform::from_xyz(-1.0, 0.0, 0.0), sprite("white", green, Vec2::new(2.0, 2.0))));
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    let internal = renderer.read_internal(&gpu, scene.passes[0].camera_id).unwrap();
    assert_eq!(internal.len(), 4 * 2 * 4);
    expect_px(&internal, 4, 0, 0, [0, 255, 0, 255], 2, "internal left");
    expect_px(&internal, 4, 1, 1, [0, 255, 0, 255], 2, "internal left bottom");
    expect_px(&internal, 4, 2, 0, [255, 0, 0, 255], 2, "internal right");
    expect_px(&image, 8, 0, 0, [0, 255, 0, 255], 2, "block origin");
    expect_px(&image, 8, 3, 3, [0, 255, 0, 255], 2, "end of the first blocks");
    expect_px(&image, 8, 4, 0, [255, 0, 0, 255], 2, "first pixel of the next column");
    expect_px(&image, 8, 7, 3, [255, 0, 0, 255], 2, "far corner");
    assert_eq!(renderer.stats().presentation_draws, 1);
    assert_eq!(renderer.stats().world_draw_calls, 1);
    assert_eq!(renderer.internal_targets().cameras, 1);
    assert_eq!(renderer.internal_targets().color_bytes, 4 * 2 * 4);
    assert_eq!(renderer.internal_targets().depth_bytes, 4 * 2 * 4);
}

#[test]
fn three_hundred_twenty_by_one_eighty_fills_a_720p_window() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let green = assets.insert("green", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let mut world = world_with([(
        Camera::orthographic(180.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(320, 180),
        pose(),
    )]);
    world.spawn((Transform::from_xyz(-152.0, 0.0, 0.0), sprite("white", green, Vec2::new(16.0, 16.0))));
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 1280, 720);
    let internal = renderer.read_internal(&gpu, scene.passes[0].camera_id).unwrap();
    assert_eq!(internal.len(), 320 * 180 * 4);
    expect_px(&internal, 320, 0, 90, [0, 255, 0, 255], 2, "internal sprite");
    expect_px(&internal, 320, 16, 90, [255, 0, 0, 255], 2, "internal clear past the sprite");
    expect_px(&image, 1280, 0, 360, [0, 255, 0, 255], 2, "scaled sprite block");
    expect_px(&image, 1280, 63, 360, [0, 255, 0, 255], 2, "last pixel of that block column");
    expect_px(&image, 1280, 64, 360, [255, 0, 0, 255], 2, "next internal pixel");
    assert_eq!(renderer.stats().presentation_draws, 1);
}

#[test]
fn thirteen_sixty_six_letterboxes_without_stretching() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with([(
        Camera::orthographic(180.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_letterbox_color(Color::rgb(0.0, 1.0, 0.0))
            .with_internal_resolution(320, 180),
        pose(),
    )]);
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 1366, 768);
    expect_px(&image, 1366, 0, 0, [0, 255, 0, 255], 2, "top-left bar");
    expect_px(&image, 1366, 42, 100, [0, 255, 0, 255], 2, "pixel before the image");
    expect_px(&image, 1366, 43, 24, [255, 0, 0, 255], 2, "image origin");
    expect_px(&image, 1366, 1322, 743, [255, 0, 0, 255], 2, "image far corner");
    expect_px(&image, 1366, 1323, 100, [0, 255, 0, 255], 2, "right bar");
    expect_px(&image, 1366, 100, 23, [0, 255, 0, 255], 2, "top bar");
    expect_px(&image, 1366, 100, 744, [0, 255, 0, 255], 2, "bottom bar");
    assert_eq!(renderer.stats().world_draw_calls, 0);
    assert_eq!(renderer.stats().presentation_draws, 1);
}

#[test]
fn a_nonzero_viewport_origin_places_the_scaled_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with([
        (Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()),
        (
            Camera {
                order: 1,
                ..Camera::orthographic(4.0)
                    .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
                    .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
                    .with_internal_resolution(4, 4)
            },
            pose(),
        ),
    ]);
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 32, 16);
    expect_px(&image, 32, 15, 8, [255, 0, 0, 255], 2, "pixel before the right viewport");
    expect_px(&image, 32, 16, 8, [0, 255, 0, 255], 2, "scaled image origin");
    expect_px(&image, 32, 31, 8, [0, 255, 0, 255], 2, "scaled image edge");
}

#[test]
fn scale_zero_fills_the_viewport_and_skips_the_scene() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let green = assets.insert("green", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let mut world = world_with([(
        Camera::orthographic(4.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_letterbox_color(Color::rgb(0.0, 0.0, 1.0))
            .with_internal_resolution(16, 16),
        pose(),
    )]);
    world.spawn((Transform::IDENTITY, sprite("white", green, Vec2::splat(2.0))));
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 8);
    assert!(matches!(scene.passes[0].view.fit, PresentationFit::Letterbox { .. }));
    assert_eq!(scene.passes[0].stats.sprites_visible, 0);
    for y in 0..8 {
        for x in 0..8 {
            expect_px(&image, 8, x, y, [0, 0, 255, 255], 2, "letterbox");
        }
    }
    assert_eq!(renderer.stats().world_draw_calls, 0);
    assert_eq!(renderer.stats().presentation_draws, 0);
    assert_eq!(renderer.internal_targets().cameras, 0);
}

#[test]
fn sprites_and_meshes_share_one_internal_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    assets.insert("card", card()).unwrap();
    let green = assets.insert("green", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let red = assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let mut world = world_with([(
        Camera::orthographic(8.0).with_clear_color(Color::BLACK).with_internal_resolution(32, 16),
        pose(),
    )]);
    world.spawn((Transform::IDENTITY, sprite("white", green, Vec2::splat(6.0))));
    world.spawn((Transform::from_xyz(0.0, 0.0, -1.0), Mesh::new("card").with_material_asset(red)));
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 64, 32);
    expect_px(&image, 64, 32, 16, [255, 0, 0, 255], 2, "nearer mesh");
    expect_px(&image, 64, 24, 16, [0, 255, 0, 255], 2, "sprite beside the mesh");
    assert!(renderer.stats().world_draw_calls >= 2);
    assert_eq!(renderer.stats().presentation_draws, 1);
}

#[test]
fn mask_and_blend_are_resolved_before_the_blit() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "solid", [255, 255, 255, 255]);
    insert_sprite(&mut assets, "hole", [255, 255, 255, 0]);
    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.5).unwrap()))
        .unwrap();
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(1.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let mut world = world_with([(
        Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(8, 4),
        pose(),
    )]);
    // Each quad is one half of the 8-wide view, so its pixel centers sit inside the primitive.
    world.spawn((Transform::from_xyz(-2.0, 0.0, 0.0), sprite("solid", mask, Vec2::splat(4.0))));
    world.spawn((Transform::from_xyz(2.0, 0.0, -0.2), sprite("hole", mask, Vec2::splat(4.0))));
    world.spawn((Transform::from_xyz(2.0, 0.0, -0.1), sprite("solid", glass, Vec2::splat(4.0))));
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    expect_px(&image, 16, 2, 4, [255, 255, 255, 255], 2, "kept mask, scaled");
    expect_px(&image, 16, 12, 4, [255, 128, 0, 255], 4, "blend over the internal clear, then scaled");
}

#[test]
fn a_point_light_still_shades_the_internal_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let lit = assets.insert("lit", MaterialAsset::color(Color::WHITE)).unwrap();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    world.spawn((Camera::orthographic(4.0).with_clear_color(Color::BLACK).with_internal_resolution(8, 4), pose()));
    world.spawn((Transform::IDENTITY, sprite("white", lit, Vec2::splat(2.0))));
    world.spawn((Transform::from_xyz(0.0, 0.0, -2.0), Light::point(Color::rgb(1.0, 0.0, 0.0), 4.0, 8.0)));
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    let pixel = px(&image, 16, 8, 4);
    assert!(pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40, "lit red, got {pixel:?}");
}

#[test]
fn depth_presentation_is_scaled_after_the_internal_view() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let green = assets.insert("green", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let mut world = world_with([(
        Camera::orthographic(4.0)
            .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
            .with_internal_resolution(8, 4)
            .with_presentation(CameraPresentation::Depth),
        pose(),
    )]);
    world.spawn((Transform::IDENTITY, sprite("white", green, Vec2::splat(4.0))));
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    let empty = [
        (DEPTH_VIEW_EMPTY[0] * 255.0) as u8,
        (DEPTH_VIEW_EMPTY[1] * 255.0) as u8,
        (DEPTH_VIEW_EMPTY[2] * 255.0) as u8,
        255,
    ];
    expect_px(&image, 16, 0, 0, empty, 2, "unwritten internal depth, scaled");
    let center = px(&image, 16, 8, 4);
    assert!(
        center[0] > 200 && (center[0] as i16 - center[1] as i16).abs() < 8,
        "linearized depth is bright gray, got {center:?}"
    );
    assert_eq!(renderer.stats().depth_view_draws, 1);
    assert_eq!(renderer.stats().presentation_draws, 1);
}

#[test]
fn perspective_and_custom_cameras_scale_without_a_new_projection() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let green = assets.insert("green", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let custom = CameraProjection::oblique_orthographic(4.0, 2.0, 0.1, 100.0, Vec2::new(0.2, 0.0)).unwrap();
    let mut world = world_with([
        (Camera::perspective(50.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)).with_internal_resolution(8, 4), pose()),
        (
            Camera {
                order: 1,
                ..Camera::new(custom).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(8, 4)
            },
            Transform::from_xyz(0.0, 0.0, -8.0),
        ),
    ]);
    world.spawn((Transform::IDENTITY, sprite("white", green, Vec2::splat(1.0))));
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    assert!(matches!(scene.passes[0].view.fit, PresentationFit::Scaled(_)));
    assert!(matches!(scene.passes[1].view.fit, PresentationFit::Scaled(_)));
    assert_eq!(scene.passes[1].view.proj, custom.matrix(99.0));
    assert_eq!(renderer.stats().presentation_draws, 2);
    let pixel = px(&image, 8, 4, 2);
    assert_ne!(pixel, [0, 0, 255, 255], "the later custom camera replaced the perspective clear");
}

#[test]
fn two_viewports_and_an_overlap_composite_independently() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut split = world_with([
        (
            Camera::orthographic(4.0)
                .with_clear_color(Color::rgb(1.0, 0.0, 0.0))
                .with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
            pose(),
        ),
        (
            Camera {
                order: 1,
                ..Camera::orthographic(4.0)
                    .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
                    .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
                    .with_internal_resolution(4, 8)
            },
            pose(),
        ),
    ]);
    let (image, _) = draw(&gpu, &mut renderer, &mut split, &assets, 32, 16);
    expect_px(&image, 32, 4, 8, [255, 0, 0, 255], 2, "direct half");
    expect_px(&image, 32, 20, 8, [0, 255, 0, 255], 2, "scaled half");
    assert_eq!(renderer.stats().presentation_draws, 1);
    assert_eq!(renderer.internal_targets().cameras, 1);

    let mut overlap = world_with([
        (Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()),
        (
            Camera {
                order: 1,
                ..Camera::orthographic(4.0)
                    .with_clear_color(Color::rgb(0.0, 0.0, 1.0))
                    .with_letterbox_color(Color::rgb(0.0, 1.0, 0.0))
                    .with_viewport(ViewportRect::new(0.0, 0.0, 1.0, 1.0))
                    .with_internal_resolution(6, 4)
            },
            pose(),
        ),
    ]);
    let (image, scene) = draw(&gpu, &mut renderer, &mut overlap, &assets, 20, 16);
    let PresentationFit::Scaled(image_fit) = scene.passes[1].view.fit else { panic!("{:?}", scene.passes[1].view.fit) };
    let bar_x = 0u32;
    let image_x = image_fit.display_origin.x as u32;
    expect_px(&image, 20, bar_x, 8, [0, 255, 0, 255], 2, "letterbox over the first camera");
    expect_px(&image, 20, image_x, image_fit.display_origin.y as u32, [0, 0, 255, 255], 2, "scaled clear");
}

#[test]
fn load_with_internal_resolution_does_not_replace_the_earlier_camera() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with([
        (Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), pose()),
        (
            Camera {
                order: 1,
                ..Camera::orthographic(4.0)
                    .with_clear(CameraClear::Load)
                    .with_clear_color(Color::rgb(0.0, 0.0, 1.0))
                    .with_internal_resolution(4, 4)
            },
            pose(),
        ),
    ]);
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    assert_eq!(scene.passes.len(), 1);
    assert_eq!(scene.stats.presentation_skipped, 1);
    expect_px(&image, 16, 8, 8, [255, 0, 0, 255], 2, "Load was not turned into a clear");
    assert_eq!(renderer.stats().presentation_draws, 0);
    assert_eq!(renderer.internal_targets().cameras, 0);
    assert_eq!(renderer.stats().world_draw_calls, 0);
}

#[test]
fn an_offscreen_target_receives_the_presented_image_and_a_later_camera_can_sample_it() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    let mut store = RenderTargetStore::new();
    let target = store.create(8, 4).unwrap();
    world.insert_resource(store);
    world.spawn((
        Camera::orthographic(2.0)
            .with_clear_color(Color::rgb(0.0, 0.0, 1.0))
            .with_output(CameraOutput::Target(target))
            .with_internal_resolution(4, 2),
        pose(),
    ));
    world.spawn((Camera { order: 1, ..Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)) }, pose()));
    let sampled = assets.insert("sampled", MaterialAsset::unlit(Color::WHITE)).unwrap();
    world.spawn((
        Transform::from_xyz(3.0, 0.0, 0.0),
        sprite("white", sampled, Vec2::new(1.5, 1.5)).with_render_target(target),
    ));
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    let offscreen = renderer.read_render_target(&gpu, target).unwrap();
    assert_eq!(offscreen.len(), 8 * 4 * 4);
    expect_px(&offscreen, 8, 0, 0, [0, 0, 255, 255], 2, "presented target");
    expect_px(&offscreen, 8, 7, 3, [0, 0, 255, 255], 2, "presented target corner");
    expect_px(&image, 16, 14, 4, [0, 0, 255, 255], 2, "surface sprite samples the presented target");
    expect_px(&image, 16, 0, 0, [255, 0, 0, 255], 2, "surface clear outside the sprite");
}

#[test]
fn switching_off_drops_the_private_image_and_matches_a_direct_camera() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut scaled = world_with([(
        Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 1.0, 0.0)).with_internal_resolution(4, 4),
        pose(),
    )]);
    let (first, _) = draw(&gpu, &mut renderer, &mut scaled, &assets, 8, 8);
    assert_eq!(renderer.internal_targets().cameras, 1);
    assert_eq!(renderer.resource_stats().internal_target_creates, 1);
    let (second, _) = draw(&gpu, &mut renderer, &mut scaled, &assets, 8, 8);
    assert_eq!(first, second);
    assert_eq!(renderer.resource_stats().internal_target_creates, 1);
    assert_eq!(renderer.resource_stats().internal_target_reallocs, 0);

    let mut direct = world_with([(Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 1.0, 0.0)), pose())]);
    // Change the scaled camera to direct and render again.
    {
        let mut query = scaled.query::<&mut Camera>();
        query.iter().next().unwrap().1.internal_resolution = None;
    }
    let (switched, _) = draw(&gpu, &mut renderer, &mut scaled, &assets, 8, 8);
    let (reference, _) = draw(&gpu, &mut renderer, &mut direct, &assets, 8, 8);
    assert_eq!(switched, reference);
    assert_eq!(renderer.stats().presentation_draws, 0);
    assert_eq!(renderer.internal_targets().cameras, 0);
    assert!(renderer.resource_stats().internal_target_drops >= 1);
}

#[test]
fn changing_the_internal_size_reallocates_once_and_a_direct_frame_allocates_nothing() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with([(
        Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(4, 2),
        pose(),
    )]);
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    {
        let mut query = world.query::<&mut Camera>();
        query.iter().next().unwrap().1.internal_resolution = Some((8, 4));
    }
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    let internal = renderer.read_internal(&gpu, scene.passes[0].camera_id).unwrap();
    assert_eq!(internal.len(), 8 * 4 * 4);
    expect_px(&image, 16, 0, 0, [255, 0, 0, 255], 2, "new size still covers the window");
    assert_eq!(renderer.resource_stats().internal_target_creates, 1);
    assert_eq!(renderer.resource_stats().internal_target_reallocs, 1);
    assert_eq!(renderer.internal_targets().color_bytes, 8 * 4 * 4);

    let mut plain = world_with([(Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)), pose())]);
    let before = renderer.resource_stats().internal_target_creates;
    let (_, _) = draw(&gpu, &mut renderer, &mut plain, &assets, 16, 8);
    assert_eq!(renderer.stats().presentation_draws, 0);
    assert_eq!(renderer.stats().draw_calls, 0);
    assert_eq!(renderer.stats().world_draw_calls, 0);
    assert_eq!(renderer.resource_stats().internal_target_creates, before);
    assert_eq!(renderer.internal_targets().cameras, 0);
}

fn srgb_renderer(gpu: &GpuContext) -> SceneRenderer {
    SceneRenderer::new(gpu, wgpu::TextureFormat::Rgba8UnormSrgb)
}

fn srgb_channel(linear: f32) -> u8 {
    let encoded = if linear <= 0.0031308 { 12.92 * linear } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn srgb_pixel(color: Color) -> [u8; 4] {
    [srgb_channel(color.r), srgb_channel(color.g), srgb_channel(color.b), 255]
}

fn assert_images_close(got: &[u8], expected: &[u8], tolerance: i16, what: &str) {
    assert_eq!(got.len(), expected.len(), "{what} byte length");
    let mut worst = 0i16;
    let mut index = 0usize;
    for (offset, (left, right)) in got.iter().zip(expected).enumerate() {
        let delta = (*left as i16 - *right as i16).abs();
        if delta > worst {
            worst = delta;
            index = offset;
        }
    }
    let pixel = index / 4 * 4;
    assert!(
        worst <= tolerance,
        "{what} differs by {worst} at byte {index}: {:?} vs {:?}",
        &got[pixel..pixel + 4],
        &expected[pixel..pixel + 4]
    );
}

fn covered(clear: Color, material: AssetHandle<MaterialAsset>) -> World {
    let mut world =
        world_with([(Camera::orthographic(4.0).with_clear_color(clear).with_internal_resolution(16, 8), pose())]);
    world.spawn((Transform::IDENTITY, sprite("white", material, Vec2::new(8.0, 4.0))));
    world
}

#[test]
fn srgb_presentation_matches_one_encode_of_the_direct_path() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = srgb_renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let paint = Color::rgb(0.5, 0.2, 0.8);
    let unlit = assets.insert("paint", MaterialAsset::unlit(paint)).unwrap();
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(1.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let lit = assets.insert("lit", MaterialAsset::color(Color::WHITE)).unwrap();

    let mut unlit_world = covered(Color::BLACK, unlit);
    let (internal, _) = draw(&gpu, &mut renderer, &mut unlit_world, &assets, 16, 8);
    {
        let mut query = unlit_world.query::<&mut Camera>();
        query.iter().next().unwrap().1.internal_resolution = None;
    }
    let (direct, _) = draw(&gpu, &mut renderer, &mut unlit_world, &assets, 16, 8);
    assert_images_close(&internal, &direct, 1, "unlit sRGB presentation");
    let expected = srgb_pixel(paint);
    let doubled =
        srgb_pixel(Color::rgb(expected[0] as f32 / 255.0, expected[1] as f32 / 255.0, expected[2] as f32 / 255.0));
    expect_px(&internal, 16, 8, 4, expected, 1, "one sRGB encode of the linear material");
    let center = px(&internal, 16, 8, 4);
    let double_gap = center.iter().zip(doubled).map(|(got, bad)| (*got as i16 - bad as i16).abs()).max().unwrap();
    assert!(double_gap > 8, "presentation applied a second gamma, got {center:?} near {doubled:?}");
    assert_eq!(renderer.stats().presentation_draws, 0, "the comparison frame is the direct path");

    let mut blend_world = covered(Color::rgb(1.0, 0.0, 0.0), glass);
    let (internal, _) = draw(&gpu, &mut renderer, &mut blend_world, &assets, 16, 8);
    {
        let mut query = blend_world.query::<&mut Camera>();
        query.iter().next().unwrap().1.internal_resolution = None;
    }
    let (direct, _) = draw(&gpu, &mut renderer, &mut blend_world, &assets, 16, 8);
    assert_images_close(&internal, &direct, 1, "blended sRGB presentation");

    let mut lit_world = World::new();
    lit_world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    lit_world.spawn((Camera::orthographic(4.0).with_clear_color(Color::BLACK).with_internal_resolution(16, 8), pose()));
    lit_world.spawn((Transform::IDENTITY, sprite("white", lit, Vec2::new(8.0, 4.0))));
    lit_world.spawn((Transform::from_xyz(0.0, 0.0, -2.0), Light::point(Color::rgb(1.0, 0.0, 0.0), 4.0, 8.0)));
    let (internal, _) = draw(&gpu, &mut renderer, &mut lit_world, &assets, 16, 8);
    {
        let mut query = lit_world.query::<&mut Camera>();
        query.iter().next().unwrap().1.internal_resolution = None;
    }
    let (direct, _) = draw(&gpu, &mut renderer, &mut lit_world, &assets, 16, 8);
    assert_images_close(&internal, &direct, 1, "lit sRGB presentation");
    let lit_pixel = px(&internal, 16, 8, 4);
    assert!(lit_pixel[0] > lit_pixel[1] + 20 && lit_pixel[0] > 40, "lit red survived presentation, got {lit_pixel:?}");
}

#[test]
fn srgb_nearest_blocks_keep_one_encoded_color() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = srgb_renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let paint = Color::rgb(0.4, 0.15, 0.7);
    let unlit = assets.insert("paint", MaterialAsset::unlit(paint)).unwrap();
    let mut world = world_with([(
        Camera::orthographic(2.0).with_clear_color(Color::rgb(0.05, 0.1, 0.2)).with_internal_resolution(4, 2),
        pose(),
    )]);
    world.spawn((Transform::from_xyz(-1.0, 0.0, 0.0), sprite("white", unlit, Vec2::splat(2.0))));
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    let sprite_px = srgb_pixel(paint);
    let clear_px = srgb_pixel(Color::rgb(0.05, 0.1, 0.2));
    expect_px(&image, 8, 0, 0, sprite_px, 1, "sRGB block origin");
    expect_px(&image, 8, 1, 1, sprite_px, 1, "sRGB block stays flat");
    expect_px(&image, 8, 3, 1, sprite_px, 1, "end of the sprite blocks");
    expect_px(&image, 8, 4, 0, clear_px, 1, "next internal pixel is the clear, not a filtered mix");
    assert_eq!(px(&image, 8, 0, 0), px(&image, 8, 1, 1));
}

#[test]
fn a_reused_entity_index_clears_the_previous_private_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    let first = world.spawn((
        Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(4, 2),
        pose(),
    ));
    let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    expect_px(&image, 8, 1, 1, [255, 0, 0, 255], 2, "first camera");
    assert_eq!(renderer.internal_targets().cameras, 1);
    world.despawn(first).unwrap();
    let second = world.spawn((
        Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 0.0, 1.0)).with_internal_resolution(4, 2),
        pose(),
    ));
    assert_eq!(first.id(), second.id(), "the live index is reused; the private image must not keep the old picture");
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    expect_px(&image, 8, 1, 1, [0, 0, 255, 255], 2, "reused index presents the new clear");
    let internal = renderer.read_internal(&gpu, scene.passes[0].camera_id).unwrap();
    expect_px(&internal, 4, 0, 0, [0, 0, 255, 255], 2, "reused image was cleared");
    assert_eq!(renderer.resource_stats().internal_target_creates, 1);
    assert_eq!(renderer.resource_stats().internal_target_reallocs, 0);
    assert_eq!(renderer.internal_targets().cameras, 1);

    {
        let mut query = world.query::<&mut Camera>();
        query.iter().next().unwrap().1.internal_resolution = None;
    }
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    assert_eq!(renderer.internal_targets().cameras, 0);
    assert_eq!(renderer.stats().presentation_draws, 0);
    assert!(renderer.resource_stats().internal_target_drops >= 1);
}

#[test]
fn replacing_cameras_does_not_retain_a_private_image_per_generation() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    for step in 0..16 {
        let red = step % 2 == 0;
        let color = if red { Color::rgb(1.0, 0.0, 0.0) } else { Color::rgb(0.0, 0.0, 1.0) };
        let entity =
            world.spawn((Camera::orthographic(4.0).with_clear_color(color).with_internal_resolution(4, 2), pose()));
        let (image, _) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
        let expected = if red { [255, 0, 0, 255] } else { [0, 0, 255, 255] };
        expect_px(&image, 8, 2, 1, expected, 2, "this generation");
        assert_eq!(renderer.internal_targets().cameras, 1);
        world.despawn(entity).unwrap();
        world.spawn((Camera::orthographic(4.0).with_clear_color(Color::BLACK), pose()));
        let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
        assert_eq!(renderer.internal_targets().cameras, 0);
        assert_eq!(renderer.stats().presentation_draws, 0);
        let direct = {
            let mut query = world.query::<&Camera>();
            query.iter().next().unwrap().0
        };
        world.despawn(direct).unwrap();
    }
    assert_eq!(renderer.resource_stats().internal_target_creates, 16);
    assert_eq!(renderer.resource_stats().internal_target_drops, 16);
    assert_eq!(renderer.internal_targets().color_bytes, 0);
}

#[test]
fn two_cameras_keep_distinct_private_images_when_reordered() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with([
        (Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)).with_internal_resolution(4, 2), pose()),
        (
            Camera {
                order: 1,
                ..Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 1.0, 0.0)).with_internal_resolution(4, 2)
            },
            pose(),
        ),
    ]);
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    expect_px(&image, 8, 1, 1, [0, 255, 0, 255], 2, "later camera covers the destination");
    let red = renderer.read_internal(&gpu, scene.passes[0].camera_id).unwrap();
    let green = renderer.read_internal(&gpu, scene.passes[1].camera_id).unwrap();
    expect_px(&red, 4, 0, 0, [255, 0, 0, 255], 2, "first private image");
    expect_px(&green, 4, 0, 0, [0, 255, 0, 255], 2, "second private image");
    assert_eq!(renderer.internal_targets().cameras, 2);
    assert_eq!(renderer.resource_stats().internal_target_creates, 2);

    {
        let mut query = world.query::<&mut Camera>();
        for camera in query.iter() {
            if camera.1.clear_color == Color::rgb(1.0, 0.0, 0.0) {
                camera.1.order = 2;
            }
        }
    }
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    expect_px(&image, 8, 1, 1, [255, 0, 0, 255], 2, "reordered red camera draws last");
    let first = renderer.read_internal(&gpu, scene.passes[0].camera_id).unwrap();
    let second = renderer.read_internal(&gpu, scene.passes[1].camera_id).unwrap();
    expect_px(&first, 4, 0, 0, [0, 255, 0, 255], 2, "green image after reorder");
    expect_px(&second, 4, 0, 0, [255, 0, 0, 255], 2, "red image after reorder");
    assert_eq!(renderer.resource_stats().internal_target_creates, 2);
    assert_eq!(renderer.resource_stats().internal_target_reallocs, 0);
}

#[test]
fn moving_a_camera_to_an_offscreen_target_reuses_its_private_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with([(
        Camera::orthographic(4.0).with_clear_color(Color::rgb(0.0, 1.0, 0.0)).with_internal_resolution(4, 2),
        pose(),
    )]);
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    assert_eq!(renderer.resource_stats().internal_target_creates, 1);
    let mut store = RenderTargetStore::new();
    let target = store.create(8, 4).unwrap();
    world.insert_resource(store);
    {
        let mut query = world.query::<&mut Camera>();
        query.iter().next().unwrap().1.output = CameraOutput::Target(target);
    }
    let _ = draw(&gpu, &mut renderer, &mut world, &assets, 8, 4);
    let image = renderer.read_render_target(&gpu, target).unwrap();
    expect_px(&image, 8, 1, 1, [0, 255, 0, 255], 2, "presented target");
    expect_px(&image, 8, 6, 2, [0, 255, 0, 255], 2, "presented target far side");
    assert_eq!(renderer.resource_stats().internal_target_creates, 1);
    assert_eq!(renderer.resource_stats().internal_target_reallocs, 0);
    assert_eq!(renderer.internal_targets().cameras, 1);
    assert_eq!(renderer.stats().presentation_draws, 1);
}

#[test]
fn a_rejected_load_camera_leaves_the_next_writer_s_clear_defined() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with([
        (
            Camera::orthographic(4.0)
                .with_clear(CameraClear::Load)
                .with_clear_color(Color::rgb(0.0, 0.0, 1.0))
                .with_internal_resolution(4, 4),
            pose(),
        ),
        (
            Camera {
                order: 1,
                ..Camera::orthographic(4.0)
                    .with_clear_color(Color::rgb(0.0, 1.0, 0.0))
                    .with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0))
            },
            pose(),
        ),
    ]);
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 16, 16);
    assert_eq!(scene.passes.len(), 1);
    assert_eq!(scene.stats.presentation_skipped, 1);
    expect_px(&image, 16, 1, 1, [0, 255, 0, 255], 2, "outside the later viewport is that camera's attachment clear");
    expect_px(&image, 16, 12, 8, [0, 255, 0, 255], 2, "inside the later viewport");
    assert_eq!(renderer.internal_targets().cameras, 0);
    assert_eq!(renderer.stats().presentation_draws, 0);
}

#[test]
fn a_direct_load_camera_still_composites_over_the_first_clear() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", [255, 255, 255, 255]);
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(1.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let mut world = world_with([
        (Camera::orthographic(4.0).with_clear_color(Color::rgb(1.0, 0.0, 0.0)), Transform::from_xyz(40.0, 0.0, -4.0)),
        (
            Camera { order: 1, clear: CameraClear::Load, ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) },
            pose(),
        ),
    ]);
    world.spawn((Transform::IDENTITY, sprite("white", glass, Vec2::splat(8.0))));
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, 16, 8);
    assert_eq!(scene.passes.len(), 2);
    assert_eq!(scene.stats.presentation_skipped, 0);
    expect_px(&image, 16, 8, 4, [255, 128, 0, 255], 4, "direct Load blends over the first camera");
    assert_eq!(renderer.internal_targets().cameras, 0);
    assert_eq!(renderer.stats().presentation_draws, 0);
}
