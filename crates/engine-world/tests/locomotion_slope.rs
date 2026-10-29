//! Walkable slopes and ground adhesion. These tests call the collision sync directly.
//!
//! A 30° ramp rotates a box about Z so its top face has normal about `(-0.5, 0.866, 0)`. The face
//! passes through the origin. A sphere of radius 0.5 rests with its center along that normal. Speed
//! 90 at 60 Hz asks for 1.5 units along the surface. Uphill +X is about `(1.299, 0.750, 0)` before
//! the motor cancels the stick. Adhesion is `stick` along the negated support normal.

use engine_math::{Quat, Transform, Vec3};
use engine_world::{
    sync_collision_with_step, CharacterLocomotion, CharacterMotion, CharacterMotor, Collider, Entity, GroundSupport,
    LocomotionEvent, LocomotionIntent, LocomotionPhase, MoveIntent, MoveResult, World,
};

const DT: f32 = 1.0 / 60.0;
const RADIUS: f32 = 0.5;
const SPEED: f32 = 90.0;
const ALONG: f32 = SPEED * DT;
const GRAVITY: f32 = -400.0;
const JUMP: f32 = 200.0;

fn step(world: &mut World) {
    sync_collision_with_step(world, Some(DT));
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

fn wish(world: &mut World, entity: Entity, direction: Vec3) {
    world.insert(entity, (LocomotionIntent::new(direction),)).unwrap();
}

fn landings(world: &mut World) -> Vec<LocomotionEvent> {
    world.promote_events();
    world.events::<LocomotionEvent>()
}

fn near(actual: f32, expected: f32, tol: f32) {
    assert!((actual - expected).abs() <= tol, "{actual} vs {expected} (tol {tol})");
}

fn near_vec(actual: Vec3, expected: Vec3, tol: f32) {
    let gap = (actual - expected).length();
    assert!(gap <= tol, "{actual:?} vs {expected:?} (gap {gap}, tol {tol})");
}

struct Ramp {
    entity: Entity,
    normal: Vec3,
    rotation: Quat,
    half: Vec3,
}

/// A box whose top face passes through the origin and rises along local +X.
fn ramp(world: &mut World, degrees: f32, yaw: f32, half: Vec3) -> Ramp {
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(degrees.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    let center = -normal * half.y;
    let entity = world.spawn((Transform::from_position(center).with_rotation(rotation), Collider::aabb_box(half)));
    Ramp { entity, normal, rotation, half }
}

fn on_ramp(ramp: &Ramp) -> Vec3 {
    ramp.normal * RADIUS
}

fn support_normal(world: &World, body: Entity) -> Vec3 {
    match motion(world, body).support {
        GroundSupport::Supported { normal, .. } => normal,
        other => panic!("expected support, got {other:?}"),
    }
}

fn supported_by(world: &World, body: Entity, ground: Entity) -> bool {
    matches!(motion(world, body).support, GroundSupport::Supported { entity, .. } if entity == ground)
}

#[test]
fn uphill_and_downhill_on_a_30_degree_slope_follow_the_tangent() {
    let mut up = World::new();
    let slope = ramp(&mut up, 30.0, 0.0, Vec3::new(40.0, 2.0, 20.0));
    near(slope.normal.x, -0.5, 1e-4);
    near(slope.normal.y, 0.8660254, 1e-4);
    let body = walker(&mut up, on_ramp(&slope));
    wish(&mut up, body, Vec3::X);
    step(&mut up);
    let moved = position(&up, body) - on_ramp(&slope);
    near_vec(moved, Vec3::new(1.299, 0.750, 0.0), 2e-2);
    near(moved.length(), ALONG, 2e-2);
    assert!(moved.y > 0.5, "uphill gains Y {moved:?}");
    assert!(moved.z.abs() < 1e-3, "no sideways drift {moved:?}");
    let requested = up.component::<MoveResult>(body).unwrap().requested;
    let normal = support_normal(&up, body);
    let adhesion = -normal.normalized() * 0.08;
    near_vec(requested - adhesion, Vec3::new(1.299, 0.750, 0.0), 2e-2);
    assert_eq!(motion(&up, body).phase, LocomotionPhase::Walk);
    assert_eq!(motion(&up, body).velocity, Vec3::ZERO);
    assert!(supported_by(&up, body, slope.entity));

    let mut down = World::new();
    let slope = ramp(&mut down, 30.0, 0.0, Vec3::new(40.0, 2.0, 20.0));
    let body = walker(&mut down, on_ramp(&slope));
    wish(&mut down, body, -Vec3::X);
    step(&mut down);
    let moved = position(&down, body) - on_ramp(&slope);
    near_vec(moved, Vec3::new(-1.299, -0.750, 0.0), 2e-2);
    near(moved.length(), ALONG, 2e-2);
    assert!(moved.y < -0.5, "downhill loses Y {moved:?}");
    assert_eq!(motion(&down, body).phase, LocomotionPhase::Walk);
    assert_eq!(motion(&down, body).velocity, Vec3::ZERO, "downhill walking does not store a fall");
    assert!(supported_by(&down, body, slope.entity));
    let plane = position(&down, body).dot(slope.normal) - RADIUS;
    assert!(plane.abs() < 0.02, "downhill stays on the surface, gap {plane}");
}

#[test]
fn a_diagonal_wish_and_a_yawed_slope_keep_surface_speed() {
    let mut straight = World::new();
    let slope = ramp(&mut straight, 30.0, 0.0, Vec3::new(40.0, 2.0, 20.0));
    let body = walker(&mut straight, on_ramp(&slope));
    wish(&mut straight, body, Vec3::X);
    step(&mut straight);
    let axis = (position(&straight, body) - on_ramp(&slope)).length();

    let mut diagonal = World::new();
    let slope = ramp(&mut diagonal, 30.0, 0.0, Vec3::new(40.0, 2.0, 20.0));
    let body = walker(&mut diagonal, on_ramp(&slope));
    wish(&mut diagonal, body, Vec3::new(1.0, 4.0, 1.0));
    step(&mut diagonal);
    let moved = position(&diagonal, body) - on_ramp(&slope);
    near(moved.length(), axis, 2e-2);
    assert!(moved.x > 0.2 && moved.z > 0.2, "diagonal should move on both axes {moved:?}");
    assert!(moved.y > 0.2, "the X part of a diagonal still climbs {moved:?}");

    let mut yawed = World::new();
    let yaw = std::f32::consts::FRAC_PI_2;
    let slope = ramp(&mut yawed, 30.0, yaw, Vec3::new(40.0, 2.0, 20.0));
    let body = walker(&mut yawed, on_ramp(&slope));
    wish(&mut yawed, body, -Vec3::Z);
    step(&mut yawed);
    let moved = position(&yawed, body) - on_ramp(&slope);
    let expected = Quat::from_rotation_y(yaw).rotate(Vec3::new(1.299, 0.750, 0.0));
    near_vec(moved, expected, 2e-2);
    assert_eq!(motion(&yawed, body).phase, LocomotionPhase::Walk);

    let mut contour = World::new();
    let slope = ramp(&mut contour, 30.0, 0.0, Vec3::new(20.0, 2.0, 40.0));
    let body = walker(&mut contour, on_ramp(&slope));
    wish(&mut contour, body, Vec3::Z);
    step(&mut contour);
    let moved = position(&contour, body) - on_ramp(&slope);
    near_vec(moved, Vec3::new(0.0, 0.0, ALONG), 2e-2);
    assert_eq!(motion(&contour, body).phase, LocomotionPhase::Walk);
}

#[test]
fn standing_on_a_30_and_a_45_degree_slope_does_not_creep() {
    for degrees in [30.0_f32, 45.0] {
        let mut world = World::new();
        let slope = ramp(&mut world, degrees, 0.0, Vec3::new(20.0, 2.0, 20.0));
        let body = walker(&mut world, on_ramp(&slope));
        step(&mut world);
        let settled = position(&world, body);
        assert!(supported_by(&world, body, slope.entity), "{degrees}°");
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle, "{degrees}°");
        for _ in 0..1000 {
            step(&mut world);
        }
        let end = position(&world, body);
        near_vec(end, settled, 1e-3);
        assert!(supported_by(&world, body, slope.entity), "{degrees}° drifted off support");
        assert_eq!(motion(&world, body).phase, LocomotionPhase::Idle, "{degrees}°");
        assert_eq!(motion(&world, body).velocity, Vec3::ZERO, "{degrees}°");
        assert!(landings(&mut world).is_empty(), "{degrees}° standing is not a landing");
        let applied = world.component::<MoveResult>(body).unwrap().applied;
        assert!(applied.length() < 1e-3, "{degrees}° adhesion walked the body {applied:?}");
    }
}

#[test]
fn slope_angles_are_accepted_only_inside_the_limit() {
    let cases =
        [(0.0_f32, true), (15.0, true), (30.0, true), (45.0, true), (45.05, false), (60.0, false), (89.0, false)];
    for (degrees, walkable) in cases {
        let mut world = World::new();
        let slope = ramp(&mut world, degrees, 0.2, Vec3::new(20.0, 2.0, 20.0));
        let body = walker(&mut world, on_ramp(&slope));
        step(&mut world);
        let is_support = supported_by(&world, body, slope.entity);
        assert_eq!(is_support, walkable, "{degrees}° support was {is_support}");
        if walkable {
            assert_eq!(motion(&world, body).velocity.y, 0.0, "{degrees}°");
            wish(&mut world, body, Vec3::X);
            let start = position(&world, body);
            step(&mut world);
            let moved = position(&world, body) - start;
            near(moved.length(), ALONG, 0.05);
            assert_eq!(motion(&world, body).velocity, Vec3::ZERO, "{degrees}° stored a fall");
        } else {
            assert_eq!(motion(&world, body).support, GroundSupport::Airborne, "{degrees}°");
            assert!(motion(&world, body).velocity.y < 0.0, "{degrees}° kept gravity");
            assert!(landings(&mut world).is_empty(), "{degrees}°");
        }
    }
}

#[test]
fn a_vertical_wall_is_not_ground_even_at_ninety_degrees() {
    let mut world = World::new();
    world.spawn((Transform::from_xyz(-0.5, 2.0, 0.0), Collider::aabb_box(Vec3::new(0.5, 4.0, 4.0))));
    let body = walker(&mut world, Vec3::new(RADIUS, 2.0, 0.0));
    world
        .insert(
            body,
            (CharacterLocomotion { max_slope: std::f32::consts::FRAC_PI_2, ..CharacterLocomotion::default() },),
        )
        .unwrap();
    let start = position(&world, body);
    step(&mut world);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert!(motion(&world, body).velocity.y < 0.0);
    assert!(position(&world, body).y < start.y);
    assert!(landings(&mut world).is_empty());
    assert_ne!(motion(&world, body).phase, LocomotionPhase::Walk);
}

#[test]
fn a_near_vertical_slope_is_walkable_only_inside_ninety_degrees() {
    let mut rejected = World::new();
    let slope = ramp(&mut rejected, 89.0, 0.0, Vec3::new(30.0, 1.0, 10.0));
    let body = walker(&mut rejected, on_ramp(&slope));
    step(&mut rejected);
    assert_eq!(motion(&rejected, body).support, GroundSupport::Airborne);

    let mut accepted = World::new();
    let slope = ramp(&mut accepted, 89.0, 0.0, Vec3::new(30.0, 1.0, 10.0));
    let body = walker(&mut accepted, on_ramp(&slope));
    accepted
        .insert(
            body,
            (CharacterLocomotion { max_slope: std::f32::consts::FRAC_PI_2, ..CharacterLocomotion::default() },),
        )
        .unwrap();
    step(&mut accepted);
    assert!(supported_by(&accepted, body, slope.entity));
    assert_eq!(motion(&accepted, body).velocity, Vec3::ZERO);
    assert_eq!(motion(&accepted, body).phase, LocomotionPhase::Idle);
}

#[test]
fn flat_ground_transitions_onto_a_slope_and_back() {
    let mut onto = World::new();
    let flat = onto.spawn((Transform::from_xyz(-14.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(14.0, 0.5, 16.0))));
    let slope = ramp(&mut onto, 30.0, 0.0, Vec3::new(36.0, 2.0, 16.0));
    let body = walker(&mut onto, Vec3::new(-6.0, RADIUS, 0.0));
    let mut climbed = false;
    let mut saw_flat = false;
    let mut saw_slope = false;
    for _ in 0..8 {
        wish(&mut onto, body, Vec3::X);
        let before = position(&onto, body).y;
        step(&mut onto);
        if supported_by(&onto, body, flat) {
            saw_flat = true;
        }
        if supported_by(&onto, body, slope.entity) {
            saw_slope = true;
            if position(&onto, body).y > before + 0.2 {
                climbed = true;
            }
        }
        assert_eq!(motion(&onto, body).velocity.y, 0.0);
    }
    assert!(saw_flat && saw_slope, "flat {saw_flat} slope {saw_slope}");
    assert!(climbed, "never gained height on the slope, at {:?}", position(&onto, body));
    assert!(position(&onto, body).y > RADIUS + 0.4, "{:?}", position(&onto, body));

    let mut back = World::new();
    let slope = ramp(&mut back, 30.0, 0.0, Vec3::new(8.0, 2.0, 12.0));
    let uphill = slope.rotation.rotate(Vec3::X);
    let lip = uphill * slope.half.x;
    let deck_y = lip.y;
    let deck = back
        .spawn((Transform::from_xyz(lip.x + 10.0, deck_y - 0.5, 0.0), Collider::aabb_box(Vec3::new(10.0, 0.5, 12.0))));
    let body = walker(&mut back, on_ramp(&slope));
    let mut reached = false;
    for _ in 0..12 {
        wish(&mut back, body, Vec3::X);
        step(&mut back);
        if supported_by(&back, body, deck) {
            reached = true;
            break;
        }
    }
    assert!(reached, "did not reach the deck at {:?}", position(&back, body));
    let rest = position(&back, body).y;
    near(rest, deck_y + RADIUS, 0.08);
    wish(&mut back, body, Vec3::X);
    step(&mut back);
    near(position(&back, body).y, rest, 0.05);
    assert_eq!(motion(&back, body).velocity, Vec3::ZERO);
    assert_eq!(motion(&back, body).phase, LocomotionPhase::Walk);
}

#[test]
fn a_crest_rises_then_descends_without_leaving_the_ground() {
    let mut world = World::new();
    let left = ramp_edge(&mut world, 20.0, true);
    let right = ramp_edge(&mut world, -20.0, false);
    // A default step is 1.5 units. The support window is only skin + stick (0.09), so that step
    // continues the old tangent past a 20° crest and misses the far face. A shorter step stays
    // inside the window and walks over.
    let body = walker(&mut world, left.normal * RADIUS + left.rotation.rotate(-Vec3::X) * 2.5);
    world.insert(body, (CharacterLocomotion { speed: 15.0, ..CharacterLocomotion::default() },)).unwrap();
    let start_y = position(&world, body).y;
    let mut peak = start_y;
    let mut grounded = 0;
    for _ in 0..28 {
        wish(&mut world, body, Vec3::X);
        step(&mut world);
        let y = position(&world, body).y;
        peak = peak.max(y);
        if matches!(motion(&world, body).support, GroundSupport::Supported { .. }) {
            grounded += 1;
            assert_eq!(motion(&world, body).velocity.y, 0.0);
            assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump);
            assert_ne!(motion(&world, body).phase, LocomotionPhase::Fall);
        }
    }
    assert!(peak > start_y + 0.4, "did not climb, peak {peak} start {start_y}");
    assert!(position(&world, body).y < peak - 0.25, "did not descend from {peak}, at {:?}", position(&world, body));
    assert!(position(&world, body).x > 0.5, "{:?}", position(&world, body));
    assert_eq!(grounded, 28, "left the ground");
    assert!(supported_by(&world, body, left.entity) || supported_by(&world, body, right.entity));
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO);
}

