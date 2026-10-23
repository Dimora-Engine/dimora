//! Material-aware batch keys.
//!
//! A run shares one draw only when the pipeline, material identity, material revision, and bound
//! texture match, and the geometry is the shared sprite quad or the same mesh. Opaque and mask
//! sprites are grouped by that key. Blended runs merge only when they are already contiguous in
//! the sorted sequence. These tests call extraction and [`SpriteLayer::from_ordered`].

use engine_assets::{
    AlphaMode, AssetHandle, AssetId, AssetManager, MaterialAsset, MeshData, SamplerDesc, SpriteAsset, Texture,
    TextureFilter,
};
use engine_core::Color;
use engine_math::{Mat4, Rect, Transform, Vec2, Vec3};
use engine_renderer::{
    extract_scene, Camera, Mesh, MeshInstance, RenderOrder, RenderScene, Sprite, SpriteBlend, SpriteInstance,
    SpriteLayer, SpritePipelineKind, SpriteShading, TransparentBatch,
};
use engine_world::{propagate_transforms, World};

fn world() -> World {
    let mut world = World::new();
    world.spawn((Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -8.0)));
    world
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, Vec2::new(64.0, 64.0)).expect("camera")
}

fn sprite_texture(assets: &mut AssetManager, name: &str) -> AssetHandle<Texture> {
    let texture = assets.insert(&format!("{name}-tex"), Texture::solid(2, 1, [255, 255, 255, 255])).unwrap();
    assets.insert(name, SpriteAsset::whole_texture(texture, 2, 1).with_pixels_per_unit(1.0)).unwrap();
    texture
}

fn card() -> MeshData {
    let n = Vec3::new(0.0, 0.0, -1.0);
    let verts = [
        engine_assets::Vertex::new(Vec3::new(-0.2, -0.2, 0.0), n, [0.0, 1.0]),
        engine_assets::Vertex::new(Vec3::new(0.2, -0.2, 0.0), n, [1.0, 1.0]),
        engine_assets::Vertex::new(Vec3::new(0.2, 0.2, 0.0), n, [1.0, 0.0]),
        engine_assets::Vertex::new(Vec3::new(-0.2, 0.2, 0.0), n, [0.0, 0.0]),
    ];
    MeshData::new(verts.to_vec(), vec![0, 1, 2, 0, 2, 3])
}

fn runs(meshes: &[MeshInstance]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < meshes.len() {
        let end = MeshInstance::run_end(meshes, i);
        out.push(i..end);
        i = end;
    }
    out
}

fn sprite_at(name: &str, material: AssetHandle<MaterialAsset>, tint: Color, z: f32) -> (Transform, Sprite) {
    (
        Transform::from_xyz(0.0, 0.0, z),
        Sprite::new(name)
            .with_size(Vec2::splat(0.4))
            .with_anchor(Vec2::splat(0.5))
            .with_material(material)
            .with_tint(tint),
    )
}

#[test]
fn material_identity_groups_opaque_sprites_and_does_not_interleave_them() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    let red = assets.insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let blue = assets.insert("blue", MaterialAsset::unlit(Color::rgb(0.0, 0.0, 1.0))).unwrap();
    let mut world = world();
    for i in 0..4 {
        let material = if i % 2 == 0 { red } else { blue };
        world.spawn(sprite_at("white", material, Color::WHITE, i as f32 * 0.01));
    }
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().opaque_sprites.instances.len(), 4);
    assert_eq!(
        scene.sole_pass().opaque_sprites.batches.len(),
        2,
        "alternating materials become two groups, not four draws"
    );
    assert!(scene.sole_pass().opaque_sprites.batches.iter().all(|batch| batch.range.end - batch.range.start == 2));
    let first = scene.sole_pass().opaque_sprites.batches[0].range.clone();
    let ids: Vec<_> = scene.sole_pass().opaque_sprites.instances[first.start as usize..first.end as usize]
        .iter()
        .map(|instance| instance.material_id)
        .collect();
    assert!(ids.iter().all(|id| *id == ids[0]));
}

