//! Depth presentation reads the depth image the production pass just stored.
//!
//! These tests do not sort entity centers and do not draw a second scene. A shaded camera keeps
//! its color and its draw count. Pixels are `Rgba8Unorm`, so a linear nearness maps to a byte
//! with ordinary rounding.

use engine_assets::{AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture};
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

fn px(img: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn expect_px(img: &[u8], width: u32, x: u32, y: u32, expected: [u8; 4], tolerance: i16, what: &str) {
    let pixel = px(img, width, x, y);
    let close = pixel.iter().zip(expected).all(|(got, want)| (*got as i16 - want as i16).abs() <= tolerance);
    assert!(close, "{what} at ({x},{y}) is {pixel:?}, expected {expected:?} ±{tolerance}");
}

fn color_bytes(color: [f32; 4]) -> [u8; 4] {
    color.map(|channel| (channel * 255.0).round() as u8)
}

fn assert_empty(img: &[u8], width: u32, x: u32, y: u32, what: &str) {
    expect_px(img, width, x, y, color_bytes(DEPTH_VIEW_EMPTY), 3, what);
}

fn assert_unsupported(img: &[u8], width: u32, x: u32, y: u32, what: &str) {
    expect_px(img, width, x, y, color_bytes(DEPTH_VIEW_UNSUPPORTED), 3, what);
}

fn assert_nearness(img: &[u8], width: u32, x: u32, y: u32, nearness: f32, tolerance: i16, what: &str) {
    let expected = (nearness.clamp(0.0, 1.0) * 255.0).round() as u8;
    let pixel = px(img, width, x, y);
    assert!(
        pixel[0].abs_diff(pixel[1]) <= 2 && pixel[0].abs_diff(pixel[2]) <= 2 && pixel[3] >= 250,
        "{what} at ({x},{y}) is {pixel:?}, expected a gray depth sample"
    );
    assert!(
        (pixel[0] as i16 - expected as i16).abs() <= tolerance,
        "{what} at ({x},{y}) is {pixel:?}, expected nearness {nearness} (byte {expected}) ±{tolerance}"
    );
}

fn assert_brighter(img: &[u8], width: u32, near: (u32, u32), far: (u32, u32), margin: u8, what: &str) {
    let a = px(img, width, near.0, near.1);
    let b = px(img, width, far.0, far.1);
    assert!(
        a[0] > b[0].saturating_add(margin) && a[0].abs_diff(a[1]) <= 2 && b[0].abs_diff(b[1]) <= 2,
        "{what}: nearer {a:?} at {near:?} should be brighter than farther {b:?} at {far:?} by >{margin}"
    );
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let white = assets.insert("white", Texture::solid(1, 1, [255; 4]).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(2.0)).unwrap()).unwrap();
    assets
}

fn scene_world() -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world
}

fn render_at(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
    width: u32,
    height: u32,
) -> (Vec<u8>, RenderScene) {
    propagate_transforms(world);
    let scene = extract_scene(world, assets, Vec2::new(width as f32, height as f32)).unwrap();
    let image = renderer.render_to_rgba(gpu, &scene, assets, width, height).unwrap();
    (image, scene)
}

fn render(
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    world: &mut World,
    assets: &AssetManager,
) -> (Vec<u8>, RenderScene) {
    render_at(gpu, renderer, world, assets, W, H)
}

fn depth_cam(camera: Camera, near: f32, far: f32) -> Camera {
    Camera {
        projection: camera.projection.with_clip_range(near, far),
        ..camera.with_presentation(CameraPresentation::Depth).with_clear_color(Color::BLACK)
    }
}

fn quad(color: Color, size: f32) -> Sprite {
    Sprite::new("quad").with_size(Vec2::splat(size)).with_anchor(Vec2::splat(0.5)).with_tint(color).cutout().unlit()
}

fn mesh(color: Color) -> Mesh {
    Mesh::new("cube").with_material(Material { lit: false, ..Material::color(color) })
}

