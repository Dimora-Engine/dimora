//! Public camera presets and the oblique builder on the existing renderer.
//!
//! These scenes are small. The `spatial_scene` configuration tests are the check that the same
//! constructors reproduce that example. Pixels are `Rgba8Unorm`.

use engine_assets::{AlphaMode, AssetManager, ColorSpace, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_renderer_wgpu::{GpuContext, SceneRenderer};
use engine_world::{propagate_transforms, World};

const W: u32 = 128;

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

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let white = assets
        .insert("white", Texture::solid(1, 1, [255, 255, 255, 255]).with_color_space(ColorSpace::Linear))
        .unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(white, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::splat(1.2)).unwrap()).unwrap();
    assets
        .insert("glass", MaterialAsset::unlit(Color::rgba(0.2, 0.8, 1.0, 0.4)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
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
    render_at(gpu, renderer, world, assets, W, W)
}

fn quad(color: Color, size: f32) -> Sprite {
    Sprite::new("quad").with_size(Vec2::splat(size)).with_anchor(Vec2::splat(0.5)).with_tint(color).cutout().unlit()
}

fn view_point(transform: &Transform, x: f32, depth: f32) -> Vec3 {
    transform.position + transform.right() * x + transform.forward() * depth
}

fn sample(view: &CameraView, point: Vec3, img: &[u8], width: u32) -> [u8; 4] {
    let screen = view.project(point).unwrap_or_else(|| panic!("point {point:?} does not project"));
    let height = img.len() as u32 / (width * 4);
    let x = (screen.pixels.x + view.viewport_origin.x).round().clamp(0.0, (width - 1) as f32) as u32;
    let y = (screen.pixels.y + view.viewport_origin.y).round().clamp(0.0, (height - 1) as f32) as u32;
    px(img, width, x, y)
}

fn place(mut placed: PlacedCamera) -> (Camera, Transform) {
    placed = placed.with_clip_range(0.4, 40.0);
    placed.camera = placed.camera.with_clear_color(Color::BLACK);
    (placed.camera, placed.transform)
}

fn occlusion(
    name: &str,
    camera: Camera,
    pose: Transform,
    gpu: &GpuContext,
    renderer: &mut SceneRenderer,
    assets: &AssetManager,
) {
    let near_at = view_point(&pose, -1.6, 4.0);
    let mesh_at = view_point(&pose, 1.3, 7.0);
    let far_at = view_point(&pose, 0.0, 12.0);
    let mut world = scene_world();
    world.spawn((camera, pose));
    world.spawn((Transform::from_position(far_at), quad(Color::rgb(0.85, 0.1, 0.1), 5.0)));
    world.spawn((Transform::from_position(near_at), quad(Color::rgb(0.1, 0.35, 0.95), 1.4)));
    world.spawn((
        Transform::from_position(mesh_at),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.15, 0.75, 0.25))),
    ));
    let (image, scene) = render(gpu, renderer, &mut world, assets);
    let view = &scene.sole_pass().view;
    let near = sample(view, near_at, &image, W);
    let far = sample(view, far_at, &image, W);
    let mesh = sample(view, mesh_at, &image, W);
    assert!(near[2] > near[0] + 40, "{name} near sprite is blue {near:?}");
    assert!(far[0] > far[2] + 40, "{name} far sprite is red {far:?}");
    assert!(mesh[1] > mesh[0] + 40 && mesh[1] > mesh[2] + 20, "{name} mesh is in front of the far sprite {mesh:?}");
    assert_eq!(renderer.stats().depth_view_draws, 0, "{name}");
    assert!(renderer.stats().world_draw_calls >= 2, "{name} keeps a sprite batch and a mesh draw");
}

#[test]
fn perspective_orthographic_and_the_placed_views_occlude_sprites_and_meshes() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let cases = [
        ("perspective", place(PlacedCamera::perspective(Vec3::new(0.0, 1.0, -14.0), 0.0, 0.15, 50.0).unwrap())),
        ("orthographic", place(PlacedCamera::orthographic(Vec3::new(0.0, 1.0, -14.0), 0.0, 0.15, 18.0).unwrap())),
        ("isometric", place(PlacedCamera::isometric(Vec3::ZERO, 18.0, 20.0).unwrap())),
        ("side", place(PlacedCamera::side(Vec3::ZERO, 14.0, 18.0).unwrap())),
        ("top-down", place(PlacedCamera::top_down(Vec3::ZERO, 16.0, 20.0).unwrap())),
        ("steep", place(PlacedCamera::steep(Vec3::ZERO, 16.0, 50.0).unwrap())),
    ];
    for (name, (camera, pose)) in cases {
        occlusion(name, camera, pose, &gpu, &mut renderer, &assets);
    }
}

