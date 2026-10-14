//! Deferred structural changes. Systems append here while a query is open; [`World::flush_commands`]
//! applies the queue in order through the existing world methods.

use std::any::type_name;

use hecs::{Component, ComponentError, DynamicBundle, Entity, NoSuchEntity};

use crate::hierarchy::{Children, HierarchyError, Parent};
use crate::resources::missing;
use crate::World;

/// One command that did not apply. Later commands in the same flush still run.
///
/// A failed command writes nothing. The queue does not roll back commands that already applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    /// `entity` was not alive when the command ran.
    NoSuchEntity { entity: Entity },
    /// `entity` was alive and did not have `component`.
    MissingComponent { entity: Entity, component: &'static str },
    /// [`World::set_parent`] refused this pair. The previous parent is unchanged.
    Cycle { child: Entity, parent: Entity },
    /// The bundle contained [`Parent`] or [`Children`]. Hierarchy edits go through
    /// [`CommandWriter::set_parent`], [`CommandWriter::detach_children`] and
    /// [`CommandWriter::remove`].
    HierarchyComponent { entity: Entity, component: &'static str },
}

impl core::fmt::Display for CommandError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CommandError::NoSuchEntity { entity } => write!(f, "entity {entity:?} does not exist"),
            CommandError::MissingComponent { entity, component } => {
                write!(f, "entity {entity:?} has no `{component}` component")
            }
            CommandError::Cycle { child, parent } => {
                write!(f, "parenting {child:?} to {parent:?} would create a cycle")
            }
            CommandError::HierarchyComponent { entity, component } => {
                write!(
                    f,
                    "command for {entity:?} cannot insert `{component}`; use set_parent, detach_children or remove"
                )
            }
        }
    }
}

impl std::error::Error for CommandError {}

/// A command that failed during a flush, with the stage that flushed it when the frame loop did.
///
/// This is the only failure record. A later logger reads the list; it does not keep a second copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandFailure {
    /// Stage name from the frame loop. `None` when [`World::flush_commands`] was called directly.
    pub stage: Option<&'static str>,
    pub error: CommandError,
}

impl core::fmt::Display for CommandFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.stage {
            Some(stage) => write!(f, "{stage}: {}", self.error),
            None => write!(f, "{}", self.error),
        }
    }
}

impl std::error::Error for CommandFailure {}

type CommandFn = Box<dyn FnOnce(&mut World) -> Result<(), CommandError> + Send>;

#[derive(Default)]
pub(crate) struct Commands {
    queue: Vec<CommandFn>,
    failures: Vec<CommandFailure>,
}

/// Appends structural changes without borrowing the world mutably.
///
/// [`World::commands`] does not hold the queue borrow. Each method borrows it only for the append,
/// so a second writer in the same system is safe, including while a query is open. [`World::try_commands`]
/// returns `None` when that brief borrow would panic. The frame loop flushes at each stage boundary,
/// so a system does not call `flush_commands` itself.
pub struct CommandWriter<'a> {
    world: &'a World,
}

