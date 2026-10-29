//! Gravity, jumping, and landing. These tests call the collision sync directly.
//!
//! Gravity is world −Y, default −400 units/s². Integration is semi-implicit Euler, and only when
//! the pre-move probe is already airborne (or the takeoff lock is set). The step that walks off a
//! floor does not fall. Takeoff sets `velocity.y` to `jump_speed` and skips gravity for that
//! iteration. `velocity` is not `MoveResult.applied / dt`. `velocity.x` and `velocity.z` stay zero.
//!
//! A landing is a promoted [`LocomotionEvent::Landed`]. Spawning inside the support window is not
//! one. A rejected move restores the pre-integration velocity, so a permanent rejection does not
//! accumulate gravity and does not land.

use engine_math::{Transform, Vec3};
use engine_world::{
    sync_collision, sync_collision_with_step, CharacterLocomotion, CharacterMotion, CharacterMotor, Collider, Entity,
    GroundSupport, LocomotionEvent, LocomotionIntent, LocomotionPhase, MoveIntent, MoveResult, World,
};

const DT: f32 = 1.0 / 60.0;
const RADIUS: f32 = 0.5;
const REST_Y: f32 = 0.5;
const GRAVITY: f32 = -400.0;
const JUMP: f32 = 200.0;

fn step(world: &mut World) {
    sync_collision_with_step(world, Some(DT));
}

fn floor(world: &mut World, center: Vec3, half: Vec3) -> Entity {
    world.spawn((Transform::from_position(center), Collider::aabb_box(half)))
}

fn flat_floor(world: &mut World) -> Entity {
    floor(world, Vec3::new(0.0, -0.5, 0.0), Vec3::new(8.0, 0.5, 8.0))
}

fn walker(world: &mut World, position: Vec3) -> Entity {
    world.spawn((
        Transform::from_position(position),
        Collider::sphere(RADIUS),
        CharacterMotor::default(),
        CharacterLocomotion::default(),
        CharacterMotion::default(),
    ))
}

fn position(world: &World, entity: Entity) -> Vec3 {
    world.component::<Transform>(entity).unwrap().position
}

fn motion(world: &World, entity: Entity) -> CharacterMotion {
    *world.component::<CharacterMotion>(entity).unwrap()
}

fn near(actual: f32, expected: f32, tol: f32) {
    assert!((actual - expected).abs() <= tol, "{actual} vs {expected} (tol {tol})");
}

fn finite_position(world: &World, entity: Entity) {
    let position = position(world, entity);
    assert!(position.x.is_finite() && position.y.is_finite() && position.z.is_finite(), "{position:?}");
}

/// Reference semi-implicit step. Gravity is applied before the displacement, matching prepare.
fn euler(velocity: &mut f32, drop: &mut f32, gravity: f32, terminal: f32) {
    *velocity += gravity * DT;
    if terminal > 0.0 {
        *velocity = (*velocity).max(-terminal);
    }
    *drop += *velocity * DT;
}

fn predict_fall(steps: u32, gravity: f32, terminal: f32) -> (f32, f32) {
    let mut velocity = 0.0;
    let mut drop = 0.0;
    for _ in 0..steps {
        euler(&mut velocity, &mut drop, gravity, terminal);
    }
    (velocity, drop)
}

fn predict_jump_peak(jump: f32, gravity: f32, terminal: f32) -> f32 {
    let mut velocity = jump;
    let mut height = jump * DT;
    let mut peak = height;
    for _ in 0..200 {
        let before = velocity;
        euler(&mut velocity, &mut height, gravity, terminal);
        peak = peak.max(height);
        if before > 0.0 && velocity <= 0.0 {
            break;
        }
    }
    peak
}

fn landings(world: &mut World) -> Vec<LocomotionEvent> {
    world.promote_events();
    world.events::<LocomotionEvent>()
}

fn supported(world: &World, entity: Entity) -> bool {
    matches!(motion(world, entity).support, GroundSupport::Supported { .. })
}

