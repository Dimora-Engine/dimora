//! CPU shelf packer for sprite atlases.
//!
//! [`AtlasBuilder`] copies RGBA8 images, places them in [`AtlasBuilder::add`] order, and returns one
//! [`Texture`] plus visible-region metadata. It does not register that texture, hold a world or an
//! [`AssetManager`], create a GPU image, or turn regions into entities.
//!
//! # Packing order
//!
//! Placement follows insertion order. Names and heights are not sorted. [`HashMap`] iteration is not
//! used. Swapping two [`AtlasBuilder::add`] calls can change both positions and atlas bytes. Two
//! builds with the same images, the same order, and the same [`AtlasConfig`] return the same bytes
//! and the same regions.
//!
//! The cursor starts at texel `(0, 0)`, which is the top-left byte row, matching [`Texture`] and
//! [`SpriteAsset`] UV space. Each image reserves an allocated rectangle of
//! `(width + 2 * padding)` by `(height + 2 * padding)`. Rectangles fill the current shelf from left
//! to right. When the next allocated rectangle would pass the atlas width, the shelf advances down
//! by the tallest allocated height on that shelf and the cursor returns to x = 0. Shelf height is
//! that maximum. Allocated rectangles do not overlap. They may touch. Every allocated rectangle lies
//! inside the atlas.
//!
//! The visible origin is the allocated origin plus `padding` on both axes. The visible size is the
//! source size. [`AtlasRegion`] reports that visible rectangle only.
//!
//! # Padding and extrusion
//!
//! `padding` is the gutter reserved on every side of the source, inside the allocated rectangle.
//! `extrusion` is how many of those texels, starting at the source edge and moving outward, are
//! copies of the source edge. All four RGBA channels are copied. Nothing is premultiplied.
//! `extrusion` greater than `padding` is [`AtlasError::ExtrusionExceedsPadding`].
//!
//! Corners copy the corresponding source corner. The remaining outer `padding - extrusion` texels
//! are left transparent black (`[0, 0, 0, 0]`), which is the value the atlas buffer starts as.
//! Extrusion is written only inside its own allocated rectangle, so it cannot land in a neighbour.
//! When allocated rectangles touch, the gap between their visible rectangles is `2 * padding` on
//! that shared side, and at least that much when a taller shelf neighbour increases the gap.
//!
//! Source images passed to [`AtlasBuilder::add`] are copied. Building does not write them back.
//!
//! # Size
//!
//! [`AtlasConfig::width`] and [`AtlasConfig::height`] are the atlas size. The builder does not grow
//! the atlas and does not search for another size. A source that is already larger than the atlas is
//! [`AtlasError::ExceedsAtlas`]. A source that would fit, but not after padding, is
//! [`AtlasError::PaddingOverflow`]. An image that fits on an empty shelf but not in the remaining
//! shelves is [`AtlasError::OutOfSpace`]. A `u32` sum that overflows is [`AtlasError::Overflow`].
//! An atlas whose byte length does not fit in `usize` is [`AtlasError::AtlasByteOverflow`], and no
//! buffer is allocated. [`AtlasBuilder::build`] returns [`Err`] for all of these and does not yield
//! an atlas.
//!
//! # Texture configuration and mips
//!
//! The atlas has one [`ColorSpace`], one [`SamplerDesc`], and one [`Mipmaps`] value, taken from
//! [`AtlasConfig`]. Bytes are not converted between sRGB and linear. The defaults match
//! [`Texture::from_rgba8`]: [`ColorSpace::Srgb`], [`SamplerDesc::default`] (nearest, clamp, no mip
//! filter, anisotropy 1), and [`Mipmaps::None`].
//!
//! [`Mipmaps::None`] is the sampling policy this builder recommends. Padding and extrusion do not
//! keep arbitrary mip levels inside one region. [`Mipmaps::Generate`] is stored when the caller sets
//! it, and the GPU backend may then build a whole-atlas chain on upload. That request does not mean
//! every packed region can be sampled at every level. This crate does not generate mip pixels and
//! does not change renderer LOD.
//!
//! # Sprites and later updates
//!
//! [`Atlas::sprite`] builds a [`SpriteAsset`] with [`SpriteAsset::from_atlas_rect`] from the visible
//! rectangle. `pixels_per_unit` stays [`SpriteAsset::DEFAULT_PIXELS_PER_UNIT`]. Atlas position does
//! not change world size.
//!
//! A caller who has registered [`Atlas::texture`] can pass a region's visible rectangle to
//! [`Texture::update_region`]. That replaces those texels and leaves region metadata where it is.
//! It does not refresh extrusion and it does not move other regions. A source whose dimensions
//! change needs a new [`AtlasBuilder::build`]. Regions from an older layout must not be reused after
//! the layout changes.
//!
//! [`AssetManager`]: crate::AssetManager
//! [`HashMap`]: std::collections::HashMap

use engine_math::{Rect, Vec2};

use crate::{AssetHandle, ColorSpace, Mipmaps, SamplerConfigError, SamplerDesc, SpriteAsset, Texture};

/// Width, height, gutter, and the one texture configuration shared by every region.
///
/// The builder reads this value at [`AtlasBuilder::build`]. It does not resize it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtlasConfig {
    /// Atlas width in texels. Zero is [`AtlasError::ZeroAtlasWidth`].
    pub width: u32,
    /// Atlas height in texels. Zero is [`AtlasError::ZeroAtlasHeight`].
    pub height: u32,
    /// Texels reserved on every side of each source image, inside its allocated rectangle.
    pub padding: u32,
    /// Edge texels inside `padding` that copy the source edge. Must be `<= padding` at build.
    pub extrusion: u32,
    /// How the GPU should interpret the atlas bytes. The builder does not convert them.
    pub color_space: ColorSpace,
    /// Sampler stored on the atlas [`Texture`]. Anisotropy below 1 fails at build.
    pub sampler: SamplerDesc,
    /// Mip request stored on the atlas [`Texture`]. [`Mipmaps::None`] is the default.
    pub mips: Mipmaps,
}

