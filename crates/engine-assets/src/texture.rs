use crate::{Asset, AssetError, AssetKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextureFilter {
    /// Crisp texels; the default for pixel art.
    #[default]
    Nearest,
    Linear,
}

/// CPU-side image, tightly packed RGBA8 in sRGB.
#[derive(Clone, Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub filter: TextureFilter,
}

impl Texture {
    pub fn from_rgba8(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert_eq!(rgba.len(), (width * height * 4) as usize, "RGBA8 buffer has the wrong size");
        Self { width, height, rgba, filter: TextureFilter::default() }
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

    pub fn with_filter(mut self, filter: TextureFilter) -> Self {
        self.filter = filter;
        self
    }
}

impl Asset for Texture {
    const KIND: AssetKind = AssetKind::Texture;

    fn decode(bytes: &[u8]) -> Result<Self, AssetError> {
        let img = image::load_from_memory(bytes).map_err(|e| AssetError::Decode(e.to_string()))?.to_rgba8();
        Ok(Texture::from_rgba8(img.width(), img.height(), img.into_raw()))
    }
}
