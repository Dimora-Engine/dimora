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
    assert!(loaded_scene.cutout_sprites.instances.is_empty());
    assert_eq!(loaded_scene.meshes.len(), spawned_scene.meshes.len());
    let loaded_sprite = loaded_scene.blended_sprites.instances[0];
    let spawned_sprite = spawned_scene.blended_sprites.instances[0];
    assert_eq!(loaded_sprite.model, spawned_sprite.model);
    assert_eq!(loaded_sprite.uv, spawned_sprite.uv);
    assert_eq!(loaded_sprite.tint, spawned_sprite.tint);
    assert_eq!(loaded_sprite.lit, spawned_sprite.lit);
    assert_eq!(loaded_sprite.texture, spawned_sprite.texture);
    assert_eq!(loaded_scene.view.view_proj, spawned_scene.view.view_proj);
}
