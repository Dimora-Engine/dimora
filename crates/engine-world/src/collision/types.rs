use engine_math::Vec3;
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

/// Mutual filter. A collider does not interact with itself. Sensors still pass; they block nothing
/// until movement exists.
pub fn pair_interacts(a_entity: Entity, a: &Collider, b_entity: Entity, b: &Collider) -> bool {
    a_entity != b_entity && a.enabled && b.enabled && a.layers & b.mask != 0 && b.layers & a.mask != 0
}

#[allow(dead_code)] // Used when the world-space conversion is built on these types.
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

#[allow(dead_code)]
fn finite_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

#[allow(dead_code)]
pub(crate) fn point_finite(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}
