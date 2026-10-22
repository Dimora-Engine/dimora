//! Several active cameras in one extraction.
//!
//! Ordering, culling, billboards, transparency, and picking all go through the production
//! functions. There is no second camera sort in this file.

use engine_assets::{AssetManager, MaterialAsset, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat4, Quat, Transform, Vec2, Vec3, Vec4};
use engine_renderer::{
    active_surface_cameras, camera_view_of, extract_scene, top_surface_camera, Camera, CameraClear, CameraProjection,
    RenderOrder, RenderScene, Sprite, SpriteBlend, SpriteOrientation, ViewportRect,
};
use engine_world::{propagate_transforms, Visibility, World};

const SIZE: Vec2 = Vec2::new(200.0, 100.0);

fn prepare(world: &mut World) {
    propagate_transforms(world);
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    prepare(world);
    extract_scene(world, assets, SIZE).expect("cameras")
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("tex", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    assets
}

fn sprite(tint: Color) -> Sprite {
    Sprite::new("sprite")
        .with_size(Vec2::splat(0.5))
        .with_anchor(Vec2::splat(0.5))
        .with_tint(tint)
        .with_material(engine_assets::AssetHandle::from_name("flat"))
}

fn cam(order: i32, viewport: ViewportRect) -> Camera {
    Camera { order, viewport, ..Camera::orthographic(4.0).with_clear_color(Color::BLACK) }
}

fn pose(position: Vec3) -> Transform {
    Transform::from_xyz(position.x, position.y, position.z)
}

fn on_ray(point: Vec3, origin: Vec3, direction: Vec3) -> bool {
    let along = (point - origin).dot(direction);
    let closest = origin + direction * along;
    along > 0.0 && point.approx_eq(closest, 2e-3)
}

#[test]
fn zero_active_cameras_extract_nothing() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((Camera { active: false, ..Camera::perspective(60.0) }, Transform::IDENTITY));
    prepare(&mut world);
    assert!(extract_scene(&world, &assets, SIZE).is_none());
    assert!(active_surface_cameras(&world, SIZE).is_empty());
    assert!(top_surface_camera(&world, Vec2::new(10.0, 10.0), SIZE).is_none());
}

#[test]
fn one_camera_is_one_pass_and_the_frame_stats_match_it() {
    let assets = assets();
    let mut world = World::new();
    let entity = world.spawn((cam(0, ViewportRect::FULL), pose(Vec3::new(0.0, 0.0, -6.0))));
    world.spawn((Transform::IDENTITY, sprite(Color::WHITE)));
    let scene = extract(&mut world, &assets);
    let pass = scene.sole_pass();
    assert_eq!(pass.camera_id, entity.id());
    assert_eq!(pass.clear, CameraClear::Color);
    assert_eq!(pass.stats.sprites_visible, 1);
    assert_eq!(scene.stats.sprites_visible, pass.stats.sprites_visible);
    assert_eq!(scene.stats.missing_assets, 0);
    assert_eq!(pass.opaque_sprites.batches.len(), 1);
}

#[test]
fn active_cameras_sort_by_order_then_entity_id() {
    let mut world = World::new();
    let later = world.spawn((cam(5, ViewportRect::FULL), pose(Vec3::new(0.0, 0.0, -6.0))));
    let earlier = world.spawn((cam(-1, ViewportRect::new(0.0, 0.0, 0.5, 1.0)), pose(Vec3::new(0.0, 0.0, -6.0))));
    let tied_low = world.spawn((cam(5, ViewportRect::new(0.5, 0.0, 0.5, 1.0)), pose(Vec3::new(1.0, 0.0, -6.0))));
    world.spawn((Camera { active: false, order: -10, ..cam(0, ViewportRect::FULL) }, Transform::IDENTITY));
    prepare(&mut world);
    let ids: Vec<u32> = active_surface_cameras(&world, SIZE).iter().map(|camera| camera.camera_id).collect();
    let (first_tie, second_tie) =
        if later.id() < tied_low.id() { (later.id(), tied_low.id()) } else { (tied_low.id(), later.id()) };
    assert_eq!(ids, vec![earlier.id(), first_tie, second_tie], "order -1 is first; equal order uses entity id");
    let scene = extract_scene(&world, &assets(), SIZE).unwrap();
    assert_eq!(scene.passes.iter().map(|pass| pass.camera_id).collect::<Vec<_>>(), ids);
    assert_eq!(scene.passes.iter().map(|pass| pass.order).collect::<Vec<_>>(), vec![-1, 5, 5]);
}

