//! Single-frame world-space debug lines.
//!
//! [`DebugDraw`] is a world resource, not a set of entities and not a second scene. Gameplay and,
//! later, editor tools submit primitives here. [`extract_scene`](crate::extract_scene) copies the
//! segments into [`RenderScene`](crate::RenderScene) and does not clear the resource, so every
//! camera in that frame sees the same list. `Game::step` clears the resource at the start of the
//! next step, before systems run.
//!
//! A primitive with a non-finite coordinate, a non-finite color, a zero length, or an invalid
//! size is omitted. This module does not emit a non-finite vertex.
//!
//! The GPU backend draws each segment as a one-pixel line. Thickness is not a parameter.

use engine_core::Color;
use engine_math::{Mat4, Quat, Vec3, Vec4};

/// Edges per great circle used by [`DebugDraw::wire_sphere`].
pub const DEBUG_SPHERE_SEGMENTS: u32 = 16;

/// Upper bound on edges per great circle. Larger requests are clamped so a caller cannot allocate
/// without limit.
pub const DEBUG_SPHERE_SEGMENT_LIMIT: u32 = 128;

const BOX_EDGES: [(usize, usize); 12] =
    [(0, 1), (1, 3), (3, 2), (2, 0), (4, 5), (5, 7), (7, 6), (6, 4), (0, 4), (1, 5), (2, 6), (3, 7)];

/// One finite world-space segment and a linear RGBA color.
///
/// This is extracted render data. It is not an entity, and it is not stored in a scene file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugSegment {
    pub start: Vec3,
    pub end: Vec3,
    pub color: [f32; 4],
}

/// Segments submitted for the current frame.
///
/// Lines, rays, wire boxes, wire spheres, frusta, and axis helpers all become segments in this
/// one list. The renderer batches that list into one line draw per camera. An empty list adds no
/// draw.
///
/// ```
/// use engine_core::Color;
/// use engine_math::Vec3;
/// use engine_renderer::DebugDraw;
///
/// let mut debug = DebugDraw::default();
/// debug.line(Vec3::ZERO, Vec3::Y, Color::rgb(0.0, 1.0, 0.0));
/// debug.wire_box(Vec3::ZERO, Vec3::splat(0.5), Color::WHITE);
/// assert!(!debug.is_empty());
/// ```
#[derive(Clone, Debug, Default)]
pub struct DebugDraw {
    segments: Vec<DebugSegment>,
}

impl DebugDraw {
    /// A segment from `start` to `end`. A zero-length or non-finite segment is omitted.
    pub fn line(&mut self, start: Vec3, end: Vec3, color: Color) {
        self.push_segment(start, end, color);
    }

    /// A segment from `origin` along `direction` for `length` world units.
    ///
    /// `direction` is normalized. A zero, non-finite, or non-positive length, or a direction that
    /// cannot be normalized, omits the ray.
    pub fn ray(&mut self, origin: Vec3, direction: Vec3, length: f32, color: Color) {
        let Some(direction) = direction.try_normalize() else {
            return;
        };
        if !span_ok(length) || !point_ok(origin) {
            return;
        }
        self.push_segment(origin, origin + direction * length, color);
    }

    /// Twelve edges of an axis-aligned box. `half_extents` are the distances from `center` to the
    /// faces. A negative or non-finite component omits the whole box. A zero component drops only
    /// the edges that collapse, so a flat rectangle is still drawn.
    pub fn wire_box(&mut self, center: Vec3, half_extents: Vec3, color: Color) {
        self.wire_box_matrix(&Mat4::from_translation(center), half_extents, color);
    }

    /// Twelve edges of a box in the local space of `world_from_local`.
    ///
    /// `half_extents` are local. The matrix then places them, including any scale in that matrix.
    /// A negative or non-finite extent, or a non-finite matrix, omits the whole box.
    pub fn wire_box_matrix(&mut self, world_from_local: &Mat4, half_extents: Vec3, color: Color) {
        if !extents_ok(half_extents) || !matrix_ok(world_from_local) {
            return;
        }
        let corners = box_corners(world_from_local, half_extents);
        for (a, b) in BOX_EDGES {
            self.push_segment(corners[a], corners[b], color);
        }
    }

    /// Three great circles (XY, XZ, YZ) of [`DEBUG_SPHERE_SEGMENTS`] edges each.
    pub fn wire_sphere(&mut self, center: Vec3, radius: f32, color: Color) {
        self.wire_sphere_segments(center, radius, DEBUG_SPHERE_SEGMENTS, color);
    }

