//! Headless collision geometry. No window and no GPU.

use engine_math::{Quat, Transform, Vec3};
use engine_world::{sync_collision, Collider, ColliderShape, CollisionWorld, Entity, Parent, SkipReason, World};

const EPS: f32 = 1e-4;

fn synced() -> World {
    let mut world = World::new();
    sync_collision(&mut world);
    world
}

fn place(world: &mut World, transform: Transform, collider: Collider) -> Entity {
    world.spawn((transform, collider))
}

fn index(world: &World) -> std::cell::Ref<'_, CollisionWorld> {
    world.resource::<CollisionWorld>()
}

fn near(actual: Vec3, expected: Vec3) {
    assert!(actual.approx_eq(expected, EPS), "actual {actual:?} expected {expected:?}");
}

#[test]
fn identity_and_translated_spheres_keep_their_centers() {
    let mut world = synced();
    let root = place(&mut world, Transform::IDENTITY, Collider::sphere(2.0));
    let moved = place(&mut world, Transform::from_xyz(3.0, -4.0, 5.0), Collider::sphere(0.5));
    sync_collision(&mut world);
    near(index(&world).center(root).unwrap(), Vec3::ZERO);
    assert!(
        matches!(index(&world).indexed_shape(root), Some(ColliderShape::Sphere { radius }) if (radius - 2.0).abs() < EPS)
    );
    near(index(&world).center(moved).unwrap(), Vec3::new(3.0, -4.0, 5.0));
}

#[test]
fn a_rotated_box_keeps_its_extents_and_turns_local_x_toward_world_negative_z() {
    let mut world = synced();
    let yaw = std::f32::consts::FRAC_PI_2;
    let entity = place(
        &mut world,
        Transform::IDENTITY.with_rotation(Quat::from_rotation_y(yaw)),
        Collider::aabb_box(Vec3::new(2.0, 0.5, 0.25)),
    );
    sync_collision(&mut world);
    let shape = index(&world).indexed_shape(entity).unwrap();
    let ColliderShape::Box { half_extents } = shape else { panic!("box") };
    near(half_extents, Vec3::new(2.0, 0.5, 0.25));
    near(index(&world).rotation(entity).unwrap().rotate(Vec3::X), Vec3::new(0.0, 0.0, -1.0));
}

