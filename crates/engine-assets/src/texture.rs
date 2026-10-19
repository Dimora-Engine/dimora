use crate::{Asset, AssetError, AssetKind};

/// How a texture is filtered when it is magnified or minified.
///
/// [`SamplerDesc`] is the configuration the GPU reads. [`Texture::filter`] is the legacy
/// projection of that descriptor: linear only when minification and magnification are both linear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureFilter {
    /// Crisp texels; the default for pixel art.
    #[default]
    Nearest,
    Linear,
}

/// How GPU sampling should interpret the RGBA8 bytes.
///
/// [`ColorSpace::Srgb`] selects `Rgba8UnormSrgb`, so the GPU decodes on sample. [`ColorSpace::Linear`]
/// selects `Rgba8Unorm`, so the bytes are not decoded. The CPU stores the bytes unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorSpace {
    /// Bytes are sRGB. Hardware decodes them to linear when the GPU format is sRGB.
    #[default]
    Srgb,
    /// Bytes are already linear. A later upload uses a linear format and does not decode them.
    Linear,
}

/// What happens when a sample falls outside the texture.
///
/// `Clamp` is clamp-to-edge. Clamp-to-border is not part of this set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AddressMode {
    /// Repeat the edge texel.
    #[default]
    Clamp,
    /// Tile the texture.
    Repeat,
    /// Tile the texture, mirroring on each repeat.
    Mirror,
}

/// How to blend adjacent mip levels.
///
/// Absence of a filter is [`Option::None`] on [`SamplerDesc::mip_filter`], which samples the base
/// level. This enum is not a request to generate mipmaps. That request is [`Mipmaps`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MipFilter {
    Nearest,
    Linear,
}

/// Which mip levels a texture asks the GPU to store.
///
/// Independent of [`SamplerDesc::mip_filter`]. Selecting a mip filter does not select
/// [`Mipmaps::Generate`], and [`Mipmaps::Generate`] does not select a mip filter.
///
/// [`Mipmaps::None`] together with a mip filter is valid. A mip filter does not allocate levels.
/// [`Mipmaps::Generate`] asks for `floor(log2(max(width, height))) + 1` levels. The GPU backend
/// builds that chain when the device can store it. A one-level texture keeps a wide LOD range so
/// minification still uses the minification filter; the single-level view is what limits the sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mipmaps {
    /// Base level only.
    #[default]
    None,
    /// A full chain. The GPU backend allocates `floor(log2(max(width, height))) + 1` levels and
    /// fills every level after the base. A device that cannot store that chain keeps level 0.
    Generate,
}

impl Mipmaps {
    /// Levels this request names before a device fallback.
    ///
    /// [`Mipmaps::None`] is 1. [`Mipmaps::Generate`] is `floor(log2(max(width, height))) + 1`.
    /// A zero side is treated as 1 so the count stays defined. This does not read a device.
    pub fn level_count(self, width: u32, height: u32) -> u32 {
        match self {
            Mipmaps::None => 1,
            Mipmaps::Generate => width.max(height).max(1).ilog2() + 1,
        }
    }
}

/// CPU sampler configuration.
///
/// The GPU backend calls [`SamplerDesc::normalized`] before creating a sampler, including when these
/// public fields were written directly. Anisotropy `1` means anisotropic filtering is off. The engine
/// cap of [`SamplerDesc::MAX_ANISOTROPY`] is the wgpu-hal interface range from ADR-0017, not a value
/// read from a device. A device that lacks anisotropic filtering forces the effective clamp to 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    /// `None` samples the base level. `Some` describes blending between levels that exist.
    pub mip_filter: Option<MipFilter>,
    pub address_u: AddressMode,
    pub address_v: AddressMode,
    /// `1` is off. `0` is rejected by [`SamplerDesc::normalized`].
    pub anisotropy: u8,
}

impl Default for SamplerDesc {
    fn default() -> Self {
        Self {
            min_filter: TextureFilter::Nearest,
            mag_filter: TextureFilter::Nearest,
            mip_filter: None,
            address_u: AddressMode::Clamp,
            address_v: AddressMode::Clamp,
            anisotropy: 1,
        }
    }
}

