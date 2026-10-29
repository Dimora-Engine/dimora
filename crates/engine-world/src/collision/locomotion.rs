//! Flat-ground support, gravity, and jumping for a gameplay body. This is not slope travel.
//!
//! [`CharacterMotor`] remains the displacement solver. An enabled [`CharacterLocomotion`] writes
//! that motor's [`MoveIntent`] for the iteration. The support query reads the index
//! [`sync_collision`](super::sync_collision) just rebuilt. It does not build a second index.
//! Ceiling contact is read from that iteration's fresh [`MoveResult`], not from an older one.
//!
//! The probe is along world −Y. A candidate counts only when the closest-feature gap is inside
//! the reach window: not a deep penetration, and not farther than `skin + stick`. The nearest
//! valid gap wins. Gaps within `1e-5` keep the lower [`Entity`]. The wall normal is the feature
//! normal, from the surface toward the body, and it must face up within `max_slope`. Sensors and
//! a one-way query mask do not count. The motor's mutual layer test does.
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
/// Horizontal displacement shorter than this is not a walk.
const WALK_EPSILON: f32 = 1e-3;
/// A contact normal this far below the horizon is a ceiling, not a wall or a floor.
const CEILING_Y: f32 = -0.5;
/// Upward motion shorter than this did not leave the ground.
const LEFT_GROUND: f32 = 1e-4;

/// Persistent locomotion settings for one gameplay body.
///
/// `speed` is world units per second on the XZ plane. `wish` is a direction, and this speed is
/// what turns it into a displacement. `max_slope` is the largest angle, in radians, between world
/// +Y and a support normal. `stick` is a downward adhesion along −Y, in world units, used while
/// the pre-move probe says the body is supported and it is not jumping. It does not follow a slope.
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
/// gravity has been integrated, or after a ceiling stops the rise. `Walk` is a committed
/// horizontal move that is not jumping or falling, including the step that leaves a ledge before
/// gravity starts. Holding a wish into a wall, a vertical skin push, and a deep overlap that is
/// not support stay `Idle`.
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
/// leaves them at zero and does not read them. Horizontal motion on the ground and in the air is
/// still the wish times `speed` times the fixed step. They are not `MoveResult.applied / dt`.
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
        notes.submitted.push(entity);
    }

    notes.conflicts = conflicts_now;
    notes.missing_motion = missing_now;
    world.insert_resource(notes);
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
    let mut delta = Vec3::new(wish.x * speed * step_secs, 0.0, wish.z * speed * step_secs);
    if airborne_motion || matches!(pre, GroundSupport::Airborne) {
        delta.y += motion.velocity.y * step_secs;
    } else if matches!(pre, GroundSupport::Supported { .. }) {
        delta.y -= finite_nonneg(locomotion.stick).unwrap_or(0.0);
    }
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
    let horizontal = Vec3::new(result.applied.x, 0.0, result.applied.z).length();
    if horizontal > WALK_EPSILON {
        LocomotionPhase::Walk
    } else {
        LocomotionPhase::Idle
    }
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
        if !normal.y.is_finite() || normal.y < min_cosine {
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

fn slope_cosine(max_slope: f32) -> Option<f32> {
    if max_slope.is_finite() && (0.0..=std::f32::consts::FRAC_PI_2).contains(&max_slope) {
        Some(max_slope.cos())
    } else {
        None
    }
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
