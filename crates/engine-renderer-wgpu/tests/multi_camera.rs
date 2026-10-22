//! Several surface cameras in one submitted frame.
//!
//! Pixel checks use the linear `Rgba8Unorm` target. A camera that borrowed another camera's
//! view-projection or instance range fails these reads: the expected texel is the one that
//! camera's own matrices and instances produce, and a shared range would paint the other color.

use engine_assets::{
    AlphaMode, AssetHandle, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture, Vertex,
};
use engine_core::Color;
use engine_math::{Mat4, Quat, Transform, Vec2, Vec3, Vec4};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

const W: u32 = 128;
const H: u32 = 128;

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

fn px(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect_px(img: &[u8], width: u32, x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, width, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn linear(rgba: [u8; 4]) -> Texture {
    Texture::solid(1, 1, rgba).with_color_space(ColorSpace::Linear)
}

fn insert_sprite(assets: &mut AssetManager, name: &str, texture: Texture) {
    let texture = assets.insert(&format!("{name}-tex"), texture).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
}

fn world_with(cameras: impl IntoIterator<Item = (Camera, Transform)>) -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    for (camera, transform) in cameras {
        world.spawn((camera, transform));
    }
    world
}

fn ortho(order: i32, viewport: ViewportRect, clear: Color, height: f32) -> Camera {
    Camera { order, viewport, ..Camera::orthographic(height).with_clear_color(clear) }
}

fn sprite(name: &str, material: AssetHandle<MaterialAsset>, size: f32) -> Sprite {
    Sprite::new(name).with_size(Vec2::splat(size)).with_anchor(Vec2::splat(0.5)).with_material(material).fixed()
}

fn billboard(name: &str, material: AssetHandle<MaterialAsset>, size: f32) -> Sprite {
    Sprite::new(name).with_size(Vec2::splat(size)).with_anchor(Vec2::splat(0.5)).with_material(material)
}

fn unlit(color: Color) -> MaterialAsset {
    MaterialAsset::unlit(color)
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

fn sample_projected(img: &[u8], width: u32, view: &CameraView, point: Vec3, expected: [u8; 4], what: &str) {
    let screen = view.project(point).unwrap_or_else(|| panic!("{what}: point does not project"));
    let x = (view.viewport_origin.x + screen.pixels.x).floor() as u32;
    let y = (view.viewport_origin.y + screen.pixels.y).floor() as u32;
    expect_px(img, width, x, y, expected, 2, what);
}

#[test]
fn split_screen_keeps_each_viewport_on_its_own_camera() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let green = assets.insert("green", unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let mut world = world_with([
        (
            ortho(0, ViewportRect::new(0.0, 0.0, 0.4, 1.0), Color::rgb(0.0, 0.0, 1.0), 4.0),
            Transform::from_xyz(0.0, 0.0, -4.0),
        ),
        (
            ortho(1, ViewportRect::new(0.6, 0.0, 0.4, 1.0), Color::rgb(1.0, 1.0, 0.0), 6.0),
            Transform::from_xyz(8.0, 0.0, -4.0),
        ),
    ]);
    world.spawn((Transform::IDENTITY, sprite("white", red, 8.0)));
    world.spawn((Transform::from_xyz(8.0, 0.0, 0.0), sprite("white", green, 8.0)));

    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, W, H);
    assert_eq!(scene.passes.len(), 2);
    assert_ne!(scene.passes[0].view.view_proj, scene.passes[1].view.view_proj);
    assert_eq!(scene.passes[0].stats.sprites_visible, 1);
    assert_eq!(scene.passes[1].stats.sprites_visible, 1);
    assert_eq!(scene.passes[0].opaque_sprites.instances.len(), 1);
    assert_eq!(scene.passes[1].opaque_sprites.instances.len(), 1);
    expect_px(&image, W, 25, 64, [255, 0, 0, 255], 2, "left camera's sprite");
    expect_px(&image, W, 64, 64, [0, 0, 255, 255], 2, "gap keeps the first camera's clear");
    expect_px(&image, W, 101, 64, [0, 255, 0, 255], 2, "right camera's sprite");
    expect_px(&image, W, 50, 4, [255, 0, 0, 255], 2, "left edge of the left viewport");
    expect_px(&image, W, 51, 4, [0, 0, 255, 255], 2, "the left camera does not cross its scissor");
    expect_px(&image, W, 75, 120, [0, 0, 255, 255], 2, "the right camera does not clear the gap");
    expect_px(&image, W, 76, 120, [0, 255, 0, 255], 2, "right viewport starts at its origin");
}

#[test]
fn camera_uniforms_and_instances_do_not_alias_across_passes() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let green = assets.insert("green", unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    // The left camera looks along +Z. The right camera looks along -X, so a shared billboard
    // matrix is edge-on to one of them and cannot fill both viewports.
    let mut world = world_with([
        (
            ortho(0, ViewportRect::new(0.0, 0.0, 0.25, 1.0), Color::rgb(0.0, 0.0, 1.0), 2.0),
            Transform::from_xyz(0.0, 0.0, -4.0),
        ),
        (
            ortho(1, ViewportRect::new(0.75, 0.0, 0.25, 1.0), Color::rgb(1.0, 1.0, 0.0), 2.0),
            Transform::looking_at(Vec3::new(20.0, 0.0, 10.0), Vec3::new(18.0, 0.0, 10.0), Vec3::Y),
        ),
    ]);
    world.spawn((Transform::IDENTITY, billboard("white", red, 8.0)));
    world.spawn((Transform::from_xyz(18.0, 0.0, 10.0), billboard("white", green, 8.0)));

    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, W, H);
    let (left, right) = (&scene.passes[0], &scene.passes[1]);
    assert_ne!(left.view.view_proj, right.view.view_proj);
    assert_ne!(left.view.position, right.view.position);
    assert!(!left.view.right.approx_eq(right.view.right, 1e-3));
    assert_eq!(left.stats.sprites_visible, 1, "the green billboard is outside the left frustum");
    assert_eq!(right.stats.sprites_visible, 1, "the red billboard is outside the right frustum");
    assert_ne!(left.opaque_sprites.instances[0].model, right.opaque_sprites.instances[0].model);
    assert!(left.opaque_sprites.instances[0].model.cols[0].xyz().normalized().approx_eq(left.view.right, 1e-3));
    assert!(right.opaque_sprites.instances[0].model.cols[0].xyz().normalized().approx_eq(right.view.right, 1e-3));

    // Left center is red only with the left view and the red instance. The right view would
    // paint green there. The right instances alone miss this viewport and leave it blue.
    expect_px(&image, W, 16, 64, [255, 0, 0, 255], 2, "left billboard");
    expect_px(&image, W, 31, 64, [255, 0, 0, 255], 2, "last pixel of the left viewport");
    expect_px(&image, W, 32, 64, [0, 0, 255, 255], 2, "outside the left scissor");
    expect_px(&image, W, 64, 64, [0, 0, 255, 255], 2, "neither later clear nor later sprite covers the gap");
    expect_px(&image, W, 95, 64, [0, 0, 255, 255], 2, "pixel before the right viewport");
    expect_px(&image, W, 96, 64, [0, 255, 0, 255], 2, "right viewport starts with its own instance");
    expect_px(&image, W, 112, 64, [0, 255, 0, 255], 2, "right billboard, which faces -X");
    let stats = renderer.stats();
    assert_eq!(stats.camera_passes, 2);
    assert_eq!(stats.world_draw_calls, 2);
    assert_eq!(stats.clear_draw_calls, 1, "only the later Color camera fills its rectangle");
    assert_eq!(stats.submitted_instances, 2);
    assert_eq!(renderer.pass_stats()[0].clear_draw_calls, 0);
    assert_eq!(renderer.pass_stats()[1].clear_draw_calls, 1);
    assert_eq!(renderer.pass_stats()[0].submitted_instances, 1);
    assert_eq!(renderer.pass_stats()[1].submitted_instances, 1);
}

