//! Hand-written registrations for components defined in this crate.
//!
//! Field names are declared here on purpose. Changing a struct does not change this list.

use crate::collision::{
    CharacterLocomotion, CharacterMotion, CharacterMotor, Collider, ColliderShape, GroundSupport, LocomotionIntent,
    LocomotionPhase, MoveIntent, MoveResult, MoveStatus,
};
use crate::hierarchy::{Children, GlobalTransform, Parent};
use crate::interpolate::Interpolate;
use crate::lookup::Tags;
use crate::registry::{ComponentDesc, ComponentRegistry, FieldDesc, Persistence};
use crate::scene::SceneMember;
use crate::world::{Name, Visibility};
use crate::Transform;

const NAME_FIELDS: &[FieldDesc] = &[FieldDesc::new("value", "String")];
const TAG_FIELDS: &[FieldDesc] = &[FieldDesc::new("values", "Vec<String>")];
const TRANSFORM_FIELDS: &[FieldDesc] =
    &[FieldDesc::new("position", "Vec3"), FieldDesc::new("rotation", "Quat"), FieldDesc::new("scale", "Vec3")];
const VISIBILITY_FIELDS: &[FieldDesc] = &[FieldDesc::new("state", "Visibility")];
const SCENE_MEMBER_FIELDS: &[FieldDesc] =
    &[FieldDesc::new("instance", "SceneInstanceId"), FieldDesc::new("local", "u32")];
const PARENT_FIELDS: &[FieldDesc] = &[FieldDesc::new("entity", "Entity")];
const CHILDREN_FIELDS: &[FieldDesc] = &[FieldDesc::new("entities", "Vec<Entity>")];
const GLOBAL_FIELDS: &[FieldDesc] = &[FieldDesc::new("matrix", "Mat4")];
const INTERPOLATE_FIELDS: &[FieldDesc] = &[FieldDesc::new("previous", "Option<Transform>")];
const MOTOR_FIELDS: &[FieldDesc] =
    &[FieldDesc::new("skin", "f32"), FieldDesc::new("max_slides", "u8"), FieldDesc::new("locked_axes", "BVec3")];
const MOVE_INTENT_FIELDS: &[FieldDesc] = &[FieldDesc::new("translation", "Vec3")];
const LOCOMOTION_FIELDS: &[FieldDesc] = &[
    FieldDesc::new("enabled", "bool"),
    FieldDesc::new("speed", "f32"),
    FieldDesc::new("max_slope", "f32"),
    FieldDesc::new("stick", "f32"),
];
const MOTION_FIELDS: &[FieldDesc] =
    &[FieldDesc::new("support", "GroundSupport"), FieldDesc::new("phase", "LocomotionPhase")];
const LOCOMOTION_INTENT_FIELDS: &[FieldDesc] = &[FieldDesc::new("wish", "Vec3")];
const MOVE_RESULT_FIELDS: &[FieldDesc] = &[
    FieldDesc::new("requested", "Vec3"),
    FieldDesc::new("applied", "Vec3"),
    FieldDesc::new("status", "MoveStatus"),
    FieldDesc::new("normals", "Vec<Vec3>"),
    FieldDesc::new("recovery_normal", "Option<Vec3>"),
];
const COLLIDER_FIELDS: &[FieldDesc] = &[
    FieldDesc::new("shape", "ColliderShape"),
    FieldDesc::new("offset", "Vec3"),
    FieldDesc::new("layers", "u32"),
    FieldDesc::new("mask", "u32"),
    FieldDesc::new("sensor", "bool"),
    FieldDesc::new("enabled", "bool"),
];

