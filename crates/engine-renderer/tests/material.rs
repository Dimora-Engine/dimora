//! Material handles on sprites and meshes, resolved at extraction.
//!
//! No handle keeps the legacy sprite or inline mesh path. A handle that does not resolve skips
//! that renderable. A mesh material in `AlphaMode::Blend` joins the combined transparent sequence.

use engine_assets::{AlphaMode, AssetHandle, AssetManager, LoadState, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Rect, Transform, Vec2, Vec3};
use engine_renderer::{
    extract_scene, Camera, Material, Mesh, MeshDraw, MissingAssetWarnings, RenderOrder, RenderScene, Sprite,
    SpriteBlend, SpritePipelineKind, SpriteShading, TileMap,
};
use engine_world::{propagate_transforms, World};

mod capture {
    include!("../../log_capture.rs");
}

fn world() -> World {
    let mut world = World::new();
    world.insert_resource(MissingAssetWarnings::default());
    world.spawn((Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -8.0)));
    world
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, Vec2::new(64.0, 64.0)).expect("camera")
}

fn sprite_asset(assets: &mut AssetManager, name: &str, rgba: [u8; 4]) -> AssetHandle<SpriteAsset> {
    let texture = assets.insert(&format!("{name}-tex"), Texture::solid(2, 1, rgba)).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 2, 1).with_pixels_per_unit(1.0)).unwrap()
}

fn right_half(assets: &mut AssetManager, name: &str) -> (AssetHandle<SpriteAsset>, AssetHandle<Texture>) {
    let texture = assets.insert(&format!("{name}-tex"), Texture::solid(2, 1, [255, 0, 0, 255])).unwrap();
    let sprite = SpriteAsset::from_atlas_rect(texture, Vec2::new(2.0, 1.0), 1, 0, 1, 1).with_pixels_per_unit(1.0);
    (assets.insert(name, sprite).unwrap(), texture)
}

#[test]
fn handles_default_to_none_and_existing_constructors_stay_compatible() {
    let sprite = Sprite::new("hero").with_size(Vec2::splat(2.0)).with_tint(Color::rgb(0.2, 0.3, 0.4)).unlit();
    assert!(sprite.material.is_none());
    assert!(!sprite.lit);
    assert_eq!(sprite.tint, Color::rgb(0.2, 0.3, 0.4));

    let from_asset = Sprite::from_asset(AssetHandle::from_name("hero"));
    assert!(from_asset.material.is_none());
    assert_eq!(from_asset.blend, SpriteBlend::Alpha);

    let inline = Material::unlit(Color::rgb(0.0, 0.0, 1.0));
    let mesh = Mesh::new("crate").with_material(inline);
    assert!(mesh.material_asset.is_none());
    assert_eq!(mesh.material.base_color, Color::rgb(0.0, 0.0, 1.0));
    assert!(!mesh.material.lit);

    let mut assets = AssetManager::in_memory();
    assets.insert("crate", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();
    let mut world = world();
    world.spawn((Transform::IDENTITY, mesh));
    let scene = extract(&mut world, &assets);
    let drawn = &scene.sole_pass().meshes[0];
    assert_eq!(drawn.color, Color::rgb(0.0, 0.0, 1.0).to_array());
    assert!(!drawn.lit);
    assert_eq!(drawn.draw, MeshDraw::Solid { double_sided: false });
    assert!(drawn.material_id.is_none());
    assert!(drawn.texture.is_none());
}

#[test]
fn an_unassigned_sprite_and_tile_keep_the_legacy_classification() {
    let mut assets = AssetManager::in_memory();
    sprite_asset(&mut assets, "hero", [255, 255, 255, 255]);
    assets.insert("tiles", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    let mut world = world();
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("hero").with_tint(Color::rgba(0.25, 0.5, 0.75, 1.0)).with_blend(SpriteBlend::Cutout).unlit(),
    ));
    world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Sprite::new("hero").with_blend(SpriteBlend::Alpha)));
    world.spawn((
        Transform::from_xyz(-2.0, 0.0, 0.0),
        TileMap::new("tiles", 1, 1, Vec2::ONE, 1, 1).filled_with(|_, _| 0),
    ));

    let scene = extract(&mut world, &assets);
    assert!(scene.sole_pass().opaque_sprites.instances.is_empty());
    assert!(scene.sole_pass().mask_sprites.instances.is_empty());
    assert_eq!(scene.sole_pass().cutout_sprites.instances.len(), 2);
    assert!(scene.sole_pass().cutout_sprites.instances.iter().all(|i| i.shading == SpriteShading::LegacyCutout));
    assert!(scene.sole_pass().cutout_sprites.instances.iter().all(|i| i.material_id.is_none()));
    let hero = scene.sole_pass().cutout_sprites.instances.iter().find(|i| i.tint[0] == 0.25).expect("tinted cutout");
    assert!(!hero.lit);
    assert_eq!(hero.tint, [0.25, 0.5, 0.75, 1.0]);
    assert_eq!(scene.sole_pass().blended_sprites.instances.len(), 1);
    assert_eq!(scene.sole_pass().blended_sprites.instances[0].shading, SpriteShading::LegacyBlend);
    assert!(scene.sole_pass().blended_sprites.instances[0].lit);
    assert_eq!(scene.sole_pass().blended_sprites.batches[0].pipeline, SpritePipelineKind::LegacyBlend);
}

