//! Hand-written registrations for renderable components.
//!
//! The field list is metadata. It is not generated from the struct definition.

use engine_assets::AssetHandle;
use engine_core::Color;
use engine_math::Vec2;
use engine_world::{ComponentDesc, ComponentRegistry, FieldDesc, Persistence};

use crate::{Camera, CoplanarPriority, Light, Mesh, RenderOrder, Sprite, TileMap};

const SPRITE_FIELDS: &[FieldDesc] = &[
    FieldDesc::new("asset", "AssetHandle<SpriteAsset>"),
    FieldDesc::new("region", "Option<Rect>"),
    FieldDesc::new("size", "Option<Vec2>"),
    FieldDesc::new("anchor", "Option<Vec2>"),
    FieldDesc::new("flip_x", "bool"),
    FieldDesc::new("flip_y", "bool"),
    FieldDesc::new("tint", "Color"),
    FieldDesc::new("orientation", "SpriteOrientation"),
    FieldDesc::new("blend", "SpriteBlend"),
    FieldDesc::new("lit", "bool"),
    FieldDesc::new("material", "Option<AssetHandle<MaterialAsset>>"),
    FieldDesc::new("render_target", "Option<RenderTargetId>"),
];

const ORDER_FIELDS: &[FieldDesc] = &[FieldDesc::new("layer", "i32"), FieldDesc::new("order", "i32")];

const COPLANAR_FIELDS: &[FieldDesc] = &[FieldDesc::new("level", "u8")];

const MESH_FIELDS: &[FieldDesc] = &[
    FieldDesc::new("asset", "AssetHandle<MeshData>"),
    FieldDesc::new("material", "Material"),
    FieldDesc::new("material_asset", "Option<AssetHandle<MaterialAsset>>"),
];

const TILE_FIELDS: &[FieldDesc] = &[
    FieldDesc::new("texture", "AssetHandle<Texture>"),
    FieldDesc::new("atlas_columns", "u32"),
    FieldDesc::new("atlas_rows", "u32"),
    FieldDesc::new("tile_size", "Vec2"),
    FieldDesc::new("width", "u32"),
    FieldDesc::new("height", "u32"),
    FieldDesc::new("tiles", "Vec<u16>"),
    FieldDesc::new("lit", "bool"),
];

const CAMERA_FIELDS: &[FieldDesc] = &[
    FieldDesc::new("projection", "CameraProjection"),
    FieldDesc::new("viewport", "ViewportRect"),
    FieldDesc::new("clear_color", "Color"),
    FieldDesc::new("clear", "CameraClear"),
    FieldDesc::new("output", "CameraOutput"),
    FieldDesc::new("active", "bool"),
    FieldDesc::new("order", "i32"),
    FieldDesc::new("presentation", "CameraPresentation"),
    FieldDesc::new("pixel_snap", "bool"),
];

const LIGHT_FIELDS: &[FieldDesc] =
    &[FieldDesc::new("kind", "LightKind"), FieldDesc::new("color", "Color"), FieldDesc::new("intensity", "f32")];

/// Registers the renderable components. `Material` is a field of `Mesh`, not its own component.
/// `Fog`, `AmbientLight`, and `CoplanarSettings` are resources and are not registered here.
/// `CoplanarPriority` is. A tile map does not read that component.
///
/// An identical registration already stored for one of these types is left in place.
/// A conflicting name or field list is returned.
pub fn register_render_components(registry: &mut ComponentRegistry) -> Result<(), engine_world::RegistryError> {
    registry.register_idempotent(ComponentDesc::new(
        "Sprite",
        SPRITE_FIELDS,
        Persistence::Persist,
        Some(sprite_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "RenderOrder",
        ORDER_FIELDS,
        Persistence::Persist,
        Some(order_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "CoplanarPriority",
        COPLANAR_FIELDS,
        Persistence::Persist,
        Some(coplanar_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new("Mesh", MESH_FIELDS, Persistence::Persist, Some(mesh_values)))?;
    registry.register_idempotent(ComponentDesc::new(
        "TileMap",
        TILE_FIELDS,
        Persistence::Persist,
        Some(tile_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "Camera",
        CAMERA_FIELDS,
        Persistence::Persist,
        Some(camera_values),
    ))?;
    registry.register_idempotent(ComponentDesc::new(
        "Light",
        LIGHT_FIELDS,
        Persistence::Persist,
        Some(light_values),
    ))?;
    Ok(())
}

fn sprite_values(sprite: &Sprite) -> Vec<String> {
    vec![
        asset_id(sprite.asset),
        option_debug(sprite.region),
        option_debug(sprite.size),
        option_debug(sprite.anchor),
        sprite.flip_x.to_string(),
        sprite.flip_y.to_string(),
        color(sprite.tint),
        format!("{:?}", sprite.orientation),
        format!("{:?}", sprite.blend),
        sprite.lit.to_string(),
        option_asset(sprite.material),
        match sprite.render_target {
            Some(id) => id.to_string(),
            None => "none".to_string(),
        },
    ]
}

fn order_values(order: &RenderOrder) -> Vec<String> {
    vec![order.layer.to_string(), order.order.to_string()]
}

fn coplanar_values(priority: &CoplanarPriority) -> Vec<String> {
    vec![priority.0.to_string()]
}

fn mesh_values(mesh: &Mesh) -> Vec<String> {
    let texture = match mesh.material.base_texture {
        Some(texture) => asset_id(texture),
        None => "none".to_string(),
    };
    vec![
        asset_id(mesh.asset),
        format!("base_color={} base_texture={texture} lit={}", color(mesh.material.base_color), mesh.material.lit),
        option_asset(mesh.material_asset),
    ]
}

fn tile_values(map: &TileMap) -> Vec<String> {
    vec![
        asset_id(map.texture),
        map.atlas_columns.to_string(),
        map.atlas_rows.to_string(),
        vec2(map.tile_size),
        map.width.to_string(),
        map.height.to_string(),
        format!("len={}", map.tiles.len()),
        map.lit.to_string(),
    ]
}

fn camera_values(camera: &Camera) -> Vec<String> {
    vec![
        format!("{:?}", camera.projection),
        format!("{:?}", camera.viewport),
        color(camera.clear_color),
        format!("{:?}", camera.clear),
        format!("{:?}", camera.output),
        camera.active.to_string(),
        camera.order.to_string(),
        format!("{:?}", camera.presentation),
        camera.pixel_snap.to_string(),
    ]
}

fn light_values(light: &Light) -> Vec<String> {
    vec![format!("{:?}", light.kind), color(light.color), format!("{:?}", light.intensity)]
}

fn asset_id<T>(handle: AssetHandle<T>) -> String {
    format!("{:016x}", handle.id().0)
}

fn option_asset<T>(handle: Option<AssetHandle<T>>) -> String {
    match handle {
        Some(handle) => asset_id(handle),
        None => "none".to_string(),
    }
}

fn option_debug<T: core::fmt::Debug>(value: Option<T>) -> String {
    match value {
        Some(value) => format!("{value:?}"),
        None => "none".to_string(),
    }
}

fn color(value: Color) -> String {
    format!("({:?}, {:?}, {:?}, {:?})", value.r, value.g, value.b, value.a)
}

fn vec2(value: Vec2) -> String {
    format!("({:?}, {:?})", value.x, value.y)
}