/// Top face ends at the origin and runs downhill along local −X (`uphill_at_origin`) or local +X.
fn ramp_edge(world: &mut World, degrees: f32, uphill_at_origin: bool) -> Ramp {
    let rotation = Quat::from_rotation_z(degrees.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    let half = Vec3::new(12.0, 2.0, 16.0);
    let along = if uphill_at_origin { half.x } else { -half.x };
    let center = -rotation.rotate(Vec3::new(along, half.y, 0.0));
    let entity = world.spawn((Transform::from_position(center).with_rotation(rotation), Collider::aabb_box(half)));
    Ramp { entity, normal, rotation, half }
}

#[test]
fn walking_off_a_slope_ledge_falls_on_the_next_step() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(2.0, 2.0, 8.0));
    let body = walker(&mut world, on_ramp(&slope));
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    assert!(supported_by(&world, body, slope.entity));
    assert!(position(&world, body).y > on_ramp(&slope).y);
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    let departed = position(&world, body);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Walk);
    assert!(landings(&mut world).is_empty());
    step(&mut world);
    assert!(position(&world, body).y < departed.y, "the next step should fall");
    assert!(motion(&world, body).velocity.y < 0.0);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
}

#[test]
fn uphill_into_a_wall_does_not_climb_or_become_a_jump() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(30.0, 2.0, 12.0));
    let start = on_ramp(&slope);
    let body = walker(&mut world, start);
    world.spawn((Transform::from_xyz(start.x + 1.2, start.y + 2.0, 0.0), Collider::aabb_box(Vec3::new(0.4, 3.0, 6.0))));
    // The motor keeps the tangent's Y when a vertical wall blocks the X component. That leftover
    // separates the body from the slope; gravity brings it back. The body bounces. It does not
    // climb the wall, and the wall never becomes ground.
    let mut highest = start.y;
    let mut returned = 0;
    for _ in 0..20 {
        wish(&mut world, body, Vec3::X);
        step(&mut world);
        let at = position(&world, body);
        highest = highest.max(at.y);
        assert!(at.x < start.x + 0.6, "walked through the wall {at:?}");
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Jump);
        assert_eq!(motion(&world, body).velocity.x, 0.0);
        assert_eq!(motion(&world, body).velocity.z, 0.0);
        match motion(&world, body).support {
            GroundSupport::Supported { entity, normal } => {
                assert_eq!(entity, slope.entity);
                assert!(normal.y > 0.8, "wall became ground {normal:?}");
                assert_eq!(motion(&world, body).velocity.y, 0.0);
                returned += 1;
            }
            GroundSupport::Airborne => {}
            other => panic!("unexpected support {other:?}"),
        }
    }
    assert!(highest < start.y + 1.2, "wall contact climbed to {highest} from {}", start.y);
    assert!(returned >= 2, "the slope never caught the body again");
    assert!(landings(&mut world).iter().all(|event| *event == LocomotionEvent::Landed { entity: body }));
}

