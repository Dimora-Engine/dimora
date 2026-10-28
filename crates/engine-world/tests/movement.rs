//! Kinematic movement. No window and no GPU.

use engine_math::{Quat, Transform, Vec3};
use engine_world::{
    register_world_components, sync_collision, CharacterMotor, Collider, ColliderShape, CollisionWorld,
    ComponentRegistry, MoveIntent, MoveResult, MoveStatus, Persistence, World,
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
    // The box face is x = 0. The sphere center starts at x = 0.3, so the overlap is 0.8.
    // The push is 0.81 along -X (overlap plus skin). Applied displacement is -0.81 and the
    // center finishes at 0.3 - 0.81 = -0.51. Those are one fixture, not two results.
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

fn spawn_face(world: &mut World, half_y: f32, half_z: f32) {
    let half_x = 2.0;
    world.spawn((Transform::from_xyz(half_x, 0.0, 0.0), Collider::aabb_box(Vec3::new(half_x, half_y, half_z))));
}

fn assert_moved(kind: &str, start: Vec3, motion: Vec3, end: Vec3, result: &MoveResult) {
    assert!(
        end.approx_eq(start + motion, EPS),
        "{kind} start {start:?} motion {motion:?} end {end:?} status {:?} normals {:?}",
        result.status,
        result.normals
    );
}

#[test]
fn tangent_requests_along_a_flat_wall_keep_their_length() {
    let motions = [Vec3::Y * 4.0, -Vec3::Y * 4.0, Vec3::Z * 4.0, -Vec3::Z * 4.0];
    for skin in [0.0, 0.01] {
        for half_z in [4.0, 8_000.0] {
            for motion in motions {
                let start = Vec3::new(-(0.5 + skin), 0.0, 0.0);
                let mut world = fresh();
                spawn_face(&mut world, 6.0, half_z);
                let sphere =
                    world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(0.5), motor(skin)));
                let result = slide(&mut world, sphere, motion);
                assert_moved("sphere", start, motion, position(&world, sphere), &result);
                assert!(result.normals.iter().all(|normal| normal.yz_flat()), "tilted {result:?}");

                let mut world = fresh();
                spawn_face(&mut world, 6.0, half_z);
                let capsule = world.spawn((
                    Transform::from_xyz(start.x, start.y, start.z),
                    Collider::capsule(0.3, 0.4),
                    motor(skin),
                ));
                let capsule_start = Vec3::new(-(0.3 + skin), 0.0, 0.0);
                world.component_mut::<Transform>(capsule).unwrap().position = capsule_start;
                let result = slide(&mut world, capsule, motion);
                assert_moved("capsule", capsule_start, motion, position(&world, capsule), &result);
            }
        }
    }
}

#[test]
fn a_capsule_lying_along_the_wall_stays_on_the_face() {
    let rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    for skin in [0.0, 0.01] {
        let start = Vec3::new(-(0.25 + skin), 0.0, 0.0);
        let mut world = fresh();
        spawn_face(&mut world, 4.0, 8_000.0);
        let entity = world.spawn((
            Transform::from_xyz(start.x, start.y, start.z).with_rotation(rotation),
            Collider::capsule(0.25, 1.5),
            motor(skin),
        ));
        let motion = Vec3::Z * 4.0;
        let result = slide(&mut world, entity, motion);
        assert_moved("horizontal capsule", start, motion, position(&world, entity), &result);
    }
}

#[test]
fn twenty_diagonal_steps_along_a_wall_do_not_drift_or_amplify() {
    let skin = 0.01;
    let start = Vec3::new(-(0.5 + skin), 1.0, 0.0);
    let mut world = fresh();
    spawn_face(&mut world, 4.0, 8_000.0);
    let entity = world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(0.5), motor(skin)));
    for _ in 0..20 {
        let result = slide(&mut world, entity, Vec3::new(4.0, 0.0, 4.0));
        assert!(!result.rejected());
        assert!(result.applied.length() <= 4.0 + EPS, "amplified {:?}", result.applied);
    }
    near(position(&world, entity), start + Vec3::new(0.0, 0.0, 80.0));
}