#[test]
fn falling_from_rest_matches_one_semi_implicit_step() {
    let mut world = World::new();
    let body = walker(&mut world, Vec3::new(0.0, 10.0, 0.0));
    step(&mut world);
    let (velocity, drop) = predict_fall(1, GRAVITY, 800.0);
    let end = position(&world, body);
    near(end.y, 10.0 + drop, 1e-4);
    near(motion(&world, body).velocity.y, velocity, 1e-4);
    assert_eq!(motion(&world, body).velocity.x, 0.0);
    assert_eq!(motion(&world, body).velocity.z, 0.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert!(end.x.abs() < 1e-5 && end.z.abs() < 1e-5);
    assert!(landings(&mut world).is_empty());
}

#[test]
fn gravity_accumulates_once_per_iteration_and_not_on_a_skipped_step() {
    let mut world = World::new();
    let body = walker(&mut world, Vec3::new(0.0, 10.0, 0.0));
    for count in 1..=4 {
        step(&mut world);
        let (velocity, drop) = predict_fall(count, GRAVITY, 800.0);
        near(position(&world, body).y, 10.0 + drop, 1e-3);
        near(motion(&world, body).velocity.y, velocity, 1e-3);
    }
    let frozen = position(&world, body);
    let frozen_velocity = motion(&world, body).velocity;
    sync_collision(&mut world);
    sync_collision_with_step(&mut world, Some(0.0));
    sync_collision_with_step(&mut world, None);
    assert_eq!(position(&world, body), frozen);
    assert_eq!(motion(&world, body).velocity, frozen_velocity);
}

#[test]
fn terminal_fall_speed_clamps_and_a_non_positive_limit_does_not() {
    let mut clamped = World::new();
    let body = walker(&mut clamped, Vec3::new(0.0, 30.0, 0.0));
    clamped.insert(body, (CharacterLocomotion { terminal_fall: 10.0, ..CharacterLocomotion::default() },)).unwrap();
    for _ in 0..5 {
        step(&mut clamped);
    }
    let (velocity, drop) = predict_fall(5, GRAVITY, 10.0);
    near(motion(&clamped, body).velocity.y, velocity, 1e-3);
    near(motion(&clamped, body).velocity.y, -10.0, 1e-3);
    near(position(&clamped, body).y, 30.0 + drop, 1e-3);

    let mut open = World::new();
    let body = walker(&mut open, Vec3::new(0.0, 40.0, 0.0));
    open.insert(body, (CharacterLocomotion { terminal_fall: 0.0, ..CharacterLocomotion::default() },)).unwrap();
    for _ in 0..130 {
        step(&mut open);
    }
    assert!(motion(&open, body).velocity.y < -800.0, "{}", motion(&open, body).velocity.y);
    finite_position(&open, body);
}

#[test]
fn a_grounded_body_rests_and_does_not_store_the_stick_as_velocity() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    let settled = position(&world, body);
    for _ in 0..120 {
        step(&mut world);
        let end = position(&world, body);
        near(end.y, settled.y, 1e-3);
        assert!(end.x.abs() < 1e-4 && end.z.abs() < 1e-4);
    }
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
    assert!(supported(&world, body));
    assert!(landings(&mut world).is_empty(), "standing on the spawn floor is not a landing");
    let applied_y = world.component::<MoveResult>(body).unwrap().applied.y;
    assert!(applied_y.abs() < 0.02, "stick should not walk the body through the floor {applied_y}");
    assert_eq!(motion(&world, body).velocity.y, 0.0);
}

#[test]
fn walking_off_a_ledge_falls_on_the_following_iteration() {
    let mut world = World::new();
    floor(&mut world, Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.4, 0.5, 4.0));
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    world.insert(body, (LocomotionIntent::new(Vec3::X),)).unwrap();
    step(&mut world);
    let departed = position(&world, body);
    assert!(departed.x > 1.0, "{departed:?}");
    near(departed.y, REST_Y, 1e-2);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
    assert!(landings(&mut world).is_empty());

    step(&mut world);
    let falling = position(&world, body);
    let (velocity, drop) = predict_fall(1, GRAVITY, 800.0);
    near(falling.y, departed.y + drop, 1e-2);
    near(motion(&world, body).velocity.y, velocity, 1e-3);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert!(falling.y < departed.y);
}