#[test]
fn overlap_color_replaces_a_viewport_and_load_keeps_it() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let green = assets.insert("green", unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let glass =
        assets.insert("glass", unlit(Color::rgba(1.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let pip = ViewportRect::new(0.25, 0.25, 0.5, 0.5);
    let full = (ortho(0, ViewportRect::FULL, Color::rgb(1.0, 0.0, 0.0), 4.0), Transform::from_xyz(0.0, 0.0, -4.0));

    let mut color_empty =
        world_with([full, (ortho(1, pip, Color::rgb(0.0, 0.0, 1.0), 2.0), Transform::from_xyz(0.0, 0.0, -4.0))]);
    let (image, _) = draw(&gpu, &mut renderer, &mut color_empty, &assets, W, H);
    expect_px(&image, W, 4, 4, [255, 0, 0, 255], 2, "outside the inset stays the first clear");
    expect_px(&image, W, 31, 64, [255, 0, 0, 255], 2, "pixel before the inset");
    expect_px(&image, W, 32, 32, [0, 0, 255, 255], 2, "Color with no objects fills the inset");
    expect_px(&image, W, 64, 64, [0, 0, 255, 255], 2, "inset centre");
    expect_px(&image, W, 95, 90, [0, 0, 255, 255], 2, "last inset pixel");
    expect_px(&image, W, 96, 64, [255, 0, 0, 255], 2, "pixel after the inset");
    assert_eq!(renderer.stats().clear_draw_calls, 1);
    assert_eq!(renderer.stats().world_draw_calls, 0);

    let mut load_empty = world_with([
        full,
        (
            ortho(1, pip, Color::rgb(0.0, 0.0, 1.0), 2.0).with_clear(CameraClear::Load),
            Transform::from_xyz(0.0, 0.0, -4.0),
        ),
    ]);
    let (image, _) = draw(&gpu, &mut renderer, &mut load_empty, &assets, W, H);
    expect_px(&image, W, 64, 64, [255, 0, 0, 255], 2, "Load with no objects keeps the first camera");
    expect_px(&image, W, 4, 4, [255, 0, 0, 255], 2, "outside the inset");
    assert_eq!(renderer.stats().clear_draw_calls, 0);
    assert_eq!(renderer.stats().draw_calls, 0);

    let inset_pose = Transform::from_xyz(6.0, 0.0, -4.0);
    let mut color_sprite = world_with([full, (ortho(1, pip, Color::rgb(0.0, 0.0, 1.0), 2.0), inset_pose)]);
    color_sprite.spawn((Transform::from_xyz(6.0, 0.0, 0.0), sprite("white", green, 4.0)));
    let (image, scene) = draw(&gpu, &mut renderer, &mut color_sprite, &assets, W, H);
    assert_eq!(scene.passes[0].stats.sprites_visible, 0, "the inset sprite is outside the full-frame frustum");
    assert_eq!(scene.passes[1].stats.sprites_visible, 1);
    expect_px(&image, W, 64, 64, [0, 255, 0, 255], 2, "opaque inset replaces the first camera");
    expect_px(&image, W, 4, 4, [255, 0, 0, 255], 2, "the inset sprite stays inside its scissor");

    let mut load_sprite =
        world_with([full, (ortho(1, pip, Color::rgb(0.0, 0.0, 1.0), 2.0).with_clear(CameraClear::Load), inset_pose)]);
    load_sprite.spawn((Transform::from_xyz(6.0, 0.0, 0.0), sprite("white", green, 4.0)));
    let (image, _) = draw(&gpu, &mut renderer, &mut load_sprite, &assets, W, H);
    expect_px(&image, W, 64, 64, [0, 255, 0, 255], 2, "Load still draws the inset sprite");
    expect_px(&image, W, 4, 4, [255, 0, 0, 255], 2, "outside the inset");

    let mut load_blend =
        world_with([full, (ortho(1, pip, Color::rgb(0.0, 0.0, 1.0), 2.0).with_clear(CameraClear::Load), inset_pose)]);
    load_blend.spawn((Transform::from_xyz(6.0, 0.0, 0.0), sprite("white", glass, 4.0)));
    let (image, _) = draw(&gpu, &mut renderer, &mut load_blend, &assets, W, H);
    // Yellow 0.5 over the first camera's red.
    expect_px(&image, W, 64, 64, [255, 128, 0, 255], 3, "Load blends over the previous color");
    expect_px(&image, W, 4, 4, [255, 0, 0, 255], 2, "the blend does not leave the inset");
}

#[test]
fn a_later_camera_does_not_inherit_the_previous_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "solid", linear([255, 255, 255, 255]));
    insert_sprite(&mut assets, "hole", linear([255, 255, 255, 0]));
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let green = assets.insert("green", unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let white =
        assets.insert("white-mask", unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.5).unwrap())).unwrap();
    let glass =
        assets.insert("glass", unlit(Color::rgba(1.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend)).unwrap();

    let mut world = world_with([
        (ortho(0, ViewportRect::FULL, Color::BLACK, 4.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        (
            ortho(1, ViewportRect::new(0.25, 0.25, 0.5, 0.5), Color::rgb(0.0, 0.0, 1.0), 2.0),
            Transform::from_xyz(12.0, 0.0, -4.0),
        ),
    ]);
    // Close to camera A, so its depth is nearer than anything camera B draws. A leaked depth
    // buffer rejects B and leaves B's blue clear.
    world.spawn((Transform::from_xyz(0.0, 0.0, -3.8), sprite("solid", red, 8.0)));
    world.spawn((Transform::from_xyz(12.0, 0.0, 0.0), sprite("solid", green, 4.0)));
    world.spawn((Transform::from_xyz(11.55, 0.0, -1.2), sprite("solid", white, 0.7)));
    world.spawn((Transform::from_xyz(12.45, 0.0, -1.2), sprite("hole", white, 0.7)));
    world.spawn((Transform::from_xyz(12.0, 0.0, -0.6), sprite("solid", glass, 4.0)));

    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, W, H);
    assert_eq!(scene.passes[1].stats.sprites_visible, 4);
    assert_eq!(scene.passes[0].stats.sprites_visible, 1);
    expect_px(&image, W, 4, 4, [255, 0, 0, 255], 2, "camera A outside the inset");
    // Mask wrote depth, so the nearer-to-green blend behind it does not cover the white.
    expect_px(&image, W, 48, 64, [255, 255, 255, 255], 2, "kept mask fragment");
    // Discard wrote no depth, so the blend passes camera B's green and mixes yellow over it.
    expect_px(&image, W, 80, 64, [128, 255, 0, 255], 3, "discarded mask does not block the blend");
    expect_px(&image, W, 96, 64, [255, 0, 0, 255], 2, "inset scissor");
}

#[test]
fn equal_order_draws_the_higher_entity_id_last() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let pose = Transform::from_xyz(0.0, 0.0, -4.0);
    let mut higher_id_green = World::new();
    higher_id_green.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    let low = higher_id_green.spawn((ortho(0, ViewportRect::FULL, Color::rgb(1.0, 0.0, 0.0), 4.0), pose));
    let high = higher_id_green.spawn((ortho(0, ViewportRect::FULL, Color::rgb(0.0, 1.0, 0.0), 4.0), pose));
    assert!(low.id() < high.id());
    let (image, scene) = draw(&gpu, &mut renderer, &mut higher_id_green, &assets, W, H);
    assert_eq!(scene.passes[1].camera_id, high.id());
    expect_px(&image, W, 64, 64, [0, 255, 0, 255], 2, "the higher entity id overlays the full surface");
    assert_eq!(renderer.stats().clear_draw_calls, 1);
    assert_eq!(renderer.stats().world_draw_calls, 0);

    let mut higher_id_first = World::new();
    higher_id_first.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    higher_id_first.spawn((ortho(-1, ViewportRect::FULL, Color::rgb(0.0, 1.0, 0.0), 4.0), pose));
    higher_id_first.spawn((ortho(0, ViewportRect::FULL, Color::rgb(1.0, 0.0, 0.0), 4.0), pose));
    let (image, scene) = draw(&gpu, &mut renderer, &mut higher_id_first, &assets, W, H);
    assert_eq!(scene.passes[0].order, -1);
    expect_px(&image, W, 10, 20, [255, 0, 0, 255], 2, "a later order replaces the earlier clear");
}

#[test]
fn the_first_camera_clears_the_surface_even_when_its_policy_is_load() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut load = world_with([(
        ortho(0, ViewportRect::FULL, Color::rgb(1.0, 0.0, 0.0), 4.0).with_clear(CameraClear::Load),
        Transform::from_xyz(0.0, 0.0, -4.0),
    )]);
    let (loaded, _) = draw(&gpu, &mut renderer, &mut load, &assets, W, H);
    assert_eq!(renderer.stats().clear_draw_calls, 0);
    assert_eq!(renderer.stats().camera_passes, 1);
    let mut color = world_with([(
        ortho(0, ViewportRect::FULL, Color::rgb(1.0, 0.0, 0.0), 4.0),
        Transform::from_xyz(0.0, 0.0, -4.0),
    )]);
    let (colored, _) = draw(&gpu, &mut renderer, &mut color, &assets, W, H);
    assert_eq!(loaded, colored, "one camera's Load and Color both use the attachment clear");
    expect_px(&loaded, W, 3, 3, [255, 0, 0, 255], 2, "defined red, not an uninitialized surface");
}