#[test]
fn a_material_revision_mismatch_splits_the_sprite_key() {
    let texture = AssetHandle::<Texture>::from_name("white-tex");
    let material = AssetId::from_name("paint");
    let shading = SpriteShading::Opaque { double_sided: false };
    let instance = |revision| SpriteInstance {
        model: Mat4::IDENTITY,
        uv: [0.0, 0.0, 1.0, 1.0],
        tint: [1.0, 0.0, 0.0, 1.0],
        lit: false,
        texture,
        render_target: None,
        unavailable_target: false,
        shading,
        material_id: Some(material),
        material_revision: revision,
        coplanar_level: None,
    };
    let same = SpriteLayer::from_ordered(vec![instance(3), instance(3)]);
    assert_eq!(same.batches.len(), 1);
    assert_eq!(same.batches[0].range, 0..2);

    let replaced = SpriteLayer::from_ordered(vec![instance(3), instance(4)]);
    assert_eq!(replaced.batches.len(), 2, "a different revision must not reuse the previous run");
}

#[test]
fn replacing_a_material_updates_every_sprite_and_keeps_one_batch() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    let material = assets.insert("paint", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0))).unwrap();
    let mut world = world();
    for _ in 0..6 {
        world.spawn(sprite_at("white", material, Color::WHITE, 0.0));
    }
    let first = extract(&mut world, &assets);
    let revision = first.sole_pass().opaque_sprites.instances[0].material_revision;
    assert_eq!(first.sole_pass().opaque_sprites.batches.len(), 1);
    assert!(first
        .sole_pass()
        .opaque_sprites
        .instances
        .iter()
        .all(|i| i.tint[0] == 1.0 && i.material_revision == revision));

    assets.insert("paint", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let second = extract(&mut world, &assets);
    assert!(second.sole_pass().opaque_sprites.instances[0].material_revision > revision);
    assert_eq!(second.sole_pass().opaque_sprites.batches.len(), 1);
    assert_eq!(second.sole_pass().opaque_sprites.batches[0].range, 0..6);
    assert!(second.sole_pass().opaque_sprites.instances.iter().all(|i| i.tint[1] == 1.0 && i.tint[0] == 0.0));
}

#[test]
fn legacy_sprites_batch_without_a_fabricated_material() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    sprite_texture(&mut assets, "other");
    let mut world = world();
    for _ in 0..3 {
        world.spawn((
            Transform::IDENTITY,
            Sprite::new("white").with_size(Vec2::splat(0.3)).with_blend(SpriteBlend::Cutout),
        ));
    }
    world.spawn((
        Transform::from_xyz(0.4, 0.0, 0.0),
        Sprite::new("other").with_size(Vec2::splat(0.3)).with_blend(SpriteBlend::Cutout),
    ));
    world.spawn((Transform::from_xyz(0.0, 0.4, 0.0), Sprite::new("white").with_blend(SpriteBlend::Alpha)));
    let scene = extract(&mut world, &assets);
    assert!(scene
        .sole_pass()
        .cutout_sprites
        .instances
        .iter()
        .all(|i| i.material_id.is_none() && i.material_revision == 0));
    assert_eq!(scene.sole_pass().cutout_sprites.batches.len(), 2, "two legacy textures stay two draws");
    assert_eq!(scene.sole_pass().blended_sprites.batches.len(), 1);
    assert!(scene.sole_pass().blended_sprites.instances[0].material_id.is_none());
}

