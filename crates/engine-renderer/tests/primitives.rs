//! Generated meshes stay on the ordinary mesh path.
//!
//! Plane, cube, cuboid, sphere, and cylinder are `MeshData` assets. Two entities that share one
//! asset and one material share a mesh batch. A different asset does not. Blended meshes stay in
//! the combined transparent sequence.

use engine_assets::{AlphaMode, AssetManager, MaterialAsset, MeshData, SpriteAsset, Texture};
use engine_core::Color;
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::{extract_scene, Camera, Mesh, Sprite, TransparentBatch};
use engine_world::{propagate_transforms, World};

fn extract(world: &mut World, assets: &AssetManager) -> engine_renderer::RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, Vec2::new(64.0, 64.0)).expect("camera")
}

fn camera(world: &mut World, height: f32) {
    world.spawn((Camera::orthographic(height), Transform::from_xyz(0.0, 0.0, -30.0)));
}

#[test]
fn shared_generated_assets_batch_and_different_geometry_does_not() {
    let mut assets = AssetManager::in_memory();
    assets.insert("box", MeshData::cuboid(Vec3::new(1.0, 2.0, 0.5)).unwrap()).unwrap();
    assets.insert("ball", MeshData::sphere(0.4, 8, 4).unwrap()).unwrap();
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let mut world = World::new();
    camera(&mut world, 20.0);
    world.spawn((Transform::from_xyz(-1.0, 0.0, 0.0), Mesh::new("box").with_material_asset(material)));
    world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Mesh::new("box").with_material_asset(material)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.meshes.len(), 2);
    assert_eq!(engine_renderer::MeshInstance::run_end(&scene.meshes, 0), 2);
    assert_eq!(scene.stats.meshes_culled, 0);

    world.spawn((Transform::from_xyz(0.0, 2.0, 0.0), Mesh::new("ball").with_material_asset(material)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.meshes.len(), 3);
    let runs = {
        let mut count = 0;
        let mut i = 0;
        while i < scene.meshes.len() {
            i = engine_renderer::MeshInstance::run_end(&scene.meshes, i);
            count += 1;
        }
        count
    };
    assert_eq!(runs, 2, "the two cuboids stay one run and the sphere is the other");
}

#[test]
fn a_parent_transform_moves_a_generated_mesh_and_bounds_cull_it() {
    let mut assets = AssetManager::in_memory();
    assets.insert("box", MeshData::cube(1.0).unwrap()).unwrap();
    let mut world = World::new();
    camera(&mut world, 8.0);
    let parent = world.spawn((Transform::from_xyz(0.5, 0.0, 0.0),));
    world.spawn_child(parent, (Transform::IDENTITY, Mesh::new("box")));
    world.spawn((Transform::from_xyz(40.0, 0.0, 0.0), Mesh::new("box")));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.meshes_visible, 1);
    assert_eq!(scene.stats.meshes_culled, 1);
    let moved = scene.meshes.iter().find(|mesh| mesh.model.cols[3].x > 0.1).expect("child");
    assert!((moved.model.cols[3].x - 0.5).abs() < 1e-4);
    assert!(moved.material_id.is_none(), "an unassigned generated mesh keeps the inline material");
}

#[test]
fn shaped_scaled_and_rotated_cuboids_stay_visible() {
    let mut assets = AssetManager::in_memory();
    assets.insert("wall", MeshData::cuboid(Vec3::new(0.2, 6.0, 2.0)).unwrap()).unwrap();
    assets.insert("platform", MeshData::cuboid(Vec3::new(4.0, 0.1, 4.0)).unwrap()).unwrap();
    assets.insert("beam", MeshData::cuboid(Vec3::new(8.0, 0.4, 0.3)).unwrap()).unwrap();
    assets.insert("cube", MeshData::cube(1.0).unwrap()).unwrap();
    let mut world = World::new();
    camera(&mut world, 20.0);
    world.spawn((Transform::from_xyz(-3.0, 0.0, 0.0), Mesh::new("wall")));
    world.spawn((Transform::from_xyz(0.0, -2.0, 0.0), Mesh::new("platform")));
    world.spawn((Transform::from_xyz(0.0, 2.0, 0.0), Mesh::new("beam")));
    world.spawn((
        Transform::from_xyz(3.0, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_y(0.4))
            .with_scale(Vec3::new(0.5, 2.0, 0.25)),
        Mesh::new("cube"),
    ));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.meshes_visible, 4);
    assert_eq!(scene.stats.meshes_culled, 0);
}

