//! Closest-feature gap and wall normal for the kinematic skin band.
//!
//! The wall is world-space and points from the obstacle toward the mover. Shape-cast `normal1` is
//! mover-local and is not used here. A missing wall means the direction is degenerate: the caller
//! stops the move instead of substituting a cast normal.
//!
//! Sphere and capsule cores are the center and the local-Y segment. A cuboid core is the finite
//! box. Cuboid-cuboid uses Parry's SAT contact normal. Signed scale is already folded into the
//! indexed pose and extents.

use engine_math::{Quat, Vec3};
use parry3d::query::contact;
use parry3d::query::details::closest_points_segment_segment_with_locations;
use parry3d::shape::Segment;

use super::convert::{to_vector, Prepared};
use super::types::{point_finite, ColliderShape};

/// A direction shorter than this is not a usable feature normal.
const DIRECTION_MIN: f32 = 1e-6;
/// Face depths within this of the nearest face count as the same feature.
const FACE_TIE: f32 = 1e-5;
/// Contact prediction for cuboid pairs. A miss means the boxes are farther than this.
const BOX_PREDICTION: f32 = 1.0e6;

#[derive(Clone, Copy, Debug)]
pub(super) struct FeatureHit {
    pub gap: f32,
    pub wall: Option<Vec3>,
}

/// Surface gap and the wall normal at `mover_center`.
///
/// `None` means the inputs are non-finite or the pair cannot be queried. A hit with `wall: None`
/// still has a finite gap, but no safe blocking direction.
pub(super) fn feature_hit(mover: &Prepared, mover_center: Vec3, obstacle: &Prepared) -> Option<FeatureHit> {
    if !point_finite(mover_center) || !point_finite(obstacle.center) {
        return None;
    }
    match (mover.shape, obstacle.shape) {
        (ColliderShape::Sphere { radius: left }, ColliderShape::Sphere { radius: right }) => {
            cores(mover_center, obstacle.center, left + right, 0.0, None)
        }
        (ColliderShape::Sphere { radius: ball_radius }, ColliderShape::Capsule { radius, half_height }) => {
            ball_capsule(mover_center, ball_radius, obstacle.center, obstacle.rotation, radius, half_height, true)
        }
        (ColliderShape::Capsule { radius, half_height }, ColliderShape::Sphere { radius: ball_radius }) => {
            ball_capsule(obstacle.center, ball_radius, mover_center, mover.rotation, radius, half_height, false)
        }
        (
            ColliderShape::Capsule { radius: left, half_height: left_height },
            ColliderShape::Capsule { radius: right, half_height },
        ) => capsule_capsule(mover, mover_center, left, left_height, obstacle, right, half_height),
        (ColliderShape::Sphere { radius }, ColliderShape::Box { half_extents }) => {
            ball_box(mover_center, radius, obstacle.center, obstacle.rotation, half_extents, true)
        }
        (ColliderShape::Box { half_extents }, ColliderShape::Sphere { radius }) => {
            ball_box(obstacle.center, radius, mover_center, mover.rotation, half_extents, false)
        }
        (ColliderShape::Capsule { radius, half_height }, ColliderShape::Box { half_extents }) => capsule_box(
            SegmentBody { center: mover_center, rotation: mover.rotation, radius, half_height },
            BoxBody { center: obstacle.center, rotation: obstacle.rotation, half: half_extents },
            true,
        ),
        (ColliderShape::Box { half_extents }, ColliderShape::Capsule { radius, half_height }) => capsule_box(
            SegmentBody { center: obstacle.center, rotation: obstacle.rotation, radius, half_height },
            BoxBody { center: mover_center, rotation: mover.rotation, half: half_extents },
            false,
        ),
        (ColliderShape::Box { .. }, ColliderShape::Box { .. }) => box_box(mover, mover_center, obstacle),
    }
}

fn ball_capsule(
    ball_center: Vec3,
    ball_radius: f32,
    capsule_center: Vec3,
    capsule_rotation: Quat,
    radius: f32,
    half_height: f32,
    ball_is_mover: bool,
) -> Option<FeatureHit> {
    let (start, end) = capsule_segment(capsule_center, capsule_rotation, half_height)?;
    let on_capsule = project_on_segment(ball_center, start, end)?;
    let (on_mover, on_obstacle) = if ball_is_mover { (ball_center, on_capsule) } else { (on_capsule, ball_center) };
    cores(on_mover, on_obstacle, ball_radius + radius, 0.0, None)
}

