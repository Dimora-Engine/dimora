//! D-26 contract for `SpriteOrientation`.
//!
//! Placement is a function of the current `GlobalTransform` and `CameraView`. It does not write
//! `Transform`. A parent sprite's rendered basis is not written back into the hierarchy, so a
//! `Fixed` child keeps the simulation matrix. The exact vertical view has no cylindrical heading:
//! `Upright` stays on world +Y and uses the camera's right axis. Approaching that pole from opposite
//! sides reverses the heading. Nothing here blends those sides together.

use core::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use engine_assets::{AssetManager, MaterialAsset, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat4, Quat, Transform, Vec2, Vec3, Vec4};
use engine_renderer::*;
use engine_world::{propagate_transforms, GlobalTransform, World};

const VIEWPORT: Vec2 = Vec2::new(128.0, 128.0);

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("tex", Texture::solid(4, 4, [255; 4])).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 4, 4)).unwrap();
    assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    assets
}

fn sprite(orientation: SpriteOrientation) -> Sprite {
    Sprite::new("sprite")
        .with_size(Vec2::new(2.0, 4.0))
        .with_anchor(Vec2::new(0.5, 0.0))
        .with_orientation(orientation)
        .cutout()
        .unlit()
}

fn extract(world: &mut World) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, &assets(), VIEWPORT).expect("camera")
}

fn model_of(scene: &RenderScene, pass: usize) -> Mat4 {
    scene.passes[pass].cutout_sprites.instances[0].model
}

fn corners(model: Mat4) -> (Vec3, Vec3, Vec3) {
    (model.transform_point3(Vec3::ZERO), model.transform_point3(Vec3::X), model.transform_point3(Vec3::Y))
}

fn edge_axes(model: Mat4) -> (Vec3, Vec3) {
    let (bl, br, tl) = corners(model);
    ((br - bl).normalized(), (tl - bl).normalized())
}

fn view_of(camera: &Camera, pose: Transform) -> CameraView {
    CameraView::new(camera, &GlobalTransform { matrix: pose.to_matrix() }, VIEWPORT)
}

/// `Y × forward`, or the camera right when that product is numerically zero.
fn upright_right(view: &CameraView) -> Vec3 {
    let right = Vec3::Y.cross(view.forward);
    if right.length_squared() > 1e-8 {
        right.normalized()
    } else {
        view.right
    }
}

fn down_from(z: f32) -> Transform {
    Transform::looking_at(Vec3::new(0.0, 10.0, z), Vec3::ZERO, Vec3::Z)
}

fn roll_about_forward(pose: Transform, angle: f32) -> Transform {
    pose.with_rotation(pose.rotation * Quat::from_rotation_z(angle))
}

#[test]
fn fixed_orientation_uses_the_global_rotation() {
    let pose = Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(Quat::from_rotation_y(FRAC_PI_4));
    let mut world = World::new();
    world.spawn((Camera::orthographic(10.0), Transform::from_xyz(0.0, 8.0, -6.0)));
    world.spawn((pose, sprite(SpriteOrientation::Fixed)));
    let (bl, br, tl) = corners(model_of(&extract(&mut world), 0));
    let expect = |p: Vec3| pose.transform_point(p * Vec3::new(2.0, 4.0, 1.0) - Vec3::new(1.0, 0.0, 0.0));
    assert!(bl.approx_eq(expect(Vec3::ZERO), 1e-4));
    assert!(br.approx_eq(expect(Vec3::X), 1e-4));
    assert!(tl.approx_eq(expect(Vec3::Y), 1e-4));
}

