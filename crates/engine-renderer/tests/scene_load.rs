//! A scene-loaded renderable takes the same extraction path as a spawned one.

use engine_assets::{AssetManager, SpriteAsset, Texture};
use engine_math::{Transform, Vec2};
use engine_renderer::*;
use engine_world::{propagate_transforms, SceneMember, World};

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("tex_a", Texture::solid(16, 16, [255; 4])).unwrap();
    assets.insert("tex_a_sprite", SpriteAsset::whole_texture(texture, 16, 16)).unwrap();
    assets
}

fn sprite() -> (Transform, Sprite) {
    (Transform::from_xyz(1.0, 2.0, 4.0), Sprite::new("tex_a_sprite"))
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, VIEWPORT).expect("camera")
}

#[test]
fn a_scene_sprite_extracts_like_a_directly_spawned_sprite() {
    let assets = assets();

    let mut spawned = World::new();
    spawned.spawn((Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -10.0)));
    spawned.spawn(sprite());
    let spawned_scene = extract(&mut spawned, &assets);

    let mut loaded = World::new();
    loaded.spawn((Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -10.0)));
    let instance = loaded
        .load_scene(|builder| {
            builder.spawn(sprite());
            Ok(())
        })
        .unwrap();
    let member = loaded.scene_entities(instance).unwrap().unwrap();
    assert_eq!(member, vec![(0, member[0].1)]);
    assert_eq!(loaded.scene_membership(member[0].1).unwrap(), Some(SceneMember { instance, local: 0 }));
    let loaded_scene = extract(&mut loaded, &assets);

    assert_eq!(loaded_scene.stats, spawned_scene.stats);
    assert_eq!(loaded_scene.stats.sprites_visible, 1);
    assert!(loaded_scene.sole_pass().cutout_sprites.instances.is_empty());
    assert_eq!(loaded_scene.sole_pass().meshes.len(), spawned_scene.sole_pass().meshes.len());
    let loaded_sprite = loaded_scene.sole_pass().blended_sprites.instances[0];
    let spawned_sprite = spawned_scene.sole_pass().blended_sprites.instances[0];
    assert_eq!(loaded_sprite.model, spawned_sprite.model);
    assert_eq!(loaded_sprite.uv, spawned_sprite.uv);
    assert_eq!(loaded_sprite.tint, spawned_sprite.tint);
    assert_eq!(loaded_sprite.lit, spawned_sprite.lit);
    assert_eq!(loaded_sprite.texture, spawned_sprite.texture);
    assert_eq!(loaded_scene.sole_pass().view.view_proj, spawned_scene.sole_pass().view.view_proj);
}

#[test]
fn unloading_a_scene_leaves_shared_and_application_assets_in_place() {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("shared", Texture::solid(2, 2, [255, 0, 0, 255])).unwrap();
    let sprite = assets.insert("shared_sprite", SpriteAsset::whole_texture(texture, 2, 2)).unwrap();
    let application = assets.insert("app_only", Texture::solid(2, 2, [0, 255, 0, 255])).unwrap();

    let mut world = World::new();
    let first = world
        .load_scene(|builder| {
            builder.spawn((Transform::IDENTITY, Sprite::new("shared_sprite")));
            Ok(())
        })
        .unwrap();
    let second = world
        .load_scene(|builder| {
            builder.spawn((Transform::IDENTITY, Sprite::new("shared_sprite")));
            Ok(())
        })
        .unwrap();
    let runtime = world.spawn((Transform::IDENTITY, Sprite::new("shared_sprite")));

    world.unload_scene(first).unwrap();
    assert!(assets.get(sprite).is_some());
    assert!(assets.get(texture).is_some());
    assert!(assets.get(application).is_some());
    assert!(world.scene_entity(second, 0).unwrap().is_some());

    world.unload_scene(second).unwrap();
    assert!(world.contains(runtime));
    assert!(assets.get(sprite).is_some());

    let again = world
        .load_scene(|builder| {
            builder.spawn((Transform::IDENTITY, Sprite::new("shared_sprite")));
            Ok(())
        })
        .unwrap();
    assert!(assets.get(assets.sprite("shared_sprite")).is_some());
    world.unload_scene(again).unwrap();
    assert!(assets.get(application).is_some());
    assert!(assets.get(sprite).is_some());
    assert!(assets.get(texture).is_some());
}
