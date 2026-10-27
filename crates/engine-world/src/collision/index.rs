//! Sorted collider list rebuilt from the ECS at the end of each fixed iteration.

use engine_math::Vec3;
use hecs::Entity;

use crate::hierarchy::{propagate_transforms, GlobalTransform, Parent};
use crate::Transform;
use crate::World;

use super::convert::{prepare, Prepared};
use super::query::wire_segments;
use super::types::{point_finite, Collider, ColliderShape, CollisionQuery, RayHit, SkipReason};

pub(super) struct Entry {
    pub entity: Entity,
    pub layers: u32,
    pub sensor: bool,
    pub prepared: Prepared,
}

/// Runtime snapshot of the colliders that passed validation.
///
/// [`sync_collision`] replaces this list. The snapshot stays as it was until the next sync. A later
/// write to `Transform`, `Parent`, or `Collider` makes it stale. Queries do not notice that write
/// and do not rebuild. A `Game::step` with no fixed iteration does not call [`sync_collision`].
#[derive(Default)]
pub struct CollisionWorld {
    entries: Vec<Entry>,
    skipped: Vec<(Entity, SkipReason)>,
    /// How many times [`sync_collision`] has rebuilt this resource.
    generation: u64,
}

impl CollisionWorld {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Indexed entities, in `Entity` order.
    pub fn entities(&self) -> Vec<Entity> {
        self.entries.iter().map(|entry| entry.entity).collect()
    }

    /// World center of an indexed collider, including its local offset.
    pub fn center(&self, entity: Entity) -> Option<Vec3> {
        self.entries.iter().find(|entry| entry.entity == entity).map(|entry| entry.prepared.center)
    }

    /// Rigid rotation used for queries. Reflections have already been folded into a proper rotation.
    pub fn rotation(&self, entity: Entity) -> Option<engine_math::Quat> {
        self.entries.iter().find(|entry| entry.entity == entity).map(|entry| entry.prepared.rotation)
    }

    /// Shape after scale has been baked in. Extents are positive.
    pub fn indexed_shape(&self, entity: Entity) -> Option<ColliderShape> {
        self.entries.iter().find(|entry| entry.entity == entity).map(|entry| entry.prepared.shape)
    }

    /// Colliders left out of the index, in `Entity` order. Unchanged skips are not logged again.
    pub fn skipped(&self) -> &[(Entity, SkipReason)] {
        &self.skipped
    }

    pub fn overlap(&self, shape: ColliderShape, pose: &Transform, query: CollisionQuery) -> Vec<Entity> {
        super::query::overlap(self, shape, pose, query)
    }

    pub fn ray(&self, origin: Vec3, direction: Vec3, max_distance: f32, query: CollisionQuery) -> Option<RayHit> {
        super::query::ray(self, origin, direction, max_distance, query)
    }

    /// World-space segments for the indexed shapes. Every endpoint is finite.
    pub fn wire_segments(&self) -> Vec<(Vec3, Vec3)> {
        wire_segments(self)
    }
}

/// Propagates transforms, then rebuilds [`CollisionWorld`] from enabled, valid colliders.
///
/// Inserts an empty [`CollisionWorld`] when the resource is missing. Disabled colliders are absent
/// and are not reported as skips. A newly invalid collider is logged once; the same skip on a later
/// sync is not logged again.
pub fn sync_collision(world: &mut World) {
    propagate_transforms(world);
    if !world.has_resource::<CollisionWorld>() {
        world.insert_resource(CollisionWorld::default());
    }

    let mut found: Vec<(Entity, Collider)> =
        world.entities.query::<&Collider>().iter().map(|(entity, collider)| (entity, *collider)).collect();
    found.sort_by_key(|(entity, _)| *entity);

    let mut entries = Vec::new();
    let mut skipped = Vec::new();
    for (entity, collider) in found {
        if !collider.enabled {
            continue;
        }
        match classify(world, entity, &collider) {
            Ok(prepared) => entries.push(Entry { entity, layers: collider.layers, sensor: collider.sensor, prepared }),
            Err(reason) => skipped.push((entity, reason)),
        }
    }

    let mut collision = world.resource_mut::<CollisionWorld>();
    log_new_skips(&collision.skipped, &skipped);
    collision.entries = entries;
    collision.skipped = skipped;
    collision.generation = collision.generation.saturating_add(1);
}

fn classify(world: &World, entity: Entity, collider: &Collider) -> Result<Prepared, SkipReason> {
    if world.component::<Transform>(entity).is_err() {
        return Err(SkipReason::NoTransform);
    }
    if let Ok(parent) = world.component::<Parent>(entity) {
        if !world.contains(parent.0) {
            return Err(SkipReason::MissingParent);
        }
    }
    let global = world.component::<GlobalTransform>(entity).map_err(|_| SkipReason::NoTransform)?;
    prepare(collider.shape, collider.offset, &global.matrix)
}

fn log_new_skips(previous: &[(Entity, SkipReason)], next: &[(Entity, SkipReason)]) {
    for (entity, reason) in fresh_skips(previous, next) {
        log::warn!("collider {entity:?} left out of the collision index: {reason}");
    }
}

fn fresh_skips<'a>(
    previous: &[(Entity, SkipReason)],
    next: &'a [(Entity, SkipReason)],
) -> Vec<&'a (Entity, SkipReason)> {
    next.iter().filter(|pair| !previous.contains(pair)).collect()
}

pub(super) fn entries(world: &CollisionWorld) -> &[Entry] {
    &world.entries
}

pub(super) fn query_allows(entry: &Entry, query: CollisionQuery) -> bool {
    if query.exclude == Some(entry.entity) {
        return false;
    }
    if entry.sensor && !query.include_sensors {
        return false;
    }
    entry.layers & query.mask != 0 && point_finite(entry.prepared.center)
}

#[cfg(test)]
mod tests {
    use super::fresh_skips;
    use super::SkipReason;
    use crate::World;

    #[test]
    fn an_unchanged_skip_is_not_logged_again() {
        let mut world = World::new();
        let entity = world.spawn(());
        let previous = vec![(entity, SkipReason::Shear)];
        assert!(fresh_skips(&previous, &previous).is_empty());
        let changed = vec![(entity, SkipReason::Degenerate)];
        assert_eq!(fresh_skips(&previous, &changed), vec![&(entity, SkipReason::Degenerate)]);
    }
}