#[test]
fn a_supported_body_jumps_and_does_not_re_ground_on_takeoff() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    assert!(supported(&world, body));
    world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
    step(&mut world);
    let end = position(&world, body);
    near(end.y, REST_Y + JUMP * DT, 1e-2);
    near(motion(&world, body).velocity.y, JUMP, 1e-3);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Jump);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert_eq!(motion(&world, body).velocity.x, 0.0);
    assert!(landings(&mut world).is_empty(), "takeoff is not a landing");
}

#[test]
fn the_first_simulated_iteration_can_jump_from_a_floor() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    assert_eq!(motion(&world, body).support, GroundSupport::Unknown);
    world.insert(body, (LocomotionIntent::jumping(Vec3::X),)).unwrap();
    step(&mut world);
    let end = position(&world, body);
    near(end.y, REST_Y + JUMP * DT, 1e-2);
    near(end.x, 90.0 * DT, 1e-2);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Jump);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert!(landings(&mut world).is_empty());
}

#[test]
fn a_short_jump_inside_the_probe_window_stays_airborne_until_it_stops_rising() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    world.insert(body, (CharacterLocomotion { jump_speed: 1.0, ..CharacterLocomotion::default() },)).unwrap();
    step(&mut world);
    world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
    step(&mut world);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Jump);
    near(motion(&world, body).velocity.y, 1.0, 1e-4);
    near(position(&world, body).y, REST_Y + DT, 1e-2);
    assert!(landings(&mut world).is_empty());

    step(&mut world);
    assert!(motion(&world, body).velocity.y <= 0.0, "{}", motion(&world, body).velocity.y);
    assert!(supported(&world, body));
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
}

#[test]
fn jump_apex_matches_semi_implicit_euler_and_then_one_landing() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
    let mut peak = REST_Y;
    let mut saw_up = false;
    let mut crossed = false;
    for _ in 0..120 {
        let before = motion(&world, body).velocity.y;
        step(&mut world);
        let velocity = motion(&world, body).velocity.y;
        peak = peak.max(position(&world, body).y);
        if velocity > 0.0 {
            saw_up = true;
        }
        if saw_up && before > 0.0 && velocity <= 0.0 {
            crossed = true;
            assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
            assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
        }
        if supported(&world, body) && crossed {
            break;
        }
    }
    assert!(crossed, "the jump never left upward speed");
    let predicted = predict_jump_peak(JUMP, GRAVITY, 800.0);
    near(peak - REST_Y, predicted, 1e-2);
    near(peak - REST_Y, 50.0, 2.0);
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
    near(position(&world, body).y, REST_Y, 0.05);
    let rested = position(&world, body).y;
    for _ in 0..30 {
        step(&mut world);
        near(position(&world, body).y, rested, 1e-3);
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        assert!(supported(&world, body));
    }
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
}

#[test]
fn a_ceiling_cancels_upward_velocity_and_the_next_step_falls() {
    for wish in [Vec3::ZERO, Vec3::X] {
        let mut world = World::new();
        flat_floor(&mut world);
        floor(&mut world, Vec3::new(0.0, 3.5, 0.0), Vec3::new(3.0, 0.5, 3.0));
        let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
        step(&mut world);
        world.insert(body, (LocomotionIntent::jumping(wish),)).unwrap();
        step(&mut world);
        let hit = position(&world, body);
        let (applied, requested) = {
            let result = world.component::<MoveResult>(body).unwrap();
            (result.applied, result.requested)
        };
        assert!(
            hit.y < REST_Y + JUMP * DT - 0.5,
            "wish {wish:?} end {hit:?} applied {applied:?} requested {requested:?}"
        );
        assert!(applied.y < requested.y - 1e-3, "applied {applied:?} requested {requested:?}");
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        assert!((motion(&world, body).velocity.y - applied.y / DT).abs() > 1e-2, "applied {applied:?}");
        assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
        if wish.x != 0.0 {
            assert!(hit.x > 0.2, "horizontal slide should survive the ceiling, {hit:?}");
        }
        step(&mut world);
        assert!(position(&world, body).y < hit.y, "wish {wish:?}");
        assert!(motion(&world, body).velocity.y < 0.0);
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
        assert!(landings(&mut world).is_empty());
    }
}

