//! 2D skeletal animation, built on the spatial hierarchy rather than beside it.
//!
//! A [`Skeleton`] is a plain asset: named bones with a parent index and a rest `Transform`.
//! [`spawn_skeleton`] instantiates it as *entities*, one per bone, parented like any other entity.
//! A sprite attached to a bone is simply a child entity with a `Sprite` component, so a 2D character
//! is a spatial skeleton whose visuals happen to be 2D, and a 3D mesh could hang off the same bone.
//!
//! Implemented now: skeleton definition, instancing, keyframed [`AnimationClip`]s and a single-clip
//! [`AnimationPlayer`]. Designed for but not built: clip blending / mixer, IK (two-bone, FABRIK) and
//! constraints. Those will operate on [`Pose`] before it is written to the bone entities.

mod clip;
mod registry;
mod skeleton;

pub use clip::{animate_system, AnimationClip, AnimationPlayer, BoneTrack, Keyframe};
pub use registry::register_animation_components;
pub use skeleton::{spawn_skeleton, Bone, BoneDef, Pose, Skeleton, SkeletonError, SkeletonInstance};