impl AtlasConfig {
    /// An atlas of `width` by `height` with no padding, no extrusion, and [`Texture`] defaults.
    ///
    /// Those defaults are sRGB, nearest, clamp, anisotropy 1, and [`Mipmaps::None`].
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            padding: 0,
            extrusion: 0,
            color_space: ColorSpace::Srgb,
            sampler: SamplerDesc::default(),
            mips: Mipmaps::None,
        }
    }

    /// Sets the gutter reserved around every source image.
    pub fn with_padding(mut self, padding: u32) -> Self {
        self.padding = padding;
        self
    }

    /// Sets how many gutter texels copy the source edge.
    ///
    /// A value above [`AtlasConfig::padding`] is rejected when the atlas is built.
    pub fn with_extrusion(mut self, extrusion: u32) -> Self {
        self.extrusion = extrusion;
        self
    }

    /// Stores the colour space. Source bytes are not converted.
    pub fn with_color_space(mut self, color_space: ColorSpace) -> Self {
        self.color_space = color_space;
        self
    }

    /// Stores `sampler` after [`SamplerDesc::normalized`].
    pub fn with_sampler(mut self, sampler: SamplerDesc) -> Result<Self, SamplerConfigError> {
        self.sampler = sampler.normalized()?;
        Ok(self)
    }

    /// Stores the mipmap request. This does not generate mip pixels.
    pub fn with_mipmaps(mut self, mips: Mipmaps) -> Self {
        self.mips = mips;
        self
    }
}

/// One source image waiting to be packed. Pixels are a copy taken at [`AtlasBuilder::add`].
#[derive(Clone, Debug)]
struct SourceImage {
    name: String,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Where one image was placed. `x` and `y` are the visible origin, not the allocated origin.
struct Placement {
    index: usize,
    x: u32,
    y: u32,
}

/// Collects named RGBA8 images and packs them on demand.
///
/// [`AtlasBuilder::build`] borrows the builder, so a second build repeats the same result until
/// images or [`AtlasConfig`] change.
#[derive(Clone, Debug)]
pub struct AtlasBuilder {
    config: AtlasConfig,
    images: Vec<SourceImage>,
}

impl AtlasBuilder {
    /// An empty builder. [`AtlasConfig`] is read later, at [`Self::build`].
    pub fn new(config: AtlasConfig) -> Self {
        Self { config, images: Vec::new() }
    }

    /// The configuration the next [`Self::build`] will use.
    pub fn config(&self) -> &AtlasConfig {
        &self.config
    }

    /// Replaces packing and texture settings without touching images already added.
    pub fn config_mut(&mut self) -> &mut AtlasConfig {
        &mut self.config
    }

    /// Copies one RGBA8 image under `name`.
    ///
    /// Names are unique and case-sensitive. An empty name, a duplicate, a zero side, or a byte
    /// length other than `width * height * 4` is rejected and not stored. Fit against the atlas is
    /// checked at [`Self::build`], so [`AtlasConfig`] can still change after a successful add.
    pub fn add(&mut self, name: &str, width: u32, height: u32, rgba: &[u8]) -> Result<(), AtlasError> {
        if name.is_empty() {
            return Err(AtlasError::EmptyName);
        }
        if self.images.iter().any(|image| image.name == name) {
            return Err(AtlasError::DuplicateName { name: name.to_owned() });
        }
        if width == 0 {
            return Err(AtlasError::ZeroWidth { name: name.to_owned() });
        }
        if height == 0 {
            return Err(AtlasError::ZeroHeight { name: name.to_owned() });
        }
        let expected = rgba8_byte_len(width, height).ok_or_else(|| AtlasError::Overflow { name: name.to_owned() })?;
        if rgba.len() < expected {
            return Err(AtlasError::InsufficientData { name: name.to_owned(), expected, actual: rgba.len() });
        }
        if rgba.len() > expected {
            return Err(AtlasError::ExcessData { name: name.to_owned(), expected, actual: rgba.len() });
        }
        self.images.push(SourceImage { name: name.to_owned(), width, height, rgba: rgba.to_vec() });
        Ok(())
    }

    /// Packs every added image into a new [`Atlas`].
    ///
    /// On failure no atlas is returned. Images stay on the builder. Checks run in this order:
    /// empty input, a zero atlas side, extrusion wider than padding, then each image in insertion
    /// order (source larger than the atlas, padded extent overflow, padding that no longer fits,
    /// then shelf placement), then the sampler, then the atlas byte length. Pixel writes happen
    /// only after those checks.
    pub fn build(&self) -> Result<Atlas, AtlasError> {
        if self.images.is_empty() {
            return Err(AtlasError::Empty);
        }
        if self.config.width == 0 {
            return Err(AtlasError::ZeroAtlasWidth);
        }
        if self.config.height == 0 {
            return Err(AtlasError::ZeroAtlasHeight);
        }
        if self.config.extrusion > self.config.padding {
            return Err(AtlasError::ExtrusionExceedsPadding {
                extrusion: self.config.extrusion,
                padding: self.config.padding,
            });
        }

        let placements = place_shelves(&self.images, &self.config)?;
        let sampler = self.config.sampler.normalized().map_err(AtlasError::Sampler)?;
        let byte_len = rgba8_byte_len(self.config.width, self.config.height)
            .ok_or(AtlasError::AtlasByteOverflow { width: self.config.width, height: self.config.height })?;
        let mut rgba = vec![0u8; byte_len];
        for placement in &placements {
            paint(&mut rgba, &self.config, &self.images[placement.index], placement)?;
        }

        let texture = Texture::from_rgba8(self.config.width, self.config.height, rgba)
            .with_color_space(self.config.color_space)
            .with_mipmaps(self.config.mips)
            .with_sampler(sampler)
            .map_err(AtlasError::Sampler)?;
        let regions = placements
            .iter()
            .map(|placement| {
                region_for(&self.images[placement.index], placement, self.config.width, self.config.height)
            })
            .collect();
        Ok(Atlas { texture, regions })
    }
}

/// A packed atlas texture and the visible rectangle of each source image.
#[derive(Clone, Debug)]
pub struct Atlas {
    texture: Texture,
    regions: Vec<AtlasRegion>,
}

impl Atlas {
    /// The packed RGBA8 texture. Register a clone with [`crate::AssetManager`] when a handle is needed.
    pub fn texture(&self) -> &Texture {
        &self.texture
    }

