//! Acceleration, running, and air control. These tests use the gameplay defaults.
//!
//! Walk is 90 units per second along the surface. Run is 140. Ground acceleration is 600 units
//! per second squared, ground deceleration is 800, and air steering is 300. One fixed step is
//! 1/60 s, so a grounded step changes world-horizontal speed by at most 10 while speeding up and
//! about 13.333 while slowing down. That rate is not along-surface acceleration. On a 30° slope a
//! gradient walk stores about 77.94, a contour walk stores 90, and an equal diagonal stores about
//! `(51.03, 68.03)`. The body still travels 90 along the surface.

use engine_math::{Quat, Transform, Vec3};
use engine_world::{
    sync_collision, sync_collision_with_step, CharacterLocomotion, CharacterMotion, CharacterMotor, Collider, Entity,
    GroundSupport, LocomotionEvent, LocomotionIntent, LocomotionPhase, MoveIntent, MoveResult, World,
};

const DT: f32 = 1.0 / 60.0;
const RADIUS: f32 = 0.5;
const ACCEL_STEP: f32 = 600.0 * DT;
const DECEL_STEP: f32 = 800.0 * DT;
const AIR_STEP: f32 = 300.0 * DT;

fn step(world: &mut World) {
    sync_collision_with_step(world, Some(DT));
}

fn floor(world: &mut World) -> Entity {
    world.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(400.0, 0.5, 400.0))))
}

fn body_at(world: &mut World, position: Vec3) -> Entity {
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

fn drive(world: &mut World, entity: Entity, wish: Vec3, run: bool, jump: bool) {
    world.insert(entity, (LocomotionIntent { wish, jump, run },)).unwrap();
    step(world);
}

fn near(actual: f32, expected: f32, tol: f32) {
    assert!((actual - expected).abs() <= tol, "{actual} vs {expected} (tol {tol})");
}

fn horizontal(velocity: Vec3) -> f32 {
    Vec3::new(velocity.x, 0.0, velocity.z).length()
}

fn landings(world: &mut World) -> Vec<LocomotionEvent> {
    world.promote_events();
    world.events::<LocomotionEvent>()
}

struct Ramp {
    entity: Entity,
    normal: Vec3,
}

fn ramp(world: &mut World, degrees: f32, yaw: f32) -> Ramp {
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(degrees.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    let half = Vec3::new(80.0, 2.0, 80.0);
    let entity =
        world.spawn((Transform::from_position(-normal * half.y).with_rotation(rotation), Collider::aabb_box(half)));
    Ramp { entity, normal }
}

fn on_ramp(ramp: &Ramp) -> Vec3 {
    ramp.normal * RADIUS
}

fn supported_by(world: &World, body: Entity, ground: Entity) -> bool {
    matches!(motion(world, body).support, GroundSupport::Supported { entity, .. } if entity == ground)
}

#[test]
fn walking_accelerates_from_rest_and_stops_without_reversing() {
    let mut world = World::new();
    floor(&mut world);
    let body = body_at(&mut world, Vec3::new(0.0, RADIUS, 0.0));
    drive(&mut world, body, Vec3::ZERO, false, false);
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);

    let mut speed = 0.0;
    for _ in 0..8 {
        let before = position(&world, body).x;
        drive(&mut world, body, Vec3::X, false, false);
        speed += ACCEL_STEP;
        near(motion(&world, body).velocity.x, speed, 1e-3);
        near(position(&world, body).x - before, speed * DT, 1e-3);
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
    }
    drive(&mut world, body, Vec3::X, false, false);
    near(motion(&world, body).velocity.x, 90.0, 1e-3);
    assert!(horizontal(motion(&world, body).velocity) < 115.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);

    let mut slowing = 90.0;
    for _ in 0..6 {
        drive(&mut world, body, Vec3::ZERO, false, false);
        slowing = (slowing - DECEL_STEP).max(0.0);
        near(motion(&world, body).velocity.x, slowing, 1e-2);
        assert!(motion(&world, body).velocity.x >= -1e-3);
        if slowing > 1.0 {
            assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
        }
    }
    drive(&mut world, body, Vec3::ZERO, false, false);
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
    let rested = position(&world, body);
    for _ in 0..5 {
        drive(&mut world, body, Vec3::ZERO, false, false);
    }
    assert_eq!(position(&world, body), rested);
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO);
}

