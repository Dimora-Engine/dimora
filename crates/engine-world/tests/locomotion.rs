//! Ground support on a flat floor. These tests call the collision sync directly.
//!
//! A sphere of radius 0.5 with its center at y = 0.5 rests on a box whose top is y = 0. The
//! feature gap there is zero. Skin is 0.01 and stick is 0.08, so the downward probe reaches 0.09.
//! One fixed step is 1/60 s. Speed 90 writes a horizontal displacement of 1.5.

use engine_math::{Transform, Vec3};
use engine_world::{
    sync_collision, sync_collision_with_step, CharacterLocomotion, CharacterMotion, CharacterMotor, Collider,
    GroundSupport, LocomotionIntent, LocomotionPhase, MoveIntent, World,
};

const DT: f32 = 1.0 / 60.0;
const RADIUS: f32 = 0.5;
const REST_Y: f32 = 0.5;

/// One step still reaches the configured speed. The gameplay default accelerates.
fn kinematic() -> CharacterLocomotion {
    CharacterLocomotion {
        acceleration: 1.0e6,
        deceleration: 1.0e6,
        air_acceleration: 1.0e6,
        ..CharacterLocomotion::default()
    }
}

fn step(world: &mut World) {
    sync_collision_with_step(world, Some(DT));
}

fn floor(world: &mut World, center: Vec3, half: Vec3) -> engine_world::Entity {
    world.spawn((Transform::from_position(center), Collider::aabb_box(half)))
}

fn walker(world: &mut World, position: Vec3) -> engine_world::Entity {
    world.spawn((
        Transform::from_position(position),
        Collider::sphere(RADIUS),
        CharacterMotor::default(),
        kinematic(),
        CharacterMotion::default(),
    ))
}

fn flat_floor(world: &mut World) -> engine_world::Entity {
    floor(world, Vec3::new(0.0, -0.5, 0.0), Vec3::new(8.0, 0.5, 8.0))
}

fn position(world: &World, entity: engine_world::Entity) -> Vec3 {
    world.component::<Transform>(entity).unwrap().position
}

fn motion(world: &World, entity: engine_world::Entity) -> CharacterMotion {
    *world.component::<CharacterMotion>(entity).unwrap()
}

fn wish(world: &mut World, entity: engine_world::Entity, wish: Vec3) {
    world.insert(entity, (LocomotionIntent::new(wish),)).unwrap();
}

#[test]
fn first_step_on_a_flat_floor_is_supported() {
    let mut world = World::new();
    let ground = flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    match motion(&world, body).support {
        GroundSupport::Supported { entity, normal } => {
            assert_eq!(entity, ground);
            assert!((normal.y - 1.0).abs() < 1e-4, "{normal:?}");
        }
        other => panic!("expected support, got {other:?}"),
    }
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
    assert!((position(&world, body).y - REST_Y).abs() < 1e-4);
}

#[test]
fn a_supported_body_rests_for_a_thousand_steps() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    let settled = position(&world, body);
    for _ in 0..1000 {
        step(&mut world);
    }
    let end = position(&world, body);
    assert!((end.x - settled.x).abs() < 1e-4);
    assert!((end.y - settled.y).abs() < 1e-4);
    assert!((end.z - settled.z).abs() < 1e-4);
    assert!(matches!(motion(&world, body).support, GroundSupport::Supported { .. }));
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
}

#[test]
fn flat_ground_walks_along_x() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    let end = position(&world, body);
    assert!((end.x - 1.5).abs() < 1e-3, "{end:?}");
    assert!((end.y - REST_Y).abs() < 1e-3, "{end:?}");
    assert!(end.z.abs() < 1e-4);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
}

#[test]
fn a_diagonal_wish_does_not_go_faster_than_one_axis() {
    let mut straight = World::new();
    flat_floor(&mut straight);
    let along = walker(&mut straight, Vec3::new(0.0, REST_Y, 0.0));
    wish(&mut straight, along, Vec3::X);
    step(&mut straight);

    let mut diagonal = World::new();
    flat_floor(&mut diagonal);
    let body = walker(&mut diagonal, Vec3::new(0.0, REST_Y, 0.0));
    wish(&mut diagonal, body, Vec3::new(1.0, 4.0, 1.0));
    step(&mut diagonal);
    let end = position(&diagonal, body);
    let traveled = (end.x * end.x + end.z * end.z).sqrt();
    assert!((traveled - position(&straight, along).x).abs() < 1e-3, "{end:?}");
    assert!((end.x - end.z).abs() < 1e-3, "{end:?}");
    assert!((end.y - REST_Y).abs() < 1e-3);
}