#[test]
fn a_hidden_camera_entity_still_renders_when_it_is_active() {
    let mut world = World::new();
    let entity = world.spawn((cam(0, ViewportRect::FULL), Transform::from_xyz(0.0, 0.0, -6.0), Visibility::Hidden));
    prepare(&mut world);
    let scene = extract_scene(&world, &assets(), SIZE).unwrap();
    assert_eq!(scene.sole_pass().camera_id, entity.id());
}

#[test]
fn each_camera_keeps_its_projection_viewport_and_aspect() {
    let mut world = World::new();
    let wide = ViewportRect::new(0.0, 0.0, 0.5, 1.0);
    let tall = ViewportRect::new(0.5, 0.0, 0.5, 1.0);
    world
        .spawn((Camera { order: 0, viewport: wide, ..Camera::perspective(60.0) }, Transform::from_xyz(0.0, 0.0, -6.0)));
    world
        .spawn((Camera { order: 1, viewport: tall, ..Camera::orthographic(8.0) }, Transform::from_xyz(0.0, 0.0, -6.0)));
    let scene = extract(&mut world, &assets());
    let (left, right) = (&scene.passes[0], &scene.passes[1]);
    assert!(left.view.proj != right.view.proj);
    assert!((left.view.aspect() - 1.0).abs() < 1e-4, "half of 200x100 is square");
    assert!((right.view.aspect() - left.view.aspect()).abs() < 1e-4);
    assert_eq!(left.viewport_px[0], 0.0);
    assert!(right.viewport_px[0] > left.viewport_px[0]);
    let narrow = extract_scene(&world, &assets(), Vec2::new(300.0, 100.0)).unwrap();
    let square = extract_scene(&world, &assets(), Vec2::new(100.0, 100.0)).unwrap();
    assert!(!narrow.passes[0].view.proj.approx_eq(&square.passes[0].view.proj, 1e-4));
    assert!(!narrow.passes[1].view.proj.approx_eq(&square.passes[1].view.proj, 1e-4));
}

#[test]
fn a_custom_matrix_stays_fixed_when_another_camera_changes_aspect() {
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.2, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let ortho = Mat4::orthographic_lh(-2.0, 2.0, -1.0, 1.0, 0.1, 40.0);
    let matrix = ortho * shear;
    let custom =
        Camera::new(CameraProjection::custom(matrix).unwrap()).with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0));
    let mut world = World::new();
    world.spawn((custom, Transform::from_xyz(0.0, 0.0, -6.0)));
    world.spawn((
        Camera { order: 1, viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0), ..Camera::perspective(50.0) },
        Transform::from_xyz(0.0, 0.0, -6.0),
    ));
    prepare(&mut world);
    let wide = extract_scene(&world, &assets(), Vec2::new(200.0, 100.0)).unwrap();
    let square = extract_scene(&world, &assets(), Vec2::new(100.0, 100.0)).unwrap();
    assert!(wide.passes[0].view.proj.approx_eq(&matrix, 0.0));
    assert!(square.passes[0].view.proj.approx_eq(&matrix, 0.0));
    assert!(!wide.passes[1].view.proj.approx_eq(&square.passes[1].view.proj, 1e-4));
}

#[test]
fn an_oversize_viewport_is_clamped_to_the_target() {
    let (origin, size) = ViewportRect::new(0.9, 0.9, 0.5, 0.5).to_pixels(Vec2::new(100.0, 100.0));
    assert_eq!(origin, Vec2::new(90.0, 90.0));
    assert_eq!(size, Vec2::new(10.0, 10.0));
    let mut world = World::new();
    world.spawn((cam(0, ViewportRect::new(0.9, 0.9, 0.5, 0.5)), Transform::from_xyz(0.0, 0.0, -6.0)));
    prepare(&mut world);
    let scene = extract_scene(&world, &assets(), Vec2::new(100.0, 100.0)).unwrap();
    assert_eq!(scene.sole_pass().viewport_px, [90.0, 90.0, 10.0, 10.0]);
    assert!(scene.sole_pass().view.contains_window_pixel(Vec2::new(99.0, 99.0)));
    assert!(!scene.sole_pass().view.contains_window_pixel(Vec2::new(89.0, 99.0)));
}

#[test]
fn zero_viewport_still_occupies_one_pixel() {
    let (origin, size) = ViewportRect::new(0.0, 0.0, 0.0, 0.0).to_pixels(Vec2::new(100.0, 100.0));
    assert_eq!(size, Vec2::ONE);
    assert_eq!(origin, Vec2::ZERO);
    let mut world = World::new();
    world.spawn((cam(0, ViewportRect::new(0.0, 0.0, 0.0, 0.0)), Transform::from_xyz(0.0, 0.0, -6.0)));
    let scene = extract(&mut world, &assets());
    assert_eq!(scene.sole_pass().view.viewport, Vec2::ONE);
}

