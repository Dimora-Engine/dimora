//! Scene instance identity and membership.
//!
//! The scene manager resource is inserted by [`World::new`] and is not part of the public API. Scene
//! operations on [`World`] take it out, do their work, and put it back before returning `Ok` or
//! `Err`. A panic during that work is not rolled back: the manager stays out of the world, and
//! entities created by the interrupted build stay. That is not a transaction.
//!
//! [`World::load_scene`] instantiates a code-defined scene through [`SceneBuilder`].
//! [`World::unload_scene`] and [`World::replace_scene`] remove an instance synchronously.
//! [`CommandWriter`](crate::CommandWriter) queues those same three operations. The queue runs them
//! at the next flush. Each queued call returns a [`SceneOpId`] immediately. The outcome stays on
//! the scene manager until the caller dismisses it. A scene failure is not a [`CommandError`](crate::CommandError).

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

/// Runtime id of one deferred scene operation.
///
/// Issued in increasing order, starting at 0, when the operation is queued. An id is never issued
/// twice. It is not a [`SceneInstanceId`] and it is not a file id. The operation stays
/// [`SceneOpStatus::Pending`] until the command queue runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneOpId(u64);

impl SceneOpId {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SceneOpId {
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
    /// `entity` carries [`SceneMember`] for an instance, and the instance map does not name that entity.
    ///
    /// Engine-maintained loads do not produce this. Unload and replace refuse it and change nothing.
    InconsistentMembership { entity: Entity },
    /// Lifting `entity` out of a scene would store a non-finite transform. Nothing was removed.
    TransformUnrepresentable { entity: Entity },
    /// `operation` was never queued, or its result was already dismissed.
    NoSuchOperation { operation: SceneOpId },
    /// `operation` is still queued. Its result cannot be dismissed yet.
    OperationPending { operation: SceneOpId },
    /// Every `u64` operation id has been issued.
    OperationIdsExhausted,
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
            SceneError::InconsistentMembership { entity } => {
                write!(f, "scene membership for {entity:?} does not match the instance map")
            }
            SceneError::TransformUnrepresentable { entity } => {
                write!(f, "lifting {entity:?} would store a non-finite transform")
            }
            SceneError::NoSuchOperation { operation } => write!(f, "scene operation {operation} does not exist"),
            SceneError::OperationPending { operation } => write!(f, "scene operation {operation} has not run yet"),
            SceneError::OperationIdsExhausted => f.write_str("every scene operation id has been issued"),
        }
    }
}

