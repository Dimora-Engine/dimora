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

/// One rectangle of texels. Coordinates are texels, with `(0, 0)` at the top-left byte row.
///
/// This is not a world size and not a UV rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// How many region transitions stay on a texture value.
///
/// The newest transitions are kept. A GPU image older than that window uses a full upload. A GPU
/// image that still appears in the window can use a partial upload. Passing the cap does not freeze
/// later edits into a permanent full upload.
pub const MAX_TRACKED_REGIONS: usize = 8;

/// One successful [`Texture::update_region`] and the byte fingerprints on either side of it.
///
/// The rectangle is where the final CPU bytes differ from `from_fingerprint` for this step. An
/// upload copies the current bytes of that rectangle, not a saved copy of the bytes from the step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureEdit {
    /// [`rgba_fingerprint`] of the bytes before this update.
    pub from_fingerprint: u64,
    /// [`rgba_fingerprint`] of the bytes after this update.
    pub to_fingerprint: u64,
    pub rect: TexelRect,
}

/// CPU record of [`Texture::update_region`] calls on this texture value.
///
/// [`TexturePixelEdits::Clean`] is every texture that has not had a successful region update,
/// including a texture inserted as a whole replacement. A sampler, colour-space, or mipmap-preference
/// change does not write this record. Writing [`Texture::rgba`] directly does not either.
///
/// [`TexturePixelEdits::Journal`] is oldest first and holds at most [`MAX_TRACKED_REGIONS`]
/// transitions. Each step's `from_fingerprint` is the previous step's `to_fingerprint` when every
/// edit went through [`Texture::update_region`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TexturePixelEdits {
    /// No successful [`Texture::update_region`] on this value.
    Clean,
    /// Bounded transition history. Oldest first.
    Journal(Vec<TextureEdit>),
}

/// What a later GPU upload should do with the base level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseLevelUpload<'a> {
    /// `current_fingerprint` matches the GPU image. Do not upload pixels and do not rebuild mips.
    Unchanged,
    /// Upload these transitions, in order, from the current CPU bytes. Together they are the whole
    /// delta from the GPU image to those bytes.
    Partial(&'a [TextureEdit]),
    /// Upload every base-level texel.
    Full,
}

impl TexturePixelEdits {
    /// Chooses a base-level upload from the GPU image's fingerprint and the current bytes.
    ///
    /// `cached_fingerprint` is `None` when the renderer has no image. A matching current fingerprint
    /// is [`BaseLevelUpload::Unchanged`]: the pixels did not change, or this journal was already
    /// applied. Otherwise the newest contiguous chain whose first `from_fingerprint` is the GPU
    /// image and whose last `to_fingerprint` is `current_fingerprint` is [`BaseLevelUpload::Partial`].
    /// A broken chain, a GPU image older than the retained window, a [`TexturePixelEdits::Clean`]
    /// value whose bytes differ, and a direct [`Texture::rgba`] write are [`BaseLevelUpload::Full`].
    pub fn base_upload(&self, cached_fingerprint: Option<u64>, current_fingerprint: u64) -> BaseLevelUpload<'_> {
        let Some(cached) = cached_fingerprint else {
            return BaseLevelUpload::Full;
        };
        if current_fingerprint == cached {
            return BaseLevelUpload::Unchanged;
        }
        let TexturePixelEdits::Journal(edits) = self else {
            return BaseLevelUpload::Full;
        };
        let mut chosen = None;
        for start in 0..edits.len() {
            if edits[start].from_fingerprint == cached && chain_reaches(&edits[start..], current_fingerprint) {
                chosen = Some(start);
            }
        }
        match chosen {
            Some(start) => BaseLevelUpload::Partial(&edits[start..]),
            None => BaseLevelUpload::Full,
        }
    }
}

fn chain_reaches(edits: &[TextureEdit], current_fingerprint: u64) -> bool {
    let Some(last) = edits.last() else {
        return false;
    };
    last.to_fingerprint == current_fingerprint
        && edits.windows(2).all(|pair| pair[0].to_fingerprint == pair[1].from_fingerprint)
}

/// Why [`Texture::update_region`] refused to write.
///
/// The texture name is not stored here. The caller that has an [`crate::AssetManager`] can attach
/// [`crate::AssetManager::name_of`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureRegionError {
    /// `width` was 0.
    ZeroWidth,
    /// `height` was 0.
    ZeroHeight,
    /// `x + width` is past the texture width.
    ExceedsWidth { x: u32, width: u32, texture_width: u32 },
    /// `y + height` is past the texture height.
    ExceedsHeight { y: u32, height: u32, texture_height: u32 },
    /// `x + width` or `y + height` overflowed `u32`.
    CoordinateOverflow { x: u32, y: u32, width: u32, height: u32 },
    /// `width * height * 4` overflowed.
    ByteLengthOverflow { width: u32, height: u32 },
    /// `rgba` has fewer bytes than the rectangle.
    InsufficientData { expected: usize, actual: usize },
    /// `rgba` has more bytes than the rectangle.
    ExcessData { expected: usize, actual: usize },
    /// The texture buffer cannot hold the rectangle its dimensions claim.
    BufferLength { expected_end: usize, actual: usize },
}

