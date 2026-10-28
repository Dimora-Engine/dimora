//! Sensor overlap and trigger events for one `sync_collision` call.
//!
//! The stored pair list is the overlaps from the previous sync. Motors append the displacements
//! they actually write, and those segments are cast with `target_distance` 0 against the index as
//! that motor sees it: earlier movers are already patched, later movers are still at their start
//! pose. This is sequential, not a simultaneous sweep of two moving bodies. After every motor, the
//! final index is the end state. Each pair then follows one rule: enter, exit, stay silent, or
//! enter and exit when a committed segment crossed it while both endpoints are outside.
//!
//! A pair crossed more than once in the same iteration still produces that one outcome. The path
//! hit is a boolean. `MoveResult.applied` is the net origin change and is not cast as one segment.
//! A direct transform write has no segments, so it can only change the endpoint sets.
//!
//! A pair blocks only when both colliders are solid. A sensor mover still writes its committed
//! segments, and those segments are cast here. A rejected move writes none.

use engine_math::{Quat, Vec3};
use hecs::Entity;
use parry3d::bounding_volume::Aabb;
use parry3d::math::{Pose, Rotation};
use parry3d::query::{cast_shapes, intersection_test, ShapeCastOptions};
use parry3d::shape::Shape;

use crate::World;

use super::convert::to_vector;
use super::index::{entries, CollisionWorld, Entry};
use super::types::TriggerEvent;

/// One displacement `advance` wrote, in world space, from the shape center at its start.
#[derive(Clone, Copy)]
pub(super) struct Segment {
    pub start: Vec3,
    pub delta: Vec3,
}

/// Sends [`TriggerEvent`] values and stores the new pair list.
///
/// `hits` are canonical pairs touched by committed segments. They may contain duplicates.
/// When the index has no sensors and the stored list is empty, this returns without a pair pass.
pub(super) fn commit(world: &mut World, mut hits: Vec<(Entity, Entity)>) {
    let idle = {
        let collision = world.resource::<CollisionWorld>();
        collision.stored_pairs().is_empty() && entries(&collision).iter().all(|entry| !entry.sensor)
    };
    if idle {
        return;
    }

    let (old, new) = {
        let mut collision = world.resource_mut::<CollisionWorld>();
        let new = if entries(&collision).iter().any(|entry| entry.sensor) {
            overlapping_pairs(entries(&collision))
        } else {
            Vec::new()
        };
        let old = collision.take_pairs();
        (old, new)
    };

    hits.sort_unstable();
    hits.dedup();
    for event in classify(&old, &new, &hits) {
        world.send(event);
    }
    world.resource_mut::<CollisionWorld>().store_pairs(new);
}

/// Casts committed segments for `mover` and appends canonical pairs. The mover's indexed center
/// may already be the final pose; each cast starts at `segment.start`.
pub(super) fn collect_hits(
    collision: &CollisionWorld,
    mover: Entity,
    segments: &[Segment],
    hits: &mut Vec<(Entity, Entity)>,
) {
    let list = entries(collision);
    let Some(index) = list.iter().position(|entry| entry.entity == mover) else {
        return;
    };
    let mut previous: Option<(Vec3, Vec3)> = None;
    for segment in segments {
        if previous.is_some_and(|(start, delta)| start == segment.start && delta == segment.delta) {
            continue;
        }
        previous = Some((segment.start, segment.delta));
        cast_segment(&list[index], list, segment, hits);
    }
}

fn overlapping_pairs(list: &[Entry]) -> Vec<(Entity, Entity)> {
    let mut pairs = Vec::new();
    for (index, entry) in list.iter().enumerate() {
        for other in &list[index + 1..] {
            if !qualifies(entry, other) || !bounds_overlap(&entry.prepared.aabb, &other.prepared.aabb) {
                continue;
            }
            if intersects(&entry.prepared.pose, entry.prepared.parry.as_shape(), other) {
                pairs.push((entry.entity, other.entity));
            }
        }
    }
    pairs
}

fn cast_segment(mover: &Entry, list: &[Entry], segment: &Segment, hits: &mut Vec<(Entity, Entity)>) {
    let pose = pose_at(mover.prepared.rotation, segment.start);
    let shape = mover.prepared.parry.as_shape();
    let bounds = swept_bounds(shape, &pose, segment.delta);
    let options = ShapeCastOptions {
        max_time_of_impact: 1.0,
        target_distance: 0.0,
        stop_at_penetration: true,
        compute_impact_geometry_on_penetration: false,
    };
    let velocity = to_vector(segment.delta);

    for entry in list {
        if entry.entity == mover.entity || !qualifies(mover, entry) || !bounds_overlap(&bounds, &entry.prepared.aabb) {
            continue;
        }
        if intersects(&pose, shape, entry) {
            remember(hits, mover.entity, entry.entity);
            continue;
        }
        match cast_shapes(
            &pose,
            velocity,
            shape,
            &entry.prepared.pose,
            to_vector(Vec3::ZERO),
            entry.prepared.parry.as_shape(),
            options,
        ) {
            Ok(Some(hit)) if hit.time_of_impact.is_finite() && (0.0..=1.0).contains(&hit.time_of_impact) => {
                remember(hits, mover.entity, entry.entity);
            }
            Ok(_) => {}
            Err(error) => {
                log::warn!("trigger shape cast is unsupported ({error}); that candidate is skipped");
            }
        }
    }
}

