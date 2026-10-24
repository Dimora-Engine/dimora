//! The plain-data snapshot handed to a rendering backend.

use core::ops::Range;

use engine_assets::{AssetHandle, AssetId, MeshData, Texture};
use engine_core::Color;
use engine_math::{Mat4, Vec3};

use crate::camera::{CameraClear, CameraOutput, CameraView};
use crate::debug::DebugSegment;
use crate::render_target::{RenderTargetDesc, RenderTargetId};

/// Most lights a single frame can use; extras are dropped (nearest to the camera are kept).
pub const MAX_LIGHTS: usize = 16;

/// How a sprite instance is shaded. The legacy variants are an unassigned sprite or a tile.
/// The other variants come from a resolved [`engine_assets::MaterialAsset`].
///
/// `cutoff` is a copy of the one threshold on [`engine_assets::AlphaMode::Mask`]. A
/// [`engine_assets::MaterialAsset`] has one cutoff. `Sprite` and `Mesh` have no cutoff field, so
/// two renderables that share one material discard at that same value. The shader reads the copy
/// from the instance. [`Self::pipeline`] ignores it. A different cutoff is a different material,
/// and material identity already splits the batch. There is no per-sprite cutoff override.
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
    /// Sampled offscreen target. Ignored when [`Self::unavailable_target`] is set. When `Some`
    /// and the target is available, [`Self::texture`] is not the sampled image.
    pub render_target: Option<RenderTargetId>,
    /// The explicit render-target source cannot be sampled in this camera's pass. The backend
    /// binds the missing-texture placeholder.
    pub unavailable_target: bool,
    pub shading: SpriteShading,
    /// `Some` when a material handle resolved. Together with [`Self::material_revision`] this is part
    /// of the batch key. `None` is an unassigned sprite, not a shared material.
    pub material_id: Option<AssetId>,
    pub material_revision: u32,
    /// Opt-in coplanar level, `0..=7`. `None` is ordinary spatial depth, including level 0's absence.
    /// Not a batch key and not a blend sort key.
    pub coplanar_level: Option<u8>,
}

impl SpriteInstance {
    /// Whether these sprites can share one instanced draw.
    ///
    /// The key is the pipeline, the material identity and revision, and the texture. That texture
    /// handle is the sampled view and the sampler: one texture asset has one sampler descriptor.
    /// Tint, UV, the model, `lit`, and [`Self::coplanar_level`] stay on the instance, so they do not split the draw. The mask
    /// cutoff is copied from the shared material onto each instance. It is not a second cutoff and
    /// it is not a batch key: one material revision has one cutoff. Unassigned sprites use
    /// `material_id: None` and revision 0. They are not given a fabricated material, and they batch
    /// with each other only when the texture and pipeline match.
    pub fn shares_batch(&self, other: &Self) -> bool {
        self.sample_matches(other)
            && self.shading.pipeline() == other.shading.pipeline()
            && self.material_id == other.material_id
            && self.material_revision == other.material_revision
    }

    fn sample_matches(&self, other: &Self) -> bool {
        if self.unavailable_target || other.unavailable_target {
            return self.unavailable_target && other.unavailable_target;
        }
        match (self.render_target, other.render_target) {
            (Some(left), Some(right)) => left == right,
            (None, None) => self.texture == other.texture,
            _ => false,
        }
    }
}

/// A run of consecutive instances that share a batch key: one instanced draw call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteBatch {
    pub texture: AssetHandle<Texture>,
    /// See [`SpriteInstance::render_target`].
    pub render_target: Option<RenderTargetId>,
    /// See [`SpriteInstance::unavailable_target`].
    pub unavailable_target: bool,
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
    /// Builds batches from instances already in their final order. A run grows only while the next
    /// instance [`SpriteInstance::shares_batch`] with the one before it. This does not reorder, so a
    /// caller that has already sorted blended sprites keeps that order.
    pub fn from_ordered(instances: Vec<SpriteInstance>) -> Self {
        let mut batches: Vec<SpriteBatch> = Vec::new();
        for (i, inst) in instances.iter().enumerate() {
            if i > 0 && instances[i - 1].shares_batch(inst) {
                batches.last_mut().expect("the previous instance opened a batch").range.end = i as u32 + 1;
            } else {
                batches.push(SpriteBatch {
                    texture: inst.texture,
                    render_target: inst.render_target,
                    unavailable_target: inst.unavailable_target,
                    pipeline: inst.shading.pipeline(),
                    range: i as u32..i as u32 + 1,
                });
            }
        }
        Self { instances, batches }
    }
}

