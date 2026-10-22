//! Behavioural tests for the spatial claims of the engine: projection-driven parallax, depth
//! ordering, culling, batching and hierarchy.

use core::f32::consts::FRAC_PI_2;

use engine_assets::{AssetManager, MeshData, SpriteAsset, Texture};
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_world::{propagate_transforms, Visibility, World};

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

fn assets() -> AssetManager {
    let mut a = AssetManager::in_memory();
    for name in ["tex_a", "tex_b"] {
        let t = a.insert(name, Texture::solid(16, 16, [255; 4])).unwrap();
        a.insert(&format!("{name}_sprite"), SpriteAsset::whole_texture(t, 16, 16)).unwrap();
    }
    a.insert("cube", MeshData::cuboid(Vec3::ONE).unwrap()).unwrap();
    a
}

fn camera_at(world: &mut World, camera: Camera, pos: Vec3) -> engine_world::Entity {
    world.spawn((camera, Transform::from_position(pos)))
}

fn project_x(camera: Camera, cam_pos: Vec3, point: Vec3) -> f32 {
    let mut w = World::new();
    camera_at(&mut w, camera, cam_pos);
    propagate_transforms(&mut w);
    let (_, (c, g)) =
        w.query::<(&Camera, &engine_world::GlobalTransform)>().iter().next().map(|(e, (c, g))| (e, (*c, *g))).unwrap();
    CameraView::new(&c, &g, VIEWPORT).project(point).unwrap().pixels.x
}

fn shift_when_camera_moves(camera: Camera, z: f32) -> f32 {
    let p = Vec3::new(0.0, 0.0, z);
    (project_x(camera, Vec3::new(10.0, 0.0, -5.0), p) - project_x(camera, Vec3::new(0.0, 0.0, -5.0), p)).abs()
}

#[test]
fn parallax_emerges_from_perspective_depth_alone() {
    let cam = Camera::perspective(60.0);
    let shifts: Vec<f32> = [0.0, 100.0, 300.0, 800.0].iter().map(|&z| shift_when_camera_moves(cam, z)).collect();
    // Strictly decreasing: farther objects move less on screen.
    assert!(shifts.windows(2).all(|w| w[0] > w[1]), "{shifts:?}");
    // Shift is inversely proportional to distance from the camera.
    let (near, far) = (shifts[1], shifts[3]);
    assert!((near / far - 805.0 / 105.0).abs() < 0.05, "{near} / {far}");
}

#[test]
fn orthographic_has_no_parallax() {
    let cam = Camera::orthographic(10.0);
    let shifts: Vec<f32> = [0.0, 100.0, 800.0].iter().map(|&z| shift_when_camera_moves(cam, z)).collect();
    assert!(shifts.iter().all(|s| (s - shifts[0]).abs() < 1e-3), "{shifts:?}");
}

#[test]
fn projection_trait_agrees_with_camera_view() {
    let cam = Camera::perspective(70.0);
    let mut w = World::new();
    camera_at(&mut w, cam, Vec3::new(1.0, 2.0, -8.0));
    propagate_transforms(&mut w);
    let g = *w.query::<&engine_world::GlobalTransform>().iter().next().unwrap().1;
    let view = CameraView::new(&cam, &g, VIEWPORT);
    let p = Vec3::new(3.0, -1.0, 20.0);
    assert_eq!(cam.projection.project(p, &view).unwrap().pixels, view.project(p).unwrap().pixels);
    assert!(view.project(Vec3::new(0.0, 0.0, -50.0)).is_none(), "behind the camera");
}

#[test]
fn screen_rays_roundtrip_through_projection() {
    for cam in [Camera::perspective(60.0), Camera::orthographic(12.0)] {
        let mut w = World::new();
        w.spawn((cam, Transform::looking_at(Vec3::new(0.0, 6.0, -12.0), Vec3::new(0.0, 0.0, 4.0), Vec3::Y)));
        propagate_transforms(&mut w);
        let g = *w.query::<&engine_world::GlobalTransform>().iter().next().unwrap().1;
        let view = CameraView::new(&cam, &g, VIEWPORT);
        let target = Vec3::new(2.0, 0.0, 10.0);
        let screen = view.project(target).unwrap().pixels;
        let (origin, dir) = view.ray_from_pixels(screen).unwrap();
        let t = (target - origin).dot(dir);
        assert!((origin + dir * t).approx_eq(target, 1e-2), "{cam:?}");
    }
}