#[test]
fn a_steep_slope_keeps_gravity_and_the_motor_may_slide() {
    let mut world = World::new();
    let slope = ramp(&mut world, 60.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let body = walker(&mut world, on_ramp(&slope));
    wish(&mut world, body, Vec3::X);
    step(&mut world);
    let (requested, applied) = {
        let result = world.component::<MoveResult>(body).unwrap();
        (result.requested, result.applied)
    };
    assert!(requested.y < 0.0, "locomotion must not project a steep wish {requested:?}");
    near(requested.x, ALONG, 1e-3);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
    near(motion(&world, body).velocity.y, GRAVITY * DT, 1e-3);
    assert_eq!(motion(&world, body).velocity.x, 0.0);
    assert!(landings(&mut world).is_empty());
    // The sweep removes the into-face part of a horizontal wish. That leftover can point up the
    // steep face. It is not grounding and it is not a tangent walk.
    assert!(applied.y > 0.0, "expected the motor slide to carry Y, got {applied:?}");

    let mut rest = World::new();
    let slope = ramp(&mut rest, 60.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let body = walker(&mut rest, on_ramp(&slope));
    for _ in 0..5 {
        step(&mut rest);
        assert_eq!(motion(&rest, body).support, GroundSupport::Airborne);
        assert!(motion(&rest, body).velocity.y < 0.0);
        assert_ne!(motion(&rest, body).phase, LocomotionPhase::Walk);
        assert!(landings(&mut rest).is_empty());
    }
}

#[test]
fn a_jump_from_a_slope_uses_world_up_and_lands_once() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(80.0, 2.0, 30.0));
    let start = on_ramp(&slope);
    let body = walker(&mut world, start);
    step(&mut world);
    world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
    step(&mut world);
    let takeoff = position(&world, body);
    near(takeoff.y - start.y, JUMP * DT, 0.05);
    assert!((takeoff.x - start.x).abs() < 0.05, "takeoff drifted along X {takeoff:?} from {start:?}");
    assert!((takeoff.z - start.z).abs() < 0.05, "takeoff drifted along Z {takeoff:?}");
    let requested = world.component::<MoveResult>(body).unwrap().requested;
    assert!(requested.x.abs() < 1e-3 && requested.z.abs() < 1e-3, "jump inherited adhesion {requested:?}");
    near(requested.y, JUMP * DT, 1e-3);
    assert_eq!(motion(&world, body).velocity.y, JUMP);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Jump);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert!(landings(&mut world).is_empty());

    step(&mut world);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne, "the next probe re-grounded the jump");
    assert!(motion(&world, body).velocity.y < JUMP);

    let mut landed = false;
    for _ in 0..90 {
        step(&mut world);
        if supported_by(&world, body, slope.entity) && motion(&world, body).velocity.y == 0.0 {
            landed = true;
            break;
        }
    }
    assert!(landed, "never landed, at {:?} {:?}", position(&world, body), motion(&world, body));
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
    let gap = position(&world, body).dot(slope.normal) - RADIUS;
    assert!(gap.abs() < 0.05, "landing gap {gap}");
    assert!((position(&world, body).x - start.x).abs() < 1.0, "landing slid {:?}", position(&world, body));
    assert_ne!(motion(&world, body).phase, LocomotionPhase::Fall);
    step(&mut world);
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
    assert_eq!(motion(&world, body).velocity, Vec3::ZERO);
}