#[test]
fn alpha_class_and_culling_choose_different_pipelines() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    let opaque = assets.insert("opaque", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.3).unwrap()))
        .unwrap();
    let both = assets.insert("both", MaterialAsset::unlit(Color::WHITE).double_sided()).unwrap();
    let mut world = world();
    world.spawn(sprite_at("white", opaque, Color::WHITE, 0.0));
    world.spawn(sprite_at("white", opaque, Color::WHITE, 0.1));
    world.spawn(sprite_at("white", both, Color::WHITE, 0.2));
    world.spawn(sprite_at("white", mask, Color::WHITE, 0.3));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().opaque_sprites.batches.len(), 2, "double_sided selects another opaque pipeline");
    let pipelines: Vec<_> = scene.sole_pass().opaque_sprites.batches.iter().map(|batch| batch.pipeline).collect();
    assert!(pipelines.contains(&SpritePipelineKind::OpaqueCulled));
    assert!(pipelines.contains(&SpritePipelineKind::OpaqueDouble));
    assert_eq!(scene.sole_pass().mask_sprites.batches.len(), 1);
    assert_eq!(
        scene.sole_pass().mask_sprites.instances[0].shading,
        SpriteShading::Mask { cutoff: 0.3, double_sided: false }
    );
}

#[test]
fn per_instance_tint_and_cutoff_do_not_split_one_material() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.25).unwrap()))
        .unwrap();
    let other = assets
        .insert("other", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.8).unwrap()))
        .unwrap();
    let mut world = world();
    world.spawn(sprite_at("white", mask, Color::rgb(1.0, 0.0, 0.0), 0.0));
    world.spawn(sprite_at("white", mask, Color::rgb(0.0, 1.0, 0.0), 0.1));
    world.spawn(sprite_at("white", other, Color::WHITE, 0.2));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().mask_sprites.batches.len(), 2, "a different cutoff is a different material");
    let pair = scene
        .sole_pass()
        .mask_sprites
        .batches
        .iter()
        .find(|batch| batch.range.end - batch.range.start == 2)
        .expect("the shared material is one run of two");
    let paired = &scene.sole_pass().mask_sprites.instances[pair.range.start as usize..pair.range.end as usize];
    assert_ne!(paired[0].tint, paired[1].tint);
    assert!(paired[0].shares_batch(&paired[1]));
    match paired[0].shading {
        SpriteShading::Mask { cutoff, .. } => assert_eq!(cutoff, 0.25),
        other => panic!("expected a mask, got {other:?}"),
    }
}