    /// Three great circles with `segments` edges each.
    ///
    /// Fewer than 3 segments, a non-positive radius, or a non-finite center omits the sphere.
    /// `segments` above [`DEBUG_SPHERE_SEGMENT_LIMIT`] is clamped to that limit.
    pub fn wire_sphere_segments(&mut self, center: Vec3, radius: f32, segments: u32, color: Color) {
        if segments < 3 || !span_ok(radius) || !point_ok(center) {
            return;
        }
        let segments = segments.min(DEBUG_SPHERE_SEGMENT_LIMIT);
        let step = core::f32::consts::TAU / segments as f32;
        for circle in 0..3 {
            for index in 0..segments {
                let a0 = index as f32 * step;
                let a1 = (index + 1) as f32 * step;
                self.push_segment(
                    center + circle_offset(circle, a0, radius),
                    center + circle_offset(circle, a1, radius),
                    color,
                );
            }
        }
    }

    /// Near and far rectangles of a camera, plus the edges that join them.
    ///
    /// `view_proj` is `projection * view` in Dimora's clip convention: NDC depth 1 is the near
    /// plane and NDC depth 0 is the far plane. A singular or non-finite matrix, or a corner that
    /// does not unproject to a finite point, adds nothing.
    pub fn frustum(&mut self, view_proj: &Mat4, color: Color) {
        let Some(corners) = frustum_corners(view_proj) else {
            return;
        };
        for (a, b) in BOX_EDGES {
            self.push_segment(corners[a], corners[b], color);
        }
    }

    /// X, Y, and Z axes through `origin`, rotated by `rotation`, each `length` world units.
    ///
    /// Colors are fixed: X red, Y green, Z blue. There are no arrowheads. A non-positive or
    /// non-finite length omits all three.
    pub fn axes(&mut self, origin: Vec3, rotation: Quat, length: f32) {
        if !span_ok(length) || !point_ok(origin) {
            return;
        }
        self.line(origin, origin + rotation.rotate(Vec3::X * length), Color::rgb(1.0, 0.0, 0.0));
        self.line(origin, origin + rotation.rotate(Vec3::Y * length), Color::rgb(0.0, 1.0, 0.0));
        self.line(origin, origin + rotation.rotate(Vec3::Z * length), Color::rgb(0.0, 0.0, 1.0));
    }

    /// Drops every segment. `Game::step` does this at the start of a step. Extraction does not.
    pub fn clear(&mut self) {
        self.segments.clear();
    }

    /// Segments accepted so far, in submission order.
    pub fn segments(&self) -> &[DebugSegment] {
        &self.segments
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    fn push_segment(&mut self, start: Vec3, end: Vec3, color: Color) {
        let color = color.to_array();
        if !point_ok(start) || !point_ok(end) || color.iter().any(|channel| !channel.is_finite()) {
            return;
        }
        if start.distance_squared(end) == 0.0 {
            return;
        }
        self.segments.push(DebugSegment { start, end, color });
    }
}

/// Eight world-space corners of the frustum encoded by `view_proj`.
///
/// Index bit 0 is NDC x = +1, bit 1 is NDC y = +1, and bit 2 is the far plane (NDC z = 0).
/// Near corners use NDC z = 1, which is Dimora's reverse-Z near plane. `None` when the matrix
/// is non-finite, has no inverse, or a corner is not a finite world position.
pub fn frustum_corners(view_proj: &Mat4) -> Option<[Vec3; 8]> {
    if !matrix_ok(view_proj) {
        return None;
    }
    let inv = view_proj.inverse()?;
    let mut corners = [Vec3::ZERO; 8];
    for (index, corner) in corners.iter_mut().enumerate() {
        let x = if index & 1 == 0 { -1.0 } else { 1.0 };
        let y = if index & 2 == 0 { -1.0 } else { 1.0 };
        let z = if index & 4 == 0 { 1.0 } else { 0.0 };
        let clip = inv * Vec4::new(x, y, z, 1.0);
        if !clip.w.is_finite() || clip.w.abs() <= engine_math::EPSILON {
            return None;
        }
        let world = clip.xyz() / clip.w;
        if !point_ok(world) {
            return None;
        }
        *corner = world;
    }
    Some(corners)
}

fn box_corners(world_from_local: &Mat4, half: Vec3) -> [Vec3; 8] {
    let mut corners = [Vec3::ZERO; 8];
    for (index, corner) in corners.iter_mut().enumerate() {
        let local = Vec3::new(
            if index & 1 == 0 { -half.x } else { half.x },
            if index & 2 == 0 { -half.y } else { half.y },
            if index & 4 == 0 { -half.z } else { half.z },
        );
        *corner = world_from_local.transform_point3(local);
    }
    corners
}

fn circle_offset(circle: u32, angle: f32, radius: f32) -> Vec3 {
    let (sin, cos) = angle.sin_cos();
    let (x, y) = (cos * radius, sin * radius);
    match circle {
        0 => Vec3::new(x, y, 0.0),
        1 => Vec3::new(x, 0.0, y),
        _ => Vec3::new(0.0, x, y),
    }
}

fn point_ok(point: Vec3) -> bool {
    point.x.is_finite() && point.y.is_finite() && point.z.is_finite()
}

fn span_ok(length: f32) -> bool {
    length.is_finite() && length > 0.0
}

fn extents_ok(half: Vec3) -> bool {
    half.x.is_finite() && half.y.is_finite() && half.z.is_finite() && half.x >= 0.0 && half.y >= 0.0 && half.z >= 0.0
}

fn matrix_ok(matrix: &Mat4) -> bool {
    matrix
        .cols
        .iter()
        .all(|column| column.x.is_finite() && column.y.is_finite() && column.z.is_finite() && column.w.is_finite())
}

#[cfg(test)]
mod tests {
    use core::f32::consts::FRAC_PI_2;