    /// Visible regions in insertion order.
    pub fn regions(&self) -> &[AtlasRegion] {
        &self.regions
    }

    /// The region added under `name`, if that add succeeded and the build included it.
    pub fn region(&self, name: &str) -> Option<&AtlasRegion> {
        self.regions.iter().find(|region| region.name == name)
    }

    /// A [`SpriteAsset`] for `name` on `texture`.
    ///
    /// `texture` should be the handle of [`Self::texture`] after registration. The sprite uses the
    /// visible rectangle, [`SpriteAsset::from_atlas_rect`], the default pivot, and
    /// [`SpriteAsset::DEFAULT_PIXELS_PER_UNIT`]. `None` means `name` was not packed.
    pub fn sprite(&self, name: &str, texture: AssetHandle<Texture>) -> Option<SpriteAsset> {
        let region = self.region(name)?;
        let atlas_size = Vec2::new(self.texture.width as f32, self.texture.height as f32);
        Some(SpriteAsset::from_atlas_rect(texture, atlas_size, region.x, region.y, region.width, region.height))
    }
}

/// Visible texels of one packed image.
///
/// `x` and `y` are the top-left texel of the source inside the atlas. `width` and `height` exclude
/// padding and extrusion. `source_width` and `source_height` are the image size passed to
/// [`AtlasBuilder::add`]. UV uses the final atlas size. `(0, 0)` is the top-left texel. There is no
/// half-texel bias and no Y flip.
#[derive(Clone, Debug, PartialEq)]
pub struct AtlasRegion {
    /// The name passed to [`AtlasBuilder::add`].
    pub name: String,
    /// Visible origin, in texels, from the top-left of the atlas.
    pub x: u32,
    /// Visible origin, in texels, from the top-left of the atlas.
    pub y: u32,
    /// Visible width in texels. Equal to [`Self::source_width`].
    pub width: u32,
    /// Visible height in texels. Equal to [`Self::source_height`].
    pub height: u32,
    /// Width of the source image. Padding is not included.
    pub source_width: u32,
    /// Height of the source image. Padding is not included.
    pub source_height: u32,
    /// Normalized UV of the visible rectangle. `(0, 0)` is the top-left texel of the atlas.
    pub uv: Rect,
}

/// Why [`AtlasBuilder::add`] or [`AtlasBuilder::build`] refused a result.
///
/// The packer does not log. A caller that wants a diagnostic uses [`Display`](std::fmt::Display).
/// Image errors include the name that was passed in. They do not invent one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtlasError {
    /// [`AtlasBuilder::build`] was called with no successful [`AtlasBuilder::add`].
    Empty,
    /// [`AtlasBuilder::add`] was given an empty name.
    EmptyName,
    /// `name` was already added.
    DuplicateName { name: String },
    /// The image named `name` has width 0.
    ZeroWidth { name: String },
    /// The image named `name` has height 0.
    ZeroHeight { name: String },
    /// [`AtlasConfig::width`] is 0.
    ZeroAtlasWidth,
    /// [`AtlasConfig::height`] is 0.
    ZeroAtlasHeight,
    /// `rgba` is shorter than `width * height * 4`.
    InsufficientData { name: String, expected: usize, actual: usize },
    /// `rgba` is longer than `width * height * 4`.
    ExcessData { name: String, expected: usize, actual: usize },
    /// [`AtlasConfig::extrusion`] is greater than [`AtlasConfig::padding`].
    ExtrusionExceedsPadding { extrusion: u32, padding: u32 },
    /// A `u32` sum for `name` overflowed. This includes `padding * 2 + side`.
    Overflow { name: String },
    /// The source image itself is wider or taller than the atlas, before padding.
    ExceedsAtlas { name: String, width: u32, height: u32, atlas_width: u32, atlas_height: u32 },
    /// The source fits, and the padded rectangle does not.
    PaddingOverflow {
        name: String,
        width: u32,
        height: u32,
        padded_width: u32,
        padded_height: u32,
        atlas_width: u32,
        atlas_height: u32,
    },
    /// `name` fits on an empty shelf and does not fit in the shelves that remain.
    OutOfSpace { name: String, atlas_width: u32, atlas_height: u32 },
    /// `width * height * 4` does not fit in `usize`. No atlas buffer is allocated.
    AtlasByteOverflow { width: u32, height: u32 },
    /// [`SamplerDesc::normalized`] rejected the atlas sampler.
    Sampler(SamplerConfigError),
}

impl std::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtlasError::Empty => write!(f, "atlas has no images"),
            AtlasError::EmptyName => write!(f, "atlas image name is empty"),
            AtlasError::DuplicateName { name } => write!(f, "atlas image name {name:?} is already used"),
            AtlasError::ZeroWidth { name } => write!(f, "atlas image {name:?} has width 0"),
            AtlasError::ZeroHeight { name } => write!(f, "atlas image {name:?} has height 0"),
            AtlasError::ZeroAtlasWidth => write!(f, "atlas width must be at least 1"),
            AtlasError::ZeroAtlasHeight => write!(f, "atlas height must be at least 1"),
            AtlasError::InsufficientData { name, expected, actual } => {
                write!(f, "atlas image {name:?} needs {expected} bytes, got {actual}")
            }
            AtlasError::ExcessData { name, expected, actual } => {
                write!(f, "atlas image {name:?} needs {expected} bytes, got {actual}")
            }
            AtlasError::ExtrusionExceedsPadding { extrusion, padding } => {
                write!(f, "atlas extrusion {extrusion} is wider than padding {padding}")
            }
            AtlasError::Overflow { name } => write!(f, "atlas layout for {name:?} overflows u32"),
            AtlasError::ExceedsAtlas { name, width, height, atlas_width, atlas_height } => write!(
                f,
                "atlas image {name:?} is {width}x{height}, which is larger than the atlas {atlas_width}x{atlas_height}"
            ),
            AtlasError::PaddingOverflow {
                name,
                width,
                height,
                padded_width,
                padded_height,
                atlas_width,
                atlas_height,
            } => write!(
                f,
                "atlas image {name:?} is {width}x{height} and becomes {padded_width}x{padded_height} with padding, which does not fit in {atlas_width}x{atlas_height}"
            ),
            AtlasError::OutOfSpace { name, atlas_width, atlas_height } => {
                write!(f, "atlas image {name:?} does not fit in the remaining {atlas_width}x{atlas_height} shelves")
            }
            AtlasError::AtlasByteOverflow { width, height } => {
                write!(f, "atlas {width}x{height} byte length overflows")
            }
            AtlasError::Sampler(err) => write!(f, "atlas sampler: {err}"),
        }
    }
}

