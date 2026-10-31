//! Slope traversal against blocking walls. These tests call the collision sync directly.
//!
//! The character is an upright capsule. A 30° ramp rotates a box about Z so its top face has
//! normal about `(-0.5, 0.866, 0)` and passes through the origin. The capsule's lower sphere rests
//! on that face. Uphill is local +X.

use engine_math::{Quat, Transform, Vec3};
use engine_world::{
    sync_collision, sync_collision_with_step, CharacterLocomotion, CharacterMotion, CharacterMotor, Collider, Entity,
    GroundSupport, LocomotionEvent, LocomotionIntent, LocomotionPhase, MoveIntent, MoveResult, MoveStatus, World,
};

const DT: f32 = 1.0 / 60.0;
const RADIUS: f32 = 0.35;
const HALF: f32 = 0.45;
const SPEED: f32 = 90.0;
const ALONG: f32 = SPEED * DT;
const GRAVITY: f32 = -400.0;
const JUMP: f32 = 200.0;

fn step_at(world: &mut World, dt: f32) {
    sync_collision_with_step(world, Some(dt));
}

fn step(world: &mut World) {
    step_at(world, DT);
}

fn capsule(world: &mut World, position: Vec3) -> Entity {
    world.spawn((
        Transform::from_position(position),
        Collider::capsule(RADIUS, HALF),
        CharacterMotor::default(),
        kinematic(),
        CharacterMotion::default(),
    ))
}

fn position(world: &World, entity: Entity) -> Vec3 {
    world.component::<Transform>(entity).unwrap().position
}

fn motion(world: &World, entity: Entity) -> CharacterMotion {
    *world.component::<CharacterMotion>(entity).unwrap()
}

fn wish(world: &mut World, entity: Entity, direction: Vec3) {
    world.insert(entity, (LocomotionIntent::new(direction),)).unwrap();
}

fn landings(world: &mut World) -> Vec<LocomotionEvent> {
    world.promote_events();
    world.events::<LocomotionEvent>()
}

/// One step still reaches the configured speed. The gameplay default accelerates.
fn kinematic() -> CharacterLocomotion {
    CharacterLocomotion {
        acceleration: 1.0e6,
        deceleration: 1.0e6,
        air_acceleration: 1.0e6,
        ..CharacterLocomotion::default()
    }
}

fn near(actual: f32, expected: f32, tol: f32) {
    assert!((actual - expected).abs() <= tol, "{actual} vs {expected} (tol {tol})");
}

struct Ramp {
    entity: Entity,
    normal: Vec3,
}

