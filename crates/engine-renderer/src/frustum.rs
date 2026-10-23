use engine_math::{Mat4, Vec3, Vec4};

/// The six planes bounding the visible volume, normals pointing inwards.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// Extracts inward planes from a world-to-clip matrix with NDC depth in `[0, 1]`.
    ///
    /// The six combinations are `r3 ± r0`, `r3 ± r1`, `r2` (far, NDC `z = 0`) and `r3 - r2`
    /// (near, NDC `z = w`). Reverse-Z puts the near plane at 1 and the far plane at 0, so those
    /// two depth planes swap names relative to a 0-near matrix. The visible volume is still
    /// `0 <= clip.z <= clip.w`. They match the clip cube used by [`crate::CameraProjection`],
    /// including a custom matrix that follows that convention.
    ///
    /// A plane is dropped when its xyz length is at most `1e-6` times that plane's own largest
    /// coefficient, or when a coefficient
    /// is non-finite. A dropped plane is stored as zero and does not cull: the sphere test fails
    /// open for that plane. No replacement plane is invented. Scaling a finite matrix by a
    /// non-zero constant leaves the ratio unchanged, so a valid plane is not dropped just because
    /// the homogeneous matrix was scaled. The ratio uses the plane's own coefficients, not one
    /// absolute epsilon and not the largest element of the whole matrix: a near plane can be
    /// small next to a wide orthographic X scale and still be a real half-space.
    ///
    /// Negating the whole matrix flips every half-space. After the homogeneous divide, NDC is
    /// unchanged, because both the clip numerator and `w` change sign. When the centre of the
    /// NDC cube reprojects with negative `w`, the raw planes are negated first so CPU culling
    /// matches the positive matrix. If that centre cannot be recovered, the historical plane
    /// orientation is kept and unusable planes still fail open.
    ///
    /// That correction is a CPU frustum result. It does not make a negative-scaled matrix render
    /// the same image. The GPU clips in homogeneous space before the divide, and a reversed `w`
    /// can reject or invert fragments even when the divided NDC point matches. Equal NDC and
    /// equal culling are not a claim of equal GPU clipping.
    pub fn from_view_proj(m: &Mat4) -> Self {
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        let sign = interior_clip_w_sign(m);
        let raw = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
        Self { planes: raw.map(|plane| usable_plane(plane * sign)) }
    }

    /// Conservative: false only if the sphere is entirely outside one plane.
    ///
    /// A zero plane, used for a dropped degenerate plane, does not reject the sphere.
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        self.planes.iter().all(|p| p.xyz().dot(center) + p.w >= -radius)
    }
}

/// A normal at or below this fraction of the plane's largest coefficient is not a half-space.
/// The test is unchanged when every coefficient is multiplied by the same non-zero scale.
const PLANE_NORMAL_RELATIVE_EPSILON: f32 = 1e-6;

fn usable_plane(plane: Vec4) -> Vec4 {
    if !finite(plane) {
        return Vec4::ZERO;
    }
    let length = plane.xyz().length();
    let coeff = plane.x.abs().max(plane.y.abs()).max(plane.z.abs()).max(plane.w.abs());
    if length <= coeff * PLANE_NORMAL_RELATIVE_EPSILON {
        return Vec4::ZERO;
    }
    let normalised = plane / length;
    if finite(normalised) {
        normalised
    } else {
        Vec4::ZERO
    }
}

fn finite(value: Vec4) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite()
}