impl std::error::Error for AtlasError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AtlasError::Sampler(err) => Some(err),
            _ => None,
        }
    }
}

fn rgba8_byte_len(width: u32, height: u32) -> Option<usize> {
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    let bytes = pixels.checked_mul(4)?;
    usize::try_from(bytes).ok()
}

fn padded_side(size: u32, padding: u32) -> Option<u32> {
    padding.checked_mul(2)?.checked_add(size)
}

fn place_shelves(images: &[SourceImage], config: &AtlasConfig) -> Result<Vec<Placement>, AtlasError> {
    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    let mut shelf_height = 0u32;
    let mut placed = Vec::with_capacity(images.len());

    for (index, image) in images.iter().enumerate() {
        if image.width > config.width || image.height > config.height {
            return Err(AtlasError::ExceedsAtlas {
                name: image.name.clone(),
                width: image.width,
                height: image.height,
                atlas_width: config.width,
                atlas_height: config.height,
            });
        }
        let (allocated_width, allocated_height) =
            match (padded_side(image.width, config.padding), padded_side(image.height, config.padding)) {
                (Some(width), Some(height)) => (width, height),
                _ => return Err(AtlasError::Overflow { name: image.name.clone() }),
            };
        if allocated_width > config.width || allocated_height > config.height {
            return Err(AtlasError::PaddingOverflow {
                name: image.name.clone(),
                width: image.width,
                height: image.height,
                padded_width: allocated_width,
                padded_height: allocated_height,
                atlas_width: config.width,
                atlas_height: config.height,
            });
        }

        if cursor_x > 0 && cursor_x.checked_add(allocated_width).is_none_or(|end| end > config.width) {
            cursor_y =
                cursor_y.checked_add(shelf_height).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;
            cursor_x = 0;
            shelf_height = 0;
        }
        let x_end =
            cursor_x.checked_add(allocated_width).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;
        let y_end =
            cursor_y.checked_add(allocated_height).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;
        if x_end > config.width || y_end > config.height {
            return Err(AtlasError::OutOfSpace {
                name: image.name.clone(),
                atlas_width: config.width,
                atlas_height: config.height,
            });
        }

        placed.push(Placement { index, x: cursor_x + config.padding, y: cursor_y + config.padding });
        shelf_height = shelf_height.max(allocated_height);
        cursor_x = x_end;
    }
    Ok(placed)
}

fn region_for(image: &SourceImage, placement: &Placement, atlas_width: u32, atlas_height: u32) -> AtlasRegion {
    let atlas_size = Vec2::new(atlas_width as f32, atlas_height as f32);
    let min = Vec2::new(placement.x as f32, placement.y as f32) / atlas_size;
    let max = Vec2::new((placement.x + image.width) as f32, (placement.y + image.height) as f32) / atlas_size;
    AtlasRegion {
        name: image.name.clone(),
        x: placement.x,
        y: placement.y,
        width: image.width,
        height: image.height,
        source_width: image.width,
        source_height: image.height,
        uv: Rect::new(min, max),
    }
}

fn paint(atlas: &mut [u8], config: &AtlasConfig, image: &SourceImage, placement: &Placement) -> Result<(), AtlasError> {
    let extrusion = config.extrusion;
    let paint_width =
        padded_side(image.width, extrusion).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;
    let paint_height =
        padded_side(image.height, extrusion).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;
    let origin_x =
        placement.x.checked_sub(extrusion).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;
    let origin_y =
        placement.y.checked_sub(extrusion).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;

    for row in 0..paint_height {
        for col in 0..paint_width {
            let sample_x = i64::from(col) - i64::from(extrusion);
            let sample_y = i64::from(row) - i64::from(extrusion);
            let pixel = sample_clamped(image, sample_x, sample_y);
            let dest_x = origin_x.checked_add(col).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;
            let dest_y = origin_y.checked_add(row).ok_or_else(|| AtlasError::Overflow { name: image.name.clone() })?;
            write_texel(atlas, config.width, config.height, dest_x, dest_y, pixel)?;
        }
    }
    Ok(())
}

fn sample_clamped(image: &SourceImage, x: i64, y: i64) -> [u8; 4] {
    let x = x.clamp(0, i64::from(image.width) - 1) as u32;
    let y = y.clamp(0, i64::from(image.height) - 1) as u32;
    let index = (y as usize * image.width as usize + x as usize) * 4;
    let mut pixel = [0; 4];
    pixel.copy_from_slice(&image.rgba[index..index + 4]);
    pixel
}

fn write_texel(
    atlas: &mut [u8],
    atlas_width: u32,
    atlas_height: u32,
    x: u32,
    y: u32,
    pixel: [u8; 4],
) -> Result<(), AtlasError> {
    if x >= atlas_width || y >= atlas_height {
        return Err(AtlasError::AtlasByteOverflow { width: atlas_width, height: atlas_height });
    }
    let stride = usize::try_from(u64::from(atlas_width) * 4)
        .map_err(|_| AtlasError::AtlasByteOverflow { width: atlas_width, height: atlas_height })?;
    let start = (y as usize)
        .checked_mul(stride)
        .and_then(|row| row.checked_add(x as usize * 4))
        .ok_or(AtlasError::AtlasByteOverflow { width: atlas_width, height: atlas_height })?;
    let end = start + 4;
    if end > atlas.len() {
        return Err(AtlasError::AtlasByteOverflow { width: atlas_width, height: atlas_height });
    }
    atlas[start..end].copy_from_slice(&pixel);
    Ok(())
}

