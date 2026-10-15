//! Hand-written registrations for animation components.
//!
//! `SkeletonInstance`'s private name index is omitted. It mirrors `bones` and stores runtime ids.

use engine_world::{ComponentDesc, ComponentRegistry, FieldDesc, Persistence};

use crate::clip::AnimationPlayer;
use crate::skeleton::{Bone, SkeletonInstance};

const BONE_FIELDS: &[FieldDesc] = &[FieldDesc::new("index", "usize")];
const PLAYER_FIELDS: &[FieldDesc] = &[
    FieldDesc::new("clip", "AssetHandle<AnimationClip>"),
    FieldDesc::new("time", "f32"),
    FieldDesc::new("speed", "f32"),
    FieldDesc::new("playing", "bool"),
];
const INSTANCE_FIELDS: &[FieldDesc] = &[FieldDesc::new("bones", "Vec<Entity>")];

/// Registers bone, playback, and skeleton-instance components as [`Persistence::Runtime`].
///
/// A skeleton in a future scene file is an asset reference plus a spawn, not a copy of these
/// runtime components. That choice belongs to M19.
///
/// An identical registration already stored for one of these types is left in place.
/// A conflicting name or field list is returned.
pub fn register_animation_components(registry: &mut ComponentRegistry) -> Result<(), engine_world::RegistryError> {
    registry.register_idempotent(ComponentDesc::new("Bone", BONE_FIELDS, Persistence::Runtime, Some(bone_values)))?;
    registry.register_idempotent(ComponentDesc::new(
        "AnimationPlayer",
        PLAYER_FIELDS,
        Persistence::Runtime,
        Some(player_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "SkeletonInstance",
        INSTANCE_FIELDS,
        Persistence::Runtime,
        Some(instance_values),
    ))?;
    Ok(())
}

fn bone_values(bone: &Bone) -> Vec<String> {
    vec![bone.index.to_string()]
}

fn player_values(player: &AnimationPlayer) -> Vec<String> {
    vec![
        format!("{:016x}", player.clip.id().0),
        format!("{:?}", player.time),
        format!("{:?}", player.speed),
        player.playing.to_string(),
    ]
}

fn instance_values(instance: &SkeletonInstance) -> Vec<String> {
    vec![instance.bones.iter().map(|entity| format!("{entity:?}")).collect::<Vec<_>>().join(", ")]
}
