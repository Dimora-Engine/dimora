//! Renderable ECS components. They describe *what* something is; *where* it is comes from the
//! entity's `Transform`, the same as for every other spatial object.

use engine_assets::{AssetHandle, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Rect, Vec2};

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

/// How a sprite quad is oriented in the world. This is a *rendering* decision about the sprite's
/// geometry; where the sprite is always comes from its `Transform`.
///
/// Billboards align to the camera's **view plane** (its right/up axes), not to the camera's
/// position. Every billboard therefore shares one orientation per frame, so a perspective camera
/// never skews or rotates individual sprites; they only change position and apparent size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpriteOrientation {
    /// Faces the camera. The entity's position and scale are used, plus its roll about the view
    /// axis (so a bone can rotate a sprite in the screen plane). Pitch and yaw of the entity are
    /// ignored because the camera decides them.
    #[default]
    Billboard,
    /// Faces the camera but stays upright: its quad is vertical and only turns about world +Y,
    /// so a tilted camera foreshortens it like a real standing object and trees do not lean back.
    /// Roll within that vertical plane is kept (bones can swing it). The usual choice for
    /// characters and props standing on a ground plane.
    Upright,
    /// A plain quad in the entity's local XY plane, facing local `-Z`, fully oriented by the
    /// entity's `Transform`. Use for decals, signs and anything that must lie in the world.
    Fixed,
}

/// A rectangle of texture placed in the world.
///
/// The sprite holds no position of any kind: *where* it is comes from the entity's `Transform`,
/// *how it is projected* from the active camera. The renderer takes `Transform + Sprite + Camera`
/// and generates a world-space quad (see [`SpriteOrientation`]).
///
/// The texture is referenced through a [`SpriteAsset`] (texture + UV rect + natural size + pivot),
/// so atlases and sprite sheets work by pointing several sprites at one texture.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub asset: AssetHandle<SpriteAsset>,
    /// Sub-rectangle of the asset in normalized `[0,1]^2` asset space (`(0,0)` = the asset's
    /// min-UV corner). Selects a frame of a sheet-like asset; `None` shows the whole asset.
    pub region: Option<Rect>,
    /// Size in world units; `None` uses the asset's natural size.
    pub size: Option<Vec2>,
    /// Pivot `(0,0)` bottom-left .. `(1,1)` top-right, the point that sits at the entity's
    /// position; `None` uses the asset's pivot.
    pub anchor: Option<Vec2>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub tint: Color,
    pub orientation: SpriteOrientation,
    pub blend: SpriteBlend,
    /// Whether scene lights shade this sprite. Unlit sprites show their texture as-is.
    pub lit: bool,
}

impl Sprite {
    pub fn new(name: &str) -> Self {
        Self::from_asset(AssetHandle::from_name(name))
    }
    pub fn from_asset(asset: AssetHandle<SpriteAsset>) -> Self {
        Self {
            asset,
            region: None,
            size: None,
            anchor: None,
            flip_x: false,
            flip_y: false,
            tint: Color::WHITE,
            orientation: SpriteOrientation::default(),
            blend: SpriteBlend::Alpha,
            lit: true,
        }
    }
    pub fn with_region(mut self, region: Rect) -> Self {
        self.region = Some(region);
        self
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
    pub fn with_flip_y(mut self, flip: bool) -> Self {
        self.flip_y = flip;
        self
    }
    pub fn with_orientation(mut self, orientation: SpriteOrientation) -> Self {
        self.orientation = orientation;
        self
    }
    pub fn upright(self) -> Self {
        self.with_orientation(SpriteOrientation::Upright)
    }
    pub fn fixed(self) -> Self {
        self.with_orientation(SpriteOrientation::Fixed)
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
}

/// Optional explicit ordering override for **blended** (transparent) sprites.
///
/// This is *not* position and *not* depth. Alpha-blended sprites are drawn back-to-front by
/// camera-space depth; `RenderOrder` is applied on top of that when you need a deterministic
/// result the depth cannot give you (e.g. a world-space marker that must always draw over
/// everything else, or two coplanar sprites):
///
/// * `layer` dominates depth: all of layer 0 draws before any of layer 1.
/// * within a layer, sprites sort by depth, far to near.
/// * `order` breaks ties between sprites at the same depth.
///
/// Entities without this component behave as `RenderOrder::default()` (0, 0), which leaves depth
/// in charge. Opaque geometry (meshes, cutout sprites, tiles) is resolved by the GPU depth buffer
/// and ignores it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderOrder {
    pub layer: i32,
    pub order: i32,
}

impl RenderOrder {
    pub const fn new(layer: i32, order: i32) -> Self {
        Self { layer, order }
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
    /// A flat color shown as-is: lights do not shade it (distance fog still applies).
    pub fn unlit(base_color: Color) -> Self {
        Self { base_color, lit: false, ..Self::default() }
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

/// Distance fog, stored as a world resource. Blends shaded color toward `color` with true camera
/// distance (`1 - exp(-(density * distance)^2)`), which gives depth cues and hides the far edge of
/// the world. Match the camera's clear color for a seamless horizon. Density 0 disables it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub color: Color,
    pub density: f32,
}

impl Default for Fog {
    fn default() -> Self {
        Self { color: Color::WHITE, density: 0.0 }
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
