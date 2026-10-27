//! Spatial colliders and the fixed-step query index.
//!
//! [`Collider`] is saved component data. [`CollisionWorld`] is the runtime snapshot built by
//! [`sync_collision`]. Parry types stay in this module.

mod convert;
mod index;
mod query;
mod types;

pub use index::{sync_collision, CollisionWorld};
pub use types::{pair_interacts, Collider, ColliderShape, CollisionDebug, CollisionQuery, RayHit, SkipReason};