#[test]
fn reversing_and_partial_and_diagonal_wishes_stay_bounded() {
    let mut world = World::new();
    floor(&mut world);
    let body = body_at(&mut world, Vec3::new(0.0, RADIUS, 0.0));
    for _ in 0..12 {
        drive(&mut world, body, Vec3::X, false, false);
    }
    near(motion(&world, body).velocity.x, 90.0, 1e-3);
    drive(&mut world, body, -Vec3::X, false, false);
    near(motion(&world, body).velocity.x, 90.0 - DECEL_STEP, 1e-2);
    assert!(motion(&world, body).velocity.x > 0.0, "a reverse snapped through zero");

    let mut partial = World::new();
    floor(&mut partial);
    let body = body_at(&mut partial, Vec3::new(0.0, RADIUS, 0.0));
    for _ in 0..8 {
        drive(&mut partial, body, Vec3::X * 0.5, false, false);
    }
    near(motion(&partial, body).velocity.x, 45.0, 1e-2);
    assert!(motion(&partial, body).velocity.x < 50.0);

    let mut diagonal = World::new();
    floor(&mut diagonal);
    let body = body_at(&mut diagonal, Vec3::new(0.0, RADIUS, 0.0));
    for _ in 0..12 {
        drive(&mut diagonal, body, Vec3::new(1.0, 4.0, 1.0), false, false);
    }
    let velocity = motion(&diagonal, body).velocity;
    near(horizontal(velocity), 90.0, 1e-2);
    near(velocity.x, velocity.z, 1e-3);
    let before = position(&diagonal, body);
    drive(&mut diagonal, body, Vec3::new(1.0, 0.0, 1.0), false, false);
    let moved = position(&diagonal, body) - before;
    near(Vec3::new(moved.x, 0.0, moved.z).length(), 90.0 * DT, 1e-2);
}

#[test]
fn running_accelerates_above_the_walk_and_releasing_it_returns() {
    let mut world = World::new();
    floor(&mut world);
    let body = body_at(&mut world, Vec3::new(0.0, RADIUS, 0.0));
    let mut saw_walk = false;
    let mut saw_run = false;
    for _ in 0..20 {
        drive(&mut world, body, Vec3::X, true, false);
        let speed = motion(&world, body).velocity.x;
        match motion(&world, body).phase {
            LocomotionPhase::Walk => {
                saw_walk = true;
                assert!(speed < 120.0, "{speed}");
            }
            LocomotionPhase::Run => {
                saw_run = true;
                assert!(speed > 110.0, "{speed}");
            }
            other => panic!("unexpected {other:?} at {speed}"),
        }
    }
    assert!(saw_walk && saw_run);
    near(motion(&world, body).velocity.x, 140.0, 1e-2);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Run);

    let mut still_run = false;
    for _ in 0..3 {
        let before = motion(&world, body).velocity.x;
        drive(&mut world, body, Vec3::X, false, false);
        let speed = motion(&world, body).velocity.x;
        near(before - speed, DECEL_STEP, 0.05);
        assert!(speed > 99.0, "released run snapped down to {speed}");
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Run);
        still_run = true;
    }
    assert!(still_run);
    for _ in 0..6 {
        drive(&mut world, body, Vec3::X, false, false);
    }
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
    near(motion(&world, body).velocity.x, 90.0, 1e-2);
}

#[test]
fn invalid_rates_do_not_create_nonfinite_motion() {
    let cases = [
        CharacterLocomotion { speed: f32::NAN, ..CharacterLocomotion::default() },
        CharacterLocomotion { acceleration: -50.0, ..CharacterLocomotion::default() },
        CharacterLocomotion { acceleration: 0.0, ..CharacterLocomotion::default() },
        CharacterLocomotion { acceleration: 1.0e20, ..CharacterLocomotion::default() },
        CharacterLocomotion { acceleration: f32::MAX, ..CharacterLocomotion::default() },
        CharacterLocomotion { run_speed: -5.0, ..CharacterLocomotion::default() },
        CharacterLocomotion { run_speed: f32::NAN, ..CharacterLocomotion::default() },
        CharacterLocomotion { run_speed: 40.0, ..CharacterLocomotion::default() },
        CharacterLocomotion { deceleration: -10.0, ..CharacterLocomotion::default() },
        CharacterLocomotion { deceleration: 0.0, ..CharacterLocomotion::default() },
        CharacterLocomotion { air_acceleration: f32::NAN, ..CharacterLocomotion::default() },
    ];
    for locomotion in cases {
        let mut world = World::new();
        floor(&mut world);
        let body = body_at(&mut world, Vec3::new(0.0, RADIUS, 0.0));
        world.insert(body, (locomotion,)).unwrap();
        for _ in 0..5 {
            drive(&mut world, body, Vec3::X, true, false);
            let at = position(&world, body);
            let velocity = motion(&world, body).velocity;
            assert!(at.x.is_finite() && at.y.is_finite() && at.z.is_finite(), "{at:?}");
            assert!(velocity.x.is_finite() && velocity.y.is_finite() && velocity.z.is_finite(), "{velocity:?}");
        }
    }

    let mut held = World::new();
    floor(&mut held);
    let body = body_at(&mut held, Vec3::new(0.0, RADIUS, 0.0));
    held.insert(body, (CharacterLocomotion { acceleration: 0.0, ..CharacterLocomotion::default() },)).unwrap();
    for _ in 0..5 {
        drive(&mut held, body, Vec3::X, false, false);
    }
    assert_eq!(motion(&held, body).velocity.x, 0.0);
    near(position(&held, body).x, 0.0, 1e-3);

    let mut coast = World::new();
    floor(&mut coast);
    let body = body_at(&mut coast, Vec3::new(0.0, RADIUS, 0.0));
    drive(&mut coast, body, Vec3::ZERO, false, false);
    let mut stored = motion(&coast, body);
    stored.velocity.x = 90.0;
    coast.insert(body, (stored,)).unwrap();
    coast.insert(body, (CharacterLocomotion { deceleration: 0.0, ..CharacterLocomotion::default() },)).unwrap();
    for _ in 0..4 {
        drive(&mut coast, body, Vec3::ZERO, false, false);
    }
    near(motion(&coast, body).velocity.x, 90.0, 1e-2);

    let mut slower = World::new();
    floor(&mut slower);
    let body = body_at(&mut slower, Vec3::new(0.0, RADIUS, 0.0));
    slower.insert(body, (CharacterLocomotion { run_speed: 40.0, ..CharacterLocomotion::default() },)).unwrap();
    for _ in 0..10 {
        drive(&mut slower, body, Vec3::X, true, false);
    }
    near(motion(&slower, body).velocity.x, 40.0, 1e-2);
    assert_ne!(motion(&slower, body).phase, LocomotionPhase::Run);
}

