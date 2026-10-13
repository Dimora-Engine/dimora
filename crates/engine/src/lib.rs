//! The engine facade.
//!
//! Use this crate from a game; it re-exports the pieces and provides [`Game`], a small struct that
//! owns a [`World`](engine_world::World), two [`Schedule`](engine_world::Schedule)s and the frame pipeline:
//!
//! ```text
//! time -> asset loads -> fixed-step systems -> update systems -> animation
//!      -> transform propagation -> extract -> render
//! ```
//!
//! `Game` is *not* the world: it holds one. Everything gameplay touches (time, input, assets,
//! ambient light) is a resource inside the world, so systems are plain `FnMut(&mut World)`.
//!
//! ```no_run
//! use engine::prelude::*;
//!
//! let mut game = Game::new(GameConfig::default());
//! game.world.spawn((Transform::from_xyz(0.0, 2.0, -10.0), Camera::perspective(50.0)));
//! game.run().unwrap();
//! ```

mod game;
mod runner;

pub use engine_animation as animation;
pub use engine_assets as assets;
pub use engine_core as core;
pub use engine_math as math;
pub use engine_platform as platform;
pub use engine_renderer as renderer;
pub use engine_renderer_wgpu as renderer_wgpu;
pub use engine_world as world;

pub use game::{Game, GameConfig, RenderStats, Viewport, WindowControl};

/// Everything a typical game needs in scope.
pub mod prelude {
    pub use crate::{Game, GameConfig, RenderStats, Viewport, WindowControl};
    pub use engine_animation::{
        animate_system, spawn_skeleton, AnimationClip, AnimationPlayer, Bone, BoneTrack, Keyframe, Pose, Skeleton,
        SkeletonInstance,
    };
    pub use engine_assets::{
        AssetHandle, AssetManager, AssetSource, FileSystemSource, MemorySource, MeshData, SpriteAsset, Texture,
        TextureFilter,
    };
    pub use engine_core::{Color, Input, Key, MouseButton, Time};
    pub use engine_math::{Mat4, Quat, Rect, Transform, Vec2, Vec3, Vec4};
    pub use engine_renderer::{
        AmbientLight, Camera, CameraProjection, CameraView, Fog, Light, LightKind, Material, Mesh, Orthographic,
        Perspective, Projection, RenderOrder, Sprite, SpriteBlend, SpriteOrientation, TileMap, ViewportRect,
    };
    pub use engine_world::{Children, Entity, GlobalTransform, Name, Parent, Schedule, Visibility, World};
}