/// `+1` when the NDC-cube centre reprojects with positive clip `w`, `-1` when it does not.
/// `+1` when the centre cannot be recovered, which leaves the historical plane orientation.
fn interior_clip_w_sign(m: &Mat4) -> f32 {
    let Some(inverse) = m.inverse() else {
        return 1.0;
    };
    let unprojected = inverse * Vec4::new(0.0, 0.0, 0.5, 1.0);
    if !finite(unprojected) || unprojected.w.abs() <= 1e-12 {
        return 1.0;
    }
    let world = unprojected.xyz() / unprojected.w;
    if !world.x.is_finite() || !world.y.is_finite() || !world.z.is_finite() {
        return 1.0;
    }
    let clip = m.mul_vec4(world.extend(1.0));
    if !clip.w.is_finite() || clip.w.abs() <= 1e-12 {
        1.0
    } else {
        clip.w.signum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{projection::Projection, CameraProjection};

    #[test]
    fn perspective_frustum_culls_outside_points() {
        let proj = CameraProjection::perspective(60.0).matrix(1.0);
        let f = Frustum::from_view_proj(&(proj * Mat4::IDENTITY));
        assert!(f.intersects_sphere(Vec3::new(0.0, 0.0, 10.0), 0.1));
        assert!(!f.intersects_sphere(Vec3::new(0.0, 0.0, -10.0), 0.1), "behind camera");
        assert!(!f.intersects_sphere(Vec3::new(0.0, 0.0, 5000.0), 0.1), "beyond far plane");
        assert!(!f.intersects_sphere(Vec3::new(100.0, 0.0, 10.0), 1.0), "far to the side");
        assert!(f.intersects_sphere(Vec3::new(100.0, 0.0, 10.0), 100.0), "large radius reaches in");
    }

    #[test]
    fn orthographic_frustum_is_a_box() {
        let proj = CameraProjection::orthographic(10.0).matrix(2.0);
        let f = Frustum::from_view_proj(&proj);
        assert!(f.intersects_sphere(Vec3::new(9.0, 4.0, 5.0), 0.1));
        assert!(!f.intersects_sphere(Vec3::new(11.0, 0.0, 5.0), 0.5));
    }

    fn same_culling(reference: &Frustum, other: &Frustum, samples: &[Vec3]) {
        for (center, radius) in samples.iter().map(|c| (*c, 0.2)) {
            assert_eq!(
                reference.intersects_sphere(center, radius),
                other.intersects_sphere(center, radius),
                "{center:?} radius {radius}"
            );
        }
    }

    fn scale_matrix(m: &Mat4, factor: f32) -> Mat4 {
        Mat4::from_cols(m.cols[0] * factor, m.cols[1] * factor, m.cols[2] * factor, m.cols[3] * factor)
    }

    #[test]
    fn equivalent_custom_projection_matches_builtin_frustum() {
        let samples = [
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 5000.0),
            Vec3::new(100.0, 0.0, 10.0),
            Vec3::new(9.0, 4.0, 5.0),
            Vec3::new(11.0, 0.0, 5.0),
        ];
        for builtin in [CameraProjection::perspective(60.0), CameraProjection::orthographic(10.0)] {
            let matrix = builtin.matrix(2.0);
            let custom = CameraProjection::custom(matrix).unwrap().matrix(9.0);
            let reference = Frustum::from_view_proj(&matrix);
            let other = Frustum::from_view_proj(&custom);
            same_culling(&reference, &other, &samples);
            for (plane, copied) in reference.planes.iter().zip(other.planes.iter()) {
                assert!((plane.x - copied.x).abs() < 1e-5, "{plane:?} vs {copied:?}");
                assert!((plane.y - copied.y).abs() < 1e-5);
                assert!((plane.z - copied.z).abs() < 1e-5);
                assert!((plane.w - copied.w).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn oblique_custom_frustum_keeps_visible_sphere() {
        let ortho = CameraProjection::orthographic(10.0).matrix(1.0);
        let shear = Mat4::from_cols(
            Vec4::new(1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec4::new(0.5, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        let oblique = CameraProjection::custom(ortho * shear).unwrap().matrix(1.0);
        let frustum = Frustum::from_view_proj(&oblique);
        // View x' = x + 0.5 z. Height 10, aspect 1: visible x' is [-5, 5].
        assert!(frustum.intersects_sphere(Vec3::new(0.0, 0.0, 4.0), 0.1), "sheared origin stays inside");
        assert!(!frustum.intersects_sphere(Vec3::new(4.0, 0.0, 4.0), 0.1), "x' = 6 is outside");
    }

    #[test]
    fn offcenter_custom_frustum_culls_outside_sphere() {
        let offcentre = Mat4::orthographic_lh(-1.0, 9.0, -5.0, 5.0, 0.1, 100.0);
        let frustum = Frustum::from_view_proj(&CameraProjection::custom(offcentre).unwrap().matrix(4.0));
        assert!(frustum.intersects_sphere(Vec3::new(4.0, 0.0, 5.0), 0.1), "centre of [-1, 9]");
        assert!(frustum.intersects_sphere(Vec3::new(-0.5, 0.0, 5.0), 0.1));
        assert!(!frustum.intersects_sphere(Vec3::new(-2.0, 0.0, 5.0), 0.2));
        assert!(!frustum.intersects_sphere(Vec3::new(10.0, 0.0, 5.0), 0.2));
    }

    /// Replaces NDC `z = 0` with a constant. Under reverse-Z that row is the far plane.
    /// The near plane (`r3 - r2`) is copied into the new `r3` so a centred ortho does not lose it.
    fn drop_ndc_zero_plane(matrix: &mut Mat4) {
        let r2 = [matrix.cols[0].z, matrix.cols[1].z, matrix.cols[2].z, matrix.cols[3].z];
        let r3 = [matrix.cols[0].w, matrix.cols[1].w, matrix.cols[2].w, matrix.cols[3].w];
        let near = [r3[0] - r2[0], r3[1] - r2[1], r3[2] - r2[2], r3[3] - r2[3]];
        matrix.cols[0].z = 0.0;
        matrix.cols[1].z = 0.0;
        matrix.cols[2].z = 0.0;
        matrix.cols[3].z = 1.0;
        matrix.cols[0].w = near[0];
        matrix.cols[1].w = near[1];
        matrix.cols[2].w = near[2];
        matrix.cols[3].w = near[3] + 1.0;
    }

    #[test]
    fn degenerate_far_plane_does_not_falsely_cull() {
        let mut matrix = Mat4::orthographic_lh(-5.0, 5.0, -5.0, 5.0, 0.1, 100.0);
        drop_ndc_zero_plane(&mut matrix);
        let frustum = Frustum::from_view_proj(&matrix);
        assert!(frustum.intersects_sphere(Vec3::new(0.0, 0.0, 5000.0), 0.1), "no usable far plane");
        assert!(!frustum.intersects_sphere(Vec3::new(100.0, 0.0, 5.0), 0.1), "the side plane still culls");
        assert!(frustum.planes.contains(&Vec4::ZERO), "the far plane was dropped");
    }

    /// Positive and negative scales must cull the same spheres. This is the CPU plane test only.
    /// A negative factor also flips clip `w`, so it is not evidence that the GPU image matches.
    #[test]
    fn scaled_equivalent_projection_preserves_culling() {
        let matrix = CameraProjection::perspective(60.0).matrix(1.5);
        let ortho = CameraProjection::orthographic(10.0).matrix(2.0);
        let samples = [
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 5000.0),
            Vec3::new(100.0, 0.0, 10.0),
            Vec3::new(1.0, 4.0, 8.0),
            Vec3::new(11.0, 0.0, 5.0),
        ];
        for source in [matrix, ortho] {
            let reference = Frustum::from_view_proj(&source);
            for factor in [1e-6, 1e-3, 3.5, 1e6, -0.25, -4.0] {
                same_culling(&reference, &Frustum::from_view_proj(&scale_matrix(&source, factor)), &samples);
            }
        }
    }

    #[test]
    fn invalid_plane_falls_back_conservatively() {
        let zero = Frustum::from_view_proj(&Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::ZERO));
        assert!(zero.intersects_sphere(Vec3::new(1.0e6, -1.0e6, 1.0e6), 0.0));
        assert!(zero.planes.iter().all(|plane| *plane == Vec4::ZERO));

        let mut broken = Mat4::IDENTITY;
        broken.cols[0].x = f32::NAN;
        let frustum = Frustum::from_view_proj(&broken);
        assert!(frustum.intersects_sphere(Vec3::new(10.0, 0.0, 0.5), 0.1), "unusable x planes fail open");
        assert!(!frustum.intersects_sphere(Vec3::new(0.0, 10.0, 0.5), 0.1), "a finite y plane still culls");

        let mut infinite = CameraProjection::orthographic(10.0).matrix(1.0);
        infinite.cols[2].z = f32::INFINITY;
        let open = Frustum::from_view_proj(&infinite);
        assert!(open.intersects_sphere(Vec3::new(0.0, 0.0, 10.0), 1.0));
    }

    #[test]
    fn rotated_view_projection_keeps_inward_planes() {
        let projection = CameraProjection::perspective(60.0).matrix(1.0);
        let looking_back = Mat4::look_to_lh(Vec3::new(0.0, 0.0, 5.0), -Vec3::Z, Vec3::Y);
        let back = Frustum::from_view_proj(&(projection * looking_back));
        assert!(back.intersects_sphere(Vec3::new(0.0, 0.0, 0.0), 0.1));
        assert!(!back.intersects_sphere(Vec3::new(0.0, 0.0, 20.0), 0.1));

        let looking_x = Mat4::look_to_lh(Vec3::ZERO, Vec3::X, Vec3::Y);
        let side = Frustum::from_view_proj(&(projection * looking_x));
        assert!(side.intersects_sphere(Vec3::new(10.0, 0.0, 0.0), 0.1));
        assert!(!side.intersects_sphere(Vec3::new(-10.0, 0.0, 0.0), 0.1));
    }
}
