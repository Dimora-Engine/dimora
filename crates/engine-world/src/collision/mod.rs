//! Spatial colliders and the fixed-step query index.
//!
//! [`Collider`] is saved component data. [`CollisionWorld`] is the runtime snapshot built by
//! [`sync_collision`]. Parry types stay in this module.

mod convert;
mod feature;
mod index;
mod locomotion;
mod motor;
mod query;
mod trigger;
mod types;

pub use index::{sync_collision, sync_collision_with_step, CollisionWorld};
pub use locomotion::{
    clear_locomotion_intents, CharacterLocomotion, CharacterMotion, GroundSupport, LocomotionEvent, LocomotionIntent,
    LocomotionPhase,
};
pub use motor::clear_move_intents;
pub use types::{
    pair_interacts, CharacterMotor, Collider, ColliderShape, CollisionDebug, CollisionQuery, MoveIntent, MoveResult,
    MoveStatus, RayHit, SkipReason, TriggerEvent,
};
