//! Scene instance identity and membership.
//!
//! The scene manager resource is inserted by [`World::new`] and is not part of the public API. Scene
//! operations on [`World`] take it out, do their work, and put it back before returning `Ok` or
//! `Err`. A panic during that work is not rolled back: the manager stays out of the world.
//!
//! This module records membership. It does not spawn a scene, roll one back, unload one, or
//! queue a deferred scene command. Those are later M4 checkpoints.

use std::collections::BTreeMap;
use std::fmt;

use hecs::Entity;

use crate::World;

/// Runtime id of one scene instance in this [`World`].
///
/// Issued in increasing order, starting at 0. An id is never issued twice. A later failed
/// instantiation must consume its id the same way and must not publish that instance. This type
/// is not a file id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneInstanceId(u64);

impl SceneInstanceId {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SceneInstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Membership of one entity in one scene instance.
///
/// An entity has at most one. `local` is unique inside `instance` and may repeat in another
/// instance. It is the order in which [`World::track_scene_entity`] recorded the entity, starting
/// at 0. It is not a persistent file id. A future stable id would be a separate field.
///
/// [`Persistence`](crate::Persistence) is `Runtime`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneMember {
    pub instance: SceneInstanceId,
    pub local: u32,
}

/// A scene-tracking operation could not be applied.
///
/// The scene manager is back in the world when it was present at the start of the call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneError {
    /// No manager is stored. A scene operation already holds it, or a caller removed it.
    /// This call does not insert a replacement.
    SceneManagerUnavailable,
    /// `instance` was never created, or was not created in this world.
    NoSuchInstance { instance: SceneInstanceId },
    /// `entity` is not alive.
    NoSuchEntity { entity: Entity },
    /// `entity` already carries [`SceneMember`].
    AlreadyMember { entity: Entity, instance: SceneInstanceId },
    /// Every `u64` instance id has been issued.
    InstanceIdsExhausted,
    /// Every `u32` local id in `instance` has been issued.
    LocalIdsExhausted { instance: SceneInstanceId },
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SceneError::SceneManagerUnavailable => {
                f.write_str("scene manager is not available; a scene operation may already be running")
            }
            SceneError::NoSuchInstance { instance } => write!(f, "scene instance {instance} does not exist"),
            SceneError::NoSuchEntity { entity } => write!(f, "entity {entity:?} does not exist"),
            SceneError::AlreadyMember { entity, instance } => {
                write!(f, "entity {entity:?} is already in scene instance {instance}")
            }
            SceneError::InstanceIdsExhausted => f.write_str("no scene instance ids remain"),
            SceneError::LocalIdsExhausted { instance } => {
                write!(f, "no scene-local ids remain in instance {instance}")
            }
        }
    }
}

impl std::error::Error for SceneError {}

#[derive(Clone, Debug)]
struct InstanceRecord {
    next_local: u32,
    /// Local id to the runtime [`Entity`] recorded for it. Keys are not reused.
    locals: BTreeMap<u32, Entity>,
}

/// Internal bookkeeping for scene instances. Absent from the prelude.
///
/// Lookup copies an [`Entity`] out of this resource before it touches components, so the
/// resource borrow and the component borrow do not overlap.
#[derive(Clone, Debug, Default)]
pub(crate) struct SceneManager {
    next_instance: u64,
    instances: BTreeMap<SceneInstanceId, InstanceRecord>,
}

impl SceneManager {
    fn allocate(&mut self) -> Result<SceneInstanceId, SceneError> {
        let Some(next) = self.next_instance.checked_add(1) else {
            return Err(SceneError::InstanceIdsExhausted);
        };
        let id = SceneInstanceId(self.next_instance);
        self.next_instance = next;
        Ok(id)
    }
}

impl World {
    /// Borrows the manager by taking it out of the resource map.
    ///
    /// On `Ok` and on `Err` from `body`, the same manager value is inserted again. If `body`
    /// panics, that insert does not run. A nested scene operation sees
    /// `SceneError::SceneManagerUnavailable` and does not insert an empty manager over the
    /// value this call still holds.
    fn with_scene_manager<T>(
        &mut self,
        body: impl FnOnce(&mut SceneManager, &mut hecs::World) -> Result<T, SceneError>,
    ) -> Result<T, SceneError> {
        let mut manager = self.remove_resource::<SceneManager>().ok_or(SceneError::SceneManagerUnavailable)?;
        let result = body(&mut manager, &mut self.entities);
        self.insert_resource(manager);
        result
    }