/// Window pixel of a world point, plus the nearness [`classify_stored_depth`] assigns to its
/// stored reverse-Z value.
fn sample(view: &CameraView, point: Vec3) -> (u32, u32, f32) {
    let screen = view.project(point).unwrap_or_else(|| panic!("point {point:?} is not in front of the camera"));
    let x = (view.viewport_origin.x + screen.pixels.x).round() as u32;
    let y = (view.viewport_origin.y + screen.pixels.y).round() as u32;
    let StoredDepthClass::Linear { nearness, .. } =
        classify_stored_depth(view.proj, screen.ndc.x, screen.ndc.y, screen.depth)
    else {
        panic!("point {point:?} did not linearize, ndc {:?} depth {}", screen.ndc, screen.depth);
    };
    (x, y, nearness)
}

#[test]
fn perspective_depth_is_linear_in_view_z_and_orders_near_and_far() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = scene_world();
    world.spawn((depth_cam(Camera::perspective(60.0), 1.0, 9.0), Transform::IDENTITY));
    world.spawn((Transform::from_xyz(-1.6, 0.0, 2.0), quad(Color::rgb(1.0, 0.0, 0.0), 1.2)));
    world.spawn((Transform::from_xyz(0.0, 0.0, 5.0), quad(Color::rgb(0.0, 1.0, 0.0), 1.2)));
    world.spawn((Transform::from_xyz(1.6, 0.0, 7.0), quad(Color::rgb(0.0, 0.0, 1.0), 1.2)));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let view = &scene.sole_pass().view;
    let (nx, ny, nearness) = sample(view, Vec3::new(-1.6, 0.0, 2.0));
    let (mx, my, mid) = sample(view, Vec3::new(0.0, 0.0, 5.0));
    let (fx, fy, farness) = sample(view, Vec3::new(1.6, 0.0, 7.0));
    assert_nearness(&image, W, mx, my, mid, 4, "perspective midpoint is linear view depth");
    assert!(mid > 0.4 && mid < 0.6, "z=5 of 1..9 is the linear midpoint, got {mid}");
    assert_brighter(&image, W, (nx, ny), (fx, fy), 40, "perspective nearer sprite is brighter");
    assert_nearness(&image, W, nx, ny, nearness, 6, "near sprite");
    assert_nearness(&image, W, fx, fy, farness, 6, "far sprite");
    let raw_mid = {
        let clip = view.proj * Vec4::new(0.0, 0.0, 5.0, 1.0);
        clip.z / clip.w
    };
    assert!(raw_mid < 0.2, "stored perspective depth at the midpoint is not the displayed gray");
    assert_eq!(renderer.stats().depth_view_draws, 1);
    assert_eq!(renderer.stats().world_draw_calls, 1);

    let (small, _) = render_at(&gpu, &mut renderer, &mut world, &assets, 64, 64);
    let scene = extract_scene(&world, &assets, Vec2::splat(64.0)).unwrap();
    let view = &scene.sole_pass().view;
    let (nx, ny, _) = sample(view, Vec3::new(-1.6, 0.0, 2.0));
    let (fx, fy, _) = sample(view, Vec3::new(1.6, 0.0, 7.0));
    assert_brighter(&small, 64, (nx, ny), (fx, fy), 30, "a resized viewport keeps the depth order");
}