#[test]
fn a_resolved_material_selects_texture_color_alpha_light_and_culling() {
    let mut assets = AssetManager::in_memory();
    let (sprite, sprite_texture) = right_half(&mut assets, "sign");
    let override_tex = assets.insert("paint", Texture::solid(2, 1, [0, 255, 0, 255])).unwrap();
    let material = assets
        .insert(
            "paint-mat",
            MaterialAsset::unlit(Color::rgba(2.0, 0.5, -1.0, 0.5))
                .with_alpha_mode(AlphaMode::mask(0.25).unwrap())
                .double_sided(),
        )
        .unwrap();
    material_albedo(&mut assets, material, Some(override_tex));

    let mut world = world();
    world.spawn((
        Transform::IDENTITY,
        Sprite::from_asset(sprite)
            .with_material(material)
            .with_tint(Color::rgba(3.0, 2.0, 0.5, 0.5))
            .with_region(Rect::new(Vec2::ZERO, Vec2::new(0.5, 1.0)))
            .with_blend(SpriteBlend::Cutout),
    ));
    let scene = extract(&mut world, &assets);
    let drawn = &scene.sole_pass().mask_sprites.instances[0];
    assert_eq!(drawn.texture, override_tex);
    assert_ne!(drawn.texture, sprite_texture);
    assert_eq!(drawn.tint, [6.0, 1.0, -0.5, 0.25], "linear product is not clamped");
    assert!(!drawn.lit, "the sprite lit flag is ignored once a material resolves");
    assert_eq!(drawn.shading, SpriteShading::Mask { cutoff: 0.25, double_sided: true });
    assert_eq!(drawn.shading.pipeline(), SpritePipelineKind::MaskDouble);
    assert_eq!(drawn.material_id, Some(material.id()));
    assert_eq!(drawn.material_revision, assets.revision(material));
    assert!(scene.sole_pass().cutout_sprites.instances.is_empty(), "mask does not stay on the legacy cutout list");

    let bare = Sprite::from_asset(sprite).with_region(Rect::new(Vec2::ZERO, Vec2::new(0.5, 1.0)));
    world.spawn((Transform::from_xyz(3.0, 0.0, 0.0), bare));
    let scene = extract(&mut world, &assets);
    let legacy = scene.sole_pass().blended_sprites.instances.iter().find(|i| i.material_id.is_none()).unwrap();
    let masked = &scene.sole_pass().mask_sprites.instances[0];
    assert_eq!(masked.uv, legacy.uv, "an albedo override keeps the sprite asset UVs");
}

fn material_albedo(
    assets: &mut AssetManager,
    handle: AssetHandle<MaterialAsset>,
    albedo: Option<AssetHandle<Texture>>,
) {
    let mut material = *assets.get(handle).unwrap();
    material.albedo = albedo;
    assets.insert("paint-mat", material).unwrap();
}