fn scene_world() -> World {
    let mut w = World::new();
    camera_at(&mut w, Camera::perspective(60.0), Vec3::new(0.0, 0.0, -10.0));
    w
}

fn extract(w: &mut World, a: &AssetManager) -> RenderScene {
    propagate_transforms(w);
    extract_scene(w, a, VIEWPORT).expect("camera present")
}

#[test]
fn blended_sprites_sort_by_real_depth_not_spawn_order() {
    let a = assets();
    let mut w = scene_world();
    // Spawn near-to-far; draw order must be far-to-near regardless.
    for z in [5.0, 50.0, 20.0] {
        w.spawn((Transform::from_xyz(0.0, 0.0, z), Sprite::new("tex_a_sprite")));
    }
    let scene = extract(&mut w, &a);
    let depths: Vec<f32> =
        scene.sole_pass().blended_sprites.instances.iter().map(|i| i.model.translation().z).collect();
    assert_eq!(depths, vec![50.0, 20.0, 5.0]);
}

#[test]
fn explicit_layer_overrides_depth_and_order_breaks_ties() {
    let a = assets();
    let mut w = scene_world();
    w.spawn((Transform::from_xyz(0.0, 0.0, 5.0), Sprite::new("tex_a_sprite"), RenderOrder::new(1, 0)));
    w.spawn((Transform::from_xyz(0.0, 0.0, 50.0), Sprite::new("tex_a_sprite")));
    w.spawn((Transform::from_xyz(1.0, 0.0, 50.0), Sprite::new("tex_a_sprite"), RenderOrder::new(0, 3)));
    w.spawn((Transform::from_xyz(2.0, 0.0, 50.0), Sprite::new("tex_a_sprite"), RenderOrder::new(0, -3)));
    let xs: Vec<(f32, f32)> = extract(&mut w, &a)
        .sole_pass()
        .blended_sprites
        .instances
        .iter()
        .map(|i| {
            let origin = i.model.transform_point3(Vec3::new(0.5, 0.0, 0.0));
            (origin.x, origin.z)
        })
        .collect();
    // Layer 0 first (equal depth: order -3, 0, 3), then layer 1 last even though it is nearer.
    assert_eq!(xs, vec![(2.0, 50.0), (0.0, 50.0), (1.0, 50.0), (0.0, 5.0)]);
}

#[test]
fn sprites_batch_by_texture_without_reordering() {
    let a = assets();
    let mut w = scene_world();
    for (z, tex) in [(40.0, "tex_a_sprite"), (30.0, "tex_a_sprite"), (20.0, "tex_b_sprite"), (10.0, "tex_a_sprite")] {
        w.spawn((Transform::from_xyz(0.0, 0.0, z), Sprite::new(tex)));
    }
    let scene = extract(&mut w, &a);
    let ranges: Vec<_> = scene.sole_pass().blended_sprites.batches.iter().map(|b| b.range.clone()).collect();
    assert_eq!(ranges, vec![0..2, 2..3, 3..4], "interleaved textures must not be merged across other sprites");

    // A thousand sprites on one texture are a single draw.
    let mut w = scene_world();
    for i in 0..1000 {
        w.spawn((
            Transform::from_xyz((i % 40) as f32 * 0.1, 0.0, 20.0 + (i / 40) as f32),
            Sprite::new("tex_a_sprite").cutout(),
        ));
    }
    let scene = extract(&mut w, &a);
    assert_eq!(scene.sole_pass().cutout_sprites.instances.len(), 1000);
    assert_eq!(scene.sole_pass().cutout_sprites.batches.len(), 1);
}

#[test]
fn culling_and_visibility_and_missing_assets() {
    let a = assets();
    let mut w = scene_world();
    w.spawn((Transform::from_xyz(0.0, 0.0, 20.0), Sprite::new("tex_a_sprite")));
    w.spawn((Transform::from_xyz(500.0, 0.0, 20.0), Sprite::new("tex_a_sprite")));
    w.spawn((Transform::from_xyz(0.0, 0.0, -100.0), Sprite::new("tex_a_sprite")));
    w.spawn((Transform::from_xyz(0.0, 0.0, 22.0), Sprite::new("tex_a_sprite"), Visibility::Hidden));
    w.spawn((Transform::from_xyz(0.0, 0.0, 24.0), Sprite::new("nonexistent")));
    w.spawn((Transform::from_xyz(0.0, 0.0, 30.0), Mesh::new("cube")));
    w.spawn((Transform::from_xyz(0.0, 900.0, 30.0), Mesh::new("cube")));
    let s = extract(&mut w, &a).stats;
    assert_eq!((s.sprites_visible, s.sprites_culled, s.missing_assets), (1, 2, 1));
    assert_eq!((s.meshes_visible, s.meshes_culled), (1, 1));
}