impl CommandWriter<'_> {
    /// Reserves an id now and spawns `bundle` when the queue flushes.
    ///
    /// The id is not visible to queries until the flush. A bundle that contains [`Parent`] or
    /// [`Children`] is rejected at flush and the reserved id is despawned, so the world is
    /// unchanged.
    pub fn spawn(&mut self, bundle: impl DynamicBundle + Send + 'static) -> Entity {
        let entity = self.world.entities.reserve_entity();
        if let Some(component) = hierarchy_component(&bundle) {
            self.push(Box::new(move |world| {
                if world.contains(entity) {
                    let _ = world.despawn(entity);
                }
                Err(CommandError::HierarchyComponent { entity, component })
            }));
            return entity;
        }
        self.push(Box::new(move |world| {
            world.insert(entity, bundle).map_err(|NoSuchEntity| CommandError::NoSuchEntity { entity })
        }));
        entity
    }

    /// Despawns `entity` and its descendants when the queue flushes ([`World::despawn`]).
    pub fn despawn(&mut self, entity: Entity) {
        self.push(Box::new(move |world| {
            world.despawn(entity).map_err(|NoSuchEntity| CommandError::NoSuchEntity { entity })
        }));
    }

    /// Inserts `bundle` when the queue flushes ([`World::insert`]).
    ///
    /// A bundle that contains [`Parent`] or [`Children`] fails and writes nothing.
    pub fn insert(&mut self, entity: Entity, bundle: impl DynamicBundle + Send + 'static) {
        if let Some(component) = hierarchy_component(&bundle) {
            self.push(Box::new(move |_| Err(CommandError::HierarchyComponent { entity, component })));
            return;
        }
        self.push(Box::new(move |world| {
            world.insert(entity, bundle).map_err(|NoSuchEntity| CommandError::NoSuchEntity { entity })
        }));
    }

    /// Removes one `T` when the queue flushes ([`World::remove`]).
    pub fn remove<T: Component>(&mut self, entity: Entity) {
        let component = type_name::<T>();
        self.push(Box::new(move |world| {
            world.remove::<T>(entity).map_err(|error| match error {
                ComponentError::NoSuchEntity => CommandError::NoSuchEntity { entity },
                ComponentError::MissingComponent(_) => CommandError::MissingComponent { entity, component },
            })
        }));
    }

    /// Parents `child` to `parent` when the queue flushes ([`World::set_parent`]).
    ///
    /// A cycle or a missing entity fails and leaves the previous parent in place.
    pub fn set_parent(&mut self, child: Entity, parent: Entity) {
        self.push(Box::new(move |world| {
            world.set_parent(child, parent).map_err(|error| match error {
                HierarchyError::Cycle => CommandError::Cycle { child, parent },
                HierarchyError::NoSuchEntity => {
                    CommandError::NoSuchEntity { entity: if world.contains(child) { parent } else { child } }
                }
            })
        }));
    }

    /// Detaches the children of `parent` when the queue flushes ([`World::detach_children`]).
    pub fn detach_children(&mut self, parent: Entity) {
        self.push(Box::new(move |world| {
            world.detach_children(parent).map_err(|NoSuchEntity| CommandError::NoSuchEntity { entity: parent })
        }));
    }

    fn push(&self, command: CommandFn) {
        self.world.resource_mut::<Commands>().queue.push(command);
    }
}

fn hierarchy_component(bundle: &impl DynamicBundle) -> Option<&'static str> {
    if bundle.has::<Parent>() {
        Some(type_name::<Parent>())
    } else if bundle.has::<Children>() {
        Some(type_name::<Children>())
    } else {
        None
    }
}