#[test]
fn albedo_none_uses_the_sprite_texture_and_leaves_a_mesh_untextured() {
    let mut assets = AssetManager::in_memory();
    let sprite = sprite_asset(&mut assets, "hero", [255, 0, 0, 255]);
    let sprite_texture = assets.get(sprite).unwrap().texture;
    let material = assets.insert("flat", MaterialAsset::color(Color::rgb(0.2, 0.3, 0.4))).unwrap();
    assets.insert("crate", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();
    let mut world = world();
    world.spawn((Transform::IDENTITY, Sprite::from_asset(sprite).with_material(material).cutout()));
    world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Mesh::new("crate").with_material_asset(material)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().opaque_sprites.instances[0].texture, sprite_texture);
    assert_eq!(scene.sole_pass().opaque_sprites.instances[0].shading, SpriteShading::Opaque { double_sided: false });
    assert!(scene.sole_pass().meshes[0].texture.is_none());
    assert_eq!(scene.sole_pass().meshes[0].color, Color::rgb(0.2, 0.3, 0.4).to_array());
    assert!(scene.sole_pass().meshes[0].lit);
    assert_eq!(scene.sole_pass().meshes[0].draw, MeshDraw::Solid { double_sided: false });
    assert_eq!(scene.sole_pass().meshes[0].material_id, Some(material.id()));
}

#[test]
fn a_missing_material_is_skipped_and_a_pending_load_is_quiet() {
    capture::begin();
    let mut assets = AssetManager::in_memory();
    sprite_asset(&mut assets, "hero", [255, 0, 0, 255]);
    let unknown = AssetHandle::<MaterialAsset>::from_name("not-registered");
    let mut world = world();
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("hero").with_material(unknown).with_tint(Color::rgb(1.0, 0.0, 0.0)).with_blend(SpriteBlend::Alpha),
    ));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.sprites_visible, 0);
    assert_eq!(scene.stats.missing_assets, 1);
    assert!(
        scene.sole_pass().blended_sprites.instances.is_empty(),
        "a missing material does not fall back to SpriteBlend"
    );
    let id = format!("{:016x}", unknown.id().0);
    let messages = capture::messages();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(messages[0].contains("skipping material"), "{messages:?}");
    assert!(messages[0].contains(&id), "{messages:?}");
    assert!(!messages[0].contains("not-registered"), "{messages:?}");
    assert_eq!(capture::records()[0].level, log::Level::Warn);

    extract(&mut world, &assets);
    assert_eq!(capture::messages().len(), 1, "the same missing material logs once");

    let pending = assets.load::<MaterialAsset>("later.mat").unwrap();
    assert_eq!(assets.state(pending), LoadState::Loading);
    world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Sprite::new("hero").with_material(pending)));
    extract(&mut world, &assets);
    assert_eq!(capture::messages().len(), 1, "a pending material load does not warn");

    assets.process_loads();
    assert_eq!(assets.state(pending), LoadState::Failed);
    extract(&mut world, &assets);
    extract(&mut world, &assets);
    let messages = capture::messages();
    let skips = messages.iter().filter(|m| m.contains("skipping material") && m.contains("later.mat")).count();
    assert_eq!(skips, 1, "{messages:?}");
}

