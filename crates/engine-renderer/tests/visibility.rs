//! Effective visibility is the entity's own `Visibility` plus every ancestor.
//! Extraction reads that chain and does not write a cached component.

use engine_assets::{AssetManager, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_world::{propagate_transforms, GlobalTransform, Name, Parent, Tags, Visibility, World};

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("tex_a", Texture::solid(16, 16, [255; 4])).unwrap();
    assets.insert("tex_a_sprite", SpriteAsset::whole_texture(texture, 16, 16)).unwrap();
    assets.insert("cube", MeshData::cuboid(Vec3::ONE).unwrap()).unwrap();
    assets
}

fn scene_world() -> World {
    let mut world = World::new();
    world.spawn((Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -10.0)));
    world
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, VIEWPORT).expect("camera")
}

fn sprite_at(z: f32) -> (Transform, Sprite) {
    (Transform::from_xyz(0.0, 0.0, z), Sprite::new("tex_a_sprite"))
}

#[test]
fn a_hidden_parent_hides_descendants_and_a_hidden_child_stays_hidden() {
    let assets = assets();
    let mut world = scene_world();
    let hidden = world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Visibility::Hidden));
    let visible_parent = world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Visibility::Visible));
    world.spawn_child(hidden, sprite_at(20.0));
    let shown = world.spawn_child(visible_parent, sprite_at(20.0));
    world.spawn_child(
        visible_parent,
        (Transform::from_xyz(2.0, 0.0, 20.0), Sprite::new("tex_a_sprite"), Visibility::Hidden),
    );

    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.sprites_visible, 1);
    assert_eq!(scene.sole_pass().blended_sprites.instances.len(), 1);
    assert_eq!(*world.component::<Visibility>(hidden).unwrap(), Visibility::Hidden);
    assert!(world.component::<Visibility>(shown).is_err(), "the shown child has no local visibility");
}

#[test]
fn a_hidden_ancestor_hides_every_deeper_level_until_that_bit_is_removed() {
    let assets = assets();
    let mut world = scene_world();
    let root = world.spawn((Transform::IDENTITY, Visibility::Hidden));
    let middle = world.spawn_child(root, (Transform::from_xyz(0.0, 1.0, 0.0), Visibility::Visible));
    let leaf = world.spawn_child(middle, sprite_at(20.0));
    let locally_hidden = world
        .spawn_child(middle, (Transform::from_xyz(1.0, 0.0, 20.0), Sprite::new("tex_a_sprite"), Visibility::Hidden));

    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 0);

    world.remove::<Visibility>(root).unwrap();
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.sprites_visible, 1, "the locally visible leaf returns");
    assert_eq!(*world.component::<Visibility>(locally_hidden).unwrap(), Visibility::Hidden);
    assert!(world.component::<Visibility>(leaf).is_err());
}

#[test]
fn a_transform_less_hidden_parent_hides_children_without_moving_them() {
    let assets = assets();
    let mut world = scene_world();
    let group = world.spawn((Visibility::Hidden,));
    let child = world.spawn_child(group, sprite_at(20.0));
    propagate_transforms(&mut world);
    let before = world.component::<GlobalTransform>(child).unwrap().translation();

    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 0);
    assert_eq!(world.component::<GlobalTransform>(child).unwrap().translation(), before);
    assert_eq!(*world.component::<Visibility>(group).unwrap(), Visibility::Hidden);
    assert!(world.component::<Visibility>(child).is_err());
}

#[test]
fn missing_visibility_is_locally_visible() {
    let assets = assets();
    let mut world = scene_world();
    let parent = world.spawn((Transform::IDENTITY,));
    world.spawn_child(parent, sprite_at(20.0));
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 1);
}

#[test]
fn reparenting_and_detaching_follow_the_new_ancestor_chain() {
    let assets = assets();
    let mut world = scene_world();
    let hidden = world.spawn((Transform::IDENTITY, Visibility::Hidden));
    let shown = world.spawn((Transform::from_xyz(3.0, 0.0, 0.0),));
    let child = world.spawn_child(hidden, sprite_at(20.0));
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 0);

    world.set_parent(child, shown).unwrap();
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 1);

    world.set_parent(child, hidden).unwrap();
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 0);

    world.detach_children(hidden).unwrap();
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 1);
    assert!(world.component::<Parent>(child).is_err());
}

