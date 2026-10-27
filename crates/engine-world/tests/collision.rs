//! Headless collision geometry. No window and no GPU.

use engine_math::{Quat, Transform, Vec3};
use engine_world::{
    pair_interacts, register_world_components, sync_collision, Collider, ColliderShape, CollisionQuery, CollisionWorld,
    ComponentRegistry, Entity, Parent, Persistence, SkipReason, World,
};

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

fn shape_at(world: &World, shape: ColliderShape, pose: Transform) -> Vec<Entity> {
    index(world).overlap(shape, &pose, CollisionQuery::all())
}

#[test]
fn collider_defaults_and_registry_are_persisted_data() {
    let collider = Collider::sphere(1.0);
    assert_eq!(collider.offset, Vec3::ZERO);
    assert_eq!(collider.layers, 1);
    assert_eq!(collider.mask, u32::MAX);
    assert!(!collider.sensor);
    assert!(collider.enabled);

    let world = World::new();
    register_world_components(&mut world.resource_mut::<ComponentRegistry>()).unwrap();
    let registry = world.resource::<ComponentRegistry>();
    let info = registry.get::<Collider>().unwrap();
    assert_eq!(info.name, "Collider");
    assert_eq!(info.persistence, Persistence::Persist);
    assert!(!info.script_visible);
    let names: Vec<_> = info.fields.iter().map(|field| field.name).collect();
    assert_eq!(names, ["shape", "offset", "layers", "mask", "sensor", "enabled"]);
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
fn tiny_and_large_scales_use_a_relative_uniform_test() {
    let mut world = synced();
    let tiny = place(&mut world, Transform::from_scale(Vec3::splat(1e-5)), Collider::sphere(1.0));
    let tiny_skew = place(
        &mut world,
        Transform::from_xyz(1.0, 0.0, 0.0).with_scale(Vec3::new(1e-5, 2e-5, 1e-5)),
        Collider::sphere(1.0),
    );
    let large =
        place(&mut world, Transform::from_xyz(2.0, 0.0, 0.0).with_uniform_scale(10_000.0), Collider::sphere(1.0));
    let large_close = place(
        &mut world,
        Transform::from_xyz(3.0, 0.0, 0.0).with_scale(Vec3::new(10_000.0, 10_000.25, 10_000.0)),
        Collider::sphere(1.0),
    );
    let large_skew = place(
        &mut world,
        Transform::from_xyz(4.0, 0.0, 0.0).with_scale(Vec3::new(10_000.0, 20_000.0, 10_000.0)),
        Collider::sphere(1.0),
    );
    sync_collision(&mut world);
    match index(&world).indexed_shape(tiny).expect("tiny uniform") {
        ColliderShape::Sphere { radius } => assert!((radius - 1e-5).abs() < 1e-9, "{radius}"),
        other => panic!("{other:?}"),
    }
    assert!(index(&world).skipped().contains(&(tiny_skew, SkipReason::NonUniformScale)));
    match index(&world).indexed_shape(large).expect("large uniform") {
        ColliderShape::Sphere { radius } => assert!((radius - 10_000.0).abs() < 1e-2, "{radius}"),
        other => panic!("{other:?}"),
    }
    assert!(index(&world).center(large_close).is_some(), "a 0.25 difference on a 10000 axis is uniform");
    assert!(index(&world).skipped().contains(&(large_skew, SkipReason::NonUniformScale)));
}

#[test]
fn a_free_query_ignores_the_stored_mask() {
    let mut world = synced();
    let hidden = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0).with_layers(1).with_mask(0));
    let peer = place(&mut world, Transform::from_xyz(8.0, 0.0, 0.0), Collider::sphere(1.0).with_layers(1));
    sync_collision(&mut world);
    let hits = index(&world).overlap(
        ColliderShape::Sphere { radius: 1.0 },
        &Transform::IDENTITY,
        CollisionQuery::all().mask(1),
    );
    assert!(hits.contains(&hidden));
    let hidden_collider = world.component::<Collider>(hidden).unwrap().clone();
    let peer_collider = world.component::<Collider>(peer).unwrap().clone();
    assert!(!pair_interacts(hidden, &hidden_collider, peer, &peer_collider));
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

