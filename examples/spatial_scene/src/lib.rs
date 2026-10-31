//! The reusable spatial test scene behind the `spatial_scene` example and its tests.
//!
//! [`scene::build_world`] publishes environment A and spawns the camera and walker beside it. Those
//! two stay when [`scene::request_environment_switch`] replaces A with B, or B with A.
//! [`scene::install_collision`] is the opt-in `--collision` path. [`locomotion`] is the opt-in
//! `--locomotion` course. The default world calls neither. [`prefab::spawn_lamp`]
//! is the documented code-defined prefab: a function of `&mut World` that returns one root entity.
//! A camera configuration ([`cameras`]) is only a `Camera` and a `Transform`: there is no isometric mode
//! and no per-view code path anywhere in extraction or the backend. `custom` stores
//! `CameraProjection::oblique_orthographic` on `CameraProjection::Custom`.

pub mod art;
pub mod cameras;
pub mod locomotion;
pub mod prefab;
pub mod scene;
