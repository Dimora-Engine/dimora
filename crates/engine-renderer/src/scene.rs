//! The plain-data snapshot handed to a rendering backend.

use core::ops::Range;

use engine_assets::{AssetHandle, AssetId, MeshData, Texture};
use engine_core::Color;
use engine_math::{Mat4, Vec3};

use crate::camera::CameraView;

/// Most lights a single frame can use; extras are dropped (nearest to the camera are kept).
pub const MAX_LIGHTS: usize = 16;

/// How a sprite instance is shaded. The legacy variants are an unassigned sprite or a tile.
/// The other variants come from a resolved [`engine_assets::MaterialAsset`].
///
/// `cutoff` is the mask threshold. It is not a pipeline. [`Self::pipeline`] ignores it so two masks
/// that share a texture and a cull mode can stay one draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpriteShading {
    LegacyCutout,
    LegacyBlend,
    Opaque { double_sided: bool },
    Mask { cutoff: f32, double_sided: bool },
    Blend { double_sided: bool },
}

impl SpriteShading {
    pub fn pipeline(self) -> SpritePipelineKind {
        match self {
            Self::LegacyCutout => SpritePipelineKind::LegacyCutout,
            Self::LegacyBlend => SpritePipelineKind::LegacyBlend,
            Self::Opaque { double_sided: false } => SpritePipelineKind::OpaqueCulled,
            Self::Opaque { double_sided: true } => SpritePipelineKind::OpaqueDouble,
            Self::Mask { double_sided: false, .. } => SpritePipelineKind::MaskCulled,
            Self::Mask { double_sided: true, .. } => SpritePipelineKind::MaskDouble,
            Self::Blend { double_sided: false } => SpritePipelineKind::BlendCulled,
            Self::Blend { double_sided: true } => SpritePipelineKind::BlendDouble,
        }
    }

    /// Bits of the mask cutoff. Other classes carry zero; their shaders do not read it.
    pub fn cutoff_bits(self) -> u32 {
        match self {
            Self::Mask { cutoff, .. } => cutoff.to_bits(),
            _ => 0,
        }
    }
}

/// Pipeline state for one sprite draw. Cutoff stays on the instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpritePipelineKind {
    LegacyCutout,
    LegacyBlend,
    OpaqueCulled,
    OpaqueDouble,
    MaskCulled,
    MaskDouble,
    BlendCulled,
    BlendDouble,
}

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
    pub shading: SpriteShading,
    /// `Some` when a material handle resolved. A later batch key can use the pair.
    pub material_id: Option<AssetId>,
    pub material_revision: u32,
}

/// A run of consecutive instances that share a texture: one instanced draw call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteBatch {
    pub texture: AssetHandle<Texture>,
    pub pipeline: SpritePipelineKind,
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
                Some(b) if b.texture == inst.texture && b.pipeline == inst.shading.pipeline() => {
                    b.range.end = i as u32 + 1
                }
                _ => batches.push(SpriteBatch {
                    texture: inst.texture,
                    pipeline: inst.shading.pipeline(),
                    range: i as u32..i as u32 + 1,
                }),
            }
        }
        Self { instances, batches }
    }
}

/// Depth-writing mesh class. Blend is not a variant: that material is omitted at extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MeshDraw {
    /// No blend and no alpha discard. Legacy meshes and opaque materials.
    Solid {
        double_sided: bool,
    },
    Mask {
        double_sided: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct MeshInstance {
    pub model: Mat4,
    pub mesh: AssetHandle<MeshData>,
    pub color: [f32; 4],
    pub texture: Option<AssetHandle<Texture>>,
    pub lit: bool,
    pub draw: MeshDraw,
    /// Mask threshold. Solid draws do not read it.
    pub cutoff: f32,
    pub material_id: Option<AssetId>,
    pub material_revision: u32,
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
    /// `[x, y, width, height]` in pixels (origin top-left): the part of the target this camera
    /// draws into. Equal to the whole target unless the camera has a sub-viewport.
    pub viewport_px: [f32; 4],
    /// Ambient RGB with intensity applied.
    pub ambient: [f32; 3],
    /// Fog color (rgb) and density.
    pub fog: [f32; 4],
    pub lights: Vec<LightData>,
    /// Opaque geometry, drawn first with depth writes: meshes, then material opaque sprites,
    /// material mask sprites, then legacy cutout sprites and tiles.
    pub meshes: Vec<MeshInstance>,
    pub opaque_sprites: SpriteLayer,
    pub mask_sprites: SpriteLayer,
    pub cutout_sprites: SpriteLayer,
    /// Alpha-blended sprites, back-to-front, drawn last with depth test but no depth write.
    pub blended_sprites: SpriteLayer,
    pub stats: ExtractStats,
}