#[test]
fn replacing_a_shared_material_changes_the_next_extract() {
    let mut assets = AssetManager::in_memory();
    sprite_asset(&mut assets, "hero", [255, 255, 255, 255]);
    let sprite_texture = assets.get(assets.sprite("hero")).unwrap().texture;
    let other = assets.insert("other", Texture::solid(1, 1, [0, 255, 0, 255])).unwrap();
    let material = assets.insert("shared", MaterialAsset::color(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let first_revision = assets.revision(material);
    let mut world = world();
    world.spawn((Transform::IDENTITY, Sprite::new("hero").with_material(material)));
    world.spawn((Transform::from_xyz(1.5, 0.0, 0.0), Sprite::new("hero").with_material(material)));

    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().opaque_sprites.instances.len(), 2);
    assert!(scene.sole_pass().opaque_sprites.instances.iter().all(|i| i.tint == [1.0, 0.0, 0.0, 1.0]));
    assert!(scene.sole_pass().opaque_sprites.instances.iter().all(|i| i.material_id == Some(material.id())));
    assert_eq!(scene.sole_pass().opaque_sprites.batches.len(), 1);

    assets.insert("shared", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0)).double_sided()).unwrap();
    let scene = extract(&mut world, &assets);
    assert!(assets.revision(material) > first_revision);
    assert!(scene.sole_pass().opaque_sprites.instances.iter().all(|i| i.tint == [0.0, 1.0, 0.0, 1.0]));
    assert!(scene.sole_pass().opaque_sprites.instances.iter().all(|i| !i.lit));
    assert!(scene.sole_pass().opaque_sprites.instances.iter().all(|i| {
        i.shading == SpriteShading::Opaque { double_sided: true } && i.material_revision == assets.revision(material)
    }));

    assets
        .insert("shared", MaterialAsset::default().with_alpha_mode(AlphaMode::mask(0.8).unwrap()).double_sided())
        .unwrap();
    let scene = extract(&mut world, &assets);
    assert!(scene.sole_pass().opaque_sprites.instances.is_empty());
    assert_eq!(scene.sole_pass().mask_sprites.instances.len(), 2);
    assert!(scene
        .sole_pass()
        .mask_sprites
        .instances
        .iter()
        .all(|i| i.shading == SpriteShading::Mask { cutoff: 0.8, double_sided: true }));

    let mut textured = MaterialAsset::default().with_alpha_mode(AlphaMode::Blend);
    textured.albedo = Some(other);
    assets.insert("shared", textured).unwrap();
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().blended_sprites.instances.len(), 2);
    assert!(scene
        .sole_pass()
        .blended_sprites
        .instances
        .iter()
        .all(|i| i.texture == other && i.texture != sprite_texture));
    assert!(scene
        .sole_pass()
        .blended_sprites
        .instances
        .iter()
        .all(|i| i.shading == SpriteShading::Blend { double_sided: false }));
}

#[test]
fn blended_sprites_and_a_blended_mesh_share_one_sort() {
    capture::begin();
    let mut assets = AssetManager::in_memory();
    sprite_asset(&mut assets, "red", [255, 0, 0, 255]);
    sprite_asset(&mut assets, "blue", [0, 0, 255, 255]);
    let blend = assets.insert("glass", MaterialAsset::default().with_alpha_mode(AlphaMode::Blend)).unwrap();
    assets.insert("crate", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();
    let mut world = world();
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Sprite::new("red").with_material(blend), RenderOrder::new(0, 0)));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Sprite::new("blue").with_blend(SpriteBlend::Alpha),
        RenderOrder::new(0, 0),
    ));
    world.spawn((Transform::from_xyz(0.0, 0.0, 4.0), Sprite::new("red").with_material(blend), RenderOrder::new(0, 0)));
    world.spawn((Transform::from_xyz(3.0, 0.0, 0.0), Mesh::new("crate").with_material_asset(blend)));

    let scene = extract(&mut world, &assets);
    let names: Vec<_> = scene
        .sole_pass()
        .blended_sprites
        .instances
        .iter()
        .map(|i| if i.material_id.is_some() { "material" } else { "legacy" })
        .collect();
    assert_eq!(names, ["material", "legacy", "material"], "sprite order stays far to near");
    assert_eq!(scene.sole_pass().blended_sprites.batches.len(), 3, "depth order is not rearranged to merge textures");
    assert!(scene.sole_pass().meshes.is_empty(), "a blend mesh is not an opaque draw");
    assert_eq!(scene.sole_pass().blended_meshes.len(), 1);
    assert_eq!(scene.sole_pass().blended_meshes[0].draw, MeshDraw::Blend { double_sided: false });
    assert_eq!(scene.stats.missing_assets, 0);
    assert_eq!(scene.stats.meshes_visible, 1);
    assert!(
        scene.sole_pass().transparent.len() >= 4,
        "the mesh at z = 0 follows the sprite at the same depth, after the farther sprites"
    );
    assert!(capture::messages().is_empty(), "a drawn blend mesh is not an unsupported-state warning");
}

