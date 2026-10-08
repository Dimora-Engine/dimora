//! Renderable ECS components. They describe *what* something is; *where* it is comes from the
//! entity's `Transform`, the same as for every other spatial object.

use engine_assets::{AssetHandle, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::Vec2;

/// How a sprite combines with what is behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpriteBlend {
    /// Alpha-blended. Sorted back-to-front by view depth, does not write depth.
    #[default]
    Alpha,
    /// Alpha-tested (hard edges, typical for pixel art). Writes depth, needs no sorting, and
    /// intersects correctly with 3D geometry.
    Cutout,
}

/// A rectangle of texture placed in the world.
///
/// The sprite lies in its entity's local XY plane, facing local `-Z` (towards the default camera),
/// positioned by the entity's `Transform` and the pivot. Rotating the transform tilts it in 3D.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub asset: AssetHandle<SpriteAsset>,
    /// Size in local world units; `None` uses the asset's natural size.
    pub size: Option<Vec2>,
    /// Pivot `(0,0)` bottom-left .. `(1,1)` top-right; `None` uses the asset's pivot.
    pub anchor: Option<Vec2>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub tint: Color,
    pub blend: SpriteBlend,
    /// Whether scene lights shade this sprite. Unlit sprites show their texture as-is.
    pub lit: bool,
    /// Explicit ordering override. Sprites sort by `(layer, depth, order)`; leave both at 0 to let
    /// spatial depth decide. `layer` dominates depth (use it for e.g. world-space overlays),
    /// `order` breaks ties between sprites at identical depth.
    pub layer: i16,
    pub order: i16,
}

impl Sprite {
    pub fn new(name: &str) -> Self {
        Self::from_asset(AssetHandle::from_name(name))
    }
    pub fn from_asset(asset: AssetHandle<SpriteAsset>) -> Self {
        Self {
            asset,
            size: None,
            anchor: None,
            flip_x: false,
            flip_y: false,
            tint: Color::WHITE,
            blend: SpriteBlend::Alpha,
            lit: true,
            layer: 0,
            order: 0,
        }
    }
    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = Some(size);
        self
    }
    pub fn with_anchor(mut self, anchor: Vec2) -> Self {
        self.anchor = Some(anchor);
        self
    }
    pub fn with_tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }
    pub fn with_flip_x(mut self, flip: bool) -> Self {
        self.flip_x = flip;
        self
    }
    pub fn with_blend(mut self, blend: SpriteBlend) -> Self {
        self.blend = blend;
        self
    }
    pub fn cutout(self) -> Self {
        self.with_blend(SpriteBlend::Cutout)
    }
    pub fn unlit(mut self) -> Self {
        self.lit = false;
        self
    }
    pub fn with_layer(mut self, layer: i16, order: i16) -> Self {
        self.layer = layer;
        self.order = order;
        self
    }
}

/// Surface description for meshes. Intentionally tiny: this struct is the boundary where a real
/// material system (PBR, custom shaders) will attach later.
#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub base_color: Color,
    pub base_texture: Option<AssetHandle<Texture>>,
    pub lit: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self { base_color: Color::WHITE, base_texture: None, lit: true }
    }
}

impl Material {
    pub fn color(base_color: Color) -> Self {
        Self { base_color, ..Self::default() }
    }
    pub fn textured(texture: &str) -> Self {
        Self { base_texture: Some(AssetHandle::from_name(texture)), ..Self::default() }
    }
}

/// Real 3D geometry, placed by the entity's `Transform`. Opaque, depth-tested, lit.
#[derive(Clone, Copy, Debug)]
pub struct Mesh {
    pub asset: AssetHandle<MeshData>,
    pub material: Material,
}

impl Mesh {
    pub fn new(name: &str) -> Self {
        Self { asset: AssetHandle::from_name(name), material: Material::default() }
    }
    pub fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }
}

/// A grid of tiles taken from an atlas texture, laid out in the entity's local XY plane. Tile
/// `(x, y)` occupies `[x, x+1] * tile_size.x` by `[y, y+1] * tile_size.y` from the local origin.
/// Rotate the entity (e.g. 90 degrees about X) to lay it on the ground. Tiles are cutout-shaded and
/// depth-tested, so a map costs no sorting.
#[derive(Clone, Debug)]
pub struct TileMap {
    pub texture: AssetHandle<Texture>,
    pub atlas_columns: u32,
    pub atlas_rows: u32,
    /// World size of one tile.
    pub tile_size: Vec2,
    pub width: u32,
    pub height: u32,
    /// Row-major from the local origin; [`TileMap::EMPTY`] leaves a hole.
    pub tiles: Vec<u16>,
    pub lit: bool,
}

impl TileMap {
    pub const EMPTY: u16 = u16::MAX;

    pub fn new(texture: &str, atlas_columns: u32, atlas_rows: u32, tile_size: Vec2, width: u32, height: u32) -> Self {
        Self {
            texture: AssetHandle::from_name(texture),
            atlas_columns,
            atlas_rows,
            tile_size,
            width,
            height,
            tiles: vec![Self::EMPTY; (width * height) as usize],
            lit: true,
        }
    }

    pub fn filled_with(mut self, f: impl Fn(u32, u32) -> u16) -> Self {
        for y in 0..self.height {
            for x in 0..self.width {
                self.set(x, y, f(x, y));
            }
        }
        self
    }

    pub fn set(&mut self, x: u32, y: u32, tile: u16) {
        self.tiles[(y * self.width + x) as usize] = tile;
    }

    pub fn get(&self, x: u32, y: u32) -> u16 {
        self.tiles[(y * self.width + x) as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightKind {
    /// Parallel light travelling along the entity's forward (+Z) axis; position is irrelevant.
    Directional,
    /// Omnidirectional light at the entity's position, fading to zero at `range`.
    Point { range: f32 },
    /// Cone along the entity's forward axis. Angles are the half-angles of the full-intensity and
    /// zero-intensity cones, in radians.
    Spot { range: f32, inner_angle: f32, outer_angle: f32 },
}

/// A light in the world. Position and direction come from the entity's `Transform`; everything
/// that is lit (sprites, tiles, meshes) is shaded in world space, so depth matters to light
/// falloff exactly as it does to projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub kind: LightKind,
    pub color: Color,
    pub intensity: f32,
}

impl Light {
    pub fn directional(color: Color, intensity: f32) -> Self {
        Self { kind: LightKind::Directional, color, intensity }
    }
    pub fn point(color: Color, intensity: f32, range: f32) -> Self {
        Self { kind: LightKind::Point { range }, color, intensity }
    }
    pub fn spot(color: Color, intensity: f32, range: f32, inner_angle: f32, outer_angle: f32) -> Self {
        Self { kind: LightKind::Spot { range, inner_angle, outer_angle }, color, intensity }
    }
}

/// Scene-wide base illumination. Stored as a world resource.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbientLight {
    pub color: Color,
    pub intensity: f32,
}

impl Default for AmbientLight {
    fn default() -> Self {
        Self { color: Color::WHITE, intensity: 0.25 }
    }
}