impl SamplerDesc {
    /// Upper anisotropy stored after normalization when min, mag, and mip filters are all linear.
    ///
    /// Matches the wgpu-hal sampler clamp of 16. Device downlevel behaviour is applied by the GPU
    /// checkpoint, not here.
    pub const MAX_ANISOTROPY: u8 = 16;

    /// Validate and normalize this descriptor on the CPU.
    ///
    /// Anisotropy below 1 is [`SamplerConfigError::AnisotropyBelowOne`]. Anisotropy above 1 stays
    /// only when minification, magnification, and mip filtering are all linear; otherwise it
    /// becomes 1 and the filters are unchanged. Linear anisotropy above [`Self::MAX_ANISOTROPY`]
    /// becomes that cap. A non-power-of-two value is kept. The same input always produces the same
    /// output, and normalizing an `Ok` result again returns that result.
    ///
    /// This function does not read a device and does not turn nearest filtering into linear filtering.
    pub fn normalized(self) -> Result<Self, SamplerConfigError> {
        if self.anisotropy < 1 {
            return Err(SamplerConfigError::AnisotropyBelowOne);
        }
        let linear_filters = self.min_filter == TextureFilter::Linear
            && self.mag_filter == TextureFilter::Linear
            && self.mip_filter == Some(MipFilter::Linear);
        let anisotropy =
            if self.anisotropy > 1 && !linear_filters { 1 } else { self.anisotropy.min(Self::MAX_ANISOTROPY) };
        Ok(Self { anisotropy, ..self })
    }
}

/// CPU rejection of a sampler descriptor. GPU sampler creation is a later checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerConfigError {
    /// Anisotropy was `0`. The smallest valid value is `1`, which means anisotropy is off.
    AnisotropyBelowOne,
}

impl std::fmt::Display for SamplerConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SamplerConfigError::AnisotropyBelowOne => write!(f, "sampler anisotropy must be at least 1"),
        }
    }
}

impl std::error::Error for SamplerConfigError {}

/// CPU-side image, tightly packed RGBA8.
///
/// [`Texture::sampler`] is the sampling configuration. The GPU reads it after
/// [`SamplerDesc::normalized`], including when the public fields were changed without
/// [`Texture::with_sampler`]. [`Texture::color_space`] selects the GPU format. The RGBA bytes are
/// stored as given. [`Texture::mips`] selects how many levels the GPU backend allocates.
///
/// [`Texture::filter`] is a legacy projection of min and mag, not a second configuration. It is
/// private so a caller cannot publish a filter the backend will ignore.
#[derive(Clone, Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Legacy projection of [`Texture::sampler`]'s min and mag filters.
    ///
    /// [`Texture::with_filter`] sets this and both filters. [`Texture::with_sampler`] sets it to
    /// [`TextureFilter::Linear`] only when both filters are linear. The GPU does not read it.
    filter: TextureFilter,
    pub color_space: ColorSpace,
    pub mips: Mipmaps,
    pub sampler: SamplerDesc,
}

impl Texture {
    pub fn from_rgba8(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert_eq!(rgba.len(), (width * height * 4) as usize, "RGBA8 buffer has the wrong size");
        Self {
            width,
            height,
            rgba,
            filter: TextureFilter::default(),
            color_space: ColorSpace::default(),
            mips: Mipmaps::default(),
            sampler: SamplerDesc::default(),
        }
    }