#[test]
fn render_order_keeps_layer_then_depth_then_order() {
    let mut assets = AssetManager::in_memory();
    sprite_asset(&mut assets, "red", [255, 0, 0, 255]);
    let blend = assets.insert("glass", MaterialAsset::default().with_alpha_mode(AlphaMode::Blend)).unwrap();
    let mut world = world();
    let same = Transform::from_xyz(0.0, 0.0, 1.0);
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Sprite::new("red").with_material(blend).with_tint(Color::rgb(0.1, 0.0, 0.0)),
        RenderOrder::new(1, 0),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 3.0),
        Sprite::new("red").with_tint(Color::rgb(0.2, 0.0, 0.0)),
        RenderOrder::new(0, 0),
    ));
    world.spawn((
        same,
        Sprite::new("red").with_material(blend).with_tint(Color::rgb(0.3, 0.0, 0.0)),
        RenderOrder::new(0, 5),
    ));
    world.spawn((same, Sprite::new("red").with_tint(Color::rgb(0.4, 0.0, 0.0)), RenderOrder::new(0, 1)));
    let scene = extract(&mut world, &assets);
    let reds: Vec<f32> = scene.sole_pass().blended_sprites.instances.iter().map(|i| i.tint[0]).collect();
    assert_eq!(reds, [0.2, 0.4, 0.3, 0.1], "layer 0 before layer 1, far before near, lower order first");
}

#[test]
fn a_mesh_mask_uses_the_material_cutoff_and_not_the_inline_lit_flag() {
    let mut assets = AssetManager::in_memory();
    assets.insert("crate", MeshData::cuboid(Vec3::splat(1.0)).unwrap()).unwrap();
    let albedo = assets.insert("decal", Texture::solid(1, 1, [0, 255, 0, 128])).unwrap();
    let mut material =
        MaterialAsset::unlit(Color::rgba(1.0, 1.0, 1.0, 0.5)).with_alpha_mode(AlphaMode::mask(0.4).unwrap());
    material.albedo = Some(albedo);
    material.double_sided = true;
    let material = assets.insert("decal-mat", material).unwrap();
    let mut world = world();
    world.spawn((
        Transform::IDENTITY,
        Mesh::new("crate").with_material(Material::color(Color::rgb(1.0, 0.0, 0.0))).with_material_asset(material),
    ));
    let scene = extract(&mut world, &assets);
    let drawn = &scene.sole_pass().meshes[0];
    assert_eq!(drawn.draw, MeshDraw::Mask { double_sided: true });
    assert_eq!(drawn.cutoff, 0.4);
    assert!(!drawn.lit);
    assert_eq!(drawn.color, [1.0, 1.0, 1.0, 0.5]);
    assert_eq!(drawn.texture, Some(albedo));
}

#[test]
fn material_class_does_not_depend_on_the_camera_projection() {
    let mut assets = AssetManager::in_memory();
    sprite_asset(&mut assets, "hero", [255, 0, 0, 255]);
    let material = assets
        .insert("flat", MaterialAsset::default().with_alpha_mode(AlphaMode::mask(0.3).unwrap()).double_sided())
        .unwrap();
    for camera in [Camera::perspective(50.0), Camera::orthographic(8.0)] {
        let mut world = World::new();
        world.spawn((
            camera,
            Transform::from_xyz(0.0, 1.0, -6.0).with_rotation(engine_math::Quat::from_rotation_y(0.4)),
        ));
        world.spawn((Transform::IDENTITY, Sprite::new("hero").with_material(material).fixed()));
        let scene = extract(&mut world, &assets);
        assert_eq!(scene.sole_pass().mask_sprites.instances.len(), 1);
        assert_eq!(
            scene.sole_pass().mask_sprites.instances[0].shading,
            SpriteShading::Mask { cutoff: 0.3, double_sided: true }
        );
    }
}