#[test]
fn texture_identity_splits_a_batch_and_carries_the_sampler() {
    let mut assets = AssetManager::in_memory();
    let nearest = Texture::solid(1, 1, [255, 0, 0, 255]);
    let mut linear = Texture::solid(1, 1, [0, 0, 255, 255]);
    linear.sampler =
        SamplerDesc { min_filter: TextureFilter::Linear, mag_filter: TextureFilter::Linear, ..linear.sampler };
    let red = assets.insert("red-tex", nearest).unwrap();
    let blue = assets.insert("blue-tex", linear).unwrap();
    assets.insert("red", SpriteAsset::whole_texture(red, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("blue", SpriteAsset::whole_texture(blue, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assert_ne!(assets.get(red).unwrap().sampler, assets.get(blue).unwrap().sampler);
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = world();
    world.spawn(sprite_at("red", material, Color::WHITE, 0.0));
    world.spawn(sprite_at("blue", material, Color::WHITE, 0.0));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().opaque_sprites.batches.len(), 2);
    assert_ne!(
        scene.sole_pass().opaque_sprites.batches[0].texture,
        scene.sole_pass().opaque_sprites.batches[1].texture
    );
}

#[test]
fn mesh_geometry_and_legacy_meshes_follow_the_same_run_rule() {
    let mut assets = AssetManager::in_memory();
    assets.insert("card", card()).unwrap();
    assets.insert("box", MeshData::cuboid(Vec3::splat(0.2)).unwrap()).unwrap();
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = world();
    world.spawn((Transform::from_xyz(-0.5, 0.0, 0.0), Mesh::new("box")));
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Mesh::new("box")));
    world.spawn((Transform::from_xyz(0.5, 0.0, 0.0), Mesh::new("card").with_material_asset(material)));
    world.spawn((Transform::from_xyz(0.8, 0.0, 0.0), Mesh::new("card").with_material_asset(material)));
    let scene = extract(&mut world, &assets);
    let grouped = runs(&scene.sole_pass().meshes);
    assert_eq!(grouped.len(), 2, "two legacy boxes are one run; the cards are another");
    assert!(scene.sole_pass().meshes[..2].iter().all(|mesh| mesh.material_id.is_none()));
    assert_eq!(scene.sole_pass().meshes[0].mesh, scene.sole_pass().meshes[1].mesh);
    assert!(scene.sole_pass().meshes[0].shares_batch(&scene.sole_pass().meshes[1]));
    assert!(!scene.sole_pass().meshes[1].shares_batch(&scene.sole_pass().meshes[2]));
    assert_eq!(grouped[1].end - grouped[1].start, 2);
}

#[test]
fn transparent_runs_merge_only_while_the_sorted_order_is_contiguous() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    assets.insert("card", card()).unwrap();
    let glass = assets.insert("glass", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let other = assets
        .insert("other", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let mut world = world();
    world.spawn(sprite_at("white", glass, Color::rgb(1.0, 0.0, 0.0), 4.0));
    world.spawn(sprite_at("white", glass, Color::rgb(0.0, 1.0, 0.0), 3.0));
    world.spawn((Transform::from_xyz(0.0, 0.0, 2.0), Mesh::new("card").with_material_asset(glass)));
    world.spawn(sprite_at("white", glass, Color::WHITE, 1.0));
    world.spawn(sprite_at("white", other, Color::WHITE, 0.5));
    let scene = extract(&mut world, &assets);
    assert_eq!(
        scene.sole_pass().transparent.len(),
        4,
        "two far sprites merge; the mesh and the other material split the rest"
    );
    match &scene.sole_pass().transparent[0] {
        TransparentBatch::Sprites(batch) => assert_eq!(batch.range, 0..2),
        other => panic!("expected the far sprite run, got {other:?}"),
    }
    assert!(matches!(scene.sole_pass().transparent[1], TransparentBatch::Meshes { .. }));
    assert!(matches!(scene.sole_pass().transparent[2], TransparentBatch::Sprites(_)));
    assert!(matches!(scene.sole_pass().transparent[3], TransparentBatch::Sprites(_)));
    assert_ne!(
        scene.sole_pass().blended_sprites.instances[2].material_id,
        scene.sole_pass().blended_sprites.instances[3].material_id
    );
    assert_ne!(
        scene.sole_pass().blended_sprites.instances[0].tint,
        scene.sole_pass().blended_sprites.instances[1].tint
    );
}

#[test]
fn a_transparent_sequence_is_not_regrouped_by_material() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    let red = assets
        .insert("red", MaterialAsset::unlit(Color::rgb(1.0, 0.0, 0.0)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let blue = assets
        .insert("blue", MaterialAsset::unlit(Color::rgb(0.0, 0.0, 1.0)).with_alpha_mode(AlphaMode::Blend))
        .unwrap();
    let mut world = world();
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 3.0),
        Sprite::new("white").with_material(red).with_size(Vec2::splat(0.3)),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 2.0),
        Sprite::new("white").with_material(blue).with_size(Vec2::splat(0.3)),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("white").with_material(red).with_size(Vec2::splat(0.3)),
    ));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Sprite::new("white").with_material(blue).with_size(Vec2::splat(0.3)),
    ));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().transparent.len(), 4, "alternating materials stay in depth order");
    let tints: Vec<_> = scene.sole_pass().blended_sprites.instances.iter().map(|i| i.tint[0]).collect();
    assert_eq!(tints, vec![1.0, 0.0, 1.0, 0.0]);
}