#[test]
fn a_30_degree_slope_keeps_along_surface_speed_while_accelerating() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0);
    let body = body_at(&mut world, on_ramp(&slope));
    drive(&mut world, body, Vec3::ZERO, false, false);
    assert!(supported_by(&world, body, slope.entity));
    for index in 0..7 {
        drive(&mut world, body, Vec3::X, false, false);
        let speed = motion(&world, body).velocity.x;
        near(speed, ACCEL_STEP * (index + 1) as f32, 0.05);
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
        assert!(supported_by(&world, body, slope.entity));
    }
    drive(&mut world, body, Vec3::X, false, false);
    near(motion(&world, body).velocity.x, 77.942, 0.05);
    let before = position(&world, body);
    let stored = motion(&world, body).velocity.x;
    drive(&mut world, body, Vec3::X, false, false);
    let moved = position(&world, body) - before;
    near(moved.length(), 90.0 * DT, 0.03);
    near(moved.x, stored * DT, 0.03);
    assert!(moved.y > 0.5, "uphill did not gain Y {moved:?}");
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);

    let mut down = World::new();
    let slope = ramp(&mut down, 30.0, 0.0);
    let body = body_at(&mut down, on_ramp(&slope));
    for _ in 0..20 {
        drive(&mut down, body, -Vec3::X, true, false);
    }
    near(motion(&down, body).velocity.x, -121.24, 0.2);
    assert_eq!(motion(&down, body).phase, LocomotionPhase::Run);
    let before = position(&down, body);
    drive(&mut down, body, -Vec3::X, true, false);
    let moved = position(&down, body) - before;
    near(moved.length(), 140.0 * DT, 0.04);
    assert!(moved.y < -0.5, "downhill did not lose Y {moved:?}");
    assert_eq!(motion(&down, body).velocity.y, 0.0);

    let mut up = World::new();
    let slope = ramp(&mut up, 30.0, 0.0);
    let body = body_at(&mut up, on_ramp(&slope));
    for _ in 0..20 {
        drive(&mut up, body, Vec3::X, true, false);
    }
    near(motion(&up, body).velocity.x, 121.24, 0.2);
    let before = position(&up, body);
    drive(&mut up, body, Vec3::X, true, false);
    near((position(&up, body) - before).length(), 140.0 * DT, 0.04);

    let mut yawed = World::new();
    let slope = ramp(&mut yawed, 30.0, std::f32::consts::FRAC_PI_2);
    let body = body_at(&mut yawed, on_ramp(&slope));
    for _ in 0..16 {
        drive(&mut yawed, body, Vec3::new(-1.0, 0.0, 1.0), true, false);
    }
    let before = position(&yawed, body);
    drive(&mut yawed, body, Vec3::new(-1.0, 0.0, 1.0), true, false);
    let moved = position(&yawed, body) - before;
    near(moved.length(), 140.0 * DT, 0.05);
    assert!(horizontal(motion(&yawed, body).velocity) <= 140.5, "{:?}", motion(&yawed, body).velocity);
    assert_ne!(motion(&yawed, body).phase, LocomotionPhase::Jump);
    assert_ne!(motion(&yawed, body).phase, LocomotionPhase::Fall);
}

