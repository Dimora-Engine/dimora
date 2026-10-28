//! Sweep-and-slide for [`CharacterMotor`]. World axes stay world axes. A camera and a visual
//! child are not inputs.
//!
//! A gap inside the skin band is classified with the closest-feature wall, not the shape-cast
//! normal. Outside that band the sweep keeps the cast's time of impact and slides on the feature
//! wall at the impact pose.

use engine_math::{Mat4, Quat, Vec3};
use hecs::Entity;
use parry3d::bounding_volume::Aabb;
use parry3d::math::{Pose, Rotation, Vector};
use parry3d::query::{cast_shapes, contact, ShapeCastHit, ShapeCastOptions, ShapeCastStatus};
use parry3d::shape::Shape;

use crate::hierarchy::{Children, GlobalTransform, Parent};
use crate::Transform;
use crate::World;

use super::convert::{to_vector, Prepared};
use super::feature::feature_hit;
use super::index::{entries, entries_mut, CollisionWorld};
use super::types::{point_finite, CharacterMotor, MoveIntent, MoveResult, MoveStatus};

const STOP: f32 = 1e-6;
const TIE: f32 = 1e-5;
const RECOVERY_MIN: f32 = 1e-4;
/// Absolute world-unit width of a near-contact. This is the position tolerance, not a fraction of
/// the request. A gap in `(-BAND, skin + BAND]` uses the closest-feature classifier.
const BAND: f32 = 1e-4;

struct Motion {
    delta: Vec3,
    blocked: bool,
    normals: Vec<Vec3>,
    recovery_normal: Option<Vec3>,
}

enum Scan {
    Clear,
    Stop,
    Push { outward: Vec3, dist: f32 },
    Hit { toi: f32, wall: Vec3 },
}

/// Resolves every motor that has a [`MoveIntent`], in ascending [`Entity`] order.
///
/// The caller has already propagated transforms and rebuilt the index. Each successful move writes
/// `Transform::position` and patches that collider's center, pose, and bounds before the next motor
/// runs. `GlobalTransform` is left for a later propagation.
pub(super) fn resolve_motors(world: &mut World) {
    let mut jobs: Vec<(Entity, CharacterMotor, Vec3)> = world
        .query::<(&CharacterMotor, &MoveIntent)>()
        .iter()
        .map(|(entity, (motor, intent))| (entity, *motor, intent.translation))
        .collect();
    jobs.sort_by_key(|(entity, _, _)| *entity);
    for (entity, motor, translation) in jobs {
        resolve_one(world, entity, motor, translation);
    }
}

/// Drops every [`MoveIntent`]. A fixed loop calls this after its iterations, including when that
/// loop ran zero times, so an unused intent does not leak into a later frame.
pub fn clear_move_intents(world: &mut World) {
    let pending: Vec<Entity> = world.query::<&MoveIntent>().iter().map(|(entity, _)| entity).collect();
    for entity in pending {
        let _ = world.remove::<MoveIntent>(entity);
    }
}

fn resolve_one(world: &mut World, entity: Entity, motor: CharacterMotor, requested: Vec3) {
    if !motor_ok(&motor) || !point_finite(requested) {
        log::warn!("character motor on {entity:?} rejected a move: skin or displacement is not usable");
        publish(world, entity, rejected(requested));
        return;
    }
    if indexed_descendant(world, entity) {
        log::warn!("character motor on {entity:?} rejected a move: an indexed descendant is in the way");
        publish(world, entity, rejected(requested));
        return;
    }

    let motion = {
        let collision = world.resource::<CollisionWorld>();
        sweep(&collision, entity, &motor, requested)
    };
    let Some(motion) = motion else {
        publish(world, entity, rejected(requested));
        return;
    };
    if !apply_world_delta(world, entity, motion.delta) {
        log::warn!("character motor on {entity:?} rejected a move: the parent transform has no inverse");
        publish(world, entity, rejected(requested));
        return;
    }
    if motion.delta != Vec3::ZERO {
        let mut collision = world.resource_mut::<CollisionWorld>();
        if let Some(prepared) = entries_mut(&mut collision).iter_mut().find(|entry| entry.entity == entity) {
            translate_prepared(&mut prepared.prepared, motion.delta);
        }
    }
    publish(
        world,
        entity,
        MoveResult {
            requested,
            applied: motion.delta,
            status: if motion.blocked { MoveStatus::Blocked } else { MoveStatus::Completed },
            normals: motion.normals,
            recovery_normal: motion.recovery_normal,
        },
    );
}

