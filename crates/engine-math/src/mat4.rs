use crate::{Mat3, Quat, Vec3, Vec4};

/// Column-major 4x4 matrix; transforms column vectors (`M * v`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub cols: [Vec4; 4],
}

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        cols: [
            Vec4::new(1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        ],
    };

    pub const fn from_cols(x: Vec4, y: Vec4, z: Vec4, w: Vec4) -> Self {
        Self { cols: [x, y, z, w] }
    }

    pub fn from_translation(t: Vec3) -> Self {
        let mut m = Self::IDENTITY;
        m.cols[3] = t.extend(1.0);
        m
    }

    pub fn from_scale(s: Vec3) -> Self {
        Self::from_cols(
            Vec4::new(s.x, 0.0, 0.0, 0.0),
            Vec4::new(0.0, s.y, 0.0, 0.0),
            Vec4::new(0.0, 0.0, s.z, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        )
    }

    pub fn from_quat(q: Quat) -> Self {
        Self::from_mat3(&Mat3::from_quat(q))
    }

    pub fn from_mat3(m: &Mat3) -> Self {
        Self::from_cols(m.cols[0].extend(0.0), m.cols[1].extend(0.0), m.cols[2].extend(0.0), Vec4::new(0.0, 0.0, 0.0, 1.0))
    }

    /// `T * R * S`: scale first, then rotate, then translate.
    pub fn from_trs(t: Vec3, r: Quat, s: Vec3) -> Self {
        let m = Mat3::from_quat(r);
        Self::from_cols(
            (m.cols[0] * s.x).extend(0.0),
            (m.cols[1] * s.y).extend(0.0),
            (m.cols[2] * s.z).extend(0.0),
            t.extend(1.0),
        )
    }

    /// View matrix for a camera at `eye` looking along `dir` (left-handed, +Z forward).
    pub fn look_to_lh(eye: Vec3, dir: Vec3, up: Vec3) -> Self {
        let f = dir.normalized();
        let r = up.cross(f).normalized();
        let u = f.cross(r);
        Self::from_cols(
            Vec4::new(r.x, u.x, f.x, 0.0),
            Vec4::new(r.y, u.y, f.y, 0.0),
            Vec4::new(r.z, u.z, f.z, 0.0),
            Vec4::new(-r.dot(eye), -u.dot(eye), -f.dot(eye), 1.0),
        )
    }

    /// Perspective projection, left-handed, NDC depth `[0, 1]`.
    pub fn perspective_lh(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        let range = far / (far - near);
        Self::from_cols(
            Vec4::new(f / aspect, 0.0, 0.0, 0.0),
            Vec4::new(0.0, f, 0.0, 0.0),
            Vec4::new(0.0, 0.0, range, 1.0),
            Vec4::new(0.0, 0.0, -near * range, 0.0),
        )
    }

    /// Orthographic projection, left-handed, NDC depth `[0, 1]`.
    pub fn orthographic_lh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let (w, h, d) = (right - left, top - bottom, far - near);
        Self::from_cols(
            Vec4::new(2.0 / w, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 / h, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0 / d, 0.0),
            Vec4::new(-(right + left) / w, -(top + bottom) / h, -near / d, 1.0),
        )
    }

    #[inline]
    pub fn row(&self, i: usize) -> Vec4 {
        let c = &self.cols;
        let get = |v: Vec4| [v.x, v.y, v.z, v.w][i];
        Vec4::new(get(c[0]), get(c[1]), get(c[2]), get(c[3]))
    }

    pub fn mul_vec4(&self, v: Vec4) -> Vec4 {
        self.cols[0] * v.x + self.cols[1] * v.y + self.cols[2] * v.z + self.cols[3] * v.w
    }

    pub fn mul_mat4(&self, r: &Mat4) -> Mat4 {
        Mat4::from_cols(
            self.mul_vec4(r.cols[0]),
            self.mul_vec4(r.cols[1]),
            self.mul_vec4(r.cols[2]),
            self.mul_vec4(r.cols[3]),
        )
    }

    /// Transforms a point (w = 1), ignoring any projective divide.
    #[inline]
    pub fn transform_point3(&self, p: Vec3) -> Vec3 {
        self.mul_vec4(p.extend(1.0)).xyz()
    }

    /// Transforms a direction (w = 0).
    #[inline]
    pub fn transform_vector3(&self, v: Vec3) -> Vec3 {
        self.mul_vec4(v.extend(0.0)).xyz()
    }

    pub fn translation(&self) -> Vec3 {
        self.cols[3].xyz()
    }

    pub fn transpose(&self) -> Mat4 {
        Mat4::from_cols(self.row(0), self.row(1), self.row(2), self.row(3))
    }

    /// General inverse via Gauss-Jordan elimination. `None` if singular.
    pub fn inverse(&self) -> Option<Mat4> {
        let mut a = [[0.0f32; 8]; 4];
        for (r, row) in a.iter_mut().enumerate() {
            let v = self.row(r).to_array();
            row[..4].copy_from_slice(&v);
            row[4 + r] = 1.0;
        }
        for i in 0..4 {
            let pivot = (i..4).max_by(|&x, &y| a[x][i].abs().total_cmp(&a[y][i].abs()))?;
            if a[pivot][i].abs() < 1e-12 {
                return None;
            }
            a.swap(i, pivot);
            let d = 1.0 / a[i][i];
            a[i].iter_mut().for_each(|e| *e *= d);
            for r in 0..4 {
                if r != i {
                    let f = a[r][i];
                    let src = a[i];
                    a[r].iter_mut().zip(src).for_each(|(e, s)| *e -= f * s);
                }
            }
        }
        let col = |c: usize| Vec4::new(a[0][4 + c], a[1][4 + c], a[2][4 + c], a[3][4 + c]);
        Some(Mat4::from_cols(col(0), col(1), col(2), col(3)))
    }

    /// Column-major flat array, ready for GPU upload.
    pub fn to_cols_array(&self) -> [f32; 16] {
        let mut o = [0.0; 16];
        for (i, c) in self.cols.iter().enumerate() {
            o[i * 4..i * 4 + 4].copy_from_slice(&c.to_array());
        }
        o
    }

    pub fn approx_eq(&self, o: &Mat4, eps: f32) -> bool {
        self.to_cols_array().iter().zip(o.to_cols_array()).all(|(a, b)| (a - b).abs() <= eps)
    }
}