    use engine_math::{Quat, Transform, Vec2};
    use engine_world::GlobalTransform;

    use super::*;
    use crate::camera::{Camera, CameraView};
    use crate::projection::CameraProjection;

    fn close(got: Vec3, want: Vec3) {
        assert!(got.approx_eq(want, 1e-4), "{got:?} != {want:?}");
    }

    fn has_endpoint(draw: &DebugDraw, point: Vec3) -> bool {
        draw.segments().iter().any(|segment| segment.start.approx_eq(point, 1e-4) || segment.end.approx_eq(point, 1e-4))
    }

    fn view_at(projection: CameraProjection, transform: Transform) -> CameraView {
        let global = GlobalTransform { matrix: transform.to_matrix() };
        CameraView::new(&Camera::new(projection), &global, Vec2::splat(128.0))
    }

    #[test]
    fn line_and_ray_keep_finite_segments_only() {
        let mut draw = DebugDraw::default();
        draw.line(Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), Color::rgba(1.0, 0.0, 0.0, 0.0));
        draw.line(Vec3::ONE, Vec3::ONE, Color::WHITE);
        draw.line(Vec3::new(f32::NAN, 0.0, 0.0), Vec3::X, Color::WHITE);
        draw.line(Vec3::ZERO, Vec3::new(f32::INFINITY, 0.0, 0.0), Color::WHITE);
        draw.line(Vec3::ZERO, Vec3::X, Color::rgba(f32::NAN, 0.0, 0.0, 1.0));
        draw.ray(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 4.0, 0.0), 3.0, Color::WHITE);
        draw.ray(Vec3::ZERO, Vec3::ZERO, 1.0, Color::WHITE);
        draw.ray(Vec3::ZERO, Vec3::X, 0.0, Color::WHITE);
        draw.ray(Vec3::ZERO, Vec3::X, -2.0, Color::WHITE);
        draw.ray(Vec3::ZERO, Vec3::X, f32::INFINITY, Color::WHITE);
        draw.ray(Vec3::new(f32::NAN, 0.0, 0.0), Vec3::X, 1.0, Color::WHITE);

        assert_eq!(draw.len(), 2);
        close(draw.segments()[0].start, Vec3::ZERO);
        close(draw.segments()[0].end, Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(draw.segments()[0].color, [1.0, 0.0, 0.0, 0.0]);
        close(draw.segments()[1].start, Vec3::new(0.0, 1.0, 0.0));
        close(draw.segments()[1].end, Vec3::new(0.0, 4.0, 0.0));
    }