fn motor_ok(motor: &CharacterMotor) -> bool {
    motor.skin.is_finite() && motor.skin >= 0.0
}

fn rejected(requested: Vec3) -> MoveResult {
    MoveResult {
        requested,
        applied: Vec3::ZERO,
        status: MoveStatus::Rejected,
        normals: Vec::new(),
        recovery_normal: None,
    }
}

fn publish(world: &mut World, entity: Entity, result: MoveResult) {
    if world.insert(entity, (result,)).is_err() {
        return;
    }
    let _ = world.remove::<MoveIntent>(entity);
}

fn sweep(collision: &CollisionWorld, entity: Entity, motor: &CharacterMotor, requested: Vec3) -> Option<Motion> {
    let list = entries(collision);
    let mover = list.iter().position(|entry| entry.entity == entity)?;
    let rotation = list[mover].prepared.rotation;
    let layers = list[mover].layers;
    let mask = list[mover].mask;
    let mut center = list[mover].prepared.center;
    let start = center;
    let mut remainder = motor.locked_axes.clear(requested);
    let mut blocked = false;
    let mut normals = Vec::new();
    let mut recovery_normal = None;
    let mut pushed = false;

    if motor.max_slides == 0 {
        return Some(Motion { delta: Vec3::ZERO, blocked: remainder.length() > STOP, normals, recovery_normal });
    }

    let mut slides = 0u8;
    while remainder.length() > STOP && slides < motor.max_slides {
        slides += 1;
        let pose = pose_at(rotation, center);
        let cast =
            SweepCast { list, mover, pose: &pose, layers, mask, remainder, skin: motor.skin, pushed, center, entity };
        match scan(&cast) {
            Scan::Stop => {
                blocked = true;
                break;
            }
            Scan::Clear => {
                if !advance(&mut center, remainder) {
                    blocked = true;
                }
                remainder = Vec3::ZERO;
                break;
            }
            Scan::Push { outward, dist } => {
                let full = outward * (-dist + motor.skin);
                let push = motor.locked_axes.clear(full);
                let locked = (full - push).length() > STOP;
                if push.length() < RECOVERY_MIN && locked {
                    log::warn!("character motor on {entity:?} skipped a penetration recovery locked by an axis");
                } else if push.length() >= RECOVERY_MIN && !advance(&mut center, push) {
                    blocked = true;
                    break;
                } else if push.length() >= RECOVERY_MIN {
                    recovery_normal = Some(outward);
                    blocked = true;
                }
                pushed = true;
            }
            Scan::Hit { toi, wall } => {
                let traveled = remainder * toi;
                let unconsumed = remainder - traveled;
                if !advance(&mut center, traveled) {
                    blocked = true;
                    break;
                }
                normals.push(wall);
                blocked = true;
                let into = unconsumed.dot(wall);
                let projected = if into < 0.0 { unconsumed - wall * into } else { unconsumed };
                remainder = motor.locked_axes.clear(projected);
                if !point_finite(remainder) {
                    blocked = true;
                    break;
                }
            }
        }
    }
    if remainder.length() > STOP {
        blocked = true;
    }
    let delta = center - start;
    if !point_finite(delta) {
        return None;
    }
    Some(Motion { delta, blocked, normals, recovery_normal })
}

fn advance(center: &mut Vec3, delta: Vec3) -> bool {
    if !point_finite(delta) {
        return false;
    }
    let next = *center + delta;
    if !point_finite(next) {
        return false;
    }
    *center = next;
    true
}

