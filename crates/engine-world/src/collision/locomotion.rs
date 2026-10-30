//! Ground support, gravity, jumping, and walkable-slope traversal for a gameplay body.
//!
//! [`CharacterMotor`] remains the displacement solver. An enabled [`CharacterLocomotion`] writes
//! that motor's [`MoveIntent`] for the iteration. The support query reads the index
//! [`sync_collision`](super::sync_collision) just rebuilt. It does not build a second index.
//! Ceiling contact is read from that iteration's fresh [`MoveResult`], not from an older one.
//!
//! The probe uses the closest-feature gap. A candidate counts only when that gap is inside the
//! reach window: not a deep penetration, and not farther than `skin + stick`. The nearest valid
//! gap wins. Gaps within `1e-5` keep the lower [`Entity`]. The wall normal is the feature normal,
//! from the surface toward the body. It is walkable only when it faces up within `max_slope`.
//! `normal.y` must be strictly positive, so an exactly vertical wall is not ground even when
//! `max_slope` is π/2 and `cos(π/2)` is a tiny negative value. Sensors and a one-way query mask
//! do not count. The motor's mutual layer test does.
//!
//! Grounded walking projects the XZ wish onto the plane perpendicular to that normal and travels
//! `speed` along the surface. The tangent keeps its Y component. Adhesion is `stick` along the
//! negated normal, in the same intent. Flat ground, whose normal is world +Y, is the previous
//! horizontal step plus a downward stick. Airborne motion and takeoff stay on world axes: the
//! wish is horizontal, and the jump is world +Y. Neither inherits the slope tangent or the stick.
//!
//! That same intent carries a private slide limit into the one motor sweep. Grounded motion may
//! keep only the direction that lies in both the support plane and a blocking unwalkable wall.
//! A walkable contact, including the support itself, still uses the motor's usual plane slide, so
//! a slope transition is not treated as a wall. Airborne motion may slide a wall's level contour
//! and may slide the vertical velocity along the wall. The horizontal wish does not add climb on
//! an unwalkable face. A motor move with no locomotion limit is unchanged.
//!
//! `Unknown` becomes `Supported` or `Airborne` on the first positive fixed step. Spawning already
//! inside the support window is that classification, not a landing. Gravity is world −Y. It is
//! integrated only when this iteration's pre-move probe is already `Airborne` (or the takeoff lock
//! is still set). The step that walks off a floor does not fall. The next one does. Removing the
//! floor makes the next probe `Airborne`, so gravity starts on that step. A jump reads the same
//! pre-move probe. [`LocomotionEvent::Landed`] is sent when that probe was `Airborne`, or the
//! stored support was already `Airborne`, and the post-move probe is `Supported`, and the move was
//! not rejected. The takeoff lock keeps the stored support `Airborne` while the body is still
//! rising inside the probe window, so that iteration does not land.

use engine_math::Vec3;
use hecs::Entity;

use crate::World;

use super::feature::feature_hit;
use super::index::{entries, CollisionWorld};
use super::types::{point_finite, CharacterMotor, MoveIntent, MoveResult, MoveStatus};

/// Gaps this close are the same support. This matches the motor's candidate tie.
const SUPPORT_TIE: f32 = 1e-5;
/// Extra penetration, beyond skin, that still counts as standing on the surface.
const PENETRATION_SLOP: f32 = 1e-4;
/// Committed traversal shorter than this is not a walk.
const WALK_EPSILON: f32 = 1e-3;
/// A contact normal this far below the horizon is a ceiling, not a wall or a floor.
const CEILING_Y: f32 = -0.5;
/// Upward motion shorter than this did not leave the ground.
const LEFT_GROUND: f32 = 1e-4;

/// Persistent locomotion settings for one gameplay body.
///
/// `speed` is world units per second along a walkable surface. In the air it is horizontal XZ
/// speed. `wish` is a direction, and this speed is what turns it into a displacement.
/// `max_slope` is the largest angle, in radians, between world +Y and a support normal. A normal
/// with `y <= 0` is never walkable, including an exactly vertical wall at `max_slope` of π/2.
/// `stick` is adhesion toward the supporting surface, along the negated support normal, in world
/// units. On flat ground that direction is world −Y. It is applied while the pre-move probe says
/// the body is supported and the body is not jumping or still rising in the takeoff lock.
///
/// `gravity` is acceleration along world Y, in units per second squared. Up is +Y, so the default
/// is negative. A positive or non-finite value contributes nothing. `jump_speed` is the upward
/// speed assigned at takeoff, in units per second. `terminal_fall` is the downward speed limit as
/// a positive magnitude. A non-positive or non-finite limit does not clamp.
///
/// A non-finite or negative `speed` or `stick` contributes nothing. A `max_slope` outside
/// `0..=π/2`, or a non-finite one, accepts no support. Those cases do not write a non-finite move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterLocomotion {
    pub enabled: bool,
    pub speed: f32,
    pub max_slope: f32,
    pub stick: f32,
    pub gravity: f32,
    pub jump_speed: f32,
    pub terminal_fall: f32,
}