#[test]
fn slope_direction_stores_the_tangent_and_acceleration_stays_horizontal() {
    fn instant() -> CharacterLocomotion {
        CharacterLocomotion {
            acceleration: 1.0e6,
            deceleration: 1.0e6,
            air_acceleration: 1.0e6,
            ..CharacterLocomotion::default()
        }
    }
    fn ride(wish: Vec3, run: bool, yaw: f32) -> (Vec3, Vec3) {
        let mut world = World::new();
        let slope = ramp(&mut world, 30.0, yaw);
        let body = body_at(&mut world, on_ramp(&slope));
        world.insert(body, (instant(),)).unwrap();
        drive(&mut world, body, Vec3::ZERO, false, false);
        let start = position(&world, body);
        drive(&mut world, body, wish, run, false);
        (motion(&world, body).velocity, position(&world, body) - start)
    }

    let (uphill, moved) = ride(Vec3::X, false, 0.0);
    near(uphill.x, 77.942, 0.05);
    assert!(uphill.y.abs() < 1e-3 && uphill.z.abs() < 0.02, "{uphill:?}");
    near(moved.length(), 1.5, 0.03);
    near(moved.x, uphill.x * DT, 0.03);

    let (downhill, moved) = ride(-Vec3::X, false, 0.0);
    near(downhill.x, -77.942, 0.05);
    assert!(moved.y < -0.5, "{moved:?}");

    let (contour, moved) = ride(Vec3::Z, false, 0.0);
    near(contour.z, 90.0, 0.05);
    assert!(contour.x.abs() < 0.02, "{contour:?}");
    near(moved.z, 1.5, 0.03);
    assert!(moved.x.abs() < 0.03 && moved.y.abs() < 0.03, "{moved:?}");

    let (diagonal, moved) = ride(Vec3::new(1.0, 0.0, 1.0), false, 0.0);
    near(diagonal.x, 51.025, 0.08);
    near(diagonal.z, 68.034, 0.08);
    near(horizontal(diagonal), 85.042, 0.1);
    near(moved.length(), 1.5, 0.04);
    near(moved.x, diagonal.x * DT, 0.04);
    near(moved.z, diagonal.z * DT, 0.04);
    let wish = Vec3::new(1.0, 0.0, 1.0).normalized();
    let stored = Vec3::new(diagonal.x, 0.0, diagonal.z).normalized();
    assert!((stored - wish).length() > 0.05, "stored the wish {stored:?}");

    let (running, moved) = ride(Vec3::Z, true, 0.0);
    near(running.z, 140.0, 0.1);
    near(moved.length(), 140.0 * DT, 0.05);

    let (yawed, _) = ride(-Vec3::Z, false, std::f32::consts::FRAC_PI_2);
    near(yawed.z, -77.942, 0.1);
    assert!(yawed.x.abs() < 0.1, "{yawed:?}");

    let mut reversing = World::new();
    let slope = ramp(&mut reversing, 30.0, 0.0);
    let body = body_at(&mut reversing, on_ramp(&slope));
    reversing.insert(body, (instant(),)).unwrap();
    drive(&mut reversing, body, Vec3::X, false, false);
    reversing.insert(body, (CharacterLocomotion::default(),)).unwrap();
    drive(&mut reversing, body, -Vec3::X, false, false);
    let speed = motion(&reversing, body).velocity.x;
    near(speed, 77.942 - DECEL_STEP, 0.2);
    assert!(speed > 0.0, "the reverse snapped through zero at {speed}");

    let mut up = World::new();
    let slope = ramp(&mut up, 30.0, 0.0);
    let body = body_at(&mut up, on_ramp(&slope));
    drive(&mut up, body, Vec3::ZERO, false, false);
    drive(&mut up, body, Vec3::X, false, false);
    near(motion(&up, body).velocity.x, ACCEL_STEP, 0.05);

    let mut side = World::new();
    let slope = ramp(&mut side, 30.0, 0.0);
    let body = body_at(&mut side, on_ramp(&slope));
    drive(&mut side, body, Vec3::ZERO, false, false);
    drive(&mut side, body, Vec3::Z, false, false);
    near(motion(&side, body).velocity.z, ACCEL_STEP, 0.05);
    assert!(motion(&side, body).velocity.x.abs() < 0.05);

    let mut jump = World::new();
    let slope = ramp(&mut jump, 30.0, std::f32::consts::FRAC_PI_2);
    let body = body_at(&mut jump, on_ramp(&slope));
    jump.insert(body, (instant(),)).unwrap();
    drive(&mut jump, body, -Vec3::Z, false, false);
    let before = motion(&jump, body).velocity;
    drive(&mut jump, body, Vec3::ZERO, false, true);
    let after = motion(&jump, body).velocity;
    near(after.x, before.x, 0.05);
    near(after.z, before.z, 0.05);
    near(after.y, 200.0, 0.05);
    assert_eq!(motion(&jump, body).phase, LocomotionPhase::Jump);
}