#[test]
fn contact_with_a_steep_slope_does_not_land() {
    let mut world = World::new();
    let slope = ramp(&mut world, 70.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let body = walker(&mut world, slope.normal * (RADIUS + 0.2));
    for _ in 0..8 {
        step(&mut world);
        assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
        assert_ne!(motion(&world, body).phase, LocomotionPhase::Idle);
    }
    assert!(landings(&mut world).is_empty());
    assert!(motion(&world, body).velocity.y < -1.0);
}

#[test]
fn a_ceiling_above_a_slope_cancels_the_rise_and_the_slope_lands_later() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(40.0, 2.0, 16.0));
    let start = on_ramp(&slope);
    let body = walker(&mut world, start);
    world.spawn((Transform::from_xyz(start.x, start.y + 2.0, 0.0), Collider::aabb_box(Vec3::new(3.0, 0.4, 3.0))));
    step(&mut world);
    world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
    step(&mut world);
    assert!(position(&world, body).y < start.y + JUMP * DT - 0.2, "ceiling did not stop the jump");
    assert_eq!(motion(&world, body).velocity.y, 0.0);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert_eq!(motion(&world, body).phase, LocomotionPhase::Fall);
    assert!(landings(&mut world).is_empty());
    let mut landed = false;
    for _ in 0..40 {
        step(&mut world);
        if supported_by(&world, body, slope.entity) {
            landed = true;
            break;
        }
    }
    assert!(landed);
    assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
    assert_eq!(motion(&world, body).velocity.y, 0.0);
}

