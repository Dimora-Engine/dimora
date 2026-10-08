use engine_math::{Rect, Vec2};

use crate::{Asset, AssetHandle, AssetKind, Texture};

/// A drawable region of a texture plus its natural size and pivot. Many sprites can share one
/// texture (an atlas), which is what lets the renderer batch them.
#[derive(Clone, Debug)]
pub struct SpriteAsset {
    pub texture: AssetHandle<Texture>,
    /// Region of the texture in normalized UV space; `(0,0)` is the top-left texel.
    pub uv: Rect,
    /// Size of the region in texels.
    pub size_px: Vec2,
    /// Pivot inside the sprite, `(0,0)` bottom-left to `(1,1)` top-right.
    pub pivot: Vec2,
    /// Texels per world unit; converts `size_px` into the sprite's natural world size.
    pub pixels_per_unit: f32,
}

impl SpriteAsset {
    pub const DEFAULT_PIXELS_PER_UNIT: f32 = 16.0;

    pub fn whole_texture(texture: AssetHandle<Texture>, width: u32, height: u32) -> Self {
        Self {
            texture,
            uv: Rect::UNIT,
            size_px: Vec2::new(width as f32, height as f32),
            pivot: Vec2::new(0.5, 0.0),
            pixels_per_unit: Self::DEFAULT_PIXELS_PER_UNIT,
        }
    }

    /// A pixel-rect cut out of an atlas texture of `atlas_size` texels.
    pub fn from_atlas_rect(texture: AssetHandle<Texture>, atlas_size: Vec2, x: u32, y: u32, w: u32, h: u32) -> Self {
        let min = Vec2::new(x as f32, y as f32) / atlas_size;
        let max = Vec2::new((x + w) as f32, (y + h) as f32) / atlas_size;
        Self { uv: Rect::new(min, max), size_px: Vec2::new(w as f32, h as f32), ..Self::whole_texture(texture, w, h) }
    }

    pub fn with_pivot(mut self, pivot: Vec2) -> Self {
        self.pivot = pivot;
        self
    }
    pub fn with_pixels_per_unit(mut self, ppu: f32) -> Self {
        self.pixels_per_unit = ppu;
        self
    }

    /// Natural size in world units.
    pub fn world_size(&self) -> Vec2 {
        self.size_px / self.pixels_per_unit
    }
}

impl Asset for SpriteAsset {
    const KIND: AssetKind = AssetKind::Sprite;
}