#[test]
fn frustum_culling_and_billboards_are_per_camera() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((cam(0, ViewportRect::new(0.0, 0.0, 0.5, 1.0)), pose(Vec3::new(0.0, 0.0, -6.0))));
    world.spawn((
        Camera { order: 1, viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0), ..Camera::orthographic(40.0) },
        Transform::from_xyz(0.0, 0.0, -6.0).with_rotation(Quat::from_rotation_y(0.6)),
    ));
    world.spawn((Transform::from_xyz(6.0, 0.0, 0.0), sprite(Color::rgb(1.0, 0.0, 0.0))));
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("sprite")
            .with_size(Vec2::splat(1.0))
            .with_anchor(Vec2::splat(0.5))
            .with_orientation(SpriteOrientation::Billboard)
            .cutout(),
    ));
    let scene = extract(&mut world, &assets);
    let (left, right) = (&scene.passes[0], &scene.passes[1]);
    assert_eq!(left.stats.sprites_visible, 1, "the offset sprite is outside the left frustum");
    assert_eq!(left.stats.sprites_culled, 1);
    assert_eq!(right.stats.sprites_visible, 2);
    let left_billboard = left
        .opaque_sprites
        .instances
        .iter()
        .chain(left.cutout_sprites.instances.iter())
        .find(|instance| instance.material_id.is_none())
        .expect("unassigned billboard");
    let right_billboard = right
        .opaque_sprites
        .instances
        .iter()
        .chain(right.cutout_sprites.instances.iter())
        .find(|instance| instance.material_id.is_none())
        .expect("unassigned billboard");
    assert!(!left_billboard.model.approx_eq(&right_billboard.model, 1e-4));
    assert!(left_billboard.model.cols[0].xyz().normalized().approx_eq(left.view.right.normalized(), 1e-3));
    assert!(right_billboard.model.cols[0].xyz().normalized().approx_eq(right.view.right.normalized(), 1e-3));
    assert!(left.opaque_sprites.instances.is_empty(), "the material sprite is outside the left frustum");
    let flat = engine_assets::AssetHandle::<MaterialAsset>::from_name("flat");
    assert_eq!(right.opaque_sprites.instances.len(), 1);
    assert_eq!(right.opaque_sprites.instances[0].material_id, Some(flat.id()));
    assert_eq!(right.opaque_sprites.instances[0].material_revision, assets.revision(flat));
    assert_eq!(right.opaque_sprites.batches.len(), 1);
    assert_eq!(left.cutout_sprites.batches.len(), 1);
}

#[test]
fn transparent_order_follows_each_camera() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((cam(0, ViewportRect::new(0.0, 0.0, 0.5, 1.0)), pose(Vec3::new(0.0, 0.0, -8.0))));
    world.spawn((
        Camera {
            order: 1,
            viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0),
            ..Camera::orthographic(10.0).with_clear_color(Color::BLACK)
        },
        Transform::from_xyz(0.0, 0.0, 8.0).with_rotation(Quat::from_rotation_y(core::f32::consts::PI)),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Sprite::new("sprite")
            .with_size(Vec2::splat(0.4))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(1.0, 0.0, 0.0))
            .with_blend(SpriteBlend::Alpha),
        RenderOrder::default(),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Sprite::new("sprite")
            .with_size(Vec2::splat(0.4))
            .with_anchor(Vec2::splat(0.5))
            .with_tint(Color::rgb(0.0, 0.0, 1.0))
            .with_blend(SpriteBlend::Alpha),
    ));
    let scene = extract(&mut world, &assets);
    let front: Vec<_> = scene.passes[0].blended_sprites.instances.iter().map(|i| i.tint[2] > 0.5).collect();
    let back: Vec<_> = scene.passes[1].blended_sprites.instances.iter().map(|i| i.tint[2] > 0.5).collect();
    assert_eq!(front, vec![true, false], "from the front the blue sprite is farther and draws first");
    assert_eq!(back, vec![false, true], "from behind, the red sprite is farther");
    assert_eq!(scene.passes[0].transparent.len(), 1, "the two sprites share a texture and stay one run");
    assert_eq!(scene.passes[0].blended_sprites.batches[0].range, 0..2);
    assert_eq!(scene.passes[1].blended_sprites.batches[0].range, 0..2);
    assert_eq!(scene.stats.sprites_visible, 4, "each camera counts the sprites it draws");
}