#[test]
fn holding_a_wish_into_a_wall_stays_idle() {
    let mut world = World::new();
    flat_floor(&mut world);
    world.spawn((Transform::from_xyz(1.0, 1.0, 0.0), Collider::aabb_box(Vec3::new(0.5, 2.0, 2.0))));
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    let end = position(&world, body);
    assert!(end.x.abs() < 1e-3, "{end:?}");
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
    assert!(matches!(motion(&world, body).support, GroundSupport::Supported { .. }));
}

#[test]
fn walking_off_a_ledge_becomes_airborne_without_dropping() {
    let mut world = World::new();
    floor(&mut world, Vec3::new(0.0, -0.5, 0.0), Vec3::new(1.0, 0.5, 4.0));
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    let end = position(&world, body);
    assert!(end.x > 1.0, "{end:?}");
    assert!((end.y - REST_Y).abs() < 1e-2, "{end:?}");
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
}

#[test]
fn removing_the_floor_clears_support_on_the_next_step() {
    let mut world = World::new();
    let ground = flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    assert!(matches!(motion(&world, body).support, GroundSupport::Supported { .. }));
    world.despawn(ground).unwrap();
    step(&mut world);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
}

#[test]
fn a_sensor_floor_is_not_support() {
    let mut world = World::new();
    world.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(8.0, 0.5, 8.0)).sensor()));
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
}

#[test]
fn a_mask_mismatch_is_not_support() {
    let mut world = World::new();
    world.spawn((
        Transform::from_xyz(0.0, -0.5, 0.0),
        Collider::aabb_box(Vec3::new(8.0, 0.5, 8.0)).with_layers(0b10).with_mask(0b10),
    ));
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
}

#[test]
fn a_wall_or_a_ceiling_is_not_support() {
    let mut world = World::new();
    world.spawn((Transform::from_xyz(1.0, REST_Y, 0.0), Collider::aabb_box(Vec3::new(0.5, 2.0, 2.0))));
    world.spawn((Transform::from_xyz(0.0, 1.5, 0.0), Collider::aabb_box(Vec3::new(2.0, 0.5, 2.0))));
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
}

#[test]
fn equal_support_distance_keeps_the_lower_entity() {
    let mut world = World::new();
    let low = floor(&mut world, Vec3::new(0.0, -0.5, 0.0), Vec3::new(4.0, 0.5, 4.0));
    let high = floor(&mut world, Vec3::new(0.0, -0.5, 0.0), Vec3::new(4.0, 0.5, 4.0));
    assert!(low < high);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    match motion(&world, body).support {
        GroundSupport::Supported { entity, .. } => assert_eq!(entity, low),
        other => panic!("expected the lower floor, got {other:?}"),
    }
}

#[test]
fn a_nearer_floor_wins_over_an_earlier_farther_one() {
    let mut world = World::new();
    let farther = floor(&mut world, Vec3::new(0.0, -0.55, 0.0), Vec3::new(4.0, 0.5, 4.0));
    let nearer = floor(&mut world, Vec3::new(0.0, -0.5, 0.0), Vec3::new(4.0, 0.5, 4.0));
    assert!(farther < nearer);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    match motion(&world, body).support {
        GroundSupport::Supported { entity, .. } => assert_eq!(entity, nearer),
        other => panic!("expected the nearer floor, got {other:?}"),
    }
}

#[test]
fn a_small_overlap_is_support_and_a_deep_one_is_not() {
    let mut shallow = World::new();
    flat_floor(&mut shallow);
    let body = walker(&mut shallow, Vec3::new(0.0, REST_Y - 0.005, 0.0));
    step(&mut shallow);
    assert!(matches!(motion(&shallow, body).support, GroundSupport::Supported { .. }));

    let mut deep = World::new();
    floor(&mut deep, Vec3::ZERO, Vec3::new(4.0, 2.0, 4.0));
    let buried = walker(&mut deep, Vec3::ZERO);
    let start = position(&deep, buried);
    step(&mut deep);
    assert_eq!(motion(&deep, buried).support, GroundSupport::Airborne);
    assert_eq!(position(&deep, buried), start);
}

#[test]
fn a_disabled_or_invalid_collider_is_unavailable() {
    let mut world = World::new();
    flat_floor(&mut world);
    let disabled = world.spawn((
        Transform::from_xyz(0.0, REST_Y, 0.0),
        Collider::sphere(RADIUS).disabled(),
        CharacterMotor::default(),
        kinematic(),
        CharacterMotion::default(),
    ));
    let invalid = world.spawn((
        Transform { position: Vec3::new(2.0, REST_Y, 0.0), rotation: engine_math::Quat::IDENTITY, scale: Vec3::ZERO },
        Collider::sphere(RADIUS),
        CharacterMotor::default(),
        kinematic(),
        CharacterMotion::default(),
    ));
    step(&mut world);
    assert_eq!(motion(&world, disabled).support, GroundSupport::Unavailable);
    assert_eq!(motion(&world, invalid).support, GroundSupport::Unavailable);
}

