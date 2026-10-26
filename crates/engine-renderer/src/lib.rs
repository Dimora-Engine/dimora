//! Backend-agnostic rendering model for 2.5D worlds.
//!
//! This crate answers "what should be drawn, and where does it land on screen?" without touching a
//! GPU:
//!
//! * [`projection`] and [`camera`] turn real 3D world positions into screen positions.
//!   [`Camera::pixel_snap`] quantizes an orthographic view translation onto that camera's viewport
//!   pixel grid. Perspective and custom projections keep the flag and do not snap.
//! * [`components`] are the renderable things that live in the ECS next to `Transform`:
//!   [`Sprite`], [`Mesh`], [`TileMap`], [`Light`].
//! * [`extract_scene`] snapshots the world into a plain-data [`RenderScene`], doing visibility,
//!   frustum culling, depth sorting and batching on the way.
//! * [`DebugDraw`] is single-frame world-space line data. Extraction copies it; it is not an entity.
//! * [`CameraPresentation::Depth`] replaces one camera's viewport with a linearized view of the
//!   depth that camera wrote. A shaded camera does not run that pass.
//! * [`PlacedCamera`] builds a [`Camera`] and a `Transform` for the common placements.
//!   Isometric, top-down, steep, and side are those two values, not projection types.
//!   [`CameraProjection::oblique_orthographic`] and [`CameraProjection::oblique_perspective`]
//!   store a reverse-Z shear as [`CameraProjection::Custom`].
//!
//! A GPU backend (see `engine-renderer-wgpu`) consumes a `RenderScene` and nothing else.

pub mod bench_scene;
pub mod camera;
pub mod components;
pub mod coplanar;
pub mod debug;
pub mod depth_view;
mod extract;
pub mod frustum;
mod interpolate;
mod missing;
mod msaa;
pub mod presets;
pub mod projection;
mod registry;
mod render_target;
pub mod scene;

pub use camera::{
    fit_internal_resolution, Camera, CameraClear, CameraOutput, CameraPresentation, CameraView, PresentationFit,
    PresentationReject, ScaledImage, ViewportRect,
};
pub use components::{
    AmbientLight, Fog, Light, LightKind, Material, Mesh, RenderOrder, Sprite, SpriteBlend, SpriteOrientation, TileMap,
};
pub use coplanar::{
    resolve_coplanar_thickness, CoplanarError, CoplanarPriority, CoplanarSettings, CANDIDATE_COPLANAR_THICKNESS,
    MAX_COPLANAR_PRIORITY,
};
pub use debug::{frustum_corners, DebugDraw, DebugSegment, DEBUG_SPHERE_SEGMENTS, DEBUG_SPHERE_SEGMENT_LIMIT};
pub use depth_view::{
    classify_stored_depth, linear_nearness, StoredDepthClass, DEPTH_VIEW_EMPTY, DEPTH_VIEW_MIN_SPAN,
    DEPTH_VIEW_UNSUPPORTED,
};
pub use extract::{
    active_surface_cameras, camera_view_of, extract_scene, top_surface_camera, OrderedCamera, SurfaceHit,
};
pub use frustum::Frustum;
pub use missing::{MissingAssetWarnings, MISSING_ASSET_WARNING_CAPACITY};
pub use msaa::{msaa4_camera_skip, select_msaa, MsaaError, MsaaSamples, MsaaSkip};
pub use presets::PlacedCamera;
pub use projection::{CameraProjection, Orthographic, Perspective, Projection, ProjectionError, ScreenPosition};
pub use registry::register_render_components;
pub use render_target::{
    check_render_target_size, RenderTargetDesc, RenderTargetError, RenderTargetId, RenderTargetStore,
    RENDER_TARGET_MAX_EXTENT,
};
pub use scene::{
    CameraPass, ExtractStats, LightData, MeshDraw, MeshInstance, RenderScene, SpriteBatch, SpriteInstance, SpriteLayer,
    SpritePipelineKind, SpriteShading, TransparentBatch, MAX_LIGHTS,
};