#[test]
fn orthographic_side_top_down_isometric_and_steep_views_order_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let cases = [
        (
            "orthographic",
            depth_cam(Camera::orthographic(8.0), 0.5, 20.0),
            Transform::from_xyz(0.0, 0.0, -4.0),
            Vec3::new(-1.5, 0.0, 1.0),
            Vec3::new(1.5, 0.0, 10.0),
        ),
        (
            "side",
            depth_cam(Camera::orthographic(8.0), 0.5, 24.0),
            Transform::looking_at(Vec3::ZERO, Vec3::X, Vec3::Y),
            Vec3::new(4.0, 1.4, 0.0),
            Vec3::new(14.0, -1.4, 0.0),
        ),
        (
            "top-down",
            depth_cam(Camera::orthographic(10.0), 0.5, 24.0),
            Transform::looking_at(Vec3::new(0.0, 16.0, 0.0), Vec3::ZERO, Vec3::Z),
            Vec3::new(1.5, 10.0, 0.0),
            Vec3::new(-1.5, 2.0, 0.0),
        ),
        (
            "isometric",
            depth_cam(Camera::orthographic(16.0), 0.5, 40.0),
            Transform::looking_at(Vec3::new(12.0, 12.0, 12.0), Vec3::ZERO, Vec3::Y),
            Vec3::new(3.0, 2.0, -1.0),
            Vec3::new(-3.0, 0.0, 1.0),
        ),
        (
            "steep",
            depth_cam(Camera::perspective(50.0), 0.5, 30.0),
            Transform::looking_at(Vec3::new(0.0, 8.0, -4.0), Vec3::new(0.0, 0.0, 6.0), Vec3::Y),
            Vec3::new(1.2, 0.5, 1.0),
            Vec3::new(-1.2, 0.5, 8.0),
        ),
    ];
    for (name, camera, pose, near, far) in cases {
        let mut world = scene_world();
        world.spawn((camera, pose));
        world.spawn((Transform::from_position(near), quad(Color::WHITE, 1.2)));
        world.spawn((Transform::from_position(far), quad(Color::WHITE, 1.2)));
        let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
        let view = &scene.sole_pass().view;
        let (nx, ny, nearness) = sample(view, near);
        let (fx, fy, farness) = sample(view, far);
        assert!(nearness > farness + 0.08, "{name} cpu nearness {nearness} vs {farness}");
        assert_brighter(&image, W, (nx, ny), (fx, fy), 12, name);
        assert_eq!(renderer.stats().depth_view_draws, 1, "{name}");
        assert_empty(&image, W, 2, 2, name);
    }
}

#[test]
fn sprites_and_meshes_store_the_nearer_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let camera = depth_cam(Camera::orthographic(8.0), 0.5, 20.0);
    let pose = Transform::from_xyz(0.0, 0.0, -4.0);

    let mut sprite_front = scene_world();
    sprite_front.spawn((camera, pose));
    sprite_front.spawn((Transform::from_xyz(0.0, 0.0, 0.0), quad(Color::rgb(1.0, 0.0, 0.0), 3.0)));
    sprite_front.spawn((Transform::from_xyz(0.0, 0.0, 4.0), mesh(Color::rgb(0.0, 0.0, 1.0))));
    let (image, scene) = render(&gpu, &mut renderer, &mut sprite_front, &assets);
    let (x, y, nearness) = sample(&scene.sole_pass().view, Vec3::ZERO);
    assert_nearness(&image, W, x, y, nearness, 5, "a cutout sprite in front of a mesh stores the sprite");

    let mut mesh_front = scene_world();
    mesh_front.spawn((camera, pose));
    mesh_front.spawn((Transform::from_xyz(0.0, 0.0, 5.0), quad(Color::rgb(1.0, 0.0, 0.0), 4.0)));
    mesh_front.spawn((Transform::from_xyz(0.0, 0.0, 1.0), mesh(Color::rgb(0.0, 0.0, 1.0))));
    let (image, scene) = render(&gpu, &mut renderer, &mut mesh_front, &assets);
    // The cuboid is 2 units across and centered at z=1, so the face toward the camera is z=0.
    let (x, y, nearness) = sample(&scene.sole_pass().view, Vec3::new(0.0, 0.0, 0.0));
    assert_nearness(&image, W, x, y, nearness, 6, "a mesh in front of a sprite stores the mesh's near face");
    let (_, _, sprite_only) = sample(&scene.sole_pass().view, Vec3::new(0.0, 0.0, 5.0));
    assert!(nearness > sprite_only + 0.1);
}