impl Default for CharacterLocomotion {
    fn default() -> Self {
        Self {
            enabled: true,
            speed: 90.0,
            max_slope: std::f32::consts::FRAC_PI_4,
            stick: 0.08,
            gravity: -400.0,
            jump_speed: 200.0,
            terminal_fall: 800.0,
        }
    }
}

impl CharacterLocomotion {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Who, if anyone, is holding the body up after the latest support probe.
///
/// `Unknown` means this body has not been probed by a positive fixed step yet.
/// `Supported` names one solid and the feature normal that points from that solid toward the body.
/// `Airborne` means the body has a usable solid collider and the probe found no walkable support.
/// `Unavailable` means this body has no solid indexed collider to stand with: it is missing,
/// disabled, invalid, or itself a sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GroundSupport {
    Unknown,
    Supported { entity: Entity, normal: Vec3 },
    Airborne,
    Unavailable,
}

/// How the body is moving. `Jump` is airborne with upward velocity. `Fall` is airborne after
/// gravity has been integrated, or after a ceiling stops the rise. `Walk` is committed traversal
/// along the ground longer than `1e-3` that is not jumping or falling, including the step that
/// leaves a ledge before gravity starts. On a walkable slope that length is the part of the
/// committed move lying in the support plane, so adhesion and a recovery along the normal stay
/// `Idle`, walking uphill is not `Jump`, and walking downhill is not `Fall`. Airborne motion with
/// no vertical velocity still uses the horizontal XZ length. Holding a wish into a wall and a
/// deep overlap that is not support stay `Idle`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LocomotionPhase {
    #[default]
    Idle,
    Walk,
    Jump,
    Fall,
}

/// Runtime support, phase, and vertical momentum.
///
/// `velocity.y` is the jump and fall speed in world units per second. Positive is up. It is the
/// value gravity and takeoff write. It is not `MoveResult.applied / dt`.
///
/// `velocity.x` and `velocity.z` are reserved for a later acceleration checkpoint. This checkpoint
/// leaves them at zero and does not read them. Grounded motion is the wish projected onto the
/// support plane, times `speed`, times the fixed step. Its Y component is slope travel, not
/// `velocity.y`. Airborne motion is still the horizontal wish times `speed` times the fixed step,
/// plus `velocity.y`. Neither is `MoveResult.applied / dt`. The support stick is not stored here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterMotion {
    pub support: GroundSupport,
    pub phase: LocomotionPhase,
    pub velocity: Vec3,
}

impl Default for CharacterMotion {
    fn default() -> Self {
        Self { support: GroundSupport::Unknown, phase: LocomotionPhase::Idle, velocity: Vec3::ZERO }
    }
}

impl CharacterMotion {
    pub fn new() -> Self {
        Self::default()
    }
}

/// World XZ direction and a jump edge for one fixed iteration.
///
/// `wish` is not a displacement and not a velocity. Its Y is ignored. A non-finite X or Z becomes
/// no direction. A horizontal length above 1 is normalized so a diagonal does not go faster than
/// `speed`. A shorter direction is kept.
///
/// `jump` is consumed this iteration. It is not a held boost, and it is not remembered while the
/// body is airborne. A fixed system copies the input latch into this flag. The latch, not this
/// component, is what survives a frame with no fixed iteration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocomotionIntent {
    pub wish: Vec3,
    pub jump: bool,
}

impl LocomotionIntent {
    pub fn new(wish: Vec3) -> Self {
        Self { wish, jump: false }
    }

    /// A wish that also requests takeoff on this iteration.
    pub fn jumping(wish: Vec3) -> Self {
        Self { wish, jump: true }
    }
}

/// A locomotion transition that gameplay and animation can read after the fixed loop.
///
/// [`World::send`](crate::World::send) appends it. `update` and `animate` see it after
/// [`World::promote_events`](crate::World::promote_events), including when a later fixed iteration
/// of the same step keeps moving. A fixed system does not see it during that loop. Spawning onto
/// a floor does not send this. Neither does staying on the same floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocomotionEvent {
    Landed { entity: Entity },
}