#[test]
fn a_wall_blocks_horizontal_motion_without_cancelling_the_fall() {
    let mut world = World::new();
    world.spawn((Transform::from_xyz(3.0, 4.0, 0.0), Collider::aabb_box(Vec3::new(0.5, 4.0, 2.0))));
    let body = walker(&mut world, Vec3::new(0.0, 4.0, 0.0));
    world.insert(body, (LocomotionIntent::new(Vec3::X),)).unwrap();
    step(&mut world);
    world.insert(body, (LocomotionIntent::new(Vec3::X),)).unwrap();
    step(&mut world);
    let end = position(&world, body);
    let (velocity, drop) = predict_fall(2, GRAVITY, 800.0);
    assert!(end.x < 2.2, "{end:?}");
    assert!(end.x > 0.5, "{end:?}");
    near(end.y, 4.0 + drop, 1e-2);
    near(motion(&world, body).velocity.y, velocity, 1e-2);
    assert!(motion(&world, body).velocity.y < -1.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert!(landings(&mut world).is_empty());
}

#[test]
fn horizontal_wish_still_moves_an_airborne_body() {
    let mut world = World::new();
    let body = walker(&mut world, Vec3::new(0.0, 6.0, 0.0));
    world.insert(body, (LocomotionIntent::new(Vec3::new(1.0, 9.0, 0.0)),)).unwrap();
    step(&mut world);
    let end = position(&world, body);
    let (velocity, drop) = predict_fall(1, GRAVITY, 800.0);
    near(end.x, 90.0 * DT, 1e-3);
    near(end.y, 6.0 + drop, 1e-3);
    near(motion(&world, body).velocity.y, velocity, 1e-3);
    assert_eq!(motion(&world, body).velocity.x, 0.0);
    assert_eq!(motion(&world, body).velocity.z, 0.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
}

#[test]
fn an_airborne_jump_request_does_not_take_off_and_holding_it_does_not_either() {
    let mut world = World::new();
    let body = walker(&mut world, Vec3::new(0.0, 8.0, 0.0));
    for _ in 0..4 {
        world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
        step(&mut world);
        assert!(motion(&world, body).velocity.y < 0.0, "{}", motion(&world, body).velocity.y);
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump);
    }
    let (velocity, drop) = predict_fall(4, GRAVITY, 800.0);
    near(position(&world, body).y, 8.0 + drop, 1e-3);
    near(motion(&world, body).velocity.y, velocity, 1e-3);
    assert!(landings(&mut world).is_empty());
}

#[test]
fn a_sensor_a_wall_and_a_ceiling_do_not_grant_a_jump() {
    let cases: Vec<fn(&mut World)> = vec![
        |world| {
            world.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(8.0, 0.5, 8.0)).sensor()));
        },
        |world| {
            world.spawn((Transform::from_xyz(1.0, REST_Y, 0.0), Collider::aabb_box(Vec3::new(0.5, 2.0, 2.0))));
        },
        |world| {
            world.spawn((Transform::from_xyz(0.0, 1.5, 0.0), Collider::aabb_box(Vec3::new(2.0, 0.5, 2.0))));
        },
    ];
    for place in cases {
        let mut world = World::new();
        place(&mut world);
        let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
        world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
        step(&mut world);
        assert!(position(&world, body).y <= REST_Y, "{:?}", position(&world, body));
        assert!(motion(&world, body).velocity.y <= 0.0, "{:?}", motion(&world, body));
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump);
        assert!(landings(&mut world).is_empty());
    }
}

#[test]
fn spawning_on_a_floor_and_staying_there_does_not_land() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    world.insert(body, (LocomotionIntent::new(Vec3::X),)).unwrap();
    step(&mut world);
    step(&mut world);
    assert!(supported(&world, body));
    assert!(landings(&mut world).is_empty());
}

#[test]
fn a_one_step_drop_onto_a_floor_lands_once_and_a_sensor_does_not() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y + 0.15, 0.0));
    step(&mut world);
    assert!(supported(&world, body));
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
    step(&mut world);
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);

    let mut sensors = World::new();
    sensors.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(8.0, 0.5, 8.0)).sensor()));
    let body = walker(&mut sensors, Vec3::new(0.0, REST_Y + 0.15, 0.0));
    let start = position(&sensors, body).y;
    step(&mut sensors);
    assert!(position(&sensors, body).y < start);
    assert_eq!(motion(&sensors, body).support, GroundSupport::Airborne);
    assert!(landings(&mut sensors).is_empty());
}

