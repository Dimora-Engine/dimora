//! Assets, separated from rendering.
//!
//! This crate defines *what* assets are (identity, handles, CPU-side data, where bytes come from)
//! and nothing about GPUs. Renderers resolve [`AssetHandle`]s through the [`AssetManager`] and keep
//! their own caches keyed by `(AssetId, revision)`.
//!
//! ```
//! use engine_assets::*;
//! let mut assets = AssetManager::in_memory();
//! let tex = assets.insert("player_idle", Texture::solid(4, 4, [255, 0, 255, 255])).unwrap();
//! assets.insert("player", SpriteAsset::whole_texture(tex, 4, 4)).unwrap();
//! assert!(assets.get(assets.sprite("player")).is_some());
//! ```

mod atlas;
mod error;
mod handle;
mod manager;
pub mod mesh;
mod source;
mod sprite;
mod texture;

pub use atlas::{Atlas, AtlasBuilder, AtlasConfig, AtlasError, AtlasRegion};
pub use error::AssetError;
pub use handle::{Asset, AssetHandle, AssetId, AssetKind};
pub use manager::{AssetManager, LoadState};
pub use mesh::{MeshData, Vertex};
pub use source::{AssetSource, FileSystemSource, MemorySource};
pub use sprite::SpriteAsset;
pub use texture::{
    rgba_fingerprint, validate_texture_dimensions, AddressMode, BaseLevelUpload, ColorSpace, MipFilter, Mipmaps,
    SamplerConfigError, SamplerDesc, TexelRect, Texture, TextureDimensionError, TextureDimensionLimits, TextureEdit,
    TextureFilter, TexturePixelEdits, TextureRegionError, MAX_TRACKED_REGIONS,
};