#[cfg(test)]
mod tests {
    use engine_math::Vec2;

    use super::*;
    use crate::{AddressMode, AssetManager, MipFilter, SpriteAsset, TextureFilter, TexturePixelEdits};

    fn solid(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
        pixel.repeat((width * height) as usize)
    }

    fn texel(texture: &Texture, x: u32, y: u32) -> [u8; 4] {
        let index = (y as usize * texture.width as usize + x as usize) * 4;
        let mut pixel = [0; 4];
        pixel.copy_from_slice(&texture.rgba[index..index + 4]);
        pixel
    }

    fn pack(config: AtlasConfig, images: &[(&str, u32, u32, [u8; 4])]) -> Atlas {
        let mut builder = AtlasBuilder::new(config);
        for (name, width, height, pixel) in images {
            builder.add(name, *width, *height, &solid(*width, *height, *pixel)).unwrap();
        }
        builder.build().unwrap()
    }

    fn overlaps(a: &AtlasRegion, b: &AtlasRegion) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    #[test]
    fn identical_inputs_produce_identical_placements_and_bytes() {
        let config = AtlasConfig::new(9, 9).with_padding(1).with_extrusion(1);
        let images = [("zeta", 3u32, 2u32, [1, 2, 3, 4]), ("alpha", 2, 3, [5, 6, 7, 8])];
        let first = pack(config.clone(), &images);
        let second = pack(config, &images);
        assert_eq!(first.texture().rgba, second.texture().rgba);
        assert_eq!(first.regions(), second.regions());
    }

    #[test]
    fn repeated_builds_match() {
        let mut builder = AtlasBuilder::new(AtlasConfig::new(6, 8).with_padding(1));
        builder.add("one", 2, 2, &solid(2, 2, [9, 8, 7, 6])).unwrap();
        builder.add("two", 2, 1, &solid(2, 1, [1, 0, 0, 255])).unwrap();
        let first = builder.build().unwrap();
        let second = builder.build().unwrap();
        assert_eq!(first.texture().rgba, second.texture().rgba);
        assert_eq!(first.regions(), second.regions());
    }