#[test]
fn into_and_away_requests_respect_the_skin_band() {
    for into in [0.001, 0.05] {
        let start = Vec3::new(-0.51, 0.0, 0.0);
        let mut world = fresh();
        spawn_face(&mut world, 4.0, 8.0);
        let entity = world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(0.5), motor(0.01)));
        let result = slide(&mut world, entity, Vec3::new(into, 0.0, 4.0));
        assert_moved("into", start, Vec3::new(0.0, 0.0, 4.0), position(&world, entity), &result);
        assert!(result.blocked());
        assert!(result.normals[0].approx_eq(-Vec3::X, 1e-3), "normal {:?}", result.normals);
    }

    let start = Vec3::new(-0.51, 0.0, 0.0);
    let mut world = fresh();
    spawn_face(&mut world, 4.0, 8.0);
    let entity = world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(0.5), motor(0.01)));
    let away = slide(&mut world, entity, Vec3::new(-0.05, 0.0, 4.0));
    assert_moved("away", start, Vec3::new(-0.05, 0.0, 4.0), position(&world, entity), &away);
    assert_eq!(away.status, MoveStatus::Completed);
}

#[test]
fn repeated_tiny_into_steps_stay_inside_the_tolerance() {
    let radius = 0.5;
    let start = Vec3::new(-radius, 0.0, 0.0);
    let mut world = fresh();
    spawn_face(&mut world, 2.0, 2.0);
    let entity = world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(radius), motor(0.0)));
    for _ in 0..40 {
        slide(&mut world, entity, Vec3::new(5e-5, 0.0, 0.0));
    }
    let end = position(&world, entity);
    let penetration = end.x - start.x;
    assert!(penetration < 2e-4, "40 steps of 5e-5 penetrated {penetration}");
    assert!(penetration > -2e-4, "recovery jumped by {penetration}");
    assert!(end.y.abs() < EPS && end.z.abs() < EPS, "end {end:?}");
}

#[test]
fn a_skin_wall_does_not_hide_the_wall_ahead() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(-2.51, 0.0, 0.0), Collider::aabb_box(Vec3::new(2.0, 3.0, 10.0))));
    world.spawn((Transform::from_xyz(0.0, 0.0, 4.0), Collider::aabb_box(Vec3::new(2.0, 2.0, 1.0))));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.01)));
    let result = slide(&mut world, entity, Vec3::new(0.0, 0.0, 4.0));
    near(position(&world, entity), Vec3::new(0.0, 0.0, 2.49));
    assert!(result.blocked());
    assert!(result.normals[0].approx_eq(-Vec3::Z, 1e-3), "normal {:?}", result.normals);
}

#[test]
fn several_near_skin_walls_still_stop_on_the_wall_ahead() {
    let mut world = fresh();
    world.spawn((Transform::from_xyz(-2.51, 0.0, 0.0), Collider::aabb_box(Vec3::new(2.0, 3.0, 10.0))));
    world.spawn((Transform::from_xyz(2.51, 0.0, 0.0), Collider::aabb_box(Vec3::new(2.0, 3.0, 10.0))));
    world.spawn((Transform::from_xyz(0.0, 2.51, 0.0), Collider::aabb_box(Vec3::new(3.0, 2.0, 10.0))));
    world.spawn((Transform::from_xyz(0.0, 0.0, 4.0), Collider::aabb_box(Vec3::new(2.0, 2.0, 1.0))));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(0.5), motor(0.01)));
    let result = slide(&mut world, entity, Vec3::Z * 4.0);
    near(position(&world, entity), Vec3::new(0.0, 0.0, 2.49));
    assert!(result.blocked());
    assert!(result.applied.x.abs() < EPS && result.applied.y.abs() < EPS, "drift {:?}", result.applied);
}