    #[test]
    fn wire_box_edges_meet_the_eight_corners() {
        let center = Vec3::new(4.0, 5.0, 6.0);
        let half = Vec3::new(1.0, 2.0, 3.0);
        let mut draw = DebugDraw::default();
        draw.wire_box(center, half, Color::rgb(0.2, 0.4, 0.6));
        assert_eq!(draw.len(), 12);
        assert!(draw.segments().iter().all(|segment| segment.color == [0.2, 0.4, 0.6, 1.0]));

        let mut corners = [Vec3::ZERO; 8];
        for (index, corner) in corners.iter_mut().enumerate() {
            *corner = center
                + Vec3::new(
                    if index & 1 == 0 { -half.x } else { half.x },
                    if index & 2 == 0 { -half.y } else { half.y },
                    if index & 4 == 0 { -half.z } else { half.z },
                );
            assert!(has_endpoint(&draw, *corner), "missing {index}");
        }
        for segment in draw.segments() {
            let mut matched = false;
            for (a, b) in BOX_EDGES {
                let forward = segment.start.approx_eq(corners[a], 1e-4) && segment.end.approx_eq(corners[b], 1e-4);
                let backward = segment.start.approx_eq(corners[b], 1e-4) && segment.end.approx_eq(corners[a], 1e-4);
                matched |= forward || backward;
            }
            assert!(matched, "unexpected edge {:?} -> {:?}", segment.start, segment.end);
        }
    }

    #[test]
    fn wire_box_follows_a_transform_and_rejects_bad_extents() {
        let matrix = Mat4::from_trs(Vec3::new(3.0, 4.0, 5.0), Quat::from_rotation_y(0.4), Vec3::new(2.0, 1.0, 0.5));
        let half = Vec3::new(1.0, 2.0, 3.0);
        let mut draw = DebugDraw::default();
        draw.wire_box_matrix(&matrix, half, Color::WHITE);
        assert_eq!(draw.len(), 12);
        let world = matrix.transform_point3(half);
        assert!(has_endpoint(&draw, world));

        draw.clear();
        draw.wire_box(Vec3::ZERO, Vec3::new(-1.0, 1.0, 1.0), Color::WHITE);
        draw.wire_box(Vec3::ZERO, Vec3::new(1.0, f32::NAN, 1.0), Color::WHITE);
        draw.wire_box(Vec3::new(f32::INFINITY, 0.0, 0.0), Vec3::ONE, Color::WHITE);
        draw.wire_box(Vec3::ZERO, Vec3::ZERO, Color::WHITE);
        assert!(draw.is_empty());

        draw.wire_box(Vec3::new(0.0, 0.0, 9.0), Vec3::new(1.0, 2.0, 0.0), Color::WHITE);
        assert_eq!(draw.len(), 8, "the two faces collapse onto one rectangle and the connectors drop");
        assert!(draw.segments().iter().all(|segment| (segment.start.z - 9.0).abs() < 1e-5));
        assert!(draw.segments().iter().all(|segment| segment.start.distance_squared(segment.end) > 0.0));
    }

    #[test]
    fn sphere_segments_are_deterministic_and_bounded() {
        let center = Vec3::new(1.0, 2.0, 3.0);
        let mut first = DebugDraw::default();
        first.wire_sphere(center, 2.0, Color::rgb(0.0, 1.0, 1.0));
        let mut second = DebugDraw::default();
        second.wire_sphere(center, 2.0, Color::rgb(0.0, 1.0, 1.0));
        assert_eq!(first.segments(), second.segments());
        assert_eq!(first.len(), DEBUG_SPHERE_SEGMENTS as usize * 3);
        assert!(has_endpoint(&first, center + Vec3::new(2.0, 0.0, 0.0)));
        assert!(has_endpoint(&first, center + Vec3::new(0.0, 2.0, 0.0)));
        assert!(has_endpoint(&first, center + Vec3::new(0.0, 0.0, 2.0)));
        let chord = 2.0 * 2.0 * (core::f32::consts::PI / DEBUG_SPHERE_SEGMENTS as f32).sin();
        for segment in first.segments() {
            let length = segment.start.distance(segment.end);
            assert!((length - chord).abs() < 1e-4, "{length} != {chord}");
        }

        let mut draw = DebugDraw::default();
        draw.wire_sphere(center, 0.0, Color::WHITE);
        draw.wire_sphere(center, -1.0, Color::WHITE);
        draw.wire_sphere(center, f32::NAN, Color::WHITE);
        draw.wire_sphere(Vec3::new(0.0, f32::INFINITY, 0.0), 1.0, Color::WHITE);
        draw.wire_sphere_segments(center, 1.0, 2, Color::WHITE);
        assert!(draw.is_empty());
        draw.wire_sphere_segments(center, 1.0, 3, Color::WHITE);
        assert_eq!(draw.len(), 9);
        draw.clear();
        draw.wire_sphere_segments(center, 1.0, u32::MAX, Color::WHITE);
        assert_eq!(draw.len(), DEBUG_SPHERE_SEGMENT_LIMIT as usize * 3);
    }