impl std::fmt::Display for TextureRegionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureRegionError::ZeroWidth => write!(f, "texture region width must be at least 1"),
            TextureRegionError::ZeroHeight => write!(f, "texture region height must be at least 1"),
            TextureRegionError::ExceedsWidth { x, width, texture_width } => {
                write!(f, "texture region x={x} width={width} exceeds texture width {texture_width}")
            }
            TextureRegionError::ExceedsHeight { y, height, texture_height } => {
                write!(f, "texture region y={y} height={height} exceeds texture height {texture_height}")
            }
            TextureRegionError::CoordinateOverflow { x, y, width, height } => {
                write!(f, "texture region x={x} y={y} width={width} height={height} overflows")
            }
            TextureRegionError::ByteLengthOverflow { width, height } => {
                write!(f, "texture region {width}x{height} byte length overflows")
            }
            TextureRegionError::InsufficientData { expected, actual } => {
                write!(f, "texture region needs {expected} bytes, got {actual}")
            }
            TextureRegionError::ExcessData { expected, actual } => {
                write!(f, "texture region needs {expected} bytes, got {actual}")
            }
            TextureRegionError::BufferLength { expected_end, actual } => {
                write!(f, "texture buffer ends at {actual} bytes and cannot hold a write through {expected_end}")
            }
        }
    }
}

impl std::error::Error for TextureRegionError {}

/// Maximum width and height a texture may have. The numbers come from the caller.
///
/// M5-3B reads `max_texture_dimension_2d` from the device and passes that limit for both axes.
/// This function does not open a device. The default wgpu 30.0.1 limit is 8192 per axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureDimensionLimits {
    pub max_width: u32,
    pub max_height: u32,
}

/// [`validate_texture_dimensions`] rejected a size. The asset name is not part of this error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureDimensionError {
    /// `width` is greater than [`TextureDimensionLimits::max_width`].
    Width { width: u32, limit: u32 },
    /// `height` is greater than [`TextureDimensionLimits::max_height`].
    Height { height: u32, limit: u32 },
}

impl std::fmt::Display for TextureDimensionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureDimensionError::Width { width, limit } => {
                write!(f, "texture width {width} exceeds the device limit {limit}")
            }
            TextureDimensionError::Height { height, limit } => {
                write!(f, "texture height {height} exceeds the device limit {limit}")
            }
        }
    }
}

impl std::error::Error for TextureDimensionError {}

/// Rejects a texture whose dimensions exceed `limits`.
///
/// Check this before creating a GPU texture. A failure does not allocate. When both axes exceed
/// the limit, the width error is returned. The caller attaches the asset name from
/// [`crate::AssetManager::name_of`] when that returns `Some`. [`crate::AssetHandle`] does not store
/// the name, and this function does not invent one.
pub fn validate_texture_dimensions(
    width: u32,
    height: u32,
    limits: TextureDimensionLimits,
) -> Result<(), TextureDimensionError> {
    if width > limits.max_width {
        return Err(TextureDimensionError::Width { width, limit: limits.max_width });
    }
    if height > limits.max_height {
        return Err(TextureDimensionError::Height { height, limit: limits.max_height });
    }
    Ok(())
}