#[test]
fn an_oblique_projection_shows_sprites_and_meshes_and_its_depth() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let projection = CameraProjection::oblique_orthographic(18.0, 1.0, 0.4, 40.0, Vec2::new(0.2, 0.05)).unwrap();
    let posed = PlacedCamera::at(Vec3::new(0.0, 0.5, -14.0), 0.0, 0.1, projection).unwrap();
    let (camera, pose) = place(posed);
    // Shear moves a deeper point farther in view x, so these centers stay apart after the shear.
    let near_at = view_point(&pose, -3.0, 4.0);
    let mesh_at = view_point(&pose, 2.8, 7.0);
    let far_at = view_point(&pose, 0.0, 12.0);
    let mut world = scene_world();
    world.spawn((camera, pose));
    world.spawn((Transform::from_position(far_at), quad(Color::rgb(0.85, 0.1, 0.1), 1.6)));
    world.spawn((Transform::from_position(near_at), quad(Color::rgb(0.1, 0.35, 0.95), 1.2)));
    world.spawn((
        Transform::from_position(mesh_at),
        Mesh::new("cube").with_material(Material::unlit(Color::rgb(0.15, 0.75, 0.25))),
    ));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let view = &scene.sole_pass().view;
    let near = sample(view, near_at, &image, W);
    let far = sample(view, far_at, &image, W);
    let mesh = sample(view, mesh_at, &image, W);
    assert!(near[2] > near[0] + 40, "oblique near sprite is blue {near:?}");
    assert!(far[0] > far[2] + 40, "oblique far sprite is red {far:?}");
    assert!(mesh[1] > mesh[0] + 40, "oblique mesh is green {mesh:?}");

    let mut presented = posed.with_presentation(CameraPresentation::Depth);
    presented.camera = presented.camera.with_clear_color(Color::BLACK);
    let mut depth = scene_world();
    depth.spawn((presented.camera, presented.transform));
    let near_at = view_point(&presented.transform, -1.2, 4.0);
    let far_at = view_point(&presented.transform, 1.2, 12.0);
    depth.spawn((Transform::from_position(near_at), quad(Color::WHITE, 1.5)));
    depth.spawn((Transform::from_position(far_at), quad(Color::WHITE, 1.5)));
    let (image, scene) = render(&gpu, &mut renderer, &mut depth, &assets);
    let view = &scene.sole_pass().view;
    let near = sample(view, near_at, &image, W);
    let far = sample(view, far_at, &image, W);
    assert!(near[0] > far[0] + 20, "oblique depth is brighter nearby: {near:?} vs {far:?}");
    assert_eq!(renderer.stats().depth_view_draws, 1);
    expect_px(&image, W, 2, 2, [89, 5, 115, 255], 8, "oblique background is the empty depth color");
}

#[test]
fn isometric_depth_shows_coplanar_bias_and_ignores_blend_and_debug_lines() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let (camera, pose) =
        place(PlacedCamera::isometric(Vec3::ZERO, 16.0, 18.0).unwrap().with_presentation(CameraPresentation::Depth));
    let mut world = scene_world();
    world.insert_resource(CoplanarSettings::new(0.4).unwrap());
    world.spawn((camera, pose));
    let low = view_point(&pose, -1.5, 6.0);
    let high = view_point(&pose, 1.5, 6.0);
    world.spawn((Transform::from_position(low), quad(Color::WHITE, 1.3), CoplanarPriority::new(0).unwrap()));
    world.spawn((Transform::from_position(high), quad(Color::WHITE, 1.3), CoplanarPriority::new(7).unwrap()));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let view = &scene.sole_pass().view;
    let dim = sample(view, low, &image, W);
    let bright = sample(view, high, &image, W);
    assert!(bright[0] > dim[0] + 15, "coplanar level 7 is stored nearer: {bright:?} vs {dim:?}");

    let behind = view_point(&pose, 0.0, 9.0);
    let front = view_point(&pose, 0.0, 4.0);
    let mut alone = scene_world();
    alone.spawn((camera, pose));
    alone.spawn((Transform::from_position(behind), quad(Color::WHITE, 3.0)));
    let (only, scene) = render(&gpu, &mut renderer, &mut alone, &assets);
    let expected = sample(&scene.sole_pass().view, behind, &only, W);

    let mut blended = scene_world();
    blended.spawn((camera, pose));
    blended.spawn((Transform::from_position(behind), quad(Color::WHITE, 3.0)));
    blended.spawn((
        Transform::from_position(front),
        Sprite::new("quad")
            .with_size(Vec2::splat(2.2))
            .with_anchor(Vec2::splat(0.5))
            .with_material(assets.handle("glass")),
    ));
    let (with_blend, scene) = render(&gpu, &mut renderer, &mut blended, &assets);
    let got = sample(&scene.sole_pass().view, behind, &with_blend, W);
    assert!(
        (i16::from(got[0]) - i16::from(expected[0])).abs() <= 2,
        "blend does not write depth: {got:?} vs {expected:?}"
    );

    let mut lines = scene_world();
    lines.insert_resource(DebugDraw::default());
    lines.spawn((camera, pose));
    lines.spawn((Transform::from_position(behind), quad(Color::WHITE, 3.0)));
    lines.resource_mut::<DebugDraw>().line(
        view_point(&pose, -4.0, 3.0),
        view_point(&pose, 4.0, 3.0),
        Color::rgb(0.0, 1.0, 0.0),
    );
    let (image, scene) = render(&gpu, &mut renderer, &mut lines, &assets);
    let got = sample(&scene.sole_pass().view, behind, &image, W);
    assert!(
        (i16::from(got[0]) - i16::from(expected[0])).abs() <= 2,
        "a debug line does not change stored depth: {got:?} vs {expected:?}"
    );
}

