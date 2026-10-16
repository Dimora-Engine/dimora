//! The reusable spatial test scene behind the `spatial_scene` example and its tests.
//!
//! [`scene::build_world`] publishes environment A and spawns the camera and walker beside it. Those
//! two stay when [`scene::request_environment_switch`] replaces A with B, or B with A. A camera
//! configuration ([`cameras`]) is only a `Camera` and a `Transform`: there is no isometric mode and
//! no per-view code path anywhere in extraction or the backend.

pub mod art;
pub mod cameras;
pub mod scene;