/// FNV-1a of tightly packed RGBA8 bytes, mixed with the length.
///
/// The GPU cache uses this same function. A region record is comparable with a cached GPU image
/// only when both sides hash with it.
pub fn rgba_fingerprint(rgba: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in rgba {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^ (rgba.len() as u64)
}

/// CPU-side image, tightly packed RGBA8.
///
/// [`Texture::sampler`] is the sampling configuration. The GPU reads it after
/// [`SamplerDesc::normalized`], including when the public fields were changed without
/// [`Texture::with_sampler`]. [`Texture::color_space`] selects the GPU format. The RGBA bytes are
/// stored as given. [`Texture::mips`] selects how many levels the GPU backend allocates.
/// [`Texture::update_region`] replaces one rectangle of those bytes and records the rectangle.
/// Assigning [`Texture::rgba`] still compiles and does not update that record.
///
/// [`Texture::filter`] is a legacy projection of min and mag, not a second configuration. It is
/// private so a caller cannot publish a filter the backend will ignore.
#[derive(Clone, Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Region updates since this value was constructed. Not a GPU upload.
    edits: TexturePixelEdits,
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
            edits: TexturePixelEdits::Clean,
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

    /// Region updates recorded on this value. [`TexturePixelEdits::Clean`] means there are none.
    pub fn pixel_edits(&self) -> &TexturePixelEdits {
        &self.edits
    }

    /// `true` when a successful region update has changed pixels that a generated mip chain may still reflect.
    ///
    /// This does not generate mipmaps. A direct write to [`Texture::rgba`] does not set it. The GPU
    /// still has to compare fingerprints before it trusts the record.
    pub fn generated_mips_may_be_stale(&self) -> bool {
        !matches!(self.edits, TexturePixelEdits::Clean)
    }

    /// Replaces the rectangle `(x, y, width, height)` with tightly packed RGBA8 bytes.
    ///
    /// Coordinates are texels. `(0, 0)` is the first byte of the first row. `rgba` is the rectangle
    /// only: each source row is `width * 4` bytes, and there are `height` rows. Destination rows
    /// stay `self.width * 4` bytes apart. Dimensions, [`Texture::color_space`], [`Texture::sampler`],
    /// [`Texture::mips`], and the asset id are unchanged. Bytes are not converted between sRGB and linear.
    ///
    /// Validation runs before any write. [`Err`] leaves pixels and [`Texture::pixel_edits`] as they were.
    /// A successful call appends one transition. More than [`MAX_TRACKED_REGIONS`] transitions drops
    /// the oldest. The newest transitions stay usable for a later partial upload.
    pub fn update_region(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(), TextureRegionError> {
        if width == 0 {
            return Err(TextureRegionError::ZeroWidth);
        }
        if height == 0 {
            return Err(TextureRegionError::ZeroHeight);
        }
        let Some(x_end) = x.checked_add(width) else {
            return Err(TextureRegionError::CoordinateOverflow { x, y, width, height });
        };
        let Some(y_end) = y.checked_add(height) else {
            return Err(TextureRegionError::CoordinateOverflow { x, y, width, height });
        };
        if x_end > self.width {
            return Err(TextureRegionError::ExceedsWidth { x, width, texture_width: self.width });
        }
        if y_end > self.height {
            return Err(TextureRegionError::ExceedsHeight { y, height, texture_height: self.height });
        }
        let expected = region_byte_len(width, height)?;
        if rgba.len() < expected {
            return Err(TextureRegionError::InsufficientData { expected, actual: rgba.len() });
        }
        if rgba.len() > expected {
            return Err(TextureRegionError::ExcessData { expected, actual: rgba.len() });
        }
        let dest_end = destination_end(self.width, x, y, width, height)?;
        if dest_end > self.rgba.len() {
            return Err(TextureRegionError::BufferLength { expected_end: dest_end, actual: self.rgba.len() });
        }

        let before = rgba_fingerprint(&self.rgba);
        let row_bytes = (width as usize) * 4;
        let dest_stride = (self.width as usize) * 4;
        for row in 0..height as usize {
            let src = row * row_bytes;
            let dst = (y as usize + row) * dest_stride + (x as usize) * 4;
            self.rgba[dst..dst + row_bytes].copy_from_slice(&rgba[src..src + row_bytes]);
        }
        self.note_region(TexelRect { x, y, width, height }, before);
        Ok(())
    }

    fn note_region(&mut self, rect: TexelRect, before: u64) {
        let edit = TextureEdit { from_fingerprint: before, to_fingerprint: rgba_fingerprint(&self.rgba), rect };
        match &mut self.edits {
            TexturePixelEdits::Clean => self.edits = TexturePixelEdits::Journal(vec![edit]),
            TexturePixelEdits::Journal(edits) => {
                edits.push(edit);
                let overflow = edits.len().saturating_sub(MAX_TRACKED_REGIONS);
                if overflow > 0 {
                    edits.drain(0..overflow);
                }
            }
        }
    }
}

fn region_byte_len(width: u32, height: u32) -> Result<usize, TextureRegionError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(TextureRegionError::ByteLengthOverflow { width, height })?;
    let bytes = pixels.checked_mul(4).ok_or(TextureRegionError::ByteLengthOverflow { width, height })?;
    usize::try_from(bytes).map_err(|_| TextureRegionError::ByteLengthOverflow { width, height })
}