/// GPU state for one mesh draw. Variants are the real pipeline differences, not material ids.
///
/// [`Self::Blend`] is depth-tested, does not write depth, and joins the combined transparent
/// sequence. Opaque and mask meshes stay in [`CameraPass::meshes`] and ignore [`crate::RenderOrder`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MeshDraw {
    /// No blend and no alpha discard. Legacy meshes and opaque materials.
    Solid {
        double_sided: bool,
    },
    Mask {
        double_sided: bool,
    },
    /// Alpha blending. Depth test stays on. Depth write stays off.
    Blend {
        double_sided: bool,
    },
}

/// One run in the combined back-to-front sequence.
///
/// Sprite ranges index [`CameraPass::blended_sprites`]. Mesh ranges index
/// [`CameraPass::blended_meshes`]. A run grows only while the next item is the same kind and
/// [`SpriteInstance::shares_batch`] or [`MeshInstance::shares_batch`] with the previous one.
/// A mesh between two sprites keeps those sprites as separate draws, even when the sprites match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransparentBatch {
    Sprites(SpriteBatch),
    Meshes { mesh: AssetHandle<MeshData>, texture: Option<AssetHandle<Texture>>, draw: MeshDraw, range: Range<u32> },
}

#[derive(Clone, Copy, Debug)]
pub struct MeshInstance {
    pub model: Mat4,
    pub mesh: AssetHandle<MeshData>,
    pub color: [f32; 4],
    pub texture: Option<AssetHandle<Texture>>,
    pub lit: bool,
    pub draw: MeshDraw,
    /// Copy of the material's one mask threshold. Solid and blend draws do not read it. It is not a
    /// batch key, and a mesh has no separate cutoff field.
    pub cutoff: f32,
    /// `None` is an unassigned mesh. It is part of [`Self::shares_batch`], not a fabricated material.
    pub material_id: Option<AssetId>,
    pub material_revision: u32,
}

impl MeshInstance {
    /// Whether these meshes can share one instanced draw.
    ///
    /// Geometry must be the same mesh asset. The texture handle, when present, is the view and the
    /// sampler. Colour, `lit`, and the model stay on the instance. Cutoff is the material's one
    /// threshold, copied here. An unassigned mesh has no material id. A different mesh asset is a
    /// different draw; sprites never use this layout.
    pub fn shares_batch(&self, other: &Self) -> bool {
        self.mesh == other.mesh
            && self.texture == other.texture
            && self.draw == other.draw
            && self.material_id == other.material_id
            && self.material_revision == other.material_revision
    }

    /// End index of the contiguous run that starts at `start`.
    pub fn run_end(meshes: &[Self], start: usize) -> usize {
        let first = meshes[start];
        start + 1 + meshes[start + 1..].iter().take_while(|mesh| first.shares_batch(mesh)).count()
    }
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
    /// Sprites that named a render target this camera could not sample, plus cameras whose output
    /// target is missing. Counted once per occurrence, not once per logged line.
    pub missing_targets: u32,
    pub lights: u32,
    /// Tile cells read while building instances. Empty cells in a visible chunk count. Cells in a
    /// rejected chunk do not. Summed across cameras on [`RenderScene::stats`].
    pub tiles_visited: u32,
    /// Tilemap chunks whose bounds were tested against a camera. Summed across cameras.
    pub chunks_tested: u32,
    /// Chunks rejected before their cells were read. Summed across cameras.
    pub chunks_rejected: u32,
    /// Chunk occupancy records scanned because they were dirty. A chunk is counted once per frame
    /// even when several cameras draw the map.
    pub chunks_rebuilt: u32,
    /// Debug segments copied from [`crate::DebugDraw`] for this frame. Counted once, not once per
    /// camera. Zero when the resource is absent or empty.
    pub debug_segments: u32,
}

