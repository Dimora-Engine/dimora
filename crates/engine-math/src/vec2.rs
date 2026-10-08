use crate::impl_vec_ops;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl_vec_ops!(Vec2 { x, y });

impl Vec2 {
    pub const ZERO: Vec2 = Vec2::new(0.0, 0.0);
    pub const ONE: Vec2 = Vec2::new(1.0, 1.0);
    pub const X: Vec2 = Vec2::new(1.0, 0.0);
    pub const Y: Vec2 = Vec2::new(0.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    #[inline]
    pub const fn splat(v: f32) -> Self {
        Self::new(v, v)
    }
    #[inline]
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
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
    pub fn normalized(self) -> Vec2 {
        let l = self.length();
        if l > crate::EPSILON {
            self / l
        } else {
            Vec2::ZERO
        }
    }
    #[inline]
    pub fn distance(self, o: Vec2) -> f32 {
        (self - o).length()
    }
    #[inline]
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        self + (o - self) * t
    }
    #[inline]
    pub fn min(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.min(o.x), self.y.min(o.y))
    }
    #[inline]
    pub fn max(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.max(o.x), self.y.max(o.y))
    }
    #[inline]
    pub fn clamp(self, lo: Vec2, hi: Vec2) -> Vec2 {
        self.max(lo).min(hi)
    }
    #[inline]
    pub fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }
}

impl From<[f32; 2]> for Vec2 {
    fn from(a: [f32; 2]) -> Self {
        Vec2::new(a[0], a[1])
    }
}
impl From<(f32, f32)> for Vec2 {
    fn from(a: (f32, f32)) -> Self {
        Vec2::new(a.0, a.1)
    }
}