/// Outcome of one deferred scene operation.
///
/// [`World::scene_operation`] copies this out and leaves it stored. [`World::dismiss_scene_operation`]
/// removes one completed outcome. [`World::clear_scene_operations`] removes every completed outcome
/// and leaves pending ones. Completed outcomes stay until one of those calls. The frame loop does
/// not drop them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneOpStatus {
    /// Queued. The flush has not run this operation.
    Pending,
    /// [`World::load_scene`] published `instance`.
    Loaded { instance: SceneInstanceId },
    /// [`World::unload_scene`] removed `instance`.
    Unloaded { instance: SceneInstanceId },
    /// [`World::replace_scene`] removed `old` and published `new`.
    Replaced { old: SceneInstanceId, new: SceneInstanceId },
    /// The synchronous operation returned `error`. A failed load or replace published nothing from that attempt.
    Failed { error: SceneError },
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
    next_operation: u64,
    operations: BTreeMap<SceneOpId, SceneOpStatus>,
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

    fn allocate_operation(&mut self) -> Result<SceneOpId, SceneError> {
        let Some(next) = self.next_operation.checked_add(1) else {
            return Err(SceneError::OperationIdsExhausted);
        };
        let id = SceneOpId(self.next_operation);
        self.next_operation = next;
        self.operations.insert(id, SceneOpStatus::Pending);
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

    /// Records a pending deferred scene operation and returns its id.
    ///
    /// The id is consumed even though the operation has not run. A missing or borrowed manager
    /// returns [`SceneError::SceneManagerUnavailable`] and does not issue an id.
    pub(crate) fn begin_scene_operation(&self) -> Result<SceneOpId, SceneError> {
        let mut manager = match self.try_resource_mut::<SceneManager>() {
            Ok(Some(manager)) => manager,
            Ok(None) | Err(()) => return Err(SceneError::SceneManagerUnavailable),
        };
        manager.allocate_operation()
    }

    /// Stores the outcome of a deferred operation that was [`SceneOpStatus::Pending`].
    ///
    /// A missing record is left alone. Dismissing a pending operation is refused, so the record
    /// is still here when the flush runs.
    pub(crate) fn complete_scene_op(&self, id: SceneOpId, status: SceneOpStatus) {
        let Ok(Some(mut manager)) = self.try_resource_mut::<SceneManager>() else {
            return;
        };
        if let Some(slot) = manager.operations.get_mut(&id) {
            if matches!(*slot, SceneOpStatus::Pending) {
                if let SceneOpStatus::Failed { error } = status {
                    log::error!("scene operation {id} failed: {error}");
                }
                *slot = status;
            }
        }
    }

    /// The stored outcome of `operation`.
    ///
    /// This copies the status and leaves it stored. [`SceneError::NoSuchOperation`] means the id
    /// was never issued or the outcome was dismissed. A queued operation that has not flushed is
    /// [`SceneOpStatus::Pending`].
    pub fn scene_operation(&self, operation: SceneOpId) -> Result<SceneOpStatus, SceneError> {
        self.scene_manager()?.operations.get(&operation).copied().ok_or(SceneError::NoSuchOperation { operation })
    }

    /// Removes one completed outcome and returns it.
    ///
    /// A pending operation stays queued and returns [`SceneError::OperationPending`].
    pub fn dismiss_scene_operation(&mut self, operation: SceneOpId) -> Result<SceneOpStatus, SceneError> {
        self.with_scene_manager(|manager, _| match manager.operations.get(&operation).copied() {
            None => Err(SceneError::NoSuchOperation { operation }),
            Some(SceneOpStatus::Pending) => Err(SceneError::OperationPending { operation }),
            Some(status) => {
                manager.operations.remove(&operation);
                Ok(status)
            }
        })
    }

    /// Removes every completed outcome. Pending operations stay. Returns how many were removed.
    ///
    /// Completed results accumulate until this call or [`World::dismiss_scene_operation`]. The
    /// frame loop does not clear them.
    pub fn clear_scene_operations(&mut self) -> Result<usize, SceneError> {
        self.with_scene_manager(|manager, _| {
            let done: Vec<SceneOpId> = manager
                .operations
                .iter()
                .filter(|(_, status)| !matches!(status, SceneOpStatus::Pending))
                .map(|(&id, _)| id)
                .collect();
            let removed = done.len();
            for id in done {
                manager.operations.remove(&id);
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

    /// Removes `instance` and the entities that still belong to it.
    ///
    /// A non-member whose parent belongs to `instance` is detached first. Its local [`crate::Transform`]
    /// becomes the world transform from [`crate::Transform::from_matrix`], which drops shear. Other scene
    /// instances, runtime entities, and assets stay. An unknown instance returns
    /// [`SceneError::NoSuchInstance`] and changes nothing. An empty instance succeeds.
    ///
    /// Stale map rows (a dead entity, or a live entity whose [`SceneMember`] does not match) are
    /// dropped with the instance and are not despawned. A live entity that claims this instance
    /// while the map names someone else is [`SceneError::InconsistentMembership`], and nothing is
    /// removed. A broken parent or child link, a parent cycle, or a non-finite lifted transform
    /// is the same: the world is unchanged.
    ///
    /// [`World::despawn`] is not used, so a non-member descendant is not destroyed with a member.
    /// A panic after this method has started changing entities is not undone.
    pub fn unload_scene(&mut self, instance: SceneInstanceId) -> Result<(), SceneError> {
        let Some(mut manager) = self.remove_resource::<SceneManager>() else {
            return Err(SceneError::SceneManagerUnavailable);
        };
        let result = unload_with_manager(self, &mut manager, instance);
        self.insert_resource(manager);
        result
    }

    /// Builds a new instance, then unloads `instance` if that build succeeds.
    ///
    /// The original instance is checked before the build. A build error rolls the new entities
    /// back and leaves `instance` published. The new id is still consumed. On success the old id
    /// no longer resolves and the returned id is the replacement. Assets are not removed.
    ///
    /// [`SceneBuilder`] cannot change an entity it did not create. A [`SceneMember`], an outside
    /// parent, or a child link that leaves the build fails validation and rolls the new entities
    /// back before the new instance is published, so the preflighted instance is unchanged when
    /// unload runs. That unload therefore completes for engine-maintained state. The guard that
    /// unloads a published replacement if the original unload returns an error is not reached
    /// through this method. A panic is not a transaction, and this method does not undo a lift
    /// that already happened.
    pub fn replace_scene(
        &mut self,
        instance: SceneInstanceId,
        build: impl FnOnce(&mut SceneBuilder<'_>) -> Result<(), SceneError>,
    ) -> Result<SceneInstanceId, SceneError> {
        let Some(mut manager) = self.remove_resource::<SceneManager>() else {
            return Err(SceneError::SceneManagerUnavailable);
        };
        let result = replace_with_manager(self, &mut manager, instance, build);
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

fn replace_with_manager(
    world: &mut World,
    manager: &mut SceneManager,
    instance: SceneInstanceId,
    build: impl FnOnce(&mut SceneBuilder<'_>) -> Result<(), SceneError>,
) -> Result<SceneInstanceId, SceneError> {
    if !manager.instances.contains_key(&instance) {
        return Err(SceneError::NoSuchInstance { instance });
    }
    preflight_unload(world, manager, instance)?;
    let new_id = load_with_manager(world, manager, build)?;
    // Unreachable after the preflight above and a builder that only creates new entities:
    // validate_build rejects membership and links that would make this unload fail, and it
    // does so before publish. Kept so a later change cannot leave the replacement published
    // beside the original. A partial lift is not rolled back.
    if let Err(error) = unload_with_manager(world, manager, instance) {
        let _ = unload_with_manager(world, manager, new_id);
        return Err(error);
    }
    Ok(new_id)
}

fn unload_with_manager(
    world: &mut World,
    manager: &mut SceneManager,
    instance: SceneInstanceId,
) -> Result<(), SceneError> {
    let members = preflight_unload(world, manager, instance)?;
    let owned: HashSet<Entity> = members.iter().copied().collect();
    let mut lift = Vec::new();
    for &member in &members {
        let children = world.component::<Children>(member).ok().map(|children| children.0.clone()).unwrap_or_default();
        for child in children {
            if !owned.contains(&child) {
                lift.push(child);
            }
        }
    }
    for child in lift {
        world.lift_to_world(child).map_err(|_| SceneError::NoSuchEntity { entity: child })?;
    }
    for member in members {
        world.despawn_one(member);
    }
    manager.instances.remove(&instance);
    Ok(())
}

fn preflight_unload(
    world: &World,
    manager: &SceneManager,
    instance: SceneInstanceId,
) -> Result<Vec<Entity>, SceneError> {
    let Some(record) = manager.instances.get(&instance) else {
        return Err(SceneError::NoSuchInstance { instance });
    };
    let mut members = Vec::new();
    let mut owned = HashSet::new();
    for (&local, &entity) in &record.locals {
        if entity_matches(&world.entities, instance, local, entity) {
            members.push(entity);
            owned.insert(entity);
        }
    }
    let claimants: Vec<(Entity, u32)> = world
        .query::<&SceneMember>()
        .iter()
        .filter(|(_, member)| member.instance == instance)
        .map(|(entity, member)| (entity, member.local))
        .collect();
    for (entity, local) in claimants {
        if record.locals.get(&local).copied() != Some(entity) || !owned.contains(&entity) {
            return Err(SceneError::InconsistentMembership { entity });
        }
    }
    for &entity in &members {
        check_links(world, entity)?;
        check_chain(world, entity)?;
    }
    let parent_links: Vec<(Entity, Entity)> =
        world.query::<&Parent>().iter().map(|(entity, parent)| (entity, parent.0)).collect();
    for (entity, parent) in parent_links {
        if !owned.contains(&parent) {
            continue;
        }
        let listed = world.component::<Children>(parent).ok().is_some_and(|children| children.0.contains(&entity));
        if !listed {
            return Err(SceneError::InvalidHierarchy { entity });
        }
    }
    for &member in &members {
        let children = world.component::<Children>(member).ok().map(|children| children.0.clone()).unwrap_or_default();
        for child in children {
            if owned.contains(&child) {
                continue;
            }
            check_links(world, child)?;
            check_chain(world, child)?;
            if let Some(transform) = world.preview_lift(child) {
                if !transform_is_finite(&transform) {
                    return Err(SceneError::TransformUnrepresentable { entity: child });
                }
            }
        }
    }
    Ok(members)
}

fn check_links(world: &World, entity: Entity) -> Result<(), SceneError> {
    if !world.contains(entity) {
        return Err(SceneError::InvalidHierarchy { entity });
    }
    if let Some(parent) = world.component::<Parent>(entity).ok().map(|parent| parent.0) {
        if !world.contains(parent) {
            return Err(SceneError::InvalidHierarchy { entity });
        }
        let listed = world.component::<Children>(parent).ok().is_some_and(|children| children.0.contains(&entity));
        if !listed {
            return Err(SceneError::InvalidHierarchy { entity });
        }
    }
    let child_ids = world.component::<Children>(entity).ok().map(|children| children.0.clone());
    if let Some(child_ids) = child_ids {
        for child in child_ids {
            if !world.contains(child) {
                return Err(SceneError::InvalidHierarchy { entity });
            }
            let points_here = world.component::<Parent>(child).ok().is_some_and(|parent| parent.0 == entity);
            if !points_here {
                return Err(SceneError::InvalidHierarchy { entity });
            }
        }
    }
    Ok(())
}

fn check_chain(world: &World, entity: Entity) -> Result<(), SceneError> {
    let mut seen = HashSet::new();
    let mut current = Some(entity);
    while let Some(entity_id) = current {
        if !seen.insert(entity_id) || !world.contains(entity_id) {
            return Err(SceneError::InvalidHierarchy { entity });
        }
        current = world.component::<Parent>(entity_id).ok().map(|parent| parent.0);
    }
    Ok(())
}

fn transform_is_finite(transform: &crate::Transform) -> bool {
    let position = transform.position;
    let scale = transform.scale;
    let rotation = transform.rotation;
    [position.x, position.y, position.z, scale.x, scale.y, scale.z, rotation.x, rotation.y, rotation.z, rotation.w]
        .iter()
        .all(|component| component.is_finite())
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
    use crate::hierarchy::{Children, GlobalTransform, Parent};
    use crate::registry::{ComponentRegistry, Persistence};
    use crate::{Interpolate, Name, Transform};
    use engine_math::{Mat4, Quat, Vec3};

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

    fn one_entity(world: &mut World, name: &str) -> (SceneInstanceId, Entity) {
        let instance = world
            .load_scene(|builder| {
                builder.spawn((Name::new(name), Transform::IDENTITY));
                Ok(())
            })
            .unwrap();
        let entity = world.scene_entity(instance, 0).unwrap().unwrap();
        (instance, entity)
    }

    #[test]
    fn unloading_removes_only_that_instance() {
        let mut world = World::new();
        let empty = world.load_scene(|_| Ok(())).unwrap();
        world.unload_scene(empty).unwrap();
        assert!(world.scene_instances().unwrap().is_empty());
        assert_eq!(world.unload_scene(empty).unwrap_err(), SceneError::NoSuchInstance { instance: empty });
        assert_eq!(world.entities.len(), 0);

        let (first, first_entity) = one_entity(&mut world, "first");
        let (second, second_entity) = one_entity(&mut world, "second");
        let runtime = world.spawn((Name::new("runtime"),));
        world.unload_scene(first).unwrap();
        assert!(!world.contains(first_entity));
        assert!(world.scene_entity(first, 0).unwrap().is_none());
        assert!(world.scene_instances().unwrap() == vec![second]);
        assert_eq!(world.scene_entity(second, 0).unwrap(), Some(second_entity));
        assert!(world.contains(runtime));
        assert_eq!(world.unload_scene(first).unwrap_err(), SceneError::NoSuchInstance { instance: first });
        assert!(world.contains(second_entity));
        assert_hierarchy_closed(&world);
    }

    #[test]
    fn unloading_a_hierarchy_keeps_other_owners_and_preserves_world_transforms() {
        let mut world = World::new();
        let parent_local =
            Transform::from_xyz(10.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(0.4)).with_scale(Vec3::splat(2.0));
        let child_local = Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(Quat::from_rotation_z(0.3));
        let instance = world
            .load_scene(|builder| {
                let root = builder.spawn((Name::new("root"), parent_local));
                builder.spawn_child(root, (Name::new("sibling-a"), Transform::IDENTITY))?;
                builder.spawn_child(root, (Name::new("sibling-b"), Transform::IDENTITY))?;
                let nested = builder.spawn_child(root, (Name::new("nested"), Transform::IDENTITY))?;
                builder.spawn_child(nested, (Name::new("leaf"), Transform::IDENTITY))?;
                Ok(())
            })
            .unwrap();
        let root = world.scene_entity(instance, 0).unwrap().unwrap();
        let runtime = world.spawn((child_local, Interpolate { previous: Some(Transform::from_xyz(4.0, 0.0, 0.0)) }));
        world.set_parent(runtime, root).unwrap();
        let grandchild_local = Transform::from_xyz(0.0, 1.0, 0.0);
        let grandchild_previous = Transform::from_xyz(0.0, 3.0, 0.0);
        let grandchild = world.spawn((grandchild_local, Interpolate { previous: Some(grandchild_previous) }));
        world.set_parent(grandchild, runtime).unwrap();
        let bystander = world.spawn((
            Transform::from_xyz(5.0, 0.0, 0.0),
            GlobalTransform { matrix: Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)) },
        ));
        let plain = world.spawn((Name::new("plain"),));
        let expected = parent_local.mul_transform(&child_local);

        world.unload_scene(instance).unwrap();
        assert_eq!(world.entities.len(), 4);
        assert!(world.component::<Parent>(runtime).is_err());
        assert!(world.component::<Transform>(runtime).unwrap().to_matrix().approx_eq(&expected.to_matrix(), 1e-4));
        assert_eq!(
            world.component::<Interpolate>(runtime).unwrap().previous,
            Some(*world.component::<Transform>(runtime).unwrap())
        );
        assert_eq!(world.component::<Parent>(grandchild).unwrap().0, runtime);
        assert_eq!(*world.component::<Transform>(grandchild).unwrap(), grandchild_local);
        assert_eq!(world.component::<Interpolate>(grandchild).unwrap().previous, Some(grandchild_previous));
        assert_eq!(
            world.component::<GlobalTransform>(bystander).unwrap().matrix,
            Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0))
        );
        assert!(world.component::<GlobalTransform>(plain).is_err());
        assert!(world.component::<GlobalTransform>(runtime).is_err());
        assert_hierarchy_closed(&world);
        assert!(world.scene_instances().unwrap().is_empty());
    }

    #[test]
    fn unloading_stops_at_every_ownership_boundary() {
        let mut world = World::new();
        let (scene_a, member_a) = one_entity(&mut world, "a");
        let (scene_b, member_b) = one_entity(&mut world, "b");
        let runtime = world.spawn((Name::new("runtime"), Transform::from_xyz(2.0, 0.0, 0.0)));
        let deep_runtime = world.spawn((Name::new("deep"), Transform::IDENTITY));
        world.set_parent(runtime, member_a).unwrap();
        world.set_parent(member_b, runtime).unwrap();
        world.set_parent(deep_runtime, member_b).unwrap();
        let kept_local = *world.component::<Transform>(deep_runtime).unwrap();

        world.unload_scene(scene_a).unwrap();
        assert!(!world.contains(member_a));
        assert!(world.contains(runtime));
        assert!(world.component::<Parent>(runtime).is_err());
        assert_eq!(world.component::<Parent>(member_b).unwrap().0, runtime);
        assert_eq!(world.scene_entity(scene_b, 0).unwrap(), Some(member_b));
        assert_eq!(world.component::<Parent>(deep_runtime).unwrap().0, member_b);
        assert_eq!(*world.component::<Transform>(deep_runtime).unwrap(), kept_local);

        let world_before = world.preview_lift(deep_runtime).unwrap();
        world.unload_scene(scene_b).unwrap();
        assert!(!world.contains(member_b));
        assert!(world.contains(runtime));
        assert!(world.component::<Children>(runtime).is_err());
        assert!(world.component::<Parent>(deep_runtime).is_err());
        assert!(world
            .component::<Transform>(deep_runtime)
            .unwrap()
            .to_matrix()
            .approx_eq(&world_before.to_matrix(), 1e-4));
        assert!(world.scene_instances().unwrap().is_empty());
        assert_hierarchy_closed(&world);
    }

    #[test]
    fn a_member_under_a_member_with_a_runtime_between_them_is_removed() {
        let mut world = World::new();
        let instance = world
            .load_scene(|builder| {
                builder.spawn((Name::new("outer"), Transform::from_xyz(4.0, 0.0, 0.0)));
                builder.spawn((Name::new("inner"), Transform::from_xyz(1.0, 0.0, 0.0)));
                Ok(())
            })
            .unwrap();
        let outer = world.scene_entity(instance, 0).unwrap().unwrap();
        let inner = world.scene_entity(instance, 1).unwrap().unwrap();
        let runtime = world.spawn((Name::new("between"), Transform::from_xyz(0.0, 2.0, 0.0)));
        world.set_parent(runtime, outer).unwrap();
        world.set_parent(inner, runtime).unwrap();
        world.unload_scene(instance).unwrap();
        assert!(!world.contains(outer) && !world.contains(inner));
        assert!(world.contains(runtime));
        assert!(world.component::<Parent>(runtime).is_err());
        assert!(world.component::<Children>(runtime).is_err());
        assert_hierarchy_closed(&world);
    }

    #[test]
    fn a_nonuniform_rotated_lift_stores_the_trs_approximation() {
        let mut world = World::new();
        let parent_local = Transform::from_scale(Vec3::new(2.0, 0.5, 1.0));
        let child_local = Transform::IDENTITY.with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4));
        let instance = world
            .load_scene(|builder| {
                builder.spawn((Name::new("scaled"), parent_local));
                Ok(())
            })
            .unwrap();
        let parent = world.scene_entity(instance, 0).unwrap().unwrap();
        let child = world.spawn((child_local,));
        world.set_parent(child, parent).unwrap();
        let exact = parent_local.to_matrix() * child_local.to_matrix();
        let stored = Transform::from_matrix(&exact);
        assert!(!stored.to_matrix().approx_eq(&exact, 1e-4), "this pose shears, so TRS cannot store it exactly");
        world.unload_scene(instance).unwrap();
        assert!(world.component::<Transform>(child).unwrap().to_matrix().approx_eq(&stored.to_matrix(), 1e-4));
    }

    #[test]
    fn stale_and_inconsistent_membership_follow_the_preflight_rules() {
        let mut world = World::new();
        let (instance, entity) = one_entity(&mut world, "member");
        world.remove::<SceneMember>(entity).unwrap();
        world.unload_scene(instance).unwrap();
        assert!(world.contains(entity));
        assert!(world.scene_instances().unwrap().is_empty());

        let (instance, entity) = one_entity(&mut world, "despawned");
        world.despawn(entity).unwrap();
        world.unload_scene(instance).unwrap();
        assert!(world.scene_entity(instance, 0).unwrap().is_none());

        let (instance, entity) = one_entity(&mut world, "forged");
        let intruder = world.spawn((Name::new("intruder"),));
        world.insert(intruder, (SceneMember { instance, local: 9 },)).unwrap();
        assert_eq!(world.unload_scene(instance).unwrap_err(), SceneError::InconsistentMembership { entity: intruder });
        assert!(world.contains(entity) && world.contains(intruder));
        assert_eq!(world.scene_instances().unwrap(), vec![instance]);

        let mut world = World::new();
        let (instance, entity) = one_entity(&mut world, "linked");
        let child = world.spawn((Name::new("child"),));
        world.set_parent(child, entity).unwrap();
        world.entities.get::<&mut Children>(entity).unwrap().0.clear();
        assert_eq!(world.unload_scene(instance).unwrap_err(), SceneError::InvalidHierarchy { entity: child });
        assert!(world.contains(entity) && world.contains(child));
        assert_eq!(world.scene_entity(instance, 0).unwrap(), Some(entity));
    }

    #[test]
    fn a_non_finite_lift_refuses_the_unload_and_a_zero_axis_is_stored() {
        let mut world = World::new();
        let instance = world
            .load_scene(|builder| {
                builder.spawn((Name::new("parent"), Transform::IDENTITY));
                Ok(())
            })
            .unwrap();
        let parent = world.scene_entity(instance, 0).unwrap().unwrap();
        let child = world.spawn((Transform { position: Vec3::new(f32::NAN, 0.0, 0.0), ..Transform::IDENTITY },));
        world.set_parent(child, parent).unwrap();
        assert_eq!(world.unload_scene(instance).unwrap_err(), SceneError::TransformUnrepresentable { entity: child });
        assert!(world.contains(parent) && world.contains(child));
        assert_eq!(world.scene_instances().unwrap(), vec![instance]);

        let mut world = World::new();
        let instance = world
            .load_scene(|builder| {
                builder.spawn((Name::new("flat"), Transform::from_scale(Vec3::new(0.0, 1.0, 1.0))));
                Ok(())
            })
            .unwrap();
        let parent = world.scene_entity(instance, 0).unwrap().unwrap();
        let child = world.spawn((Transform::IDENTITY,));
        world.set_parent(child, parent).unwrap();
        world.unload_scene(instance).unwrap();
        assert!(!world.contains(parent));
        assert_eq!(world.component::<Transform>(child).unwrap().scale, Vec3::new(0.0, 1.0, 1.0));
    }

    #[test]
    fn replacement_keeps_the_original_until_the_new_build_succeeds() {
        let mut world = World::new();
        let (original, entity) = one_entity(&mut world, "original");
        let (other, other_entity) = one_entity(&mut world, "other");
        let runtime = world.spawn((Name::new("keep"), Transform::IDENTITY));

        let before_spawn = world.replace_scene(original, |_| Err(SceneError::BuildRejected)).unwrap_err();
        assert_eq!(before_spawn, SceneError::BuildRejected);
        assert_eq!(world.scene_entity(original, 0).unwrap(), Some(entity));
        assert!(world.scene_instances().unwrap().iter().all(|id| *id != SceneInstanceId(2)));

        let after_spawns = world
            .replace_scene(original, |builder| {
                let root = builder.spawn((Name::new("new-root"),));
                let child = builder.spawn_child(root, (Name::new("new-child"),))?;
                builder.spawn_child(child, (Name::new("new-leaf"),))?;
                Err(SceneError::BuildRejected)
            })
            .unwrap_err();
        assert_eq!(after_spawns, SceneError::BuildRejected);
        assert_eq!(world.entities.len(), 3);
        assert!(world.contains(entity) && world.contains(other_entity) && world.contains(runtime));
        assert_eq!(world.scene_instances().unwrap(), vec![original, other]);

        let empty = world.load_scene(|_| Ok(())).unwrap();
        let replaced_empty = world.replace_scene(empty, |_| Ok(())).unwrap();
        assert_ne!(replaced_empty, empty);
        assert!(world.scene_entities(empty).unwrap().is_none());
        assert_eq!(world.scene_entities(replaced_empty).unwrap(), Some(vec![]));

        let replacement = world
            .replace_scene(original, |builder| {
                builder.spawn((Name::new("replacement"), Transform::from_xyz(1.0, 0.0, 0.0)));
                Ok(())
            })
            .unwrap();
        assert!(!world.contains(entity));
        assert!(world.scene_entity(original, 0).unwrap().is_none());
        assert_eq!(world.scene_entities(replacement).unwrap().unwrap().len(), 1);
        assert_eq!(world.scene_entity(other, 0).unwrap(), Some(other_entity));
        assert!(world.contains(runtime));
        assert!(replacement.raw() > original.raw());
        assert_hierarchy_closed(&world);
        assert!(world.has_resource::<SceneManager>());
    }

    #[test]
    fn a_forged_member_fails_the_replacement_build_and_a_later_unload_keeps_the_new_instance() {
        let mut world = World::new();
        let (original, entity) = one_entity(&mut world, "original");
        let runtime = world.spawn((Name::new("runtime"), Transform::from_xyz(3.0, 0.0, 0.0)));
        world.set_parent(runtime, entity).unwrap();
        let world_before = world.preview_lift(runtime).unwrap();

        let forged = world
            .replace_scene(original, |builder| {
                builder.spawn((Name::new("forged"), SceneMember { instance: original, local: 9 }));
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(forged, SceneError::AlreadyMember { instance, .. } if instance == original));
        assert!(world.contains(entity) && world.contains(runtime));
        assert_eq!(world.component::<Parent>(runtime).unwrap().0, entity);
        assert_eq!(world.scene_entity(original, 0).unwrap(), Some(entity));
        assert_eq!(world.scene_instances().unwrap(), vec![original]);
        assert!(world.scene_entity(SceneInstanceId(1), 0).unwrap().is_none());

        let replacement = world
            .replace_scene(original, |builder| {
                builder.spawn((Name::new("next"), Transform::IDENTITY));
                Ok(())
            })
            .unwrap();
        assert!(!world.contains(entity));
        assert!(world.contains(runtime));
        assert!(world.component::<Parent>(runtime).is_err());
        assert!(world.component::<Transform>(runtime).unwrap().to_matrix().approx_eq(&world_before.to_matrix(), 1e-4));
        assert!(world.scene_entity(original, 0).unwrap().is_none());
        assert_eq!(world.scene_entities(replacement).unwrap().unwrap().len(), 1);
        assert_eq!(replacement.raw(), 2);
        assert_hierarchy_closed(&world);
    }

    #[test]
    fn a_missing_manager_rejects_unload_and_replace_without_inserting_one() {
        let mut world = World::new();
        let manager = world.remove_resource::<SceneManager>().unwrap();
        let missing = SceneInstanceId(0);
        assert_eq!(world.unload_scene(missing).unwrap_err(), SceneError::SceneManagerUnavailable);
        assert_eq!(world.replace_scene(missing, |_| Ok(())).unwrap_err(), SceneError::SceneManagerUnavailable);
        assert!(!world.has_resource::<SceneManager>());
        world.insert_resource(manager);
    }

    #[test]
    fn a_member_parented_under_a_runtime_entity_leaves_that_parent() {
        let mut world = World::new();
        let runtime = world.spawn((Name::new("runtime"), Transform::IDENTITY));
        let sibling = world.spawn_child(runtime, (Name::new("sibling"), Transform::IDENTITY));
        let (instance, member) = one_entity(&mut world, "member");
        world.set_parent(member, runtime).unwrap();
        world.unload_scene(instance).unwrap();
        assert!(!world.contains(member));
        assert_eq!(world.component::<Children>(runtime).unwrap().0, vec![sibling]);
        assert_hierarchy_closed(&world);
    }

    #[test]
    fn deferred_operations_stay_pending_until_flush_and_completed_results_stay_until_cleared() {
        let mut world = World::new();
        let (original, entity) = one_entity(&mut world, "original");
        let (other, other_entity) = one_entity(&mut world, "other");
        let runtime = world.spawn((Name::new("runtime"), Transform::from_xyz(1.0, 0.0, 0.0)));
        world.set_parent(runtime, entity).unwrap();

        let load = world
            .commands()
            .load_scene(|builder| {
                builder.spawn((Name::new("later"),));
                Ok(())
            })
            .unwrap();
        assert_eq!(world.scene_operation(load).unwrap(), SceneOpStatus::Pending);
        assert_eq!(world.scene_operation(load).unwrap(), SceneOpStatus::Pending);
        assert_eq!(world.dismiss_scene_operation(load).unwrap_err(), SceneError::OperationPending { operation: load });
        assert_eq!(
            world.scene_operation(SceneOpId(9)).unwrap_err(),
            SceneError::NoSuchOperation { operation: SceneOpId(9) }
        );
        assert!(world.find_by_name("later").is_empty());

        let failed_load = world
            .commands()
            .load_scene(|builder| {
                let root = builder.spawn((Name::new("doomed"),));
                builder.spawn_child(root, (Name::new("doomed-child"),))?;
                Err(SceneError::BuildRejected)
            })
            .unwrap();
        let bad_unload = world.commands().unload_scene(SceneInstanceId(50)).unwrap();
        let failed_replace = world
            .commands()
            .replace_scene(other, |builder| {
                let root = builder.spawn((Name::new("nope"),));
                builder.spawn_child(root, (Name::new("nope-child"),))?;
                Err(SceneError::BuildRejected)
            })
            .unwrap();
        let unload = world.commands().unload_scene(original).unwrap();
        let replace = world
            .commands()
            .replace_scene(other, |builder| {
                builder.spawn((Name::new("replacement"),));
                Ok(())
            })
            .unwrap();
        world.commands().spawn((Name::new("after"),));
        assert!(load.raw() < failed_load.raw() && failed_load.raw() < bad_unload.raw());
        assert!(bad_unload.raw() < failed_replace.raw() && failed_replace.raw() < unload.raw());
        assert!(unload.raw() < replace.raw());

        world.flush_commands();

        let SceneOpStatus::Loaded { instance: loaded } = world.scene_operation(load).unwrap() else {
            panic!("load should publish");
        };
        assert_eq!(
            *world.component::<Name>(world.scene_entity(loaded, 0).unwrap().unwrap()).unwrap(),
            Name::new("later")
        );
        assert_eq!(
            world.scene_operation(failed_load).unwrap(),
            SceneOpStatus::Failed { error: SceneError::BuildRejected }
        );
        assert!(world.find_by_name("doomed").is_empty());
        assert!(world.find_by_name("doomed-child").is_empty());
        assert_eq!(
            world.scene_operation(bad_unload).unwrap(),
            SceneOpStatus::Failed { error: SceneError::NoSuchInstance { instance: SceneInstanceId(50) } }
        );
        assert_eq!(
            world.scene_operation(failed_replace).unwrap(),
            SceneOpStatus::Failed { error: SceneError::BuildRejected }
        );
        assert!(world.find_by_name("nope").is_empty());
        assert_eq!(world.scene_operation(unload).unwrap(), SceneOpStatus::Unloaded { instance: original });
        assert!(!world.contains(entity));
        assert!(world.contains(runtime));
        assert!(world.component::<Parent>(runtime).is_err());
        let SceneOpStatus::Replaced { old, new } = world.scene_operation(replace).unwrap() else {
            panic!("replace should publish a new instance");
        };
        assert_eq!(old, other);
        assert!(!world.contains(other_entity));
        assert_eq!(world.scene_entities(new).unwrap().unwrap().len(), 1);
        assert!(world.scene_entity(original, 0).unwrap().is_none());
        assert_eq!(world.find_by_name("after").len(), 1);
        assert!(world.command_failures().is_empty());
        assert!(world.has_resource::<SceneManager>());
        assert_hierarchy_closed(&world);

        assert_eq!(world.clear_scene_operations().unwrap(), 6);
        assert_eq!(world.scene_operation(load).unwrap_err(), SceneError::NoSuchOperation { operation: load });
        let still = world
            .commands()
            .load_scene(|builder| {
                builder.spawn((Name::new("kept-pending"),));
                Ok(())
            })
            .unwrap();
        assert_eq!(world.clear_scene_operations().unwrap(), 0);
        assert_eq!(world.scene_operation(still).unwrap(), SceneOpStatus::Pending);
        world.flush_commands();
        let status = world.dismiss_scene_operation(still).unwrap();
        assert!(matches!(status, SceneOpStatus::Loaded { .. }));
        assert_eq!(world.scene_operation(still).unwrap_err(), SceneError::NoSuchOperation { operation: still });
        assert_eq!(
            world.dismiss_scene_operation(SceneOpId(40)).unwrap_err(),
            SceneError::NoSuchOperation { operation: SceneOpId(40) }
        );
    }

    #[test]
    fn a_missing_manager_rejects_a_queued_scene_operation_without_inserting_one() {
        let mut world = World::new();
        let manager = world.remove_resource::<SceneManager>().unwrap();
        assert_eq!(world.commands().load_scene(|_| Ok(())).unwrap_err(), SceneError::SceneManagerUnavailable);
        assert_eq!(world.commands().unload_scene(SceneInstanceId(0)).unwrap_err(), SceneError::SceneManagerUnavailable);
        assert!(!world.has_resource::<SceneManager>());
        assert!(world.command_failures().is_empty());
        world.insert_resource(manager);
    }
}