#[test]
fn a_uniformly_scaled_capsule_bakes_scale_into_radius_and_half_height() {
    let mut world = synced();
    let entity = place(&mut world, Transform::from_scale(Vec3::splat(3.0)), Collider::capsule(0.2, 1.0));
    sync_collision(&mut world);
    let shape = index(&world).indexed_shape(entity).unwrap();
    match shape {
        ColliderShape::Capsule { radius, half_height } => {
            assert!((radius - 0.6).abs() < EPS);
            assert!((half_height - 3.0).abs() < EPS);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_nonuniform_orthogonal_box_scales_each_axis() {
    let mut world = synced();
    let entity = place(&mut world, Transform::from_scale(Vec3::new(2.0, 3.0, 4.0)), Collider::aabb_box(Vec3::ONE));
    sync_collision(&mut world);
    let shape = index(&world).indexed_shape(entity).unwrap();
    match shape {
        ColliderShape::Box { half_extents } => near(half_extents, Vec3::new(2.0, 3.0, 4.0)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn nonuniform_sphere_and_capsule_are_rejected() {
    let mut world = synced();
    let sphere = place(&mut world, Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)), Collider::sphere(1.0));
    let capsule = place(&mut world, Transform::from_scale(Vec3::new(1.0, 1.0, 3.0)), Collider::capsule(0.5, 1.0));
    sync_collision(&mut world);
    assert!(index(&world).center(sphere).is_none());
    assert!(index(&world).center(capsule).is_none());
    assert!(index(&world).skipped().contains(&(sphere, SkipReason::NonUniformScale)));
    assert!(index(&world).skipped().contains(&(capsule, SkipReason::NonUniformScale)));
}

#[test]
fn a_rotated_child_under_a_uniform_parent_lands_at_the_hierarchy_point() {
    let mut world = synced();
    let parent = world.spawn((Transform::from_xyz(10.0, 0.0, 5.0)
        .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
        .with_uniform_scale(2.0),));
    let child = world.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0), Collider::sphere(0.5)));
    sync_collision(&mut world);
    near(index(&world).center(child).unwrap(), Vec3::new(10.0, 0.0, 3.0));
    let shape = index(&world).indexed_shape(child).unwrap();
    match shape {
        ColliderShape::Sphere { radius } => assert!((radius - 1.0).abs() < EPS),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_translated_parent_moves_the_child_center() {
    let mut world = synced();
    let parent = world.spawn((Transform::from_xyz(4.0, -2.0, 1.0),));
    let child = world.spawn_child(parent, (Transform::from_xyz(0.0, 1.0, 0.0), Collider::aabb_box(Vec3::splat(0.25))));
    sync_collision(&mut world);
    near(index(&world).center(child).unwrap(), Vec3::new(4.0, -1.0, 1.0));
}

#[test]
fn negative_x_scale_mirrors_a_nonzero_offset_to_world_x_minus_two() {
    let mut world = synced();
    let entity = place(
        &mut world,
        Transform::from_scale(Vec3::new(-2.0, 1.0, 1.0)),
        Collider::aabb_box(Vec3::splat(0.25)).with_offset(Vec3::X),
    );
    sync_collision(&mut world);
    near(index(&world).center(entity).unwrap(), Vec3::new(-2.0, 0.0, 0.0));
    near(index(&world).rotation(entity).unwrap().rotate(Vec3::X), Vec3::X);
}

#[test]
fn a_rotated_child_under_nonuniform_scale_is_shear() {
    let mut world = synced();
    let parent = world.spawn((Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)),));
    let child = world.spawn_child(
        parent,
        (
            Transform::IDENTITY.with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
            Collider::aabb_box(Vec3::splat(0.5)),
        ),
    );
    sync_collision(&mut world);
    assert!(index(&world).center(child).is_none());
    assert!(index(&world).skipped().contains(&(child, SkipReason::Shear)));
}

#[test]
fn a_right_angle_child_under_nonuniform_scale_stays_a_box() {
    let mut world = synced();
    let parent = world.spawn((Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)),));
    let child = world.spawn_child(
        parent,
        (
            Transform::IDENTITY.with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Collider::aabb_box(Vec3::ONE),
        ),
    );
    sync_collision(&mut world);
    assert!(index(&world).center(child).is_some());
}

#[test]
fn singular_non_finite_disabled_and_bad_shapes_stay_out() {
    let mut world = synced();
    let singular = place(&mut world, Transform::from_scale(Vec3::new(0.0, 1.0, 1.0)), Collider::sphere(1.0));
    let non_finite = place(&mut world, Transform::from_xyz(f32::NAN, 0.0, 0.0), Collider::sphere(1.0));
    let disabled = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0).disabled());
    let flat = place(&mut world, Transform::IDENTITY, Collider::sphere(0.0));
    let missing = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0));
    let gone = world.spawn(());
    world.despawn(gone).unwrap();
    world.insert(missing, (Parent(gone),)).unwrap();
    sync_collision(&mut world);

    let skipped = index(&world).skipped().to_vec();
    assert!(skipped.contains(&(singular, SkipReason::Degenerate)));
    assert!(skipped.contains(&(non_finite, SkipReason::NonFinite)));
    assert!(skipped.contains(&(flat, SkipReason::BadShape)));
    assert!(skipped.iter().all(|(entity, _)| *entity != disabled));
    assert!(!index(&world).entities().contains(&disabled));
    assert!(skipped.iter().any(|(entity, reason)| *entity == missing && *reason == SkipReason::MissingParent));
}

#[test]
fn a_second_sync_does_not_treat_the_same_skip_as_new() {
    let mut world = synced();
    place(&mut world, Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)), Collider::sphere(1.0));
    sync_collision(&mut world);
    let first = index(&world).skipped().to_vec();
    let generation = index(&world).generation();
    sync_collision(&mut world);
    assert_eq!(index(&world).skipped(), first.as_slice());
    assert_eq!(index(&world).generation(), generation + 1);
}
