//! Orthographic pixel snap on Apple GPU pixels.
//!
//! Textures are linear `Rgba8Unorm`. A camera move that stays inside one rounding bin keeps the
//! image. Crossing the halfway pixel does not, even when the move is shorter than one pixel.

use engine_assets::{AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
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
    let got = px(img, width, x, y);
    for channel in 0..4 {
        let delta = i16::from(got[channel]) - i16::from(expected[channel]);
        assert!(delta.abs() <= tolerance, "{what} at ({x},{y}): got {got:?}, expected {expected:?}");
    }
}

fn color_bytes(color: [f32; 4]) -> [u8; 4] {
    color.map(|channel| (channel * 255.0).round() as u8)
}

fn pixel_of(view: &CameraView, point: Vec3) -> (u32, u32) {
    let screen = view.project(point).expect("point projects");
    let x = (screen.pixels.x + view.viewport_origin.x).floor() as u32;
    let y = (screen.pixels.y + view.viewport_origin.y).floor() as u32;
    (x, y)
}

fn render(
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

fn checker_assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let mut texels = vec![0u8; 4 * 4 * 4];
    for y in 0..4u32 {
        for x in 0..4u32 {
            let i = ((y * 4 + x) * 4) as usize;
            texels[i] = 30 + x as u8 * 50;
            texels[i + 1] = 20 + y as u8 * 40;
            texels[i + 2] = 210;
            texels[i + 3] = 255;
        }
    }
    let texture =
        assets.insert("checker-tex", Texture::from_rgba8(4, 4, texels).with_color_space(ColorSpace::Linear)).unwrap();
    assets.insert("checker", SpriteAsset::whole_texture(texture, 4, 4).with_pixels_per_unit(1.0)).unwrap();
    let white = assets
        .insert("white", Texture::solid(1, 1, [255, 255, 255, 255]).with_color_space(ColorSpace::Linear))
        .unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.2)).unwrap()).unwrap();
    let tiles = assets
        .insert(
            "tiles-tex",
            Texture::from_rgba8(2, 1, vec![220, 40, 40, 255, 40, 180, 60, 255]).with_color_space(ColorSpace::Linear),
        )
        .unwrap();
    let _ = tiles;
    assets
}

fn world() -> World {
    let mut world = World::new();
    world.insert_resource(AmbientLight { color: Color::WHITE, intensity: 1.0 });
    world
}

fn snap_camera(height: f32, snap: bool, eye: Vec3) -> (Camera, Transform) {
    (Camera::orthographic(height).with_pixel_snap(snap).with_clear_color(Color::BLACK), Transform::from_position(eye))
}

fn checker(anchor: Vec2) -> Sprite {
    Sprite::new("checker").with_anchor(anchor).cutout().unlit()
}

fn assert_shifted_left(original: &[u8], moved: &[u8], width: u32, height: u32) {
    for y in 0..height {
        for x in 0..width - 1 {
            assert_eq!(px(moved, width, x, y), px(original, width, x + 1, y), "one-pixel camera step at ({x},{y})");
        }
        assert_eq!(px(moved, width, width - 1, y), [0, 0, 0, 255], "the column that entered from the right is clear");
    }
}

