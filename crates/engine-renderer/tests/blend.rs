//! Combined transparent sequence for blended sprites and blended meshes.
//!
//! The sort key is layer, then camera-space depth descending, then order, then visit order.
//! Sprites are visited before meshes, so a tie draws the sprite first. Opaque and mask meshes
//! stay out of the sequence. Projection does not choose the class.

use engine_assets::{AlphaMode, AssetManager, MaterialAsset, MeshData, Vertex};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::{
    extract_scene, Camera, Mesh, MeshDraw, MissingAssetWarnings, RenderOrder, RenderScene, Sprite, SpriteBlend,
    TransparentBatch,
};
use engine_world::{propagate_transforms, GlobalTransform, World};

fn world_with(camera: Camera, pose: Transform) -> World {
    let mut world = World::new();
    world.insert_resource(MissingAssetWarnings::default());
    world.spawn((camera, pose));
    world
}

fn world() -> World {
    world_with(Camera::perspective(60.0), Transform::from_xyz(0.0, 0.0, -8.0))
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, Vec2::new(64.0, 64.0)).expect("camera")
}

fn glass(assets: &mut AssetManager, name: &str, red: f32) -> engine_assets::AssetHandle<MaterialAsset> {
    assets.insert(name, MaterialAsset::unlit(Color::rgb(red, 0.0, 0.0)).with_alpha_mode(AlphaMode::Blend)).unwrap()
}

fn card() -> MeshData {
    let n = Vec3::new(0.0, 0.0, -1.0);
    MeshData::new(
        vec![
            Vertex::new(Vec3::new(-0.5, -0.5, 0.0), n, [0.0, 1.0]),
            Vertex::new(Vec3::new(0.5, -0.5, 0.0), n, [1.0, 1.0]),
            Vertex::new(Vec3::new(0.5, 0.5, 0.0), n, [1.0, 0.0]),
            Vertex::new(Vec3::new(-0.5, 0.5, 0.0), n, [0.0, 0.0]),
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
}

/// A card whose local-space bounding centre is at `z`, not at the entity origin.
fn slab(z: f32) -> MeshData {
    let n = Vec3::new(0.0, 0.0, -1.0);
    let at = |x, y| Vertex::new(Vec3::new(x, y, z), n, [0.0, 0.0]);
    MeshData::new(vec![at(-0.5, -0.5), at(0.5, -0.5), at(0.5, 0.5), at(-0.5, 0.5)], vec![0, 1, 2, 0, 2, 3])
}

fn prepare(assets: &mut AssetManager) {
    let texture = assets.insert("tex", engine_assets::Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    assets.insert("hero", engine_assets::SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("card", card()).unwrap();
}

/// Red channel of each transparent item, sprites then meshes within a batch, in submission order.
fn marks(scene: &RenderScene) -> Vec<f32> {
    let mut out = Vec::new();
    let mut sprites = 0;
    let mut meshes = 0;
    for batch in &scene.transparent {
        match batch {
            TransparentBatch::Sprites(batch) => {
                for index in batch.range.clone() {
                    out.push(scene.blended_sprites.instances[index as usize].tint[0]);
                    sprites += 1;
                }
            }
            TransparentBatch::Meshes { range, .. } => {
                for index in range.clone() {
                    out.push(scene.blended_meshes[index as usize].color[0]);
                    meshes += 1;
                }
            }
        }
    }
    assert_eq!(sprites, scene.blended_sprites.instances.len());
    assert_eq!(meshes, scene.blended_meshes.len());
    out
}

fn spawn_sprite(
    world: &mut World,
    z: f32,
    mark: f32,
    material: engine_assets::AssetHandle<MaterialAsset>,
    order: Option<RenderOrder>,
) {
    let sprite = Sprite::new("hero").with_material(material).with_tint(Color::rgb(mark, 0.0, 0.0));
    match order {
        Some(order) => world.spawn((Transform::from_xyz(0.0, 0.0, z), sprite, order)),
        None => world.spawn((Transform::from_xyz(0.0, 0.0, z), sprite)),
    };
}

fn spawn_mesh(
    world: &mut World,
    z: f32,
    mark: f32,
    order: Option<RenderOrder>,
    assets: &mut AssetManager,
    mesh_name: &str,
) {
    let material = glass(assets, &format!("m-{mark}"), mark);
    let mesh = Mesh::new(mesh_name).with_material_asset(material);
    match order {
        Some(order) => world.spawn((Transform::from_xyz(0.0, 0.0, z), mesh, order)),
        None => world.spawn((Transform::from_xyz(0.0, 0.0, z), mesh)),
    };
}

#[test]
fn blend_meshes_join_the_sequence_and_opaque_and_mask_meshes_do_not() {
    let mut assets = AssetManager::in_memory();
    prepare(&mut assets);
    let white = assets.insert("white", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let blend = glass(&mut assets, "blend", 0.2);
    let opaque = assets.insert("opaque", MaterialAsset::unlit(Color::rgb(0.0, 1.0, 0.0))).unwrap();
    let mask = assets
        .insert("mask", MaterialAsset::unlit(Color::rgb(0.0, 0.0, 1.0)).with_alpha_mode(AlphaMode::mask(0.4).unwrap()))
        .unwrap();
    let mut world = world();
    spawn_sprite(&mut world, 1.0, 0.4, white, None);
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 3.0),
        Sprite::new("hero").with_blend(SpriteBlend::Alpha).with_tint(Color::rgb(0.6, 0.0, 0.0)),
    ));
    world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Mesh::new("card").with_material_asset(opaque)));
    world.spawn((Transform::from_xyz(-1.0, 0.0, 0.0), Mesh::new("card").with_material_asset(mask)));
    world.spawn((Transform::from_xyz(2.0, 0.0, 2.0), Mesh::new("card").with_material_asset(blend)));

    let scene = extract(&mut world, &assets);
    assert!(scene.blended_meshes.iter().all(|mesh| matches!(mesh.draw, MeshDraw::Blend { .. })));
    assert_eq!(scene.blended_meshes.len(), 1);
    assert_eq!(scene.meshes.len(), 2, "opaque and mask stay on the depth-writing list");
    assert!(scene.meshes.iter().any(|mesh| mesh.draw == MeshDraw::Solid { double_sided: false }));
    assert!(scene.meshes.iter().any(|mesh| mesh.draw == MeshDraw::Mask { double_sided: false }));
    assert_eq!(marks(&scene), [0.6, 0.2, 0.4], "legacy sprite, blend mesh, material sprite, far to near");
    assert_eq!(scene.stats.meshes_visible, 3);
    assert_eq!(scene.stats.missing_assets, 0);
}