#[test]
fn supported_shape_pairs_slide_along_a_contact_and_reverse() {
    let skin = 0.01;
    let cases = [
        (Collider::sphere(0.4), 0.4, Collider::sphere(0.5), 0.5),
        (Collider::sphere(0.4), 0.4, Collider::capsule(0.3, 0.6), 0.3),
        (Collider::capsule(0.3, 0.5), 0.3, Collider::capsule(0.2, 0.7), 0.2),
        (Collider::sphere(0.4), 0.4, Collider::aabb_box(Vec3::new(0.5, 1.0, 1.5)), 0.5),
        (Collider::capsule(0.3, 0.4), 0.3, Collider::aabb_box(Vec3::new(0.6, 1.2, 2.0)), 0.6),
        (Collider::aabb_box(Vec3::new(0.4, 0.5, 0.4)), 0.4, Collider::aabb_box(Vec3::new(0.7, 1.0, 1.3)), 0.7),
    ];
    for (mover_shape, mover_extent, obstacle_shape, obstacle_extent) in cases {
        let motion = Vec3::Z * 4.0;
        let end = slide_separated(mover_shape, obstacle_shape, mover_extent, obstacle_extent, skin, motion);
        assert!(end.approx_eq(motion, EPS), "mover {mover_shape:?} end {end:?}");
        let reversed = slide_separated(obstacle_shape, mover_shape, obstacle_extent, mover_extent, skin, motion);
        assert!(reversed.approx_eq(motion, EPS), "reversed {obstacle_shape:?} end {reversed:?}");
    }
}

fn slide_separated(
    mover_shape: Collider,
    obstacle_shape: Collider,
    mover_extent: f32,
    obstacle_extent: f32,
    skin: f32,
    motion: Vec3,
) -> Vec3 {
    let mut world = fresh();
    let separation = mover_extent + obstacle_extent + skin;
    world.spawn((Transform::from_xyz(separation, 0.0, 0.0), obstacle_shape));
    let mover = world.spawn((Transform::IDENTITY, mover_shape, motor(skin)));
    slide(&mut world, mover, motion);
    position(&world, mover)
}

#[test]
fn a_rotated_wall_keeps_a_face_normal_for_tangent_and_into_motion() {
    let yaw = Quat::from_rotation_y(30.0_f32.to_radians());
    let outward = yaw.rotate(Vec3::X);
    let tangent = yaw.rotate(Vec3::Z);
    let radius = 0.5;
    let skin = 0.01;
    let half_x = 1.0;
    let box_center = -outward * (half_x + radius + skin);
    let mut world = fresh();
    world.spawn((
        Transform::from_xyz(box_center.x, box_center.y, box_center.z).with_rotation(yaw),
        Collider::aabb_box(Vec3::new(half_x, 3.0, 6.0)),
    ));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(radius), motor(skin)));
    let along = slide(&mut world, entity, tangent * 4.0);
    assert_moved("yaw tangent", Vec3::ZERO, tangent * 4.0, position(&world, entity), &along);

    let mut world = fresh();
    world.spawn((
        Transform::from_xyz(box_center.x, box_center.y, box_center.z).with_rotation(yaw),
        Collider::aabb_box(Vec3::new(half_x, 3.0, 6.0)),
    ));
    let entity = world.spawn((Transform::IDENTITY, Collider::sphere(radius), motor(skin)));
    let into = slide(&mut world, entity, -outward * 0.05 + tangent * 4.0);
    assert_moved("yaw into", Vec3::ZERO, tangent * 4.0, position(&world, entity), &into);
    assert!(into.normals[0].approx_eq(outward, 1e-3), "normal {:?}", into.normals);

    let mover_half = 0.4;
    let pair_center = -outward * (half_x + mover_half + skin);
    let mut world = fresh();
    world.spawn((
        Transform::from_xyz(pair_center.x, pair_center.y, pair_center.z).with_rotation(yaw),
        Collider::aabb_box(Vec3::new(half_x, 3.0, 6.0)),
    ));
    let boxes =
        world.spawn((Transform::IDENTITY.with_rotation(yaw), Collider::aabb_box(Vec3::splat(mover_half)), motor(skin)));
    let along = slide(&mut world, boxes, tangent * 4.0);
    assert_moved("yawed boxes", Vec3::ZERO, tangent * 4.0, position(&world, boxes), &along);
}

