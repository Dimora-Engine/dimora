use crate::{Mat3, Vec3};

/// Unit quaternion rotation. Rotations follow the right-hand rule about the given axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Quat {
    pub const IDENTITY: Quat = Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        let a = axis.normalized();
        let (s, c) = (angle * 0.5).sin_cos();
        Quat::new(a.x * s, a.y * s, a.z * s, c)
    }

    /// Returns the (unit axis, angle in `[0, 2π]`). Identity yields `(Vec3::X, 0)`.
    pub fn to_axis_angle(self) -> (Vec3, f32) {
        let q = self.normalize();
        let angle = 2.0 * q.w.clamp(-1.0, 1.0).acos();
        let s = (1.0 - q.w * q.w).max(0.0).sqrt();
        if s < 1e-6 {
            (Vec3::X, 0.0)
        } else {
            (Vec3::new(q.x, q.y, q.z) / s, angle)
        }
    }

    pub fn from_rotation_x(a: f32) -> Self {
        Self::from_axis_angle(Vec3::X, a)
    }
    pub fn from_rotation_y(a: f32) -> Self {
        Self::from_axis_angle(Vec3::Y, a)
    }
    pub fn from_rotation_z(a: f32) -> Self {
        Self::from_axis_angle(Vec3::Z, a)
    }

    /// Builds a rotation from Euler angles in radians. The composition order is
    /// roll (Z) first, then pitch (X), then yaw (Y): `q = yaw * pitch * roll`.
    pub fn from_euler(pitch: f32, yaw: f32, roll: f32) -> Self {
        Self::from_rotation_y(yaw) * Self::from_rotation_x(pitch) * Self::from_rotation_z(roll)
    }

    /// Inverse of [`Quat::from_euler`]; returns `(pitch, yaw, roll)`.
    pub fn to_euler(self) -> (f32, f32, f32) {
        let m = Mat3::from_quat(self.normalize());
        let sx = -m.at(1, 2);
        let pitch = sx.clamp(-1.0, 1.0).asin();
        if sx.abs() < 0.9999 {
            (pitch, m.at(0, 2).atan2(m.at(2, 2)), m.at(1, 0).atan2(m.at(1, 1)))
        } else {
            // Gimbal lock: fold roll into yaw.
            (pitch, (-m.at(2, 0)).atan2(m.at(0, 0)), 0.0)
        }
    }

    /// Rotation looking along `forward` with `up` as the approximate up direction (left-handed).
    pub fn look_rotation(forward: Vec3, up: Vec3) -> Self {
        let f = forward.normalized();
        if f == Vec3::ZERO {
            return Self::IDENTITY;
        }
        let mut r = up.cross(f);
        if r.length_squared() < 1e-10 {
            // `up` is parallel to `forward`; pick any perpendicular axis.
            r = if f.x.abs() < 0.9 { Vec3::X.cross(f) } else { Vec3::Y.cross(f) };
        }
        let r = r.normalized();
        let u = f.cross(r);
        Self::from_mat3(&Mat3::from_cols(r, u, f))
    }

    pub fn from_mat3(m: &Mat3) -> Self {
        let (m00, m11, m22) = (m.at(0, 0), m.at(1, 1), m.at(2, 2));
        let trace = m00 + m11 + m22;
        let q = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            Quat::new((m.at(2, 1) - m.at(1, 2)) / s, (m.at(0, 2) - m.at(2, 0)) / s, (m.at(1, 0) - m.at(0, 1)) / s, 0.25 * s)
        } else if m00 > m11 && m00 > m22 {
            let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
            Quat::new(0.25 * s, (m.at(0, 1) + m.at(1, 0)) / s, (m.at(0, 2) + m.at(2, 0)) / s, (m.at(2, 1) - m.at(1, 2)) / s)
        } else if m11 > m22 {
            let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
            Quat::new((m.at(0, 1) + m.at(1, 0)) / s, 0.25 * s, (m.at(1, 2) + m.at(2, 1)) / s, (m.at(0, 2) - m.at(2, 0)) / s)
        } else {
            let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
            Quat::new((m.at(0, 2) + m.at(2, 0)) / s, (m.at(1, 2) + m.at(2, 1)) / s, 0.25 * s, (m.at(1, 0) - m.at(0, 1)) / s)
        };
        q.normalize()
    }

    #[inline]
    pub fn dot(self, o: Quat) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z + self.w * o.w
    }
    #[inline]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn normalize(self) -> Quat {
        let l = self.length();
        if l > crate::EPSILON {
            Quat::new(self.x / l, self.y / l, self.z / l, self.w / l)
        } else {
            Quat::IDENTITY
        }
    }
    #[inline]
    pub fn conjugate(self) -> Quat {
        Quat::new(-self.x, -self.y, -self.z, self.w)
    }
    /// Inverse of a (not necessarily unit) quaternion.
    pub fn inverse(self) -> Quat {
        let n = self.dot(self);
        if n > 0.0 {
            let c = self.conjugate();
            Quat::new(c.x / n, c.y / n, c.z / n, c.w / n)
        } else {
            Quat::IDENTITY
        }
    }

    /// Rotates `v` by this (unit) quaternion.
    #[inline]
    pub fn rotate(self, v: Vec3) -> Vec3 {
        let u = Vec3::new(self.x, self.y, self.z);
        let t = u.cross(v) * 2.0;
        v + t * self.w + u.cross(t)
    }

    /// Normalized linear interpolation (shortest arc). Cheap and good for small steps.
    pub fn nlerp(self, mut o: Quat, t: f32) -> Quat {
        if self.dot(o) < 0.0 {
            o = Quat::new(-o.x, -o.y, -o.z, -o.w);
        }
        Quat::new(
            self.x + (o.x - self.x) * t,
            self.y + (o.y - self.y) * t,
            self.z + (o.z - self.z) * t,
            self.w + (o.w - self.w) * t,
        )
        .normalize()
    }

    /// Spherical linear interpolation along the shortest arc.
    pub fn slerp(self, mut o: Quat, t: f32) -> Quat {
        let mut d = self.dot(o);
        if d < 0.0 {
            o = Quat::new(-o.x, -o.y, -o.z, -o.w);
            d = -d;
        }
        if d > 0.9995 {
            return self.nlerp(o, t);
        }
        let theta = d.clamp(-1.0, 1.0).acos();
        let sin = theta.sin();
        let (a, b) = (((1.0 - t) * theta).sin() / sin, (t * theta).sin() / sin);
        Quat::new(self.x * a + o.x * b, self.y * a + o.y * b, self.z * a + o.z * b, self.w * a + o.w * b).normalize()
    }

    pub fn approx_eq(self, o: Quat, eps: f32) -> bool {
        // q and -q are the same rotation.
        self.dot(o).abs() >= 1.0 - eps
    }
}