#[derive(Clone, Copy)]
struct SweepCast<'a> {
    list: &'a [super::index::Entry],
    mover: usize,
    pose: &'a Pose,
    layers: u32,
    mask: u32,
    remainder: Vec3,
    skin: f32,
    pushed: bool,
    center: Vec3,
    entity: Entity,
}

fn scan(cast: &SweepCast<'_>) -> Scan {
    let SweepCast { list, mover, pose, layers, mask, remainder, skin, center, entity, .. } = *cast;
    let shape = list[mover].prepared.parry.as_shape();
    let bounds = swept_bounds(shape, pose, remainder, skin);
    let options = ShapeCastOptions {
        max_time_of_impact: 1.0,
        target_distance: skin,
        stop_at_penetration: false,
        compute_impact_geometry_on_penetration: true,
    };
    let velocity = to_vector(remainder);
    let mut hits: Vec<(f32, Entity, Vec3)> = Vec::new();
    let mut push: Option<(f32, Entity, Vec3)> = None;

    for (index, entry) in list.iter().enumerate() {
        if index == mover || entry.sensor || !masks(layers, mask, entry.layers, entry.mask) {
            continue;
        }
        if !bounds_overlap(&bounds, &entry.prepared.aabb) {
            continue;
        }
        let Some(feature) = feature_hit(&list[mover].prepared, center, &entry.prepared) else {
            log::warn!("character motor on {entity:?} stopped a move: closest-feature query failed");
            return Scan::Stop;
        };
        if !feature.gap.is_finite() {
            log::warn!("character motor on {entity:?} stopped a move: closest-feature gap is not finite");
            return Scan::Stop;
        }

        // Near the surface, a shape cast can report a motion-aligned normal and a tiny positive
        // time. Classify this candidate from the feature wall and keep scanning the others.
        if in_skin_band(feature.gap, skin) {
            if !consider_feature(&mut hits, entry.entity, feature.wall, remainder) {
                log::warn!("character motor on {entity:?} stopped a move: closest-feature normal is degenerate");
                return Scan::Stop;
            }
            continue;
        }

        let cast_hit = match cast_shapes(
            pose,
            velocity,
            shape,
            &entry.prepared.pose,
            to_vector(Vec3::ZERO),
            entry.prepared.parry.as_shape(),
            options,
        ) {
            Ok(hit) => hit,
            Err(error) => {
                log::warn!("character motor shape cast is unsupported ({error}); the rest of the move is dropped");
                return Scan::Stop;
            }
        };

        if feature.gap <= -BAND {
            if !resolve_overlap(cast, entry, cast_hit.as_ref(), feature.wall, &mut hits, &mut push) {
                return Scan::Stop;
            }
            continue;
        }

        if let Some(hit) = cast_hit {
            if !consider_separated(cast, entry, &hit, &mut hits) {
                return Scan::Stop;
            }
        }
    }

    if let Some((dist, _, outward)) = push {
        return Scan::Push { outward, dist };
    }
    earliest(hits).map(|(toi, wall)| Scan::Hit { toi, wall }).unwrap_or(Scan::Clear)
}

fn in_skin_band(gap: f32, skin: f32) -> bool {
    gap > -BAND && gap <= skin + BAND
}

/// Block at time zero when the remainder approaches the wall. A tangent or separating remainder
/// drops this candidate only.
fn consider_feature(hits: &mut Vec<(f32, Entity, Vec3)>, entity: Entity, wall: Option<Vec3>, remainder: Vec3) -> bool {
    match wall {
        Some(wall) if remainder.dot(wall) < -BAND => {
            hits.push((0.0, entity, wall));
            true
        }
        Some(_) => true,
        None => false,
    }
}