#[test]
fn stopping_on_a_slope_does_not_creep_and_a_target_change_is_rate_limited() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0);
    let body = body_at(&mut world, on_ramp(&slope));
    for _ in 0..12 {
        drive(&mut world, body, Vec3::X, true, false);
    }
    assert!(motion(&world, body).velocity.x > 100.0);
    for _ in 0..20 {
        drive(&mut world, body, Vec3::ZERO, true, false);
    }
    near(motion(&world, body).velocity.x, 0.0, 1e-2);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle);
    let rested = position(&world, body);
    for _ in 0..30 {
        drive(&mut world, body, Vec3::ZERO, false, false);
    }
    let drift = (position(&world, body) - rested).length();
    assert!(drift < 1e-3, "stopped slope drifted {drift}");
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO);

    let mut limited = World::new();
    let slope = ramp(&mut limited, 30.0, 0.0);
    let body = body_at(&mut limited, on_ramp(&slope));
    drive(&mut limited, body, Vec3::ZERO, false, false);
    let mut stored = motion(&limited, body);
    stored.velocity.x = 77.942;
    limited.insert(body, (stored,)).unwrap();
    drive(&mut limited, body, Vec3::X, true, false);
    let gained = motion(&limited, body).velocity.x - 77.942;
    assert!(gained > 0.0 && gained <= ACCEL_STEP + 0.05, "slope run injected {gained}");
}

#[test]
fn running_uphill_into_a_wall_does_not_store_a_launch() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0);
    let start = slope.normal * 0.35 + Vec3::Y * 0.45;
    let body = world.spawn((
        Transform::from_position(start),
        Collider::capsule(0.35, 0.45),
        CharacterMotor::default(),
        CharacterLocomotion::default(),
        CharacterMotion::default(),
    ));
    let face = start.x + 0.35 + 0.85;
    world.spawn((Transform::from_xyz(face + 0.4, start.y + 1.5, 0.0), Collider::aabb_box(Vec3::new(0.4, 4.0, 8.0))));

    let mut stopped_y = None;
    for _ in 0..300 {
        let before = position(&world, body);
        drive(&mut world, body, Vec3::X, true, false);
        let at = position(&world, body);
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump);
        assert!(supported_by(&world, body, slope.entity), "{at:?}");
        if (at.x - before.x).abs() < 0.02 {
            if motion(&world, body).phase == LocomotionPhase::Run {
                panic!("blocked run reported Run at {at:?} {:?}", motion(&world, body).velocity);
            }
            near(motion(&world, body).velocity.x, 0.0, 0.05);
            near(motion(&world, body).velocity.z, 0.0, 0.05);
            let stop = stopped_y.get_or_insert(at.y);
            assert!((at.y - *stop).abs() < 0.02, "wall drifted vertically");
        }
    }
    assert!(stopped_y.is_some(), "never reached the wall");
    assert!(landings(&mut world).is_empty(), "{:?}", landings(&mut world));

    let held = position(&world, body);
    for _ in 0..20 {
        drive(&mut world, body, Vec3::ZERO, true, false);
    }
    let released = (position(&world, body) - held).length();
    assert!(released < 0.05, "release launched {released}");
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO);
}

#[test]
fn a_corner_and_a_diagonal_wall_keep_only_the_free_direction() {
    let mut corner = World::new();
    let slope = ramp(&mut corner, 30.0, 0.0);
    let start = on_ramp(&slope);
    let body = body_at(&mut corner, start);
    corner.spawn((
        Transform::from_xyz(start.x + RADIUS + 0.5, start.y + 1.0, 0.0),
        Collider::aabb_box(Vec3::new(0.4, 3.0, 8.0)),
    ));
    corner.spawn((
        Transform::from_xyz(start.x, start.y + 1.0, start.z + RADIUS + 0.5),
        Collider::aabb_box(Vec3::new(8.0, 3.0, 0.4)),
    ));
    for _ in 0..40 {
        drive(&mut corner, body, Vec3::new(1.0, 0.0, 1.0), true, false);
    }
    near(horizontal(motion(&corner, body).velocity), 0.0, 0.2);
    let trapped = position(&corner, body);
    for _ in 0..15 {
        drive(&mut corner, body, Vec3::ZERO, false, false);
    }
    assert!((position(&corner, body) - trapped).length() < 0.05, "{:?}", position(&corner, body));

    let mut slide = World::new();
    let slope = ramp(&mut slide, 30.0, 0.0);
    let start = on_ramp(&slope);
    let body = body_at(&mut slide, start);
    slide.spawn((
        Transform::from_xyz(start.x + RADIUS + 0.45, start.y + 1.0, 0.0),
        Collider::aabb_box(Vec3::new(0.4, 3.0, 30.0)),
    ));
    for _ in 0..24 {
        drive(&mut slide, body, Vec3::new(1.0, 0.0, 1.0), true, false);
    }
    let velocity = motion(&slide, body).velocity;
    assert!(velocity.x.abs() < 1.0, "into-wall speed remained {velocity:?}");
    assert!(velocity.z > 40.0, "the free direction was cleared {velocity:?}");
    assert_eq!(velocity.y, 0.0);
    let before = position(&slide, body);
    drive(&mut slide, body, Vec3::new(1.0, 0.0, 1.0), true, false);
    let moved = position(&slide, body) - before;
    assert!(moved.z > 0.5, "{moved:?}");
    assert!(moved.x.abs() < 0.08, "{moved:?}");
    assert!(moved.y.abs() < 0.08, "{moved:?}");
}

