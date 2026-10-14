//! The world: entities, components, resources and systems.
//!
//! Storage is `hecs`. Gameplay code uses [`World`], [`Schedule`] and the resource API.
//! [`World::entities`] and the `hecs` re-exports below stay reachable until M9 writes the API list
//! (ADR-0010). M3a does not wrap `hecs` and adds no further `hecs` type.
//!
//! Spatial components shared by everything that exists in the scene live here: [`Transform`],
//! [`GlobalTransform`], [`Parent`]/[`Children`], [`Visibility`], [`Name`], [`Interpolate`].

mod commands;
mod events;
mod hierarchy;
mod interpolate;
mod resources;
mod schedule;
mod world;

/// `Entity`, `EntityRef`, `Query` and `QueryBorrow` from `hecs`.
///
/// Internal until M9 (ADR-0010). `Entity` is the id the rest of the API already uses.
/// M3a adds no further `hecs` re-export.
pub use hecs::{Entity, EntityRef, Query, QueryBorrow};

pub use commands::{CommandError, CommandFailure, CommandWriter};
pub use engine_math::Transform;
pub use hierarchy::{propagate_transforms, Children, GlobalTransform, HierarchyError, Parent};
pub use interpolate::{snapshot_interpolation, Interpolate};
pub use resources::Resources;
pub use schedule::{Schedule, ScheduleError};
pub use world::{Name, Visibility, World};
