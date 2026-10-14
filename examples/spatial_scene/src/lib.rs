//! The reusable spatial test scene behind the `spatial_scene` example and its tests.
//!
//! One world is built once ([`scene::build_world`]) and viewed through named camera configurations
//! ([`cameras`]). A configuration is only a `Camera` and a `Transform`: there is no isometric mode and no
//! per-view code path anywhere in extraction or the backend.

pub mod art;
pub mod cameras;
pub mod scene;