fn ramp(world: &mut World, degrees: f32, yaw: f32, half: Vec3) -> Ramp {
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(degrees.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    let center = -normal * half.y;
    let entity = world.spawn((Transform::from_position(center).with_rotation(rotation), Collider::aabb_box(half)));
    Ramp { entity, normal }
}

fn on_ramp(ramp: &Ramp) -> Vec3 {
    ramp.normal * RADIUS + Vec3::Y * HALF
}

fn supported_by(world: &World, body: Entity, ground: Entity) -> bool {
    matches!(motion(world, body).support, GroundSupport::Supported { entity, .. } if entity == ground)
}

/// The lower sphere's gap above the plane through the origin. Zero is resting on the face.
fn plane_gap(ramp: &Ramp, at: Vec3) -> f32 {
    (at - Vec3::Y * HALF).dot(ramp.normal.normalized()) - RADIUS
}

fn face_x(world: &mut World, x: f32, y: f32, z: f32, half_z: f32) -> Entity {
    let half = Vec3::new(0.4, 4.0, half_z);
    world.spawn((Transform::from_xyz(x + half.x, y, z), Collider::aabb_box(half)))
}

fn result(world: &World, body: Entity) -> (Vec3, Vec3, MoveStatus, Option<Vec3>) {
    let result = world.component::<MoveResult>(body).unwrap();
    (result.requested, result.applied, result.status, result.recovery_normal)
}

#[test]
fn uphill_input_held_against_a_vertical_wall_does_not_bounce() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(30.0, 2.0, 12.0));
    let start = on_ramp(&slope);
    let body = capsule(&mut world, start);
    face_x(&mut world, start.x + RADIUS + 0.85, start.y + 1.5, 0.0, 8.0);

    let mut stopped = None;
    let mut y_when_stopped = 0.0;
    let mut drift = 0.0_f32;
    for index in 0..300 {
        wish(&mut world, body, Vec3::X);
        let before = position(&world, body);
        step(&mut world);
        let at = position(&world, body);
        let moved = at - before;
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump, "step {index}");
        assert_eq!(motion(&world, body).velocity.x, 0.0, "step {index}");
        assert_eq!(motion(&world, body).velocity.z, 0.0, "step {index}");
        let (_, applied, status, recovery) = result(&world, body);
        assert_ne!(status, MoveStatus::Rejected, "step {index}");
        assert!(recovery.is_none(), "step {index} recovered along {recovery:?}");
        assert!(
            supported_by(&world, body, slope.entity),
            "step {index} left the slope at {at:?} {:?}",
            motion(&world, body)
        );
        assert_eq!(motion(&world, body).velocity.y, 0.0, "step {index}");
        let off_plane = moved.dot(slope.normal).abs();
        assert!(off_plane < 0.02, "step {index} left the support plane by {off_plane}, moved {moved:?}");
        // A stopped uphill wish must not keep the tangent's Y after the wall removes X.
        if moved.x.abs() < 0.02 {
            assert!(moved.y.abs() < 0.02, "step {index} rose without along-slope travel {moved:?} applied {applied:?}");
            if stopped.is_none() {
                stopped = Some(index);
                y_when_stopped = at.y;
            }
            drift = drift.max((at.y - y_when_stopped).abs());
        }
        assert!(plane_gap(&slope, at).abs() < 0.05, "step {index} gap {}", plane_gap(&slope, at));
    }
    let stop_at = stopped.expect("the wall never stopped the capsule");
    assert!(stop_at < 5, "contact took until step {stop_at}");
    assert!(drift < 0.02, "vertical drift after the stop was {drift}");
    assert!(landings(&mut world).is_empty(), "the wall hold landed {:?}", landings(&mut world));
    let end = position(&world, body);
    assert!(end.x < start.x + RADIUS + 0.9, "walked through the wall {end:?}");
    assert!((end.y - y_when_stopped).abs() < 0.02, "end drifted from the stop height");
}

#[test]
fn diagonal_input_slides_along_the_slope_wall_intersection() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(24.0, 2.0, 40.0));
    let start = on_ramp(&slope);
    let body = capsule(&mut world, start);
    face_x(&mut world, start.x + RADIUS + 0.05, start.y + 1.5, 0.0, 30.0);

    wish(&mut world, body, Vec3::new(1.0, 0.0, 1.0));
    step(&mut world);
    assert!(supported_by(&world, body, slope.entity));
    let first = position(&world, body);
    wish(&mut world, body, Vec3::new(1.0, 0.0, 1.0));
    step(&mut world);
    let moved = position(&world, body) - first;
    assert!(moved.z > 0.8, "did not follow the contour {moved:?}");
    assert!(moved.x.abs() < 0.05, "into-wall X remained {moved:?}");
    assert!(moved.y.abs() < 0.05, "contour picked up Y {moved:?}");
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    assert!(motion(&world, body).velocity.x.abs() < 1.0, "{:?}", motion(&world, body).velocity);
    assert!(motion(&world, body).velocity.z > 40.0, "{:?}", motion(&world, body).velocity);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
    assert!(plane_gap(&slope, position(&world, body)).abs() < 0.05);

    let mut blocked = World::new();
    let slope = ramp(&mut blocked, 30.0, 0.0, Vec3::new(24.0, 2.0, 16.0));
    let start = on_ramp(&slope);
    let body = capsule(&mut blocked, start);
    face_x(&mut blocked, start.x + RADIUS + 0.05, start.y + 1.5, 0.0, 8.0);
    face_z(&mut blocked, start.z + RADIUS + 0.05, start.y + 1.5, start.x);
    for _ in 0..40 {
        wish(&mut blocked, body, Vec3::new(1.0, 0.0, 1.0));
        step(&mut blocked);
        assert!(supported_by(&blocked, body, slope.entity));
        assert_eq!(motion(&blocked, body).velocity.y, 0.0);
        assert_ne!(motion(&blocked, body).phase, LocomotionPhase::Jump);
    }
    let end = position(&blocked, body);
    assert!((end.x - start.x).abs() < 0.15, "corner released X {end:?} from {start:?}");
    assert!((end.z - start.z).abs() < 0.15, "corner released Z {end:?}");
    assert!((end.y - start.y).abs() < 0.08, "corner climbed {end:?}");
    assert!(landings(&mut blocked).is_empty());
}

