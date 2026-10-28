//! Flat-ground support for a gameplay body. This is not gravity, jumping, or slope travel.
//!
//! [`CharacterMotor`] remains the displacement solver. An enabled [`CharacterLocomotion`] writes
//! that motor's [`MoveIntent`] for the iteration. The support query reads the index
//! [`sync_collision`](super::sync_collision) just rebuilt. It does not build a second index, and
//! it does not read `MoveResult` normals.
//!
//! The probe is along world −Y. A candidate counts only when the closest-feature gap is inside
//! the reach window: not a deep penetration, and not farther than `skin + stick`. The nearest
//! valid gap wins. Gaps within `1e-5` keep the lower [`Entity`]. The wall normal is the feature
//! normal, from the surface toward the body, and it must face up within `max_slope`. Sensors and
//! a one-way query mask do not count. The motor's mutual layer test does.
//!
//! `Unknown` becomes `Supported` or `Airborne` on the first positive fixed step. That first
//! contact is not a landing. Leaving a floor sets `Airborne` and stops the downward stick. There
//! is no gravity here, so an airborne body stays at the height where it lost the floor.

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

/// Persistent locomotion settings for one gameplay body.
///
/// `speed` is world units per second on the XZ plane. `wish` is a direction, and this speed is
/// what turns it into a displacement. `max_slope` is the largest angle, in radians, between world
/// +Y and a support normal. `stick` is a downward adhesion along −Y, in world units, used while
/// the pre-move probe says the body is supported. It does not follow a slope.
///
/// A non-finite or negative `speed` or `stick` contributes nothing. A `max_slope` outside
/// `0..=π/2`, or a non-finite one, accepts no support. Those cases do not write a non-finite move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterLocomotion {
    pub enabled: bool,
    pub speed: f32,
    pub max_slope: f32,
    pub stick: f32,
}

impl Default for CharacterLocomotion {
    fn default() -> Self {
        Self { enabled: true, speed: 90.0, max_slope: std::f32::consts::FRAC_PI_4, stick: 0.08 }
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

/// How the last committed horizontal move looked. `Walk` is locomotion that actually moved the
/// body on XZ. Holding a wish into a wall, and a vertical skin push, stay `Idle`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LocomotionPhase {
    #[default]
    Idle,
    Walk,
}

/// Runtime support and phase. This is not saved configuration and it is not a velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterMotion {
    pub support: GroundSupport,
    pub phase: LocomotionPhase,
}

impl Default for CharacterMotion {
    fn default() -> Self {
        Self { support: GroundSupport::Unknown, phase: LocomotionPhase::Idle }
    }
}

impl CharacterMotion {
    pub fn new() -> Self {
        Self::default()
    }
}

/// World XZ direction for one fixed iteration. This is not a displacement and not a velocity.
///
/// Y is ignored. A non-finite X or Z becomes no direction. A horizontal length above 1 is
/// normalized so a diagonal does not go faster than `speed`. A shorter direction is kept, so a
/// partial stick is a slower walk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocomotionIntent {
    pub wish: Vec3,
}

impl LocomotionIntent {
    pub fn new(wish: Vec3) -> Self {
        Self { wish }
    }
}

/// Entities whose overlapping `MoveIntent` was already reported, and which bodies received a
/// locomotion intent during the prepare that the following settle must read.
#[derive(Clone, Debug, Default)]
struct LocomotionNotes {
    conflicts: Vec<Entity>,
    missing_motion: Vec<Entity>,
    submitted: Vec<Entity>,
}

/// `Some` only for a finite duration strictly above zero. Zero, negative, and non-finite values
/// are `None`: collision still runs, and locomotion does not.
pub(super) fn usable_step(step_secs: Option<f32>) -> Option<f32> {
    step_secs.filter(|step| step.is_finite() && *step > 0.0)
}