    fn scene_manager(&self) -> Result<std::cell::Ref<'_, SceneManager>, SceneError> {
        self.try_resource::<SceneManager>().ok_or(SceneError::SceneManagerUnavailable)
    }

    /// Opens an empty scene instance. No entities are spawned.
    ///
    /// The id is new. Ids already issued are not used again.
    pub fn create_scene_instance(&mut self) -> Result<SceneInstanceId, SceneError> {
        self.with_scene_manager(|manager, _| {
            let id = manager.allocate()?;
            manager.instances.insert(id, InstanceRecord { next_local: 0, locals: BTreeMap::new() });
            Ok(id)
        })
    }

    /// Records `entity` in `instance` and assigns the next scene-local id.
    ///
    /// Does not spawn, despawn, or edit any other entity. `entity` must be alive and must not
    /// already have [`SceneMember`].
    pub fn track_scene_entity(&mut self, instance: SceneInstanceId, entity: Entity) -> Result<u32, SceneError> {
        self.with_scene_manager(|manager, entities| {
            let Some(record) = manager.instances.get_mut(&instance) else {
                return Err(SceneError::NoSuchInstance { instance });
            };
            if !entities.contains(entity) {
                return Err(SceneError::NoSuchEntity { entity });
            }
            if let Ok(existing) = entities.get::<&SceneMember>(entity) {
                return Err(SceneError::AlreadyMember { entity, instance: existing.instance });
            }
            let Some(next_local) = record.next_local.checked_add(1) else {
                return Err(SceneError::LocalIdsExhausted { instance });
            };
            let local = record.next_local;
            if entities.insert(entity, (SceneMember { instance, local },)).is_err() {
                return Err(SceneError::NoSuchEntity { entity });
            }
            record.next_local = next_local;
            record.locals.insert(local, entity);
            Ok(local)
        })
    }

    /// The live entity recorded at `local` in `instance`.
    ///
    /// `Ok(None)` when the instance is unknown, `local` was never issued, the entity is gone,
    /// or [`SceneMember`] is missing or names a different instance or local id. A stale row is
    /// left in place until [`World::sweep_scene_mappings`].
    pub fn scene_entity(&self, instance: SceneInstanceId, local: u32) -> Result<Option<Entity>, SceneError> {
        let entity = {
            let manager = self.scene_manager()?;
            let Some(record) = manager.instances.get(&instance) else {
                return Ok(None);
            };
            let Some(entity) = record.locals.get(&local).copied() else {
                return Ok(None);
            };
            entity
        };
        Ok(self.member_matches(instance, local, entity).then_some(entity))
    }

    /// Live members of `instance`, in scene-local id order.
    ///
    /// `Ok(None)` when the instance was never created. Stale rows are omitted and not deleted.
    pub fn scene_entities(&self, instance: SceneInstanceId) -> Result<Option<Vec<(u32, Entity)>>, SceneError> {
        let pairs = {
            let manager = self.scene_manager()?;
            let Some(record) = manager.instances.get(&instance) else {
                return Ok(None);
            };
            record.locals.iter().map(|(&local, &entity)| (local, entity)).collect::<Vec<_>>()
        };
        let live = pairs.into_iter().filter(|&(local, entity)| self.member_matches(instance, local, entity)).collect();
        Ok(Some(live))
    }

    /// Instance ids that have been created, in issuance order.
    pub fn scene_instances(&self) -> Result<Vec<SceneInstanceId>, SceneError> {
        Ok(self.scene_manager()?.instances.keys().copied().collect())
    }

    /// [`SceneMember`] on `entity` when it agrees with the instance map.
    ///
    /// `Ok(None)` when `entity` is dead, has no [`SceneMember`], or the component disagrees
    /// with the recorded entity for that instance and local id.
    pub fn scene_membership(&self, entity: Entity) -> Result<Option<SceneMember>, SceneError> {
        if !self.contains(entity) {
            return Ok(None);
        }
        let member = {
            let Ok(member) = self.component::<SceneMember>(entity) else {
                return Ok(None);
            };
            *member
        };
        Ok(self.scene_entity(member.instance, member.local)?.filter(|found| *found == entity).map(|_| member))
    }

    /// Drops recorded rows whose entity is dead or whose [`SceneMember`] does not match.
    ///
    /// Local ids and instance ids are not reused. Empty instances stay. [`World::despawn`] does
    /// not call this. Returns how many rows were removed.
    pub fn sweep_scene_mappings(&mut self) -> Result<usize, SceneError> {
        self.with_scene_manager(|manager, entities| {
            let mut removed = 0;
            for (&instance, record) in &mut manager.instances {
                let stale: Vec<u32> = record
                    .locals
                    .iter()
                    .filter(|(&local, &entity)| !entity_matches(entities, instance, local, entity))
                    .map(|(&local, _)| local)
                    .collect();
                for local in stale {
                    record.locals.remove(&local);
                    removed += 1;
                }
            }
            Ok(removed)
        })
    }

    fn member_matches(&self, instance: SceneInstanceId, local: u32, entity: Entity) -> bool {
        self.contains(entity) && component_matches(self, instance, local, entity)
    }
}