fn face_z(world: &mut World, z: f32, y: f32, x: f32) -> Entity {
    let half = Vec3::new(8.0, 4.0, 0.4);
    world.spawn((Transform::from_xyz(x, y, z + half.z), Collider::aabb_box(half)))
}

#[test]
fn uphill_motion_resumes_after_the_wall_ends() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(30.0, 2.0, 30.0));
    let start = on_ramp(&slope);
    let body = capsule(&mut world, start);
    let wall = face_x(&mut world, start.x + RADIUS + 0.2, start.y + 1.5, 0.0, 1.2);
    let mut passed = false;
    for _ in 0..40 {
        wish(&mut world, body, Vec3::new(1.0, 0.0, 1.0));
        let before = position(&world, body);
        step(&mut world);
        let at = position(&world, body);
        assert!(supported_by(&world, body, slope.entity), "left the slope at {at:?}");
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        if at.z > 1.6 && at.x > before.x + 0.2 && at.y > before.y + 0.05 {
            passed = true;
            break;
        }
    }
    assert!(passed, "never resumed uphill past the wall at {:?}", position(&world, body));
    assert!(landings(&mut world).is_empty());

    let held = position(&world, body);
    world.despawn(wall).unwrap();
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    let resumed = position(&world, body) - held;
    assert!(resumed.x > 0.5 && resumed.y > 0.2, "removing the wall did not restore uphill {resumed:?}");
    assert!(supported_by(&world, body, slope.entity));
}

#[test]
fn downhill_motion_beside_a_wall_keeps_the_downhill_tangent() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(30.0, 2.0, 20.0));
    let start = on_ramp(&slope);
    let body = capsule(&mut world, start);
    face_z(&mut world, start.z + RADIUS + 0.04, start.y + 1.5, start.x);
    wish(&mut world, body, -Vec3::X);
    step(&mut world);
    let moved = position(&world, body) - start;
    assert!(moved.x < -0.8, "downhill X was blocked {moved:?}");
    assert!(moved.y < -0.4, "downhill Y was removed {moved:?}");
    assert!(moved.z.abs() < 0.05, "the side wall pushed Z {moved:?}");
    assert!(supported_by(&world, body, slope.entity));
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    assert!(motion(&world, body).velocity.x < -50.0, "{:?}", motion(&world, body).velocity);
    assert!(motion(&world, body).velocity.z.abs() < 1.0, "{:?}", motion(&world, body).velocity);
}

#[test]
fn a_yawed_wall_keeps_motion_on_the_crease() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(30.0, 2.0, 30.0));
    let start = on_ramp(&slope);
    let body = capsule(&mut world, start);
    let yaw = 25.0_f32.to_radians();
    let half = Vec3::new(0.35, 4.0, 12.0);
    let center = start + Vec3::new(0.9, 1.2, 0.0);
    world.spawn((Transform::from_position(center).with_rotation(Quat::from_rotation_y(yaw)), Collider::aabb_box(half)));

    let mut crease = None;
    let mut along = 0.0_f32;
    let mut blocked_before = false;
    for _ in 0..25 {
        wish(&mut world, body, Vec3::X);
        let before = position(&world, body);
        step(&mut world);
        let at = position(&world, body);
        let moved = at - before;
        assert!(
            supported_by(&world, body, slope.entity),
            "yawed wall became support at {at:?} {:?}",
            motion(&world, body)
        );
        assert_eq!(motion(&world, body).velocity.y, 0.0);
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump);
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Fall);
        let (_, _, _, recovery) = result(&world, body);
        assert!(recovery.is_none());
        if moved.length() > 0.05 {
            let normals = world.component::<MoveResult>(body).unwrap().normals.clone();
            let wall = normals.iter().copied().find(|normal| normal.y.abs() < 0.25);
            if let Some(wall) = wall {
                let axis = slope.normal.cross(wall).normalized();
                // The first blocking step still contains the approach along the slope.
                // Later steps start against the wall, so the committed move is the crease.
                if blocked_before {
                    let off = (moved - axis * moved.dot(axis)).length();
                    assert!(off < 0.05, "left the crease {moved:?} wall {wall:?} off {off}");
                }
                along += moved.dot(axis).abs();
                crease = Some(axis);
                blocked_before = true;
            }
        }
    }
    assert!(crease.is_some(), "the yawed wall never blocked a step");
    assert!(along > 1.0, "the crease did not carry the body, accumulated {along}");
    assert!(landings(&mut world).is_empty());
    assert!(plane_gap(&slope, position(&world, body)).abs() < 0.05);
}