#[test]
fn a_nearest_sprite_is_stable_inside_a_bin_and_jumps_on_the_boundary() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let mut scene = world();
    let (camera, pose) = snap_camera(32.0, true, Vec3::new(0.125, 0.0, -8.0));
    scene.spawn((camera, pose));
    scene.spawn((Transform::IDENTITY, checker(Vec2::ZERO)));

    let (low, low_scene) = render(&gpu, &mut renderer, &mut scene, &assets, 32, 32);
    *scene.query::<&mut Transform>().iter().find(|(_, transform)| transform.position.z < -1.0).unwrap().1 =
        Transform::from_xyz(0.375, 0.0, -8.0);
    let (high, _) = render(&gpu, &mut renderer, &mut scene, &assets, 32, 32);
    assert_eq!(low, high, "0.125 and 0.375 are the same pixel");

    *scene.query::<&mut Transform>().iter().find(|(_, transform)| transform.position.z < -1.0).unwrap().1 =
        Transform::from_xyz(0.6, 0.0, -8.0);
    *scene.query::<&mut Camera>().iter().next().unwrap().1 = Camera::orthographic(32.0).with_clear_color(Color::BLACK);
    let (unsnapped, _) = render(&gpu, &mut renderer, &mut scene, &assets, 32, 32);
    assert_ne!(unsnapped, high, "0.6 of a pixel is past a pixel center, so the unsnapped sprite moves");

    *scene.query::<&mut Camera>().iter().next().unwrap().1 =
        Camera::orthographic(32.0).with_pixel_snap(true).with_clear_color(Color::BLACK);
    *scene.query::<&mut Transform>().iter().find(|(_, transform)| transform.position.z < -1.0).unwrap().1 =
        Transform::from_xyz(0.5, 0.0, -8.0);
    let (halfway, halfway_scene) = render(&gpu, &mut renderer, &mut scene, &assets, 32, 32);
    assert_ne!(halfway, low, "0.5 rounds away from zero and leaves the bin");
    assert_shifted_left(&low, &halfway, 32, 32);

    let view = &low_scene.sole_pass().view;
    // Sprite V starts at the bottom of the quad, so world (0.5, 0.5) is texture row 3.
    let (x, y) = pixel_of(view, Vec3::new(0.5, 0.5, 0.0));
    expect_px(&low, 32, x, y, [30, 140, 210, 255], 2, "bottom-left texel");
    assert_eq!(renderer.stats().depth_view_draws, 0);
    assert_eq!(
        halfway_scene.sole_pass().opaque_sprites.batches.len() + halfway_scene.sole_pass().cutout_sprites.batches.len(),
        1
    );
    let _ = halfway_scene;
}

#[test]
fn adjacent_tiles_stay_continuous_when_the_camera_snaps() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let mut scene = world();
    scene.spawn(snap_camera(32.0, true, Vec3::new(0.4, 0.0, -8.0)));
    let mut map = TileMap::new("tiles-tex", 2, 1, Vec2::ONE, 2, 1);
    map.set(0, 0, 0);
    map.set(1, 0, 1);
    scene.spawn((Transform::IDENTITY, map));
    let (image, extracted) = render(&gpu, &mut renderer, &mut scene, &assets, 32, 32);
    let view = &extracted.sole_pass().view;
    let (left_x, left_y) = pixel_of(view, Vec3::new(0.5, 0.5, 0.0));
    let (right_x, right_y) = pixel_of(view, Vec3::new(1.5, 0.5, 0.0));
    assert_eq!(right_y, left_y);
    assert_eq!(right_x, left_x + 1, "the shared edge is one pixel, not a gap");
    expect_px(&image, 32, left_x, left_y, [220, 40, 40, 255], 2, "left tile");
    expect_px(&image, 32, right_x, right_y, [40, 180, 60, 255], 2, "right tile");

    *scene.query::<&mut Transform>().iter().find(|(_, transform)| transform.position.z < -1.0).unwrap().1 =
        Transform::from_xyz(0.1, 0.0, -8.0);
    let (again, _) = render(&gpu, &mut renderer, &mut scene, &assets, 32, 32);
    assert_eq!(image, again, "0.4 and 0.1 share a bin");
}

#[test]
fn a_snapped_camera_keeps_sprite_and_mesh_occlusion() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let pose = Transform::from_xyz(0.4, 0.0, -14.0);
    let camera = Camera::orthographic(18.0).with_pixel_snap(true).with_clear_color(Color::BLACK);
    let sprite = |color: Color, size: f32| {
        Sprite::new("quad").with_size(Vec2::splat(size)).with_anchor(Vec2::splat(0.5)).with_tint(color).cutout().unlit()
    };
    let mut front = world();
    front.spawn((camera, pose));
    front.spawn((Transform::from_xyz(0.0, 0.0, -2.0), sprite(Color::rgb(0.1, 0.35, 0.95), 1.6)));
    front.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.15, 0.75, 0.25))),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut front, &assets, 64, 64);
    let view = &scene.sole_pass().view;
    let (sx, sy) = pixel_of(view, Vec3::new(0.0, 0.0, -2.0));
    let sprite_px = px(&image, 64, sx, sy);
    assert!(sprite_px[2] > sprite_px[1] + 40, "the nearer sprite wins {sprite_px:?}");

    let mut behind = world();
    behind.spawn((camera, pose));
    behind.spawn((Transform::from_xyz(0.0, 0.0, 3.0), sprite(Color::rgb(0.85, 0.1, 0.1), 4.0)));
    behind.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.15, 0.75, 0.25))),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut behind, &assets, 64, 64);
    let view = &scene.sole_pass().view;
    let (mx, my) = pixel_of(view, Vec3::ZERO);
    let mesh_px = px(&image, 64, mx, my);
    assert!(mesh_px[1] > mesh_px[0] + 40, "the nearer mesh wins {mesh_px:?}");
    let draws = renderer.stats().world_draw_calls;
    assert!(draws >= 2, "sprite batch plus mesh, got {draws}");
    assert_eq!(renderer.stats().depth_view_draws, 0);
}

