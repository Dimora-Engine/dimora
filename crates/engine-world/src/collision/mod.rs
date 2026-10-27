//! Spatial collider components. Parry types stay inside this module.

mod types;

pub use types::{pair_interacts, Collider, ColliderShape, CollisionQuery, RayHit, SkipReason};