    #[test]
    fn frustum_uses_reverse_z_near_and_far() {
        let ortho = view_at(CameraProjection::orthographic(2.0).with_clip_range(1.0, 5.0), Transform::IDENTITY);
        let corners = frustum_corners(&ortho.view_proj).expect("ortho inverse");
        let expected = [
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, -1.0, 5.0),
            Vec3::new(1.0, -1.0, 5.0),
            Vec3::new(-1.0, 1.0, 5.0),
            Vec3::new(1.0, 1.0, 5.0),
        ];
        for (index, want) in expected.iter().enumerate() {
            close(corners[index], *want);
        }

        let perspective = view_at(CameraProjection::perspective(90.0).with_clip_range(1.0, 4.0), Transform::IDENTITY);
        let corners = frustum_corners(&perspective.view_proj).expect("perspective inverse");
        close(corners[0], Vec3::new(-1.0, -1.0, 1.0));
        close(corners[3], Vec3::new(1.0, 1.0, 1.0));
        close(corners[4], Vec3::new(-4.0, -4.0, 4.0));
        close(corners[7], Vec3::new(4.0, 4.0, 4.0));

        let custom = CameraProjection::custom(perspective.proj).expect("finite");
        let custom_view = view_at(custom, Transform::IDENTITY);
        let custom_corners = frustum_corners(&custom_view.view_proj).expect("custom inverse");
        for (got, want) in custom_corners.iter().zip(corners) {
            close(*got, want);
        }

        let mut draw = DebugDraw::default();
        draw.frustum(&perspective.view_proj, Color::WHITE);
        assert_eq!(draw.len(), 12);
        draw.frustum(&Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::ZERO), Color::WHITE);
        let mut nan = Mat4::IDENTITY;
        nan.cols[2].z = f32::NAN;
        draw.frustum(&nan, Color::WHITE);
        assert_eq!(draw.len(), 12);
        assert!(frustum_corners(&nan).is_none());
    }

    #[test]
    fn rotated_camera_frustum_keeps_near_closer_than_far() {
        let projection = CameraProjection::orthographic(4.0).with_clip_range(0.5, 6.0);
        let transform = Transform::looking_at(Vec3::new(0.0, 5.0, 0.0), Vec3::ZERO, Vec3::Z);
        let view = view_at(projection, transform);
        let corners = frustum_corners(&view.view_proj).expect("top-down inverse");
        let camera = Vec3::new(0.0, 5.0, 0.0);
        let near = (corners[0] + corners[1] + corners[2] + corners[3]) * 0.25;
        let far = (corners[4] + corners[5] + corners[6] + corners[7]) * 0.25;
        assert!((near.distance(camera) - 0.5).abs() < 1e-3, "near {near:?}");
        assert!((far.distance(camera) - 6.0).abs() < 1e-3, "far {far:?}");
        assert!(near.distance(camera) < far.distance(camera));
    }

    #[test]
    fn axes_reuse_lines_with_fixed_colors() {
        let mut draw = DebugDraw::default();
        let origin = Vec3::new(1.0, 2.0, 3.0);
        draw.axes(origin, Quat::from_rotation_z(FRAC_PI_2), 2.0);
        assert_eq!(draw.len(), 3);
        close(draw.segments()[0].end, origin + Vec3::new(0.0, 2.0, 0.0));
        close(draw.segments()[1].end, origin + Vec3::new(-2.0, 0.0, 0.0));
        close(draw.segments()[2].end, origin + Vec3::Z * 2.0);
        assert_eq!(draw.segments()[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(draw.segments()[1].color, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(draw.segments()[2].color, [0.0, 0.0, 1.0, 1.0]);
        draw.clear();
        draw.axes(origin, Quat::IDENTITY, 0.0);
        draw.axes(origin, Quat::IDENTITY, f32::NAN);
        assert!(draw.is_empty());
    }
}