#[test]
fn fixed_upright_and_billboard_sprites_share_the_snapped_view() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let mut scene = world();
    scene.spawn(snap_camera(16.0, true, Vec3::new(0.3, 0.1, -10.0)));
    let quad = |orientation: SpriteOrientation, color: Color, at: Vec3| {
        (
            Transform::from_position(at),
            Sprite::new("quad")
                .with_size(Vec2::splat(1.5))
                .with_anchor(Vec2::splat(0.5))
                .with_orientation(orientation)
                .with_tint(color)
                .cutout()
                .unlit(),
        )
    };
    scene.spawn(quad(SpriteOrientation::Fixed, Color::rgb(0.9, 0.1, 0.1), Vec3::new(-2.2, 0.0, 0.0)));
    scene.spawn(quad(SpriteOrientation::Upright, Color::rgb(0.1, 0.8, 0.2), Vec3::ZERO));
    scene.spawn(quad(SpriteOrientation::Billboard, Color::rgb(0.15, 0.3, 0.95), Vec3::new(2.2, 0.0, 0.0)));
    let (image, extracted) = render(&gpu, &mut renderer, &mut scene, &assets, 64, 64);
    let view = &extracted.sole_pass().view;
    let sample = |point: Vec3, channel: usize, what: &str| {
        let (x, y) = pixel_of(view, point);
        let pixel = px(&image, 64, x, y);
        assert!(pixel[channel] > 140, "{what} {pixel:?}");
    };
    sample(Vec3::new(-2.2, 0.0, 0.0), 0, "fixed");
    sample(Vec3::ZERO, 1, "upright");
    sample(Vec3::new(2.2, 0.0, 0.0), 2, "billboard");
}

#[test]
fn isometric_top_down_and_side_cameras_snap_what_they_draw() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let quad = Sprite::new("quad")
        .with_size(Vec2::splat(2.0))
        .with_anchor(Vec2::splat(0.5))
        .with_tint(Color::rgb(0.9, 0.2, 0.15))
        .cutout()
        .unlit();
    let cases = [
        ("side", PlacedCamera::side(Vec3::ZERO, 12.0, 16.0).unwrap(), Transform::IDENTITY, Vec3::ZERO),
        (
            "top-down",
            PlacedCamera::top_down(Vec3::ZERO, 12.0, 16.0).unwrap(),
            Transform::from_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)),
            Vec3::ZERO,
        ),
        ("isometric", PlacedCamera::isometric(Vec3::ZERO, 14.0, 18.0).unwrap(), Transform::IDENTITY, Vec3::ZERO),
    ];
    for (name, mut placed, sprite_pose, point) in cases {
        placed.camera = placed.camera.with_pixel_snap(true).with_clear_color(Color::BLACK);
        let shifted = placed.transform.position + placed.transform.right() * 0.05;
        let mut scene = world();
        scene.spawn((placed.camera, Transform::from_position(shifted).with_rotation(placed.transform.rotation)));
        let mut sprite = quad;
        if name == "isometric" {
            sprite = sprite.with_orientation(SpriteOrientation::Billboard);
        }
        scene.spawn((sprite_pose, sprite));
        let (image, extracted) = render(&gpu, &mut renderer, &mut scene, &assets, 64, 64);
        let view = &extracted.sole_pass().view;
        let (x, y) = pixel_of(view, point);
        let pixel = px(&image, 64, x, y);
        assert!(pixel[0] > 140, "{name} sprite {pixel:?}");
        assert!(view.position.distance(shifted) > 1e-3, "{name} moved the fractional eye");
    }
}