#[test]
fn discarded_mask_and_cutout_pixels_keep_the_depth_behind_them() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = assets();
    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.5).unwrap()))
        .unwrap();
    let camera = depth_cam(Camera::orthographic(8.0), 0.5, 20.0);
    let pose = Transform::from_xyz(0.0, 0.0, -4.0);
    let far_at = Vec3::new(0.0, 0.0, 8.0);

    let mut far_only = scene_world();
    far_only.spawn((camera, pose));
    far_only.spawn((Transform::from_position(far_at), quad(Color::WHITE, 5.0)));
    let (far_image, far_scene) = render(&gpu, &mut renderer, &mut far_only, &assets);
    let (x, y, farness) = sample(&far_scene.sole_pass().view, far_at);
    assert_nearness(&far_image, W, x, y, farness, 4, "far sprite alone");

    let mut kept = scene_world();
    kept.spawn((camera, pose));
    kept.spawn((Transform::from_position(far_at), quad(Color::WHITE, 5.0)));
    kept.spawn((
        Transform::IDENTITY,
        Sprite::new("quad").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).with_material(mask),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut kept, &assets);
    let (_, _, nearness) = sample(&scene.sole_pass().view, Vec3::ZERO);
    assert_nearness(&image, W, x, y, nearness, 5, "a kept mask writes its own depth");
    assert!(nearness > farness + 0.2);

    let mut dropped = scene_world();
    dropped.spawn((camera, pose));
    dropped.spawn((Transform::from_position(far_at), quad(Color::WHITE, 5.0)));
    dropped.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgba(1.0, 1.0, 1.0, 0.0))
            .with_material(mask),
    ));
    let (image, _) = render(&gpu, &mut renderer, &mut dropped, &assets);
    assert_nearness(&image, W, x, y, farness, 5, "a discarded mask pixel does not replace the depth behind it");

    let mut cutout = scene_world();
    cutout.spawn((camera, pose));
    cutout.spawn((Transform::from_position(far_at), quad(Color::WHITE, 5.0)));
    cutout.spawn((Transform::IDENTITY, quad(Color::rgba(1.0, 1.0, 1.0, 0.0), 2.0)));
    let (image, _) = render(&gpu, &mut renderer, &mut cutout, &assets);
    assert_nearness(&image, W, x, y, farness, 5, "a discarded cutout pixel does not replace the depth behind it");
}

#[test]
fn blended_sprites_and_debug_lines_do_not_write_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let mut assets = assets();
    let glass = assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(1.0, 1.0, 1.0, 0.5)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let camera = depth_cam(Camera::orthographic(8.0), 0.5, 20.0);
    let pose = Transform::from_xyz(0.0, 0.0, -4.0);

    let mut opaque = scene_world();
    opaque.spawn((camera, pose));
    opaque.spawn((Transform::from_xyz(0.0, 0.0, 6.0), quad(Color::WHITE, 4.0)));
    let (alone, scene) = render(&gpu, &mut renderer, &mut opaque, &assets);
    let (x, y, farness) = sample(&scene.sole_pass().view, Vec3::new(0.0, 0.0, 6.0));
    let center = px(&alone, W, x, y);

    let mut blended = scene_world();
    blended.spawn((camera, pose));
    blended.spawn((Transform::from_xyz(0.0, 0.0, 6.0), quad(Color::WHITE, 4.0)));
    blended.spawn((
        Transform::IDENTITY,
        Sprite::new("quad").with_size(Vec2::splat(3.0)).with_anchor(Vec2::splat(0.5)).with_material(glass),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut blended, &assets);
    expect_px(&image, W, x, y, center, 2, "a blended sprite does not write a nearer depth");
    let (_, _, hidden) = sample(&scene.sole_pass().view, Vec3::ZERO);
    assert!(hidden > farness + 0.15, "the blend sits nearer, but that depth is not stored");

    let mut lines = scene_world();
    lines.insert_resource(DebugDraw::default());
    lines.spawn((camera, pose));
    lines.spawn((Transform::from_xyz(0.0, 0.0, 6.0), quad(Color::WHITE, 4.0)));
    lines.resource_mut::<DebugDraw>().line(
        Vec3::new(-6.0, -6.0, 6.0),
        Vec3::new(6.0, 6.0, 6.0),
        Color::rgb(0.0, 1.0, 0.0),
    );
    let (image, _) = render(&gpu, &mut renderer, &mut lines, &assets);
    expect_px(&image, W, x, y, center, 2, "a debug line does not change stored depth");
    assert_empty(&image, W, 2, 2, "background outside the sprite stays unwritten");
    assert!(renderer.stats().world_draw_calls > 1, "the line is a real draw and still not a depth write");
    assert_eq!(renderer.stats().depth_view_draws, 1);
}

