//! Overlap, ray casts, and wireframe segments. Candidate tests are a linear AABB scan.

use engine_math::{Quat, Transform, Vec3};
use hecs::Entity;
use parry3d::bounding_volume::Aabb;
use parry3d::math::Vector;
use parry3d::query::{intersection_test, Ray};

use super::convert::{prepare, to_vector, Prepared};
use super::index::{entries, query_allows, CollisionWorld};
use super::types::{point_finite, ColliderShape, CollisionQuery, RayHit};

const RAY_TIE: f32 = 1e-5;
const WIRE_SEGMENTS: u32 = 16;

pub(super) fn overlap(
    world: &CollisionWorld,
    shape: ColliderShape,
    pose: &Transform,
    query: CollisionQuery,
) -> Vec<Entity> {
    let Ok(probe) = prepare(shape, Vec3::ZERO, &pose.to_matrix()) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for entry in entries(world) {
        if !query_allows(entry, query) || !aabb_overlaps(&probe.aabb, &entry.prepared.aabb) {
            continue;
        }
        let intersecting = intersection_test(
            &probe.pose,
            probe.parry.as_shape(),
            &entry.prepared.pose,
            entry.prepared.parry.as_shape(),
        );
        if intersecting.is_ok_and(|hit| hit.intersecting) {
            hits.push(entry.entity);
        }
    }
    hits
}

pub(super) fn ray(
    world: &CollisionWorld,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    query: CollisionQuery,
) -> Option<RayHit> {
    if !point_finite(origin) || !max_distance.is_finite() || max_distance < 0.0 {
        return None;
    }
    let direction = direction.try_normalize()?;
    let end = origin + direction * max_distance;
    if !point_finite(end) {
        return None;
    }
    let ray = Ray::new(to_vector(origin), to_vector(direction));
    let bounds = segment_aabb(origin, end);

    let mut best: Option<RayHit> = None;
    for entry in entries(world) {
        if !query_allows(entry, query) || !aabb_overlaps(&bounds, &entry.prepared.aabb) {
            continue;
        }
        let Some(hit) =
            entry.prepared.parry.as_shape().cast_ray_and_get_normal(&entry.prepared.pose, &ray, max_distance, true)
        else {
            continue;
        };
        if !hit.time_of_impact.is_finite() || hit.time_of_impact < 0.0 || hit.time_of_impact > max_distance {
            continue;
        }
        let normal = from_vector(hit.normal);
        let Some(normal) = finite_unit(normal) else {
            continue;
        };
        let distance = hit.time_of_impact;
        let point = origin + direction * distance;
        if !point_finite(point) {
            continue;
        }
        let candidate = RayHit { entity: entry.entity, distance, point, normal };
        best = Some(match best {
            None => candidate,
            Some(current) if nearer(candidate, current) => candidate,
            Some(current) => current,
        });
    }
    best
}

fn nearer(candidate: RayHit, current: RayHit) -> bool {
    if candidate.distance < current.distance - RAY_TIE {
        return true;
    }
    (candidate.distance - current.distance).abs() <= RAY_TIE && candidate.entity < current.entity
}

pub(super) fn wire_segments(world: &CollisionWorld) -> Vec<(Vec3, Vec3)> {
    let mut segments = Vec::new();
    for entry in entries(world) {
        match entry.prepared.shape {
            ColliderShape::Sphere { radius } => sphere(&mut segments, entry.prepared.center, radius),
            ColliderShape::Box { half_extents } => box_edges(&mut segments, &entry.prepared, half_extents),
            ColliderShape::Capsule { radius, half_height } => {
                capsule(&mut segments, &entry.prepared, radius, half_height)
            }
        }
    }
    segments.retain(|(start, end)| point_finite(*start) && point_finite(*end));
    segments
}

fn sphere(out: &mut Vec<(Vec3, Vec3)>, center: Vec3, radius: f32) {
    for axis in 0..3 {
        circle(out, center, radius, axis, Vec3::ZERO, Quat::IDENTITY);
    }
}

fn box_edges(out: &mut Vec<(Vec3, Vec3)>, prepared: &Prepared, half: Vec3) {
    let corner = |x: f32, y: f32, z: f32| prepared.center + prepared.rotation.rotate(Vec3::new(x, y, z));
    let corners = [
        corner(-half.x, -half.y, -half.z),
        corner(half.x, -half.y, -half.z),
        corner(half.x, half.y, -half.z),
        corner(-half.x, half.y, -half.z),
        corner(-half.x, -half.y, half.z),
        corner(half.x, -half.y, half.z),
        corner(half.x, half.y, half.z),
        corner(-half.x, half.y, half.z),
    ];
    const EDGES: [(usize, usize); 12] =
        [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (a, b) in EDGES {
        out.push((corners[a], corners[b]));
    }
}

fn capsule(out: &mut Vec<(Vec3, Vec3)>, prepared: &Prepared, radius: f32, half_height: f32) {
    let along = Vec3::Y * half_height;
    circle(out, prepared.center, radius, 0, along, prepared.rotation);
    circle(out, prepared.center, radius, 0, -along, prepared.rotation);
    for axis in [Vec3::X, Vec3::Z] {
        let offset = axis * radius;
        let start = prepared.center + prepared.rotation.rotate(-along + offset);
        let end = prepared.center + prepared.rotation.rotate(along + offset);
        out.push((start, end));
    }
}

/// `plane` 0 is XZ (capsule ends), 1 is XY, 2 is YZ.
fn circle(out: &mut Vec<(Vec3, Vec3)>, center: Vec3, radius: f32, plane: u32, local_offset: Vec3, rotation: Quat) {
    let step = std::f32::consts::TAU / WIRE_SEGMENTS as f32;
    for index in 0..WIRE_SEGMENTS {
        let a0 = index as f32 * step;
        let a1 = (index + 1) as f32 * step;
        let start = center + rotation.rotate(local_offset + circle_offset(plane, a0, radius));
        let end = center + rotation.rotate(local_offset + circle_offset(plane, a1, radius));
        out.push((start, end));
    }
}

fn circle_offset(plane: u32, angle: f32, radius: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    match plane {
        0 => Vec3::new(c, 0.0, s) * radius,
        1 => Vec3::new(c, s, 0.0) * radius,
        _ => Vec3::new(0.0, c, s) * radius,
    }
}

fn aabb_overlaps(a: &Aabb, b: &Aabb) -> bool {
    a.mins.x <= b.maxs.x
        && b.mins.x <= a.maxs.x
        && a.mins.y <= b.maxs.y
        && b.mins.y <= a.maxs.y
        && a.mins.z <= b.maxs.z
        && b.mins.z <= a.maxs.z
}

fn segment_aabb(origin: Vec3, end: Vec3) -> Aabb {
    Aabb {
        mins: Vector::new(origin.x.min(end.x), origin.y.min(end.y), origin.z.min(end.z)),
        maxs: Vector::new(origin.x.max(end.x), origin.y.max(end.y), origin.z.max(end.z)),
    }
}

fn from_vector(value: Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

fn finite_unit(value: Vec3) -> Option<Vec3> {
    if !point_finite(value) {
        return None;
    }
    let length = value.length();
    if !length.is_finite() || length < 1e-6 {
        return None;
    }
    Some(value / length)
}