#[test]
fn two_presets_and_an_offscreen_target_share_one_world() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let mut store = RenderTargetStore::new();
    let id = store.create(64, 64).unwrap();
    let mut world = scene_world();
    world.insert_resource(store);

    let (mut off_cam, off_pose) = place(PlacedCamera::orthographic(Vec3::new(40.0, 0.0, -8.0), 0.0, 0.0, 8.0).unwrap());
    off_cam = off_cam.with_output(CameraOutput::Target(id));
    world.spawn((off_cam, off_pose));
    world.spawn((Transform::from_xyz(40.0, 0.0, 0.0), quad(Color::rgb(0.1, 0.9, 0.2), 2.0)));

    let (left_cam, left_pose) = place(PlacedCamera::side(Vec3::new(-16.0, 0.0, 0.0), 10.0, 14.0).unwrap());
    let mut left_cam = left_cam.with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0));
    left_cam.order = 1;
    let (right_cam, right_pose) =
        place(PlacedCamera::perspective(Vec3::new(16.0, 1.5, -10.0), 0.0, 0.15, 50.0).unwrap());
    let mut right_cam = right_cam.with_viewport(ViewportRect::new(0.5, 0.0, 0.5, 1.0));
    right_cam.order = 2;
    world.spawn((left_cam, left_pose));
    world.spawn((right_cam, right_pose));
    let red_at = view_point(&left_pose, -2.2, 4.0);
    let sample_at = view_point(&left_pose, 2.2, 5.0);
    let blue_at = view_point(&right_pose, 0.0, 6.0);
    world.spawn((Transform::from_position(red_at), quad(Color::rgb(1.0, 0.0, 0.0), 1.6)));
    world.spawn((Transform::from_position(blue_at), quad(Color::rgb(0.0, 0.35, 1.0), 1.6)));
    world.spawn((
        Transform::from_position(sample_at),
        Sprite::new("quad").with_size(Vec2::splat(1.6)).with_anchor(Vec2::splat(0.5)).with_render_target(id).unlit(),
    ));

    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    assert_eq!(scene.passes.len(), 3);
    let left = sample(&scene.passes[1].view, red_at, &image, W);
    let sampled = sample(&scene.passes[1].view, sample_at, &image, W);
    let right = sample(&scene.passes[2].view, blue_at, &image, W);
    assert!(left[0] > 200 && left[1] < 40, "left side preset shows its red sprite {left:?}");
    assert!(sampled[1] > 180, "the surface sprite samples the offscreen preset {sampled:?}");
    assert!(right[2] > 180, "right perspective preset shows its blue sprite {right:?}");
    let target = renderer.read_render_target(&gpu, id).unwrap();
    let off = sample(&scene.passes[0].view, Vec3::new(40.0, 0.0, 0.0), &target, 64);
    assert!(off[1] > 180, "offscreen orthographic preset shows the green sprite {off:?}");
    assert_eq!(renderer.stats().offscreen_passes, 1);
    assert_eq!(renderer.stats().depth_view_draws, 0);
}