/// Bookkeeping that must survive from prepare to settle, and a jump lock that must survive to the
/// next fixed step.
///
/// `launched` is the body that jumped and has not yet moved clear of the floor probe, hit a
/// ceiling, or stopped rising. While it is set, a pre-move probe that still sees the takeoff
/// floor does not stick and does not cancel the upward speed. The lock is dropped once the body
/// is geometrically airborne, `velocity.y` is no longer upward, a ceiling blocks the rise, or the
/// move is rejected.
#[derive(Clone, Debug, Default)]
struct LocomotionNotes {
    conflicts: Vec<Entity>,
    missing_motion: Vec<Entity>,
    submitted: Vec<Entity>,
    launched: Vec<Entity>,
    /// Pre-move probe was `Airborne` and the body was not buried. Used so a first-step fall can land.
    was_air: Vec<Entity>,
    /// Gravity was integrated this iteration, so an airborne body with zero vertical speed is falling.
    fell: Vec<Entity>,
    before: Vec<(Entity, Vec3)>,
}

/// How one locomotion sweep may redirect the unconsumed remainder after a blocking hit.
///
/// This is not a motor setting and it is not saved. [`prepare_locomotion`] attaches it to the
/// move it submits. [`super::motor::resolve_motors`] consumes it. A [`MoveIntent`] written by
/// anything else has no limit, so the sweep's public slide is unchanged.
#[derive(Clone, Copy, Debug)]
pub(super) enum SlideLimit {
    /// Stay in `normal` and do not climb out of an unwalkable blocker.
    Ground { normal: Vec3, min_cosine: f32 },
    /// Horizontal input slides on the wall's level contour. Vertical velocity still uses the wall.
    Air,
}

/// Slide limits for the moves [`prepare_locomotion`] just submitted, in entity order.
#[derive(Clone, Debug, Default)]
pub(super) struct SlideLimits {
    pub entries: Vec<(Entity, SlideLimit)>,
}

/// `Some` only for a finite duration strictly above zero. Zero, negative, and non-finite values
/// are `None`: collision still runs, and locomotion does not.
pub(super) fn usable_step(step_secs: Option<f32>) -> Option<f32> {
    step_secs.filter(|step| step.is_finite() && *step > 0.0)
}

/// Writes locomotion displacements and drops a competing [`MoveIntent`].
///
/// The index is the one just rebuilt, before motors move anyone. Bodies run in ascending
/// [`Entity`] order. Vertical speed is integrated here, before the sweep, and corrected in
/// [`settle_locomotion`] from the committed result.
pub(super) fn prepare_locomotion(world: &mut World, step_secs: f32) {
    let mut bodies: Vec<(Entity, CharacterLocomotion, CharacterMotor)> = world
        .query::<(&CharacterLocomotion, &CharacterMotor)>()
        .iter()
        .map(|(entity, (locomotion, motor))| (entity, *locomotion, *motor))
        .collect();
    bodies.sort_by_key(|(entity, _, _)| *entity);

    let mut notes = take_notes(world);
    let launched = notes.launched.clone();
    let mut conflicts_now = Vec::new();
    let mut missing_now = Vec::new();
    let mut slide_limits = Vec::new();
    notes.submitted.clear();
    notes.was_air.clear();
    notes.fell.clear();
    notes.before.clear();

    for (entity, locomotion, motor) in bodies {
        if !locomotion.enabled {
            continue;
        }
        if world.component::<MoveIntent>(entity).is_ok() {
            conflicts_now.push(entity);
            if !notes.conflicts.contains(&entity) {
                log::warn!(
                    "locomotion on {entity:?} discarded a MoveIntent; enabled CharacterLocomotion owns this displacement"
                );
            }
            let _ = world.remove::<MoveIntent>(entity);
        }
        let Ok(mut motion) = world.component::<CharacterMotion>(entity).map(|motion| *motion) else {
            missing_now.push(entity);
            if !notes.missing_motion.contains(&entity) {
                log::warn!(
                    "locomotion on {entity:?} has no CharacterMotion; its MoveIntent was discarded and it did not move"
                );
            }
            continue;
        };

        let (pre, buried) = {
            let collision = world.resource::<CollisionWorld>();
            probe(&collision, entity, &motor, &locomotion)
        };
        let (wish, jump) = world
            .component::<LocomotionIntent>(entity)
            .map(|intent| (horizontal_wish(intent.wish), intent.jump))
            .unwrap_or((Vec3::ZERO, false));
        let _ = world.remove::<LocomotionIntent>(entity);

        let rising = launched.contains(&entity);
        notes.before.push((entity, motion.velocity));
        // A body buried deeper than the support window is not falling through air. Submitting the
        // gravity displacement would let the motor's recovery push invent a floor. A center-deep
        // overlap can also have no walkable normal, because the nearest faces cancel.
        let vertical =
            if buried { Vertical::Skip } else { integrate(&locomotion, &mut motion, &pre, jump, step_secs, rising) };
        if matches!(pre, GroundSupport::Airborne) && !buried {
            notes.was_air.push(entity);
        }
        match vertical {
            Vertical::Jump => {
                if !notes.launched.contains(&entity) {
                    notes.launched.push(entity);
                }
            }
            Vertical::Air => notes.fell.push(entity),
            Vertical::Ground | Vertical::Skip => {}
        }
        if world.insert(entity, (motion,)).is_err() {
            continue;
        }
        let rising_move = matches!(vertical, Vertical::Jump | Vertical::Air) || rising;
        let Some(delta) =
            (!buried).then(|| displacement(&locomotion, &motion, wish, step_secs, &pre, rising_move)).flatten()
        else {
            continue;
        };
        if world.insert(entity, (MoveIntent::new(delta),)).is_err() {
            continue;
        }
        slide_limits.push((entity, slide_limit(&pre, rising_move, &locomotion)));
        notes.submitted.push(entity);
    }

    notes.conflicts = conflicts_now;
    notes.missing_motion = missing_now;
    world.insert_resource(notes);
    world.insert_resource(SlideLimits { entries: slide_limits });
}

