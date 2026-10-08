use crate::{Quat, Vec3};

/// Column-major 3x3 matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub cols: [Vec3; 3],
}

impl Default for Mat3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat3 {
    pub const IDENTITY: Mat3 = Mat3 { cols: [Vec3::X, Vec3::Y, Vec3::Z] };

    pub const fn from_cols(x: Vec3, y: Vec3, z: Vec3) -> Self {
        Self { cols: [x, y, z] }
    }

    pub fn from_scale(s: Vec3) -> Self {
        Self::from_cols(Vec3::new(s.x, 0.0, 0.0), Vec3::new(0.0, s.y, 0.0), Vec3::new(0.0, 0.0, s.z))
    }

    pub fn from_quat(q: Quat) -> Self {
        let (x, y, z, w) = (q.x, q.y, q.z, q.w);
        Self::from_cols(
            Vec3::new(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)),
            Vec3::new(2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)),
            Vec3::new(2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)),
        )
    }

    /// Element at `(row, col)`.
    #[inline]
    pub fn at(&self, row: usize, col: usize) -> f32 {
        let c = self.cols[col];
        [c.x, c.y, c.z][row]
    }

    pub fn mul_vec3(&self, v: Vec3) -> Vec3 {
        self.cols[0] * v.x + self.cols[1] * v.y + self.cols[2] * v.z
    }

    pub fn mul_mat3(&self, r: &Mat3) -> Mat3 {
        Mat3::from_cols(self.mul_vec3(r.cols[0]), self.mul_vec3(r.cols[1]), self.mul_vec3(r.cols[2]))
    }

    pub fn transpose(&self) -> Mat3 {
        let [a, b, c] = self.cols;
        Mat3::from_cols(Vec3::new(a.x, b.x, c.x), Vec3::new(a.y, b.y, c.y), Vec3::new(a.z, b.z, c.z))
    }

    pub fn determinant(&self) -> f32 {
        self.cols[0].dot(self.cols[1].cross(self.cols[2]))
    }

    /// Returns `None` for singular matrices.
    pub fn inverse(&self) -> Option<Mat3> {
        let [a, b, c] = self.cols;
        let det = self.determinant();
        if det.abs() < crate::EPSILON * crate::EPSILON {
            return None;
        }
        let inv = 1.0 / det;
        Some(Mat3::from_cols(b.cross(c) * inv, c.cross(a) * inv, a.cross(b) * inv).transpose())
    }
}

impl core::ops::Mul for Mat3 {
    type Output = Mat3;
    fn mul(self, r: Mat3) -> Mat3 {
        self.mul_mat3(&r)
    }
}
impl core::ops::Mul<Vec3> for Mat3 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        self.mul_vec3(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_roundtrip() {
        let m = Mat3::from_quat(Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).normalized(), 0.7))
            * Mat3::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let i = m.inverse().unwrap() * m;
        for r in 0..3 {
            for c in 0..3 {
                let e = if r == c { 1.0 } else { 0.0 };
                assert!((i.at(r, c) - e).abs() < 1e-5);
            }
        }
        assert!(Mat3::from_scale(Vec3::new(1.0, 0.0, 1.0)).inverse().is_none());
    }
}
