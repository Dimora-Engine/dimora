use engine_math::{BVec3, Vec3};
use hecs::Entity;

/// Sphere, box, or capsule. A capsule is centered and runs along local +Y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColliderShape {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
    Capsule { radius: f32, half_height: f32 },
}

/// One collider on a gameplay entity.
///
/// `offset` is the shape center in the entity's unscaled local space. The world center is
/// `GlobalTransform`'s matrix applied to that point, signed scale included.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Collider {
    pub shape: ColliderShape,
    pub offset: Vec3,
    pub layers: u32,
    pub mask: u32,
    pub sensor: bool,
    pub enabled: bool,
}

impl Collider {
    pub fn new(shape: ColliderShape) -> Self {
        Self { shape, offset: Vec3::ZERO, layers: 1, mask: u32::MAX, sensor: false, enabled: true }
    }

    pub fn sphere(radius: f32) -> Self {
        Self::new(ColliderShape::Sphere { radius })
    }

    pub fn aabb_box(half_extents: Vec3) -> Self {
        Self::new(ColliderShape::Box { half_extents })
    }

    pub fn capsule(radius: f32, half_height: f32) -> Self {
        Self::new(ColliderShape::Capsule { radius, half_height })
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_layers(mut self, layers: u32) -> Self {
        self.layers = layers;
        self
    }

    pub fn with_mask(mut self, mask: u32) -> Self {
        self.mask = mask;
        self
    }

    pub fn sensor(mut self) -> Self {
        self.sensor = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for Collider {
    fn default() -> Self {
        Self::sphere(0.5)
    }
}

/// A free query against the collision snapshot.
///
/// The mask is tested one way, against each collider's `layers`. Pair tests use
/// [`pair_interacts`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionQuery {
    pub mask: u32,
    pub exclude: Option<Entity>,
    pub include_sensors: bool,
}

impl Default for CollisionQuery {
    fn default() -> Self {
        Self { mask: u32::MAX, exclude: None, include_sensors: true }
    }
}

impl CollisionQuery {
    pub fn all() -> Self {
        Self::default()
    }

    pub fn exclude(mut self, entity: Entity) -> Self {
        self.exclude = Some(entity);
        self
    }

    pub fn solids_only(mut self) -> Self {
        self.include_sensors = false;
        self
    }

    pub fn mask(mut self, mask: u32) -> Self {
        self.mask = mask;
        self
    }
}

/// One ray hit in world space. `distance` is along the normalized direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub entity: Entity,
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// Whether the fixed collision system copies indexed shapes into `DebugDraw`.
///
/// Off unless a caller turns it on. [`crate::sync_collision`] does not read this and does not draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollisionDebug {
    pub enabled: bool,
}

impl CollisionDebug {
    pub fn off() -> Self {
        Self { enabled: false }
    }

    pub fn on() -> Self {
        Self { enabled: true }
    }
}

/// Why a collider was left out of the index. Disabled colliders are omitted and are not a skip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    NoTransform,
    MissingParent,
    NonFinite,
    Degenerate,
    Shear,
    NonUniformScale,
    BadShape,
}

impl core::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            SkipReason::NoTransform => "collider has no Transform",
            SkipReason::MissingParent => "Parent names an entity that is gone",
            SkipReason::NonFinite => "transform or shape is non-finite",
            SkipReason::Degenerate => "a scale axis is zero",
            SkipReason::Shear => "the world basis is not orthogonal",
            SkipReason::NonUniformScale => "a sphere or capsule has non-uniform scale",
            SkipReason::BadShape => "radius or extents are not positive and finite",
        })
    }
}

/// How the last movement resolution finished.
///
/// `Blocked` and `Rejected` cannot both be true. A blocked move may still have changed the
/// transform. A rejected move did not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveStatus {
    /// The unlocked part of the request was applied.
    Completed,
    /// A contact, an unfinished cast, or a slide limit kept some of the request back.
    Blocked,
    /// The transform and the collision index were left as they were.
    Rejected,
}

/// Persistent movement settings. This is not a velocity, a gravity setting, or a jump.
///
/// `skin` is the gap the sweep keeps from a solid. `max_slides` is how many sweep iterations one
/// fixed step may spend. `locked_axes` zeroes those world components on the way in, after a slide,
/// and on a recovery push. Persistence marks this as editor data. It does not write a scene file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterMotor {
    pub skin: f32,
    pub max_slides: u8,
    pub locked_axes: BVec3,
}

impl Default for CharacterMotor {
    fn default() -> Self {
        Self { skin: 0.01, max_slides: 4, locked_axes: BVec3::FALSE }
    }
}

impl CharacterMotor {
    pub fn new() -> Self {
        Self::default()
    }
}

/// World-space displacement for one fixed iteration.
///
/// This is not a velocity and it is not a camera direction. The collision pass consumes it.
/// An intent still present after the fixed loop, including a loop that ran zero iterations, is
/// removed. An intent written during `update` waits for the next simulating step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveIntent {
    pub translation: Vec3,
}

impl MoveIntent {
    pub fn new(translation: Vec3) -> Self {
        Self { translation }
    }
}

/// The last movement resolution for this entity.
///
/// `requested` is the intent before locked axes were cleared. `applied` is how far the entity
/// origin moved in world space, including a recovery push when one was applied. `normals` are
/// world-space obstacle normals in slide order, pointing back toward the mover. `recovery_normal`
/// is that same kind of normal when a penetration push was applied.
///
/// This is the last resolution, not a claim that the character is standing on a surface. A later
/// frame that does not process a new intent leaves this value as it was. A new resolution replaces
/// it, including a zero move, so old normals are not reported as a fresh contact.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveResult {
    pub requested: Vec3,
    pub applied: Vec3,
    pub status: MoveStatus,
    pub normals: Vec<Vec3>,
    pub recovery_normal: Option<Vec3>,
}

impl MoveResult {
    pub fn blocked(&self) -> bool {
        self.status == MoveStatus::Blocked
    }

    pub fn rejected(&self) -> bool {
        self.status == MoveStatus::Rejected
    }
}

/// One overlap lifecycle event. `a` and `b` are in ascending [`Entity`] order, and at least one was
/// a sensor in the index when the pair was detected.
///
/// [`World::send`](crate::World::send) appends to this type's queue. A pass through a sensor sends
/// [`TriggerEvent::Entered`] and then [`TriggerEvent::Exited`] for that pair before the next pair.
/// A pair that is crossed more than once in the same fixed iteration still produces that one
/// outcome. `update` and the stages after it read the queue with
/// [`World::events`](crate::World::events). A fixed system does not see events sent in the fixed
/// loop, including events from an earlier iteration of the same step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerEvent {
    Entered { a: Entity, b: Entity },
    Exited { a: Entity, b: Entity },
}

/// Mutual filter. A collider does not interact with itself. Sensors pass the filter and do not
/// block movement.
pub fn pair_interacts(a_entity: Entity, a: &Collider, b_entity: Entity, b: &Collider) -> bool {
    a_entity != b_entity && a.enabled && b.enabled && a.layers & b.mask != 0 && b.layers & a.mask != 0
}

pub(crate) fn shape_ok(shape: ColliderShape) -> bool {
    match shape {
        ColliderShape::Sphere { radius } => finite_positive(radius),
        ColliderShape::Box { half_extents } => {
            finite_positive(half_extents.x) && finite_positive(half_extents.y) && finite_positive(half_extents.z)
        }
        ColliderShape::Capsule { radius, half_height } => {
            finite_positive(radius) && half_height.is_finite() && half_height >= 0.0
        }
    }
}

fn finite_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

pub(crate) fn point_finite(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}