#[test]
fn a_viewport_past_the_target_is_clamped_before_the_scissor() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = AssetManager::in_memory();
    let mut world = world_with([
        (ortho(0, ViewportRect::FULL, Color::rgb(1.0, 0.0, 0.0), 4.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        (
            ortho(1, ViewportRect::new(0.9, 0.0, 0.5, 1.0), Color::rgb(0.0, 1.0, 0.0), 4.0),
            Transform::from_xyz(0.0, 0.0, -4.0),
        ),
    ]);
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, W, H);
    assert_eq!(scene.passes[1].viewport_px, [115.0, 0.0, 13.0, 128.0]);
    expect_px(&image, W, 114, 40, [255, 0, 0, 255], 2, "pixel before the clamped viewport");
    expect_px(&image, W, 115, 40, [0, 255, 0, 255], 2, "clamped origin");
    expect_px(&image, W, 127, 100, [0, 255, 0, 255], 2, "clamped far edge");
}

#[test]
fn perspective_orthographic_and_custom_each_project_their_own_sprite() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    let red = assets.insert("red", unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let green = assets.insert("green", unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let blue = assets.insert("blue", unlit(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.2, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let custom = Mat4::orthographic_lh(-2.0, 2.0, -1.0, 1.0, 0.1, 40.0) * shear;
    let width = 192u32;
    let height = 128u32;
    let third = ViewportRect::new(0.0, 0.0, 1.0 / 3.0, 1.0);
    let mid = ViewportRect::new(1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0);
    let last = ViewportRect::new(2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0);
    let mut world = world_with([
        (
            Camera { order: 0, viewport: third, ..Camera::perspective(60.0).with_clear_color(Color::BLACK) },
            Transform::from_xyz(0.0, 0.0, -4.0),
        ),
        (
            Camera { order: 1, viewport: mid, ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) },
            Transform::from_xyz(30.0, 0.0, -4.0),
        ),
        (
            Camera {
                order: 2,
                viewport: last,
                ..Camera::new(CameraProjection::custom(custom).unwrap()).with_clear_color(Color::BLACK)
            },
            Transform::from_xyz(60.0, 0.0, -4.0),
        ),
    ]);
    let red_at = Vec3::new(0.35, 0.0, 0.0);
    let green_at = Vec3::new(30.0, 0.55, 0.0);
    let blue_at = Vec3::new(60.0, 0.0, 0.0);
    world.spawn((Transform::from_xyz(red_at.x, red_at.y, red_at.z), sprite("white", red, 0.4)));
    world.spawn((Transform::from_xyz(green_at.x, green_at.y, green_at.z), sprite("white", green, 0.4)));
    world.spawn((Transform::from_xyz(blue_at.x, blue_at.y, blue_at.z), sprite("white", blue, 0.4)));

    propagate_transforms(&mut world);
    let wide = extract_scene(&world, &assets, Vec2::new(width as f32, height as f32)).unwrap();
    let square = extract_scene(&world, &assets, Vec2::new(96.0, 128.0)).unwrap();
    assert!(wide.passes[2].view.proj.approx_eq(&custom, 1e-5));
    assert!(square.passes[2].view.proj.approx_eq(&custom, 1e-5), "custom ignores the resize");
    assert!(!wide.passes[0].view.proj.approx_eq(&square.passes[0].view.proj, 1e-4));
    assert!(!wide.passes[1].view.proj.approx_eq(&square.passes[1].view.proj, 1e-4));
    assert_ne!(wide.passes[0].view.proj, wide.passes[1].view.proj);
    assert_ne!(wide.passes[1].view.proj, wide.passes[2].view.proj);

    let image = renderer.render_to_rgba(&gpu, &wide, &assets, width, height).unwrap();
    // One renderer, the existing sprite pipelines. Projection kind is not a pipeline key.
    assert_eq!(wide.passes[0].stats.sprites_visible, 1);
    assert_eq!(wide.passes[1].stats.sprites_visible, 1);
    assert_eq!(wide.passes[2].stats.sprites_visible, 1);
    assert_eq!(renderer.stats().camera_passes, 3);
    assert_eq!(renderer.stats().world_draw_calls, 3);
    assert_eq!(renderer.stats().clear_draw_calls, 2);
    sample_projected(&image, width, &wide.passes[0].view, red_at, [255, 0, 0, 255], "perspective");
    sample_projected(&image, width, &wide.passes[1].view, green_at, [0, 255, 0, 255], "orthographic");
    sample_projected(&image, width, &wide.passes[2].view, blue_at, [0, 0, 255, 255], "custom");
    let custom_center_x = (wide.passes[2].view.viewport_origin.x + wide.passes[2].view.viewport.x * 0.5) as u32;
    let custom_center_y = (wide.passes[2].view.viewport.y * 0.5) as u32;
    let sheared = wide.passes[2].view.project(blue_at).unwrap();
    assert!(
        (sheared.pixels.x - wide.passes[2].view.viewport.x * 0.5).abs() > 4.0,
        "the shear moves the origin off the viewport centre"
    );
    expect_px(&image, width, custom_center_x, custom_center_y, [0, 0, 0, 255], 2, "custom centre is clear");
    expect_px(&image, width, 2, 2, [0, 0, 0, 255], 2, "perspective corner is not the red sprite");
}

#[test]
fn each_camera_submits_its_own_transparent_order() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "white", linear([255, 255, 255, 255]));
    assets.insert("card", card()).unwrap();
    let far = assets
        .insert("far", unlit(Color::rgba(0.0, 0.0, 1.0, 1.0)).with_alpha_mode(AlphaMode::Blend).double_sided())
        .unwrap();
    let mid = assets
        .insert("mid", unlit(Color::rgba(0.0, 1.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend).double_sided())
        .unwrap();
    let near = assets
        .insert("near", unlit(Color::rgba(1.0, 0.0, 0.0, 0.5)).with_alpha_mode(AlphaMode::Blend).double_sided())
        .unwrap();
    let mut world = world_with([
        (ortho(0, ViewportRect::new(0.0, 0.0, 0.5, 1.0), Color::BLACK, 2.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        (
            ortho(1, ViewportRect::new(0.5, 0.0, 0.5, 1.0), Color::BLACK, 2.0),
            Transform::from_xyz(0.0, 0.0, 6.0).with_rotation(Quat::from_rotation_y(core::f32::consts::PI)),
        ),
    ]);
    world.spawn((Transform::from_xyz(0.0, 0.0, 2.0), Mesh::new("card").with_material_asset(far)));
    world.spawn((Transform::from_xyz(0.0, 0.0, 1.0), sprite("white", mid, 2.0)));
    world.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(near)));

    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, W, H);
    let kind = |pass: &CameraPass| -> String {
        pass.transparent
            .iter()
            .map(|batch| match batch {
                TransparentBatch::Meshes { range, .. } => {
                    let color = pass.blended_meshes[range.start as usize].color;
                    if color[2] > 0.5 {
                        'b'
                    } else {
                        'r'
                    }
                }
                TransparentBatch::Sprites(batch) => {
                    let tint = pass.blended_sprites.instances[batch.range.start as usize].tint;
                    if tint[1] > 0.5 {
                        'g'
                    } else {
                        '?'
                    }
                }
            })
            .collect()
    };
    assert_eq!(kind(&scene.passes[0]), "bgr", "from the front, blue is farthest");
    assert_eq!(kind(&scene.passes[1]), "rgb", "from behind, red is farthest");
    assert_eq!(renderer.pass_stats()[0].world_draw_calls, 3, "the sequence is not rebatched");
    assert_eq!(renderer.pass_stats()[1].world_draw_calls, 3);
    // Blue, green 0.5, red 0.5 over black → (128, 64, 64). The back view draws blue last at alpha 1.
    expect_px(&image, W, 32, 64, [128, 64, 64, 255], 3, "front composite");
    expect_px(&image, W, 96, 64, [0, 0, 255, 255], 2, "back view draws the blue mesh last");
    expect_px(&image, W, 63, 64, [128, 64, 64, 255], 3, "front viewport ends at x=63");
    expect_px(&image, W, 64, 64, [0, 0, 255, 255], 2, "back viewport starts at x=64");
}