impl core::ops::Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, r: Mat4) -> Mat4 {
        self.mul_mat4(&r)
    }
}
impl core::ops::Mul<Vec4> for Mat4 {
    type Output = Vec4;
    fn mul(self, v: Vec4) -> Vec4 {
        self.mul_vec4(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trs_matches_manual_transform() {
        let (t, r, s) = (Vec3::new(1.0, 2.0, 3.0), Quat::from_rotation_y(0.9), Vec3::new(2.0, 1.0, 0.5));
        let m = Mat4::from_trs(t, r, s);
        let p = Vec3::new(0.3, -0.2, 4.0);
        assert!(m.transform_point3(p).approx_eq(t + r.rotate(p * s), 1e-5));
        assert!((Mat4::from_translation(t) * Mat4::from_quat(r) * Mat4::from_scale(s)).approx_eq(&m, 1e-5));
    }

    #[test]
    fn inverse_roundtrip() {
        let m = Mat4::from_trs(Vec3::new(5.0, -3.0, 9.0), Quat::from_euler(0.3, 1.2, -0.4), Vec3::new(1.5, 2.0, 0.7));
        assert!((m * m.inverse().unwrap()).approx_eq(&Mat4::IDENTITY, 1e-4));
        assert!(Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0)).inverse().is_none());
    }

    #[test]
    fn view_matrix_places_eye_at_origin() {
        let eye = Vec3::new(3.0, 4.0, -10.0);
        let v = Mat4::look_to_lh(eye, Vec3::Z, Vec3::Y);
        assert!(v.transform_point3(eye).approx_eq(Vec3::ZERO, 1e-6));
        assert!(v.transform_point3(eye + Vec3::new(1.0, 2.0, 5.0)).approx_eq(Vec3::new(1.0, 2.0, 5.0), 1e-6));
    }

    #[test]
    fn projection_depth_range_is_zero_to_one() {
        let p = Mat4::perspective_lh(1.0, 1.5, 0.1, 100.0);
        let near = p.mul_vec4(Vec4::new(0.0, 0.0, 0.1, 1.0));
        let far = p.mul_vec4(Vec4::new(0.0, 0.0, 100.0, 1.0));
        assert!((near.z / near.w).abs() < 1e-5);
        assert!((far.z / far.w - 1.0).abs() < 1e-5);
        let o = Mat4::orthographic_lh(-2.0, 2.0, -1.0, 1.0, 0.0, 10.0);
        assert!(o.transform_point3(Vec3::new(2.0, 1.0, 10.0)).approx_eq(Vec3::ONE, 1e-6));
        assert!(o.transform_point3(Vec3::new(-2.0, -1.0, 0.0)).approx_eq(Vec3::new(-1.0, -1.0, 0.0), 1e-6));
    }
}