#[test]
fn a_parented_mover_slides_along_a_world_wall() {
    let start = Vec3::new(-0.51, 2.0, 1.0);
    let mut world = fresh();
    world.spawn((Transform::from_xyz(-3.02, 2.0, 1.0), Collider::aabb_box(Vec3::new(2.0, 4.0, 8.0))));
    let parent =
        world.spawn((Transform::from_xyz(start.x, start.y, start.z).with_rotation(Quat::from_rotation_y(0.7)),));
    let child = world.spawn_child(parent, (Transform::IDENTITY, Collider::sphere(0.5), motor(0.01)));
    let result = slide(&mut world, child, Vec3::Z * 4.0);
    near(center(&world, child), start + Vec3::Z * 4.0);
    assert_eq!(result.status, MoveStatus::Completed);
    assert!(result.applied.approx_eq(Vec3::Z * 4.0, EPS), "applied {:?}", result.applied);
}

#[test]
fn reflected_wall_and_mover_keep_a_tangent_slide() {
    let mut world = fresh();
    let wall = world.spawn((
        Transform::IDENTITY.with_scale(Vec3::new(-1.0, 1.0, 1.0)),
        Collider::aabb_box(Vec3::new(1.0, 2.0, 4.0)).with_offset(Vec3::X * 2.0),
    ));
    sync_collision(&mut world);
    let wall_center = center(&world, wall);
    let ColliderShape::Box { half_extents } = world.resource::<CollisionWorld>().indexed_shape(wall).unwrap() else {
        panic!("reflected box was not indexed");
    };
    let face = wall_center.x + half_extents.x;
    let start = Vec3::new(face + 0.5 + 0.01, 0.0, 0.0);
    let entity = world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(0.5), motor(0.01)));
    let result = slide(&mut world, entity, Vec3::Z * 4.0);
    assert_moved("reflected wall", start, Vec3::Z * 4.0, position(&world, entity), &result);

    let mut world = fresh();
    world.spawn((Transform::from_xyz(3.0, 0.0, 0.0), Collider::aabb_box(Vec3::new(2.0, 2.0, 4.0))));
    let mover = world.spawn((Transform::IDENTITY.with_scale(Vec3::splat(-2.0)), Collider::sphere(0.5), motor(0.0)));
    sync_collision(&mut world);
    let radius = match world.resource::<CollisionWorld>().indexed_shape(mover).unwrap() {
        ColliderShape::Sphere { radius } => radius,
        other => panic!("indexed {other:?}"),
    };
    let face = 1.0;
    let start = Vec3::new(face - radius, 0.0, 0.0);
    world.component_mut::<Transform>(mover).unwrap().position = start;
    let result = slide(&mut world, mover, Vec3::Z * 4.0);
    near(center(&world, mover), start + Vec3::Z * 4.0);
    assert!(result.applied.approx_eq(Vec3::Z * 4.0, EPS), "applied {:?}", result.applied);
}

#[test]
fn small_and_large_colliders_keep_a_tangent_slide() {
    for (radius, skin, half_z) in [(0.02, 0.0, 4.0), (0.02, 0.01, 4.0), (2.0, 0.01, 8_000.0)] {
        let start = Vec3::new(-(radius + skin), 0.0, 0.0);
        let mut world = fresh();
        spawn_face(&mut world, 6.0, half_z);
        let entity =
            world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(radius), motor(skin)));
        let result = slide(&mut world, entity, Vec3::Z * 4.0);
        assert_moved("size", start, Vec3::Z * 4.0, position(&world, entity), &result);
    }

    let mut world = fresh();
    spawn_face(&mut world, 6.0, 20.0);
    let entity =
        world.spawn((Transform::from_xyz(-1.5, 0.0, 0.0), Collider::aabb_box(Vec3::new(1.5, 0.4, 1.5)), motor(0.0)));
    let result = slide(&mut world, entity, Vec3::Z * 4.0);
    assert_moved("large box", Vec3::new(-1.5, 0.0, 0.0), Vec3::Z * 4.0, position(&world, entity), &result);
}

