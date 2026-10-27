//! Kinematic movement. No window and no GPU.

use engine_math::{Quat, Transform, Vec3};
use engine_world::{
    register_world_components, sync_collision, CharacterMotor, Collider, CollisionWorld, ComponentRegistry, MoveIntent,
    MoveResult, MoveStatus, Persistence, World,
};

const EPS: f32 = 1e-4;

fn fresh() -> World {
    let mut world = World::new();
    sync_collision(&mut world);
    world
}

fn motor(skin: f32) -> CharacterMotor {
    CharacterMotor { skin, ..CharacterMotor::default() }
}

fn slide(world: &mut World, entity: engine_world::Entity, translation: Vec3) -> MoveResult {
    world.insert(entity, (MoveIntent::new(translation),)).unwrap();
    sync_collision(world);
    (*world.component::<MoveResult>(entity).unwrap()).clone()
}

fn position(world: &World, entity: engine_world::Entity) -> Vec3 {
    world.component::<Transform>(entity).unwrap().position
}

fn near(actual: Vec3, expected: Vec3) {
    assert!(actual.approx_eq(expected, EPS), "actual {actual:?} expected {expected:?}");
}

fn center(world: &World, entity: engine_world::Entity) -> Vec3 {
    world.resource::<CollisionWorld>().center(entity).unwrap()
}

#[test]
fn motor_defaults_are_persistent_and_intent_is_runtime() {
    let motor = CharacterMotor::default();
    assert!((motor.skin - 0.01).abs() < EPS);
    assert_eq!(motor.max_slides, 4);
    assert_eq!(motor.locked_axes, engine_math::BVec3::FALSE);

    let world = World::new();
    register_world_components(&mut world.resource_mut::<ComponentRegistry>()).unwrap();
    let registry = world.resource::<ComponentRegistry>();
    let motor_info = registry.get::<CharacterMotor>().unwrap();
    assert_eq!(motor_info.persistence, Persistence::Persist);
    assert_eq!(registry.get::<MoveIntent>().unwrap().persistence, Persistence::Runtime);
    assert_eq!(registry.get::<MoveResult>().unwrap().persistence, Persistence::Runtime);
}

#[test]
fn an_open_move_applies_xyz_in_both_directions() {
    let mut world = fresh();
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), CharacterMotor::default()));
    let result = slide(&mut world, entity, Vec3::new(1.0, 2.0, 3.0));
    near(position(&world, entity), Vec3::new(1.0, 2.0, 3.0));
    near(center(&world, entity), Vec3::new(1.0, 2.0, 3.0));
    near(result.applied, Vec3::new(1.0, 2.0, 3.0));
    near(result.requested, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(result.status, MoveStatus::Completed);
    assert!(result.normals.is_empty());
    assert!(world.component::<MoveIntent>(entity).is_err());

    let back = slide(&mut world, entity, Vec3::new(-1.0, -2.0, -3.0));
    near(position(&world, entity), Vec3::ZERO);
    assert_eq!(back.status, MoveStatus::Completed);
    let transform = world.component::<Transform>(entity).unwrap();
    assert_eq!(transform.rotation, Quat::IDENTITY);
    assert_eq!(transform.scale, Vec3::ONE);
}

#[test]
fn a_diagonal_slide_does_not_reuse_the_consumed_motion() {
    let mut world = fresh();
    let wall = world.spawn((Transform::from_xyz(5.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(5.0, 5.0, 20.0))));
    let entity = world.spawn((Transform::from_xyz(-2.5, 0.0, 0.0), Collider::sphere(0.5), motor(0.0)));
    let result = slide(&mut world, entity, Vec3::new(4.0, 0.0, 4.0));
    near(position(&world, entity), Vec3::new(-0.5, 0.0, 4.0));
    near(result.applied, Vec3::new(2.0, 0.0, 4.0));
    assert!(result.blocked());
    assert!(!result.rejected());
    assert!(result.normals[0].approx_eq(-Vec3::X, 1e-3), "normal {:?}", result.normals);
    assert!(position(&world, entity).z < 5.0, "slide reused motion and ran past the request");
    let _ = wall;
}