#[test]
fn missing_render_order_is_zero_and_mesh_render_order_uses_the_sprite_key() {
    let mut assets = AssetManager::in_memory();
    prepare(&mut assets);
    let blend = glass(&mut assets, "blend", 1.0);
    let mut world = world();
    // No RenderOrder. Same depth as the mesh below. The sprite is visited first.
    spawn_sprite(&mut world, 1.0, 0.3, blend, None);
    spawn_mesh(&mut world, 1.0, 0.5, None, &mut assets, "card");
    // Layer 1 is drawn after layer 0 even though this mesh is farther.
    spawn_mesh(&mut world, 4.0, 0.9, Some(RenderOrder::new(1, 0)), &mut assets, "card");
    // Same layer and depth as the 0.5 mesh, but a lower order, so it is drawn first among the tie.
    spawn_mesh(&mut world, 1.0, 0.1, Some(RenderOrder::new(0, -1)), &mut assets, "card");

    let scene = extract(&mut world, &assets);
    assert_eq!(marks(&scene), [0.1, 0.3, 0.5, 0.9]);
    let again = extract(&mut world, &assets);
    assert_eq!(marks(&again), marks(&scene), "equal keys stay in visit order across extractions");
}

#[test]
fn sprite_and_mesh_depths_share_view_space_and_the_bounding_centre() {
    let mut assets = AssetManager::in_memory();
    prepare(&mut assets);
    assets.insert("slab", slab(2.0)).unwrap();
    assert_eq!(assets.get(assets.mesh("slab")).unwrap().bounding_sphere().0, Vec3::new(0.0, 0.0, 2.0));
    let blend = glass(&mut assets, "blend", 1.0);
    let mut world = world();
    // Mesh spawned first. At the same world point the sprite still precedes it.
    spawn_mesh(&mut world, 1.0, 0.3, None, &mut assets, "card");
    spawn_sprite(&mut world, 4.0, 0.1, blend, None);
    spawn_sprite(&mut world, 1.0, 0.2, blend, None);
    spawn_sprite(&mut world, 0.0, 0.4, blend, None);
    world.spawn((Transform::IDENTITY, Mesh::new("slab").with_material_asset(glass(&mut assets, "offset", 0.25))));

    let scene = extract(&mut world, &assets);
    let far = scene.view.view.transform_point3(Vec3::new(0.0, 0.0, 4.0)).z;
    let mid = scene.view.view.transform_point3(Vec3::new(0.0, 0.0, 1.0)).z;
    let near = scene.view.view.transform_point3(Vec3::ZERO).z;
    assert!(far > mid && mid > near, "larger view Z is farther: {far} {mid} {near}");
    // 0.25 is the slab. Its centre is at z = 2, between the sprite at 4 and the pair at 1.
    // Using the entity origin would place that mesh nearer than the sprite at 0.
    assert_eq!(marks(&scene), [0.1, 0.25, 0.2, 0.3, 0.4]);
}