#[test]
fn upright_stays_on_world_up_away_from_the_pole() {
    let camera = Camera::perspective(50.0);
    let pose = Transform::from_xyz(0.0, 4.0, -8.0).with_rotation(Quat::from_rotation_x(0.6));
    let view = view_of(&camera, pose);
    let mut world = World::new();
    world.spawn((camera, pose));
    world.spawn((Transform::from_xyz(1.0, 0.0, 2.0), sprite(SpriteOrientation::Upright)));
    let (x, y) = edge_axes(model_of(&extract(&mut world), 0));
    assert!(y.approx_eq(Vec3::Y, 1e-4), "up is world up: {y:?}");
    assert!(x.approx_eq(upright_right(&view), 1e-4), "yaw follows the horizontal heading: {x:?}");
    assert!(x.y.abs() < 1e-4);
}

#[test]
fn both_exact_vertical_views_are_deterministic_and_stay_vertical() {
    for (z_sign, name) in [(1.0, "from above"), (-1.0, "from below")] {
        let camera = Camera::orthographic(8.0);
        let pose = Transform::looking_at(Vec3::new(0.0, 10.0 * z_sign, 0.0), Vec3::ZERO, Vec3::Z);
        let view = view_of(&camera, pose);
        assert!(view.forward.dot(Vec3::Y).abs() > 0.999, "{name}");
        let mut world = World::new();
        world.spawn((camera, pose));
        let entity = world.spawn((Transform::IDENTITY, sprite(SpriteOrientation::Upright)));
        let transform = *world.component::<Transform>(entity).unwrap();
        let first = extract(&mut world);
        let again = extract(&mut world);
        let model = model_of(&first, 0);
        assert!(model.approx_eq(&model_of(&again, 0), 0.0), "{name} repeats");
        let (x, y) = edge_axes(model);
        assert!(y.approx_eq(Vec3::Y, 1e-4), "{name} stays vertical: {y:?}");
        assert!(x.approx_eq(view.right.normalized(), 1e-4), "{name} uses the camera right: {x:?}");
        assert_eq!(*world.component::<Transform>(entity).unwrap(), transform);
    }
}

#[test]
fn opposite_near_vertical_headings_reverse_and_ignore_history() {
    // A continuous path through exact vertical cannot keep one cylindrical heading. These two
    // views are the two limits. Each result is computed on its own, then repeated.
    let camera = Camera::orthographic(8.0);
    let mut headings = Vec::new();
    for z in [0.5, -0.5] {
        let pose = roll_about_forward(down_from(z), 0.7);
        let view = view_of(&camera, pose);
        let expected = Vec3::Y.cross(view.forward).normalized();
        assert!(expected.dot(view.right.normalized()).abs() < 0.95, "roll separates camera right from the heading");
        let mut world = World::new();
        world.spawn((camera, pose));
        world.spawn((Transform::IDENTITY, sprite(SpriteOrientation::Upright)));
        let first = edge_axes(model_of(&extract(&mut world), 0)).0;
        let second = edge_axes(model_of(&extract(&mut world), 0)).0;
        assert!(first.approx_eq(second, 0.0));
        assert!(first.approx_eq(expected, 1e-4), "z={z}: {first:?} vs {expected:?}");
        assert!(first.y.abs() < 1e-4, "a near-vertical view does not tilt the quad");
        headings.push(first);
    }
    assert!(headings[0].dot(headings[1]) < -0.99, "the two sides of the pole face opposite ways");
}

#[test]
fn camera_yaw_turns_upright_and_billboard_with_the_view() {
    let camera = Camera::orthographic(8.0);
    let pose = Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_2));
    let view = view_of(&camera, pose);
    assert!(view.forward.approx_eq(Vec3::X, 1e-4));
    for orientation in [SpriteOrientation::Upright, SpriteOrientation::Billboard] {
        let mut world = World::new();
        world.spawn((camera, pose));
        world.spawn((Transform::from_rotation(Quat::from_rotation_y(1.1)), sprite(orientation)));
        let (x, y) = edge_axes(model_of(&extract(&mut world), 0));
        if orientation == SpriteOrientation::Upright {
            assert!(y.approx_eq(Vec3::Y, 1e-4));
            assert!(x.approx_eq(upright_right(&view), 1e-4));
        } else {
            assert!(x.approx_eq(view.right.normalized(), 1e-4));
            assert!(y.approx_eq(view.up.normalized(), 1e-4));
        }
    }
}

