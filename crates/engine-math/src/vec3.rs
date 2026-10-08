use crate::{impl_vec_ops, Vec2, Vec4};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl_vec_ops!(Vec3 { x, y, z });

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const ONE: Vec3 = Vec3::new(1.0, 1.0, 1.0);
    pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    /// Points away from the default camera ("into the screen").
    pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    #[inline]
    pub const fn splat(v: f32) -> Self {
        Self::new(v, v, v)
    }
    #[inline]
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    #[inline]
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    #[inline]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }
    #[inline]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }
    /// Unit-length copy, or zero if the length is (near) zero.
    #[inline]
    pub fn normalized(self) -> Vec3 {
        self.try_normalize().unwrap_or(Vec3::ZERO)
    }
    #[inline]
    pub fn try_normalize(self) -> Option<Vec3> {
        let l = self.length();
        (l > crate::EPSILON).then(|| self / l)
    }
    #[inline]
    pub fn distance(self, o: Vec3) -> f32 {
        (self - o).length()
    }
    #[inline]
    pub fn distance_squared(self, o: Vec3) -> f32 {
        (self - o).length_squared()
    }
    #[inline]
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
    #[inline]
    pub fn min(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }
    #[inline]
    pub fn max(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
    #[inline]
    pub fn clamp(self, lo: Vec3, hi: Vec3) -> Vec3 {
        self.max(lo).min(hi)
    }
    #[inline]
    pub fn abs(self) -> Vec3 {
        Vec3::new(self.x.abs(), self.y.abs(), self.z.abs())
    }
    #[inline]
    pub fn max_element(self) -> f32 {
        self.x.max(self.y).max(self.z)
    }
    #[inline]
    pub fn recip(self) -> Vec3 {
        Vec3::new(1.0 / self.x, 1.0 / self.y, 1.0 / self.z)
    }
    #[inline]
    pub fn xy(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
    #[inline]
    pub fn extend(self, w: f32) -> Vec4 {
        Vec4::new(self.x, self.y, self.z, w)
    }
    #[inline]
    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
    pub fn approx_eq(self, o: Vec3, eps: f32) -> bool {
        (self - o).abs().max_element() <= eps
    }
}

impl From<[f32; 3]> for Vec3 {
    fn from(a: [f32; 3]) -> Self {
        Vec3::new(a[0], a[1], a[2])
    }
}
impl From<(f32, f32, f32)> for Vec3 {
    fn from(a: (f32, f32, f32)) -> Self {
        Vec3::new(a.0, a.1, a.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_products() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, -5.0, 6.0);
        assert_eq!(a + b, Vec3::new(5.0, -3.0, 9.0));
        assert_eq!(a - b, Vec3::new(-3.0, 7.0, -3.0));
        assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(a / 2.0, Vec3::new(0.5, 1.0, 1.5));
        assert_eq!(a.dot(b), 12.0);
        assert_eq!(Vec3::X.cross(Vec3::Y), Vec3::Z);
        assert_eq!(a.cross(b), Vec3::new(27.0, 6.0, -13.0));
    }

    #[test]
    fn length_and_normalize() {
        let v = Vec3::new(3.0, 0.0, 4.0);
        assert_eq!(v.length(), 5.0);
        assert!((v.normalized().length() - 1.0).abs() < 1e-6);
        assert_eq!(Vec3::ZERO.normalized(), Vec3::ZERO);
        assert_eq!(Vec3::ZERO.distance(v), 5.0);
    }

    #[test]
    fn lerp_min_max_clamp() {
        let a = Vec3::new(0.0, 10.0, -4.0);
        let b = Vec3::new(10.0, 20.0, 4.0);
        assert_eq!(a.lerp(b, 0.5), Vec3::new(5.0, 15.0, 0.0));
        assert_eq!(a.min(b), a);
        assert_eq!(a.max(b), b);
        assert_eq!(
            Vec3::new(-5.0, 15.0, 9.0).clamp(Vec3::ZERO, Vec3::splat(10.0)),
            Vec3::new(0.0, 10.0, 9.0).min(Vec3::splat(10.0))
        );
    }
}