#[test]
fn a_mixed_scene_snaps_sprites_meshes_and_tiles_together() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let mut scene = world();
    scene.spawn(snap_camera(20.0, true, Vec3::new(0.2, -0.15, -12.0)));
    scene.spawn((
        Transform::from_xyz(-2.0, 0.5, 0.0),
        Sprite::new("quad")
            .with_size(Vec2::splat(1.8))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(0.2, 0.45, 0.95))
            .cutout()
            .unlit(),
    ));
    scene.spawn((
        Transform::from_xyz(2.2, 0.4, 1.0),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.85, 0.75, 0.2))),
    ));
    let mut map = TileMap::new("tiles-tex", 2, 1, Vec2::splat(1.5), 1, 1);
    map.set(0, 0, 0);
    scene.spawn((Transform::from_xyz(-0.2, -2.0, 0.5), map));
    let (image, extracted) = render(&gpu, &mut renderer, &mut scene, &assets, 96, 96);
    let view = &extracted.sole_pass().view;
    let hit = |point: Vec3, channel: usize, what: &str| {
        let (x, y) = pixel_of(view, point);
        let pixel = px(&image, 96, x, y);
        assert!(pixel[channel] > 100, "{what} {pixel:?} at ({x},{y})");
    };
    hit(Vec3::new(-2.0, 0.5, 0.0), 2, "sprite");
    hit(Vec3::new(2.2, 0.4, 1.0), 0, "mesh");
    hit(Vec3::new(-0.2 + 0.75, -2.0 + 0.75, 0.5), 0, "tile");
    let draws = renderer.stats().world_draw_calls;
    *scene.query::<&mut Camera>().iter().next().unwrap().1 = Camera::orthographic(20.0).with_clear_color(Color::BLACK);
    let _ = render(&gpu, &mut renderer, &mut scene, &assets, 96, 96);
    assert_eq!(renderer.stats().world_draw_calls, draws, "snap adds no draw");
}

#[test]
fn snapped_lighting_and_blending_still_shade_the_sprite() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let mut lit = world();
    lit.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.0 });
    lit.spawn(snap_camera(8.0, true, Vec3::new(0.3, 0.0, -8.0)));
    lit.spawn((
        Transform::IDENTITY,
        Sprite::new("quad").with_size(Vec2::splat(2.0)).with_anchor(Vec2::splat(0.5)).cutout(),
    ));
    lit.spawn((Transform::from_xyz(0.0, 0.0, -3.0), Light::point(Color::rgb(1.0, 0.1, 0.1), 8.0, 30.0)));
    let (image, extracted) = render(&gpu, &mut renderer, &mut lit, &assets, 48, 48);
    let (x, y) = pixel_of(&extracted.sole_pass().view, Vec3::ZERO);
    let pixel = px(&image, 48, x, y);
    assert!(pixel[0] > pixel[2] + 30, "the red point light reaches the snapped sprite {pixel:?}");

    let mut blended = world();
    blended.spawn(snap_camera(8.0, true, Vec3::new(0.3, 0.0, -8.0)));
    blended.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("quad")
            .with_size(Vec2::splat(3.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(0.9, 0.05, 0.05))
            .cutout()
            .unlit(),
    ));
    let mut blend_assets = checker_assets();
    let glass = blend_assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(0.15, 0.35, 0.95, 0.55)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    blended.spawn((
        Transform::IDENTITY,
        Sprite::new("quad").with_size(Vec2::splat(1.4)).with_anchor(Vec2::splat(0.5)).with_material(glass),
    ));
    let (image, extracted) = render(&gpu, &mut renderer, &mut blended, &blend_assets, 48, 48);
    let (x, y) = pixel_of(&extracted.sole_pass().view, Vec3::ZERO);
    let pixel = px(&image, 48, x, y);
    assert!(pixel[2] > 40 && pixel[0] > 40, "blend over the red sprite {pixel:?}");
}