#[test]
fn two_slopes_keep_the_nearer_then_the_lower_entity() {
    let mut nearer = World::new();
    let far = ramp(&mut nearer, 30.0, 0.0, Vec3::new(20.0, 2.0, 12.0));
    let close_rotation = far.rotation;
    let close_normal = far.normal;
    let close_center = -close_normal * 2.0 + close_normal * 0.02;
    let close = nearer.spawn((
        Transform::from_position(close_center).with_rotation(close_rotation),
        Collider::aabb_box(Vec3::new(20.0, 2.0, 12.0)),
    ));
    let body = walker(&mut nearer, close_normal * (RADIUS + 0.02));
    step(&mut nearer);
    assert!(supported_by(&nearer, body, close), "far {:?} close {:?}", far.entity, close);
    assert!(close > far.entity, "the fixture needs the nearer body to be the later entity");

    let mut tied = World::new();
    let low = ramp(&mut tied, 25.0, 0.4, Vec3::new(16.0, 2.0, 16.0));
    let high = tied.spawn((
        Transform::from_position(-low.normal * 2.0).with_rotation(low.rotation),
        Collider::aabb_box(Vec3::new(16.0, 2.0, 16.0)),
    ));
    assert!(low.entity < high);
    let body = walker(&mut tied, on_ramp(&low));
    step(&mut tied);
    assert!(supported_by(&tied, body, low.entity));
}