fn resolve_overlap(
    cast: &SweepCast<'_>,
    entry: &super::index::Entry,
    hit: Option<&ShapeCastHit>,
    wall: Option<Vec3>,
    hits: &mut Vec<(f32, Entity, Vec3)>,
    push: &mut Option<(f32, Entity, Vec3)>,
) -> bool {
    if let Some(hit) = hit {
        if !hit.time_of_impact.is_finite() {
            return false;
        }
        let time_zero = hit.status == ShapeCastStatus::PenetratingOrWithinTargetDist || hit.time_of_impact <= STOP;
        if time_zero {
            match classify_touch(cast, entry) {
                Touch::Ignore => return true,
                Touch::Stop => return false,
                Touch::Push { dist, outward } => {
                    consider_push(push, dist, entry.entity, outward);
                    return true;
                }
                Touch::Block { wall } => {
                    hits.push((0.0, entry.entity, wall));
                    return true;
                }
                Touch::Unconfirmed => {}
            }
        }
    }
    // The cast did not confirm a penetration. Do not travel its time of impact while already inside.
    if !consider_feature(hits, entry.entity, wall, cast.remainder) {
        log::warn!("character motor on {:?} stopped a move: overlapped shapes have no feature normal", cast.entity);
        return false;
    }
    true
}

fn consider_separated(
    cast: &SweepCast<'_>,
    entry: &super::index::Entry,
    hit: &ShapeCastHit,
    hits: &mut Vec<(f32, Entity, Vec3)>,
) -> bool {
    if !hit.time_of_impact.is_finite() {
        return false;
    }
    // A time-zero cast disagrees with a gap outside the skin band. Ignore that cast.
    if hit.time_of_impact <= STOP || hit.time_of_impact > 1.0 {
        return true;
    }
    let impact = cast.center + cast.remainder * hit.time_of_impact;
    let Some(feature) = feature_hit(&cast.list[cast.mover].prepared, impact, &entry.prepared) else {
        log::warn!("character motor on {:?} stopped a move: impact feature query failed", cast.entity);
        return false;
    };
    match feature.wall {
        Some(wall) => {
            hits.push((hit.time_of_impact, entry.entity, wall));
            true
        }
        None => {
            log::warn!("character motor on {:?} stopped a move: impact feature normal is degenerate", cast.entity);
            false
        }
    }
}

enum Touch {
    Ignore,
    Stop,
    Unconfirmed,
    Push { dist: f32, outward: Vec3 },
    Block { wall: Vec3 },
}

fn classify_touch(cast: &SweepCast<'_>, entry: &super::index::Entry) -> Touch {
    let prediction = if cast.skin > 0.0 { cast.skin } else { 1e-4 };
    let shape = cast.list[cast.mover].prepared.parry.as_shape();
    let touch = match contact(cast.pose, shape, &entry.prepared.pose, entry.prepared.parry.as_shape(), prediction) {
        Ok(Some(touch)) => touch,
        Ok(None) => return Touch::Unconfirmed,
        Err(_) => return Touch::Stop,
    };
    if !touch.dist.is_finite() {
        return Touch::Stop;
    }
    // Contact `normal1` is already world-space and points from the mover toward the obstacle.
    let Some(toward_obstacle) = finite_unit(from_vector(touch.normal1)) else {
        return Touch::Stop;
    };
    let outward = -toward_obstacle;
    if touch.dist < 0.0 {
        if cast.pushed {
            return block_or_ignore(cast.remainder, outward);
        }
        return Touch::Push { dist: touch.dist, outward };
    }
    Touch::Unconfirmed
}

fn block_or_ignore(remainder: Vec3, wall: Vec3) -> Touch {
    if remainder.dot(wall) > 0.0 {
        Touch::Ignore
    } else {
        Touch::Block { wall }
    }
}

fn consider_push(push: &mut Option<(f32, Entity, Vec3)>, dist: f32, entity: Entity, outward: Vec3) {
    let replace = match push {
        None => true,
        Some((best, best_entity, _)) => dist < *best - TIE || ((dist - *best).abs() <= TIE && entity < *best_entity),
    };
    if replace {
        *push = Some((dist, entity, outward));
    }
}

