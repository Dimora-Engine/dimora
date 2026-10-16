//! Hand-written registrations for components defined in this crate.
//!
//! Field names are declared here on purpose. Changing a struct does not change this list.

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
