//! Coplanar priority is data on the sprite. It is not a batch key, and a tile map does not read it.

use engine_assets::{AssetManager, MaterialAsset, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Mat4, Transform, Vec2};
use engine_renderer::{
    extract_scene, Camera, CoplanarError, CoplanarPriority, CoplanarSettings, Sprite, SpriteInstance, SpriteShading,
    TileMap, CANDIDATE_COPLANAR_THICKNESS,
};
use engine_world::{propagate_transforms, World};

fn extract(world: &mut World, assets: &AssetManager) -> engine_renderer::RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, Vec2::new(64.0, 64.0)).unwrap()
}

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("white-tex", Texture::solid(1, 1, [255; 4])).unwrap();
    assets.insert("quad", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("tiles", Texture::solid(2, 2, [255; 4])).unwrap();
    assets
}

fn sprite(level: Option<u8>) -> (Transform, Sprite, Option<CoplanarPriority>) {
    let sprite = Sprite::new("quad").with_size(Vec2::splat(1.0)).with_anchor(Vec2::splat(0.5)).cutout().unlit();
    (Transform::from_xyz(0.0, 0.0, 0.0), sprite, level.map(CoplanarPriority))
}

#[test]
fn priority_is_not_a_batch_key() {
    let shading = SpriteShading::LegacyCutout;
    let instance = |level| SpriteInstance {
        model: Mat4::IDENTITY,
        uv: [0.0, 0.0, 1.0, 1.0],
        tint: [1.0; 4],
        lit: false,
        texture: engine_assets::AssetHandle::from_name("white-tex"),
        render_target: None,
        unavailable_target: false,
        shading,
        material_id: None,
        material_revision: 0,
        coplanar_level: level,
    };
    assert!(instance(None).shares_batch(&instance(Some(7))));
    assert!(instance(Some(0)).shares_batch(&instance(Some(3))));
}

#[test]
fn extraction_keeps_each_sprite_level_and_one_batch() {
    let mut assets = assets();
    let material = assets.insert("paint", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = World::new();
    world.spawn((Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -5.0)));
    for (index, level) in [0u8, 4, 7].into_iter().enumerate() {
        world.spawn((
            Transform::from_xyz(index as f32 * 0.6 - 0.6, 0.0, 0.0),
            Sprite::new("quad").with_size(Vec2::splat(0.4)).with_anchor(Vec2::splat(0.5)).with_material(material),
            CoplanarPriority::new(level).unwrap(),
        ));
    }
    let scene = extract(&mut world, &assets);
    let layer = &scene.sole_pass().opaque_sprites;
    assert_eq!(layer.batches.len(), 1, "priority must not split a shared material");
    let levels: Vec<_> = layer.instances.iter().map(|instance| instance.coplanar_level).collect();
    assert_eq!(levels, vec![Some(0), Some(4), Some(7)]);
    assert_eq!(scene.coplanar_thickness, CANDIDATE_COPLANAR_THICKNESS);
}

#[test]
fn a_missing_component_is_ordinary_depth_and_an_out_of_range_level_is_not_clamped() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -5.0)));
    world.spawn(sprite(None));
    world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), sprite(Some(9)).1, CoplanarPriority(9)));
    let scene = extract(&mut world, &assets);
    let levels: Vec<_> = scene.sole_pass().cutout_sprites.instances.iter().map(|i| i.coplanar_level).collect();
    assert_eq!(levels, vec![None, None]);
    assert_eq!(CoplanarPriority::new(8), Err(CoplanarError::PriorityOutOfRange { level: 8 }));
}

#[test]
fn an_explicit_thickness_is_used_and_an_invalid_one_disables_the_pull() {
    let assets = assets();
    let mut world = World::new();
    world.insert_resource(CoplanarSettings::new(1.0 / 64.0).unwrap());
    world.spawn((Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -5.0)));
    world.spawn((sprite(Some(1)).0, sprite(Some(1)).1, CoplanarPriority::new(1).unwrap()));
    assert_eq!(extract(&mut world, &assets).coplanar_thickness, 1.0 / 64.0);

    world.insert_resource(CoplanarSettings { thickness: -1.0 });
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.coplanar_thickness, 0.0);
    assert_eq!(scene.sole_pass().cutout_sprites.instances[0].coplanar_level, Some(1));
}

#[test]
fn a_tile_map_does_not_read_coplanar_priority() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -5.0)));
    world.spawn((
        Transform::IDENTITY,
        TileMap::new("tiles", 1, 1, Vec2::ONE, 1, 1).filled_with(|_, _| 0),
        CoplanarPriority::new(7).unwrap(),
    ));
    let scene = extract(&mut world, &assets);
    assert!(scene.sole_pass().cutout_sprites.instances.iter().all(|tile| tile.coplanar_level.is_none()));
}