#[test]
fn a_sensor_and_a_mask_mismatch_do_not_support_a_slope() {
    let mut sensed = World::new();
    let slope = ramp(&mut sensed, 30.0, 0.0, Vec3::new(20.0, 2.0, 12.0));
    sensed.spawn((Transform::from_position(on_ramp(&slope)), Collider::aabb_box(Vec3::splat(2.0)).sensor()));
    let body = walker(&mut sensed, on_ramp(&slope));
    wish(&mut sensed, body, Vec3::X);
    step(&mut sensed);
    assert!(supported_by(&sensed, body, slope.entity));
    assert_eq!(motion(&sensed, body).phase, LocomotionPhase::Walk);
    near((position(&sensed, body) - on_ramp(&slope)).length(), ALONG, 0.05);

    let mut only_sensor = World::new();
    let rotation = Quat::from_rotation_z(30.0_f32.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    only_sensor.spawn((
        Transform::from_position(-normal * 2.0).with_rotation(rotation),
        Collider::aabb_box(Vec3::new(20.0, 2.0, 12.0)).sensor(),
    ));
    let body = walker(&mut only_sensor, normal * RADIUS);
    step(&mut only_sensor);
    assert_eq!(motion(&only_sensor, body).support, GroundSupport::Airborne);
    assert!(landings(&mut only_sensor).is_empty());

    let mut masked = World::new();
    let rotation = Quat::from_rotation_z(30.0_f32.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    masked.spawn((
        Transform::from_position(-normal * 2.0).with_rotation(rotation),
        Collider::aabb_box(Vec3::new(20.0, 2.0, 12.0)).with_layers(0b10).with_mask(0b10),
    ));
    let body = walker(&mut masked, normal * RADIUS);
    step(&mut masked);
    assert_eq!(motion(&masked, body).support, GroundSupport::Airborne);
    assert!(motion(&masked, body).velocity.y < 0.0);
    assert!(landings(&mut masked).is_empty());
}

#[test]
fn max_slope_and_enabled_can_change_at_runtime() {
    let mut world = World::new();
    let slope = ramp(&mut world, 30.0, 0.0, Vec3::new(30.0, 2.0, 12.0));
    let body = walker(&mut world, on_ramp(&slope));
    step(&mut world);
    assert!(supported_by(&world, body, slope.entity));
    world
        .insert(body, (CharacterLocomotion { max_slope: 15.0_f32.to_radians(), ..CharacterLocomotion::default() },))
        .unwrap();
    wish(&mut world, body, Vec3::X);
    let before = position(&world, body);
    step(&mut world);
    assert_eq!(motion(&world, body).support, GroundSupport::Airborne);
    assert!(motion(&world, body).velocity.y < 0.0);
    let requested = world.component::<MoveResult>(body).unwrap().requested;
    assert!(requested.y < 0.2, "a rejected slope was still walked {requested:?}");
    world.component_mut::<Transform>(body).unwrap().position = before;
    world
        .insert(
            body,
            (
                CharacterLocomotion { max_slope: 50.0_f32.to_radians(), ..CharacterLocomotion::default() },
                CharacterMotion::default(),
            ),
        )
        .unwrap();
    step(&mut world);
    assert!(supported_by(&world, body, slope.entity));

    wish(&mut world, body, Vec3::X);
    step(&mut world);
    let walked = position(&world, body);
    assert!((walked - before).length() > 1.0);
    world.insert(body, (CharacterLocomotion { enabled: false, ..CharacterLocomotion::default() },)).unwrap();
    world.insert(body, (MoveIntent::new(Vec3::Z), LocomotionIntent::new(Vec3::X))).unwrap();
    step(&mut world);
    let shifted = position(&world, body);
    near(shifted.z - walked.z, 1.0, 0.05);
    assert!((shifted.x - walked.x).abs() < 0.05, "disabled locomotion still walked the tangent {shifted:?}");
    assert!(world.component::<LocomotionIntent>(body).is_ok(), "a disabled body does not consume the wish");
}

#[test]
fn the_same_slope_traversal_replays_in_two_worlds() {
    fn trace() -> Vec<(Vec3, CharacterMotion)> {
        let mut world = World::new();
        let slope = ramp(&mut world, 30.0, 0.35, Vec3::new(70.0, 2.0, 30.0));
        let body = walker(&mut world, on_ramp(&slope));
        let mut out = Vec::new();
        step(&mut world);
        out.push((position(&world, body), motion(&world, body)));
        for _ in 0..4 {
            wish(&mut world, body, Vec3::new(1.0, 0.0, 0.5));
            step(&mut world);
            out.push((position(&world, body), motion(&world, body)));
        }
        world.insert(body, (LocomotionIntent::jumping(Vec3::ZERO),)).unwrap();
        step(&mut world);
        out.push((position(&world, body), motion(&world, body)));
        for _ in 0..90 {
            step(&mut world);
            let done =
                supported_by(&world, body, slope.entity) && motion(&world, body).velocity.y == 0.0 && out.len() > 8;
            out.push((position(&world, body), motion(&world, body)));
            if done {
                break;
            }
        }
        for _ in 0..3 {
            wish(&mut world, body, -Vec3::X);
            step(&mut world);
            out.push((position(&world, body), motion(&world, body)));
        }
        assert!(out.len() > 8, "the replay never landed");
        assert_eq!(landings(&mut world), vec![LocomotionEvent::Landed { entity: body }]);
        let last = out.last().unwrap();
        assert_eq!(last.1.phase, LocomotionPhase::Walk);
        assert_eq!(last.1.velocity.x, 0.0);
        assert_eq!(last.1.velocity.z, 0.0);
        assert!(last.1.velocity.y == 0.0);
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
