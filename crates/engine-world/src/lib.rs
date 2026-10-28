//! The world: entities, components, resources and systems.
//!
//! Storage is `hecs`. Gameplay code uses [`World`], [`Schedule`] and the resource API.
//! [`World::entities`] and the `hecs` re-exports below stay reachable until M9 writes the API list
//! (ADR-0010). M3a does not wrap `hecs` and adds no further `hecs` type.
//!
//! Spatial components shared by everything that exists in the scene live here: [`Transform`],
//! [`GlobalTransform`], [`Parent`]/[`Children`], [`Visibility`], [`Name`], [`Tags`],
//! [`SceneMember`], [`Interpolate`].
//!
//! [`ComponentRegistry`] names those components for debug dumps. It does not serialize them.

mod builtin;
mod collision;
mod commands;
mod dump;
mod events;
mod hierarchy;
mod interpolate;
mod lookup;
mod registry;
mod resources;
mod scene;
mod schedule;
mod world;

/// `Entity`, `EntityRef`, `Query` and `QueryBorrow` from `hecs`.
///
/// Internal until M9 (ADR-0010). `Entity` is the id the rest of the API already uses.
/// M3a adds no further `hecs` re-export.
pub use hecs::{Entity, EntityRef, Query, QueryBorrow};

pub use builtin::register_world_components;
pub use collision::{
    clear_move_intents, pair_interacts, sync_collision, CharacterMotor, Collider, ColliderShape, CollisionDebug,
    CollisionQuery, CollisionWorld, MoveIntent, MoveResult, MoveStatus, RayHit, SkipReason, TriggerEvent,
};
pub use commands::{CommandError, CommandFailure, CommandWriter};
pub use dump::DumpError;
pub use engine_math::Transform;
pub use hierarchy::{propagate_transforms, Children, GlobalTransform, HierarchyError, Parent};
pub use interpolate::{snapshot_interpolation, Interpolate};
pub use lookup::Tags;
pub use registry::{ComponentDesc, ComponentInfo, ComponentRegistry, FieldDesc, Persistence, RegistryError};
pub use resources::Resources;
pub use scene::{SceneBuilder, SceneError, SceneInstanceId, SceneMember, SceneOpId, SceneOpStatus};
pub use schedule::{Schedule, ScheduleError};
pub use world::{Name, Visibility, World};