#[test]
fn equal_blend_keys_keep_extraction_order_and_one_batch() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    let glass = assets.insert("glass", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let mut world = world();
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("white").with_material(glass).with_tint(Color::rgb(0.2, 0.0, 0.0)).with_size(Vec2::splat(0.3)),
        RenderOrder { layer: 0, order: 0 },
    ));
    world.spawn((
        Transform::IDENTITY,
        Sprite::new("white").with_material(glass).with_tint(Color::rgb(0.4, 0.0, 0.0)).with_size(Vec2::splat(0.3)),
        RenderOrder { layer: 0, order: 0 },
    ));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().blended_sprites.instances.len(), 2);
    assert_eq!(scene.sole_pass().blended_sprites.batches.len(), 1);
    assert_eq!(scene.sole_pass().blended_sprites.instances[0].tint[0], 0.2);
    assert_eq!(scene.sole_pass().blended_sprites.instances[1].tint[0], 0.4);
}

#[test]
fn one_thousand_opaque_sprites_extract_as_one_batch() {
    let mut assets = AssetManager::in_memory();
    let texture = sprite_texture(&mut assets, "white");
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = world();
    for i in 0..1000 {
        let column = i % 25;
        let row = i / 25;
        let x = -0.9 + column as f32 * 0.07;
        let y = -0.9 + row as f32 * 0.04;
        let region = if i % 2 == 0 {
            Rect::new(Vec2::ZERO, Vec2::new(0.5, 1.0))
        } else {
            Rect::new(Vec2::new(0.5, 0.0), Vec2::ONE)
        };
        world.spawn((
            Transform::from_xyz(x, y, i as f32 * 0.0001),
            Sprite::new("white")
                .with_size(Vec2::splat(0.03))
                .with_anchor(Vec2::splat(0.5))
                .with_material(material)
                .with_region(region)
                .with_tint(Color::rgb(0.25 + (i % 3) as f32 * 0.25, 0.5, 1.0)),
        ));
    }
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.sprites_visible, 1000);
    assert_eq!(scene.sole_pass().opaque_sprites.instances.len(), 1000);
    assert_eq!(scene.sole_pass().opaque_sprites.batches.len(), 1);
    assert_eq!(scene.sole_pass().opaque_sprites.batches[0].range, 0..1000);
    assert_eq!(scene.sole_pass().opaque_sprites.batches[0].texture, texture);
    assert!(scene.sole_pass().opaque_sprites.instances.iter().all(|i| i.material_id == Some(material.id())));
    assert_ne!(scene.sole_pass().opaque_sprites.instances[0].uv, scene.sole_pass().opaque_sprites.instances[1].uv);
    assert_ne!(
        scene.sole_pass().opaque_sprites.instances[0].model,
        scene.sole_pass().opaque_sprites.instances[1].model
    );
}

#[test]
fn projection_does_not_change_the_batch_key() {
    let mut assets = AssetManager::in_memory();
    sprite_texture(&mut assets, "white");
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let pitch = (1.0_f32 / 2.0_f32.sqrt()).atan();
    let rotation =
        engine_math::Quat::from_rotation_y(core::f32::consts::FRAC_PI_4) * engine_math::Quat::from_rotation_x(pitch);
    let forward = Transform::from_rotation(rotation).forward();
    let cameras = [
        (Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -6.0)),
        (Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -6.0)),
        (Camera::orthographic(8.0), Transform::from_position(-forward * 8.0).with_rotation(rotation)),
    ];
    for (camera, pose) in cameras {
        let mut world = World::new();
        world.spawn((camera, pose));
        for i in 0..5 {
            world.spawn((
                Transform::from_xyz(i as f32 * 0.1, 0.0, 0.0),
                Sprite::new("white").with_size(Vec2::splat(0.2)).with_material(material),
            ));
        }
        let scene = extract(&mut world, &assets);
        assert_eq!(scene.sole_pass().opaque_sprites.batches.len(), 1);
        assert!(scene.sole_pass().transparent.is_empty());
        assert_eq!(scene.sole_pass().opaque_sprites.instances.len(), 5);
    }
}