#[test]
fn an_unwalkable_floor_and_a_mask_mismatch_do_not_land() {
    let mut steep = World::new();
    flat_floor(&mut steep);
    let body = walker(&mut steep, Vec3::new(0.0, 4.0, 0.0));
    steep.insert(body, (CharacterLocomotion { max_slope: -0.1, ..CharacterLocomotion::default() },)).unwrap();
    for _ in 0..10 {
        step(&mut steep);
    }
    assert_eq!(motion(&steep, body).support, GroundSupport::Airborne);
    assert!(landings(&mut steep).is_empty());
    finite_position(&steep, body);

    let mut masked = World::new();
    masked.spawn((
        Transform::from_xyz(0.0, -0.5, 0.0),
        Collider::aabb_box(Vec3::new(8.0, 0.5, 8.0)).with_layers(0b10).with_mask(0b10),
    ));
    let body = walker(&mut masked, Vec3::new(0.0, REST_Y + 0.15, 0.0));
    step(&mut masked);
    assert_eq!(motion(&masked, body).support, GroundSupport::Airborne);
    assert!(position(&masked, body).y < REST_Y + 0.15);
    assert!(landings(&mut masked).is_empty());
}

#[test]
fn removing_the_floor_starts_gravity_on_that_step() {
    let mut world = World::new();
    let ground = flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    step(&mut world);
    assert!(supported(&world, body));
    world.despawn(ground).unwrap();
    let start = position(&world, body);
    step(&mut world);
    let (velocity, drop) = predict_fall(1, GRAVITY, 800.0);
    near(position(&world, body).y, start.y + drop, 1e-2);
    near(motion(&world, body).velocity.y, velocity, 1e-3);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
    assert!(landings(&mut world).is_empty());

    floor(&mut world, Vec3::new(0.0, -1.0, 0.0), Vec3::new(8.0, 0.5, 8.0));
    for _ in 0..40 {
        step(&mut world);
        if supported(&world, body) {
            break;
        }
    }
    assert!(supported(&world, body));
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
    assert_eq!(motion(&world, body).velocity.y, 0.0);
}

#[test]
fn a_rejected_move_does_not_land_or_grow_velocity() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y + 0.15, 0.0));
    world.spawn_child(body, (Transform::IDENTITY, Collider::sphere(0.2)));
    let start = position(&world, body);
    for _ in 0..20 {
        world.insert(body, (LocomotionIntent::jumping(Vec3::X),)).unwrap();
        step(&mut world);
        assert_eq!(position(&world, body), start);
        assert_eq!(motion(&world, body).velocity, Vec3::ZERO);
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
    }
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert!(landings(&mut world).is_empty());
}

#[test]
fn invalid_gravity_and_jump_settings_do_not_corrupt_the_body() {
    for gravity in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 400.0, 0.0] {
        let mut world = World::new();
        let body = walker(&mut world, Vec3::new(0.0, 5.0, 0.0));
        world.insert(body, (CharacterLocomotion { gravity, ..CharacterLocomotion::default() },)).unwrap();
        for _ in 0..3 {
            step(&mut world);
        }
        near(position(&world, body).y, 5.0, 1e-4);
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        finite_position(&world, body);
    }
    for jump_speed in [f32::NAN, f32::INFINITY, 0.0, -10.0] {
        let mut world = World::new();
        flat_floor(&mut world);
        let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
        world.insert(body, (CharacterLocomotion { jump_speed, ..CharacterLocomotion::default() },)).unwrap();
        world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
        step(&mut world);
        near(position(&world, body).y, REST_Y, 1e-3);
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        assert!(supported(&world, body));
        finite_position(&world, body);
    }
}