fn qualifies(a: &Entry, b: &Entry) -> bool {
    (a.sensor || b.sensor) && a.layers & b.mask != 0 && b.layers & a.mask != 0
}

fn intersects(pose: &Pose, shape: &dyn Shape, other: &Entry) -> bool {
    intersection_test(pose, shape, &other.prepared.pose, other.prepared.parry.as_shape())
        .is_ok_and(|hit| hit.intersecting)
}

fn remember(hits: &mut Vec<(Entity, Entity)>, a: Entity, b: Entity) {
    hits.push(if a < b { (a, b) } else { (b, a) });
}

fn classify(old: &[(Entity, Entity)], new: &[(Entity, Entity)], hits: &[(Entity, Entity)]) -> Vec<TriggerEvent> {
    let mut events = Vec::new();
    let mut i = 0;
    let mut j = 0;
    let mut k = 0;
    while i < old.len() || j < new.len() || k < hits.len() {
        let Some(pair) = earliest(old.get(i), new.get(j), hits.get(k)) else {
            break;
        };
        let in_old = i < old.len() && old[i] == pair;
        let in_new = j < new.len() && new[j] == pair;
        let crossed = k < hits.len() && hits[k] == pair;
        match (in_old, in_new, crossed) {
            (false, true, _) => events.push(TriggerEvent::Entered { a: pair.0, b: pair.1 }),
            (true, false, _) => events.push(TriggerEvent::Exited { a: pair.0, b: pair.1 }),
            (false, false, true) => {
                events.push(TriggerEvent::Entered { a: pair.0, b: pair.1 });
                events.push(TriggerEvent::Exited { a: pair.0, b: pair.1 });
            }
            _ => {}
        }
        if in_old {
            i += 1;
        }
        if in_new {
            j += 1;
        }
        if crossed {
            k += 1;
        }
    }
    events
}

fn earliest(
    old: Option<&(Entity, Entity)>,
    new: Option<&(Entity, Entity)>,
    hit: Option<&(Entity, Entity)>,
) -> Option<(Entity, Entity)> {
    [old, new, hit].into_iter().flatten().copied().min()
}

fn swept_bounds(shape: &dyn Shape, pose: &Pose, delta: Vec3) -> Aabb {
    let aabb = shape.compute_aabb(pose);
    let (x0, x1) = span(delta.x);
    let (y0, y1) = span(delta.y);
    let (z0, z1) = span(delta.z);
    Aabb {
        mins: parry3d::math::Vector::new(aabb.mins.x + x0, aabb.mins.y + y0, aabb.mins.z + z0),
        maxs: parry3d::math::Vector::new(aabb.maxs.x + x1, aabb.maxs.y + y1, aabb.maxs.z + z1),
    }
}

fn span(delta: f32) -> (f32, f32) {
    if delta >= 0.0 {
        (0.0, delta)
    } else {
        (delta, 0.0)
    }
}

fn bounds_overlap(a: &Aabb, b: &Aabb) -> bool {
    a.mins.x <= b.maxs.x
        && b.mins.x <= a.maxs.x
        && a.mins.y <= b.maxs.y
        && b.mins.y <= a.maxs.y
        && a.mins.z <= b.maxs.z
        && b.mins.z <= a.maxs.z
}

fn pose_at(rotation: Quat, center: Vec3) -> Pose {
    Pose::from_parts(to_vector(center), Rotation::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: u32) -> Entity {
        Entity::from_bits((id as u64) << 32).expect("generation is non-zero")
    }

    fn entered(pair: (Entity, Entity)) -> TriggerEvent {
        TriggerEvent::Entered { a: pair.0, b: pair.1 }
    }

    fn exited(pair: (Entity, Entity)) -> TriggerEvent {
        TriggerEvent::Exited { a: pair.0, b: pair.1 }
    }

    #[test]
    fn classification_follows_the_endpoint_table_and_collapses_a_repeated_hit() {
        let a = (entity(1), entity(2));
        let b = (entity(1), entity(3));
        let c = (entity(4), entity(5));
        let stayed = (entity(6), entity(7));
        let events = classify(&[a, stayed], &[b, stayed], &[a, c]);
        assert_eq!(events, vec![exited(a), entered(b), entered(c), exited(c)]);
    }
}