#[test]
fn a_finite_wall_does_not_trap_a_tangent_slide() {
    let start = Vec3::new(-0.5, 0.0, 0.0);
    let mut world = fresh();
    spawn_face(&mut world, 2.0, 0.5);
    let entity = world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(0.5), motor(0.0)));
    let result = slide(&mut world, entity, Vec3::Z * 4.0);
    assert_moved("past the end", start, Vec3::Z * 4.0, position(&world, entity), &result);
    assert_eq!(result.status, MoveStatus::Completed);
}

#[test]
fn corners_and_edges_keep_a_diagonal_normal() {
    let radius = 0.5;
    let corner = Vec3::new(1.0, -1.0, 1.0);
    let outward = Vec3::new(-1.0, -1.0, -1.0).normalized();
    let expected = corner + outward * radius;
    let start = corner + outward * (radius + 2.0);
    let mut world = fresh();
    world.spawn((Transform::from_xyz(2.0, 0.0, 2.0), Collider::aabb_box(Vec3::splat(1.0))));
    let entity = world.spawn((Transform::from_xyz(start.x, start.y, start.z), Collider::sphere(radius), motor(0.0)));
    let result = slide(&mut world, entity, -outward * 3.0);
    near(position(&world, entity), expected);
    assert!(result.normals[0].approx_eq(outward, 1e-3), "corner {:?}", result.normals);

    let edge = Vec3::new(1.0, 0.0, 1.0);
    let edge_direction = edge.normalized();
    let edge_stop = edge - edge_direction * 0.3;
    let edge_start = edge_stop - edge_direction * 2.0;
    let mut world = fresh();
    world.spawn((Transform::from_xyz(2.0, 0.0, 2.0), Collider::aabb_box(Vec3::splat(1.0))));
    let capsule = world.spawn((
        Transform::from_xyz(edge_start.x, edge_start.y, edge_start.z),
        Collider::capsule(0.3, 0.4),
        motor(0.0),
    ));
    let result = slide(&mut world, capsule, edge_direction * 3.0);
    near(position(&world, capsule), edge_stop);
    assert!(result.normals[0].approx_eq(-edge_direction, 1e-3), "edge {:?}", result.normals);
    assert!(result.normals[0].y.abs() < 1e-3, "edge gained height {:?}", result.normals);

    let sphere_at = Vec3::new(2.0, 0.0, 2.0);
    let toward_sphere = sphere_at.normalized();
    let support = 0.3 * std::f32::consts::SQRT_2;
    let box_stop = sphere_at - toward_sphere * (radius + support);
    let box_start = box_stop - toward_sphere * 2.0;
    let mut world = fresh();
    world.spawn((Transform::from_xyz(sphere_at.x, sphere_at.y, sphere_at.z), Collider::sphere(radius)));
    let box_mover = world.spawn((
        Transform::from_xyz(box_start.x, box_start.y, box_start.z),
        Collider::aabb_box(Vec3::splat(0.3)),
        motor(0.0),
    ));
    let result = slide(&mut world, box_mover, toward_sphere * 3.0);
    near(position(&world, box_mover), box_stop);
    assert!(result.normals[0].approx_eq(-toward_sphere, 1e-3), "box edge {:?}", result.normals);
}

trait FlatNormal {
    fn yz_flat(self) -> bool;
}

impl FlatNormal for Vec3 {
    fn yz_flat(self) -> bool {
        self.y.abs() < 1e-3 && self.z.abs() < 1e-3
    }
}
