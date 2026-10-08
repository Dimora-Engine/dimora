//! The plain-data snapshot handed to a rendering backend.

use core::ops::Range;

use engine_assets::{AssetHandle, MeshData, Texture};
use engine_core::Color;
use engine_math::{Mat4, Vec3};

use crate::camera::CameraView;

/// Most lights a single frame can use; extras are dropped (nearest to the camera are kept).
pub const MAX_LIGHTS: usize = 16;

/// One sprite (or tile) ready to draw. The unit quad `[0,1]^2` in the XY plane is transformed by
/// `model`, which already contains size, pivot and the entity's world transform.
#[derive(Clone, Copy, Debug)]
pub struct SpriteInstance {
    pub model: Mat4,
    /// `[min_u, min_v, max_u, max_v]`, flips already applied (min may exceed max).
    pub uv: [f32; 4],
    pub tint: [f32; 4],
    pub lit: bool,
    pub texture: AssetHandle<Texture>,
}

/// A run of consecutive instances that share a texture: one instanced draw call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteBatch {
    pub texture: AssetHandle<Texture>,
    pub range: Range<u32>,
}

/// Sprite instances in submission order plus the batches that cover them.
#[derive(Clone, Debug, Default)]
pub struct SpriteLayer {
    pub instances: Vec<SpriteInstance>,
    pub batches: Vec<SpriteBatch>,
}

impl SpriteLayer {
    /// Builds batches from instances already in their final order, merging runs of equal texture.
    /// Never reorders, so the visual result is exactly the sorted order.
    pub fn from_ordered(instances: Vec<SpriteInstance>) -> Self {
        let mut batches: Vec<SpriteBatch> = Vec::new();
        for (i, inst) in instances.iter().enumerate() {
            match batches.last_mut() {
                Some(b) if b.texture == inst.texture => b.range.end = i as u32 + 1,
                _ => batches.push(SpriteBatch { texture: inst.texture, range: i as u32..i as u32 + 1 }),
            }
        }
        Self { instances, batches }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MeshInstance {
    pub model: Mat4,
    pub mesh: AssetHandle<MeshData>,
    pub color: [f32; 4],
    pub texture: Option<AssetHandle<Texture>>,
    pub lit: bool,
}

/// Light in world space with intensity already multiplied into `color`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightData {
    pub kind: crate::LightKind,
    pub position: Vec3,
    /// Direction the light travels (unit).
    pub direction: Vec3,
    pub color: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtractStats {
    pub sprites_visible: u32,
    pub sprites_culled: u32,
    pub meshes_visible: u32,
    pub meshes_culled: u32,
    /// Renderables skipped because an asset they reference is not loaded.
    pub missing_assets: u32,
    pub lights: u32,
}

/// Everything a backend needs to draw one frame. Contains no ECS or asset-manager references.
#[derive(Clone, Debug)]
pub struct RenderScene {
    pub view: CameraView,
    pub clear_color: Color,
    /// Ambient RGB with intensity applied.
    pub ambient: [f32; 3],
    pub lights: Vec<LightData>,
    /// Opaque geometry, drawn first with depth writes: meshes then cutout sprites and tiles.
    pub meshes: Vec<MeshInstance>,
    pub cutout_sprites: SpriteLayer,
    /// Alpha-blended sprites, back-to-front, drawn last with depth test but no depth write.
    pub blended_sprites: SpriteLayer,
    pub stats: ExtractStats,
}