/// Grounded surface motion keeps its support plane. A jump, a fall, and any other airborne
/// submission keep the level-contour limit so a horizontal wish cannot be turned into climb.
fn slide_limit(pre: &GroundSupport, rising_move: bool, locomotion: &CharacterLocomotion) -> SlideLimit {
    if let (GroundSupport::Supported { normal, .. }, false) = (pre, rising_move) {
        if let Some(min_cosine) = slope_cosine(locomotion.max_slope) {
            return SlideLimit::Ground { normal: *normal, min_cosine };
        }
    }
    SlideLimit::Air
}

/// Stores the support at the pose the motor committed, settles vertical speed, and sends
/// [`LocomotionEvent::Landed`] for a real airborne-to-supported transition.
pub(super) fn settle_locomotion(world: &mut World) {
    let mut bodies: Vec<(Entity, CharacterLocomotion, CharacterMotor)> = world
        .query::<(&CharacterLocomotion, &CharacterMotor, &CharacterMotion)>()
        .iter()
        .filter(|(_, (locomotion, _, _))| locomotion.enabled)
        .map(|(entity, (locomotion, motor, _))| (entity, *locomotion, *motor))
        .collect();
    bodies.sort_by_key(|(entity, _, _)| *entity);
    let mut notes = take_notes(world);
    let submitted = notes.submitted.clone();
    let before = notes.before.clone();
    let was_air = notes.was_air.clone();
    let fell = notes.fell.clone();
    let mut launched = notes.launched.clone();

    for (entity, locomotion, motor) in &bodies {
        let Ok(mut motion) = world.component::<CharacterMotion>(*entity).map(|motion| *motion) else {
            continue;
        };
        let previous = motion.support;
        let prior =
            before.iter().find(|(id, _)| id == entity).map(|(_, velocity)| *velocity).unwrap_or(motion.velocity);
        let post = {
            let collision = world.resource::<CollisionWorld>();
            probe(&collision, *entity, motor, locomotion).0
        };
        let result = if submitted.contains(entity) {
            world.component::<MoveResult>(*entity).ok().map(|result| (*result).clone())
        } else {
            None
        };
        let rejected = result.as_ref().is_some_and(|result| result.status == MoveStatus::Rejected);
        if rejected {
            motion.velocity = prior;
            if matches!(post, GroundSupport::Supported { .. }) {
                motion.velocity.y = 0.0;
            }
            motion.support = post;
            motion.phase = LocomotionPhase::Idle;
            launched.retain(|id| id != entity);
            let _ = world.insert(*entity, (motion,));
            continue;
        }

        let ceiling = result.as_ref().is_some_and(ceiling_blocked);
        if ceiling {
            motion.velocity.y = 0.0;
        }
        let moved_up = result.as_ref().is_some_and(|result| result.applied.y > LEFT_GROUND);
        let suppressing = launched.contains(entity)
            && motion.velocity.y > 0.0
            && !ceiling
            && moved_up
            && matches!(post, GroundSupport::Supported { .. });
        let support = if suppressing { GroundSupport::Airborne } else { post };
        if matches!(support, GroundSupport::Supported { .. }) {
            motion.velocity.y = 0.0;
        }
        let landed = matches!(support, GroundSupport::Supported { .. })
            && (matches!(previous, GroundSupport::Airborne) || was_air.contains(entity));
        if landed {
            world.send(LocomotionEvent::Landed { entity: *entity });
        }
        let falling = fell.contains(entity) || (ceiling && matches!(support, GroundSupport::Airborne));
        motion.phase = phase_after_move(&support, motion.velocity, result.as_ref(), falling);
        motion.support = support;
        if !suppressing {
            launched.retain(|id| id != entity);
        }
        let _ = world.insert(*entity, (motion,));
    }

    let live: Vec<Entity> = bodies.iter().map(|(entity, _, _)| *entity).collect();
    launched.retain(|entity| live.contains(entity));
    notes.launched = launched;
    notes.submitted.clear();
    notes.was_air.clear();
    notes.fell.clear();
    notes.before.clear();
    world.insert_resource(notes);
}