#[test]
fn coplanar_priority_changes_the_stored_depth_the_view_displays() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = scene_world();
    world.insert_resource(CoplanarSettings::new(0.4).unwrap());
    world.spawn((depth_cam(Camera::orthographic(8.0), 0.5, 20.0), Transform::from_xyz(0.0, 0.0, -4.0)));
    world.spawn((Transform::from_xyz(-1.4, 0.0, 2.0), quad(Color::WHITE, 1.4), CoplanarPriority::new(0).unwrap()));
    world.spawn((Transform::from_xyz(1.4, 0.0, 2.0), quad(Color::WHITE, 1.4), CoplanarPriority::new(7).unwrap()));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let view = &scene.sole_pass().view;
    let (lx, ly, _) = sample(view, Vec3::new(-1.4, 0.0, 2.0));
    let (rx, ry, _) = sample(view, Vec3::new(1.4, 0.0, 2.0));
    assert_brighter(&image, W, (rx, ry), (lx, ly), 15, "level 7 is stored closer than level 0");
}

#[test]
fn near_and_far_clipping_leave_no_written_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let camera = depth_cam(Camera::orthographic(8.0), 1.0, 8.0);
    let pose = Transform::from_xyz(0.0, 0.0, -2.0);

    let mut beyond = scene_world();
    beyond.spawn((camera, pose));
    beyond.spawn((Transform::from_xyz(0.0, 0.0, 12.0), quad(Color::WHITE, 3.0)));
    let (image, _) = render(&gpu, &mut renderer, &mut beyond, &assets);
    assert_empty(&image, W, W / 2, H / 2, "a sprite past the far plane does not write depth");

    let mut before = scene_world();
    before.spawn((camera, pose));
    before.spawn((Transform::from_xyz(0.0, 0.0, -4.0), quad(Color::WHITE, 3.0)));
    let (image, _) = render(&gpu, &mut renderer, &mut before, &assets);
    assert_empty(&image, W, W / 2, H / 2, "a sprite in front of the near plane does not write depth");

    let mut inside = scene_world();
    inside.spawn((camera, pose));
    inside.spawn((Transform::from_xyz(0.0, 0.0, 2.0), quad(Color::WHITE, 3.0)));
    let (image, scene) = render(&gpu, &mut renderer, &mut inside, &assets);
    let (x, y, nearness) = sample(&scene.sole_pass().view, Vec3::new(0.0, 0.0, 2.0));
    assert_nearness(&image, W, x, y, nearness, 5, "a sprite inside the clip range is written");
    assert!(px(&image, W, x, y)[0] > 40 && px(&image, W, x, y)[0] < 230);
}