/// Registers `Name`, `Tags`, `Transform`, `Visibility`, `SceneMember`, and the hierarchy and interpolation components.
///
/// `SceneMember`, `Parent`, `Children`, `GlobalTransform`, and `Interpolate` are [`Persistence::Runtime`].
/// Their values hold derived data or runtime entity ids.
///
/// An identical registration already stored for one of these types is left in place.
/// A conflicting name or field list is returned.
pub fn register_world_components(registry: &mut ComponentRegistry) -> Result<(), crate::registry::RegistryError> {
    registry.register_idempotent(ComponentDesc::new("Name", NAME_FIELDS, Persistence::Persist, Some(name_values)))?;
    registry.register_idempotent(ComponentDesc::new("Tags", TAG_FIELDS, Persistence::Persist, Some(tags_values)))?;
    registry.register_idempotent(ComponentDesc::new(
        "Transform",
        TRANSFORM_FIELDS,
        Persistence::Persist,
        Some(transform_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "Visibility",
        VISIBILITY_FIELDS,
        Persistence::Persist,
        Some(visibility_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "SceneMember",
        SCENE_MEMBER_FIELDS,
        Persistence::Runtime,
        Some(scene_member_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "Parent",
        PARENT_FIELDS,
        Persistence::Runtime,
        Some(parent_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "Children",
        CHILDREN_FIELDS,
        Persistence::Runtime,
        Some(children_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "GlobalTransform",
        GLOBAL_FIELDS,
        Persistence::Runtime,
        Some(global_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "Interpolate",
        INTERPOLATE_FIELDS,
        Persistence::Runtime,
        Some(interpolate_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "Collider",
        COLLIDER_FIELDS,
        Persistence::Persist,
        Some(collider_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "CharacterMotor",
        MOTOR_FIELDS,
        Persistence::Persist,
        Some(motor_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "MoveIntent",
        MOVE_INTENT_FIELDS,
        Persistence::Runtime,
        Some(intent_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "MoveResult",
        MOVE_RESULT_FIELDS,
        Persistence::Runtime,
        Some(result_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "CharacterLocomotion",
        LOCOMOTION_FIELDS,
        Persistence::Persist,
        Some(locomotion_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "CharacterMotion",
        MOTION_FIELDS,
        Persistence::Runtime,
        Some(motion_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "LocomotionIntent",
        LOCOMOTION_INTENT_FIELDS,
        Persistence::Runtime,
        Some(locomotion_intent_values),
    ))?;
    Ok(())
}

fn name_values(name: &Name) -> Vec<String> {
    vec![escape_debug(&name.0)]
}

fn tags_values(tags: &Tags) -> Vec<String> {
    let listed: Vec<String> = tags.iter().map(|tag| format!("{tag:?}")).collect();
    if listed.is_empty() {
        vec!["(none)".to_string()]
    } else {
        vec![listed.join(", ")]
    }
}

fn transform_values(transform: &Transform) -> Vec<String> {
    vec![vec3(transform.position), quat(transform.rotation), vec3(transform.scale)]
}

fn visibility_values(visibility: &Visibility) -> Vec<String> {
    vec![match visibility {
        Visibility::Visible => "Visible".to_string(),
        Visibility::Hidden => "Hidden".to_string(),
    }]
}

fn scene_member_values(member: &SceneMember) -> Vec<String> {
    vec![member.instance.raw().to_string(), member.local.to_string()]
}

fn parent_values(parent: &Parent) -> Vec<String> {
    vec![format!("{:?}", parent.0)]
}

fn children_values(children: &Children) -> Vec<String> {
    vec![children.0.iter().map(|entity| format!("{entity:?}")).collect::<Vec<_>>().join(", ")]
}

fn global_values(global: &GlobalTransform) -> Vec<String> {
    let translation = global.translation();
    vec![format!("translation={}", vec3(translation))]
}

fn interpolate_values(mark: &Interpolate) -> Vec<String> {
    vec![match mark.previous {
        Some(transform) => format!(
            "position={} rotation={} scale={}",
            vec3(transform.position),
            quat(transform.rotation),
            vec3(transform.scale)
        ),
        None => "none".to_string(),
    }]
}

fn motor_values(motor: &CharacterMotor) -> Vec<String> {
    vec![
        format!("{:?}", motor.skin),
        motor.max_slides.to_string(),
        format!("({}, {}, {})", motor.locked_axes.x, motor.locked_axes.y, motor.locked_axes.z),
    ]
}

fn intent_values(intent: &MoveIntent) -> Vec<String> {
    vec![vec3(intent.translation)]
}

fn locomotion_values(locomotion: &CharacterLocomotion) -> Vec<String> {
    vec![
        locomotion.enabled.to_string(),
        format!("{:?}", locomotion.speed),
        format!("{:?}", locomotion.max_slope),
        format!("{:?}", locomotion.stick),
    ]
}

fn motion_values(motion: &CharacterMotion) -> Vec<String> {
    vec![support_text(motion.support), phase_text(motion.phase)]
}

fn locomotion_intent_values(intent: &LocomotionIntent) -> Vec<String> {
    vec![vec3(intent.wish)]
}

fn support_text(support: GroundSupport) -> String {
    match support {
        GroundSupport::Unknown => "Unknown".to_string(),
        GroundSupport::Airborne => "Airborne".to_string(),
        GroundSupport::Unavailable => "Unavailable".to_string(),
        GroundSupport::Supported { entity, normal } => format!("Supported({entity:?}, {})", vec3(normal)),
    }
}

fn phase_text(phase: LocomotionPhase) -> String {
    match phase {
        LocomotionPhase::Idle => "Idle".to_string(),
        LocomotionPhase::Walk => "Walk".to_string(),
    }
}

fn result_values(result: &MoveResult) -> Vec<String> {
    let recovery = match result.recovery_normal {
        Some(normal) => vec3(normal),
        None => "none".to_string(),
    };
    vec![
        vec3(result.requested),
        vec3(result.applied),
        match result.status {
            MoveStatus::Completed => "Completed",
            MoveStatus::Blocked => "Blocked",
            MoveStatus::Rejected => "Rejected",
        }
        .to_string(),
        format!("{} normals", result.normals.len()),
        recovery,
    ]
}

fn collider_values(collider: &Collider) -> Vec<String> {
    vec![
        shape_text(collider.shape),
        vec3(collider.offset),
        collider.layers.to_string(),
        collider.mask.to_string(),
        collider.sensor.to_string(),
        collider.enabled.to_string(),
    ]
}

fn shape_text(shape: ColliderShape) -> String {
    match shape {
        ColliderShape::Sphere { radius } => format!("Sphere(radius={radius:?})"),
        ColliderShape::Box { half_extents } => format!("Box(half_extents={})", vec3(half_extents)),
        ColliderShape::Capsule { radius, half_height } => {
            format!("Capsule(radius={radius:?}, half_height={half_height:?})")
        }
    }
}

fn vec3(value: engine_math::Vec3) -> String {
    format!("({:?}, {:?}, {:?})", value.x, value.y, value.z)
}

fn quat(value: engine_math::Quat) -> String {
    format!("({:?}, {:?}, {:?}, {:?})", value.x, value.y, value.z, value.w)
}

fn escape_debug(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}