impl core::ops::Mul for Quat {
    type Output = Quat;
    /// Hamilton product: `(a * b).rotate(v) == a.rotate(b.rotate(v))`.
    fn mul(self, r: Quat) -> Quat {
        Quat::new(
            self.w * r.x + self.x * r.w + self.y * r.z - self.z * r.y,
            self.w * r.y - self.x * r.z + self.y * r.w + self.z * r.x,
            self.w * r.z + self.x * r.y - self.y * r.x + self.z * r.w,
            self.w * r.w - self.x * r.x - self.y * r.y - self.z * r.z,
        )
    }
}
impl core::ops::Mul<Vec3> for Quat {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        self.rotate(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn rotates_about_axes() {
        let q = Quat::from_rotation_z(FRAC_PI_2);
        assert!(q.rotate(Vec3::X).approx_eq(Vec3::Y, 1e-5));
        let q = Quat::from_rotation_y(FRAC_PI_2);
        assert!(q.rotate(Vec3::Z).approx_eq(Vec3::X, 1e-5));
    }

    #[test]
    fn multiplication_composes_right_to_left() {
        let a = Quat::from_rotation_z(FRAC_PI_2);
        let b = Quat::from_rotation_x(FRAC_PI_2);
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!((a * b).rotate(v).approx_eq(a.rotate(b.rotate(v)), 1e-5));
    }

    #[test]
    fn inverse_cancels() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 1.0, 0.0), 1.1);
        assert!((q * q.inverse()).approx_eq(Quat::IDENTITY, 1e-6));
    }

    #[test]
    fn axis_angle_roundtrip() {
        let (axis, angle) = Quat::from_axis_angle(Vec3::new(0.0, 2.0, 0.0), 0.8).to_axis_angle();
        assert!(axis.approx_eq(Vec3::Y, 1e-5));
        assert!((angle - 0.8).abs() < 1e-5);
    }

    #[test]
    fn euler_roundtrip() {
        for &(p, y, r) in &[(0.3, 0.5, -0.7), (-1.0, 2.0, 0.1), (0.0, 0.0, 0.0), (0.5, -2.5, 1.5)] {
            let (p2, y2, r2) = Quat::from_euler(p, y, r).to_euler();
            assert!(Quat::from_euler(p2, y2, r2).approx_eq(Quat::from_euler(p, y, r), 1e-5), "{p} {y} {r}");
        }
        let (p, _, _) = Quat::from_euler(FRAC_PI_2, 0.4, 0.0).to_euler();
        assert!((p - FRAC_PI_2).abs() < 1e-3);
    }

    #[test]
    fn slerp_endpoints_and_midpoint() {
        let a = Quat::IDENTITY;
        let b = Quat::from_rotation_y(PI / 2.0);
        assert!(a.slerp(b, 0.0).approx_eq(a, 1e-6));
        assert!(a.slerp(b, 1.0).approx_eq(b, 1e-6));
        assert!(a.slerp(b, 0.5).approx_eq(Quat::from_rotation_y(PI / 4.0), 1e-5));
    }

    #[test]
    fn look_rotation_faces_forward() {
        let f = Vec3::new(1.0, 0.5, 2.0).normalized();
        let q = Quat::look_rotation(f, Vec3::Y);
        assert!(q.rotate(Vec3::Z).approx_eq(f, 1e-5));
        assert!(Quat::look_rotation(Vec3::Z, Vec3::Y).approx_eq(Quat::IDENTITY, 1e-6));
        assert!(Quat::look_rotation(Vec3::Y, Vec3::Y).rotate(Vec3::Z).approx_eq(Vec3::Y, 1e-5));
    }
}