#[test]
fn a_stale_parent_and_a_cycle_finish_the_walk() {
    let assets = assets();
    let mut world = scene_world();
    let dead = world.spawn(());
    world.despawn(dead).unwrap();
    let orphan = world.spawn(sprite_at(20.0));
    world.entities.insert_one(orphan, Parent(dead)).unwrap();
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 1);

    let mut cycled = scene_world();
    let sprite = cycled.spawn(sprite_at(20.0));
    let other = cycled.spawn((Visibility::Hidden,));
    cycled.entities.insert_one(sprite, Parent(other)).unwrap();
    cycled.entities.insert_one(other, Parent(sprite)).unwrap();
    assert_eq!(extract(&mut cycled, &assets).stats.sprites_visible, 0);

    let mut plain = scene_world();
    let left = plain.spawn(sprite_at(20.0));
    let right = plain.spawn(());
    plain.entities.insert_one(left, Parent(right)).unwrap();
    plain.entities.insert_one(right, Parent(left)).unwrap();
    assert_eq!(extract(&mut plain, &assets).stats.sprites_visible, 1);
}

#[test]
fn meshes_tilemaps_and_lights_use_the_same_ancestor_rule() {
    let assets = assets();
    let mut world = scene_world();
    let hidden = world.spawn((Transform::IDENTITY, Visibility::Hidden));
    world.spawn_child(hidden, (Transform::from_xyz(0.0, 0.0, 20.0), Mesh::new("cube")));
    world.spawn_child(hidden, (Transform::from_xyz(0.0, 0.0, 5.0), TileMap::new("tex_a", 1, 1, Vec2::ONE, 2, 2)));
    world.spawn_child(hidden, (Transform::IDENTITY, Light::point(Color::WHITE, 1.0, 10.0)));
    world.spawn((Transform::from_xyz(0.0, 0.0, 20.0), Mesh::new("cube"), Visibility::Hidden));
    world.spawn((Transform::from_xyz(1.0, 0.0, 20.0), Mesh::new("cube")));
    world.spawn((Transform::IDENTITY, Light::directional(Color::WHITE, 1.0), Visibility::Hidden));
    world.spawn((Transform::from_xyz(0.0, 2.0, 0.0), Light::point(Color::WHITE, 1.0, 8.0)));

    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.meshes_visible, 1);
    assert_eq!(scene.sole_pass().meshes.len(), 1);
    assert!(scene.sole_pass().cutout_sprites.instances.is_empty(), "the hidden tilemap contributes no tiles");
    assert_eq!(scene.sole_pass().lights.len(), 1);
    assert!(matches!(scene.sole_pass().lights[0].kind, LightKind::Point { .. }));
}

#[test]
fn camera_activation_ignores_visibility() {
    let assets = assets();
    let mut world = World::new();
    let camera = world.spawn((Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -10.0), Visibility::Hidden));
    world.spawn(sprite_at(20.0));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.sprites_visible, 1, "a hidden camera entity still renders when it is active");

    let child = world.spawn_child(camera, sprite_at(18.0));
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 1, "the child of that camera is hidden");
    assert!(world.component::<Visibility>(child).is_err());
    assert!(world.component::<Camera>(camera).unwrap().active);
}

#[test]
fn names_and_tags_do_not_change_extraction() {
    let assets = assets();
    let mut world = scene_world();
    let entity = world.spawn((
        Transform::from_xyz(0.0, 0.0, 20.0),
        Sprite::new("tex_a_sprite"),
        Name::new("prop"),
        Tags::from_tags(["interactable", ""]),
    ));
    let before = world.component::<Transform>(entity).unwrap().position;
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 1);

    assert!(world.component_mut::<Tags>(entity).unwrap().insert("door"));
    assert!(world.component_mut::<Tags>(entity).unwrap().remove("interactable"));
    assert_eq!(world.component::<Transform>(entity).unwrap().position, before);
    assert!(world.component::<Visibility>(entity).is_err());
    assert_eq!(extract(&mut world, &assets).stats.sprites_visible, 1);
}