#[test]
fn parent_rotation_and_nonuniform_scale_move_the_mesh_centre() {
    let mut assets = AssetManager::in_memory();
    prepare(&mut assets);
    let blend = glass(&mut assets, "blend", 1.0);
    let mut scaled = world();
    let parent = scaled.spawn((Transform::IDENTITY.with_scale(Vec3::new(1.0, 1.0, 2.0)),));
    let material = glass(&mut assets, "scaled", 0.5);
    scaled.spawn_child(parent, (Transform::from_xyz(0.0, 0.0, 1.0), Mesh::new("card").with_material_asset(material)));
    spawn_sprite(&mut scaled, 2.5, 0.9, blend, None);
    spawn_sprite(&mut scaled, 1.5, 0.1, blend, None);
    propagate_transforms(&mut scaled);
    let child = scaled.query::<(&Mesh, &GlobalTransform)>().iter().next().unwrap().1 .1.translation().z;
    assert!((child - 2.0).abs() < 1e-4, "parent scale z=2 takes the local centre from 1 to 2, got {child}");
    assert_eq!(marks(&extract(&mut scaled, &assets)), [0.9, 0.5, 0.1]);

    let mut turned = world();
    let parent = turned.spawn((Transform::from_rotation(Quat::from_rotation_y(core::f32::consts::PI)),));
    let material = glass(&mut assets, "turned", 0.5);
    turned.spawn_child(parent, (Transform::from_xyz(0.0, 0.0, 1.0), Mesh::new("card").with_material_asset(material)));
    spawn_sprite(&mut turned, 0.0, 0.9, blend, None);
    spawn_sprite(&mut turned, -2.0, 0.1, blend, None);
    propagate_transforms(&mut turned);
    let child = turned.query::<(&Mesh, &GlobalTransform)>().iter().next().unwrap().1 .1.translation().z;
    assert!((child - -1.0).abs() < 1e-4, "a half turn about Y maps local z=1 to world z=-1, got {child}");
    // Camera at z=-8 looks along +Z, so z=0 is farther than z=-1, which is farther than z=-2.
    assert_eq!(marks(&extract(&mut turned, &assets)), [0.9, 0.5, 0.1]);
}

#[test]
fn a_material_revision_can_change_the_mesh_alpha_class() {
    let mut assets = AssetManager::in_memory();
    prepare(&mut assets);
    let material = assets.insert("surface", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = world();
    world.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(material)));

    let opaque = extract(&mut world, &assets);
    assert_eq!(opaque.meshes.len(), 1);
    assert!(opaque.blended_meshes.is_empty());
    assert_eq!(opaque.meshes[0].material_revision, assets.revision(material));

    assets.insert("surface", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let blend = extract(&mut world, &assets);
    assert!(blend.meshes.is_empty());
    assert_eq!(blend.blended_meshes.len(), 1);
    assert_eq!(blend.blended_meshes[0].draw, MeshDraw::Blend { double_sided: false });
    assert_ne!(blend.blended_meshes[0].material_revision, opaque.meshes[0].material_revision);

    assets
        .insert(
            "surface",
            MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::mask(0.3).unwrap()).double_sided(),
        )
        .unwrap();
    let mask = extract(&mut world, &assets);
    assert!(mask.transparent.is_empty());
    assert_eq!(mask.meshes[0].draw, MeshDraw::Mask { double_sided: true });
    assert_eq!(mask.meshes[0].cutoff, 0.3);
}