#[test]
fn enabled_locomotion_falls_instead_of_honouring_a_competing_move_intent() {
    let mut world = World::new();
    let body = walker(&mut world, Vec3::new(0.0, 4.0, 0.0));
    world.insert(body, (MoveIntent::new(Vec3::Y * 10.0),)).unwrap();
    step(&mut world);
    assert!(position(&world, body).y < 4.0, "{:?}", position(&world, body));
    assert!(world.component::<MoveIntent>(body).is_err());
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);

    let mut disabled = World::new();
    let body = disabled.spawn((
        Transform::from_xyz(0.0, 4.0, 0.0),
        Collider::sphere(RADIUS),
        CharacterMotor::default(),
        CharacterLocomotion { enabled: false, ..CharacterLocomotion::default() },
        CharacterMotion::default(),
    ));
    disabled.insert(body, (MoveIntent::new(Vec3::Y),)).unwrap();
    step(&mut disabled);
    near(position(&disabled, body).y, 5.0, 1e-4);
    assert_eq!(motion(&disabled, body).support, GroundSupport::Unknown);
    assert_eq!(motion(&disabled, body).velocity, Vec3::ZERO);

    let mut bare = World::new();
    let body = bare.spawn((Transform::from_xyz(0.0, 4.0, 0.0), Collider::sphere(RADIUS), CharacterMotor::default()));
    bare.insert(body, (MoveIntent::new(Vec3::new(0.25, -0.5, 0.0)),)).unwrap();
    step(&mut bare);
    near(position(&bare, body).y, 3.5, 1e-4);
    near(position(&bare, body).x, 0.25, 1e-4);
}

#[test]
fn a_shallow_overlap_stays_supported_and_a_recovery_is_not_stored_as_velocity() {
    let mut world = World::new();
    flat_floor(&mut world);
    let body = walker(&mut world, Vec3::new(0.0, REST_Y - 0.005, 0.0));
    step(&mut world);
    assert!(supported(&world, body));
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    finite_position(&world, body);
    assert!(landings(&mut world).is_empty());
}

#[test]
fn two_bodies_fall_alike_and_a_grounded_neighbour_stays_put() {
    let mut world = World::new();
    flat_floor(&mut world);
    let grounded = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
    let left = walker(&mut world, Vec3::new(-3.0, 9.0, 0.0));
    let right = walker(&mut world, Vec3::new(3.0, 9.0, 2.0));
    step(&mut world);
    let (velocity, drop) = predict_fall(1, GRAVITY, 800.0);
    near(position(&world, left).y, 9.0 + drop, 1e-3);
    near(position(&world, right).y, 9.0 + drop, 1e-3);
    near(motion(&world, left).velocity.y, velocity, 1e-3);
    near(motion(&world, right).velocity.y, velocity, 1e-3);
    near(position(&world, grounded).y, REST_Y, 1e-3);
    assert_eq!(motion(&world, grounded).velocity, Vec3::ZERO);
    assert!(supported(&world, grounded));
}

#[test]
fn the_same_jump_fall_landing_and_walk_replays_in_two_worlds() {
    fn trace() -> Vec<(Vec3, CharacterMotion)> {
        let mut world = World::new();
        flat_floor(&mut world);
        let body = walker(&mut world, Vec3::new(0.0, REST_Y, 0.0));
        let mut out = Vec::new();
        step(&mut world);
        out.push((position(&world, body), motion(&world, body)));
        world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
        step(&mut world);
        out.push((position(&world, body), motion(&world, body)));
        for index in 0..100 {
            if index < 3 {
                world.insert(body, (LocomotionIntent::new(Vec3::Z),)).unwrap();
            }
            step(&mut world);
            let done = supported(&world, body) && motion(&world, body).velocity.y == 0.0 && out.len() > 5;
            out.push((position(&world, body), motion(&world, body)));
            if done {
                break;
            }
        }
        for _ in 0..3 {
            world.insert(body, (LocomotionIntent::new(Vec3::X),)).unwrap();
            step(&mut world);
            out.push((position(&world, body), motion(&world, body)));
        }
        assert!(out.len() > 8, "the replay never landed");
        assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
        let last = out.last().unwrap();
        assert!(last.0.x > 1.0, "{:?}", last.0);
        assert_eq!(last.1.phase, LocomotionPhase::Walk);
        assert_eq!(last.1.velocity.x, 0.0);
        assert_eq!(last.1.velocity.z, 0.0);
        out
    }

    let left = trace();
    let right = trace();
    assert_eq!(left.len(), right.len());
    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        assert_eq!(left.0, right.0, "step {index}");
        assert_eq!(left.1.velocity, right.1.velocity, "step {index}");
        assert_eq!(left.1.phase, right.1.phase, "step {index}");
        assert_eq!(left.1.support, right.1.support, "step {index}");
    }
}