#[test]
fn pushing_into_a_steep_face_does_not_climb() {
    let mut world = World::new();
    let face = ramp(&mut world, 60.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let start = on_ramp(&face);
    let body = capsule(&mut world, start);
    let mut highest = start.y;
    for _ in 0..60 {
        wish(&mut world, body, Vec3::X);
        step(&mut world);
        let at = position(&world, body);
        highest = highest.max(at.y);
        assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
        assert!(motion(&world, body).velocity.y < 0.0);
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Walk);
        let (_, applied, _, recovery) = result(&world, body);
        assert!(recovery.is_none());
        assert!(applied.y <= 0.02, "step climbed {applied:?} at {at:?}");
    }
    assert!(highest < start.y + 0.08, "sustained climb to {highest} from {}", start.y);
    assert!(
        position(&world, body).y < start.y - 0.5,
        "gravity did not carry the body down {:?}",
        position(&world, body)
    );
    assert!(motion(&world, body).velocity.y < -200.0, "vertical speed did not keep integrating");
    assert!(landings(&mut world).is_empty());
}

#[test]
fn standing_and_walking_a_clear_slope_stay_on_the_tangent() {
    for degrees in [30.0_f32, 45.0] {
        let mut world = World::new();
        let slope = ramp(&mut world, degrees, 0.0, Vec3::new(24.0, 2.0, 16.0));
        let body = capsule(&mut world, on_ramp(&slope));
        step(&mut world);
        let settled = position(&world, body);
        assert!(supported_by(&world, body, slope.entity), "{degrees}°");
        for _ in 0..1000 {
            step(&mut world);
        }
        near_vec(position(&world, body), settled, 1e-3);
        assert!(supported_by(&world, body, slope.entity), "{degrees}°");
        assert_eq!(motion(&world, body).velocity, Vec3::ZERO, "{degrees}°");
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle, "{degrees}°");
        assert!(landings(&mut world).is_empty(), "{degrees}°");
    }

    let mut up = World::new();
    let slope = ramp(&mut up, 30.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let body = capsule(&mut up, on_ramp(&slope));
    wish(&mut up, body, Vec3::X);
    step(&mut up);
    let moved = position(&up, body) - on_ramp(&slope);
    near(moved.length(), ALONG, 0.05);
    assert!(moved.y > 0.4, "uphill lost its Y {moved:?}");
    assert!(supported_by(&up, body, slope.entity));
    assert_eq!(motion(&up, body).phase, LocomotionPhase::Walk);

    let mut down = World::new();
    let slope = ramp(&mut down, 30.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let body = capsule(&mut down, on_ramp(&slope));
    wish(&mut down, body, -Vec3::X);
    step(&mut down);
    let moved = position(&down, body) - on_ramp(&slope);
    near(moved.length(), ALONG, 0.05);
    assert!(moved.y < -0.4, "downhill lost its Y {moved:?}");
    assert_eq!(motion(&down, body).phase, LocomotionPhase::Walk);
}

fn near_vec(actual: Vec3, expected: Vec3, tol: f32) {
    let gap = (actual - expected).length();
    assert!(gap <= tol, "{actual:?} vs {expected:?} (gap {gap}, tol {tol})");
}

#[test]
fn a_jump_from_the_slope_lands_once() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(80.0, 2.0, 24.0));
    let start = on_ramp(&slope);
    let body = capsule(&mut world, start);
    step(&mut world);
    world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
    step(&mut world);
    let takeoff = position(&world, body);
    assert!(takeoff.y > start.y + 1.0, "takeoff {takeoff:?} from {start:?}");
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Jump);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    near(motion(&world, body).velocity.y, JUMP, 1e-3);
    assert!(landings(&mut world).is_empty());

    let mut landed = false;
    for _ in 0..90 {
        step(&mut world);
        if supported_by(&world, body, slope.entity) && motion(&world, body).velocity.y == 0.0 {
            landed = true;
            break;
        }
    }
    assert!(landed, "never landed at {:?} {:?}", position(&world, body), motion(&world, body));
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
    assert_ne!(motion(&world, body).phase, LocomotionPhase::Fall);
}