#[test]
fn a_steep_face_does_not_turn_horizontal_running_into_a_climb() {
    let mut world = World::new();
    let slope = ramp(&mut world, 60.0, 0.0);
    let body = body_at(&mut world, on_ramp(&slope));
    let start = position(&world, body);
    let mut highest = start.y;
    for _ in 0..40 {
        drive(&mut world, body, Vec3::X, true, false);
        highest = highest.max(position(&world, body).y);
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump);
        assert!(!supported_by(&world, body, slope.entity));
    }
    assert!(
        position(&world, body).y < start.y - 0.2,
        "did not slide down from {start:?} to {:?}",
        position(&world, body)
    );
    assert!(highest < start.y + 0.35, "horizontal run climbed to {highest}");
    assert!(motion(&world, body).velocity.y < 0.0);
}

#[test]
fn jumping_preserves_horizontal_speed_and_air_steering_is_bounded() {
    let mut flat = World::new();
    floor(&mut flat);
    let body = body_at(&mut flat, Vec3::new(0.0, RADIUS, 0.0));
    for _ in 0..12 {
        drive(&mut flat, body, Vec3::X, false, false);
    }
    let walk = motion(&flat, body).velocity.x;
    near(walk, 90.0, 1e-2);
    drive(&mut flat, body, Vec3::X, false, true);
    let taken = motion(&flat, body).velocity;
    assert!((taken.x - walk).abs() <= AIR_STEP + 0.05, "takeoff changed X by {}", taken.x - walk);
    near(taken.y, 200.0, 1e-2);
    assert_eq!(taken.z, 0.0);
    assert_eq!(motion(&flat, body).phase, LocomotionPhase::Jump);
    let before = position(&flat, body);
    drive(&mut flat, body, Vec3::ZERO, false, false);
    let coast = motion(&flat, body).velocity.x;
    near(coast, taken.x, 1e-3);
    near(position(&flat, body).x - before.x, coast * DT, 0.02);
    assert!(motion(&flat, body).velocity.y < taken.y);

    let mut running = World::new();
    floor(&mut running);
    let body = body_at(&mut running, Vec3::new(0.0, RADIUS, 0.0));
    for _ in 0..20 {
        drive(&mut running, body, Vec3::X, true, false);
    }
    near(motion(&running, body).velocity.x, 140.0, 1e-2);
    drive(&mut running, body, Vec3::X, true, true);
    assert!((motion(&running, body).velocity.x - 140.0).abs() <= AIR_STEP + 0.05);
    near(motion(&running, body).velocity.y, 200.0, 1e-2);

    let mut sloped = World::new();
    let slope = ramp(&mut sloped, 30.0, 0.0);
    let body = body_at(&mut sloped, on_ramp(&slope));
    drive(&mut sloped, body, Vec3::ZERO, false, false);
    let mut stored = motion(&sloped, body);
    stored.velocity.x = 77.942;
    sloped.insert(body, (stored,)).unwrap();
    drive(&mut sloped, body, Vec3::ZERO, false, true);
    near(motion(&sloped, body).velocity.x, 77.942, 0.05);
    near(motion(&sloped, body).velocity.y, 200.0, 1e-2);
    let requested = sloped.component::<MoveResult>(body).unwrap().requested;
    near(requested.x, 77.942 * DT, 0.03);
    near(requested.y, 200.0 * DT, 0.03);
    assert!(requested.y > 0.0 && requested.z.abs() < 1e-3);

    let mut steer = World::new();
    let body = body_at(&mut steer, Vec3::new(0.0, 8.0, 0.0));
    drive(&mut steer, body, Vec3::ZERO, false, false);
    let mut stored = motion(&steer, body);
    stored.velocity.x = 90.0;
    steer.insert(body, (stored,)).unwrap();
    let y_before = motion(&steer, body).velocity.y;
    drive(&mut steer, body, Vec3::ZERO, true, false);
    near(motion(&steer, body).velocity.x, 90.0, 1e-3);
    assert!((motion(&steer, body).velocity.y - (y_before - 400.0 * DT)).abs() < 1e-2);

    drive(&mut steer, body, -Vec3::X, true, false);
    near(motion(&steer, body).velocity.x, 90.0 - AIR_STEP, 0.05);
    assert!(motion(&steer, body).velocity.x > 0.0);
    for _ in 0..40 {
        drive(&mut steer, body, -Vec3::X, false, false);
    }
    near(motion(&steer, body).velocity.x, -90.0, 0.1);
    assert!(motion(&steer, body).velocity.x >= -90.1);

    let mut wiggle = World::new();
    let body = body_at(&mut wiggle, Vec3::new(0.0, 12.0, 0.0));
    for index in 0..30 {
        let wish = if index % 2 == 0 { Vec3::X } else { Vec3::Z };
        drive(&mut wiggle, body, wish, true, false);
        assert!(horizontal(motion(&wiggle, body).velocity) <= 140.0 + 0.1);
    }
}

