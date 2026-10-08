use engine_math::{Mat4, Vec3, Vec4};

/// The six planes bounding the visible volume, normals pointing inwards.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// Extracts planes from a world->clip matrix with NDC depth in `[0, 1]`.
    pub fn from_view_proj(m: &Mat4) -> Self {
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        let raw = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
        let planes = raw.map(|p| {
            let len = p.xyz().length().max(1e-12);
            p / len
        });
        Self { planes }
    }

    /// Conservative: false only if the sphere is entirely outside one plane.
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        self.planes.iter().all(|p| p.xyz().dot(center) + p.w >= -radius)
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
}