/// One active camera, already culled and sorted for its own view.
///
/// The lists are this camera's submissions. Another pass does not share them. Mesh vertex buffers
/// stay in the asset cache; these instances are transforms and material keys only.
#[derive(Clone, Debug)]
pub struct CameraPass {
    /// `Entity::id` of the source camera. Plain data, not a world borrow.
    pub camera_id: u32,
    /// [`crate::Camera::order`]. Passes in a [`RenderScene`] are already sorted by this, then by
    /// [`Self::camera_id`].
    pub order: i32,
    pub view: CameraView,
    /// Surface or offscreen destination. Passes are not grouped by this; draw order is
    /// [`Self::order`] then [`Self::camera_id`].
    pub output: CameraOutput,
    pub clear: CameraClear,
    pub clear_color: Color,
    /// `[x, y, width, height]` in pixels (origin top-left): the part of the target this camera
    /// draws into. Equal to the whole target unless the camera has a sub-viewport.
    pub viewport_px: [f32; 4],
    /// Lights for this camera. Above [`MAX_LIGHTS`], positional lights are the nearest to this
    /// camera. Directional lights are kept. The positions stay in world space.
    pub lights: Vec<LightData>,
    /// Opaque geometry, drawn first with depth writes: meshes, then material opaque sprites,
    /// material mask sprites, then legacy cutout sprites and tiles.
    pub meshes: Vec<MeshInstance>,
    pub opaque_sprites: SpriteLayer,
    pub mask_sprites: SpriteLayer,
    pub cutout_sprites: SpriteLayer,
    /// Blended sprites in this camera's transparent-sequence order.
    pub blended_sprites: SpriteLayer,
    /// Blended meshes in this camera's transparent-sequence order. Ranges in [`Self::transparent`]
    /// index this.
    pub blended_meshes: Vec<MeshInstance>,
    /// Blended sprites and blended meshes, one back-to-front order for this camera. The backend
    /// walks this list and does not sort it again.
    pub transparent: Vec<TransparentBatch>,
    /// Visibility counts for this camera. [`ExtractStats::missing_assets`] stays 0 here; missing
    /// assets are counted once on [`RenderScene::stats`].
    pub stats: ExtractStats,
}

/// Everything a backend needs to draw one frame. Contains no ECS or asset-manager references.
///
/// [`Self::passes`] is every active camera in draw order (ascending `order`, then entity id).
/// There is no separate `view` field. A frame with one camera has one pass; [`Self::sole_pass`]
/// is that pass. Ambient and fog are world resources and are shared. Lights are per pass because
/// the nearest-light cap depends on the camera position.
#[derive(Clone, Debug)]
pub struct RenderScene {
    pub passes: Vec<CameraPass>,
    /// Every live offscreen target this frame, copied from [`crate::RenderTargetStore`].
    /// The renderer allocates and resizes from this list before it records passes. A removed
    /// target is absent, and its GPU image is dropped. These descriptions hold no GPU objects.
    pub targets: Vec<RenderTargetDesc>,
    /// Ambient RGB with intensity applied.
    pub ambient: [f32; 3],
    /// Fog color (rgb) and density.
    pub fog: [f32; 4],
    /// World-unit step for [`crate::CoplanarPriority`]. `0` means an invalid explicit setting
    /// disabled the displacement. Sprites without the component ignore it.
    pub coplanar_thickness: f32,
    /// World-space debug segments for every camera this frame. Copied from [`crate::DebugDraw`].
    /// Not frustum-culled, not split per pass, and not consumed by a camera. Empty when that
    /// resource is absent or has no accepted segments. The backend draws this as one line batch
    /// after the transparent sequence when it is non-empty.
    pub debug_lines: Vec<DebugSegment>,
    /// Sum of the pass visibility counts. `missing_assets` is the shared resolution count, once
    /// per missing reference, not once per camera. `sprites_visible` counts a sprite again for
    /// each camera that draws it.
    pub stats: ExtractStats,
}

impl RenderScene {
    /// The only camera pass.
    ///
    /// Panics when this frame has zero or several passes. Single-camera callers use this.
    /// A multi-camera frame is read from [`Self::passes`], which stays the only stored list.
    pub fn sole_pass(&self) -> &CameraPass {
        assert!(self.passes.len() == 1, "sole_pass requires exactly one camera pass, found {}", self.passes.len());
        &self.passes[0]
    }
}