#[test]
fn a_preset_matches_a_hand_built_camera_and_resize_follows_the_projection_kind() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let placed =
        PlacedCamera::perspective(Vec3::new(0.0, 0.0, -8.0), 0.0, 0.0, 50.0).unwrap().with_clip_range(0.4, 40.0);
    let mut preset_world = scene_world();
    preset_world.spawn((placed.camera.with_clear_color(Color::BLACK), placed.transform));
    preset_world.spawn((Transform::from_xyz(-1.1, 0.0, 0.0), quad(Color::rgb(1.0, 0.15, 0.1), 1.2)));
    preset_world.spawn((Transform::from_xyz(1.2, 0.0, 2.5), quad(Color::rgb(0.1, 0.3, 1.0), 1.2)));
    let (preset_image, _) = render(&gpu, &mut renderer, &mut preset_world, &assets);
    let preset_stats = renderer.stats();

    let mut built = scene_world();
    let mut camera = Camera::perspective(50.0).with_clear_color(Color::BLACK);
    camera.projection = camera.projection.with_clip_range(0.4, 40.0);
    built.spawn((camera, placed.transform));
    built.spawn((Transform::from_xyz(-1.1, 0.0, 0.0), quad(Color::rgb(1.0, 0.15, 0.1), 1.2)));
    built.spawn((Transform::from_xyz(1.2, 0.0, 2.5), quad(Color::rgb(0.1, 0.3, 1.0), 1.2)));
    let (built_image, _) = render(&gpu, &mut renderer, &mut built, &assets);
    assert_eq!(preset_image, built_image);
    assert_eq!(renderer.stats().draw_calls, preset_stats.draw_calls);
    assert_eq!(renderer.stats().world_draw_calls, preset_stats.world_draw_calls);
    assert_eq!(renderer.stats().depth_view_draws, 0);

    let point = Vec3::new(-1.1, 0.0, 0.0);
    let (_, wide) = render_at(&gpu, &mut renderer, &mut preset_world, &assets, 160, 90);
    let (_, square) = render_at(&gpu, &mut renderer, &mut preset_world, &assets, 96, 96);
    let wide_x = wide.sole_pass().view.project(point).unwrap().ndc.x;
    let square_x = square.sole_pass().view.project(point).unwrap().ndc.x;
    assert!((wide_x - square_x).abs() > 0.05, "perspective aspect follows the viewport: {wide_x} vs {square_x}");

    let oblique = CameraProjection::oblique_orthographic(14.0, 16.0 / 9.0, 0.4, 40.0, Vec2::new(0.18, 0.0)).unwrap();
    let matrix = oblique.matrix(1.0);
    assert!(oblique.matrix(0.5).approx_eq(&matrix, 0.0), "the oblique matrix is not rebuilt for a new aspect");
    let posed = PlacedCamera::at(Vec3::new(0.0, 0.0, -8.0), 0.0, 0.0, oblique).unwrap();
    let mut oblique_world = scene_world();
    oblique_world.spawn((posed.camera.with_clear_color(Color::BLACK), posed.transform));
    oblique_world.spawn((Transform::from_position(point), quad(Color::rgb(1.0, 0.15, 0.1), 1.4)));
    let (_, wide) = render_at(&gpu, &mut renderer, &mut oblique_world, &assets, 160, 90);
    let (_, square) = render_at(&gpu, &mut renderer, &mut oblique_world, &assets, 96, 96);
    let wide_x = wide.sole_pass().view.project(point).unwrap().ndc.x;
    let square_x = square.sole_pass().view.project(point).unwrap().ndc.x;
    assert!((wide_x - square_x).abs() < 1.0e-4, "oblique NDC stays on the baked aspect: {wide_x} vs {square_x}");
    assert!(renderer.stats().world_draw_calls >= 1);
}

#[test]
fn a_side_preset_draws_a_tilemap_and_lighting_still_reaches_a_mesh() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = renderer(&gpu);
    let assets = assets();
    let (camera, pose) = place(PlacedCamera::side(Vec3::ZERO, 8.0, 10.0).unwrap());
    let mut world = scene_world();
    world.spawn((camera, pose));
    let mut map = TileMap::new("white", 1, 1, Vec2::splat(1.2), 2, 2).filled_with(|_, _| 0);
    map.lit = false;
    world.spawn((Transform::from_xyz(-1.6, 0.0, 0.0), map));
    world.spawn((Transform::from_xyz(1.8, 0.0, 0.0), Mesh::new("cube").with_material(Material::unlit(Color::WHITE))));
    let (image, scene) = render(&gpu, &mut renderer, &mut world, &assets);
    let tile = sample(&scene.sole_pass().view, Vec3::new(-1.0, 0.6, 0.0), &image, W);
    let unlit = sample(&scene.sole_pass().view, Vec3::new(1.8, 0.0, 0.0), &image, W);
    assert!(tile[0] > 200 && tile[1] > 200, "the tilemap keeps its world placement {tile:?}");
    assert!(unlit[0] > 200, "unlit mesh stays bright {unlit:?}");

    let mut lit = World::new();
    lit.insert_resource(AmbientLight { color: Color::WHITE, intensity: 0.15 });
    lit.spawn((camera, pose));
    lit.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Mesh::new("cube").with_material(Material::color(Color::WHITE))));
    let (image, scene) = render(&gpu, &mut renderer, &mut lit, &assets);
    let shaded = sample(&scene.sole_pass().view, Vec3::ZERO, &image, W);
    assert!(shaded[0] < 120, "a lit mesh still responds to ambient light under the preset {shaded:?}");
    assert_eq!(renderer.stats().depth_view_draws, 0);
}