#[test]
fn two_cameras_submit_two_thousand_sprite_instances_and_one_texture() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    insert_sprite(&mut assets, "atlas", linear([255, 0, 0, 255]));
    let material = assets.insert("flat", unlit(Color::WHITE)).unwrap();
    let mut world = world_with([
        (ortho(0, ViewportRect::FULL, Color::BLACK, 4.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        (ortho(1, ViewportRect::FULL, Color::BLACK, 4.0), Transform::from_xyz(0.0, 0.0, -4.0)),
    ]);
    for i in 0..1000 {
        world.spawn((Transform::from_xyz(0.0, 0.0, i as f32 * 0.0001), sprite("atlas", material, 0.2)));
    }
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, Vec2::new(W as f32, H as f32)).unwrap();
    assert_eq!(scene.passes[0].opaque_sprites.batches.len(), 1);
    assert_eq!(scene.passes[1].opaque_sprites.batches.len(), 1);
    assert_eq!(scene.passes[0].opaque_sprites.batches[0].range, 0..1000);
    assert_eq!(scene.stats.sprites_visible, 2000);

    let before = renderer.resource_stats();
    let _ = renderer.render_to_rgba(&gpu, &scene, &assets, W, H).unwrap();
    let stats = renderer.stats();
    let resources = renderer.resource_stats();
    assert_eq!(stats.world_draw_calls, 2);
    assert_eq!(stats.clear_draw_calls, 1);
    assert_eq!(stats.draw_calls, 3);
    assert_eq!(stats.submitted_instances, 2000);
    assert_eq!(stats.sprite_instances, 2000);
    assert_eq!(stats.largest_draw_instances, 1000);
    assert_eq!(stats.camera_passes, 2);
    assert_eq!(renderer.pass_stats()[0].world_draw_calls, 1);
    assert_eq!(renderer.pass_stats()[0].clear_draw_calls, 0);
    assert_eq!(renderer.pass_stats()[0].submitted_instances, 1000);
    assert_eq!(renderer.pass_stats()[1].world_draw_calls, 1);
    assert_eq!(renderer.pass_stats()[1].clear_draw_calls, 1);
    assert_eq!(renderer.pass_stats()[1].submitted_instances, 1000);
    assert_eq!(resources.texture_creates - before.texture_creates, 1);
    assert_eq!(renderer.cache_stats().textures, 1);
    assert_eq!(resources.instance_buffer_grows - before.instance_buffer_grows, 1);
    eprintln!(
        "two-camera 1000 sprites: world_draws={} clear_draws={} total_draws={} instances={} texture_creates={} buffer_grows={}",
        stats.world_draw_calls,
        stats.clear_draw_calls,
        stats.draw_calls,
        stats.submitted_instances,
        resources.texture_creates - before.texture_creates,
        resources.instance_buffer_grows - before.instance_buffer_grows
    );
}