#[test]
fn a_thin_wall_beyond_the_start_bounds_still_stops_the_sweep() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(5.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(0.05, 2.0, 2.0))));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.01)));
    let result = slide(&mut world, entity, Vec3::new(20.0, 0.0, 0.0));
    near(position(&world, entity), Vec3::new(4.44, 0.0, 0.0));
    assert!(result.blocked());
    assert!(result.applied.x < 5.0);
}

#[test]
fn two_walls_stop_the_mover_in_the_corner() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(6.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(5.0, 5.0, 20.0))));
    world.spawn((Transform::from_xyz(0.0, 0.0, 6.0), Collider::aabb_box(Vec3::new(20.0, 5.0, 5.0))));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.0)));
    let result = slide(&mut world, entity, Vec3::new(2.0, 0.0, 2.0));
    near(position(&world, entity), Vec3::new(0.5, 0.0, 0.5));
    assert!(result.blocked());
    assert!(result.normals.len() >= 2, "normals {:?}", result.normals);
}

#[test]
fn a_separating_skin_contact_does_not_hide_the_next_wall() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(-5.51, 0.0, 0.0), Collider::aabb_box(Vec3::splat(5.0))));
    world.spawn((Transform::from_xyz(7.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(5.0))));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.01)));
    let result = slide(&mut world, entity, Vec3::new(5.0, 0.0, 0.0));
    near(position(&world, entity), Vec3::new(1.49, 0.0, 0.0));
    assert!(result.blocked());
    assert!(result.applied.x < 4.0, "the ignored skin contact consumed the whole move");
}

#[test]
fn equal_time_contacts_keep_the_lower_entity() {
    // Two spheres sit at the same distance. Spawning order decides which normal is used.
    let high_first = tie_normal(true);
    let low_first = tie_normal(false);
    assert!(high_first < -0.1, "high obstacle should win, normal.y {high_first}");
    assert!(low_first > 0.1, "low obstacle should win, normal.y {low_first}");
}

fn tie_normal(high_first: bool) -> f32 {
    let mut world = fresh();
    let high = Transform::from_xyz(2.0, 0.3, 0.0);
    let low = Transform::from_xyz(2.0, -0.3, 0.0);
    let (first_pose, second_pose) = if high_first { (high, low) } else { (low, high) };
    let first = world.spawn((first_pose, Collider::sphere(0.5)));
    let second = world.spawn((second_pose, Collider::sphere(0.5)));
    assert!(first < second);
    let mover = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.0)));
    let result = slide(&mut world, mover, Vec3::new(3.0, 0.0, 0.0));
    result.normals[0].y
}

#[test]
fn penetration_pushes_once_then_the_request_stops_on_the_surface() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(5.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(5.0))));
    let entity = world.spawn((Transform::from_xyz(0.3, 0.0, 0.0), Collider::sphere(0.5), motor(0.01)));
    let result = slide(&mut world, entity, Vec3::X);
    near(position(&world, entity), Vec3::new(-0.51, 0.0, 0.0));
    near(result.applied, Vec3::new(-0.81, 0.0, 0.0));
    assert!(result.blocked());
    let recovery = result.recovery_normal.expect("recovery");
    assert!(recovery.approx_eq(-Vec3::X, 1e-3), "recovery {recovery:?}");
}

#[test]
fn a_zero_request_does_not_depenetrate() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(5.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(5.0))));
    let entity = world.spawn((Transform::from_xyz(0.3, 0.0, 0.0), Collider::sphere(0.5), CharacterMotor::default()));
    let result = slide(&mut world, entity, Vec3::ZERO);
    near(position(&world, entity), Vec3::new(0.3, 0.0, 0.0));
    assert_eq!(result.status, MoveStatus::Completed);
    assert!(result.normals.is_empty());
    assert!(result.recovery_normal.is_none());
}

#[test]
fn exhausting_the_slide_budget_keeps_the_last_finite_position() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(5.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(5.0, 5.0, 20.0))));
    let entity = world.spawn((
        Transform::from_xyz(-2.5, 0.0, 0.0),
        Collider::sphere(0.5),
        CharacterMotor { skin: 0.0, max_slides: 1, ..CharacterMotor::default() },
    ));
    let result = slide(&mut world, entity, Vec3::new(4.0, 0.0, 4.0));
    near(position(&world, entity), Vec3::new(-0.5, 0.0, 2.0));
    assert!(result.blocked());
    assert!(!result.rejected());
}

