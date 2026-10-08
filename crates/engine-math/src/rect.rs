use crate::Vec2;

/// Axis-aligned 2D rectangle. Used for UV regions, viewports and tile atlases.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    /// `(0,0)..(1,1)`, i.e. a whole texture in UV space.
    pub const UNIT: Rect = Rect { min: Vec2::ZERO, max: Vec2::ONE };

    pub const fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self { min, max: min + size }
    }
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}