/// Drops every [`LocomotionIntent`]. `Game::step` calls this after the fixed loop, including a
/// loop of zero iterations.
pub fn clear_locomotion_intents(world: &mut World) {
    let pending: Vec<Entity> = world.query::<&LocomotionIntent>().iter().map(|(entity, _)| entity).collect();
    for entity in pending {
        let _ = world.remove::<LocomotionIntent>(entity);
    }
}

fn take_notes(world: &mut World) -> LocomotionNotes {
    if !world.has_resource::<LocomotionNotes>() {
        world.insert_resource(LocomotionNotes::default());
    }
    let notes = world.resource::<LocomotionNotes>().clone();
    notes
}

/// What prepare did to vertical speed. `Jump` skips gravity. `Air` is semi-implicit Euler.
/// `Ground` cancels vertical speed. `Skip` leaves it unchanged.
enum Vertical {
    Jump,
    Air,
    Ground,
    Skip,
}

/// Semi-implicit Euler. Gravity is added to `velocity.y` before that speed becomes a displacement.
///
/// A supported body that is not in the takeoff lock does not accumulate gravity. Takeoff replaces
/// `velocity.y` and skips gravity for this iteration. A rejected move restores the velocity from
/// before this call, so a permanent rejection does not accumulate gravity.
fn integrate(
    locomotion: &CharacterLocomotion,
    motion: &mut CharacterMotion,
    pre: &GroundSupport,
    jump: bool,
    step_secs: f32,
    launched: bool,
) -> Vertical {
    if matches!(pre, GroundSupport::Unavailable) {
        return Vertical::Skip;
    }
    let speed = finite_positive(locomotion.jump_speed);
    let jumped = jump && !launched && matches!(pre, GroundSupport::Supported { .. }) && speed > 0.0;
    if jumped {
        motion.velocity.y = speed;
        return Vertical::Jump;
    }
    if matches!(pre, GroundSupport::Supported { .. }) && !launched {
        motion.velocity.y = 0.0;
        return Vertical::Ground;
    }
    motion.velocity.y += gravity_accel(locomotion.gravity) * step_secs;
    if let Some(limit) = finite_positive_opt(locomotion.terminal_fall) {
        motion.velocity.y = motion.velocity.y.max(-limit);
    }
    if !motion.velocity.y.is_finite() {
        motion.velocity.y = 0.0;
    }
    Vertical::Air
}

fn displacement(
    locomotion: &CharacterLocomotion,
    motion: &CharacterMotion,
    wish: Vec3,
    step_secs: f32,
    pre: &GroundSupport,
    airborne_motion: bool,
) -> Option<Vec3> {
    if matches!(pre, GroundSupport::Unavailable) {
        return None;
    }
    let speed = finite_nonneg(locomotion.speed).unwrap_or(0.0);
    let delta = if let GroundSupport::Supported { normal, .. } = pre {
        if airborne_motion {
            airborne_delta(wish, motion.velocity.y, speed, step_secs)
        } else {
            surface_travel(wish, *normal, speed, step_secs) + adhesion(*normal, locomotion.stick)
        }
    } else {
        airborne_delta(wish, motion.velocity.y, speed, step_secs)
    };
    if !point_finite(delta) || delta.length_squared() == 0.0 {
        None
    } else {
        Some(delta)
    }
}

fn phase_after_move(
    support: &GroundSupport,
    velocity: Vec3,
    result: Option<&MoveResult>,
    fell: bool,
) -> LocomotionPhase {
    if matches!(support, GroundSupport::Airborne) {
        if velocity.y > 0.0 {
            return LocomotionPhase::Jump;
        }
        if velocity.y < 0.0 || fell {
            return LocomotionPhase::Fall;
        }
    }
    let Some(result) = result else {
        return LocomotionPhase::Idle;
    };
    if result.status == MoveStatus::Rejected {
        return LocomotionPhase::Idle;
    }
    if ground_traversal(support, result.applied) > WALK_EPSILON {
        LocomotionPhase::Walk
    } else {
        LocomotionPhase::Idle
    }
}