#[test]
fn gravity_stays_independent_and_walls_and_ceilings_do_not_add_a_jump() {
    let mut world = World::new();
    let still = body_at(&mut world, Vec3::new(0.0, 6.0, 0.0));
    let moving = body_at(&mut world, Vec3::new(0.0, 6.0, 3.0));
    for _ in 0..5 {
        world.insert(still, (LocomotionIntent::new(Vec3::ZERO),)).unwrap();
        world.insert(moving, (LocomotionIntent::running(Vec3::X),)).unwrap();
        step(&mut world);
        near(motion(&world, still).velocity.y, motion(&world, moving).velocity.y, 1e-3);
        assert_eq!(motion(&world, still).velocity.x, 0.0);
        assert!(motion(&world, moving).velocity.x > 0.0);
    }

    let mut blocked = World::new();
    let body = body_at(&mut blocked, Vec3::new(0.0, 5.0, 0.0));
    blocked.spawn((Transform::from_xyz(1.6, 8.0, 0.0), Collider::aabb_box(Vec3::new(0.6, 16.0, 6.0))));
    let mut contacted = false;
    for _ in 0..12 {
        drive(&mut blocked, body, Vec3::X, true, false);
        let at = position(&blocked, body);
        if at.x > 0.4 && at.y > 0.0 {
            contacted = true;
            assert!(at.x < 0.7, "fell through the wall {at:?}");
            near(motion(&blocked, body).velocity.x, 0.0, 0.05);
            assert!(motion(&blocked, body).velocity.y < -1.0);
            assert_eq!(motion(&blocked, body).phase, LocomotionPhase::Fall);
        }
    }
    assert!(contacted, "never met the wall at {:?}", position(&blocked, body));

    let mut ceiling = World::new();
    floor(&mut ceiling);
    ceiling.spawn((Transform::from_xyz(0.0, 3.5, 0.0), Collider::aabb_box(Vec3::new(40.0, 0.5, 8.0))));
    let body = body_at(&mut ceiling, Vec3::new(0.0, RADIUS, 0.0));
    for _ in 0..8 {
        drive(&mut ceiling, body, Vec3::X, false, false);
    }
    let speed = motion(&ceiling, body).velocity.x;
    drive(&mut ceiling, body, Vec3::X, false, true);
    assert_eq!(motion(&ceiling, body).velocity.y, 0.0, "the ceiling did not cancel the jump");
    assert!(motion(&ceiling, body).velocity.x > speed - 1.0, "{:?}", motion(&ceiling, body).velocity);
    assert_eq!(motion(&ceiling, body).phase, LocomotionPhase::Fall);
    assert!(landings(&mut ceiling).is_empty(), "a ceiling is not a landing");
}

#[test]
fn landing_keeps_horizontal_momentum_and_emits_one_event() {
    let mut world = World::new();
    floor(&mut world);
    let body = body_at(&mut world, Vec3::new(0.0, RADIUS, 0.0));
    for _ in 0..12 {
        drive(&mut world, body, Vec3::X, false, false);
    }
    drive(&mut world, body, Vec3::X, false, true);
    let mut previous = motion(&world, body).velocity.x;
    let mut landed = false;
    for _ in 0..80 {
        let was_air = matches!(motion(&world, body).support, GroundSupport::Airborne);
        drive(&mut world, body, Vec3::X, false, false);
        let now = motion(&world, body);
        assert!((now.velocity.x - previous).abs() <= ACCEL_STEP + 0.05, "{} -> {}", previous, now.velocity.x);
        if was_air && matches!(now.support, GroundSupport::Supported { .. }) {
            landed = true;
            assert_eq!(now.velocity.y, 0.0);
            assert!(now.velocity.x > 40.0, "landing dropped horizontal speed {:?}", now.velocity);
            assert_ne!(now.phase, LocomotionPhase::Jump);
            break;
        }
        previous = now.velocity.x;
    }
    assert!(landed, "never landed at {:?}", position(&world, body));
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
    drive(&mut world, body, Vec3::X, false, false);
    assert!(landings(&mut world).len() <= 1);
}