#[test]
fn camera_roll_turns_billboards_and_leaves_upright_world_vertical() {
    let camera = Camera::orthographic(8.0);
    let pose = roll_about_forward(Transform::from_xyz(0.0, 1.0, -6.0), FRAC_PI_2);
    let view = view_of(&camera, pose);
    let mut billboard = World::new();
    billboard.spawn((camera, pose));
    billboard.spawn((Transform::IDENTITY, sprite(SpriteOrientation::Billboard)));
    let (x, y) = edge_axes(model_of(&extract(&mut billboard), 0));
    assert!(x.approx_eq(view.right.normalized(), 1e-4));
    assert!(y.approx_eq(view.up.normalized(), 1e-4));

    let level = Transform::from_xyz(0.0, 1.0, -6.0);
    let mut upright = World::new();
    upright.spawn((camera, pose));
    upright.spawn((Transform::IDENTITY, sprite(SpriteOrientation::Upright)));
    let rolled_up = edge_axes(model_of(&extract(&mut upright), 0)).1;
    let mut unrolled = World::new();
    unrolled.spawn((camera, level));
    unrolled.spawn((Transform::IDENTITY, sprite(SpriteOrientation::Upright)));
    let level_axes = edge_axes(model_of(&extract(&mut unrolled), 0));
    assert!(rolled_up.approx_eq(Vec3::Y, 1e-4));
    assert!(level_axes.0.approx_eq(edge_axes(model_of(&extract(&mut upright), 0)).0, 1e-4));
    assert!(view.up.normalized().dot(Vec3::Y).abs() < 0.1, "the camera itself is rolled");
}

#[test]
fn an_exact_pole_uses_the_rolled_camera_right_and_stays_a_pure_function() {
    let camera = Camera::orthographic(8.0);
    let pose = roll_about_forward(down_from(0.0), 0.4);
    let view = view_of(&camera, pose);
    assert!(Vec3::Y.cross(view.forward).length_squared() <= 1e-8);
    let mut world = World::new();
    world.spawn((camera, pose));
    world.spawn((Transform::IDENTITY, sprite(SpriteOrientation::Upright)));
    let (x, y) = edge_axes(model_of(&extract(&mut world), 0));
    assert!(y.approx_eq(Vec3::Y, 1e-4));
    assert!(x.approx_eq(view.right.normalized(), 1e-4));
    assert!(model_of(&extract(&mut world), 0).approx_eq(&model_of(&extract(&mut world), 0), 0.0));
}

#[test]
fn zero_scale_collapses_every_policy_at_the_entity_origin() {
    let origin = Vec3::new(2.0, 3.0, 4.0);
    for orientation in [SpriteOrientation::Fixed, SpriteOrientation::Upright, SpriteOrientation::Billboard] {
        let mut world = World::new();
        world.spawn((Camera::orthographic(8.0), Transform::from_xyz(0.0, 0.0, -6.0)));
        world.spawn((Transform::from_position(origin).with_scale(Vec3::ZERO), sprite(orientation)));
        let first = model_of(&extract(&mut world), 0);
        let (bl, br, tl) = corners(first);
        assert!(
            bl.approx_eq(origin, 1e-4) && br.approx_eq(origin, 1e-4) && tl.approx_eq(origin, 1e-4),
            "{orientation:?}"
        );
        assert!(first.approx_eq(&model_of(&extract(&mut world), 0), 0.0));
    }
}

