use crate::{impl_vec_ops, Vec3};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl_vec_ops!(Vec4 { x, y, z, w });

impl Vec4 {
    pub const ZERO: Vec4 = Vec4::new(0.0, 0.0, 0.0, 0.0);
    pub const ONE: Vec4 = Vec4::new(1.0, 1.0, 1.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
    #[inline]
    pub const fn splat(v: f32) -> Self {
        Self::new(v, v, v, v)
    }
    #[inline]
    pub fn dot(self, o: Vec4) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z + self.w * o.w
    }
    #[inline]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    #[inline]
    pub fn lerp(self, o: Vec4, t: f32) -> Vec4 {
        self + (o - self) * t
    }
    #[inline]
    pub fn xyz(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
    #[inline]
    pub fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
}

impl From<[f32; 4]> for Vec4 {
    fn from(a: [f32; 4]) -> Self {
        Vec4::new(a[0], a[1], a[2], a[3])
    }
}