fn capsule_capsule(
    mover: &Prepared,
    mover_center: Vec3,
    mover_radius: f32,
    mover_height: f32,
    obstacle: &Prepared,
    obstacle_radius: f32,
    obstacle_height: f32,
) -> Option<FeatureHit> {
    let (a, b) = capsule_segment(mover_center, mover.rotation, mover_height)?;
    let (c, d) = capsule_segment(obstacle.center, obstacle.rotation, obstacle_height)?;
    let (on_mover, on_obstacle) = closest_capsule_segments(a, b, c, d)?;
    cores(on_mover, on_obstacle, mover_radius + obstacle_radius, 0.0, None)
}

fn ball_box(
    ball_center: Vec3,
    radius: f32,
    box_center: Vec3,
    box_rotation: Quat,
    half: Vec3,
    ball_is_mover: bool,
) -> Option<FeatureHit> {
    if !radius.is_finite() || radius <= 0.0 || !half_ok(half) {
        return None;
    }
    let local = box_rotation.inverse().rotate(ball_center - box_center);
    if !point_finite(local) {
        return None;
    }
    let clamped = local.clamp(-half, half);
    let toward_ball = local - clamped;
    let on_box = box_center + box_rotation.rotate(clamped);
    let (on_mover, on_obstacle, fallback) = if ball_is_mover {
        (ball_center, on_box, face_toward_mover(local, half, box_rotation, true))
    } else {
        (on_box, ball_center, face_toward_mover(local, half, box_rotation, false))
    };
    let interior = if toward_ball.length() <= DIRECTION_MIN { interior_depth(local, half) } else { 0.0 };
    cores(on_mover, on_obstacle, radius, interior, fallback)
}

struct SegmentBody {
    center: Vec3,
    rotation: Quat,
    radius: f32,
    half_height: f32,
}

struct BoxBody {
    center: Vec3,
    rotation: Quat,
    half: Vec3,
}

fn capsule_box(capsule: SegmentBody, cube: BoxBody, capsule_is_mover: bool) -> Option<FeatureHit> {
    if !capsule.radius.is_finite() || capsule.radius <= 0.0 || !half_ok(cube.half) {
        return None;
    }
    let (start, end) = capsule_segment(capsule.center, capsule.rotation, capsule.half_height)?;
    let inverse = cube.rotation.inverse();
    let local_start = inverse.rotate(start - cube.center);
    let local_end = inverse.rotate(end - cube.center);
    if !point_finite(local_start) || !point_finite(local_end) {
        return None;
    }
    let (local_capsule, local_box) = closest_segment_aabb(arr(local_start), arr(local_end), arr(cube.half));
    let on_capsule = cube.center + cube.rotation.rotate(from_arr(local_capsule));
    let on_box = cube.center + cube.rotation.rotate(from_arr(local_box));
    let local_point = from_arr(local_capsule);
    let interior =
        if (on_capsule - on_box).length() <= DIRECTION_MIN { interior_depth(local_point, cube.half) } else { 0.0 };
    let fallback = face_toward_mover(local_point, cube.half, cube.rotation, capsule_is_mover);
    let (on_mover, on_obstacle) = if capsule_is_mover { (on_capsule, on_box) } else { (on_box, on_capsule) };
    cores(on_mover, on_obstacle, capsule.radius, interior, fallback)
}

fn box_box(mover: &Prepared, mover_center: Vec3, obstacle: &Prepared) -> Option<FeatureHit> {
    let mover_pose = pose_at(mover.rotation, mover_center);
    let touch =
        match contact(&mover_pose, mover.parry.as_shape(), &obstacle.pose, obstacle.parry.as_shape(), BOX_PREDICTION) {
            Ok(Some(touch)) => touch,
            Ok(None) => return Some(FeatureHit { gap: BOX_PREDICTION, wall: None }),
            Err(_) => return None,
        };
    if !touch.dist.is_finite() {
        return None;
    }
    // `contact` writes `normal1` in world space, from the mover toward the obstacle.
    let toward_obstacle = Vec3::new(touch.normal1.x, touch.normal1.y, touch.normal1.z);
    Some(FeatureHit { gap: touch.dist, wall: unit(-toward_obstacle) })
}