#[test]
fn overlap_covers_every_shape_pair_and_ignores_clear_separation() {
    let cases = [
        (ColliderShape::Sphere { radius: 1.0 }, Transform::from_xyz(1.5, 0.0, 0.0), true),
        (ColliderShape::Box { half_extents: Vec3::splat(0.5) }, Transform::from_xyz(1.2, 0.0, 0.0), true),
        (ColliderShape::Capsule { radius: 0.4, half_height: 0.5 }, Transform::from_xyz(1.2, 0.0, 0.0), true),
        (ColliderShape::Sphere { radius: 1.0 }, Transform::from_xyz(4.0, 0.0, 0.0), false),
    ];
    for (probe, pose, expect) in cases {
        let mut world = synced();
        let target = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0));
        sync_collision(&mut world);
        let hits = shape_at(&world, probe, pose);
        assert_eq!(hits.contains(&target), expect, "{probe:?} at {pose:?}");
    }

    let pairs = [
        (Collider::aabb_box(Vec3::splat(0.5)), ColliderShape::Box { half_extents: Vec3::splat(0.5) }),
        (Collider::aabb_box(Vec3::splat(0.5)), ColliderShape::Capsule { radius: 0.4, half_height: 0.4 }),
        (Collider::capsule(0.4, 0.4), ColliderShape::Capsule { radius: 0.4, half_height: 0.4 }),
    ];
    for (indexed, probe) in pairs {
        let mut world = synced();
        let target = place(&mut world, Transform::IDENTITY, indexed);
        sync_collision(&mut world);
        assert_eq!(shape_at(&world, probe, Transform::from_xyz(0.4, 0.0, 0.0)), vec![target]);
        assert!(shape_at(&world, probe, Transform::from_xyz(5.0, 0.0, 0.0)).is_empty());
    }
}

#[test]
fn a_rotated_box_overlaps_along_its_long_axis_only() {
    let mut world = synced();
    let long = place(
        &mut world,
        Transform::IDENTITY.with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        Collider::aabb_box(Vec3::new(2.0, 0.1, 0.1)),
    );
    sync_collision(&mut world);
    let sphere = ColliderShape::Sphere { radius: 0.2 };
    assert_eq!(shape_at(&world, sphere, Transform::from_xyz(0.0, 0.0, 1.5)), vec![long]);
    assert!(shape_at(&world, sphere, Transform::from_xyz(1.5, 0.0, 0.0)).is_empty());
}

#[test]
fn negative_coordinates_touching_shells_and_z_separation() {
    let mut world = synced();
    let there = place(&mut world, Transform::from_xyz(-4.0, -2.0, -1.0), Collider::sphere(1.0));
    let touching = place(&mut world, Transform::from_xyz(2.0, 0.0, 0.0), Collider::sphere(1.0));
    let origin = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0));
    let raised = place(&mut world, Transform::from_xyz(0.0, 0.0, 5.0), Collider::sphere(1.0));
    sync_collision(&mut world);

    let sphere = ColliderShape::Sphere { radius: 1.0 };
    assert_eq!(shape_at(&world, sphere, Transform::from_xyz(-4.0, -2.0, -1.0)), vec![there]);
    assert!(shape_at(&world, sphere, Transform::IDENTITY).contains(&touching));
    assert!(!shape_at(&world, sphere, Transform::IDENTITY).contains(&raised));
    assert!(shape_at(&world, sphere, Transform::from_xyz(0.0, 0.0, 5.0)).contains(&raised));
    assert!(!shape_at(&world, sphere, Transform::from_xyz(0.0, 0.0, 5.0)).contains(&origin));
}

#[test]
fn filters_exclude_self_sensors_disabled_and_unrelated_layers() {
    let mut world = synced();
    let solid = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0));
    let sensor = place(&mut world, Transform::from_xyz(0.2, 0.0, 0.0), Collider::sphere(1.0).sensor());
    let other = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0).with_layers(1 << 3));
    let off = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0).disabled());
    sync_collision(&mut world);

    let pose = Transform::IDENTITY;
    let sphere = ColliderShape::Sphere { radius: 1.0 };
    let all = index(&world).overlap(sphere, &pose, CollisionQuery::all());
    assert_eq!(all, sorted(&[solid, sensor, other]));
    assert!(!all.contains(&off));

    let solids = index(&world).overlap(sphere, &pose, CollisionQuery::all().solids_only());
    assert_eq!(solids, sorted(&[solid, other]));

    let excluded = index(&world).overlap(sphere, &pose, CollisionQuery::all().exclude(solid));
    assert!(!excluded.contains(&solid));
    assert!(excluded.contains(&sensor));

    let masked = index(&world).overlap(sphere, &pose, CollisionQuery::all().mask(1));
    assert!(masked.contains(&solid));
    assert!(!masked.contains(&other));

    let a = Collider::sphere(1.0).with_layers(1).with_mask(0b10);
    let b = Collider::sphere(1.0).with_layers(0b10).with_mask(1);
    let c = Collider::sphere(1.0).with_layers(1).with_mask(1);
    assert!(pair_interacts(solid, &a, sensor, &b));
    assert!(!pair_interacts(solid, &a, sensor, &c));
    assert!(!pair_interacts(solid, &a, solid, &a));
    assert!(!pair_interacts(solid, &a.disabled(), sensor, &b));
}

