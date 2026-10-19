//! Missing renderables log once. A load that is still in flight does not.

use engine_assets::{AssetManager, LoadState, SpriteAsset, Texture};
use engine_math::{Transform, Vec2};
use engine_renderer::{extract_scene, Camera, MissingAssetWarnings, Sprite, MISSING_ASSET_WARNING_CAPACITY};
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
fn a_sprite_with_a_missing_texture_warns_once_and_does_not_invent_a_name() {
    capture::begin();
    let mut assets = AssetManager::in_memory();
    let missing = engine_assets::AssetHandle::<Texture>::from_name("no-such-texture");
    assets.insert("hero", SpriteAsset::whole_texture(missing, 1, 1)).unwrap();
    let mut world = world_with_camera();
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Sprite::new("hero")));
    world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Sprite::new("hero")));

    propagate_transforms(&mut world);
    let first = extract_scene(&world, &assets, VIEWPORT).expect("camera");
    assert_eq!(first.stats.sprites_visible, 2, "the sprite is drawn; only its texture is missing");
    assert_eq!(first.stats.missing_assets, 2);
    extract(&mut world, &assets);
    let messages = capture::messages();
    let warnings = messages.iter().filter(|message| message.contains("missing sprite texture")).collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{messages:?}");
    assert!(warnings[0].contains(&format!("{:016x}", missing.id().0)), "{warnings:?}");
    assert!(warnings[0].contains("referenced by sprite 'hero'"), "{warnings:?}");
    assert!(!warnings[0].contains("no-such-texture"), "{warnings:?}");
}

#[test]
fn a_failed_texture_load_is_named_and_a_pending_one_is_silent() {
    capture::begin();
    let mut assets = AssetManager::in_memory();
    let pending = assets.load::<Texture>("tiles.png").unwrap();
    assets.insert("hero", SpriteAsset::whole_texture(pending, 1, 1)).unwrap();
    let mut world = world_with_camera();
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Sprite::new("hero")));
    extract(&mut world, &assets);
    assert!(
        capture::messages().iter().all(|message| !message.contains("missing sprite texture")),
        "{:?}",
        capture::messages()
    );

    assets.process_loads();
    assert_eq!(assets.state(pending), LoadState::Failed);
    extract(&mut world, &assets);
    extract(&mut world, &assets);
    let messages = capture::messages();
    let warnings = messages.iter().filter(|message| message.contains("missing sprite texture")).collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{messages:?}");
    assert!(warnings[0].contains("tiles.png"), "{warnings:?}");
    assert!(warnings[0].contains("failed to load"), "{warnings:?}");
}

#[test]
fn a_mesh_with_a_missing_texture_warns_once_without_inventing_a_name() {
    capture::begin();
    let mut assets = AssetManager::in_memory();
    assets.insert("crate", engine_assets::MeshData::cuboid(engine_math::Vec3::ONE)).unwrap();
    let missing = engine_assets::AssetHandle::<Texture>::from_name("no-mesh-texture");
    let mut world = world_with_camera();
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        engine_renderer::Mesh::new("crate").with_material(engine_renderer::Material {
            base_color: engine_core::Color::WHITE,
            base_texture: Some(missing),
            lit: false,
        }),
    ));
    extract(&mut world, &assets);
    extract(&mut world, &assets);
    let messages = capture::messages();
    let warnings = messages.iter().filter(|message| message.contains("missing mesh texture")).collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{messages:?}");
    assert!(warnings[0].contains(&format!("{:016x}", missing.id().0)), "{warnings:?}");
    assert!(warnings[0].contains("referenced by mesh 'crate'"), "{warnings:?}");
    assert!(!warnings[0].contains("no-mesh-texture"), "{warnings:?}");
}

#[test]
fn a_full_warning_buffer_counts_later_misses_and_logs_one_aggregate() {
    capture::begin();
    let assets = AssetManager::in_memory();
    let mut world = world_with_camera();
    let extra = 3;
    for index in 0..MISSING_ASSET_WARNING_CAPACITY + extra {
        world.spawn((Transform::from_xyz(index as f32, 0.0, 0.0), Sprite::new(&format!("missing-{index}"))));
    }
    propagate_transforms(&mut world);
    let scene = extract_scene(&world, &assets, VIEWPORT).expect("camera");
    assert_eq!(scene.stats.missing_assets, (MISSING_ASSET_WARNING_CAPACITY + extra) as u32);
    extract(&mut world, &assets);
    let messages = capture::messages();
    let skips = messages.iter().filter(|message| message.contains("skipping sprite")).count();
    let aggregates = messages.iter().filter(|message| message.contains("warning buffer is full")).count();
    assert_eq!(skips, MISSING_ASSET_WARNING_CAPACITY, "{messages:?}");
    assert_eq!(aggregates, 1, "{messages:?}");
    assert!(messages.iter().any(|message| message.contains("64")), "{messages:?}");
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
