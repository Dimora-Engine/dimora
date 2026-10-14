//! The world: entities, components, resources and systems.
//!
//! Storage is [`hecs`], a small archetype ECS. We wrap it rather than expose it directly so that
//! gameplay code depends on *our* `World`, `Schedule` and resource API, and the storage can be
//! swapped without touching every call site. The raw `hecs::World` stays reachable as
//! [`World::entities`] for queries.
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

pub use hecs::{Entity, EntityRef, Query, QueryBorrow};

pub use commands::{CommandError, CommandFailure, CommandWriter};
pub use engine_math::Transform;
pub use hierarchy::{propagate_transforms, Children, GlobalTransform, HierarchyError, Parent};
pub use interpolate::{snapshot_interpolation, Interpolate};
pub use resources::Resources;
pub use schedule::{Schedule, ScheduleError};
pub use world::{Name, Visibility, World};