#[test]
fn parented_shapes_overlap_in_world_space() {
    let mut world = synced();
    let parent = world.spawn((Transform::from_xyz(10.0, 0.0, 0.0).with_uniform_scale(2.0),));
    let child = world.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0), Collider::sphere(0.5)));
    sync_collision(&mut world);
    let hits = shape_at(&world, ColliderShape::Sphere { radius: 0.5 }, Transform::from_xyz(12.0, 0.0, 0.0));
    assert_eq!(hits, vec![child]);
    assert!(shape_at(&world, ColliderShape::Sphere { radius: 0.5 }, Transform::from_xyz(1.0, 0.0, 0.0)).is_empty());
}

#[test]
fn overlap_order_is_entity_order_and_repeats() {
    let mut world = synced();
    let first = place(&mut world, Transform::from_xyz(0.2, 0.0, 0.0), Collider::sphere(1.0));
    let second = place(&mut world, Transform::from_xyz(-0.2, 0.0, 0.0), Collider::sphere(1.0));
    sync_collision(&mut world);
    let once = shape_at(&world, ColliderShape::Sphere { radius: 1.0 }, Transform::IDENTITY);
    let twice = shape_at(&world, ColliderShape::Sphere { radius: 1.0 }, Transform::IDENTITY);
    assert_eq!(once, sorted(&[first, second]));
    assert_eq!(once, twice);
    assert_eq!(index(&world).entities(), sorted(&[first, second]));
}

#[test]
fn rays_return_world_distance_point_and_normal() {
    let mut world = synced();
    let sphere = place(&mut world, Transform::IDENTITY, Collider::sphere(1.0));
    sync_collision(&mut world);
    let hit = index(&world).ray(Vec3::new(-5.0, 0.0, 0.0), Vec3::X, 10.0, CollisionQuery::all()).unwrap();
    assert_eq!(hit.entity, sphere);
    assert!((hit.distance - 4.0).abs() < EPS);
    near(hit.point, Vec3::new(-1.0, 0.0, 0.0));
    near(hit.normal, Vec3::new(-1.0, 0.0, 0.0));
}

#[test]
fn a_rotated_box_ray_normal_is_in_world_space() {
    let mut world = synced();
    let entity = place(
        &mut world,
        Transform::IDENTITY.with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        Collider::aabb_box(Vec3::new(1.0, 1.0, 0.1)),
    );
    sync_collision(&mut world);
    let hit = index(&world).ray(Vec3::new(-5.0, 0.0, 0.0), Vec3::X, 10.0, CollisionQuery::all()).unwrap();
    assert_eq!(hit.entity, entity);
    assert!((hit.distance - 4.9).abs() < EPS, "{}", hit.distance);
    near(hit.point, Vec3::new(-0.1, 0.0, 0.0));
    near(hit.normal, Vec3::new(-1.0, 0.0, 0.0));
}

#[test]
fn a_parent_transform_moves_the_ray_hit() {
    let mut world = synced();
    let parent = world.spawn((Transform::from_xyz(10.0, 0.0, 0.0),));
    let child = world.spawn_child(parent, (Transform::IDENTITY, Collider::sphere(1.0)));
    sync_collision(&mut world);
    let hit = index(&world).ray(Vec3::ZERO, Vec3::X, 20.0, CollisionQuery::all()).unwrap();
    assert_eq!(hit.entity, child);
    assert!((hit.distance - 9.0).abs() < EPS);
    near(hit.point, Vec3::new(9.0, 0.0, 0.0));
    near(hit.normal, Vec3::new(-1.0, 0.0, 0.0));
}

