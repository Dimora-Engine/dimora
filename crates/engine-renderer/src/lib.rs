//! Backend-agnostic rendering model for 2.5D worlds.
//!
//! This crate answers "what should be drawn, and where does it land on screen?" without touching a
//! GPU:
//!
//! * [`projection`] and [`camera`] turn real 3D world positions into screen positions.
//! * [`components`] are the renderable things that live in the ECS next to `Transform`:
//!   [`Sprite`], [`Mesh`], [`TileMap`], [`Light`].
//! * [`extract_scene`] snapshots the world into a plain-data [`RenderScene`], doing visibility,
//!   frustum culling, depth sorting and batching on the way.
//!
//! A GPU backend (see `engine-renderer-wgpu`) consumes a `RenderScene` and nothing else.

pub mod camera;
pub mod components;
mod extract;
pub mod frustum;
mod interpolate;
mod missing;
pub mod projection;
mod registry;
pub mod scene;

pub use camera::{Camera, CameraView, ViewportRect};
pub use components::{
    AmbientLight, Fog, Light, LightKind, Material, Mesh, RenderOrder, Sprite, SpriteBlend, SpriteOrientation, TileMap,
};
pub use extract::extract_scene;
pub use frustum::Frustum;
pub use missing::{MissingAssetWarnings, MISSING_ASSET_WARNING_CAPACITY};
pub use projection::{CameraProjection, Orthographic, Perspective, Projection, ProjectionError, ScreenPosition};
pub use registry::register_render_components;
pub use scene::{
    ExtractStats, LightData, MeshDraw, MeshInstance, RenderScene, SpriteBatch, SpriteInstance, SpriteLayer,
    SpritePipelineKind, SpriteShading, TransparentBatch, MAX_LIGHTS,
};