#[test]
fn an_offscreen_camera_snaps_to_the_target_pixel_grid() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let mut offscreen = world();
    offscreen.insert_resource(RenderTargetStore::new());
    let target = offscreen.resource_mut::<RenderTargetStore>().create(32, 32).unwrap();
    let (mut camera, pose) = snap_camera(32.0, true, Vec3::new(0.375, 0.0, -8.0));
    camera = camera.with_output(CameraOutput::Target(target));
    offscreen.spawn((camera, pose));
    offscreen.spawn((Transform::IDENTITY, checker(Vec2::ZERO)));
    let (_, _) = render(&gpu, &mut renderer, &mut offscreen, &assets, 80, 48);
    let image = renderer.read_render_target(&gpu, target).expect("offscreen image");

    let mut surface = world();
    surface.spawn(snap_camera(32.0, true, Vec3::new(0.375, 0.0, -8.0)));
    surface.spawn((Transform::IDENTITY, checker(Vec2::ZERO)));
    let (reference, _) = render(&gpu, &mut renderer, &mut surface, &assets, 32, 32);
    assert_eq!(image, reference, "the 32px target matches a 32px surface, not the 80px window");
}

#[test]
fn two_cameras_can_snap_independently() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    // 0.4 snaps to 0. An integer-aligned sprite stays in that coverage bin either way, so the
    // quad starts at x = 0.6 and the 0.4 correction crosses a pixel center.
    let pose = Transform::from_xyz(0.4, 0.0, -8.0);
    let mut scene = world();
    scene.spawn((
        Camera {
            order: 0,
            viewport: ViewportRect::new(0.0, 0.0, 0.5, 1.0),
            ..Camera::orthographic(32.0).with_clear_color(Color::BLACK)
        },
        pose,
    ));
    scene.spawn((
        Camera {
            order: 1,
            viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0),
            ..Camera::orthographic(32.0).with_pixel_snap(true).with_clear_color(Color::BLACK)
        },
        pose,
    ));
    scene.spawn((Transform::from_xyz(0.6, 0.0, 0.0), checker(Vec2::ZERO)));
    let (image, extracted) = render(&gpu, &mut renderer, &mut scene, &assets, 64, 32);
    assert!((extracted.passes[0].view.position.x - 0.4).abs() < 1e-3);
    assert!(extracted.passes[1].view.position.x.abs() < 1e-3);
    let left = half(&image, 64, 0);
    let right = half(&image, 64, 32);
    assert_ne!(left, right, "only the right viewport snaps");
    let (x, y) = pixel_of(&extracted.passes[1].view, Vec3::new(1.1, 0.5, 0.0));
    expect_px(&image, 64, x, y, [30, 140, 210, 255], 2, "snapped viewport texel");
}

fn half(image: &[u8], width: u32, origin_x: u32) -> Vec<u8> {
    let height = image.len() as u32 / (width * 4);
    let mut out = Vec::with_capacity((32 * height * 4) as usize);
    for y in 0..height {
        for x in 0..32 {
            out.extend_from_slice(&px(image, width, origin_x + x, y));
        }
    }
    out
}

#[test]
fn resizing_changes_the_pixel_size_the_snap_uses() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let mut scene = world();
    scene.spawn(snap_camera(32.0, true, Vec3::new(0.375, 0.0, -8.0)));
    scene.spawn((Transform::IDENTITY, checker(Vec2::ZERO)));
    let (small, _) = render(&gpu, &mut renderer, &mut scene, &assets, 32, 32);
    let (large, large_scene) = render(&gpu, &mut renderer, &mut scene, &assets, 64, 64);
    assert!((large_scene.sole_pass().view.position.x - 0.5).abs() < 1e-3, "64px is two pixels per unit");

    *scene.query::<&mut Transform>().iter().find(|(_, transform)| transform.position.z < -1.0).unwrap().1 =
        Transform::from_xyz(0.0, 0.0, -8.0);
    let (small_home, _) = render(&gpu, &mut renderer, &mut scene, &assets, 32, 32);
    assert_eq!(small, small_home, "0.375 snaps away on a 32px target");
    let (large_home, _) = render(&gpu, &mut renderer, &mut scene, &assets, 64, 64);
    assert_ne!(large, large_home, "the same eye snaps to a different pixel on a 64px target");
}