#[test]
fn a_tilemap_keeps_world_depth_across_its_tiles() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut map = TileMap::new("white", 1, 1, Vec2::splat(2.0), 1, 4).filled_with(|_, _| 0);
    map.lit = false;
    let mut pose = Transform::IDENTITY;
    pose.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let mut world = scene_world();
    let camera = depth_cam(Camera::orthographic(14.0), 0.5, 30.0);
    let camera_pose = Transform::looking_at(Vec3::new(1.0, 6.0, -2.0), Vec3::new(1.0, 0.0, 4.0), Vec3::Y);
    world.spawn((camera, camera_pose));
    world.spawn((pose, map));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let model = pose.to_matrix();
    let center = |row: f32| (model * Vec3::new(1.0, (row + 0.5) * 2.0, 0.0).extend(1.0)).xyz();
    let near = center(0.0);
    let far = center(3.0);
    let view = &scene.sole_pass().view;
    let (nx, ny, nearness) = sample(view, near);
    let (fx, fy, farness) = sample(view, far);
    assert!(nearness > farness + 0.05, "tile depths {nearness} and {farness} at {near:?} {far:?}");
    assert_brighter(&image, W, (nx, ny), (fx, fy), 8, "tilemap tiles keep their world depth");
    assert_eq!(renderer.stats().depth_view_draws, 1);
}

#[test]
fn separate_viewports_show_each_cameras_own_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let left = ViewportRect::new(0.0, 0.0, 0.5, 1.0);
    let right = ViewportRect::new(0.5, 0.0, 0.5, 1.0);
    let mut world = scene_world();
    world.spawn((
        depth_cam(Camera::orthographic(6.0), 0.5, 20.0).with_viewport(left),
        Transform::from_xyz(-20.0, 0.0, -4.0),
    ));
    world.spawn((
        Camera { order: 1, viewport: right, ..depth_cam(Camera::orthographic(6.0), 0.5, 20.0) },
        Transform::from_xyz(20.0, 0.0, -4.0),
    ));
    world.spawn((Transform::from_xyz(-20.0, 0.0, 1.0), quad(Color::rgb(1.0, 0.0, 0.0), 2.0)));
    world.spawn((Transform::from_xyz(20.0, 0.0, 12.0), quad(Color::rgb(0.0, 0.0, 1.0), 2.0)));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(scene.passes.len(), 2);
    let (lx, ly, left_near) = sample(&scene.passes[0].view, Vec3::new(-20.0, 0.0, 1.0));
    let (rx, ry, right_near) = sample(&scene.passes[1].view, Vec3::new(20.0, 0.0, 12.0));
    assert!(lx < W / 2 && rx >= W / 2, "samples landed in {lx} and {rx}");
    assert_nearness(&image, W, lx, ly, left_near, 6, "left viewport is the first camera's depth");
    assert_nearness(&image, W, rx, ry, right_near, 6, "right viewport is the second camera's depth");
    assert_brighter(
        &image,
        W,
        (lx, ly),
        (rx, ry),
        25,
        "the later camera did not clear the earlier depth image before it was shown",
    );
    assert_eq!(renderer.stats().depth_view_draws, 2);
    assert_empty(&image, W, 4, 4, "left background");
    assert_empty(&image, W, W - 4, 4, "right background");

    let mut split = scene_world();
    split.spawn((
        Camera::orthographic(6.0)
            .with_viewport(left)
            .with_clear_color(Color::BLACK)
            .with_presentation(CameraPresentation::Shaded),
        Transform::from_xyz(-20.0, 0.0, -4.0),
    ));
    split.spawn((
        Camera { order: 1, viewport: right, ..depth_cam(Camera::orthographic(6.0), 0.5, 20.0) },
        Transform::from_xyz(20.0, 0.0, -4.0),
    ));
    split.spawn((Transform::from_xyz(-20.0, 0.0, 1.0), quad(Color::rgb(1.0, 0.0, 0.0), 2.0)));
    split.spawn((Transform::from_xyz(20.0, 0.0, 2.0), quad(Color::WHITE, 2.0)));
    let (image, scene) = render(&gpu, &mut renderer, &mut split, &assets);
    let (lx, ly, _) = sample(&scene.passes[0].view, Vec3::new(-20.0, 0.0, 1.0));
    expect_px(&image, W, lx, ly, [255, 0, 0, 255], 2, "the shaded viewport stays color");
    let (rx, ry, nearness) = sample(&scene.passes[1].view, Vec3::new(20.0, 0.0, 2.0));
    assert_nearness(&image, W, rx, ry, nearness, 6, "only the depth camera is visualized");
    assert_eq!(renderer.stats().depth_view_draws, 1);
}