/// Writes locomotion displacements and drops a competing [`MoveIntent`].
///
/// The index is the one just rebuilt, before motors move anyone. Bodies run in ascending
/// [`Entity`] order.
pub(super) fn prepare_locomotion(world: &mut World, step_secs: f32) {
    let mut bodies: Vec<(Entity, CharacterLocomotion, CharacterMotor)> = world
        .query::<(&CharacterLocomotion, &CharacterMotor)>()
        .iter()
        .map(|(entity, (locomotion, motor))| (entity, *locomotion, *motor))
        .collect();
    bodies.sort_by_key(|(entity, _, _)| *entity);

    let mut notes = take_notes(world);
    let mut conflicts_now = Vec::new();
    let mut missing_now = Vec::new();
    notes.submitted.clear();

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
        if world.component::<CharacterMotion>(entity).is_err() {
            missing_now.push(entity);
            if !notes.missing_motion.contains(&entity) {
                log::warn!(
                    "locomotion on {entity:?} has no CharacterMotion; its MoveIntent was discarded and it did not move"
                );
            }
            continue;
        }

        let pre = {
            let collision = world.resource::<CollisionWorld>();
            probe(&collision, entity, &motor, &locomotion)
        };
        let wish = world
            .component::<LocomotionIntent>(entity)
            .map(|intent| horizontal_wish(intent.wish))
            .unwrap_or(Vec3::ZERO);
        let _ = world.remove::<LocomotionIntent>(entity);

        let Some(delta) = displacement(&locomotion, wish, step_secs, &pre) else {
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

/// Stores the support at the pose the motor committed, and sets Idle or Walk from that move.
pub(super) fn settle_locomotion(world: &mut World) {
    let mut bodies: Vec<(Entity, CharacterLocomotion, CharacterMotor)> = world
        .query::<(&CharacterLocomotion, &CharacterMotor, &CharacterMotion)>()
        .iter()
        .filter(|(_, (locomotion, _, _))| locomotion.enabled)
        .map(|(entity, (locomotion, motor, _))| (entity, *locomotion, *motor))
        .collect();
    bodies.sort_by_key(|(entity, _, _)| *entity);
    let submitted = take_notes(world).submitted;

    for (entity, locomotion, motor) in bodies {
        let support = {
            let collision = world.resource::<CollisionWorld>();
            probe(&collision, entity, &motor, &locomotion)
        };
        let phase = phase_after_move(world, entity, submitted.contains(&entity));
        let _ = world.insert(entity, (CharacterMotion { support, phase },));
    }

    let mut notes = take_notes(world);
    notes.submitted.clear();
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

fn displacement(locomotion: &CharacterLocomotion, wish: Vec3, step_secs: f32, pre: &GroundSupport) -> Option<Vec3> {
    if matches!(pre, GroundSupport::Unavailable) {
        return None;
    }
    let speed = finite_nonneg(locomotion.speed).unwrap_or(0.0);
    let mut delta = Vec3::new(wish.x * speed * step_secs, 0.0, wish.z * speed * step_secs);
    if matches!(pre, GroundSupport::Supported { .. }) {
        delta.y -= finite_nonneg(locomotion.stick).unwrap_or(0.0);
    }
    if !point_finite(delta) || delta.length_squared() == 0.0 {
        return None;
    }
    Some(delta)
}

fn phase_after_move(world: &World, entity: Entity, submitted: bool) -> LocomotionPhase {
    if !submitted {
        return LocomotionPhase::Idle;
    }
    let Ok(result) = world.component::<MoveResult>(entity) else {
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

/// Support at the body's current indexed pose.
///
/// `Unavailable` when the body is absent from the index or its collider is a sensor. Otherwise
/// the nearest walkable solid inside the reach window, or `Airborne`.
fn probe(
    collision: &CollisionWorld,
    entity: Entity,
    motor: &CharacterMotor,
    locomotion: &CharacterLocomotion,
) -> GroundSupport {
    let list = entries(collision);
    let Some(mover_index) = list.iter().position(|entry| entry.entity == entity) else {
        return GroundSupport::Unavailable;
    };
    let mover = &list[mover_index];
    if mover.sensor {
        return GroundSupport::Unavailable;
    }
    let Some(min_cosine) = slope_cosine(locomotion.max_slope) else {
        return GroundSupport::Airborne;
    };
    let skin = finite_nonneg(motor.skin).unwrap_or(0.0);
    let stick = finite_nonneg(locomotion.stick).unwrap_or(0.0);
    let reach = skin + stick;
    let min_gap = -(skin + PENETRATION_SLOP);

    let mut best: Option<(f32, Entity, Vec3)> = None;
    for entry in list {
        if entry.entity == entity || entry.sensor || !mutual(mover.layers, mover.mask, entry.layers, entry.mask) {
            continue;
        }
        let Some(feature) = feature_hit(&mover.prepared, mover.prepared.center, &entry.prepared) else {
            continue;
        };
        let Some(normal) = feature.wall else {
            continue;
        };
        if !normal.y.is_finite() || normal.y < min_cosine {
            continue;
        }
        // A largely negative gap is inside the solid. The smallest gap in the whole index is
        // often that penetration, and it is not a floor the body can stand on.
        if !feature.gap.is_finite() || feature.gap < min_gap || feature.gap > reach {
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

    match best {
        Some((_, entity, normal)) => GroundSupport::Supported { entity, normal },
        None => GroundSupport::Airborne,
    }
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