#[test]
fn two_cameras_share_one_mesh_geometry_cache_entry() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = AssetManager::in_memory();
    assets.insert("cube", MeshData::cube(0.6).unwrap()).unwrap();
    let mut world = world_with([
        (ortho(0, ViewportRect::new(0.0, 0.0, 0.5, 1.0), Color::BLACK, 4.0), Transform::from_xyz(0.0, 0.0, -4.0)),
        (ortho(1, ViewportRect::new(0.5, 0.0, 0.5, 1.0), Color::BLACK, 4.0), Transform::from_xyz(0.0, 0.0, -4.0)),
    ]);
    world.spawn((
        Transform::IDENTITY,
        Mesh::new("cube").with_material(Material { lit: false, ..Material::color(Color::rgb(0.0, 0.0, 1.0)) }),
    ));
    let (image, scene) = draw(&gpu, &mut renderer, &mut world, &assets, W, H);
    assert_eq!(scene.stats.meshes_visible, 2);
    assert_eq!(scene.passes[0].meshes.len(), 1);
    assert_eq!(scene.passes[1].meshes.len(), 1);
    assert_eq!(renderer.cache_stats().meshes, 1);
    assert_eq!(renderer.stats().mesh_instances, 2);
    assert_eq!(renderer.stats().world_draw_calls, 2);
    assert_eq!(renderer.stats().submitted_instances, 2);
    expect_px(&image, W, 32, 64, [0, 0, 255, 255], 2, "left camera's cube");
    expect_px(&image, W, 96, 64, [0, 0, 255, 255], 2, "right camera's cube, same geometry cache");
}