impl World {
    /// A writer for the deferred command queue.
    ///
    /// The writer does not borrow the queue. [`World::try_commands`] is the fallible form for a
    /// caller that may already be inside a queue borrow.
    pub fn commands(&self) -> CommandWriter<'_> {
        CommandWriter { world: self }
    }

    /// [`World::commands`] when the queue can be borrowed, or `None` when it is already borrowed.
    ///
    /// A missing queue is still a panic. That resource is inserted by [`World::new`].
    pub fn try_commands(&self) -> Option<CommandWriter<'_>> {
        if !self.has_resource::<Commands>() {
            missing::<Commands>();
        }
        match self.try_resource_mut::<Commands>() {
            Ok(Some(guard)) => {
                drop(guard);
                Some(CommandWriter { world: self })
            }
            Ok(None) => missing::<Commands>(),
            Err(()) => None,
        }
    }

    /// Applies queued commands in call order. Each error is recorded and the queue continues.
    ///
    /// Reserved spawn ids are materialized before the first command runs. The stage on each
    /// failure is `None`. [`World::flush_commands_in`] records a stage name. Failures stay
    /// readable from [`World::command_failures`] until [`World::clear_command_failures`].
    pub fn flush_commands(&mut self) {
        self.flush_in(None);
    }

    /// [`World::flush_commands`] with `stage` stored on each failure from this flush.
    pub fn flush_commands_in(&mut self, stage: &'static str) {
        self.flush_in(Some(stage));
    }

    fn flush_in(&mut self, stage: Option<&'static str>) {
        let mut commands = self.remove_resource::<Commands>().unwrap_or_else(|| missing::<Commands>());
        self.entities.flush();
        for command in std::mem::take(&mut commands.queue) {
            if let Err(error) = command(self) {
                commands.failures.push(CommandFailure { stage, error });
            }
        }
        self.insert_resource(commands);
    }

    /// Failures from flushes since the last [`World::clear_command_failures`], in the order they happened.
    ///
    /// The frame loop clears this at the start of the next step, so the list is still readable
    /// after `step` returns. This list is the record a logger should print. It is not replaced by the log.
    pub fn command_failures(&self) -> Vec<CommandFailure> {
        self.resource::<Commands>().failures.clone()
    }

    /// Drops recorded command failures. The frame loop does this at the start of each step.
    pub fn clear_command_failures(&mut self) {
        self.resource_mut::<Commands>().failures.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Children, Parent, Transform};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Marker(u32);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Tag;

    fn hierarchy_ok(world: &World) {
        for (entity, parent) in world.entities.query::<&Parent>().iter() {
            assert!(world.contains(parent.0), "{entity:?} points at a dead parent");
            let listed = world.component::<Children>(parent.0).map(|c| c.0.contains(&entity)).unwrap_or(false);
            assert!(listed, "{entity:?} is missing from its parent's child list");
        }
        for (entity, children) in world.entities.query::<&Children>().iter() {
            for &child in &children.0 {
                assert!(world.contains(child), "{entity:?} lists a dead child");
                assert_eq!(world.component::<Parent>(child).map(|p| p.0).ok(), Some(entity));
            }
        }
    }

    #[test]
    fn commands_queued_during_a_query_apply_at_flush_in_call_order() {
        let mut world = World::new();
        let first = world.spawn((Marker(1),));
        let spawned = {
            let mut commands = world.commands();
            let mut query = world.query::<&Marker>();
            let mut seen = Vec::new();
            for (entity, marker) in query.iter() {
                seen.push(marker.0);
                commands.insert(entity, (Tag,));
                commands.spawn((Marker(marker.0 + 10),));
            }
            assert_eq!(seen, vec![1]);
            let id = commands.spawn((Marker(3),));
            commands.insert(first, (Marker(2),));
            commands.insert(first, (Marker(4),));
            drop(query);
            assert!(world.component::<Tag>(first).is_err(), "the insert is still queued");
            assert_eq!(world.query::<&Marker>().iter().count(), 1);
            id
        };
        assert!(world.command_failures().is_empty());
        world.flush_commands();
        assert!(world.command_failures().is_empty());
        assert_eq!(world.component::<Marker>(first).unwrap().0, 4);
        assert!(world.component::<Tag>(first).is_ok());
        assert_eq!(world.query::<&Marker>().iter().count(), 3);
        assert_eq!(world.component::<Marker>(spawned).unwrap().0, 3);
    }

    #[test]
    fn a_failed_command_changes_nothing_and_the_queue_continues() {
        let mut world = World::new();
        let entity = world.spawn((Marker(1),));
        let dead = world.spawn((Marker(0),));
        world.despawn(dead).unwrap();
        {
            let mut commands = world.commands();
            commands.despawn(dead);
            commands.insert(entity, (Marker(5),));
            commands.remove::<Tag>(entity);
            commands.insert(entity, (Tag,));
        }
        world.flush_commands();
        let failures = world.command_failures();
        assert_eq!(failures.len(), 2);
        assert!(failures.iter().all(|failure| failure.stage.is_none()));
        assert!(matches!(failures[0].error, CommandError::NoSuchEntity { entity: id } if id == dead));
        assert!(matches!(failures[1].error, CommandError::MissingComponent { entity: id, .. } if id == entity));
        assert_eq!(world.component::<Marker>(entity).unwrap().0, 5);
        assert!(world.component::<Tag>(entity).is_ok());
    }

    #[test]
    fn despawn_order_decides_whether_a_later_command_can_see_the_entity() {
        let mut world = World::new();
        let gone_first = world.spawn((Marker(1),));
        let written_first = world.spawn((Marker(1),));
        {
            let mut commands = world.commands();
            commands.despawn(gone_first);
            commands.insert(gone_first, (Marker(9),));
            commands.insert(written_first, (Marker(9),));
            commands.despawn(written_first);
        }
        world.flush_commands();
        assert!(!world.contains(gone_first));
        assert!(!world.contains(written_first));
        let failures = world.command_failures();
        assert_eq!(failures.len(), 1);
        assert!(matches!(failures[0].error, CommandError::NoSuchEntity { entity } if entity == gone_first));
    }

    #[test]
    fn hierarchy_commands_use_the_world_rules() {
        let mut world = World::new();
        let root = world.spawn((Transform::from_xyz(1.0, 0.0, 0.0),));
        let middle = world.spawn_child(root, (Transform::from_xyz(2.0, 0.0, 0.0),));
        let leaf = world.spawn_child(middle, (Transform::IDENTITY,));
        let local_before = *world.component::<Transform>(middle).unwrap();
        {
            let mut commands = world.commands();
            commands.set_parent(root, leaf);
            commands.set_parent(leaf, root);
            commands.remove::<Parent>(middle);
        }
        world.flush_commands();
        assert!(matches!(world.command_failures()[0].error, CommandError::Cycle { .. }));
        assert!(world.component::<Parent>(root).is_err(), "the cycle left the root unparented");
        assert_eq!(world.component::<Parent>(leaf).unwrap().0, root);
        assert!(world.component::<Parent>(middle).is_err());
        assert_eq!(*world.component::<Transform>(middle).unwrap(), local_before);
        assert!(!world.component::<Children>(root).unwrap().0.contains(&middle));
        hierarchy_ok(&world);

        world.clear_command_failures();
        {
            let mut commands = world.commands();
            commands.despawn(root);
            commands.insert(leaf, (Tag,));
        }
        world.flush_commands();
        assert!(!world.contains(root) && !world.contains(leaf));
        assert!(matches!(world.command_failures()[0].error, CommandError::NoSuchEntity { .. }));
        hierarchy_ok(&world);

        let parent = world.spawn((Transform::from_xyz(4.0, 0.0, 0.0),));
        let child = world.spawn_child(parent, (Transform::from_xyz(1.0, 0.0, 0.0),));
        world.commands().detach_children(parent);
        world.flush_commands();
        assert!(world.component::<Parent>(child).is_err());
        assert!(world.contains(child));
        hierarchy_ok(&world);
    }

    #[test]
    fn a_bundle_cannot_smuggle_a_parent_or_children_component() {
        let mut world = World::new();
        let parent = world.spawn((Transform::IDENTITY,));
        let entity = world.spawn((Marker(1),));
        let reserved = {
            let mut commands = world.commands();
            commands.insert(entity, (Marker(8), Parent(parent)));
            commands.spawn((Parent(parent), Marker(2)))
        };
        world.flush_commands();
        assert_eq!(world.component::<Marker>(entity).unwrap().0, 1, "the rejected insert wrote nothing");
        assert!(world.component::<Parent>(entity).is_err());
        assert!(!world.contains(reserved), "a rejected spawn does not leave the reserved id behind");
        assert_eq!(world.command_failures().len(), 2);
        assert!(world.command_failures().iter().all(|e| matches!(e.error, CommandError::HierarchyComponent { .. })));
        hierarchy_ok(&world);
    }

    #[test]
    fn removing_children_detaches_them_instead_of_dropping_the_list() {
        let mut world = World::new();
        let parent = world.spawn((Transform::from_xyz(3.0, 0.0, 0.0),));
        let child = world.spawn_child(parent, (Transform::IDENTITY,));
        world.commands().remove::<Children>(parent);
        world.flush_commands();
        assert!(world.command_failures().is_empty());
        assert!(world.contains(child));
        assert!(world.component::<Parent>(child).is_err());
        assert!(world.component::<Children>(parent).is_err());
        hierarchy_ok(&world);
    }

    #[test]
    fn two_writers_share_one_queue_and_keep_call_order() {
        let mut world = World::new();
        let entity = world.spawn((Marker(0),));
        {
            let mut first = world.commands();
            let mut second = world.commands();
            first.insert(entity, (Marker(1),));
            second.insert(entity, (Marker(2),));
        }
        world.flush_commands();
        assert_eq!(world.component::<Marker>(entity).unwrap().0, 2);

        {
            let mut first = world.commands();
            let mut second = world.commands();
            second.insert(entity, (Marker(4),));
            first.insert(entity, (Marker(3),));
        }
        world.flush_commands();
        assert_eq!(world.component::<Marker>(entity).unwrap().0, 3);
    }

    #[test]
    fn a_second_writer_can_enqueue_while_a_query_is_open() {
        let mut world = World::new();
        let entity = world.spawn((Marker(1),));
        {
            let mut held = world.commands();
            let mut query = world.query::<&Marker>();
            for (id, _) in query.iter() {
                world.commands().insert(id, (Tag,));
                held.insert(id, (Marker(6),));
            }
        }
        assert!(world.component::<Tag>(entity).is_err());
        world.flush_commands();
        assert!(world.component::<Tag>(entity).is_ok());
        assert_eq!(world.component::<Marker>(entity).unwrap().0, 6);
    }

    #[test]
    fn try_commands_is_none_while_the_queue_is_borrowed() {
        let world = World::new();
        let _guard = world.resource_mut::<Commands>();
        assert!(world.try_commands().is_none());
    }

    #[test]
    #[should_panic(expected = "already borrowed")]
    fn commands_panics_when_the_queue_is_already_borrowed() {
        let world = World::new();
        let _guard = world.resource_mut::<Commands>();
        let mut writer = world.commands();
        let _ = writer.spawn((Marker(1),));
    }
}