    #[test]
    fn packing_follows_insertion_order() {
        let config = AtlasConfig::new(5, 4);
        let forward = pack(
            config.clone(),
            &[("a", 3, 2, [1, 0, 0, 255]), ("b", 3, 2, [0, 1, 0, 255]), ("c", 2, 1, [0, 0, 1, 255])],
        );
        assert_eq!((forward.region("a").unwrap().x, forward.region("a").unwrap().y), (0, 0));
        assert_eq!((forward.region("b").unwrap().x, forward.region("b").unwrap().y), (0, 2));
        assert_eq!((forward.region("c").unwrap().x, forward.region("c").unwrap().y), (3, 2));
        assert_eq!(forward.regions().iter().map(|region| region.name.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);

        let swapped =
            pack(config, &[("c", 2, 1, [0, 0, 1, 255]), ("a", 3, 2, [1, 0, 0, 255]), ("b", 3, 2, [0, 1, 0, 255])]);
        assert_eq!((swapped.region("c").unwrap().x, swapped.region("c").unwrap().y), (0, 0));
        assert_eq!((swapped.region("a").unwrap().x, swapped.region("a").unwrap().y), (2, 0));
        assert_ne!(
            (swapped.region("a").unwrap().x, swapped.region("a").unwrap().y),
            (forward.region("a").unwrap().x, forward.region("a").unwrap().y)
        );

        let by_name = pack(AtlasConfig::new(8, 4), &[("zeta", 2, 1, [1, 0, 0, 255]), ("alpha", 2, 2, [0, 1, 0, 255])]);
        assert_eq!(by_name.regions()[0].name, "zeta");
        assert_eq!((by_name.region("zeta").unwrap().x, by_name.region("zeta").unwrap().y), (0, 0));
        assert_eq!((by_name.region("alpha").unwrap().x, by_name.region("alpha").unwrap().y), (2, 0));
    }

    #[test]
    fn regions_do_not_overlap_and_padding_is_inside_the_atlas() {
        let padding = 1u32;
        let atlas = pack(
            AtlasConfig::new(16, 16).with_padding(padding),
            &[
                ("a", 3, 2, [1, 0, 0, 255]),
                ("b", 2, 3, [0, 1, 0, 255]),
                ("c", 4, 1, [0, 0, 1, 255]),
                ("d", 1, 1, [1, 1, 0, 255]),
                ("e", 2, 2, [1, 0, 1, 255]),
            ],
        );
        let regions = atlas.regions();
        for region in regions {
            assert!(region.x + region.width <= atlas.texture().width);
            assert!(region.y + region.height <= atlas.texture().height);
            assert!(region.x >= padding && region.y >= padding);
            let allocated_x = region.x - padding;
            let allocated_y = region.y - padding;
            let allocated_w = region.width + padding * 2;
            let allocated_h = region.height + padding * 2;
            assert!(allocated_x + allocated_w <= atlas.texture().width);
            assert!(allocated_y + allocated_h <= atlas.texture().height);
        }
        for (i, left) in regions.iter().enumerate() {
            for right in &regions[i + 1..] {
                assert!(!overlaps(left, right), "{} overlaps {}", left.name, right.name);
                let separated_x = left.x + left.width <= right.x || right.x + right.width <= left.x;
                let separated_y = left.y + left.height <= right.y || right.y + right.height <= left.y;
                if separated_x && !separated_y {
                    let gap = if left.x + left.width <= right.x {
                        right.x - (left.x + left.width)
                    } else {
                        left.x - (right.x + right.width)
                    };
                    assert!(gap >= padding * 2, "{} and {} are closer than padding on x", left.name, right.name);
                }
                if separated_y && !separated_x {
                    let gap = if left.y + left.height <= right.y {
                        right.y - (left.y + left.height)
                    } else {
                        left.y - (right.y + right.height)
                    };
                    assert!(gap >= padding * 2, "{} and {} are closer than padding on y", left.name, right.name);
                }
            }
        }
    }

    #[test]
    fn shelf_wraps_when_the_row_is_full() {
        let atlas = pack(
            AtlasConfig::new(5, 4),
            &[("a", 3, 2, [1, 0, 0, 255]), ("b", 3, 2, [0, 1, 0, 255]), ("c", 2, 1, [0, 0, 1, 255])],
        );
        assert_eq!((atlas.region("b").unwrap().x, atlas.region("b").unwrap().y), (0, 2));
        assert_eq!((atlas.region("c").unwrap().x, atlas.region("c").unwrap().y), (3, 2));
    }

    #[test]
    fn non_square_images_and_atlases_pack() {
        let images = pack(AtlasConfig::new(5, 6), &[("wide", 4, 2, [1, 0, 0, 255]), ("tall", 2, 3, [0, 1, 0, 255])]);
        assert_eq!((images.region("wide").unwrap().width, images.region("wide").unwrap().height), (4, 2));
        assert_eq!((images.region("tall").unwrap().x, images.region("tall").unwrap().y), (0, 2));
        assert_eq!((images.region("tall").unwrap().width, images.region("tall").unwrap().height), (2, 3));

        let atlas = pack(
            AtlasConfig::new(8, 3),
            &[
                ("a", 2, 2, [1, 0, 0, 255]),
                ("b", 2, 2, [0, 1, 0, 255]),
                ("c", 2, 2, [0, 0, 1, 255]),
                ("d", 2, 2, [1, 1, 0, 255]),
            ],
        );
        assert_eq!(atlas.texture().width, 8);
        assert_eq!(atlas.texture().height, 3);
        assert_eq!(atlas.regions().iter().map(|region| region.x).collect::<Vec<_>>(), [0, 2, 4, 6]);
        assert!(atlas.regions().iter().all(|region| region.y == 0));
    }

    #[test]
    fn one_image_is_inset_by_padding() {
        let atlas = pack(AtlasConfig::new(4, 3).with_padding(1), &[("only", 2, 1, [4, 5, 6, 7])]);
        let region = atlas.region("only").unwrap();
        assert_eq!((region.x, region.y, region.width, region.height), (1, 1, 2, 1));
        assert_eq!(texel(atlas.texture(), 1, 1), [4, 5, 6, 7]);
        assert_eq!(texel(atlas.texture(), 2, 1), [4, 5, 6, 7]);
        assert_eq!(texel(atlas.texture(), 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn edges_and_corners_extrude_every_channel() {
        let mut rgba = Vec::new();
        for y in 0..2u32 {
            for x in 0..2u32 {
                rgba.extend_from_slice(&[x as u8 + 1, y as u8 + 10, 30, 40 + x as u8 + y as u8]);
            }
        }
        let mut builder = AtlasBuilder::new(AtlasConfig::new(4, 4).with_padding(1).with_extrusion(1));
        builder.add("quad", 2, 2, &rgba).unwrap();
        let atlas = builder.build().unwrap();
        let texture = atlas.texture();
        assert_eq!(texel(texture, 0, 0), texel(texture, 1, 1));
        assert_eq!(texel(texture, 3, 0), texel(texture, 2, 1));
        assert_eq!(texel(texture, 0, 3), texel(texture, 1, 2));
        assert_eq!(texel(texture, 3, 3), texel(texture, 2, 2));
        assert_eq!(texel(texture, 1, 0), texel(texture, 1, 1));
        assert_eq!(texel(texture, 2, 0), texel(texture, 2, 1));
        assert_eq!(texel(texture, 0, 1), texel(texture, 1, 1));
        assert_eq!(texel(texture, 3, 2), texel(texture, 2, 2));
        assert_eq!(texel(texture, 1, 1), [1, 10, 30, 40]);
        assert_eq!(texel(texture, 2, 2), [2, 11, 30, 42]);
    }

    #[test]
    fn transparent_pixels_are_copied_exactly() {
        let atlas = pack(AtlasConfig::new(3, 3).with_padding(1).with_extrusion(1), &[("glass", 1, 1, [10, 20, 30, 0])]);
        assert_eq!(texel(atlas.texture(), 1, 1), [10, 20, 30, 0]);
        assert_eq!(texel(atlas.texture(), 0, 0), [10, 20, 30, 0]);
        assert_eq!(texel(atlas.texture(), 2, 1), [10, 20, 30, 0]);
    }

    #[test]
    fn unfilled_padding_stays_transparent_and_extrusion_does_not_cross() {
        let mut builder = AtlasBuilder::new(AtlasConfig::new(5, 5).with_padding(2).with_extrusion(1));
        builder.add("red", 1, 1, &[255, 0, 0, 255]).unwrap();
        let alone = builder.build().unwrap();
        assert_eq!(texel(alone.texture(), 2, 2), [255, 0, 0, 255]);
        assert_eq!(texel(alone.texture(), 1, 1), [255, 0, 0, 255]);
        assert_eq!(texel(alone.texture(), 0, 0), [0, 0, 0, 0]);
        assert_eq!(texel(alone.texture(), 4, 2), [0, 0, 0, 0]);

        let atlas = pack(
            AtlasConfig::new(6, 3).with_padding(1).with_extrusion(1),
            &[("red", 1, 1, [255, 0, 0, 255]), ("blue", 1, 1, [0, 0, 255, 128])],
        );
        assert_eq!((atlas.region("red").unwrap().x, atlas.region("red").unwrap().y), (1, 1));
        assert_eq!((atlas.region("blue").unwrap().x, atlas.region("blue").unwrap().y), (4, 1));
        assert_eq!(texel(atlas.texture(), 2, 1), [255, 0, 0, 255]);
        assert_eq!(texel(atlas.texture(), 3, 1), [0, 0, 255, 128]);
        for y in 0..3 {
            for x in 0..3 {
                assert_ne!(texel(atlas.texture(), x, y)[2], 255, "blue entered red's allocation at {x},{y}");
            }
            for x in 3..6 {
                assert_eq!(texel(atlas.texture(), x, y)[0], 0, "red entered blue's allocation at {x},{y}");
            }
        }
    }

    #[test]
    fn source_bytes_are_left_unchanged() {
        let mut rgba = vec![1, 2, 3, 4];
        let mut builder = AtlasBuilder::new(AtlasConfig::new(1, 1));
        builder.add("src", 1, 1, &rgba).unwrap();
        rgba[0] = 9;
        let atlas = builder.build().unwrap();
        assert_eq!(atlas.texture().rgba, [1, 2, 3, 4]);
        assert_eq!(rgba, [9, 2, 3, 4]);
    }

    #[test]
    fn region_metadata_excludes_padding_and_uses_top_left_uv() {
        let atlas = pack(AtlasConfig::new(4, 4).with_padding(1), &[("spot", 1, 1, [1, 2, 3, 4])]);
        let region = atlas.region("spot").unwrap();
        assert_eq!((region.x, region.y, region.width, region.height), (1, 1, 1, 1));
        assert_eq!((region.source_width, region.source_height), (1, 1));
        assert_eq!(region.uv.min, Vec2::new(0.25, 0.25));
        assert_eq!(region.uv.max, Vec2::new(0.5, 0.5));
        let direct = SpriteAsset::from_atlas_rect(
            AssetHandle::from_name("atlas"),
            Vec2::new(4.0, 4.0),
            region.x,
            region.y,
            region.width,
            region.height,
        );
        assert_eq!(region.uv, direct.uv);
    }

    #[test]
    fn sprite_asset_uses_visible_size_and_default_units() {
        let atlas = pack(
            AtlasConfig::new(8, 4).with_padding(1),
            &[("left", 2, 2, [1, 0, 0, 255]), ("right", 2, 2, [0, 1, 0, 255])],
        );
        let mut assets = AssetManager::in_memory();
        let texture = assets.insert("pack", atlas.texture().clone()).unwrap();
        let left = atlas.sprite("left", texture).unwrap();
        let right = atlas.sprite("right", texture).unwrap();
        assert!(atlas.sprite("missing", texture).is_none());
        assert_eq!(left.texture, texture);
        assert_eq!(left.size_px, Vec2::new(2.0, 2.0));
        assert_eq!(left.uv, atlas.region("left").unwrap().uv);
        assert_eq!(left.pivot, Vec2::new(0.5, 0.0));
        assert_eq!(left.pixels_per_unit, SpriteAsset::DEFAULT_PIXELS_PER_UNIT);
        assert_eq!(right.pixels_per_unit, left.pixels_per_unit);
        assert_eq!(left.world_size(), Vec2::new(2.0, 2.0) / SpriteAsset::DEFAULT_PIXELS_PER_UNIT);
        assert_eq!(left.world_size(), right.world_size());
        assert_ne!(left.uv.min, right.uv.min);
        let direct = SpriteAsset::from_atlas_rect(texture, Vec2::new(8.0, 4.0), 1, 1, 2, 2);
        assert_eq!(left.uv, direct.uv);
        assert_eq!(left.size_px, direct.size_px);
        assert_eq!(left.pixels_per_unit, direct.pixels_per_unit);
        assets.insert("left", left).unwrap();
        assert!(assets.get(assets.sprite("left")).is_some());
    }

    #[test]
    fn output_is_a_texture_and_constructors_keep_their_defaults() {
        let plain = Texture::from_rgba8(1, 1, vec![1, 2, 3, 4]);
        assert_eq!(plain.color_space, ColorSpace::Srgb);
        assert_eq!(plain.mips, Mipmaps::None);
        assert_eq!(plain.sampler, SamplerDesc::default());

        let atlas = pack(AtlasConfig::new(2, 2), &[("a", 2, 2, [8, 7, 6, 5])]);
        assert_eq!(atlas.texture().width, 2);
        assert_eq!(atlas.texture().height, 2);
        assert_eq!(atlas.texture().color_space, ColorSpace::Srgb);
        assert_eq!(atlas.texture().mips, Mipmaps::None);
        assert_eq!(atlas.texture().sampler, SamplerDesc::default());
        assert_eq!(atlas.texture().rgba, solid(2, 2, [8, 7, 6, 5]));
        assert!(matches!(atlas.texture().pixel_edits(), TexturePixelEdits::Clean));

        let sampler = SamplerDesc {
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Nearest,
            mip_filter: Some(MipFilter::Nearest),
            address_u: AddressMode::Repeat,
            address_v: AddressMode::Mirror,
            anisotropy: 1,
        };
        let configured = pack(
            AtlasConfig::new(2, 1)
                .with_color_space(ColorSpace::Linear)
                .with_mipmaps(Mipmaps::Generate)
                .with_sampler(sampler)
                .unwrap(),
            &[("row", 2, 1, [1, 2, 3, 4])],
        );
        assert_eq!(configured.texture().color_space, ColorSpace::Linear);
        assert_eq!(configured.texture().mips, Mipmaps::Generate);
        assert_eq!(configured.texture().sampler, sampler);
        assert_eq!(configured.texture().rgba.len(), 8);
    }

    #[test]
    fn update_region_changes_visible_pixels_and_not_metadata() {
        let atlas = pack(
            AtlasConfig::new(6, 3).with_padding(1).with_extrusion(1),
            &[("red", 1, 1, [255, 0, 0, 255]), ("blue", 1, 1, [0, 0, 255, 255])],
        );
        let before = atlas.region("red").unwrap().clone();
        let mut texture = atlas.texture().clone();
        texture.update_region(before.x, before.y, before.width, before.height, &[0, 255, 0, 255]).unwrap();
        assert_eq!(atlas.region("red").unwrap(), &before);
        assert_eq!(texel(&texture, before.x, before.y), [0, 255, 0, 255]);
        assert_eq!(texel(&texture, before.x - 1, before.y), [255, 0, 0, 255]);
        assert_eq!(texel(&texture, atlas.region("blue").unwrap().x, atlas.region("blue").unwrap().y), [0, 0, 255, 255]);
        assert!(matches!(texture.pixel_edits(), TexturePixelEdits::Journal(_)));
    }

    #[test]
    fn add_rejects_invalid_images_without_storing_them() {
        let mut builder = AtlasBuilder::new(AtlasConfig::new(4, 4));
        assert_eq!(builder.add("", 1, 1, &[1, 2, 3, 4]), Err(AtlasError::EmptyName));
        assert_eq!(builder.add("wide", 0, 1, &[]), Err(AtlasError::ZeroWidth { name: "wide".into() }));
        assert_eq!(builder.add("tall", 1, 0, &[]), Err(AtlasError::ZeroHeight { name: "tall".into() }));
        assert_eq!(
            builder.add("short", 1, 1, &[1, 2, 3]),
            Err(AtlasError::InsufficientData { name: "short".into(), expected: 4, actual: 3 })
        );
        assert_eq!(
            builder.add("long", 1, 1, &[1, 2, 3, 4, 5]),
            Err(AtlasError::ExcessData { name: "long".into(), expected: 4, actual: 5 })
        );
        builder.add("ok", 1, 1, &[1, 2, 3, 4]).unwrap();
        assert_eq!(builder.add("ok", 1, 1, &[1, 2, 3, 4]), Err(AtlasError::DuplicateName { name: "ok".into() }));
        assert_eq!(builder.build().unwrap().regions().len(), 1);
    }

    #[test]
    fn empty_input_and_zero_atlas_sides_are_errors() {
        let empty = AtlasBuilder::new(AtlasConfig::new(4, 4));
        assert_eq!(empty.build().unwrap_err(), AtlasError::Empty);

        let mut builder = AtlasBuilder::new(AtlasConfig::new(0, 4));
        builder.add("a", 1, 1, &[1, 2, 3, 4]).unwrap();
        assert_eq!(builder.build().unwrap_err(), AtlasError::ZeroAtlasWidth);
        builder.config_mut().width = 4;
        builder.config_mut().height = 0;
        assert_eq!(builder.build().unwrap_err(), AtlasError::ZeroAtlasHeight);
    }

    #[test]
    fn oversized_padding_capacity_and_extrusion_are_errors() {
        let mut builder = AtlasBuilder::new(AtlasConfig::new(4, 4));
        builder.add("big", 5, 1, &solid(5, 1, [1, 0, 0, 255])).unwrap();
        assert_eq!(
            builder.build().unwrap_err(),
            AtlasError::ExceedsAtlas { name: "big".into(), width: 5, height: 1, atlas_width: 4, atlas_height: 4 }
        );

        let mut padded = AtlasBuilder::new(AtlasConfig::new(4, 4).with_padding(1));
        padded.add("fit", 4, 4, &solid(4, 4, [1, 0, 0, 255])).unwrap();
        assert_eq!(
            padded.build().unwrap_err(),
            AtlasError::PaddingOverflow {
                name: "fit".into(),
                width: 4,
                height: 4,
                padded_width: 6,
                padded_height: 6,
                atlas_width: 4,
                atlas_height: 4,
            }
        );

        let mut crowded = AtlasBuilder::new(AtlasConfig::new(5, 3));
        crowded.add("first", 3, 3, &solid(3, 3, [1, 0, 0, 255])).unwrap();
        crowded.add("second", 3, 3, &solid(3, 3, [0, 1, 0, 255])).unwrap();
        assert_eq!(
            crowded.build().unwrap_err(),
            AtlasError::OutOfSpace { name: "second".into(), atlas_width: 5, atlas_height: 3 }
        );

        let mut gutter = AtlasBuilder::new(AtlasConfig::new(4, 4).with_padding(1).with_extrusion(2));
        gutter.add("a", 1, 1, &[1, 2, 3, 4]).unwrap();
        assert_eq!(gutter.build().unwrap_err(), AtlasError::ExtrusionExceedsPadding { extrusion: 2, padding: 1 });
    }

    #[test]
    fn layout_and_byte_length_overflow_allocate_nothing() {
        let mut arithmetic = AtlasBuilder::new(AtlasConfig::new(u32::MAX, u32::MAX).with_padding(0x7fff_fffe));
        arithmetic.add("a", 1, 1, &[1, 2, 3, 4]).unwrap();
        arithmetic.add("b", 1, 1, &[5, 6, 7, 8]).unwrap();
        assert_eq!(arithmetic.build().unwrap_err(), AtlasError::Overflow { name: "b".into() });

        let mut bytes = AtlasBuilder::new(AtlasConfig::new(u32::MAX, u32::MAX));
        bytes.add("dot", 1, 1, &[1, 2, 3, 4]).unwrap();
        assert_eq!(bytes.build().unwrap_err(), AtlasError::AtlasByteOverflow { width: u32::MAX, height: u32::MAX });

        let mut image = AtlasBuilder::new(AtlasConfig::new(1, 1));
        assert_eq!(image.add("huge", u32::MAX, u32::MAX, &[]), Err(AtlasError::Overflow { name: "huge".into() }));
    }

    #[test]
    fn a_failed_build_can_be_retried_after_the_config_changes() {
        let mut builder = AtlasBuilder::new(AtlasConfig::new(2, 2));
        builder.add("a", 2, 2, &solid(2, 2, [1, 2, 3, 4])).unwrap();
        builder.add("b", 2, 2, &solid(2, 2, [5, 6, 7, 8])).unwrap();
        assert!(builder.build().is_err());
        builder.config_mut().width = 4;
        builder.config_mut().height = 2;
        let atlas = builder.build().unwrap();
        assert_eq!(atlas.regions().len(), 2);
        assert_eq!(atlas.texture().rgba.len(), 4 * 2 * 4);
    }

    #[test]
    fn anisotropy_below_one_is_rejected() {
        let mut builder = AtlasBuilder::new(AtlasConfig::new(1, 1));
        builder.config_mut().sampler.anisotropy = 0;
        builder.add("a", 1, 1, &[1, 2, 3, 4]).unwrap();
        assert_eq!(builder.build().unwrap_err(), AtlasError::Sampler(SamplerConfigError::AnisotropyBelowOne));
    }
}
