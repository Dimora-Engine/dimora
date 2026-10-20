//! CPU material description.
//!
//! [`MaterialAsset`] is plain data for a future sprite, mesh, or tilemap draw. It holds no GPU
//! object, shader, bind group, pipeline, entity, or world. The renderer does not read it yet.
//! A sprite or mesh with no material handle keeps the appearance it has today.

use engine_core::Color;

use crate::{Asset, AssetHandle, AssetKind, Texture};

/// Mask discard threshold in `0.0..=1.0`.
///
/// [`AlphaCutoff::new`] rejects a non-finite value and a value outside that range, so a mask
/// cannot store a cutoff a future shader would have to guess at. [`AlphaCutoff::DEFAULT`] is
/// `0.5`, the threshold the sprite cutout shader uses today.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlphaCutoff(f32);

impl AlphaCutoff {
    pub const DEFAULT: Self = Self(0.5);

    pub fn new(cutoff: f32) -> Result<Self, MaterialError> {
        if !cutoff.is_finite() {
            return Err(MaterialError::CutoffNotFinite);
        }
        if !(0.0..=1.0).contains(&cutoff) {
            return Err(MaterialError::CutoffOutOfRange { cutoff });
        }
        Ok(Self(cutoff))
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl Default for AlphaCutoff {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How a material treats coverage.
///
/// These are the three classes from ADR-0015. Depth and blend state below are the policy a later
/// checkpoint will apply. This checkpoint does not create that GPU state.
///
/// | Class | Depth test | Depth write | Blending |
/// |---|---|---|---|
/// | [`Opaque`](Self::Opaque) | on | on | off |
/// | [`Mask`](Self::Mask) | on | on | off; fragments below the cutoff are discarded |
/// | [`Blend`](Self::Blend) | on | off | on; the object joins the back-to-front sort |
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum AlphaMode {
    /// Depth test and depth write. No alpha blending.
    #[default]
    Opaque,
    /// Depth test and depth write. Fragments below `cutoff` are discarded. No alpha blending.
    Mask { cutoff: AlphaCutoff },
    /// Depth test, no depth write, alpha blending, and the back-to-front sort.
    Blend,
}

impl AlphaMode {
    /// A mask with an explicit cutoff. `0.0` and `1.0` are accepted.
    pub fn mask(cutoff: f32) -> Result<Self, MaterialError> {
        Ok(Self::Mask { cutoff: AlphaCutoff::new(cutoff)? })
    }

    /// A mask at [`AlphaCutoff::DEFAULT`].
    pub const fn mask_default() -> Self {
        Self::Mask { cutoff: AlphaCutoff::DEFAULT }
    }

    /// The mask cutoff, if this mode discards fragments.
    pub const fn cutoff(self) -> Option<f32> {
        match self {
            Self::Mask { cutoff } => Some(cutoff.get()),
            Self::Opaque | Self::Blend => None,
        }
    }
}

/// Why [`AlphaCutoff::new`] or [`MaterialAsset::validate`] refused a material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MaterialError {
    /// The cutoff was NaN or infinite.
    CutoffNotFinite,
    /// The cutoff was finite and outside `0.0..=1.0`.
    CutoffOutOfRange { cutoff: f32 },
    /// A base-colour channel was NaN or infinite. Values outside `0.0..=1.0` are allowed.
    ColorNotFinite,
}

impl core::fmt::Display for MaterialError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MaterialError::CutoffNotFinite => write!(f, "alpha cutoff must be a finite number"),
            MaterialError::CutoffOutOfRange { cutoff } => {
                write!(f, "alpha cutoff {cutoff} is outside 0.0..=1.0")
            }
            MaterialError::ColorNotFinite => write!(f, "material base color must be finite"),
        }
    }
}

impl std::error::Error for MaterialError {}

/// Shared surface description. Sprites, meshes, and later tilemaps can reference one value.
///
/// The default matches the inline mesh material: linear white, no texture, lit, and opaque.
/// `double_sided` defaults to `false`, which is how meshes are culled today. It does not
/// reproduce a sprite. An unassigned sprite stays on `SpriteBlend`, its own tint, and its own
/// lit flag until a later checkpoint reads a handle.
///
/// `normal` is stored for M14 and is not shaded. Texture handles are not resolved here. An
/// asset that is not loaded yet is still a valid reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialAsset {
    pub base_color: Color,
    pub albedo: Option<AssetHandle<Texture>>,
    pub normal: Option<AssetHandle<Texture>>,
    pub alpha_mode: AlphaMode,
    pub lit: bool,
    pub double_sided: bool,
}

impl Default for MaterialAsset {
    fn default() -> Self {
        Self {
            base_color: Color::WHITE,
            albedo: None,
            normal: None,
            alpha_mode: AlphaMode::Opaque,
            lit: true,
            double_sided: false,
        }
    }
}

impl MaterialAsset {
    pub fn color(base_color: Color) -> Self {
        Self { base_color, ..Self::default() }
    }

    /// Linear colour shown without scene lights. Distance fog is a later renderer concern.
    pub fn unlit(base_color: Color) -> Self {
        Self { base_color, lit: false, ..Self::default() }
    }

    pub fn textured(albedo: AssetHandle<Texture>) -> Self {
        Self { albedo: Some(albedo), ..Self::default() }
    }

    pub fn with_alpha_mode(mut self, alpha_mode: AlphaMode) -> Self {
        self.alpha_mode = alpha_mode;
        self
    }

    pub fn with_normal(mut self, normal: AssetHandle<Texture>) -> Self {
        self.normal = Some(normal);
        self
    }

    pub fn double_sided(mut self) -> Self {
        self.double_sided = true;
        self
    }