    pub fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        Self::from_rgba8(width, height, rgba.repeat((width * height) as usize))
    }

    /// Builds a texture by evaluating `f(x, y)` for every pixel (y = 0 is the top row).
    pub fn from_fn(width: u32, height: u32, mut f: impl FnMut(u32, u32) -> [u8; 4]) -> Self {
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                data.extend_from_slice(&f(x, y));
            }
        }
        Self::from_rgba8(width, height, data)
    }

    /// Legacy single-filter summary. Linear only when minification and magnification are both linear.
    ///
    /// The GPU samples [`Texture::sampler`], not this value.
    pub fn filter(&self) -> TextureFilter {
        self.filter
    }

    /// Sets the legacy filter and both min and mag filters to `filter`.
    ///
    /// Address modes, the mip filter, colour space, and mipmap generation stay. Anisotropy stays
    /// too, unless it is above 1 and the resulting filters are not all linear, in which case it
    /// becomes 1. That is the same rule as [`SamplerDesc::normalized`].
    pub fn with_filter(mut self, filter: TextureFilter) -> Self {
        self.filter = filter;
        self.sampler.min_filter = filter;
        self.sampler.mag_filter = filter;
        if let Ok(sampler) = self.sampler.normalized() {
            self.sampler = sampler;
        }
        self
    }

    /// Stores the colour space. The RGBA bytes are not converted.
    pub fn with_color_space(mut self, color_space: ColorSpace) -> Self {
        self.color_space = color_space;
        self
    }

    /// Stores the mipmap request. Does not generate levels and does not change [`SamplerDesc::mip_filter`].
    pub fn with_mipmaps(mut self, mips: Mipmaps) -> Self {
        self.mips = mips;
        self
    }

    /// Stores `sampler` after [`SamplerDesc::normalized`].
    ///
    /// Min and mag filters are kept even when they differ. [`Texture::filter`] becomes
    /// [`TextureFilter::Linear`] only when both are linear. The GPU reads `sampler`, so the two
    /// filters stay independent.
    pub fn with_sampler(mut self, sampler: SamplerDesc) -> Result<Self, SamplerConfigError> {
        let sampler = sampler.normalized()?;
        self.filter = legacy_filter(sampler);
        self.sampler = sampler;
        Ok(self)
    }
}

/// The single value [`Texture::filter`] reports. The GPU does not sample from it.
fn legacy_filter(sampler: SamplerDesc) -> TextureFilter {
    if sampler.min_filter == TextureFilter::Linear && sampler.mag_filter == TextureFilter::Linear {
        TextureFilter::Linear
    } else {
        TextureFilter::Nearest
    }
}

impl Asset for Texture {
    const KIND: AssetKind = AssetKind::Texture;