fn cores(on_mover: Vec3, on_obstacle: Vec3, radii: f32, interior: f32, fallback: Option<Vec3>) -> Option<FeatureHit> {
    let toward = on_mover - on_obstacle;
    if !point_finite(toward) || !radii.is_finite() || radii < 0.0 || !interior.is_finite() {
        return None;
    }
    let length = toward.length();
    if length > DIRECTION_MIN {
        let gap = length - radii;
        if !gap.is_finite() {
            return None;
        }
        return Some(FeatureHit { gap, wall: Some(toward / length) });
    }
    let gap = -(interior + radii);
    if !gap.is_finite() {
        return None;
    }
    Some(FeatureHit { gap, wall: fallback.and_then(unit) })
}

fn face_toward_mover(local: Vec3, half: Vec3, box_rotation: Quat, rounded_is_mover: bool) -> Option<Vec3> {
    let outward = face_outward(local, half)?;
    let world = box_rotation.rotate(outward);
    Some(if rounded_is_mover { world } else { -world })
}

fn face_outward(local: Vec3, half: Vec3) -> Option<Vec3> {
    let depths =
        [half.x - local.x, half.x + local.x, half.y - local.y, half.y + local.y, half.z - local.z, half.z + local.z];
    let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    let min = depths.into_iter().fold(f32::MAX, f32::min);
    if !min.is_finite() {
        return None;
    }
    let mut sum = Vec3::ZERO;
    let mut count = 0;
    for (depth, axis) in depths.iter().zip(axes) {
        if *depth <= min + FACE_TIE {
            sum += axis;
            count += 1;
        }
    }
    if count == 0 || count > 3 {
        return None;
    }
    unit(sum)
}

fn interior_depth(local: Vec3, half: Vec3) -> f32 {
    let depth = (half.x - local.x.abs()).min(half.y - local.y.abs()).min(half.z - local.z.abs());
    if depth.is_finite() {
        depth.max(0.0)
    } else {
        0.0
    }
}

fn capsule_segment(center: Vec3, rotation: Quat, half_height: f32) -> Option<(Vec3, Vec3)> {
    if !half_height.is_finite() || half_height < 0.0 {
        return None;
    }
    let axis = rotation.rotate(Vec3::Y) * half_height;
    if !point_finite(axis) {
        return None;
    }
    Some((center - axis, center + axis))
}

fn project_on_segment(point: Vec3, start: Vec3, end: Vec3) -> Option<Vec3> {
    let along = end - start;
    let denom = along.length_squared();
    if !denom.is_finite() {
        return None;
    }
    if denom <= DIRECTION_MIN * DIRECTION_MIN {
        return Some(start);
    }
    let t = ((point - start).dot(along) / denom).clamp(0.0, 1.0);
    if !t.is_finite() {
        return None;
    }
    Some(start + along * t)
}

fn closest_capsule_segments(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Option<(Vec3, Vec3)> {
    let left = Segment::new(to_vector(a), to_vector(b));
    let right = Segment::new(to_vector(c), to_vector(d));
    let (left_at, right_at) =
        closest_points_segment_segment_with_locations(&parry3d::math::Pose::identity(), &left, &right);
    let on_left = from_vector(left.point_at(&left_at));
    let on_right = from_vector(right.point_at(&right_at));
    if point_finite(on_left) && point_finite(on_right) {
        Some((on_left, on_right))
    } else {
        None
    }
}

fn closest_segment_aabb(start: [f32; 3], end: [f32; 3], half: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let direction = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let mut breaks = [0.0; 8];
    let mut count = 2;
    breaks[0] = 0.0;
    breaks[1] = 1.0;
    for axis in 0..3 {
        if direction[axis].abs() <= 1e-20 {
            continue;
        }
        for bound in [-half[axis], half[axis]] {
            let t = (bound - start[axis]) / direction[axis];
            if t.is_finite() && (0.0..1.0).contains(&t) && count < breaks.len() {
                breaks[count] = t;
                count += 1;
            }
        }
    }
    sort_prefix(&mut breaks, count);
    let mut samples = [0.0; 16];
    samples[..count].copy_from_slice(&breaks[..count]);
    let mut sample_count = count;
    for window in 0..count.saturating_sub(1) {
        if let Some(t) = critical_t(start, direction, half, breaks[window], breaks[window + 1]) {
            if sample_count < samples.len() {
                samples[sample_count] = t;
                sample_count += 1;
            }
        }
    }
    let mut best_t = 0.0;
    let mut best = f32::MAX;
    for t in samples.iter().take(sample_count).copied() {
        if !t.is_finite() {
            continue;
        }
        let point = add_scaled(start, direction, t);
        let clamped = clamp_aabb(point, half);
        let distance = dist_squared(point, clamped);
        if distance < best {
            best = distance;
            best_t = t;
        }
    }
    let point = add_scaled(start, direction, best_t);
    (point, clamp_aabb(point, half))
}

fn critical_t(start: [f32; 3], direction: [f32; 3], half: [f32; 3], t0: f32, t1: f32) -> Option<f32> {
    let mid = add_scaled(start, direction, (t0 + t1) * 0.5);
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    let mut outside = false;
    for axis in 0..3 {
        let bound = if mid[axis] > half[axis] {
            half[axis]
        } else if mid[axis] < -half[axis] {
            -half[axis]
        } else {
            continue;
        };
        outside = true;
        numerator += (bound - start[axis]) * direction[axis];
        denominator += direction[axis] * direction[axis];
    }
    if !outside || denominator <= 1e-20 {
        return None;
    }
    let t = numerator / denominator;
    (t.is_finite() && t > t0 && t < t1).then_some(t)
}

fn pose_at(rotation: Quat, center: Vec3) -> parry3d::math::Pose {
    parry3d::math::Pose::from_parts(
        to_vector(center),
        parry3d::math::Rotation::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w),
    )
}