#[test]
fn zero_slides_consumes_the_intent_and_does_not_move() {
    let mut world = fresh();
    let entity = world.spawn((
        Transform::from_xyz(1.0, 2.0, 3.0),
        Collider::sphere(0.5),
        CharacterMotor { max_slides: 0, ..CharacterMotor::default() },
    ));
    let result = slide(&mut world, entity, Vec3::X);
    near(position(&world, entity), Vec3::new(1.0, 2.0, 3.0));
    assert!(result.blocked());
    assert!(!result.rejected());
    assert!(result.normals.is_empty());
    near(result.applied, Vec3::ZERO);
    assert!(world.component::<MoveIntent>(entity).is_err());
}

#[test]
fn a_rotated_scaled_parent_converts_a_world_delta_into_local_position() {
    let mut world = fresh();
    let parent = world.spawn((Transform {
        position: Vec3::new(10.0, 0.0, 5.0),
        rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        scale: Vec3::splat(2.0),
    },));
    let child = world.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0), Collider::sphere(0.2), motor(0.0)));
    sync_collision(&mut world);
    near(world.component::<engine_world::GlobalTransform>(child).unwrap().translation(), Vec3::new(10.0, 0.0, 3.0));
    let result = slide(&mut world, child, Vec3::X);
    near(position(&world, child), Vec3::new(1.0, 0.0, 0.5));
    near(result.applied, Vec3::X);
    assert_eq!(result.status, MoveStatus::Completed);
    engine_world::propagate_transforms(&mut world);
    near(world.component::<engine_world::GlobalTransform>(child).unwrap().translation(), Vec3::new(11.0, 0.0, 3.0));
    near(center(&world, child), Vec3::new(11.0, 0.0, 3.0));
}

#[test]
fn signed_scale_keeps_the_offset_and_moves_the_world_center() {
    let mut world = fresh();
    let entity = world.spawn((
        Transform::IDENTITY.with_scale(Vec3::new(-2.0, 1.0, 1.0)),
        Collider::aabb_box(Vec3::splat(0.25)).with_offset(Vec3::X),
        motor(0.0),
    ));
    sync_collision(&mut world);
    near(center(&world, entity), Vec3::new(-2.0, 0.0, 0.0));
    slide(&mut world, entity, Vec3::X);
    near(position(&world, entity), Vec3::X);
    near(center(&world, entity), Vec3::new(-1.0, 0.0, 0.0));
}

#[test]
fn locking_world_y_holds_under_a_rotated_parent() {
    let mut world = fresh();
    let parent =
        world.spawn((Transform::IDENTITY.with_rotation(Quat::from_axis_angle(Vec3::Z, std::f32::consts::FRAC_PI_2)),));
    let child = world.spawn_child(
        parent,
        (
            Transform::IDENTITY,
            Collider::sphere(0.2),
            CharacterMotor {
                skin: 0.0,
                locked_axes: engine_math::BVec3::new(false, true, false),
                ..CharacterMotor::default()
            },
        ),
    );
    sync_collision(&mut world);
    let before = world.component::<engine_world::GlobalTransform>(child).unwrap().translation();
    let result = slide(&mut world, child, Vec3::new(1.0, 2.0, 0.0));
    assert!(result.applied.y.abs() < EPS, "applied {:?}", result.applied);
    near(result.requested, Vec3::new(1.0, 2.0, 0.0));
    engine_world::propagate_transforms(&mut world);
    let after = world.component::<engine_world::GlobalTransform>(child).unwrap().translation();
    assert!((after.y - before.y).abs() < EPS, "before {before:?} after {after:?}");
    assert!(after.x > 0.5, "after {after:?}");
}