/// Along-surface distance of a committed move. Adhesion and a normal recovery contribute nothing.
///
/// Airborne motion has no walkable plane, so the measure stays the horizontal XZ length. That
/// includes the step that leaves a ledge.
fn ground_traversal(support: &GroundSupport, applied: Vec3) -> f32 {
    match support {
        GroundSupport::Supported { normal, .. } => match normal.try_normalize() {
            Some(normal) if normal.y > 0.0 => (applied - normal * applied.dot(normal)).length(),
            _ => horizontal_length(applied),
        },
        _ => horizontal_length(applied),
    }
}

fn horizontal_length(applied: Vec3) -> f32 {
    Vec3::new(applied.x, 0.0, applied.z).length()
}

/// World-axis airborne step. The wish stays horizontal. Vertical motion is `velocity.y`, including
/// a jump, and there is no slope tangent and no adhesion.
fn airborne_delta(wish: Vec3, vertical_speed: f32, speed: f32, step_secs: f32) -> Vec3 {
    Vec3::new(wish.x * speed * step_secs, vertical_speed * step_secs, wish.z * speed * step_secs)
}

/// Projects `wish` onto the plane of `normal` and travels `|wish| * speed * step` along it.
///
/// `wish` is already horizontal and no longer than 1. A flat normal `(0, 1, 0)` therefore returns
/// the wish scaled by speed and time. The tangent is normalized only when its length is finite and
/// above [`engine_math::EPSILON`]. A zero, non-finite, or downward normal contributes nothing.
fn surface_travel(wish: Vec3, normal: Vec3, speed: f32, step_secs: f32) -> Vec3 {
    if !point_finite(wish) || !point_finite(normal) || !speed.is_finite() || !step_secs.is_finite() {
        return Vec3::ZERO;
    }
    let Some(normal) = normal.try_normalize().filter(|normal| normal.y > 0.0) else {
        return Vec3::ZERO;
    };
    let scale = wish.length();
    if !scale.is_finite() || scale <= engine_math::EPSILON {
        return Vec3::ZERO;
    }
    let Some(direction) = (wish - normal * wish.dot(normal)).try_normalize() else {
        return Vec3::ZERO;
    };
    direction * (scale * speed * step_secs)
}

/// `stick` toward the surface. Flat ground is world −Y. A non-finite, negative, or non-upward
/// normal contributes nothing, so adhesion cannot push the body sideways off a bad contact.
fn adhesion(normal: Vec3, stick: f32) -> Vec3 {
    let Some(stick) = finite_nonneg(stick) else {
        return Vec3::ZERO;
    };
    let Some(normal) = normal.try_normalize().filter(|normal| normal.y > 0.0) else {
        return Vec3::ZERO;
    };
    -normal * stick
}

fn ceiling_blocked(result: &MoveResult) -> bool {
    if result.status == MoveStatus::Rejected || result.requested.y <= LEFT_GROUND {
        return false;
    }
    if result.applied.y >= result.requested.y - LEFT_GROUND {
        return false;
    }
    let ceiling = |normal: Vec3| normal.y < CEILING_Y;
    result.normals.iter().copied().any(ceiling) || result.recovery_normal.is_some_and(ceiling)
}

fn gravity_accel(value: f32) -> f32 {
    if value.is_finite() && value < 0.0 {
        value
    } else {
        0.0
    }
}

