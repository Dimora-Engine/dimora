//! Scene instance identity and membership.
//!
//! The scene manager resource is inserted by [`World::new`] and is not part of the public API. Scene
//! operations on [`World`] take it out, do their work, and put it back before returning `Ok` or
//! `Err`. A panic during that work is not rolled back: the manager stays out of the world, and
//! entities created by the interrupted build stay. That is not a transaction.
//!
//! [`World::load_scene`] instantiates a code-defined scene through [`SceneBuilder`]. It does not
//! unload, replace, or enqueue a deferred scene command.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use hecs::{DynamicBundle, Entity};

use crate::hierarchy::{Children, HierarchyError, Parent};
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
/// instance. [`World::load_scene`] assigns it in spawn order. [`World::track_scene_entity`] assigns
/// it in track order. Both start at 0. It is not a persistent file id. A future stable id would be
/// a separate field.
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
    /// The scene closure stopped the load. Nothing from that load stays published.
    BuildRejected,
    /// `parent` was not created by the build that tried to use it.
    ParentOutsideBuild { parent: Entity },
    /// A created entity's `Parent` and `Children` links do not agree, or they leave the build.
    InvalidHierarchy { entity: Entity },
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
            SceneError::BuildRejected => f.write_str("scene build was rejected"),
            SceneError::ParentOutsideBuild { parent } => {
                write!(f, "parent {parent:?} was not created by this scene build")
            }
            SceneError::InvalidHierarchy { entity } => {
                write!(f, "scene hierarchy is inconsistent at {entity:?}")
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
    ///
    /// This publishes the instance immediately. [`World::load_scene`] is the way to instantiate a
    /// scene, and it publishes only after the build succeeds. This method stays public so a caller
    /// that already uses it keeps compiling.
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
    ///
    /// This can attach an entity that [`World::load_scene`] did not create. It stays public so a
    /// caller that already uses it keeps compiling.
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

    /// Instantiates a code-defined scene and returns its instance id.
    ///
    /// The closure builds entities through [`SceneBuilder`]. On `Ok`, each created entity receives
    /// [`SceneMember`] in spawn order and the instance becomes visible to lookup. On `Err`, every
    /// entity the builder created is despawned, parent links it added are removed, and the instance
    /// is not published. The instance id is still consumed.
    ///
    /// The same closure can be applied more than once. Each load gets its own entities and instance
    /// id. Scene-local ids start at 0 in each instance.
    ///
    /// The scene manager is out of the world while the closure runs. A nested `load_scene`,
    /// `create_scene_instance`, or `track_scene_entity` returns
    /// [`SceneError::SceneManagerUnavailable`] and does not insert another manager. This call puts
    /// the original manager back on both `Ok` and `Err`. A panic inside the closure does not.
    pub fn load_scene(
        &mut self,
        build: impl FnOnce(&mut SceneBuilder<'_>) -> Result<(), SceneError>,
    ) -> Result<SceneInstanceId, SceneError> {
        let Some(mut manager) = self.remove_resource::<SceneManager>() else {
            return Err(SceneError::SceneManagerUnavailable);
        };
        let result = load_with_manager(self, &mut manager, build);
        self.insert_resource(manager);
        result
    }

    fn member_matches(&self, instance: SceneInstanceId, local: u32, entity: Entity) -> bool {
        self.contains(entity) && component_matches(self, instance, local, entity)
    }
}

/// Creates entities for one [`World::load_scene`] call.
///
/// The closure receives this type and not `&mut World`. It can spawn entities and parent them
/// under entities this build created. It cannot insert or remove assets, change resources, despawn
/// an existing entity, parent under an entity from outside the build, or enqueue commands.
pub struct SceneBuilder<'a> {
    world: &'a mut World,
    created: Vec<Entity>,
    /// Parents whose `Children` component this build inserted.
    introduced_children: Vec<Entity>,
}

