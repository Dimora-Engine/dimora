use crate::{Mat4, Quat, Vec3};

/// Position, rotation and scale in 3D. This is the fundamental spatial component:
/// everything in the world, 2D or 3D, has one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Transform = Transform { position: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE };

    pub const fn from_position(position: Vec3) -> Self {
        Self { position, ..Self::IDENTITY }
    }
    pub const fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Self::from_position(Vec3::new(x, y, z))
    }
    pub const fn from_rotation(rotation: Quat) -> Self {
        Self { rotation, ..Self::IDENTITY }
    }
    pub const fn from_scale(scale: Vec3) -> Self {
        Self { scale, ..Self::IDENTITY }
    }
    pub const fn with_position(mut self, position: Vec3) -> Self {
        self.position = position;
        self
    }
    pub const fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }
    pub const fn with_scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }
    pub fn with_uniform_scale(self, s: f32) -> Self {
        self.with_scale(Vec3::splat(s))
    }

    /// A transform at `position` whose +Z axis points at `target`.
    pub fn looking_at(position: Vec3, target: Vec3, up: Vec3) -> Self {
        Self { position, rotation: Quat::look_rotation(target - position, up), scale: Vec3::ONE }
    }

    /// Local +Z axis in the parent space; the direction a camera or spot light faces.
    pub fn forward(&self) -> Vec3 {
        self.rotation.rotate(Vec3::Z)
    }
    pub fn right(&self) -> Vec3 {
        self.rotation.rotate(Vec3::X)
    }
    pub fn up(&self) -> Vec3 {
        self.rotation.rotate(Vec3::Y)
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        self.position + self.rotation.rotate(p * self.scale)
    }

    /// Composition: `parent.mul_transform(child)` expresses `child` in `parent`'s parent space,
    /// i.e. `result.transform_point(p) == parent.transform_point(child.transform_point(p))`.
    ///
    /// Exact when the parent's scale is uniform or when the child has no rotation. Non-uniform
    /// parent scale combined with a rotated child produces shear, which a TRS triple cannot
    /// represent, so the result is the closest TRS approximation.
    pub fn mul_transform(&self, child: &Transform) -> Transform {
        Transform {
            position: self.transform_point(child.position),
            rotation: self.rotation * child.rotation,
            scale: self.scale * child.scale,
        }
    }

    /// Inverse transform. Exact for uniform scale; for non-uniform scale the same shear caveat
    /// as [`Transform::mul_transform`] applies.
    pub fn inverse(&self) -> Transform {
        let inv_rot = self.rotation.inverse();
        let inv_scale = self.scale.recip();
        Transform { position: inv_rot.rotate(-self.position) * inv_scale, rotation: inv_rot, scale: inv_scale }
    }

    /// Linear interpolation of position/scale and spherical interpolation of rotation.
    pub fn lerp(&self, other: &Transform, t: f32) -> Transform {
        Transform {
            position: self.position.lerp(other.position, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: self.scale.lerp(other.scale, t),
        }
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_trs(self.position, self.rotation, self.scale)
    }

    /// Decomposes an affine matrix. Shear is discarded; negative determinants flip the X scale.
    pub fn from_matrix(m: &Mat4) -> Transform {
        let (cx, cy, cz) = (m.cols[0].xyz(), m.cols[1].xyz(), m.cols[2].xyz());
        let mut scale = Vec3::new(cx.length(), cy.length(), cz.length());
        if cx.dot(cy.cross(cz)) < 0.0 {
            scale.x = -scale.x;
        }
        let basis = crate::Mat3::from_cols(cx / scale.x, cy / scale.y, cz / scale.z);
        Transform { position: m.translation(), rotation: Quat::from_mat3(&basis), scale }
    }
}

impl core::ops::Mul for Transform {
    type Output = Transform;
    fn mul(self, rhs: Transform) -> Transform {
        self.mul_transform(&rhs)
    }
}
impl core::ops::Mul<Vec3> for Transform {
    type Output = Vec3;
    fn mul(self, p: Vec3) -> Vec3 {
        self.transform_point(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Transform {
        Transform {
            position: Vec3::new(1.0, -2.0, 30.0),
            rotation: Quat::from_euler(0.2, 0.9, -0.3),
            scale: Vec3::splat(2.0),
        }
    }

    #[test]
    fn identity_is_neutral() {
        let t = sample();
        assert!((Transform::IDENTITY * t).to_matrix().approx_eq(&t.to_matrix(), 1e-6));
        assert!((t * Transform::IDENTITY).to_matrix().approx_eq(&t.to_matrix(), 1e-6));
    }

    #[test]
    fn composition_matches_matrix_product() {
        let (a, b) = (sample(), Transform::from_position(Vec3::new(0.5, 1.0, -2.0)).with_rotation(Quat::from_rotation_x(0.4)));
        assert!((a * b).to_matrix().approx_eq(&(a.to_matrix() * b.to_matrix()), 1e-4));
        let p = Vec3::new(1.0, 2.0, 3.0);
        assert!((a * b * p).approx_eq(a * (b * p), 1e-4));
    }

    #[test]
    fn inverse_cancels() {
        let t = sample();
        assert!((t * t.inverse()).to_matrix().approx_eq(&Mat4::IDENTITY, 1e-5));
        let p = Vec3::new(4.0, 5.0, 6.0);
        assert!(t.inverse().transform_point(t.transform_point(p)).approx_eq(p, 1e-4));
    }

    #[test]
    fn lerp_endpoints_and_midpoint() {
        let (a, b) = (Transform::IDENTITY, Transform::from_xyz(10.0, 0.0, 4.0).with_rotation(Quat::from_rotation_y(1.0)));
        assert_eq!(a.lerp(&b, 0.0).position, a.position);
        let mid = a.lerp(&b, 0.5);
        assert!(mid.position.approx_eq(Vec3::new(5.0, 0.0, 2.0), 1e-6));
        assert!(mid.rotation.approx_eq(Quat::from_rotation_y(0.5), 1e-5));
    }

    #[test]
    fn matrix_roundtrip_and_looking_at() {
        let t = Transform { scale: Vec3::new(1.0, 2.0, 3.0), ..sample() };
        let back = Transform::from_matrix(&t.to_matrix());
        assert!(back.to_matrix().approx_eq(&t.to_matrix(), 1e-4));
        let cam = Transform::looking_at(Vec3::new(0.0, 5.0, -10.0), Vec3::ZERO, Vec3::Y);
        assert!(cam.forward().approx_eq(Vec3::new(0.0, -5.0, 10.0).normalized(), 1e-5));
    }
}