#[test]
fn parent_translation_moves_the_origin_and_parent_yaw_does_not_yaw_a_billboard() {
    let camera = Camera::orthographic(12.0);
    let pose = Transform::from_xyz(0.0, 2.0, -8.0);
    let view = view_of(&camera, pose);
    let mut world = World::new();
    world.spawn((camera, pose));
    let parent = world.spawn((Transform::from_xyz(4.0, 1.0, 0.0),));
    world.spawn_child(parent, (Transform::from_xyz(1.0, 0.5, 6.0), sprite(SpriteOrientation::Billboard)));
    let (bl, br, _) = corners(model_of(&extract(&mut world), 0));
    let anchor = (bl + br) * 0.5;
    assert!(anchor.approx_eq(Vec3::new(5.0, 1.5, 6.0), 1e-4), "{anchor:?}");

    let mut turned = World::new();
    turned.spawn((camera, pose));
    let yawed = turned.spawn((Transform::from_xyz(1.0, 0.0, 1.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2)),));
    turned.spawn_child(yawed, (Transform::from_xyz(0.4, 0.5, 0.2), sprite(SpriteOrientation::Billboard)));
    let model = model_of(&extract(&mut turned), 0);
    let (x, y) = edge_axes(model);
    assert!(x.approx_eq(view.right.normalized(), 1e-4), "parent yaw is not billboard yaw");
    assert!(y.approx_eq(view.up.normalized(), 1e-4));
    let global = turned.component::<GlobalTransform>(yawed).unwrap().matrix.transform_point3(Vec3::new(0.4, 0.5, 0.2));
    let (bl, br, _) = corners(model);
    assert!(((bl + br) * 0.5).approx_eq(global, 1e-4), "the anchor still follows the hierarchy");
}

#[test]
fn a_fixed_child_keeps_the_simulation_matrix_under_a_billboarded_parent() {
    // The rendered billboard basis is not a parent transform. The child never sees it.
    let mut world = World::new();
    world.spawn((Camera::orthographic(10.0), Transform::from_xyz(0.0, 3.0, -8.0)));
    let parent = world.spawn((
        Transform::from_xyz(2.0, 0.0, 1.0).with_rotation(Quat::from_rotation_y(0.8)),
        sprite(SpriteOrientation::Billboard).with_tint(Color::rgb(1.0, 0.0, 0.0)),
    ));
    let child = world.spawn_child(
        parent,
        (Transform::from_xyz(0.0, 1.0, 0.0), sprite(SpriteOrientation::Fixed).with_tint(Color::rgb(0.0, 0.0, 1.0))),
    );
    let scene = extract(&mut world);
    let instances = &scene.sole_pass().cutout_sprites.instances;
    let parent_global = world.component::<GlobalTransform>(parent).unwrap().matrix;
    let child_global = world.component::<GlobalTransform>(child).unwrap().matrix;
    let parent_model = instances.iter().find(|i| i.tint[0] > 0.5).unwrap().model;
    let child_model = instances.iter().find(|i| i.tint[2] > 0.5).unwrap().model;
    assert!(!parent_model.cols[0].xyz().normalized().approx_eq(parent_global.cols[0].xyz().normalized(), 1e-2));
    let child_fixed =
        child_global * Mat4::from_translation(Vec3::new(-1.0, 0.0, 0.0)) * Mat4::from_scale(Vec3::new(2.0, 4.0, 1.0));
    assert!(child_model.approx_eq(&child_fixed, 1e-4));
    assert!(!child_model.cols[0].xyz().normalized().approx_eq(parent_model.cols[0].xyz().normalized(), 1e-2));
}

#[test]
fn fixed_ground_art_lies_in_the_xz_plane() {
    let pose = Transform::from_xyz(0.0, 1.5, 0.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2));
    let mut world = World::new();
    world.spawn((Camera::orthographic(8.0), down_from(0.0)));
    world.spawn((pose, sprite(SpriteOrientation::Fixed)));
    let (bl, br, tl) = corners(model_of(&extract(&mut world), 0));
    assert!((bl.y - 1.5).abs() < 1e-4 && (br.y - 1.5).abs() < 1e-4 && (tl.y - 1.5).abs() < 1e-4);
    let normal = (br - bl).cross(tl - bl).normalized();
    assert!(normal.dot(Vec3::Y).abs() > 0.999, "the quad is horizontal: {normal:?}");
    assert!((tl - bl).z.abs() > 1.0, "local Y lands in Z");
}