#[test]
fn a_blended_cuboid_keeps_its_place_beside_a_sprite() {
    let mut assets = AssetManager::in_memory();
    let texture = assets.insert("tex", Texture::solid(1, 1, [255, 255, 255, 255])).unwrap();
    assets.insert("sprite", SpriteAsset::whole_texture(texture, 1, 1).with_pixels_per_unit(1.0)).unwrap();
    assets.insert("box", MeshData::cuboid(Vec3::splat(0.4)).unwrap()).unwrap();
    let glass = assets.insert("glass", MaterialAsset::unlit(Color::WHITE).with_alpha_mode(AlphaMode::Blend)).unwrap();
    let mut world = World::new();
    camera(&mut world, 4.0);
    world.spawn((Transform::from_xyz(0.0, 0.0, 1.0), Mesh::new("box").with_material_asset(glass)));
    world.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Sprite::new("sprite").with_size(Vec2::splat(0.5)).with_anchor(Vec2::splat(0.5)).with_material(glass),
    ));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.transparent.len(), 2);
    assert!(matches!(scene.transparent[0], TransparentBatch::Meshes { .. }), "the farther cuboid is first");
    assert!(matches!(scene.transparent[1], TransparentBatch::Sprites(_)));

    let mut swapped = World::new();
    camera(&mut swapped, 4.0);
    swapped.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Mesh::new("box").with_material_asset(glass)));
    swapped.spawn((
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite::new("sprite").with_size(Vec2::splat(0.5)).with_anchor(Vec2::splat(0.5)).with_material(glass),
    ));
    let scene = extract(&mut swapped, &assets);
    assert!(matches!(scene.transparent[0], TransparentBatch::Sprites(_)));
    assert!(matches!(scene.transparent[1], TransparentBatch::Meshes { .. }));
}

#[test]
fn three_cameras_do_not_change_a_cuboid_batch() {
    let mut assets = AssetManager::in_memory();
    assets.insert("box", MeshData::cube(0.5).unwrap()).unwrap();
    let material = assets.insert("flat", MaterialAsset::unlit(Color::WHITE)).unwrap();
    let pitch = (1.0_f32 / 2.0_f32.sqrt()).atan();
    let rotation = Quat::from_rotation_y(core::f32::consts::FRAC_PI_4) * Quat::from_rotation_x(pitch);
    let forward = Transform::from_rotation(rotation).forward();
    let views = [
        (Camera::perspective(50.0), Transform::from_xyz(0.0, 0.0, -8.0)),
        (Camera::orthographic(8.0), Transform::from_xyz(0.0, 0.0, -8.0)),
        (Camera::orthographic(12.0), Transform::from_position(-forward * 10.0).with_rotation(rotation)),
    ];
    for (camera, transform) in views {
        let mut world = World::new();
        world.spawn((camera, transform));
        world.spawn((Transform::from_xyz(-0.4, 0.0, 0.0), Mesh::new("box").with_material_asset(material)));
        world.spawn((Transform::from_xyz(0.4, 0.0, 0.0), Mesh::new("box").with_material_asset(material)));
        let scene = extract(&mut world, &assets);
        assert_eq!(scene.stats.meshes_visible, 2);
        assert_eq!(engine_renderer::MeshInstance::run_end(&scene.meshes, 0), 2);
    }
}
