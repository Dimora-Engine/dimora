//! Renderable ECS components. They describe *what* something is; *where* it is comes from the
//! entity's `Transform`, the same as for every other spatial object.

use engine_assets::{AssetHandle, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Rect, Vec2};

use crate::render_target::RenderTargetId;

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
/// Extraction rebuilds the quad for the camera that is drawing it. That camera does not write
/// `Transform` or `GlobalTransform`. A child does not inherit its parent's rendered basis: the
/// child reads the simulation hierarchy. See ADR-0021.
///
/// Billboards align to the camera's **view plane** (its right/up axes), not to the camera's
/// position. Every billboard therefore shares one orientation per frame, so a perspective camera
/// never skews or rotates individual sprites; they only change position and apparent size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpriteOrientation {
    /// Faces the view plane, including steep and exact top-down views. Camera roll rotates the
    /// quad in that plane. The entity's position and scale are used, plus its roll about its own
    /// Z axis. Pitch and yaw of the entity are ignored because the camera decides them.
    #[default]
    Billboard,
    /// Stays on world +Y and yaws about that axis toward the camera heading. A steep view
    /// foreshortens it. A view along world up sees it edge-on; the heading is undefined there, so
    /// the quad uses the camera's right axis and stays vertical. That choice is a function of the
    /// current view, with no smoothing. Camera roll does not tilt it. The entity's Z twist is kept.
    Upright,
    /// A plain quad in the entity's local XY plane, facing local `-Z`, fully oriented by the
    /// entity's `Transform`. Ground art uses this with a rotation into that plane. The camera
    /// does not turn it.
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
    /// Ignored when [`Self::material`] resolves.
    pub lit: bool,
    /// `None` keeps tint, [`SpriteBlend`], this lit flag, and the [`SpriteAsset`] texture.
    /// `Some` uses that [`MaterialAsset`] instead. UVs, size, pivot, and pixels per unit stay on
    /// the sprite asset. An albedo `Some` is sampled with those UVs and is not remapped, so the
    /// texture has to share that layout. [`MaterialAsset::default`] is opaque and single-sided.
    pub material: Option<AssetHandle<MaterialAsset>>,
    /// When `Some`, this sprite samples that offscreen target instead of the [`SpriteAsset`]
    /// texture and instead of [`engine_assets::MaterialAsset::albedo`].
    ///
    /// Material alpha class, `base_color`, tint, lighting, and culling stay on the material (or on
    /// the unassigned sprite when `material` is `None`). Size, pivot, and pixels per unit stay on
    /// the sprite asset. The sampled coordinates are the full target in `[0, 1]`, with
    /// [`Self::flip_x`] and [`Self::flip_y`] applied. An atlas [`Self::region`] is not used.
    ///
    /// The sample is valid only after an earlier camera in the same frame has written the target.
    /// The sprite's own camera cannot sample the target it is writing. A missing, removed, or
    /// not-yet-written target uses the missing-texture placeholder. The previous frame's image is
    /// not a fallback. Material revision is unchanged by this field.
    pub render_target: Option<RenderTargetId>,
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
            material: None,
            render_target: None,
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
    pub fn with_material(mut self, material: AssetHandle<MaterialAsset>) -> Self {
        self.material = Some(material);
        self
    }
    /// Samples `target` instead of the sprite asset texture and material albedo.
    pub fn with_render_target(mut self, target: RenderTargetId) -> Self {
        self.render_target = Some(target);
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
    /// Used when [`Self::material_asset`] is `None`. Meshes stay opaque on this path.
    pub material: Material,
    /// `None` keeps [`Self::material`]. `Some` supplies colour, albedo, alpha class, lit, and
    /// culling. [`AlphaMode::Blend`](engine_assets::AlphaMode::Blend) joins the same transparent
    /// sequence as blended sprites. [`RenderOrder`] applies to that class only.
    pub material_asset: Option<AssetHandle<MaterialAsset>>,
}

impl Mesh {
    pub fn new(name: &str) -> Self {
        Self { asset: AssetHandle::from_name(name), material: Material::default(), material_asset: None }
    }
    pub fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }
    pub fn with_material_asset(mut self, material: AssetHandle<MaterialAsset>) -> Self {
        self.material_asset = Some(material);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_appearance_constructors_stay_available() {
        let sprite = Sprite::new("hero").with_tint(Color::rgb(0.2, 0.3, 0.4));
        assert_eq!(sprite.tint, Color::rgb(0.2, 0.3, 0.4));
        assert_eq!(sprite.blend, SpriteBlend::Alpha);
        assert!(sprite.lit);
        assert_eq!(sprite.cutout().blend, SpriteBlend::Cutout);

        let mesh = Mesh::new("crate").with_material(Material::unlit(Color::BLACK));
        assert_eq!(mesh.material.base_color, Color::BLACK);
        assert!(mesh.material.base_texture.is_none());
        assert!(!mesh.material.lit);
        assert_eq!(Mesh::new("crate").material.base_color, Color::WHITE);
        assert!(Sprite::new("hero").material.is_none());
        assert!(Mesh::new("crate").material_asset.is_none());

        let map = TileMap::new("tiles", 4, 4, Vec2::splat(1.0), 2, 2);
        assert!(map.lit);
        assert_eq!(map.tiles.len(), 4);
        assert_eq!(map.get(0, 0), TileMap::EMPTY);
    }
}