fn component_matches(world: &World, instance: SceneInstanceId, local: u32, entity: Entity) -> bool {
    world
        .component::<SceneMember>(entity)
        .ok()
        .is_some_and(|member| member.instance == instance && member.local == local)
}

fn entity_matches(entities: &hecs::World, instance: SceneInstanceId, local: u32, entity: Entity) -> bool {
    if !entities.contains(entity) {
        return false;
    }
    entities.get::<&SceneMember>(entity).ok().is_some_and(|member| member.instance == instance && member.local == local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::register_world_components;
    use crate::registry::{ComponentRegistry, Persistence};
    use crate::{Name, Transform};

    fn member_line(instance: u64, local: u32) -> String {
        format!(
            "instance: SceneInstanceId script_visible=false script_writable=false = {instance}\n    local: u32 script_visible=false script_writable=false = {local}"
        )
    }

    #[test]
    fn instances_are_distinct_and_bookkeeping_follows_issuance_order() {
        let mut world = World::new();
        let first = world.create_scene_instance().unwrap();
        let second = world.create_scene_instance().unwrap();
        let third = world.create_scene_instance().unwrap();
        assert!(first < second && second < third);
        assert_eq!(world.scene_instances().unwrap(), vec![first, second, third]);
        assert_eq!(first.raw(), 0);
        assert_eq!(second.raw(), 1);
        assert_eq!(third.raw(), 2);
        assert_eq!(world.scene_entities(first).unwrap(), Some(vec![]));
        assert!(world.scene_entity(first, 0).unwrap().is_none());
    }

    #[test]
    fn two_instances_repeat_local_ids_and_keep_distinct_entities() {
        let mut world = World::new();
        let first = world.create_scene_instance().unwrap();
        let second = world.create_scene_instance().unwrap();
        let alpha = world.spawn((Name::new("alpha"),));
        let beta = world.spawn((Name::new("beta"),));
        assert_eq!(world.track_scene_entity(first, alpha).unwrap(), 0);
        assert_eq!(world.track_scene_entity(second, beta).unwrap(), 0);
        assert_ne!(alpha, beta);
        assert_eq!(world.scene_entity(first, 0).unwrap(), Some(alpha));
        assert_eq!(world.scene_entity(second, 0).unwrap(), Some(beta));
        assert_eq!(world.scene_membership(alpha).unwrap(), Some(SceneMember { instance: first, local: 0 }));
        assert_eq!(world.scene_entities(first).unwrap(), Some(vec![(0, alpha)]));
        assert_eq!(world.scene_entities(second).unwrap(), Some(vec![(0, beta)]));
    }

    #[test]
    fn lookup_misses_an_unknown_instance_or_local_id() {
        let mut world = World::new();
        let instance = world.create_scene_instance().unwrap();
        let entity = world.spawn(());
        world.track_scene_entity(instance, entity).unwrap();
        let missing = SceneInstanceId(40);
        assert!(world.scene_entity(missing, 0).unwrap().is_none());
        assert!(world.scene_entities(missing).unwrap().is_none());
        assert!(world.scene_entity(instance, 1).unwrap().is_none());
        assert_eq!(world.scene_entities(instance).unwrap(), Some(vec![(0, entity)]));
    }

    #[test]
    fn a_runtime_entity_without_membership_is_not_in_a_scene() {
        let mut world = World::new();
        let instance = world.create_scene_instance().unwrap();
        let runtime = world.spawn((Transform::IDENTITY,));
        assert!(world.scene_membership(runtime).unwrap().is_none());
        assert!(world.component::<SceneMember>(runtime).is_err());
        assert_eq!(world.scene_entities(instance).unwrap(), Some(vec![]));
        assert!(world.scene_entity(instance, 0).unwrap().is_none());
    }

    #[test]
    fn direct_despawn_and_a_reused_slot_do_not_satisfy_lookup() {
        let mut world = World::new();
        let instance = world.create_scene_instance().unwrap();
        let entity = world.spawn((Name::new("member"),));
        assert_eq!(world.track_scene_entity(instance, entity).unwrap(), 0);
        world.despawn(entity).unwrap();
        assert!(world.scene_entity(instance, 0).unwrap().is_none());
        assert!(world.scene_membership(entity).unwrap().is_none());
        assert_eq!(world.scene_entities(instance).unwrap(), Some(vec![]));
        assert_eq!(world.sweep_scene_mappings().unwrap(), 1);
        assert_eq!(world.sweep_scene_mappings().unwrap(), 0);

        let reused = world.spawn((Name::new("reused"),));
        assert_ne!(reused, entity);
        assert!(world.scene_entity(instance, 0).unwrap().is_none());
        assert!(world.scene_membership(reused).unwrap().is_none());
        assert_eq!(world.track_scene_entity(instance, reused).unwrap(), 1);
        assert!(world.scene_entity(instance, 0).unwrap().is_none());
        assert_eq!(world.scene_entity(instance, 1).unwrap(), Some(reused));
    }

    #[test]
    fn a_missing_or_wrong_scene_member_does_not_resolve() {
        let mut world = World::new();
        let instance = world.create_scene_instance().unwrap();
        let other = world.create_scene_instance().unwrap();
        let entity = world.spawn(());
        world.track_scene_entity(instance, entity).unwrap();

        world.remove::<SceneMember>(entity).unwrap();
        assert!(world.scene_entity(instance, 0).unwrap().is_none());
        assert!(world.scene_membership(entity).unwrap().is_none());
        assert_eq!(world.sweep_scene_mappings().unwrap(), 1);

        world.track_scene_entity(instance, entity).unwrap();
        world.insert(entity, (SceneMember { instance: other, local: 0 },)).unwrap();
        assert!(world.scene_entity(instance, 0).unwrap().is_none());
        assert!(world.scene_entity(other, 0).unwrap().is_none());
        assert!(world.scene_membership(entity).unwrap().is_none());
        assert_eq!(world.sweep_scene_mappings().unwrap(), 1);
        assert!(world.has_resource::<SceneManager>());
    }

    #[test]
    fn an_ordinary_error_puts_the_manager_back() {
        let mut world = World::new();
        let instance = world.create_scene_instance().unwrap();
        let entity = world.spawn(());
        let dead = world.spawn(());
        world.despawn(dead).unwrap();

        assert_eq!(world.track_scene_entity(instance, dead).unwrap_err(), SceneError::NoSuchEntity { entity: dead });
        assert_eq!(
            world.track_scene_entity(SceneInstanceId(9), entity).unwrap_err(),
            SceneError::NoSuchInstance { instance: SceneInstanceId(9) }
        );
        world.track_scene_entity(instance, entity).unwrap();
        assert_eq!(
            world.track_scene_entity(instance, entity).unwrap_err(),
            SceneError::AlreadyMember { entity, instance }
        );

        assert!(world.has_resource::<SceneManager>());
        let next = world.create_scene_instance().unwrap();
        assert_eq!(next.raw(), 1);
        assert_eq!(world.scene_entity(instance, 0).unwrap(), Some(entity));
        assert_eq!(world.scene_instances().unwrap().len(), 2);
    }

    #[test]
    fn a_missing_manager_is_an_error_and_is_not_replaced() {
        let mut world = World::new();
        let manager = world.remove_resource::<SceneManager>().unwrap();
        let entity = world.spawn(());
        assert_eq!(world.create_scene_instance().unwrap_err(), SceneError::SceneManagerUnavailable);
        assert_eq!(world.scene_instances().unwrap_err(), SceneError::SceneManagerUnavailable);
        assert!(!world.has_resource::<SceneManager>());
        world.insert_resource(manager);
        let instance = world.create_scene_instance().unwrap();
        assert_eq!(world.track_scene_entity(instance, entity).unwrap(), 0);
        assert_eq!(instance.raw(), 0);
    }

    #[test]
    fn scene_member_is_registered_and_the_dump_lists_its_ids() {
        let mut world = World::new();
        register_world_components(&mut world.resource_mut()).unwrap();
        {
            let registry = world.resource::<ComponentRegistry>();
            let info = registry.get::<SceneMember>().unwrap();
            assert_eq!(info.name, "SceneMember");
            assert_eq!(
                info.fields.iter().map(|field| (field.name, field.type_name)).collect::<Vec<_>>(),
                vec![("instance", "SceneInstanceId"), ("local", "u32")]
            );
            assert_eq!(info.persistence, Persistence::Runtime);
            assert!(!info.script_visible && !info.script_writable);
        }
        let instance = world.create_scene_instance().unwrap();
        let entity = world.spawn(());
        world.track_scene_entity(instance, entity).unwrap();
        let dump = world.dump_entity(entity).unwrap();
        assert!(dump.contains("SceneMember\n"));
        assert!(dump.contains("persistence: runtime"));
        assert!(dump.contains(&member_line(0, 0)));
        assert!(!dump.contains("formatter returned"));
    }
}
