//! wgpu backend for `engine-renderer`.
//!
//! Input is a [`engine_renderer::RenderScene`] plus the [`engine_assets::AssetManager`] (to
//! resolve textures and meshes into GPU resources). It never sees the ECS.
//!
//! * `GpuContext`: instance/adapter/device/queue.
//! * `SceneRenderer`: pipelines, GPU caches and the frame encoder. Target-agnostic.
//! * `WindowRenderer`: a `SceneRenderer` bound to a window surface.
//! * `SceneRenderer::render_to_rgba`: headless rendering, used for tests and screenshots.

mod draw_pipelines;
mod gpu;
mod gpu_types;
mod mip;
mod present;
mod scene_renderer;
mod window;

pub use gpu::{GpuContext, RenderError};
pub use present::{choose_surface_present, surface_presents_from_wgpu, wgpu_present_for, SurfacePresent};
pub use scene_renderer::{
    msaa4_supported, CacheStats, CachedTextureInfo, FrameStats, GpuResourceStats, InternalTargetUsage, MsaaImageUsage,
    PassFrameStats, SceneRenderer, SurfaceFrameUsage, DEFAULT_CACHE_MAX_IDLE_FRAMES,
};
pub use wgpu;
pub use window::WindowRenderer;