#[test]
fn an_indexed_descendant_rejects_the_move_and_keeps_support() {
    let mut world = World::new();
    let ground = flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    world.spawn_child(body, (Transform::IDENTITY, Collider::sphere(0.2)));
    let start = position(&world, body);
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    assert_eq!(position(&world, body), start);
    match motion(&world, body).support {
        GroundSupport::Supported { entity, .. } => assert_eq!(entity, ground),
        other => panic!("expected the floor to remain support, got {other:?}"),
    }
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
}

#[test]
fn enabled_locomotion_discards_a_competing_move_intent() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    wish(&mut world, body, Vec3::X);
    world.insert(body, (MoveIntent::new(Vec3::new(0.0, 0.0, 5.0)),)).unwrap();
    step(&mut world);
    let end = position(&world, body);
    assert!((end.x - 1.5).abs() < 1e-3, "{end:?}");
    assert!(end.z.abs() < 1e-3, "{end:?}");
    assert!(world.component::<MoveIntent>(body).is_err());
}

#[test]
fn disabled_locomotion_leaves_move_intent_in_charge() {
    let mut world = World::new();
    let body = world.spawn((
        Transform::from_xyz(0.0, REST_Y, 0.0),
        Collider::sphere(RADIUS),
        CharacterMotor::default(),
        CharacterLocomotion { enabled: false, ..kinematic() },
        CharacterMotion::default(),
    ));
    world.insert(body, (LocomotionIntent::new(Vec3::X), MoveIntent::new(Vec3::Y))).unwrap();
    step(&mut world);
    let end = position(&world, body);
    assert!(end.x.abs() < 1e-4, "{end:?}");
    assert!((end.y - (REST_Y + 1.0)).abs() < 1e-4, "{end:?}");
    assert_eq!(motion(&world, body).support, GroundSupport::Unknown);
}

#[test]
fn a_body_without_locomotion_keeps_the_motor() {
    let mut world = World::new();
    let body = world.spawn((Transform::from_xyz(0.0, 1.0, 0.0), Collider::sphere(RADIUS), CharacterMotor::default()));
    world.insert(body, (MoveIntent::new(Vec3::new(0.0, 1.25, -0.5)),)).unwrap();
    step(&mut world);
    let end = position(&world, body);
    assert!(end.x.abs() < 1e-4);
    assert!((end.y - 2.25).abs() < 1e-4, "{end:?}");
    assert!((end.z + 0.5).abs() < 1e-4, "{end:?}");
    assert!(!world.component::<CharacterMotor>(body).unwrap().locked_axes.y);
}

#[test]
fn plain_sync_does_not_advance_locomotion() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    wish(&mut world, body, Vec3::X);
    sync_collision(&mut world);
    assert_eq!(position(&world, body), Vec3::new(0.0, REST_Y, 0.0));
    assert_eq!(motion(&world, body).support, GroundSupport::Unknown);
    step(&mut world);
    assert!((position(&world, body).x - 1.5).abs() < 1e-3);
    assert!(matches!(motion(&world, body).support, GroundSupport::Supported { .. }));
}

#[test]
fn zero_and_nonfinite_steps_do_not_integrate() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    for step_secs in [Some(0.0), Some(-DT), Some(f32::NAN), Some(f32::INFINITY), None] {
        wish(&mut world, body, Vec3::X);
        sync_collision_with_step(&mut world, step_secs);
        assert_eq!(position(&world, body), Vec3::new(0.0, REST_Y, 0.0), "{step_secs:?}");
        assert_eq!(motion(&world, body).support, GroundSupport::Unknown);
    }
}

#[test]
fn two_explicit_steps_move_twice_one_step() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    let once = position(&world, body).x;
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    assert!((position(&world, body).x - once * 2.0).abs() < 1e-3);
}

#[test]
fn a_nonfinite_speed_does_not_move_or_corrupt_support() {
    let mut world = World::new();
    let ground = flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    world.insert(body, (CharacterLocomotion { speed: f32::NAN, ..kinematic() },)).unwrap();
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    assert!((position(&world, body).x).abs() < 1e-4);
    match motion(&world, body).support {
        GroundSupport::Supported { entity, .. } => assert_eq!(entity, ground),
        other => panic!("expected support, got {other:?}"),
    }
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
}