#[test]
fn locking_z_is_a_side_scroll_constraint_and_an_open_motor_keeps_y() {
    let mut world = fresh();
    let locked = world.spawn((
        Transform::IDENTITY,
        Collider::sphere(0.2),
        CharacterMotor {
            skin: 0.0,
            locked_axes: engine_math::BVec3::new(false, false, true),
            ..CharacterMotor::default()
        },
    ));
    let result = slide(&mut world, locked, Vec3::new(1.0, 2.0, 3.0));
    near(position(&world, locked), Vec3::new(1.0, 2.0, 0.0));
    near(result.requested, Vec3::new(1.0, 2.0, 3.0));

    let open = world.spawn((Transform::from_xyz(0.0, 0.0, 5.0), Collider::sphere(0.2), motor(0.0)));
    slide(&mut world, open, Vec3::new(1.0, 0.5, 1.0));
    near(position(&world, open), Vec3::new(1.0, 0.5, 6.0));
    slide(&mut world, open, Vec3::new(0.0, 2.0, 0.0));
    near(position(&world, open), Vec3::new(1.0, 2.5, 6.0));
}

#[test]
fn an_indexed_descendant_rejects_the_ancestor_move() {
    let mut world = fresh();
    let parent = world.spawn((Transform::IDENTITY, Collider::sphere(0.4), motor(0.0)));
    world.spawn_child(parent, (Transform::from_xyz(0.0, 1.0, 0.0), Collider::sphere(0.2)));
    let result = slide(&mut world, parent, Vec3::X);
    near(position(&world, parent), Vec3::ZERO);
    assert!(result.rejected());
    assert!(!result.blocked());
    near(center(&world, parent), Vec3::ZERO);
}

#[test]
fn a_disabled_descendant_does_not_reject_the_ancestor() {
    let mut world = fresh();
    let parent = world.spawn((Transform::IDENTITY, Collider::sphere(0.4), motor(0.0)));
    world.spawn_child(parent, (Transform::from_xyz(0.0, 1.0, 0.0), Collider::sphere(0.2).disabled()));
    let result = slide(&mut world, parent, Vec3::X);
    near(position(&world, parent), Vec3::X);
    assert_eq!(result.status, MoveStatus::Completed);
}

#[test]
fn a_collapsed_parent_rejects_the_move() {
    let mut world = fresh();
    let parent = world.spawn((Transform::IDENTITY.with_scale(Vec3::new(0.0, 1.0, 1.0)),));
    let child = world.spawn_child(parent, (Transform::IDENTITY, Collider::sphere(0.2), motor(0.0)));
    let result = slide(&mut world, child, Vec3::X);
    near(position(&world, child), Vec3::ZERO);
    assert!(result.rejected());
    assert!(center_missing(&world, child));
}

#[test]
fn the_later_motor_sees_the_earlier_motors_patched_collider() {
    let mut world = fresh();
    let first = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.0)));
    let second = world.spawn((Transform::from_xyz(3.0, 0.0, 0.0), Collider::sphere(0.5), motor(0.0)));
    assert!(first < second);
    world.insert(first, (MoveIntent::new(Vec3::new(1.5, 0.0, 0.0)),)).unwrap();
    world.insert(second, (MoveIntent::new(Vec3::new(-1.5, 0.0, 0.0)),)).unwrap();
    sync_collision(&mut world);
    near(position(&world, first), Vec3::new(1.5, 0.0, 0.0));
    near(position(&world, second), Vec3::new(2.5, 0.0, 0.0));
    near(center(&world, first), Vec3::new(1.5, 0.0, 0.0));
    let second_result = (*world.component::<MoveResult>(second).unwrap()).clone();
    assert!(second_result.blocked());
}

#[test]
fn masks_and_sensors_do_not_block() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(0.5)).with_layers(1).with_mask(0)));
    let masked = world.spawn((Transform::IDENTITY, Collider::sphere(0.4).with_layers(1).with_mask(1), motor(0.0)));
    slide(&mut world, masked, Vec3::new(5.0, 0.0, 0.0));
    near(position(&world, masked), Vec3::new(5.0, 0.0, 0.0));

    world.spawn((Transform::from_xyz(2.0, 3.0, 0.0), Collider::aabb_box(Vec3::splat(0.5)).sensor()));
    let sensing = world.spawn((Transform::from_xyz(0.0, 3.0, 0.0), Collider::sphere(0.4), motor(0.0)));
    slide(&mut world, sensing, Vec3::new(5.0, 0.0, 0.0));
    near(position(&world, sensing), Vec3::new(5.0, 3.0, 0.0));
}