fn destination_end(texture_width: u32, x: u32, y: u32, width: u32, height: u32) -> Result<usize, TextureRegionError> {
    let stride = u64::from(texture_width) * 4;
    let start = u64::from(y) * stride + u64::from(x) * 4;
    let last = start
        .checked_add(u64::from(height - 1) * stride)
        .and_then(|row| row.checked_add(u64::from(width) * 4))
        .ok_or(TextureRegionError::ByteLengthOverflow { width, height })?;
    usize::try_from(last).map_err(|_| TextureRegionError::ByteLengthOverflow { width, height })
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

    fn paint(width: u32, height: u32) -> Texture {
        Texture::from_fn(width, height, |x, y| [x as u8, y as u8, 7, 255])
    }

    fn rect(x: u32, y: u32, width: u32, height: u32) -> TexelRect {
        TexelRect { x, y, width, height }
    }

    fn pixel(tex: &Texture, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * tex.width + x) * 4) as usize;
        tex.rgba[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn a_one_pixel_update_writes_only_that_texel() {
        let mut tex = paint(3, 2);
        let before = tex.rgba.clone();
        tex.update_region(1, 0, 1, 1, &[9, 8, 7, 6]).unwrap();
        assert_eq!(pixel(&tex, 1, 0), [9, 8, 7, 6]);
        assert_eq!(pixel(&tex, 0, 0), [0, 0, 7, 255]);
        assert_eq!(pixel(&tex, 2, 0), [2, 0, 7, 255]);
        assert_eq!(pixel(&tex, 1, 1), [1, 1, 7, 255]);
        assert_ne!(tex.rgba, before);
        assert!(tex.generated_mips_may_be_stale());
    }

    #[test]
    fn an_interior_region_uses_source_rows_and_keeps_the_rest() {
        let mut tex = paint(4, 3);
        let source = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        tex.update_region(1, 1, 2, 2, &source).unwrap();
        assert_eq!(pixel(&tex, 1, 1), [1, 2, 3, 4]);
        assert_eq!(pixel(&tex, 2, 1), [5, 6, 7, 8]);
        assert_eq!(pixel(&tex, 1, 2), [9, 10, 11, 12]);
        assert_eq!(pixel(&tex, 2, 2), [13, 14, 15, 16]);
        assert_eq!(pixel(&tex, 0, 1), [0, 1, 7, 255]);
        assert_eq!(pixel(&tex, 3, 2), [3, 2, 7, 255]);
        assert_eq!(pixel(&tex, 0, 0), [0, 0, 7, 255]);
    }

    #[test]
    fn regions_can_touch_each_edge_and_a_non_square_texture() {
        let mut tex = paint(5, 3);
        tex.update_region(0, 0, 1, 1, &[1, 0, 0, 1]).unwrap();
        tex.update_region(4, 0, 1, 1, &[2, 0, 0, 1]).unwrap();
        tex.update_region(0, 2, 1, 1, &[3, 0, 0, 1]).unwrap();
        tex.update_region(4, 2, 1, 1, &[4, 0, 0, 1]).unwrap();
        assert_eq!(pixel(&tex, 0, 0), [1, 0, 0, 1]);
        assert_eq!(pixel(&tex, 4, 0), [2, 0, 0, 1]);
        assert_eq!(pixel(&tex, 0, 2), [3, 0, 0, 1]);
        assert_eq!(pixel(&tex, 4, 2), [4, 0, 0, 1]);
        assert_eq!(pixel(&tex, 2, 1), [2, 1, 7, 255]);
        assert_eq!(tex.width, 5);
        assert_eq!(tex.height, 3);
    }

    #[test]
    fn a_full_width_row_and_a_single_column_land_on_the_destination_stride() {
        let mut tex = paint(4, 3);
        tex.update_region(0, 1, 4, 1, &[1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4]).unwrap();
        assert_eq!(pixel(&tex, 0, 1), [1, 1, 1, 1]);
        assert_eq!(pixel(&tex, 3, 1), [4, 4, 4, 4]);
        assert_eq!(pixel(&tex, 0, 0), [0, 0, 7, 255]);
        assert_eq!(pixel(&tex, 0, 2), [0, 2, 7, 255]);

        tex.update_region(2, 0, 1, 3, &[8, 0, 0, 9, 8, 1, 0, 9, 8, 2, 0, 9]).unwrap();
        assert_eq!(pixel(&tex, 2, 0), [8, 0, 0, 9]);
        assert_eq!(pixel(&tex, 2, 1), [8, 1, 0, 9]);
        assert_eq!(pixel(&tex, 2, 2), [8, 2, 0, 9]);
        assert_eq!(pixel(&tex, 1, 1), [2, 2, 2, 2]);
        assert_eq!(pixel(&tex, 3, 2), [3, 2, 7, 255]);
    }

    #[test]
    fn a_region_update_does_not_change_sampling_or_identity_fields() {
        let sampler = SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Nearest,
            mip_filter: Some(MipFilter::Nearest),
            address_u: AddressMode::Repeat,
            ..SamplerDesc::default()
        };
        let mut tex = paint(2, 2)
            .with_color_space(ColorSpace::Linear)
            .with_mipmaps(Mipmaps::Generate)
            .with_sampler(sampler)
            .unwrap();
        tex.update_region(0, 0, 2, 1, &[4, 5, 6, 7, 8, 9, 10, 11]).unwrap();
        assert_eq!(tex.width, 2);
        assert_eq!(tex.height, 2);
        assert_eq!(tex.color_space, ColorSpace::Linear);
        assert_eq!(tex.mips, Mipmaps::Generate);
        assert_eq!(tex.sampler, sampler);
        assert_eq!(tex.filter(), TextureFilter::Nearest);
        assert_eq!(pixel(&tex, 0, 0), [4, 5, 6, 7]);
        assert_eq!(pixel(&tex, 1, 1), [1, 1, 7, 255]);
    }

    #[test]
    fn rejected_regions_leave_pixels_and_edits_untouched() {
        let mut tex = paint(2, 2).with_color_space(ColorSpace::Linear);
        let rgba = tex.rgba.clone();
        let cases = [
            tex.update_region(0, 0, 0, 1, &[]),
            tex.update_region(0, 0, 1, 0, &[]),
            tex.update_region(2, 0, 1, 1, &[1, 2, 3, 4]),
            tex.update_region(0, 1, 1, 2, &[1, 2, 3, 4, 5, 6, 7, 8]),
            tex.update_region(u32::MAX, 0, 1, 1, &[1, 2, 3, 4]),
            tex.update_region(0, 0, 1, 1, &[1, 2, 3]),
            tex.update_region(0, 0, 1, 1, &[1, 2, 3, 4, 5]),
        ];
        assert_eq!(cases[0], Err(TextureRegionError::ZeroWidth));
        assert_eq!(cases[1], Err(TextureRegionError::ZeroHeight));
        assert_eq!(cases[2], Err(TextureRegionError::ExceedsWidth { x: 2, width: 1, texture_width: 2 }));
        assert_eq!(cases[3], Err(TextureRegionError::ExceedsHeight { y: 1, height: 2, texture_height: 2 }));
        assert_eq!(cases[4], Err(TextureRegionError::CoordinateOverflow { x: u32::MAX, y: 0, width: 1, height: 1 }));
        assert_eq!(cases[5], Err(TextureRegionError::InsufficientData { expected: 4, actual: 3 }));
        assert_eq!(cases[6], Err(TextureRegionError::ExcessData { expected: 4, actual: 5 }));
        assert_eq!(tex.rgba, rgba);
        assert_eq!(tex.pixel_edits(), &TexturePixelEdits::Clean);
        assert!(!tex.generated_mips_may_be_stale());
        assert_eq!(tex.color_space, ColorSpace::Linear);
    }

    #[test]
    fn byte_length_overflow_and_a_short_buffer_do_not_write() {
        let mut tex = Texture::solid(1, 1, [1, 2, 3, 4]);
        tex.width = u32::MAX;
        tex.height = u32::MAX;
        let err = tex.update_region(0, 0, u32::MAX, u32::MAX, &[]).unwrap_err();
        assert_eq!(err, TextureRegionError::ByteLengthOverflow { width: u32::MAX, height: u32::MAX });
        assert_eq!(tex.rgba, [1, 2, 3, 4]);
        assert_eq!(tex.pixel_edits(), &TexturePixelEdits::Clean);

        let mut tex = paint(2, 2);
        tex.rgba.truncate(4);
        let err = tex.update_region(0, 1, 2, 1, &[9, 9, 9, 9, 8, 8, 8, 8]).unwrap_err();
        assert_eq!(err, TextureRegionError::BufferLength { expected_end: 16, actual: 4 });
        assert_eq!(tex.rgba, [0, 0, 7, 255]);
        assert_eq!(tex.pixel_edits(), &TexturePixelEdits::Clean);
    }

    #[test]
    fn y_overflow_is_rejected_before_a_write() {
        let mut tex = paint(1, 1);
        let err = tex.update_region(0, u32::MAX, 1, 1, &[1, 2, 3, 4]).unwrap_err();
        assert_eq!(err, TextureRegionError::CoordinateOverflow { x: 0, y: u32::MAX, width: 1, height: 1 });
        assert_eq!(tex.pixel_edits(), &TexturePixelEdits::Clean);
    }

    fn partial_rects<'a>(upload: BaseLevelUpload<'a>) -> Vec<TexelRect> {
        match upload {
            BaseLevelUpload::Partial(edits) => edits.iter().map(|edit| edit.rect).collect(),
            other => panic!("expected a partial upload, got {other:?}"),
        }
    }

    #[test]
    fn one_update_records_the_fingerprints_around_that_rectangle() {
        let mut tex = paint(2, 2);
        let base = rgba_fingerprint(&tex.rgba);
        tex.update_region(1, 1, 1, 1, &[9, 9, 9, 9]).unwrap();
        let current = rgba_fingerprint(&tex.rgba);
        match tex.pixel_edits() {
            TexturePixelEdits::Journal(edits) => {
                assert_eq!(edits.len(), 1);
                assert_eq!(edits[0].from_fingerprint, base);
                assert_eq!(edits[0].to_fingerprint, current);
                assert_eq!(edits[0].rect, rect(1, 1, 1, 1));
                assert_ne!(base, current);
            }
            other => panic!("expected one transition, got {other:?}"),
        }
        assert_eq!(partial_rects(tex.pixel_edits().base_upload(Some(base), current)), vec![rect(1, 1, 1, 1)]);
    }

    #[test]
    fn overlapping_disjoint_and_repeated_regions_stay_ordered() {
        let mut tex = paint(4, 4);
        let base = rgba_fingerprint(&tex.rgba);
        tex.update_region(0, 0, 2, 2, &[1; 16]).unwrap();
        tex.update_region(1, 1, 2, 2, &[2; 16]).unwrap();
        tex.update_region(3, 3, 1, 1, &[3, 3, 3, 3]).unwrap();
        tex.update_region(0, 0, 2, 2, &[4; 16]).unwrap();
        let current = rgba_fingerprint(&tex.rgba);
        assert_eq!(
            partial_rects(tex.pixel_edits().base_upload(Some(base), current)),
            vec![rect(0, 0, 2, 2), rect(1, 1, 2, 2), rect(3, 3, 1, 1), rect(0, 0, 2, 2)]
        );
        assert_eq!(pixel(&tex, 0, 0), [4, 4, 4, 4]);
        assert_eq!(pixel(&tex, 1, 1), [4, 4, 4, 4]);
        assert_eq!(pixel(&tex, 2, 2), [2, 2, 2, 2]);
        assert_eq!(pixel(&tex, 3, 3), [3, 3, 3, 3]);
        assert_eq!(pixel(&tex, 3, 0), [3, 0, 7, 255]);
    }

    #[test]
    fn several_regions_before_consumption_all_remain() {
        let mut tex = paint(6, 2);
        let base = rgba_fingerprint(&tex.rgba);
        for i in 0..4 {
            tex.update_region(i, 0, 1, 1, &[i as u8, 0, 0, 1]).unwrap();
        }
        let current = rgba_fingerprint(&tex.rgba);
        let upload = tex.pixel_edits().base_upload(Some(base), current);
        assert_eq!(partial_rects(upload), vec![rect(0, 0, 1, 1), rect(1, 0, 1, 1), rect(2, 0, 1, 1), rect(3, 0, 1, 1)]);
    }

    #[test]
    fn history_past_eight_keeps_recent_partial_uploads() {
        let mut tex = paint(4, 4);
        let mut fps = vec![rgba_fingerprint(&tex.rgba)];
        for i in 0..MAX_TRACKED_REGIONS {
            let x = (i % 4) as u32;
            let y = (i / 4) as u32;
            tex.update_region(x, y, 1, 1, &[i as u8, 1, 1, 1]).unwrap();
            fps.push(rgba_fingerprint(&tex.rgba));
        }
        let at_cap = rgba_fingerprint(&tex.rgba);
        assert_eq!(partial_rects(tex.pixel_edits().base_upload(Some(fps[0]), at_cap)).len(), MAX_TRACKED_REGIONS);
        tex.update_region(0, 0, 1, 1, &[4, 1, 1, 1]).unwrap();
        tex.update_region(3, 3, 1, 1, &[9, 9, 9, 9]).unwrap();
        let current = rgba_fingerprint(&tex.rgba);
        assert_eq!(tex.pixel_edits().base_upload(Some(fps[0]), current), BaseLevelUpload::Full);
        let recent = partial_rects(tex.pixel_edits().base_upload(Some(fps[2]), current));
        assert_eq!(recent.len(), MAX_TRACKED_REGIONS);
        assert_eq!(recent.last().copied(), Some(rect(3, 3, 1, 1)));
        assert_eq!(pixel(&tex, 3, 3), [9, 9, 9, 9]);
        assert_eq!(pixel(&tex, 0, 0), [4, 1, 1, 1]);
        tex.update_region(1, 1, 1, 1, &[6, 6, 6, 6]).unwrap();
        let later = rgba_fingerprint(&tex.rgba);
        assert!(matches!(tex.pixel_edits().base_upload(Some(current), later), BaseLevelUpload::Partial(_)));
        assert_eq!(tex.pixel_edits().base_upload(Some(fps[0]), later), BaseLevelUpload::Full);
    }

    #[test]
    fn constructors_and_sampler_changes_leave_no_dirty_region() {
        let tex = paint(2, 2)
            .with_filter(TextureFilter::Linear)
            .with_color_space(ColorSpace::Linear)
            .with_mipmaps(Mipmaps::Generate);
        assert_eq!(tex.pixel_edits(), &TexturePixelEdits::Clean);
        assert!(!tex.generated_mips_may_be_stale());
        let mut edited = tex.clone();
        edited.update_region(0, 0, 1, 1, &[1, 2, 3, 4]).unwrap();
        let sampled = edited
            .clone()
            .with_sampler(SamplerDesc { address_u: AddressMode::Mirror, ..SamplerDesc::default() })
            .unwrap();
        assert_eq!(sampled.sampler.address_u, AddressMode::Mirror);
        assert_eq!(sampled.rgba, edited.rgba);
        assert_eq!(sampled.pixel_edits(), edited.pixel_edits());
        assert_eq!(sampled.color_space, ColorSpace::Linear);
        assert_eq!(sampled.mips, Mipmaps::Generate);
    }

    #[test]
    fn upload_choice_follows_the_gpu_fingerprint() {
        let mut tex = paint(2, 1);
        let original = rgba_fingerprint(&tex.rgba);
        assert_eq!(tex.pixel_edits().base_upload(None, original), BaseLevelUpload::Full);
        assert_eq!(tex.pixel_edits().base_upload(Some(original), original), BaseLevelUpload::Unchanged);
        tex.update_region(1, 0, 1, 1, &[5, 5, 5, 5]).unwrap();
        let current = rgba_fingerprint(&tex.rgba);
        assert!(matches!(tex.pixel_edits().base_upload(Some(original), current), BaseLevelUpload::Partial(_)));
        assert_eq!(tex.pixel_edits().base_upload(Some(current), current), BaseLevelUpload::Unchanged);
        assert_eq!(tex.pixel_edits().base_upload(Some(1), current), BaseLevelUpload::Full);
        tex.rgba[0] = 99;
        let torn = rgba_fingerprint(&tex.rgba);
        assert_eq!(tex.pixel_edits().base_upload(Some(original), torn), BaseLevelUpload::Full);
    }

    #[test]
    fn replacing_the_asset_publishes_the_region_and_a_new_revision() {
        let mut assets = crate::AssetManager::in_memory();
        let handle = assets.insert("paint", paint(2, 2)).unwrap();
        let first = assets.revision(handle);
        let mut tex = assets.get(handle).unwrap().clone();
        tex.update_region(0, 1, 2, 1, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assets.insert("paint", tex).unwrap();
        let second = assets.revision(handle);
        assert_ne!(second, first);
        let stored = assets.get(handle).unwrap();
        assert_eq!(pixel(stored, 0, 1), [1, 2, 3, 4]);
        assert_eq!(pixel(stored, 1, 1), [5, 6, 7, 8]);
        assert!(matches!(
            stored.pixel_edits(),
            TexturePixelEdits::Journal(edits) if edits.len() == 1 && edits[0].rect == rect(0, 1, 2, 1)
        ));
        let sampler_only = stored.clone().with_filter(TextureFilter::Linear);
        assets.insert("paint", sampler_only).unwrap();
        assert_ne!(assets.revision(handle), second);
        let stored = assets.get(handle).unwrap();
        assert_eq!(stored.filter(), TextureFilter::Linear);
        assert!(matches!(stored.pixel_edits(), TexturePixelEdits::Journal(_)));
        let replaced = Texture::solid(2, 2, [8, 8, 8, 8]);
        assets.insert("paint", replaced).unwrap();
        assert_eq!(assets.get(handle).unwrap().pixel_edits(), &TexturePixelEdits::Clean);
        assert_eq!(assets.get(handle).unwrap().rgba, vec![8; 16]);
    }

    #[test]
    fn a_second_rendered_frame_uploads_only_the_new_region() {
        let mut tex = paint(4, 2);
        let fingerprint_a = rgba_fingerprint(&tex.rgba);
        tex.update_region(0, 0, 1, 1, &[1, 0, 0, 1]).unwrap();
        let fingerprint_b = rgba_fingerprint(&tex.rgba);
        assert_eq!(tex.pixel_edits().base_upload(Some(fingerprint_b), fingerprint_b), BaseLevelUpload::Unchanged);
        tex.update_region(1, 0, 1, 1, &[2, 0, 0, 1]).unwrap();
        let fingerprint_c = rgba_fingerprint(&tex.rgba);
        assert_eq!(
            partial_rects(tex.pixel_edits().base_upload(Some(fingerprint_b), fingerprint_c)),
            vec![rect(1, 0, 1, 1)]
        );
        assert_eq!(
            partial_rects(tex.pixel_edits().base_upload(Some(fingerprint_a), fingerprint_c)),
            vec![rect(0, 0, 1, 1), rect(1, 0, 1, 1)]
        );
    }

    #[test]
    fn three_rendered_frames_keep_a_partial_chain() {
        let mut tex = paint(4, 2);
        let mut gpu = rgba_fingerprint(&tex.rgba);
        let mut frame_before_last = None;
        for (i, x) in (0..3).enumerate() {
            tex.update_region(x, 0, 1, 1, &[i as u8 + 1, 0, 0, 1]).unwrap();
            let current = rgba_fingerprint(&tex.rgba);
            assert_eq!(partial_rects(tex.pixel_edits().base_upload(Some(gpu), current)), vec![rect(x, 0, 1, 1)]);
            if let Some(skipped) = frame_before_last {
                assert_eq!(partial_rects(tex.pixel_edits().base_upload(Some(skipped), current)).len(), 2);
            }
            frame_before_last = Some(gpu);
            gpu = current;
            assert_eq!(tex.pixel_edits().base_upload(Some(gpu), current), BaseLevelUpload::Unchanged);
        }
    }

    #[test]
    fn two_updates_before_one_render_stay_one_partial_upload() {
        let mut tex = paint(3, 2);
        let gpu = rgba_fingerprint(&tex.rgba);
        tex.update_region(0, 0, 1, 1, &[1, 2, 3, 4]).unwrap();
        let middle = rgba_fingerprint(&tex.rgba);
        tex.update_region(2, 1, 1, 1, &[5, 6, 7, 8]).unwrap();
        let current = rgba_fingerprint(&tex.rgba);
        assert_eq!(
            partial_rects(tex.pixel_edits().base_upload(Some(gpu), current)),
            vec![rect(0, 0, 1, 1), rect(2, 1, 1, 1)]
        );
        assert_eq!(partial_rects(tex.pixel_edits().base_upload(Some(middle), current)), vec![rect(2, 1, 1, 1)]);
    }

    #[test]
    fn repeating_one_rectangle_after_an_upload_stays_partial() {
        let mut tex = paint(2, 2);
        tex.update_region(0, 0, 1, 1, &[1, 1, 1, 1]).unwrap();
        let gpu = rgba_fingerprint(&tex.rgba);
        tex.update_region(0, 0, 1, 1, &[2, 2, 2, 2]).unwrap();
        let current = rgba_fingerprint(&tex.rgba);
        assert_eq!(partial_rects(tex.pixel_edits().base_upload(Some(gpu), current)), vec![rect(0, 0, 1, 1)]);
        assert_eq!(pixel(&tex, 0, 0), [2, 2, 2, 2]);
        assert_eq!(tex.pixel_edits().base_upload(Some(current), current), BaseLevelUpload::Unchanged);
    }

    #[test]
    fn a_replaced_texture_and_a_direct_byte_write_upload_in_full() {
        let mut tex = paint(2, 1);
        let original = rgba_fingerprint(&tex.rgba);
        tex.update_region(1, 0, 1, 1, &[1, 2, 3, 4]).unwrap();
        let edited = rgba_fingerprint(&tex.rgba);
        let replaced = Texture::solid(2, 1, [8, 8, 8, 8]);
        assert_eq!(replaced.pixel_edits(), &TexturePixelEdits::Clean);
        assert_eq!(
            replaced.pixel_edits().base_upload(Some(edited), rgba_fingerprint(&replaced.rgba)),
            BaseLevelUpload::Full
        );
        tex.rgba[0] = 99;
        let torn = rgba_fingerprint(&tex.rgba);
        assert_eq!(tex.pixel_edits().base_upload(Some(original), torn), BaseLevelUpload::Full);
        assert_eq!(tex.pixel_edits().base_upload(Some(edited), torn), BaseLevelUpload::Full);
        assert_eq!(tex.pixel_edits().base_upload(Some(torn), torn), BaseLevelUpload::Unchanged);
    }

    #[test]
    fn dimension_limits_reject_either_axis_without_allocating() {
        let limits = TextureDimensionLimits { max_width: 4, max_height: 8 };
        assert_eq!(validate_texture_dimensions(4, 8, limits), Ok(()));
        assert_eq!(validate_texture_dimensions(5, 1, limits), Err(TextureDimensionError::Width { width: 5, limit: 4 }));
        assert_eq!(
            validate_texture_dimensions(4, 9, limits),
            Err(TextureDimensionError::Height { height: 9, limit: 8 })
        );
        assert_eq!(validate_texture_dimensions(5, 9, limits), Err(TextureDimensionError::Width { width: 5, limit: 4 }));
    }
}