fn unit(value: Vec3) -> Option<Vec3> {
    if !point_finite(value) {
        return None;
    }
    let length = value.length();
    if !length.is_finite() || length < DIRECTION_MIN {
        None
    } else {
        Some(value / length)
    }
}

fn half_ok(half: Vec3) -> bool {
    point_finite(half) && half.x > 0.0 && half.y > 0.0 && half.z > 0.0
}

fn from_vector(value: parry3d::math::Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

fn arr(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

fn from_arr(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

fn add_scaled(start: [f32; 3], direction: [f32; 3], t: f32) -> [f32; 3] {
    [start[0] + direction[0] * t, start[1] + direction[1] * t, start[2] + direction[2] * t]
}

fn clamp_aabb(point: [f32; 3], half: [f32; 3]) -> [f32; 3] {
    [point[0].clamp(-half[0], half[0]), point[1].clamp(-half[1], half[1]), point[2].clamp(-half[2], half[2])]
}

fn dist_squared(left: [f32; 3], right: [f32; 3]) -> f32 {
    let x = left[0] - right[0];
    let y = left[1] - right[1];
    let z = left[2] - right[2];
    x * x + y * y + z * z
}

fn sort_prefix(values: &mut [f32], count: usize) {
    for index in 1..count {
        let key = values[index];
        let mut hole = index;
        while hole > 0 && values[hole - 1] > key {
            values[hole] = values[hole - 1];
            hole -= 1;
        }
        values[hole] = key;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_math::{Mat4, Quat};

    fn prepared(shape: ColliderShape, center: Vec3, rotation: Quat) -> Prepared {
        let matrix = Mat4::from_translation(center) * Mat4::from_quat(rotation);
        super::super::convert::prepare(shape, Vec3::ZERO, &matrix).expect("prepared shape")
    }

    fn wall_of(mover: &Prepared, obstacle: &Prepared) -> FeatureHit {
        feature_hit(mover, mover.center, obstacle).expect("feature")
    }

    #[test]
    fn a_huge_face_normal_stays_on_the_face_axis() {
        let sphere = prepared(ColliderShape::Sphere { radius: 0.5 }, Vec3::new(0.5, 0.0, 7_999.0), Quat::IDENTITY);
        let wall = prepared(
            ColliderShape::Box { half_extents: Vec3::new(1.0, 4.0, 8_000.0) },
            Vec3::new(2.0, 0.0, 0.0),
            Quat::IDENTITY,
        );
        let hit = wall_of(&sphere, &wall);
        let wall_normal = hit.wall.expect("wall");
        assert!(wall_normal.approx_eq(-Vec3::X, 1e-5), "normal {wall_normal:?}");
        assert!(hit.gap.abs() < 1e-4, "gap {}", hit.gap);
    }

    #[test]
    fn a_box_corner_normal_is_diagonal_and_reverses_with_the_mover() {
        let sphere = prepared(ColliderShape::Sphere { radius: 0.5 }, Vec3::new(2.0, 0.0, 2.0), Quat::IDENTITY);
        let cube = prepared(ColliderShape::Box { half_extents: Vec3::splat(1.0) }, Vec3::ZERO, Quat::IDENTITY);
        let forward = wall_of(&sphere, &cube);
        let backward = wall_of(&cube, &sphere);
        let expected = Vec3::new(1.0, 0.0, 1.0).normalized();
        assert!(forward.wall.expect("wall").approx_eq(expected, 1e-5), "{forward:?}");
        assert!(backward.wall.expect("wall").approx_eq(-expected, 1e-5), "{backward:?}");
        let gap = std::f32::consts::SQRT_2 - 0.5;
        assert!((forward.gap - gap).abs() < 1e-4, "gap {}", forward.gap);
        assert!((backward.gap - gap).abs() < 1e-4, "gap {}", backward.gap);
    }

    #[test]
    fn sphere_capsule_and_capsule_capsule_normals_point_at_the_mover() {
        let sphere = prepared(ColliderShape::Sphere { radius: 0.4 }, Vec3::ZERO, Quat::IDENTITY);
        let capsule = prepared(
            ColliderShape::Capsule { radius: 0.3, half_height: 0.8 },
            Vec3::new(1.2, 0.0, 0.0),
            Quat::IDENTITY,
        );
        let other = prepared(
            ColliderShape::Capsule { radius: 0.25, half_height: 0.5 },
            Vec3::new(0.0, 0.0, 1.4),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        );
        let sphere_capsule = wall_of(&sphere, &capsule);
        assert!(sphere_capsule.wall.expect("wall").approx_eq(-Vec3::X, 1e-4), "{sphere_capsule:?}");
        let capsule_sphere = wall_of(&capsule, &sphere);
        assert!(capsule_sphere.wall.expect("wall").approx_eq(Vec3::X, 1e-4), "{capsule_sphere:?}");
        let along_z = wall_of(&sphere, &other);
        assert!(along_z.wall.expect("wall").approx_eq(-Vec3::Z, 1e-4), "{along_z:?}");
        let pair = wall_of(&capsule, &other);
        let pair_wall = pair.wall.expect("wall");
        assert!(pair_wall.x > 0.5 && pair_wall.z < -0.5, "capsule pair {pair:?}");
    }

    #[test]
    fn cuboid_contact_normal_is_world_space_and_follows_the_face() {
        let left = prepared(ColliderShape::Box { half_extents: Vec3::splat(0.5) }, Vec3::ZERO, Quat::IDENTITY);
        let right =
            prepared(ColliderShape::Box { half_extents: Vec3::splat(0.5) }, Vec3::new(1.2, 0.0, 0.0), Quat::IDENTITY);
        let hit = wall_of(&left, &right);
        assert!(hit.wall.expect("wall").approx_eq(-Vec3::X, 1e-4), "{hit:?}");
        assert!((hit.gap - 0.2).abs() < 1e-4, "gap {}", hit.gap);

        let yaw = Quat::from_rotation_y(30.0_f32.to_radians());
        let normal = yaw.rotate(Vec3::X);
        let rotated_left = prepared(ColliderShape::Box { half_extents: Vec3::splat(0.5) }, Vec3::ZERO, yaw);
        let rotated_right = prepared(ColliderShape::Box { half_extents: Vec3::splat(0.5) }, normal * 1.2, yaw);
        let rotated = wall_of(&rotated_left, &rotated_right);
        assert!(rotated.wall.expect("wall").approx_eq(-normal, 1e-4), "{rotated:?}");
        assert!((rotated.gap - 0.2).abs() < 1e-3, "gap {}", rotated.gap);
        let flipped = wall_of(&rotated_right, &rotated_left);
        assert!(flipped.wall.expect("wall").approx_eq(normal, 1e-4), "{flipped:?}");
    }

    #[test]
    fn coincident_sphere_centers_have_no_feature_normal() {
        let left = prepared(ColliderShape::Sphere { radius: 0.5 }, Vec3::ZERO, Quat::IDENTITY);
        let right = prepared(ColliderShape::Sphere { radius: 0.5 }, Vec3::ZERO, Quat::IDENTITY);
        let hit = wall_of(&left, &right);
        assert!(hit.wall.is_none(), "{hit:?}");
        assert!((hit.gap + 1.0).abs() < 1e-4, "gap {}", hit.gap);
    }
}