fn finite_positive(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn finite_positive_opt(value: f32) -> Option<f32> {
    (value.is_finite() && value > 0.0).then_some(value)
}

/// Support at the body's current indexed pose, and whether a solid is penetrated deeper than the
/// support window with no walkable candidate chosen.
///
/// `Unavailable` when the body is absent from the index or its collider is a sensor. Otherwise
/// the nearest walkable solid inside the reach window, or `Airborne`. The buried flag is set only
/// when the result is `Airborne`, so a real floor still wins over a deep overlap elsewhere.
fn probe(
    collision: &CollisionWorld,
    entity: Entity,
    motor: &CharacterMotor,
    locomotion: &CharacterLocomotion,
) -> (GroundSupport, bool) {
    let list = entries(collision);
    let Some(mover_index) = list.iter().position(|entry| entry.entity == entity) else {
        return (GroundSupport::Unavailable, false);
    };
    let mover = &list[mover_index];
    if mover.sensor {
        return (GroundSupport::Unavailable, false);
    }
    let Some(min_cosine) = slope_cosine(locomotion.max_slope) else {
        return (GroundSupport::Airborne, false);
    };
    let skin = finite_nonneg(motor.skin).unwrap_or(0.0);
    let stick = finite_nonneg(locomotion.stick).unwrap_or(0.0);
    let reach = skin + stick;
    let min_gap = -(skin + PENETRATION_SLOP);

    let mut best: Option<(f32, Entity, Vec3)> = None;
    let mut buried = false;
    for entry in list {
        if entry.entity == entity || entry.sensor || !mutual(mover.layers, mover.mask, entry.layers, entry.mask) {
            continue;
        }
        let Some(feature) = feature_hit(&mover.prepared, mover.prepared.center, &entry.prepared) else {
            continue;
        };
        if !feature.gap.is_finite() {
            continue;
        }
        // A largely negative gap is inside the solid. The nearest faces can cancel, so there may
        // be no wall normal. That penetration is still not a floor, and it is not open air.
        if feature.gap < min_gap {
            buried = true;
            continue;
        }
        let Some(normal) = feature.wall else {
            continue;
        };
        if !walkable_normal(normal, min_cosine) {
            continue;
        }
        if feature.gap > reach {
            continue;
        }
        let replace = match best {
            None => true,
            Some((gap, other, _)) => {
                feature.gap < gap - SUPPORT_TIE || ((feature.gap - gap).abs() <= SUPPORT_TIE && entry.entity < other)
            }
        };
        if replace {
            best = Some((feature.gap, entry.entity, normal));
        }
    }

    let support = match best {
        Some((_, entity, normal)) => GroundSupport::Supported { entity, normal },
        None => GroundSupport::Airborne,
    };
    (support, buried && matches!(support, GroundSupport::Airborne))
}

/// Minimum upward component of a unit support normal.
///
/// `cos(π/2)` is slightly negative in `f32`. Clamping to zero keeps that value from admitting a
/// downward normal. [`walkable_normal`] still rejects `y == 0`, so a vertical wall stays unwalkable
/// when the configured maximum is exactly 90°.
fn slope_cosine(max_slope: f32) -> Option<f32> {
    if max_slope.is_finite() && (0.0..=std::f32::consts::FRAC_PI_2).contains(&max_slope) {
        Some(max_slope.cos().max(0.0))
    } else {
        None
    }
}

/// Cosine slack so a slope built at exactly `max_slope` still counts.
///
/// A 45° rotation and `cos(π/4)` can disagree by a few ulps. `1e-4` is about 0.008° near 45° and
/// does not pull a 45.05° face under a 45° limit. It also cannot admit `y <= 0`.
const SLOPE_COSINE_SLOP: f32 = 1e-4;

/// A unit normal is walkable when it faces strictly upward and its Y meets `min_cosine`.
///
/// A slope at the configured angle is walkable. `y == 0` is not, even when `min_cosine` is 0.
pub(super) fn walkable_normal(normal: Vec3, min_cosine: f32) -> bool {
    normal.y.is_finite() && normal.y > 0.0 && normal.y + SLOPE_COSINE_SLOP >= min_cosine
}

fn finite_nonneg(value: f32) -> Option<f32> {
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn horizontal_wish(wish: Vec3) -> Vec3 {
    if !wish.x.is_finite() || !wish.z.is_finite() {
        return Vec3::ZERO;
    }
    let horizontal = Vec3::new(wish.x, 0.0, wish.z);
    let length = horizontal.length();
    if length > 1.0 {
        horizontal / length
    } else {
        horizontal
    }
}

/// Both layer tests, matching the motor. A free [`CollisionQuery`](super::CollisionQuery) does
/// not do this.
fn mutual(layers: u32, mask: u32, other_layers: u32, other_mask: u32) -> bool {
    layers & other_mask != 0 && other_layers & mask != 0
}

#[cfg(test)]
mod tests {
    use engine_math::Quat;

    use super::*;

    const SPEED: f32 = 90.0;
    const STEP: f32 = 1.0 / 60.0;
    /// The 30° example from the checkpoint: a plane whose uphill direction is +X.
    const SLOPE_30: Vec3 = Vec3::new(-0.5, 0.8660254, 0.0);

    fn close(actual: Vec3, expected: Vec3) {
        let gap = (actual - expected).length();
        assert!(gap < 1e-3, "{actual:?} vs {expected:?} (gap {gap})");
    }

    #[test]
    fn flat_ground_keeps_the_horizontal_step() {
        let step = surface_travel(Vec3::X, Vec3::Y, SPEED, STEP);
        close(step, Vec3::new(1.5, 0.0, 0.0));
        close(adhesion(Vec3::Y, 0.08), Vec3::new(0.0, -0.08, 0.0));
    }

    #[test]
    fn a_30_degree_tangent_matches_the_surface_step() {
        let uphill = surface_travel(Vec3::X, SLOPE_30, SPEED, STEP);
        let downhill = surface_travel(-Vec3::X, SLOPE_30, SPEED, STEP);
        close(uphill, Vec3::new(1.299, 0.750, 0.0));
        close(downhill, Vec3::new(-1.299, -0.750, 0.0));
        let along = uphill.length();
        assert!((along - 1.5).abs() < 1e-3, "along-surface speed {along}");
        assert!(uphill.y > 0.0 && downhill.y < 0.0);
    }

    #[test]
    fn a_short_wish_and_a_diagonal_keep_their_scale() {
        let half = surface_travel(Vec3::X * 0.5, Vec3::Y, SPEED, STEP);
        close(half, Vec3::new(0.75, 0.0, 0.0));
        let diagonal = horizontal_wish(Vec3::new(1.0, 9.0, 1.0));
        let sloped = surface_travel(diagonal, SLOPE_30, SPEED, STEP);
        assert!((sloped.length() - 1.5).abs() < 1e-3, "{sloped:?}");
        let flat = surface_travel(diagonal, Vec3::Y, SPEED, STEP);
        assert!((flat.length() - 1.5).abs() < 1e-3, "{flat:?}");
    }

    #[test]
    fn a_slope_yawed_about_y_keeps_the_same_surface_step() {
        let yaw = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let normal = yaw.rotate(SLOPE_30);
        let uphill = yaw.rotate(surface_travel(Vec3::X, SLOPE_30, SPEED, STEP));
        // +X uphill rotates to −Z uphill.
        close(surface_travel(Vec3::new(0.0, 0.0, -1.0), normal, SPEED, STEP), uphill);
        // Contour of the unrotated slope is world +Z and stays horizontal.
        close(surface_travel(Vec3::Z, SLOPE_30, SPEED, STEP), Vec3::new(0.0, 0.0, 1.5));
    }

    #[test]
    fn degenerate_wishes_and_normals_contribute_nothing() {
        assert_eq!(surface_travel(Vec3::ZERO, SLOPE_30, SPEED, STEP), Vec3::ZERO);
        assert_eq!(surface_travel(Vec3::X * 1e-8, SLOPE_30, SPEED, STEP), Vec3::ZERO);
        assert_eq!(surface_travel(Vec3::new(f32::NAN, 0.0, 0.0), SLOPE_30, SPEED, STEP), Vec3::ZERO);
        assert_eq!(surface_travel(Vec3::X, Vec3::ZERO, SPEED, STEP), Vec3::ZERO);
        assert_eq!(surface_travel(Vec3::X, Vec3::new(1.0, 0.0, 0.0), SPEED, STEP), Vec3::ZERO);
        assert_eq!(surface_travel(Vec3::X, Vec3::new(f32::INFINITY, 1.0, 0.0), SPEED, STEP), Vec3::ZERO);
        assert_eq!(adhesion(Vec3::ZERO, 0.08), Vec3::ZERO);
        assert_eq!(adhesion(Vec3::X, 0.08), Vec3::ZERO);
        assert_eq!(adhesion(SLOPE_30, f32::NAN), Vec3::ZERO);
        let pull = adhesion(SLOPE_30, 0.08);
        close(pull, -SLOPE_30.normalized() * 0.08);
        assert!(pull.x > 0.0 && pull.z.abs() < 1e-6, "adhesion stays in the normal plane {pull:?}");
    }

    #[test]
    fn a_vertical_wall_is_not_walkable_at_ninety_degrees() {
        let min_cosine = slope_cosine(std::f32::consts::FRAC_PI_2).unwrap();
        assert_eq!(min_cosine, 0.0, "cos(π/2) must not stay negative, got {min_cosine}");
        assert!(std::f32::consts::FRAC_PI_2.cos() <= 0.0);
        assert!(!walkable_normal(Vec3::X, min_cosine));
        assert!(!walkable_normal(Vec3::new(0.0, 0.0, 1.0), min_cosine));
        assert!(!walkable_normal(-Vec3::Y, min_cosine));
        // A hair under vertical is inside a 90° limit. It is not a wall.
        let almost = Vec3::new(1.0, 1.0e-4, 0.0).normalized();
        assert!(almost.y > 0.0 && almost.y < 1.0e-3, "{almost:?}");
        assert!(walkable_normal(almost, min_cosine));
        let limit_45 = slope_cosine(std::f32::consts::FRAC_PI_4).unwrap();
        let slope_45 = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4).rotate(Vec3::Y);
        assert!(walkable_normal(slope_45, limit_45), "{slope_45:?} vs {limit_45}");
        let slope_60 = Quat::from_rotation_z(60.0_f32.to_radians()).rotate(Vec3::Y);
        assert!(!walkable_normal(slope_60, limit_45), "{slope_60:?}");
        assert!(slope_cosine(-0.1).is_none());
        assert!(slope_cosine(f32::NAN).is_none());
        assert!(slope_cosine(std::f32::consts::PI).is_none());
    }
}
