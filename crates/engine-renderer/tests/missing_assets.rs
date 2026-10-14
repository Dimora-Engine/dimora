//! Missing renderables log once. A load that is still in flight does not.

use engine_assets::{AssetManager, LoadState, SpriteAsset, Texture};
use engine_math::{Transform, Vec2};
use engine_renderer::{extract_scene, Camera, MissingAssetWarnings, Sprite};
use engine_world::{propagate_transforms, Visibility, World};

mod capture {
    include!("../../log_capture.rs");
}

const VIEWPORT: Vec2 = Vec2::new(64.0, 64.0);

fn world_with_camera() -> World {
    let mut world = World::new();
    world.insert_resource(MissingAssetWarnings::default());
    world.spawn((Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -8.0)));
    world
}

fn extract(world: &mut World, assets: &AssetManager) {
    propagate_transforms(world);
    extract_scene(world, assets, VIEWPORT).expect("camera");
}

#[test]
fn a_missing_sprite_warns_once_until_it_loads_and_goes_missing_again() {
    capture::begin();
    let mut assets = AssetManager::in_memory();
    let mut world = world_with_camera();
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Sprite::new("hero")));
    world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Sprite::new("hero")));

    extract(&mut world, &assets);
    extract(&mut world, &assets);
    let first = capture::messages();
    assert_eq!(first.len(), 1, "{first:?}");
    assert!(first[0].contains("skipping sprite"));
    assert!(first[0].contains("is missing"), "{first:?}");
    assert_eq!(capture::records()[0].level, log::Level::Warn);

    let texture = assets.insert("hero_tex", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    let hero = assets.insert("hero", SpriteAsset::whole_texture(texture, 1, 1)).unwrap();
    extract(&mut world, &assets);
    assert_eq!(capture::messages().len(), 1, "a loaded asset does not warn");

    assert!(assets.remove(hero));
    extract(&mut world, &assets);
    extract(&mut world, &assets);
    let again = capture::messages();
    assert_eq!(again.len(), 2, "{again:?}");
    assert!(again[1].contains("is missing"));
}

#[test]
fn a_pending_load_does_not_warn_and_a_failure_warns_once() {
    capture::begin();
    let mut assets = AssetManager::in_memory();
    let pending = assets.load::<SpriteAsset>("later.png").unwrap();
    assert_eq!(assets.state(pending), LoadState::Loading);
    let mut world = world_with_camera();
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Sprite::new("later.png")));

    extract(&mut world, &assets);
    assert!(capture::messages().is_empty(), "{:?}", capture::messages());

    assets.process_loads();
    assert_eq!(assets.state(pending), LoadState::Failed);
    extract(&mut world, &assets);
    extract(&mut world, &assets);
    let messages = capture::messages();
    let skips = messages.iter().filter(|message| message.contains("skipping sprite")).count();
    let loads = messages.iter().filter(|message| message.starts_with("asset ")).count();
    assert_eq!(loads, 1, "{messages:?}");
    assert_eq!(skips, 1, "{messages:?}");
    assert!(messages.iter().any(|message| message.contains("later.png") && message.contains("failed to load")));
}

#[test]
fn a_hidden_renderable_is_not_a_missing_asset_warning() {
    capture::begin();
    let assets = AssetManager::in_memory();
    let mut world = world_with_camera();
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Sprite::new("hidden"), Visibility::Hidden));
    extract(&mut world, &assets);
    assert!(capture::messages().is_empty(), "{:?}", capture::messages());
}
