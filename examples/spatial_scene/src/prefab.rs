//! One code-defined prefab: a lamp post.
//!
//! [`spawn_lamp`] is an ordinary function. It takes `&mut World` and returns the root [`Entity`].
//! Each call creates a new post and a new light. There is no prefab asset, manager, or variant.
//!
//! [`place_lamp`] builds the same parts through [`SceneBuilder`]. A scene closure should use that
//! form. [`spawn_lamp`] creates entities the builder is not tracking, so a failed scene build would
//! not roll them back and they would not become scene members.

use engine::prelude::*;

/// Height of the pillar mesh. The root sits at its centre, so a lamp standing on `position.y`
/// places the root `PILLAR_HEIGHT / 2` above that point.
const PILLAR_HEIGHT: f32 = 200.0;

/// Local offset from the post centre to the light, near the top of the pillar.
const LIGHT_OFFSET: Vec3 = Vec3::new(0.0, 110.0, 0.0);

/// Spawns a pillar with a point light parented above it.
///
/// `pillar` and `panel` must already be registered. The returned entity is the post. Despawning it
/// with [`World::despawn`] removes the light as well. Calling this again creates another hierarchy;
/// it does not reuse the previous entities.
pub fn spawn_lamp(world: &mut World, position: Vec3) -> Entity {
    let root = world.spawn(post_bundle(position));
    let light = world.spawn(light_bundle());
    world.set_parent(light, root).expect("the lamp light parents to its post");
    root
}

/// Spawns a lamp as members of the scene `builder` is loading.
///
/// The root and the light are created by this build, so a later `Err` from the scene closure rolls
/// both back, and a successful load records both as [`SceneMember`].
pub fn place_lamp(builder: &mut SceneBuilder<'_>, position: Vec3) -> Result<Entity, SceneError> {
    let root = builder.spawn(post_bundle(position));
    builder.spawn_child(root, light_bundle())?;
    Ok(root)
}

fn post_bundle(position: Vec3) -> (Transform, Mesh, Name) {
    (
        Transform::from_xyz(position.x, position.y + PILLAR_HEIGHT * 0.5, position.z),
        Mesh::new("pillar").with_material(Material {
            base_color: Color::rgb(0.75, 0.72, 0.62),
            base_texture: Some(AssetHandle::from_name("panel")),
            lit: true,
        }),
        Name::new("lamp"),
    )
}

fn light_bundle() -> (Transform, Light, Name) {
    (
        Transform::from_position(LIGHT_OFFSET),
        Light::point(Color::rgb(1.0, 0.90, 0.70), 3.0, 400.0),
        Name::new("lamp light"),
    )
}