#[test]
fn camera_projection_does_not_classify_and_a_turn_reverses_depth_order() {
    let mut assets = AssetManager::in_memory();
    prepare(&mut assets);
    let blend = glass(&mut assets, "blend", 0.4);
    let sprite_blend =
        assets.insert("sprite-blend", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let opaque = assets.insert("solid", MaterialAsset::unlit(Color::WHITE)).unwrap();

    for camera in [Camera::perspective(60.0), Camera::orthographic(4.0)] {
        let mut world = world_with(camera, Transform::from_xyz(0.0, 0.0, -8.0));
        world.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(blend)));
        world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Mesh::new("card").with_material_asset(opaque)));
        let scene = extract(&mut world, &assets);
        assert_eq!(scene.blended_meshes.len(), 1);
        assert_eq!(scene.meshes.len(), 1);
        assert!(matches!(scene.transparent[0], TransparentBatch::Meshes { .. }));
    }

    let mut front = world_with(Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -8.0));
    spawn_mesh(&mut front, 0.0, 0.2, None, &mut assets, "card");
    spawn_sprite(&mut front, 2.0, 0.8, sprite_blend, None);
    assert_eq!(marks(&extract(&mut front, &assets)), [0.8, 0.2], "z=2 is farther when the camera looks along +Z");

    let mut back = world_with(
        Camera::orthographic(4.0),
        Transform::from_xyz(0.0, 0.0, 6.0).with_rotation(Quat::from_rotation_y(core::f32::consts::PI)),
    );
    spawn_mesh(&mut back, 0.0, 0.2, None, &mut assets, "card");
    spawn_sprite(&mut back, 2.0, 0.8, sprite_blend, None);
    assert_eq!(marks(&extract(&mut back, &assets)), [0.2, 0.8], "looking along -Z makes z=0 the farther object");
}

#[test]
fn a_mesh_that_spans_a_sprite_still_sorts_by_its_centre() {
    let mut assets = AssetManager::in_memory();
    prepare(&mut assets);
    assets.insert("wide", MeshData::cuboid(Vec3::new(1.0, 1.0, 4.0)).unwrap()).unwrap();
    let centre = assets.get(assets.mesh("wide")).unwrap().bounding_sphere().0;
    assert_eq!(centre, Vec3::ZERO, "the cuboid covers about z = -2..2 around the origin");
    let blend = glass(&mut assets, "blend", 1.0);
    let mut world = world();
    world.spawn((Transform::IDENTITY, Mesh::new("wide").with_material_asset(glass(&mut assets, "span", 0.5))));
    spawn_sprite(&mut world, -1.0, 0.2, blend, None);
    // The sprite at z=-1 is closer to the camera at z=-8 than the cuboid centre, so the sprite is
    // drawn later. The cuboid's near face is still in front of that sprite. The centre is the
    // accepted representative; this is not a per-triangle order.
    assert_eq!(marks(&extract(&mut world, &assets)), [0.5, 0.2]);
}

#[test]
fn contiguous_blend_meshes_share_a_batch_until_a_sprite_splits_them() {
    let mut assets = AssetManager::in_memory();
    prepare(&mut assets);
    // One material, so the meshes are compatible. Distinct materials would be distinct draws.
    let shared = glass(&mut assets, "shared", 0.5);
    let mut world = world();
    world.spawn((Transform::from_xyz(0.0, 0.0, 3.0), Mesh::new("card").with_material_asset(shared)));
    world.spawn((Transform::from_xyz(0.0, 0.0, 2.0), Mesh::new("card").with_material_asset(shared)));
    spawn_sprite(&mut world, 1.0, 0.8, shared, None);
    world.spawn((Transform::IDENTITY, Mesh::new("card").with_material_asset(shared)));
    let scene = extract(&mut world, &assets);
    assert_eq!(marks(&scene), [0.5, 0.5, 0.4, 0.5]);
    assert_eq!(scene.blended_meshes[0].model.cols[3].z, 3.0);
    assert_eq!(scene.blended_meshes[1].model.cols[3].z, 2.0);
    assert_eq!(scene.blended_meshes[2].model.cols[3].z, 0.0);
    assert_eq!(scene.transparent.len(), 3, "the two far meshes are one run; the near mesh is split off");
    match &scene.transparent[0] {
        TransparentBatch::Meshes { range, .. } => assert_eq!(range, &(0..2)),
        other => panic!("expected a mesh run, got {other:?}"),
    }
}
