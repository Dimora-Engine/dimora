//! The world: entities, components, resources and systems.
//!
//! Storage is [`hecs`], a small archetype ECS. We wrap it rather than expose it directly so that
//! gameplay code depends on *our* `World`, `Schedule` and resource API, and the storage can be
//! swapped without touching every call site. The raw `hecs::World` stays reachable as
//! [`World::entities`] for queries.
//!
//! Spatial components shared by everything that exists in the scene live here: [`Transform`],
//! [`GlobalTransform`], [`Parent`]/[`Children`], [`Visibility`], [`Name`].

mod hierarchy;
mod resources;
mod schedule;
mod world;

pub use hecs::{Entity, EntityRef, Query, QueryBorrow};

pub use engine_math::Transform;
pub use hierarchy::{propagate_transforms, Children, GlobalTransform, HierarchyError, Parent};
pub use resources::Resources;
pub use schedule::Schedule;
pub use world::{Name, Visibility, World};