    /// Checks colour channels. A mask cutoff is already constrained by [`AlphaCutoff`].
    ///
    /// Finite components may lie outside `0.0..=1.0`. This does not look up textures.
    pub fn validate(&self) -> Result<(), MaterialError> {
        let color = self.base_color;
        if [color.r, color.g, color.b, color.a].iter().all(|channel| channel.is_finite()) {
            Ok(())
        } else {
            Err(MaterialError::ColorNotFinite)
        }
    }
}

impl Asset for MaterialAsset {
    const KIND: AssetKind = AssetKind::Material;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AssetManager;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn defaults_match_the_inline_mesh_material() {
        let material = MaterialAsset::default();
        assert_eq!(material.base_color, Color::WHITE);
        assert!(material.albedo.is_none());
        assert!(material.normal.is_none());
        assert_eq!(material.alpha_mode, AlphaMode::Opaque);
        assert!(material.lit);
        assert!(!material.double_sided);
        assert!(material.validate().is_ok());
    }

    #[test]
    fn alpha_modes_cover_opaque_mask_and_blend() {
        assert_eq!(AlphaMode::default(), AlphaMode::Opaque);
        assert_eq!(AlphaMode::Opaque.cutoff(), None);
        assert_eq!(AlphaMode::Blend.cutoff(), None);

        let mask = AlphaMode::mask_default();
        assert_eq!(mask.cutoff(), Some(0.5));
        assert_eq!(AlphaMode::mask(0.25).unwrap().cutoff(), Some(0.25));
        assert_eq!(AlphaMode::mask(0.0).unwrap().cutoff(), Some(0.0));
        assert_eq!(AlphaMode::mask(1.0).unwrap().cutoff(), Some(1.0));
    }

    #[test]
    fn invalid_cutoffs_are_rejected() {
        assert_eq!(AlphaCutoff::new(-0.01), Err(MaterialError::CutoffOutOfRange { cutoff: -0.01 }));
        assert_eq!(AlphaCutoff::new(1.01), Err(MaterialError::CutoffOutOfRange { cutoff: 1.01 }));
        assert_eq!(AlphaMode::mask(f32::NAN).unwrap_err(), MaterialError::CutoffNotFinite);
        assert_eq!(AlphaMode::mask(f32::INFINITY).unwrap_err(), MaterialError::CutoffNotFinite);
        assert_eq!(AlphaMode::mask(f32::NEG_INFINITY).unwrap_err(), MaterialError::CutoffNotFinite);
    }

    #[test]
    fn colors_textures_and_flags_round_trip() {
        assert_send_sync::<MaterialAsset>();
        let albedo = AssetHandle::<Texture>::from_name("albedo");
        let normal = AssetHandle::<Texture>::from_name("normal");
        let material = MaterialAsset {
            albedo: Some(albedo),
            ..MaterialAsset::color(Color::rgb(2.5, -0.25, 0.0)).with_normal(normal).with_alpha_mode(AlphaMode::Blend)
        };
        assert_eq!(material.base_color, Color::rgb(2.5, -0.25, 0.0));
        assert_eq!(material.albedo, Some(albedo));
        assert_eq!(material.normal, Some(normal));
        assert_eq!(material.alpha_mode, AlphaMode::Blend);
        assert!(material.validate().is_ok(), "finite HDR and negative channels stay valid");

        let unlit = MaterialAsset::unlit(Color::BLACK);
        assert!(!unlit.lit);
        let lit = MaterialAsset { lit: true, ..unlit };
        assert!(lit.lit);
        let sided = lit.double_sided();
        assert!(sided.double_sided);
        assert!(!MaterialAsset::default().double_sided);
    }

    #[test]
    fn non_finite_base_color_is_rejected_without_looking_up_textures() {
        let missing = AssetHandle::<Texture>::from_name("not-loaded");
        let material =
            MaterialAsset { base_color: Color::rgba(1.0, f32::NAN, 1.0, 1.0), ..MaterialAsset::textured(missing) };
        assert_eq!(material.albedo, Some(missing));
        assert_eq!(material.validate(), Err(MaterialError::ColorNotFinite));
        assert_eq!(
            MaterialAsset::color(Color::rgba(1.0, 1.0, f32::INFINITY, 1.0)).validate(),
            Err(MaterialError::ColorNotFinite)
        );
    }

    #[test]
    fn materials_register_and_share_texture_handles() {
        let mut assets = AssetManager::in_memory();
        let texture = assets.insert("albedo", crate::Texture::solid(1, 1, [1, 2, 3, 4])).unwrap();
        let first = assets
            .insert("paint", MaterialAsset::textured(texture).with_alpha_mode(AlphaMode::mask_default()))
            .unwrap();
        let second =
            assets.insert("glass", MaterialAsset::textured(texture).with_alpha_mode(AlphaMode::Blend)).unwrap();
        assert_eq!(assets.revision(first), 1);
        assert_eq!(assets.get(first).unwrap().albedo, Some(texture));
        assert_eq!(assets.get(second).unwrap().albedo, Some(texture));
        assert_eq!(assets.get(first).unwrap().albedo.unwrap().id(), texture.id());
        assert_eq!(assets.get(first).unwrap().alpha_mode.cutoff(), Some(0.5));

        let revision_before = assets.revision(first);
        assets.insert("paint", MaterialAsset::unlit(Color::rgb(4.0, 0.0, 0.0))).unwrap();
        assert!(assets.revision(assets.material("paint")) > revision_before);
        let replaced = assets.get(assets.material("paint")).unwrap();
        assert_eq!(replaced.base_color, Color::rgb(4.0, 0.0, 0.0));
        assert!(replaced.albedo.is_none());
        assert!(!replaced.lit);
        assert!(assets.get(texture).is_some());
    }
}