fn earliest(mut hits: Vec<(f32, Entity, Vec3)>) -> Option<(f32, Vec3)> {
    let min = hits.iter().map(|(toi, _, _)| *toi).fold(f32::MAX, f32::min);
    if !min.is_finite() {
        return None;
    }
    hits.retain(|(toi, _, _)| *toi <= min + TIE);
    hits.sort_by_key(|(_, entity, _)| *entity);
    hits.first().map(|(toi, _, wall)| (*toi, *wall))
}

fn masks(layers: u32, mask: u32, other_layers: u32, other_mask: u32) -> bool {
    layers & other_mask != 0 && other_layers & mask != 0
}

fn swept_bounds(shape: &dyn Shape, pose: &Pose, delta: Vec3, skin: f32) -> Aabb {
    let aabb = shape.compute_aabb(pose);
    let skin = if skin.is_finite() { skin.max(0.0) } else { 0.0 };
    let (x0, x1) = span(delta.x);
    let (y0, y1) = span(delta.y);
    let (z0, z1) = span(delta.z);
    Aabb {
        mins: Vector::new(aabb.mins.x + x0 - skin, aabb.mins.y + y0 - skin, aabb.mins.z + z0 - skin),
        maxs: Vector::new(aabb.maxs.x + x1 + skin, aabb.maxs.y + y1 + skin, aabb.maxs.z + z1 + skin),
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

fn from_vector(value: Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

fn finite_unit(value: Vec3) -> Option<Vec3> {
    if !point_finite(value) {
        return None;
    }
    let length = value.length();
    if !length.is_finite() || length < STOP {
        return None;
    }
    Some(value / length)
}

fn translate_prepared(prepared: &mut Prepared, delta: Vec3) {
    prepared.center += delta;
    prepared.pose = pose_at(prepared.rotation, prepared.center);
    prepared.aabb = prepared.parry.as_shape().compute_aabb(&prepared.pose);
}

fn apply_world_delta(world: &mut World, entity: Entity, delta: Vec3) -> bool {
    if delta == Vec3::ZERO {
        return true;
    }
    if !point_finite(delta) {
        return false;
    }
    let local = match parent_matrix(world, entity) {
        None => delta,
        Some(matrix) => {
            let Some(inverse) = matrix.inverse() else {
                return false;
            };
            let local = inverse.transform_vector3(delta);
            if !point_finite(local) {
                return false;
            }
            local
        }
    };
    let Ok(mut transform) = world.component_mut::<Transform>(entity) else {
        return false;
    };
    let next = transform.position + local;
    if !point_finite(next) {
        return false;
    }
    transform.position = next;
    true
}

fn parent_matrix(world: &World, entity: Entity) -> Option<Mat4> {
    let parent = world.component::<Parent>(entity).ok()?.0;
    Some(world.component::<GlobalTransform>(parent).ok()?.matrix)
}

fn indexed_descendant(world: &World, entity: Entity) -> bool {
    let indexed = world.resource::<CollisionWorld>().entities();
    let mut stack = children_of(world, entity);
    let mut seen = vec![entity];
    while let Some(next) = stack.pop() {
        if seen.contains(&next) {
            continue;
        }
        seen.push(next);
        if indexed.contains(&next) {
            return true;
        }
        stack.extend(children_of(world, next));
    }
    false
}

fn children_of(world: &World, entity: Entity) -> Vec<Entity> {
    world.component::<Children>(entity).map(|children| children.0.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_singular_parent_matrix_refuses_the_local_delta() {
        let singular = Mat4::from_scale(Vec3::new(0.0, 1.0, 1.0));
        assert!(singular.inverse().is_none());
        let mut world = World::new();
        let parent = world.spawn((Transform::IDENTITY,));
        let child = world.spawn((Transform::IDENTITY, Parent(parent)));
        world.insert(parent, (GlobalTransform { matrix: singular },)).unwrap();
        assert!(!apply_world_delta(&mut world, child, Vec3::X));
        assert_eq!(world.component::<Transform>(child).unwrap().position, Vec3::ZERO);
    }
}