#[test]
fn disabling_locomotion_forgets_velocity_and_a_direct_move_stays_free() {
    let mut world = World::new();
    floor(&mut world);
    let body = body_at(&mut world, Vec3::new(0.0, RADIUS, 0.0));
    for _ in 0..12 {
        drive(&mut world, body, Vec3::X, true, false);
    }
    assert!(motion(&world, body).velocity.x > 100.0);
    world.insert(body, (CharacterLocomotion { enabled: false, ..CharacterLocomotion::default() },)).unwrap();
    world.insert(body, (MoveIntent::new(Vec3::new(0.0, 0.0, 1.0)),)).unwrap();
    step(&mut world);
    near(position(&world, body).z, 1.0, 0.05);
    near(position(&world, body).y, RADIUS, 0.05);
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO, "disabled locomotion kept its speed");

    world.insert(body, (CharacterLocomotion::default(),)).unwrap();
    drive(&mut world, body, Vec3::X, true, false);
    near(motion(&world, body).velocity.x, ACCEL_STEP, 0.05);
    assert!(motion(&world, body).velocity.x < 20.0, "old speed returned");

    let mut bare = World::new();
    floor(&mut bare);
    let free = bare.spawn((Transform::from_xyz(0.0, 1.0, 0.0), Collider::sphere(RADIUS), CharacterMotor::default()));
    bare.insert(free, (MoveIntent::new(Vec3::new(0.2, -0.4, 0.6)),)).unwrap();
    step(&mut bare);
    near(position(&bare, free).x, 0.2, 1e-3);
    near(position(&bare, free).y, 0.6, 1e-3);
    near(position(&bare, free).z, 0.6, 1e-3);
}

#[test]
fn a_zero_step_and_plain_sync_do_not_steer() {
    let mut world = World::new();
    floor(&mut world);
    let body = body_at(&mut world, Vec3::new(0.0, RADIUS, 0.0));
    drive(&mut world, body, Vec3::X, false, false);
    let stored = motion(&world, body).velocity;
    let at = position(&world, body);
    world.insert(body, (LocomotionIntent::running(Vec3::X),)).unwrap();
    sync_collision_with_step(&mut world, None);
    assert_eq!(motion(&world, body).velocity, stored);
    assert_eq!(position(&world, body), at);
    sync_collision(&mut world);
    assert_eq!(motion(&world, body).velocity, stored);
    assert_eq!(position(&world, body), at);

    let once = motion(&world, body).velocity.x;
    drive(&mut world, body, Vec3::X, false, false);
    drive(&mut world, body, Vec3::X, false, false);
    near(motion(&world, body).velocity.x, once + ACCEL_STEP * 2.0, 1e-2);
}

#[test]
fn the_same_run_jump_and_slope_replays_for_a_thousand_steps() {
    fn trace() -> Vec<(Vec3, Vec3, LocomotionPhase)> {
        let mut world = World::new();
        floor(&mut world);
        ramp(&mut world, 30.0, 0.2);
        let body = body_at(&mut world, Vec3::new(-8.0, RADIUS, -2.0));
        let mut out = Vec::with_capacity(1000);
        for index in 0..1000 {
            let wish = match index % 140 {
                0..50 => Vec3::X,
                50..80 => Vec3::new(1.0, 0.0, 0.5),
                80..100 => Vec3::ZERO,
                _ => Vec3::new(-0.5, 0.0, 1.0),
            };
            let run = index % 220 < 90;
            let jump = index == 40 || index == 500;
            drive(&mut world, body, wish, run, jump);
            let motion = motion(&world, body);
            let at = position(&world, body);
            assert!(at.x.is_finite() && at.y.is_finite() && at.z.is_finite(), "step {index} {at:?}");
            assert!(motion.velocity.x.is_finite() && motion.velocity.y.is_finite() && motion.velocity.z.is_finite());
            out.push((at, motion.velocity, motion.phase));
        }
        out
    }

    let left = trace();
    let right = trace();
    assert_eq!(left.len(), 1000);
    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        assert_eq!(left.0, right.0, "step {index}");
        assert_eq!(left.1, right.1, "step {index}");
        assert_eq!(left.2, right.2, "step {index}");
    }
}