#[test]
fn invalid_rays_miss_and_the_nearer_hit_wins() {
    let mut world = synced();
    let near_one = place(&mut world, Transform::from_xyz(3.0, 0.0, 0.0), Collider::sphere(1.0));
    let far = place(&mut world, Transform::from_xyz(8.0, 0.0, 0.0), Collider::sphere(1.0));
    sync_collision(&mut world);
    let collision = index(&world);
    assert!(collision.ray(Vec3::ZERO, Vec3::ZERO, 10.0, CollisionQuery::all()).is_none());
    assert!(collision.ray(Vec3::ZERO, Vec3::X, -1.0, CollisionQuery::all()).is_none());
    assert!(collision.ray(Vec3::new(f32::NAN, 0.0, 0.0), Vec3::X, 10.0, CollisionQuery::all()).is_none());
    let hit = collision.ray(Vec3::ZERO, Vec3::X, 20.0, CollisionQuery::all()).unwrap();
    assert_eq!(hit.entity, near_one);
    assert!(hit.distance < 8.0);
    let _ = far;
}

#[test]
fn equal_distance_rays_pick_the_lower_entity() {
    let mut world = synced();
    let first = place(&mut world, Transform::from_xyz(5.0, 0.0, 0.0), Collider::sphere(1.0));
    let second = place(&mut world, Transform::from_xyz(5.0, 0.0, 0.0), Collider::sphere(1.0));
    sync_collision(&mut world);
    let hit = index(&world).ray(Vec3::ZERO, Vec3::X, 20.0, CollisionQuery::all()).unwrap();
    assert!(first < second);
    assert_eq!(hit.entity, first);
    let again = index(&world).ray(Vec3::ZERO, Vec3::X, 20.0, CollisionQuery::all()).unwrap();
    assert_eq!(again.entity, hit.entity);
    assert!((again.distance - hit.distance).abs() < 1e-5);
}

#[test]
fn ray_filters_match_overlap_filters() {
    let mut world = synced();
    let solid = place(&mut world, Transform::from_xyz(4.0, 0.0, 0.0), Collider::sphere(1.0));
    let sensor = place(&mut world, Transform::from_xyz(4.0, 0.0, 0.0), Collider::sphere(1.0).sensor());
    let hidden = place(&mut world, Transform::from_xyz(4.0, 0.0, 0.0), Collider::sphere(1.0).with_layers(1 << 2));
    let off = place(&mut world, Transform::from_xyz(4.0, 0.0, 0.0), Collider::sphere(1.0).disabled());
    sync_collision(&mut world);
    let collision = index(&world);
    let without_solid = collision.ray(Vec3::ZERO, Vec3::X, 10.0, CollisionQuery::all().exclude(solid)).unwrap();
    assert_eq!(without_solid.entity, sensor.min(hidden));
    assert_ne!(without_solid.entity, off);
    let only_hidden =
        collision.ray(Vec3::ZERO, Vec3::X, 10.0, CollisionQuery::all().solids_only().exclude(solid)).unwrap();
    assert_eq!(only_hidden.entity, hidden);
    assert!(collision
        .ray(Vec3::ZERO, Vec3::X, 10.0, CollisionQuery::all().mask(1).exclude(solid).solids_only())
        .is_none());
}

#[test]
fn wire_segments_are_finite_for_every_indexed_shape() {
    let mut world = synced();
    place(&mut world, Transform::from_xyz(1.0, 2.0, 3.0), Collider::sphere(0.5));
    place(
        &mut world,
        Transform::IDENTITY.with_rotation(Quat::from_rotation_y(0.4)).with_scale(Vec3::new(1.0, 2.0, 0.5)),
        Collider::aabb_box(Vec3::splat(0.3)),
    );
    place(&mut world, Transform::from_xyz(0.0, 1.0, 0.0).with_uniform_scale(2.0), Collider::capsule(0.2, 0.4));
    place(&mut world, Transform::IDENTITY, Collider::sphere(1.0).disabled());
    sync_collision(&mut world);
    let segments = index(&world).wire_segments();
    assert!(segments.len() > 12);
    for (start, end) in segments {
        assert!(start.x.is_finite() && start.y.is_finite() && start.z.is_finite());
        assert!(end.x.is_finite() && end.y.is_finite() && end.z.is_finite());
    }
}

fn sorted(entities: &[Entity]) -> Vec<Entity> {
    let mut entities = entities.to_vec();
    entities.sort();
    entities
}