    fn decode(bytes: &[u8]) -> Result<Self, AssetError> {
        let img = image::load_from_memory(bytes).map_err(|e| AssetError::Decode(e.to_string()))?.to_rgba8();
        Ok(Texture::from_rgba8(img.width(), img.height(), img.into_raw()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_sampling_defaults(tex: &Texture) {
        assert_eq!(tex.color_space, ColorSpace::Srgb);
        assert_eq!(tex.mips, Mipmaps::None);
        assert_eq!(tex.filter(), TextureFilter::Nearest);
        assert_eq!(tex.sampler, SamplerDesc::default());
    }

    #[test]
    fn from_rgba8_defaults_match_todays_sampling() {
        let tex = Texture::from_rgba8(1, 1, vec![9, 8, 7, 6]);
        assert_sampling_defaults(&tex);
        assert_eq!(tex.rgba, [9, 8, 7, 6]);
    }

    #[test]
    fn solid_uses_existing_defaults() {
        let tex = Texture::solid(2, 1, [1, 2, 3, 4]);
        assert_sampling_defaults(&tex);
        assert_eq!(tex.rgba, [1, 2, 3, 4, 1, 2, 3, 4]);
    }

    #[test]
    fn from_fn_uses_existing_defaults() {
        let tex = Texture::from_fn(2, 1, |x, _| [x as u8, 0, 0, 255]);
        assert_sampling_defaults(&tex);
        assert_eq!(tex.rgba, [0, 0, 0, 255, 1, 0, 0, 255]);
    }

    #[test]
    fn png_decode_uses_the_same_defaults() {
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255];
        let mut png = Vec::new();
        image::write_buffer_with_format(
            &mut std::io::Cursor::new(&mut png),
            &rgba,
            2,
            1,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap();
        let decoded = Texture::decode(&png).unwrap();
        assert_sampling_defaults(&decoded);
        assert_eq!(decoded.rgba, rgba);
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 1);
    }

    #[test]
    fn with_filter_sets_min_and_mag() {
        let tex = Texture::solid(1, 1, [0; 4]).with_filter(TextureFilter::Linear);
        assert_eq!(tex.filter(), TextureFilter::Linear);
        assert_eq!(tex.sampler.min_filter, TextureFilter::Linear);
        assert_eq!(tex.sampler.mag_filter, TextureFilter::Linear);
    }

    #[test]
    fn with_filter_preserves_unrelated_sampler_fields() {
        let tex = Texture::solid(1, 1, [0; 4])
            .with_color_space(ColorSpace::Linear)
            .with_mipmaps(Mipmaps::Generate)
            .with_sampler(SamplerDesc {
                address_u: AddressMode::Repeat,
                address_v: AddressMode::Mirror,
                mip_filter: Some(MipFilter::Nearest),
                anisotropy: 8,
                ..SamplerDesc::default()
            })
            .unwrap()
            .with_filter(TextureFilter::Linear);
        assert_eq!(tex.color_space, ColorSpace::Linear);
        assert_eq!(tex.mips, Mipmaps::Generate);
        assert_eq!(tex.sampler.address_u, AddressMode::Repeat);
        assert_eq!(tex.sampler.address_v, AddressMode::Mirror);
        assert_eq!(tex.sampler.mip_filter, Some(MipFilter::Nearest));
        assert_eq!(tex.sampler.anisotropy, 1, "nearest mip filtering drops anisotropy above 1");
        assert_eq!(tex.sampler.min_filter, TextureFilter::Linear);
        assert_eq!(tex.sampler.mag_filter, TextureFilter::Linear);
        assert_eq!(tex.rgba, [0, 0, 0, 0]);
    }

    #[test]
    fn with_filter_drops_anisotropy_the_new_filters_cannot_keep() {
        let tex = Texture::solid(1, 1, [0; 4])
            .with_sampler(SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Linear,
                mip_filter: Some(MipFilter::Linear),
                address_u: AddressMode::Repeat,
                anisotropy: 8,
                ..SamplerDesc::default()
            })
            .unwrap()
            .with_filter(TextureFilter::Nearest);
        assert_eq!(tex.filter(), TextureFilter::Nearest);
        assert_eq!(tex.sampler.min_filter, TextureFilter::Nearest);
        assert_eq!(tex.sampler.mag_filter, TextureFilter::Nearest);
        assert_eq!(tex.sampler.anisotropy, 1);
        assert_eq!(tex.sampler.mip_filter, Some(MipFilter::Linear));
        assert_eq!(tex.sampler.address_u, AddressMode::Repeat);
    }

    #[test]
    fn mixed_min_and_mag_stay_distinguishable() {
        let tex = Texture::solid(1, 1, [0; 4])
            .with_sampler(SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Nearest,
                ..SamplerDesc::default()
            })
            .unwrap();
        assert_eq!(tex.sampler.min_filter, TextureFilter::Linear);
        assert_eq!(tex.sampler.mag_filter, TextureFilter::Nearest);
        assert_eq!(tex.filter(), TextureFilter::Nearest);
    }