#[test]
fn the_snapshot_owns_per_camera_instances() {
    fn assert_send<T: Send>() {}
    assert_send::<RenderScene>();
    let assets = assets();
    let mut world = World::new();
    world.spawn((cam(0, ViewportRect::FULL), pose(Vec3::new(0.0, 0.0, -6.0))));
    world.spawn((
        Camera { order: 1, ..cam(1, ViewportRect::FULL) },
        Transform::from_xyz(0.0, 0.0, -6.0).with_rotation(Quat::from_rotation_y(0.4)),
    ));
    world.spawn((Transform::IDENTITY, sprite(Color::WHITE).with_orientation(SpriteOrientation::Billboard)));
    let scene = extract(&mut world, &assets);
    assert_ne!(scene.passes[0].view.view_proj, scene.passes[1].view.view_proj);
    assert_ne!(scene.passes[0].opaque_sprites.instances[0].model, scene.passes[1].opaque_sprites.instances[0].model);
    let mut copied = scene.passes[0].opaque_sprites.instances.clone();
    copied[0].model = Mat4::IDENTITY;
    assert_ne!(copied[0].model, scene.passes[0].opaque_sprites.instances[0].model);
}

#[test]
fn picking_selects_the_topmost_surface_camera_and_keeps_an_explicit_view() {
    let mut world = World::new();
    let left = world.spawn((cam(0, ViewportRect::new(0.0, 0.0, 0.5, 1.0)), pose(Vec3::new(0.0, 0.0, -6.0))));
    let right = world.spawn((cam(1, ViewportRect::new(0.5, 0.0, 0.5, 1.0)), pose(Vec3::new(2.0, 0.0, -6.0))));
    let inactive = world.spawn((Camera { active: false, order: 9, ..cam(9, ViewportRect::FULL) }, Transform::IDENTITY));
    prepare(&mut world);
    let left_hit = top_surface_camera(&world, Vec2::new(20.0, 40.0), SIZE).unwrap();
    let right_hit = top_surface_camera(&world, Vec2::new(150.0, 40.0), SIZE).unwrap();
    assert_eq!(left_hit.camera_id, left.id());
    assert_eq!(right_hit.camera_id, right.id());
    assert!(top_surface_camera(&world, Vec2::new(20.0, -5.0), SIZE).is_none());
    assert_eq!(left_hit.viewport_pixel, Vec2::new(20.0, 40.0));
    assert_eq!(right_hit.viewport_pixel, Vec2::new(150.0, 40.0) - right_hit.view.viewport_origin);

    let overlap_low = world.spawn((cam(0, ViewportRect::FULL), pose(Vec3::new(0.0, 0.0, -8.0))));
    let overlap_high = world.spawn((cam(0, ViewportRect::FULL), pose(Vec3::new(0.0, 1.0, -8.0))));
    prepare(&mut world);
    assert!(overlap_low.id() < overlap_high.id());
    let top = top_surface_camera(&world, Vec2::new(40.0, 50.0), SIZE).unwrap();
    assert_eq!(top.camera_id, overlap_high.id(), "equal order uses the higher entity id, which draws last");
    assert_ne!(top.camera_id, right.id(), "the right camera does not contain this pixel");

    let explicit = camera_view_of(&world, inactive.id(), SIZE).unwrap();
    let point = Vec3::new(0.2, -0.3, 1.0);
    let local = explicit.project(point).unwrap().pixels;
    let window = local + explicit.viewport_origin;
    let (origin, direction) = explicit.ray_from_pixels(window - explicit.viewport_origin).unwrap();
    assert!(on_ray(point, origin, direction), "an explicit camera ray passes through the projected point");

    let custom = Camera::new(CameraProjection::custom(oblique()).unwrap());
    let mut custom_world = World::new();
    let custom_entity = custom_world.spawn((custom, Transform::from_xyz(0.0, 0.0, -5.0)));
    prepare(&mut custom_world);
    let view = camera_view_of(&custom_world, custom_entity.id(), SIZE).unwrap();
    let sample = Vec3::new(0.4, 0.2, 0.0);
    let pixels = view.project(sample).unwrap().pixels;
    let (origin, direction) = view.ray_from_pixels(pixels).unwrap();
    assert!(on_ray(sample, origin, direction));
}

fn oblique() -> Mat4 {
    let ortho = Mat4::orthographic_lh(-4.0, 4.0, -2.0, 2.0, 0.1, 40.0);
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.25, 0.05, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    ortho * shear
}