#[test]
fn perspective_and_custom_projections_are_unchanged() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let pose = Transform::from_xyz(0.4, -0.2, -8.0);
    let mut perspective = world();
    perspective.spawn((Camera::perspective(50.0).with_pixel_snap(true).with_clear_color(Color::BLACK), pose));
    perspective.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(0.2, 0.7, 0.3))
            .cutout()
            .unlit(),
    ));
    let (on, _) = render(&gpu, &mut renderer, &mut perspective, &assets, 48, 48);
    *perspective.query::<&mut Camera>().iter().next().unwrap().1 =
        Camera::perspective(50.0).with_clear_color(Color::BLACK);
    let (off, _) = render(&gpu, &mut renderer, &mut perspective, &assets, 48, 48);
    assert_eq!(on, off, "perspective ignores pixel_snap");
    assert!(on.iter().any(|byte| *byte != 0), "the perspective sprite is visible");

    let projection = CameraProjection::oblique_orthographic(16.0, 1.0, 0.2, 40.0, Vec2::new(0.18, 0.04)).unwrap();
    let mut custom = world();
    custom.spawn((Camera::new(projection).with_pixel_snap(true).with_clear_color(Color::BLACK), pose));
    custom.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::splat(2.0))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(0.85, 0.3, 0.1))
            .cutout()
            .unlit(),
    ));
    let (on, _) = render(&gpu, &mut renderer, &mut custom, &assets, 48, 48);
    *custom.query::<&mut Camera>().iter().next().unwrap().1 = Camera::new(projection).with_clear_color(Color::BLACK);
    let (off, _) = render(&gpu, &mut renderer, &mut custom, &assets, 48, 48);
    assert_eq!(on, off, "an oblique custom matrix ignores pixel_snap");
}

#[test]
fn depth_and_debug_lines_follow_the_snapped_camera() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = checker_assets();
    let mut shaded = world();
    shaded.spawn((
        Camera::orthographic(33.0).with_pixel_snap(true).with_clear_color(Color::BLACK),
        Transform::from_xyz(0.5, 0.0, -8.0),
    ));
    shaded.spawn((
        Transform::IDENTITY,
        Sprite::new("quad")
            .with_size(Vec2::ONE)
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(0.95, 0.9, 0.2))
            .cutout()
            .unlit(),
    ));
    shaded.insert_resource(DebugDraw::default());
    shaded.resource_mut::<DebugDraw>().line(
        Vec3::new(3.0, -4.0, 0.0),
        Vec3::new(3.0, 4.0, 0.0),
        Color::rgb(0.1, 0.95, 0.3),
    );
    let (image, extracted) = render(&gpu, &mut renderer, &mut shaded, &assets, 33, 33);
    let view = &extracted.sole_pass().view;
    assert!((view.position.x - 1.0).abs() < 1e-3, "0.5 rounds away from zero");
    let (sx, sy) = pixel_of(view, Vec3::ZERO);
    let sprite = px(&image, 33, sx, sy);
    assert!(sprite[0] > 180 && sprite[1] > 160, "snapped sprite pixel {sprite:?} at ({sx},{sy})");
    let (lx, ly) = pixel_of(view, Vec3::new(3.0, 0.0, 0.0));
    let line = px(&image, 33, lx, ly);
    assert!(line[1] > 180, "debug line uses the snapped view {line:?} at ({lx},{ly})");
    assert_eq!(renderer.stats().depth_view_draws, 0);

    *shaded.query::<&mut Camera>().iter().next().unwrap().1 = Camera::orthographic(33.0)
        .with_pixel_snap(true)
        .with_presentation(CameraPresentation::Depth)
        .with_clear_color(Color::BLACK);
    let (depth, depth_scene) = render(&gpu, &mut renderer, &mut shaded, &assets, 33, 33);
    let (dx, dy) = pixel_of(&depth_scene.sole_pass().view, Vec3::ZERO);
    assert_eq!((dx, dy), (sx, sy), "depth presentation uses the same snapped pixel");
    let written = px(&depth, 33, dx, dy);
    assert!(written[0] > 80 && written[0].abs_diff(written[1]) <= 3, "written depth is gray {written:?}");
    expect_px(&depth, 33, 1, 1, color_bytes(DEPTH_VIEW_EMPTY), 3, "unwritten depth stays empty");
    assert_eq!(renderer.stats().depth_view_draws, 1);
    assert!(renderer.stats().world_draw_calls >= 1);
}