    #[test]
    fn with_sampler_stores_the_normalized_descriptor() {
        let tex = Texture::solid(1, 1, [0; 4])
            .with_sampler(SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Linear,
                mip_filter: Some(MipFilter::Linear),
                anisotropy: 32,
                ..SamplerDesc::default()
            })
            .unwrap();
        assert_eq!(tex.sampler.anisotropy, SamplerDesc::MAX_ANISOTROPY);
        assert_eq!(tex.filter(), TextureFilter::Linear);
        assert!(Texture::solid(1, 1, [0; 4])
            .with_sampler(SamplerDesc { anisotropy: 0, ..SamplerDesc::default() })
            .is_err());
    }

    #[test]
    fn a_direct_filter_write_does_not_update_the_sampler() {
        // `filter` is private. This module can still assign it; other crates cannot. The assignment
        // does not change `sampler`, and the GPU reads `sampler`.
        let mut tex = Texture::solid(1, 1, [0; 4]);
        tex.filter = TextureFilter::Linear;
        assert_eq!(tex.filter(), TextureFilter::Linear);
        assert_eq!(tex.sampler.min_filter, TextureFilter::Nearest);
        assert_eq!(tex.sampler.mag_filter, TextureFilter::Nearest);
    }

    #[test]
    fn anisotropy_zero_is_rejected() {
        let desc = SamplerDesc { anisotropy: 0, ..SamplerDesc::default() };
        assert_eq!(desc.normalized(), Err(SamplerConfigError::AnisotropyBelowOne));
    }

    #[test]
    fn anisotropy_one_is_accepted() {
        let desc = SamplerDesc::default();
        assert_eq!(desc.normalized().unwrap(), desc);
    }

    #[test]
    fn anisotropy_with_nearest_keeps_nearest() {
        let desc = SamplerDesc {
            min_filter: TextureFilter::Nearest,
            mag_filter: TextureFilter::Linear,
            mip_filter: Some(MipFilter::Linear),
            anisotropy: 8,
            ..SamplerDesc::default()
        };
        let out = desc.normalized().unwrap();
        assert_eq!(out.anisotropy, 1);
        assert_eq!(out.min_filter, TextureFilter::Nearest);
        assert_eq!(out.mag_filter, TextureFilter::Linear);
        assert_eq!(out.mip_filter, Some(MipFilter::Linear));
    }

    #[test]
    fn anisotropy_without_linear_mips_drops_to_one() {
        for mip_filter in [None, Some(MipFilter::Nearest)] {
            let desc = SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Linear,
                mip_filter,
                anisotropy: 8,
                ..SamplerDesc::default()
            };
            let out = desc.normalized().unwrap();
            assert_eq!(out.anisotropy, 1);
            assert_eq!(out.mip_filter, mip_filter);
            assert_eq!(out.min_filter, TextureFilter::Linear);
        }
    }

    #[test]
    fn linear_anisotropy_from_2_through_16_is_kept() {
        for anisotropy in 2..=16 {
            let desc = SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Linear,
                mip_filter: Some(MipFilter::Linear),
                anisotropy,
                ..SamplerDesc::default()
            };
            assert_eq!(desc.normalized().unwrap().anisotropy, anisotropy);
        }
    }

    #[test]
    fn non_power_of_two_anisotropy_is_kept() {
        for anisotropy in [3, 5, 6, 7] {
            let desc = SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Linear,
                mip_filter: Some(MipFilter::Linear),
                anisotropy,
                ..SamplerDesc::default()
            };
            assert_eq!(desc.normalized().unwrap().anisotropy, anisotropy);
        }
    }

    #[test]
    fn linear_anisotropy_above_16_clamps_to_16() {
        let desc = SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Linear,
            mip_filter: Some(MipFilter::Linear),
            anisotropy: 32,
            ..SamplerDesc::default()
        };
        assert_eq!(desc.normalized().unwrap().anisotropy, 16);
    }

    #[test]
    fn anisotropy_above_16_with_nearest_becomes_one() {
        let desc = SamplerDesc { anisotropy: 32, ..SamplerDesc::default() };
        let out = desc.normalized().unwrap();
        assert_eq!(out.anisotropy, 1);
        assert_eq!(out.min_filter, TextureFilter::Nearest);
    }

    #[test]
    fn normalization_is_idempotent() {
        let inputs = [
            SamplerDesc::default(),
            SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Linear,
                mip_filter: Some(MipFilter::Linear),
                anisotropy: 3,
                address_u: AddressMode::Repeat,
                ..SamplerDesc::default()
            },
            SamplerDesc {
                min_filter: TextureFilter::Linear,
                mag_filter: TextureFilter::Linear,
                mip_filter: Some(MipFilter::Linear),
                anisotropy: 200,
                ..SamplerDesc::default()
            },
            SamplerDesc { anisotropy: 9, ..SamplerDesc::default() },
        ];
        for desc in inputs {
            let once = desc.normalized().unwrap();
            assert_eq!(once.normalized().unwrap(), once);
        }
    }

    #[test]
    fn default_color_space_is_srgb_and_linear_keeps_the_bytes() {
        let rgba = vec![10, 20, 30, 40];
        let tex = Texture::from_rgba8(1, 1, rgba.clone());
        assert_eq!(tex.color_space, ColorSpace::Srgb);
        let linear = tex.with_color_space(ColorSpace::Linear);
        assert_eq!(linear.color_space, ColorSpace::Linear);
        assert_eq!(linear.rgba, rgba);
    }

    #[test]
    fn mip_filter_and_generation_stay_independent() {
        let generated = Texture::solid(1, 1, [4, 5, 6, 7]).with_mipmaps(Mipmaps::Generate);
        assert_eq!(generated.mips, Mipmaps::Generate);
        assert_eq!(generated.sampler.mip_filter, None);
        assert_eq!(generated.rgba, [4, 5, 6, 7]);

        let sampled = Texture::solid(1, 1, [4, 5, 6, 7])
            .with_sampler(SamplerDesc { mip_filter: Some(MipFilter::Linear), ..SamplerDesc::default() })
            .unwrap();
        assert_eq!(sampled.mips, Mipmaps::None);
        assert_eq!(sampled.sampler.mip_filter, Some(MipFilter::Linear));
        assert_eq!(sampled.rgba.len(), 4);
    }

    #[test]
    fn address_axes_default_to_clamp_and_can_differ() {
        let tex = Texture::solid(1, 1, [0; 4]);
        assert_eq!(tex.sampler.address_u, AddressMode::Clamp);
        assert_eq!(tex.sampler.address_v, AddressMode::Clamp);
        let tex = tex
            .with_sampler(SamplerDesc {
                address_u: AddressMode::Repeat,
                address_v: AddressMode::Mirror,
                ..SamplerDesc::default()
            })
            .unwrap();
        assert_eq!(tex.sampler.address_u, AddressMode::Repeat);
        assert_eq!(tex.sampler.address_v, AddressMode::Mirror);
    }

    #[test]
    fn mip_level_count_follows_the_larger_side() {
        assert_eq!(Mipmaps::None.level_count(1, 1), 1);
        assert_eq!(Mipmaps::None.level_count(256, 128), 1);
        assert_eq!(Mipmaps::Generate.level_count(1, 1), 1);
        assert_eq!(Mipmaps::Generate.level_count(2, 2), 2);
        assert_eq!(Mipmaps::Generate.level_count(256, 256), 9);
        assert_eq!(Mipmaps::Generate.level_count(3, 5), 3);
        assert_eq!(Mipmaps::Generate.level_count(640, 480), 10);
        assert_eq!(Mipmaps::Generate.level_count(8, 3), 4);
        assert_eq!(Mipmaps::Generate.level_count(1, 16), 5);
        assert_eq!(Mipmaps::Generate.level_count(16, 1), 5);
        assert_eq!(Mipmaps::Generate.level_count(1, 7), 3);
        assert_eq!(Mipmaps::Generate.level_count(7, 1), 3);
    }

    #[test]
    fn mipmap_generation_does_not_change_the_sampler() {
        let sampler = SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Nearest,
            mip_filter: None,
            ..SamplerDesc::default()
        };
        let tex = Texture::solid(8, 4, [1, 2, 3, 4]).with_sampler(sampler).unwrap().with_mipmaps(Mipmaps::Generate);
        assert_eq!(tex.mips, Mipmaps::Generate);
        assert_eq!(tex.sampler, sampler);
        assert_eq!(tex.mips.level_count(tex.width, tex.height), 4);
        let sampled = tex.with_sampler(SamplerDesc { mip_filter: Some(MipFilter::Nearest), ..sampler }).unwrap();
        assert_eq!(sampled.mips, Mipmaps::Generate);
        assert_eq!(sampled.sampler.mip_filter, Some(MipFilter::Nearest));
    }
}