#[test]
fn sprites_and_meshes_share_one_spatial_world() {
    let a = assets();
    let mut w = scene_world();
    w.spawn((Transform::from_xyz(1.0, 2.0, 30.0), Mesh::new("cube")));
    w.spawn((Transform::from_xyz(1.0, 2.0, 30.0), Sprite::new("tex_a_sprite")));
    let scene = extract(&mut w, &a);
    assert_eq!(scene.sole_pass().meshes[0].model.translation(), Vec3::new(1.0, 2.0, 30.0));
    // Sprite pivot is bottom-center by default, so its origin lands at the entity position.
    let origin = scene.sole_pass().blended_sprites.instances[0].model.transform_point3(Vec3::new(0.5, 0.0, 0.0));
    assert!(origin.approx_eq(Vec3::new(1.0, 2.0, 30.0), 1e-5));
}

#[test]
fn children_inherit_transforms_into_the_scene() {
    let a = assets();
    let mut w = scene_world();
    let root = w.spawn((Transform::from_xyz(0.0, 0.0, 10.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2)),));
    w.spawn_child(root, (Transform::from_xyz(2.0, 0.0, 0.0), Sprite::new("tex_a_sprite")));
    let scene = extract(&mut w, &a);
    let pos = scene.sole_pass().blended_sprites.instances[0].model.transform_point3(Vec3::new(0.5, 0.0, 0.0));
    // Local +X after a +90 degree yaw points along world -Z.
    assert!(pos.approx_eq(Vec3::new(0.0, 0.0, 8.0), 1e-4), "{pos:?}");
}

#[test]
fn tilemap_expands_to_culled_cutout_instances() {
    let a = assets();
    let mut w = scene_world();
    let map =
        TileMap::new("tex_a", 1, 1, Vec2::ONE, 8, 8)
            .filled_with(|x, y| if (x + y) % 2 == 0 { 0 } else { TileMap::EMPTY });
    // Lay it on the ground (rotate so local +Y points deeper into the scene).
    w.spawn((Transform::from_xyz(-4.0, -3.0, 5.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)), map));
    let scene = extract(&mut w, &a);
    assert_eq!(scene.sole_pass().cutout_sprites.instances.len(), 32);
    assert_eq!(scene.sole_pass().cutout_sprites.batches.len(), 1);
    assert!(scene.sole_pass().blended_sprites.instances.is_empty());
}

#[test]
fn lights_are_extracted_in_world_space_and_capped() {
    let a = assets();
    let mut w = scene_world();
    w.spawn((
        Transform::looking_at(Vec3::ZERO, Vec3::new(0.0, -1.0, 1.0), Vec3::Y),
        Light::directional(engine_core::Color::WHITE, 2.0),
    ));
    for i in 0..40 {
        w.spawn((Transform::from_xyz(i as f32, 0.0, 10.0), Light::point(engine_core::Color::WHITE, 1.0, 5.0)));
    }
    let scene = extract(&mut w, &a);
    assert_eq!(scene.sole_pass().lights.len(), MAX_LIGHTS);
    let sun = scene
        .sole_pass()
        .lights
        .iter()
        .find(|l| l.kind == LightKind::Directional)
        .expect("directional light survives the cap");
    assert!(sun.direction.approx_eq(Vec3::new(0.0, -1.0, 1.0).normalized(), 1e-5));
    assert_eq!(sun.color, [2.0, 2.0, 2.0]);
}

#[test]
fn no_active_camera_means_no_scene() {
    let a = assets();
    let mut w = World::new();
    w.spawn((Transform::IDENTITY, Camera { active: false, ..Camera::perspective(60.0) }));
    propagate_transforms(&mut w);
    assert!(extract_scene(&w, &a, VIEWPORT).is_none());
}