#[test]
fn a_sheared_projection_does_not_change_the_world_orientation() {
    let pose = Transform::from_xyz(0.0, 2.0, -8.0);
    let ortho = Mat4::orthographic_lh(-4.0, 4.0, -4.0, 4.0, 0.1, 50.0);
    let shear = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.25, 0.1, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let custom = Camera::new(CameraProjection::custom(shear * ortho).unwrap());
    for orientation in [SpriteOrientation::Fixed, SpriteOrientation::Upright, SpriteOrientation::Billboard] {
        let mut plain = World::new();
        plain.spawn((Camera::new(CameraProjection::custom(ortho).unwrap()), pose));
        plain.spawn((Transform::from_rotation(Quat::from_rotation_y(0.4)), sprite(orientation)));
        let mut oblique = World::new();
        oblique.spawn((custom, pose));
        oblique.spawn((Transform::from_rotation(Quat::from_rotation_y(0.4)), sprite(orientation)));
        let plain_scene = extract(&mut plain);
        let oblique_scene = extract(&mut oblique);
        assert!(model_of(&plain_scene, 0).approx_eq(&model_of(&oblique_scene, 0), 1e-5), "{orientation:?}");
        assert!(!plain_scene.sole_pass().view.view_proj.approx_eq(&oblique_scene.sole_pass().view.view_proj, 1e-3));
    }
}

#[test]
fn two_cameras_orient_one_sprite_without_writing_transform() {
    let mut world = World::new();
    world.spawn((
        Camera::orthographic(10.0).with_viewport(ViewportRect::new(0.0, 0.0, 0.5, 1.0)),
        Transform::from_xyz(0.0, 0.0, -6.0),
    ));
    world.spawn((
        Camera { order: 1, viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0), ..Camera::orthographic(10.0) },
        Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_2)),
    ));
    let entity = world.spawn((Transform::from_xyz(0.0, 1.0, 2.0), sprite(SpriteOrientation::Upright)));
    let before = *world.component::<Transform>(entity).unwrap();
    let scene = extract(&mut world);
    assert_eq!(*world.component::<Transform>(entity).unwrap(), before);
    let (left, right) = (&scene.passes[0], &scene.passes[1]);
    assert!(!model_of_pass(left).approx_eq(&model_of_pass(right), 1e-4));
    assert!(edge_axes(model_of_pass(left)).1.approx_eq(Vec3::Y, 1e-4));
    assert!(edge_axes(model_of_pass(left)).0.approx_eq(upright_right(&left.view), 1e-4));
    assert!(edge_axes(model_of_pass(right)).0.approx_eq(upright_right(&right.view), 1e-4));
}

fn model_of_pass(pass: &CameraPass) -> Mat4 {
    pass.cutout_sprites.instances[0].model
}

#[test]
fn compatible_orientation_policies_share_one_batch() {
    let mut world = World::new();
    world.spawn((Camera::orthographic(8.0), Transform::from_xyz(0.0, 0.0, -6.0)));
    let material = engine_assets::AssetHandle::<MaterialAsset>::from_name("flat");
    for (x, orientation) in
        [(-1.5, SpriteOrientation::Fixed), (0.0, SpriteOrientation::Upright), (1.5, SpriteOrientation::Billboard)]
    {
        world.spawn((
            Transform::from_xyz(x, 0.0, 0.0),
            sprite(orientation).with_material(material).with_tint(Color::rgb(x + 1.5, 0.2, 0.4)),
        ));
    }
    let scene = extract(&mut world);
    let layer = &scene.sole_pass().opaque_sprites;
    assert_eq!(layer.instances.len(), 3);
    assert_eq!(layer.batches.len(), 1);
    assert_eq!(layer.batches[0].range, 0..3);
}