impl SceneBuilder<'_> {
    /// Spawns `bundle` and records it as part of this scene. The returned id is this build's
    /// runtime entity. Its scene-local id is assigned when the load succeeds.
    pub fn spawn(&mut self, bundle: impl DynamicBundle) -> Entity {
        let entity = self.world.spawn(bundle);
        self.created.push(entity);
        entity
    }

    /// Spawns `bundle` as a child of `parent`.
    ///
    /// `parent` must already have been created by this build. An entity that existed before the
    /// load, or one created by another instance, is [`SceneError::ParentOutsideBuild`]. No link
    /// is written in that case.
    pub fn spawn_child(&mut self, parent: Entity, bundle: impl DynamicBundle) -> Result<Entity, SceneError> {
        if !self.created.contains(&parent) {
            return Err(SceneError::ParentOutsideBuild { parent });
        }
        let had_children = self.world.component::<Children>(parent).is_ok();
        let child = self.spawn(bundle);
        if let Err(error) = self.world.set_parent(child, parent) {
            let _ = self.world.despawn(child);
            self.created.pop();
            return Err(hierarchy_error(parent, child, error));
        }
        if !had_children {
            self.introduced_children.push(parent);
        }
        Ok(child)
    }
}

fn hierarchy_error(parent: Entity, child: Entity, error: HierarchyError) -> SceneError {
    match error {
        HierarchyError::NoSuchEntity => SceneError::NoSuchEntity { entity: parent },
        HierarchyError::Cycle => SceneError::InvalidHierarchy { entity: child },
    }
}