#[test]
fn flat_ground_wall_sliding_matches_an_unrestricted_motor() {
    let diagonal = Vec3::new(1.0, 0.0, 1.0).normalized();
    let travel = diagonal * ALONG;

    let mut locomotion = World::new();
    locomotion.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(20.0, 0.5, 20.0))));
    let body = capsule(&mut locomotion, Vec3::new(0.0, HALF + RADIUS, 0.0));
    face_x(&mut locomotion, RADIUS, 2.0, 0.0, 8.0);
    wish(&mut locomotion, body, diagonal);
    step(&mut locomotion);
    let moved = position(&locomotion, body) - Vec3::new(0.0, HALF + RADIUS, 0.0);
    near(moved.z, travel.z, 0.04);
    assert!(moved.x.abs() < 0.04, "flat slide kept X {moved:?}");
    assert!(moved.y.abs() < 0.04, "flat slide kept Y {moved:?}");
    assert_eq!(motion(&locomotion, body).phase, LocomotionPhase::Walk);
    assert!(supported_by_any(&locomotion, body));

    let mut motor = World::new();
    motor.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(20.0, 0.5, 20.0))));
    let body = motor.spawn((
        Transform::from_xyz(0.0, HALF + RADIUS, 0.0),
        Collider::capsule(RADIUS, HALF),
        CharacterMotor::default(),
    ));
    face_x(&mut motor, RADIUS, 2.0, 0.0, 8.0);
    motor.insert(body, (MoveIntent::new(Vec3::new(travel.x, 0.0, travel.z)),)).unwrap();
    sync_collision(&mut motor);
    let raw = position(&motor, body) - Vec3::new(0.0, HALF + RADIUS, 0.0);
    near(raw.z, moved.z, 0.04);
    assert!(raw.y.abs() < 0.04, "raw motor climbed {raw:?}");
    assert!(raw.x.abs() < 0.04, "raw motor kept X {raw:?}");
}

fn supported_by_any(world: &World, body: Entity) -> bool {
    matches!(motion(world, body).support, GroundSupport::Supported { .. })
}