#[test]
fn offscreen_depth_is_what_a_later_camera_samples() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut store = RenderTargetStore::new();
    let id = store.create(64, 64).unwrap();
    let mut world = scene_world();
    world.insert_resource(store);
    world.spawn((
        depth_cam(Camera::orthographic(8.0), 0.5, 20.0).with_output(CameraOutput::Target(id)),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((Transform::from_xyz(0.0, 0.0, 3.0), quad(Color::rgb(1.0, 0.2, 0.2), 3.0)));
    let (_, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let target = renderer.read_render_target(&gpu, id).unwrap();
    let (x, y, nearness) = sample(&scene.sole_pass().view, Vec3::new(0.0, 0.0, 3.0));
    assert_nearness(&target, 64, x, y, nearness, 6, "the offscreen target shows depth, not the sprite color");
    assert_empty(&target, 64, 2, 2, "offscreen background");
    assert_eq!(renderer.stats().depth_view_draws, 1);
    assert_eq!(renderer.stats().offscreen_passes, 1);

    world.spawn((
        Camera { order: 1, ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) },
        Transform::from_xyz(30.0, 0.0, -4.0),
    ));
    world.spawn((
        Transform::from_xyz(30.0, 0.0, 0.0),
        Sprite::new("quad")
            .with_size(Vec2::splat(4.0))
            .with_anchor(Vec2::splat(0.5))
            .with_render_target(id)
            .cutout()
            .unlit(),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(scene.passes.len(), 2);
    assert_eq!(renderer.stats().depth_view_draws, 1, "the surface camera stays shaded");
    let fresh = renderer.read_render_target(&gpu, id).unwrap();
    assert_nearness(&fresh, 64, x, y, nearness, 6, "the offscreen sprite is outside the sampling sprite");
    let (sx, sy, _) = sample(&scene.passes[1].view, Vec3::new(30.0, 0.0, 0.0));
    expect_px(&image, W, sx, sy, px(&fresh, 64, x, y), 2, "the surface sprite samples the depth image");
    assert_empty(&image, W, 4, 4, "the sampled image's empty corner is the surface corner");
}

#[test]
fn switching_back_to_shaded_restores_color_and_draw_counts() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut world = scene_world();
    let camera = world.spawn((
        Camera::orthographic(8.0).with_clear_color(Color::BLACK).with_presentation(CameraPresentation::Shaded),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((Transform::IDENTITY, quad(Color::rgb(1.0, 0.0, 0.0), 3.0)));
    let before = renderer.resource_stats().bind_group_creates;
    let (shaded, _) = render(&gpu, &mut renderer, &mut world, &assets);
    let shaded_stats = renderer.stats();
    let after_shaded = renderer.resource_stats().bind_group_creates;
    assert_eq!(shaded_stats.depth_view_draws, 0);
    assert_eq!(shaded_stats.world_draw_calls, 1);
    assert_eq!(shaded_stats.draw_calls, 1);
    expect_px(&shaded, W, W / 2, H / 2, [255, 0, 0, 255], 2, "shaded output is the sprite");

    world.component_mut::<Camera>(camera).unwrap().presentation = CameraPresentation::Depth;
    let (depth, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let depth_stats = renderer.stats();
    let (x, y, nearness) = sample(&scene.sole_pass().view, Vec3::ZERO);
    assert_nearness(&depth, W, x, y, nearness, 5, "depth presentation replaces the color");
    assert_eq!(depth_stats.depth_view_draws, 1);
    assert_eq!(depth_stats.world_draw_calls, shaded_stats.world_draw_calls);
    assert_eq!(depth_stats.draw_calls, shaded_stats.draw_calls + 1);
    assert_eq!(
        renderer.resource_stats().bind_group_creates - after_shaded,
        1,
        "the depth sample group is created on the first depth frame"
    );
    assert_eq!(after_shaded - before, 1, "a shaded frame creates the sprite group and not a depth group");

    world.component_mut::<Camera>(camera).unwrap().presentation = CameraPresentation::Shaded;
    let (again, _) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(again, shaded, "switching back restores the shaded pixels");
    assert_eq!(renderer.stats().depth_view_draws, 0);
    assert_eq!(renderer.stats().draw_calls, shaded_stats.draw_calls);
    assert_eq!(renderer.stats().world_draw_calls, shaded_stats.world_draw_calls);
    assert_eq!(renderer.resource_stats().bind_group_creates, after_shaded + 1);
}

#[test]
fn custom_reverse_z_linearizes_and_unsupported_matrices_do_not() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut sheared = Mat4::orthographic_lh(-4.0, 4.0, -4.0, 4.0, 0.5, 20.0);
    sheared.cols[2].x = 0.02;
    let mut world = scene_world();
    world.spawn((
        Camera::new(CameraProjection::custom(sheared).unwrap())
            .with_presentation(CameraPresentation::Depth)
            .with_clear_color(Color::BLACK),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    world.spawn((Transform::from_xyz(-1.2, 0.0, 2.0), quad(Color::WHITE, 1.2)));
    world.spawn((Transform::from_xyz(1.2, 0.0, 8.0), quad(Color::WHITE, 1.2)));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let view = &scene.sole_pass().view;
    let (nx, ny, nearness) = sample(view, Vec3::new(-1.2, 0.0, 2.0));
    let (fx, fy, farness) = sample(view, Vec3::new(1.2, 0.0, 8.0));
    assert!(nearness > farness);
    assert_nearness(&image, W, nx, ny, nearness, 8, "sheared reverse-Z near sprite");
    assert_nearness(&image, W, fx, fy, farness, 8, "sheared reverse-Z far sprite");

    let mut standard = Mat4::orthographic_lh(-4.0, 4.0, -4.0, 4.0, 0.5, 20.0);
    let span = 19.5;
    standard.cols[2].z = 1.0 / span;
    standard.cols[3].z = -0.5 / span;
    let mut flipped = scene_world();
    flipped.spawn((
        Camera::new(CameraProjection::custom(standard).unwrap())
            .with_presentation(CameraPresentation::Depth)
            .with_clear_color(Color::BLACK),
        Transform::from_xyz(0.0, 0.0, -4.0),
    ));
    flipped.spawn((Transform::IDENTITY, quad(Color::WHITE, 3.0)));
    let (image, _) = render(&gpu, &mut renderer, &mut flipped, &assets);
    assert_unsupported(&image, W, W / 2, H / 2, "a 0-near custom matrix is not drawn as a flipped gradient");
    assert_unsupported(&image, W, 2, 2, "an unsupported projection is not reported as an empty view");

    let zero = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::ZERO);
    let mut singular = scene_world();
    singular.spawn((
        Camera::new(CameraProjection::custom(zero).unwrap())
            .with_presentation(CameraPresentation::Depth)
            .with_clear_color(Color::BLACK),
        Transform::IDENTITY,
    ));
    singular.spawn((Transform::from_xyz(0.0, 0.0, 2.0), quad(Color::WHITE, 2.0)));
    let (image, _) = render(&gpu, &mut renderer, &mut singular, &assets);
    assert_unsupported(&image, W, W / 2, H / 2, "a singular projection is unsupported");
    assert_unsupported(&image, W, 3, 3, "a singular projection fills the viewport");
    assert_eq!(renderer.stats().depth_view_draws, 1);
}