fn load_with_manager(
    world: &mut World,
    manager: &mut SceneManager,
    build: impl FnOnce(&mut SceneBuilder<'_>) -> Result<(), SceneError>,
) -> Result<SceneInstanceId, SceneError> {
    let id = manager.allocate()?;
    let mut builder = SceneBuilder { world, created: Vec::new(), introduced_children: Vec::new() };
    let built = build(&mut builder);
    let created = std::mem::take(&mut builder.created);
    let introduced = std::mem::take(&mut builder.introduced_children);
    drop(builder);

    if let Err(error) = built {
        rollback_build(world, &created, &introduced);
        return Err(error);
    }
    if let Err(error) = validate_build(world, &created) {
        rollback_build(world, &created, &introduced);
        return Err(error);
    }
    if let Err(error) = publish_instance(world, manager, id, &created) {
        rollback_build(world, &created, &introduced);
        return Err(error);
    }
    Ok(id)
}

fn validate_build(world: &World, created: &[Entity]) -> Result<(), SceneError> {
    let owned: HashSet<Entity> = created.iter().copied().collect();
    for &entity in created {
        if !world.contains(entity) {
            return Err(SceneError::NoSuchEntity { entity });
        }
        if let Some(instance) = world.component::<SceneMember>(entity).ok().map(|member| member.instance) {
            return Err(SceneError::AlreadyMember { entity, instance });
        }
        if let Some(parent) = world.component::<Parent>(entity).ok().map(|parent| parent.0) {
            if !owned.contains(&parent) || !world.contains(parent) {
                return Err(SceneError::ParentOutsideBuild { parent });
            }
            let listed = world.component::<Children>(parent).ok().is_some_and(|children| children.0.contains(&entity));
            if !listed {
                return Err(SceneError::InvalidHierarchy { entity });
            }
        }
        let child_ids = world.component::<Children>(entity).ok().map(|children| children.0.clone());
        if let Some(child_ids) = child_ids {
            if child_ids.is_empty() {
                return Err(SceneError::InvalidHierarchy { entity });
            }
            for child in child_ids {
                if !owned.contains(&child) || !world.contains(child) {
                    return Err(SceneError::InvalidHierarchy { entity });
                }
                let points_here = world.component::<Parent>(child).ok().is_some_and(|parent| parent.0 == entity);
                if !points_here {
                    return Err(SceneError::InvalidHierarchy { entity });
                }
            }
        }
    }
    Ok(())
}

fn publish_instance(
    world: &mut World,
    manager: &mut SceneManager,
    id: SceneInstanceId,
    created: &[Entity],
) -> Result<(), SceneError> {
    let mut locals = BTreeMap::new();
    for (index, &entity) in created.iter().enumerate() {
        let local = u32::try_from(index).map_err(|_| SceneError::LocalIdsExhausted { instance: id })?;
        if !world.contains(entity) {
            return Err(SceneError::NoSuchEntity { entity });
        }
        if let Ok(member) = world.component::<SceneMember>(entity) {
            return Err(SceneError::AlreadyMember { entity, instance: member.instance });
        }
        world
            .insert(entity, (SceneMember { instance: id, local },))
            .map_err(|_| SceneError::NoSuchEntity { entity })?;
        locals.insert(local, entity);
    }
    let next_local = u32::try_from(created.len()).map_err(|_| SceneError::LocalIdsExhausted { instance: id })?;
    manager.instances.insert(id, InstanceRecord { next_local, locals });
    Ok(())
}

fn rollback_build(world: &mut World, created: &[Entity], introduced_children: &[Entity]) {
    for &entity in created.iter().rev() {
        if world.contains(entity) {
            let _ = world.despawn(entity);
        }
    }
    for &parent in introduced_children {
        if !world.contains(parent) {
            continue;
        }
        let empty = world.component::<Children>(parent).ok().is_some_and(|children| children.0.is_empty());
        if empty {
            let _ = world.remove::<Children>(parent);
        }
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
    use crate::hierarchy::{Children, Parent};
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Marker(u8);

    fn assert_hierarchy_closed(world: &World) {
        let parents: Vec<(Entity, Entity)> =
            world.query::<&Parent>().iter().map(|(entity, parent)| (entity, parent.0)).collect();
        let children: Vec<(Entity, Vec<Entity>)> =
            world.query::<&Children>().iter().map(|(entity, children)| (entity, children.0.clone())).collect();
        for (entity, parent) in parents {
            assert!(world.contains(parent), "dangling parent on {entity:?}");
            let listed = world.component::<Children>(parent).ok().is_some_and(|kids| kids.0.contains(&entity));
            assert!(listed, "{entity:?} is missing from its parent");
        }
        for (entity, kids) in children {
            assert!(!kids.is_empty(), "empty Children on {entity:?}");
            for child in kids {
                assert!(world.contains(child), "dangling child of {entity:?}");
                assert_eq!(world.component::<Parent>(child).unwrap().0, entity);
            }
        }
    }

    fn sample_tree(builder: &mut SceneBuilder<'_>) -> Result<Entity, SceneError> {
        let root = builder.spawn((Name::new("root"), Transform::IDENTITY));
        let child = builder.spawn_child(root, (Name::new("child"), Transform::from_xyz(1.0, 0.0, 0.0)))?;
        builder.spawn_child(child, (Name::new("leaf"), Transform::from_xyz(0.0, 2.0, 0.0)))?;
        Ok(root)
    }

    #[test]
    fn an_empty_load_publishes_an_instance_and_a_rejected_load_does_not() {
        let mut world = World::new();
        let empty = world.load_scene(|_| Ok(())).unwrap();
        assert_eq!(empty.raw(), 0);
        assert_eq!(world.scene_instances().unwrap(), vec![empty]);
        assert_eq!(world.scene_entities(empty).unwrap(), Some(vec![]));
        assert_eq!(world.entities.len(), 0);

        let rejected = world.load_scene(|_| Err(SceneError::BuildRejected)).unwrap_err();
        assert_eq!(rejected, SceneError::BuildRejected);
        assert_eq!(world.scene_instances().unwrap(), vec![empty]);
        assert!(world.scene_entities(SceneInstanceId(1)).unwrap().is_none());
        assert!(world.has_resource::<SceneManager>());
        let next = world.load_scene(|_| Ok(())).unwrap();
        assert_eq!(next.raw(), 2);
    }

    #[test]
    fn one_entity_gets_member_zero_and_lookup_finds_it() {
        let mut world = World::new();
        let instance = world
            .load_scene(|builder| {
                builder.spawn((Name::new("only"), Transform::from_xyz(3.0, 0.0, 0.0)));
                Ok(())
            })
            .unwrap();
        let members = world.scene_entities(instance).unwrap().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].0, 0);
        assert_eq!(world.scene_entity(instance, 0).unwrap(), Some(members[0].1));
        assert_eq!(world.scene_membership(members[0].1).unwrap(), Some(SceneMember { instance, local: 0 }));
        assert_eq!(world.component::<Name>(members[0].1).unwrap().0, "only");
    }

    #[test]
    fn a_parent_and_a_deep_chain_keep_spawn_order_and_links() {
        let mut world = World::new();
        let instance = world
            .load_scene(|builder| {
                let root = builder.spawn((Name::new("root"),));
                builder.spawn_child(root, (Name::new("left"),))?;
                builder.spawn_child(root, (Name::new("right"),))?;
                let _ = sample_tree(builder)?;
                Ok(())
            })
            .unwrap();
        let members = world.scene_entities(instance).unwrap().unwrap();
        let locals: Vec<u32> = members.iter().map(|(local, _)| *local).collect();
        assert_eq!(locals, vec![0, 1, 2, 3, 4, 5]);
        let root = members[0].1;
        let names: Vec<String> =
            members.iter().map(|(_, entity)| world.component::<Name>(*entity).unwrap().0.clone()).collect();
        assert_eq!(names, vec!["root", "left", "right", "root", "child", "leaf"]);
        let kids = world.component::<Children>(root).unwrap().0.clone();
        assert_eq!(kids.len(), 2);
        assert_eq!(world.component::<Parent>(kids[0]).unwrap().0, root);
        assert_eq!(world.component::<Parent>(kids[1]).unwrap().0, root);
        let leaf = members[5].1;
        let child = world.component::<Parent>(leaf).unwrap().0;
        let deep_root = world.component::<Parent>(child).unwrap().0;
        assert_eq!(deep_root, members[3].1);
        assert!(world.component::<Parent>(deep_root).is_err());
        assert_hierarchy_closed(&world);
    }

    #[test]
    fn two_loads_repeat_local_ids_on_distinct_entities() {
        fn load(builder: &mut SceneBuilder<'_>) -> Result<(), SceneError> {
            let root = builder.spawn((Name::new("root"),));
            builder.spawn_child(root, (Name::new("child"),))?;
            Ok(())
        }
        let mut world = World::new();
        let first = world.load_scene(load).unwrap();
        let second = world.load_scene(load).unwrap();
        assert_ne!(first, second);
        let a = world.scene_entities(first).unwrap().unwrap();
        let b = world.scene_entities(second).unwrap().unwrap();
        assert_eq!(a.iter().map(|(local, _)| *local).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(b.iter().map(|(local, _)| *local).collect::<Vec<_>>(), vec![0, 1]);
        assert_ne!(a[0].1, b[0].1);
        assert_ne!(a[1].1, b[1].1);
        assert_eq!(world.scene_entity(first, 0).unwrap(), Some(a[0].1));
        assert_eq!(world.scene_entity(second, 0).unwrap(), Some(b[0].1));
    }

    #[test]
    fn a_failed_spawn_rolls_back_entities_links_resources_and_the_id() {
        let mut world = World::new();
        world.insert_resource(Marker(4));
        let bystander = world.spawn((Name::new("keep"), Transform::from_xyz(8.0, 0.0, 0.0)));
        let child = world.spawn_child(bystander, (Name::new("kept-child"), Transform::IDENTITY));
        let before = world.entities.len();

        let after_one = world
            .load_scene(|builder| {
                builder.spawn((Name::new("temp"),));
                Err(SceneError::BuildRejected)
            })
            .unwrap_err();
        assert_eq!(after_one, SceneError::BuildRejected);
        assert_eq!(world.entities.len(), before);
        assert!(world.scene_instances().unwrap().is_empty());
        assert!(world.has_resource::<SceneManager>());
        assert_eq!(*world.resource::<Marker>(), Marker(4));
        assert_eq!(world.component::<Name>(bystander).unwrap().0, "keep");
        assert_eq!(world.component::<Children>(bystander).unwrap().0, vec![child]);
        assert_hierarchy_closed(&world);

        let nested = world
            .load_scene(|builder| {
                let _ = sample_tree(builder)?;
                Err(SceneError::BuildRejected)
            })
            .unwrap_err();
        assert_eq!(nested, SceneError::BuildRejected);
        assert_eq!(world.entities.len(), before);
        assert!(world.query::<&Parent>().iter().all(|(entity, _)| entity == child));
        assert_hierarchy_closed(&world);
        assert!(world.scene_entities(SceneInstanceId(0)).unwrap().is_none());
        assert!(world.scene_entities(SceneInstanceId(1)).unwrap().is_none());

        let published = world.load_scene(|_| Ok(())).unwrap();
        assert_eq!(published.raw(), 2);
        assert_eq!(*world.resource::<Marker>(), Marker(4));
        assert_eq!(
            world.component::<Transform>(bystander).unwrap().to_matrix(),
            Transform::from_xyz(8.0, 0.0, 0.0).to_matrix()
        );
    }

    #[test]
    fn parenting_outside_the_build_is_rejected_without_touching_that_entity() {
        let mut world = World::new();
        let existing = world.spawn((Name::new("runtime"), Transform::IDENTITY));
        let err = world
            .load_scene(|builder| {
                builder.spawn((Name::new("inside"),));
                builder.spawn_child(existing, (Name::new("nope"), Transform::IDENTITY))?;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(err, SceneError::ParentOutsideBuild { parent: existing });
        assert_eq!(world.entities.len(), 1);
        assert!(world.contains(existing));
        assert!(world.component::<Children>(existing).is_err());
        assert!(world.component::<SceneMember>(existing).is_err());
        assert!(world.scene_instances().unwrap().is_empty());
        assert!(world.has_resource::<SceneManager>());
        assert_hierarchy_closed(&world);

        let sneaky = world
            .load_scene(|builder| {
                builder.spawn((Name::new("bundle-parent"), Parent(existing)));
                Ok(())
            })
            .unwrap_err();
        assert_eq!(sneaky, SceneError::ParentOutsideBuild { parent: existing });
        assert!(world.component::<Children>(existing).is_err());
        assert_eq!(world.entities.len(), 1);
        assert!(world.scene_instances().unwrap().is_empty());
    }

    #[test]
    fn rollback_removes_empty_children_introduced_on_a_survivor() {
        let mut world = World::new();
        let parent = world.spawn((Name::new("keep"),));
        let child = world.spawn((Name::new("temp"),));
        world.set_parent(child, parent).unwrap();
        assert!(!world.component::<Children>(parent).unwrap().0.is_empty());
        rollback_build(&mut world, &[child], &[parent]);
        assert!(world.contains(parent));
        assert!(!world.contains(child));
        assert!(world.component::<Children>(parent).is_err());
        assert_hierarchy_closed(&world);
    }

    #[test]
    fn a_nested_load_does_not_replace_the_manager_or_publish() {
        let mut world = World::new();
        let err = world
            .load_scene(|builder| {
                builder.spawn((Name::new("outer"),));
                let nested = builder.world.load_scene(|inner| {
                    inner.spawn((Name::new("inner"),));
                    Ok(())
                });
                assert_eq!(nested, Err(SceneError::SceneManagerUnavailable));
                assert!(!builder.world.has_resource::<SceneManager>());
                Err(SceneError::BuildRejected)
            })
            .unwrap_err();
        assert_eq!(err, SceneError::BuildRejected);
        assert!(world.has_resource::<SceneManager>());
        assert!(world.scene_instances().unwrap().is_empty());
        assert_eq!(world.entities.len(), 0);
        assert_eq!(world.load_scene(|_| Ok(())).unwrap().raw(), 1);
    }

    #[test]
    fn a_missing_manager_rejects_load_without_inserting_one() {
        let mut world = World::new();
        let manager = world.remove_resource::<SceneManager>().unwrap();
        assert_eq!(world.load_scene(|_| Ok(())).unwrap_err(), SceneError::SceneManagerUnavailable);
        assert!(!world.has_resource::<SceneManager>());
        world.insert_resource(manager);
        assert_eq!(world.load_scene(|_| Ok(())).unwrap().raw(), 0);
    }
}