#[test]
fn a_raw_motor_still_slides_up_a_steep_face() {
    let mut motor = World::new();
    let face = ramp(&mut motor, 60.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let start = face.normal * 0.5;
    let body = motor.spawn((Transform::from_position(start), Collider::sphere(0.5), CharacterMotor::default()));
    motor.insert(body, (MoveIntent::new(Vec3::new(ALONG, GRAVITY * DT * DT, 0.0)),)).unwrap();
    sync_collision(&mut motor);
    let applied = motor.component::<MoveResult>(body).unwrap().applied;
    assert!(applied.y > 0.2, "the unrestricted sweep no longer slides up the face {applied:?}");

    let mut locomotion = World::new();
    let face = ramp(&mut locomotion, 60.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let body = locomotion.spawn((
        Transform::from_position(face.normal * 0.5),
        Collider::sphere(0.5),
        CharacterMotor::default(),
        kinematic(),
        CharacterMotion::default(),
    ));
    wish(&mut locomotion, body, Vec3::X);
    step(&mut locomotion);
    let applied = locomotion.component::<MoveResult>(body).unwrap().applied;
    assert!(applied.y < 0.0, "locomotion copied the raw climb {applied:?}");
    assert_eq!(motion(&locomotion, body).support, GroundSupport::Airborne);
    assert_eq!(motion(&locomotion, body).phase, LocomotionPhase::Fall);
}

#[test]
fn the_wall_hold_replays_in_two_worlds() {
    fn trace() -> Vec<Vec3> {
        let mut world = World::new();
        let slope = ramp(&mut world, 30.0, 0.2, Vec3::new(24.0, 2.0, 20.0));
        let start = on_ramp(&slope);
        let body = capsule(&mut world, start);
        face_x(&mut world, start.x + 0.7, start.y + 1.5, 0.0, 6.0);
        let mut out = Vec::new();
        for index in 0..40 {
            let direction = if index < 20 { Vec3::X } else { Vec3::new(0.4, 0.0, 1.0) };
            wish(&mut world, body, direction);
            step(&mut world);
            out.push(position(&world, body));
        }
        out
    }

    assert_eq!(trace(), trace());
}

#[derive(Clone, Copy, Debug)]
struct Crest {
    grounded: u32,
    steps: u32,
    jumped: bool,
    fell: bool,
    start_y: f32,
    peak_y: f32,
    end_y: f32,
    end_x: f32,
    airborne: bool,
    returned: bool,
}

fn ramp_edge(world: &mut World, degrees: f32, uphill_at_origin: bool) -> Entity {
    let rotation = Quat::from_rotation_z(degrees.to_radians());
    let half = Vec3::new(12.0, 2.0, 16.0);
    let along = if uphill_at_origin { half.x } else { -half.x };
    let center = -rotation.rotate(Vec3::new(along, half.y, 0.0));
    world.spawn((Transform::from_position(center).with_rotation(rotation), Collider::aabb_box(half)))
}

fn ride_crest(speed: f32, dt: f32, degrees: f32, with_far_side: bool) -> Crest {
    let mut world = World::new();
    let left = ramp_edge(&mut world, degrees, true);
    let right = with_far_side.then(|| ramp_edge(&mut world, -degrees, false));
    let rotation = Quat::from_rotation_z(degrees.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    let start = normal * RADIUS + Vec3::Y * HALF + rotation.rotate(-Vec3::X) * 2.5;
    let body = capsule(&mut world, start);
    world.insert(body, (CharacterLocomotion { speed, ..kinematic() },)).unwrap();
    let start_y = position(&world, body).y;
    let mut peak_y = start_y;
    let mut grounded = 0;
    let mut jumped = false;
    let mut fell = false;
    let mut returned = false;
    let mut airborne = false;
    let mut left_ground = false;
    let steps = 28;
    for _ in 0..steps {
        wish(&mut world, body, Vec3::X);
        step_at(&mut world, dt);
        let at = position(&world, body);
        peak_y = peak_y.max(at.y);
        match motion(&world, body).phase {
            LocomotionPhase::Jump => jumped = true,
            LocomotionPhase::Fall => fell = true,
            _ => {}
        }
        if supported_by(&world, body, left) || right.is_some_and(|right| supported_by(&world, body, right)) {
            grounded += 1;
            if left_ground {
                returned = true;
            }
        } else {
            airborne = true;
            left_ground = true;
        }
    }
    let end = position(&world, body);
    Crest { grounded, steps, jumped, fell, start_y, peak_y, end_y: end.y, end_x: end.x, airborne, returned }
}

#[test]
fn continuous_slope_travel_stays_grounded_at_several_speeds() {
    for speed in [15.0_f32, 45.0, 90.0] {
        let mut world = World::new();
        let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(80.0, 2.0, 16.0));
        let body = capsule(&mut world, on_ramp(&slope));
        world.insert(body, (CharacterLocomotion { speed, ..kinematic() },)).unwrap();
        let mut previous = position(&world, body).y;
        for _ in 0..8 {
            wish(&mut world, body, Vec3::X);
            step(&mut world);
            let y = position(&world, body).y;
            assert!(y > previous, "speed {speed} stopped climbing");
            previous = y;
            assert!(supported_by(&world, body, slope.entity), "speed {speed}");
            assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump);
            assert_ne!(motion(&world, body).phase, LocomotionPhase::Fall);
            assert_eq!(motion(&world, body).velocity.y, 0.0);
        }
    }
}

#[test]
fn sharp_crests_separate_from_ledges_without_raising_stick() {
    let gentle = ride_crest(15.0, DT, 8.0, true);
    assert_eq!(gentle.grounded, gentle.steps, "a short step left a gentle crest {gentle:?}");
    assert!(!gentle.jumped && !gentle.fell, "{gentle:?}");
    assert!(gentle.peak_y > gentle.start_y + 0.15 && gentle.end_y < gentle.peak_y, "{gentle:?}");

    let sharp_slow = ride_crest(15.0, DT, 20.0, true);
    assert_eq!(sharp_slow.grounded, sharp_slow.steps, "speed 15 left the 20° crest {sharp_slow:?}");
    assert!(!sharp_slow.jumped, "{sharp_slow:?}");

    // At 90 units/second one step is 1.5 units. The support window is skin + stick (0.09), so the
    // old tangent can leave a sharp far face for a step and land on it again. That is separation
    // followed by a real landing, not a jump and not a stick pull across a gap. `stick` stays 0.08.
    for (speed, dt) in [(45.0, DT), (90.0, DT), (90.0, 1.0 / 120.0), (90.0, 1.0 / 30.0)] {
        let sharp = ride_crest(speed, dt, 20.0, true);
        assert!(!sharp.jumped, "speed {speed} dt {dt} jumped {sharp:?}");
        assert!(sharp.peak_y < sharp.start_y + 2.5, "speed {speed} dt {dt} launched {sharp:?}");
        assert!(sharp.end_x > 2.0, "speed {speed} dt {dt} did not cross the crest {sharp:?}");
    }

    for speed in [15.0_f32, 45.0, 90.0] {
        let ledge = ride_crest(speed, DT, 30.0, false);
        assert!(ledge.airborne, "speed {speed} never left the ledge {ledge:?}");
        assert!(ledge.end_y < ledge.peak_y - 0.2, "speed {speed} did not fall {ledge:?}");
        assert!(ledge.end_x > 0.5, "speed {speed} did not travel off the ledge {ledge:?}");
        assert!(!ledge.returned, "speed {speed} snapped back onto the ledge {ledge:?}");
        assert!(!ledge.jumped, "{ledge:?}");
    }
}

/// A sharp 20° crest at walk 90 and run 140 can leave the ground. Measured on this fixture,
/// one fixed step of 1/60 s, the lower sphere's gap above the far plane peaked at about 1.44
/// (walk) and 4.49 (run). Each case emitted one landing and then stayed down. A smooth 30°
/// ramp did not. A real ledge was not pulled back. `stick` stays 0.08.
#[test]
fn running_speed_crosses_a_sharp_crest_without_a_jump_loop() {
    for speed in [90.0_f32, 140.0] {
        for dt in [1.0 / 120.0, DT, 1.0 / 30.0] {
            let sharp = ride_running_crest(speed, dt, 20.0, true, true);
            assert!(sharp.steps >= 8 && sharp.finite, "speed {speed} dt {dt} left the finite range {sharp:?}");
            assert!(!sharp.jumped, "speed {speed} dt {dt} treated separation as a jump {sharp:?}");
            assert!(sharp.end_x > 4.0, "speed {speed} dt {dt} did not cross {sharp:?}");
            assert_eq!(sharp.airborne_steps, sharp.hop_steps, "speed {speed} dt {dt} left the ground twice {sharp:?}");
            assert!(sharp.landings <= 1, "speed {speed} dt {dt} landed repeatedly {sharp:?}");
            assert_eq!(sharp.landings, sharp.events, "speed {speed} dt {dt} landing count {sharp:?}");
            if sharp.hop_steps > 0 {
                assert_eq!(sharp.landings, 1, "speed {speed} dt {dt} left the crest and did not land {sharp:?}");
                assert!(sharp.returned && sharp.fell, "{sharp:?}");
                assert!(sharp.peak_gap < 8.0, "speed {speed} dt {dt} separation {sharp:?}");
            }
        }
    }
    for dt in [1.0 / 120.0, DT, 1.0 / 30.0] {
        let mut smooth = World::new();
        let slope = ramp(&mut smooth, 30.0, 0.0, Vec3::new(80.0, 2.0, 16.0));
        let body = capsule(&mut smooth, on_ramp(&slope));
        smooth.insert(body, (CharacterLocomotion::default(),)).unwrap();
        for _ in 0..16 {
            smooth.insert(body, (LocomotionIntent::running(Vec3::X),)).unwrap();
            step_at(&mut smooth, dt);
            assert!(supported_by(&smooth, body, slope.entity), "dt {dt} left the continuous ramp");
            let phase = motion(&smooth, body).phase;
            assert_ne!(phase, LocomotionPhase::Jump, "dt {dt}");
            assert_ne!(phase, LocomotionPhase::Fall, "dt {dt}");
        }

        let mut cruise = World::new();
        let slope = ramp(&mut cruise, 30.0, 0.0, Vec3::new(80.0, 2.0, 16.0));
        let body = capsule(&mut cruise, on_ramp(&slope));
        cruise.insert(body, (CharacterLocomotion { run_speed: 140.0, ..kinematic() },)).unwrap();
        let mut stored = motion(&cruise, body);
        stored.velocity.x = 140.0 * slope.normal.y.max(0.05);
        cruise.insert(body, (stored,)).unwrap();
        for _ in 0..8 {
            cruise.insert(body, (LocomotionIntent::running(Vec3::X),)).unwrap();
            step_at(&mut cruise, dt);
            assert!(supported_by(&cruise, body, slope.entity), "dt {dt} a seeded run left the smooth ramp");
            let phase = motion(&cruise, body).phase;
            assert_ne!(phase, LocomotionPhase::Jump, "dt {dt}");
            assert_ne!(phase, LocomotionPhase::Fall, "dt {dt}");
        }

        let ledge = ride_running_crest(140.0, dt, 30.0, false, true);
        assert!(ledge.steps >= 8 && ledge.airborne_steps > 0, "dt {dt} never left the ledge {ledge:?}");
        assert!(!ledge.returned, "dt {dt} snapped back onto the ledge {ledge:?}");
        assert!(!ledge.jumped, "{ledge:?}");
        assert_eq!(ledge.landings, 0, "dt {dt} landed off a ledge with no floor {ledge:?}");
        assert_eq!(ledge.landings, ledge.events, "ledge dt {dt} {ledge:?}");
        assert!(ledge.peak_gap.is_finite() && ledge.fell, "{ledge:?}");
    }
}

#[derive(Debug)]
struct RunningCrest {
    steps: u32,
    airborne_steps: u32,
    landings: u32,
    events: u32,
    hop_steps: u32,
    jumped: bool,
    fell: bool,
    returned: bool,
    finite: bool,
    end_x: f32,
    peak_gap: f32,
}

/// `joined` builds the far face. `seed_run` starts already at `speed` along +X; otherwise the body
/// accelerates with the gameplay run rate from rest.
fn ride_running_crest(speed: f32, dt: f32, degrees: f32, joined: bool, seed_run: bool) -> RunningCrest {
    let mut world = World::new();
    let rotation = Quat::from_rotation_z(degrees.to_radians());
    let half = Vec3::new(80.0, 2.0, 16.0);
    let left_center = -rotation.rotate(Vec3::new(half.x, half.y, 0.0));
    let left = world.spawn((Transform::from_position(left_center).with_rotation(rotation), Collider::aabb_box(half)));
    let right = joined.then(|| {
        let right_rotation = Quat::from_rotation_z(-degrees.to_radians());
        let right_center = -right_rotation.rotate(Vec3::new(-half.x, half.y, 0.0));
        world.spawn((Transform::from_position(right_center).with_rotation(right_rotation), Collider::aabb_box(half)))
    });
    let normal = rotation.rotate(Vec3::Y).normalized();
    let start = normal * RADIUS + Vec3::Y * HALF + rotation.rotate(-Vec3::X) * 28.0;
    let body = capsule(&mut world, start);
    let settings = if seed_run {
        CharacterLocomotion { speed, run_speed: speed + 10.0, acceleration: 1.0e6, ..kinematic() }
    } else {
        CharacterLocomotion { speed: 90.0, run_speed: speed, ..CharacterLocomotion::default() }
    };
    world.insert(body, (settings,)).unwrap();
    if seed_run {
        let mut stored = motion(&world, body);
        let share = normal.y.max(0.05);
        stored.velocity.x = speed * share;
        world.insert(body, (stored,)).unwrap();
    }
    // Cross the crest and keep going until a landing or the far face is behind the body.
    // A 40-unit face is long enough to catch an ordinary hop. It is not ground snapping.
    let steps = ((70.0 / (speed * dt)).ceil() as u32).clamp(16, 90);
    let mut airborne_steps = 0;
    let mut hop_steps = 0;
    let mut touchdowns = 0;
    let mut events = 0;
    let mut jumped = false;
    let mut fell = false;
    let mut returned = false;
    let mut left_ground = false;
    let mut finite = true;
    let mut grounded_after = 0;
    let mut peak_gap: f32 = 0.0;
    for _ in 0..steps {
        world.insert(body, (LocomotionIntent::running(Vec3::X),)).unwrap();
        step_at(&mut world, dt);
        events = landings(&mut world).len() as u32;
        let at = position(&world, body);
        finite &= at.x.is_finite() && at.y.is_finite() && at.z.is_finite();
        let state = motion(&world, body);
        finite &= state.velocity.x.is_finite() && state.velocity.y.is_finite() && state.velocity.z.is_finite();
        match state.phase {
            LocomotionPhase::Jump => jumped = true,
            LocomotionPhase::Fall => fell = true,
            _ => {}
        }
        let supported =
            supported_by(&world, body, left) || right.is_some_and(|right| supported_by(&world, body, right));
        if supported {
            if left_ground {
                returned = true;
                touchdowns += 1;
                left_ground = false;
            }
            if returned {
                grounded_after += 1;
                if grounded_after >= 12 {
                    break;
                }
            }
        } else {
            airborne_steps += 1;
            if touchdowns == 0 {
                hop_steps += 1;
            }
            left_ground = true;
            let plane = if at.x >= 0.0 && joined {
                Quat::from_rotation_z(-degrees.to_radians()).rotate(Vec3::Y).normalized()
            } else {
                normal
            };
            let gap = (at - Vec3::Y * HALF).dot(plane) - RADIUS;
            if gap.is_finite() {
                peak_gap = peak_gap.max(gap);
            }
        }
    }
    RunningCrest {
        steps,
        airborne_steps,
        landings: touchdowns,
        events,
        hop_steps,
        jumped,
        fell,
        returned,
        finite,
        end_x: position(&world, body).x,
        peak_gap,
    }
}