#[test]
fn separated_z_volumes_do_not_interact() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(1.0, 0.0, 5.0), Collider::sphere(0.5)));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.0)));
    slide(&mut world, entity, Vec3::new(3.0, 0.0, 0.0));
    near(position(&world, entity), Vec3::new(3.0, 0.0, 0.0));
}

#[test]
fn invalid_parameters_missing_colliders_and_bad_intents_reject() {
    let mut world = fresh();
    let bad_skin = world.spawn((
        Transform::IDENTITY,
        Collider::sphere(0.5),
        CharacterMotor { skin: -0.1, ..CharacterMotor::default() },
    ));
    let result = slide(&mut world, bad_skin, Vec3::X);
    assert!(result.rejected());
    near(position(&world, bad_skin), Vec3::ZERO);

    let infinite = world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Collider::sphere(0.5), motor(0.0)));
    let result = slide(&mut world, infinite, Vec3::new(f32::INFINITY, 0.0, 0.0));
    assert!(result.rejected());
    near(position(&world, infinite), Vec3::new(2.0, 0.0, 0.0));
    near(result.applied, Vec3::ZERO);

    let naked = world.spawn((Transform::from_xyz(4.0, 0.0, 0.0), motor(0.0)));
    let result = slide(&mut world, naked, Vec3::X);
    assert!(result.rejected());
    near(position(&world, naked), Vec3::new(4.0, 0.0, 0.0));

    let disabled = world.spawn((Transform::IDENTITY, Collider::sphere(0.5).disabled(), motor(0.0)));
    assert!(slide(&mut world, disabled, Vec3::X).rejected());
    near(position(&world, disabled), Vec3::ZERO);
}

#[test]
fn a_sheared_collider_is_rejected_and_not_indexed() {
    let mut world = fresh();
    let parent = world.spawn((Transform::IDENTITY.with_scale(Vec3::new(2.0, 1.0, 1.0)),));
    let child = world.spawn_child(
        parent,
        (
            Transform::IDENTITY.with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
            Collider::aabb_box(Vec3::splat(0.3)),
            motor(0.0),
        ),
    );
    let result = slide(&mut world, child, Vec3::X);
    assert!(result.rejected());
    near(position(&world, child), Vec3::ZERO);
    assert!(center_missing(&world, child));
}

#[test]
fn a_missing_intent_leaves_the_previous_result_and_a_new_one_replaces_it() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(0.5))));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.4), motor(0.0)));
    let blocked = slide(&mut world, entity, Vec3::new(5.0, 0.0, 0.0));
    assert!(blocked.blocked());
    assert!(!blocked.normals.is_empty());
    let stored = blocked.clone();
    sync_collision(&mut world);
    assert_eq!((*world.component::<MoveResult>(entity).unwrap()).clone(), stored);

    let fresh_result = slide(&mut world, entity, Vec3::ZERO);
    assert_eq!(fresh_result.status, MoveStatus::Completed);
    assert!(fresh_result.normals.is_empty());
    assert!(fresh_result.recovery_normal.is_none());
}

#[test]
fn repeated_slides_hash_the_same_bits_across_two_runs() {
    let first = hash_run();
    let second = hash_run();
    assert_eq!(first, second);
}

fn hash_run() -> u64 {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Collider::sphere(0.5)));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.0)));
    let mut hash = 0u64;
    for _ in 0..1_000 {
        slide(&mut world, entity, Vec3::new(1.5, 0.0, 0.0));
        hash = mix(hash, position(&world, entity));
    }
    let end = position(&world, entity);
    near(end, Vec3::X);
    hash
}

fn mix(hash: u64, position: Vec3) -> u64 {
    let mut hash = hash;
    for bits in [position.x.to_bits(), position.y.to_bits(), position.z.to_bits()] {
        hash = hash.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(bits as u64);
    }
    hash
}

fn center_missing(world: &World, entity: engine_world::Entity) -> bool {
    world.resource::<CollisionWorld>().center(entity).is_none()
}
