//! Tilemap chunks: culling, invalidation, transforms, and instance order.
//!
//! Tile indices are `u32`. A negative cell is not representable. A map can still sit at a negative
//! world position through its `Transform`.

use core::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use engine_assets::{AssetManager, Texture};
use engine_math::{Quat, Transform, Vec2, Vec3};
use engine_renderer::*;
use engine_world::{propagate_transforms, GlobalTransform, World};

const VIEW: Vec2 = Vec2::new(128.0, 128.0);

fn assets() -> AssetManager {
    let mut assets = AssetManager::in_memory();
    assets.insert("tiles", Texture::solid(16, 16, [255, 0, 0, 255])).unwrap();
    assets.insert("other", Texture::solid(16, 16, [0, 255, 0, 255])).unwrap();
    assets
}

fn facing(width: f32, height: f32) -> (Camera, Transform) {
    let camera = Camera::orthographic(height.max(width) * 2.0);
    let pose = Transform::from_xyz(width * 0.5, height * 0.5, -20.0);
    (camera, pose)
}

fn extract(world: &mut World, assets: &AssetManager) -> RenderScene {
    propagate_transforms(world);
    extract_scene(world, assets, VIEW).expect("camera")
}

fn dense(width: u32, height: u32) -> TileMap {
    TileMap::new("tiles", 4, 4, Vec2::ONE, width, height).filled_with(|_, _| 0)
}

fn origins(scene: &RenderScene) -> Vec<Vec3> {
    scene.sole_pass().cutout_sprites.instances.iter().map(|instance| instance.model.translation()).collect()
}

#[test]
fn a_fully_visible_map_emits_every_tile_in_row_major_order() {
    let assets = assets();
    let mut world = World::new();
    let (camera, pose) = facing(4.0, 3.0);
    world.spawn((camera, pose));
    world.spawn((Transform::IDENTITY, dense(4, 3)));
    let scene = extract(&mut world, &assets);
    let got = origins(&scene);
    let mut expected = Vec::new();
    for y in 0..3 {
        for x in 0..4 {
            expected.push(Vec3::new(x as f32, y as f32, 0.0));
        }
    }
    assert_eq!(got, expected);
    assert_eq!(scene.stats.sprites_visible, 12);
    assert_eq!(scene.stats.tiles_visited, 12);
    assert_eq!(scene.stats.chunks_rejected, 0);
    assert_eq!(scene.sole_pass().cutout_sprites.batches.len(), 1);
    assert!(scene.sole_pass().cutout_sprites.instances.iter().all(|tile| tile.coplanar_level.is_none()));
}

#[test]
fn coplanar_priority_on_the_entity_does_not_reach_tile_instances() {
    let assets = assets();
    let mut world = World::new();
    let (camera, pose) = facing(1.0, 1.0);
    world.spawn((camera, pose));
    world.spawn((Transform::IDENTITY, dense(1, 1), CoplanarPriority::new(4).unwrap()));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.sole_pass().cutout_sprites.instances[0].coplanar_level, None);
}

#[test]
fn a_partial_edge_chunk_and_an_offscreen_map_visit_only_visible_cells() {
    let edge = TileMap::CHUNK_EDGE;
    let width = edge + 3;
    let height = 2;
    let assets = assets();
    let mut seen = World::new();
    let (camera, pose) = facing(width as f32, height as f32);
    seen.spawn((camera, pose));
    seen.spawn((Transform::IDENTITY, dense(width, height)));
    let scene = extract(&mut seen, &assets);
    assert_eq!(scene.stats.tiles_visited, width * height);
    assert_eq!(scene.stats.sprites_visible, width * height);
    assert_eq!(scene.stats.chunks_tested, 2);
    assert_eq!(scene.stats.chunks_rejected, 0);

    let mut missed = World::new();
    missed.spawn((Camera::orthographic(4.0), Transform::from_xyz(10_000.0, 0.0, -20.0)));
    missed.spawn((Transform::IDENTITY, dense(width, height)));
    let scene = extract(&mut missed, &assets);
    assert_eq!(scene.stats.tiles_visited, 0);
    assert_eq!(scene.stats.sprites_visible, 0);
    assert_eq!(scene.stats.sprites_culled, width * height);
    assert_eq!(scene.stats.chunks_rejected, scene.stats.chunks_tested);
}

#[test]
fn an_empty_chunk_outside_the_view_adds_no_culled_tiles() {
    let edge = TileMap::CHUNK_EDGE;
    let mut map = TileMap::new("tiles", 1, 1, Vec2::ONE, edge * 2, edge);
    for y in 0..edge {
        for x in 0..edge {
            map.set(x, y, 0);
        }
    }
    let assets = assets();
    let mut world = World::new();
    let center = edge as f32 * 0.25;
    world.spawn((Camera::orthographic(edge as f32 * 0.4), Transform::from_xyz(center, center, -30.0)));
    world.spawn((Transform::IDENTITY, map));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.chunks_tested, 2);
    assert_eq!(scene.stats.chunks_rejected, 1);
    assert_eq!(scene.stats.tiles_visited, edge * edge);
    assert!(scene.stats.sprites_visible > 0);
    assert!(scene.stats.sprites_visible < edge * edge);
    assert_eq!(scene.sole_pass().cutout_sprites.instances.len(), scene.stats.sprites_visible as usize);
}

#[test]
fn changing_one_tile_rebuilds_one_chunk_and_a_second_camera_does_not_rebuild_it_again() {
    let edge = TileMap::CHUNK_EDGE;
    let assets = assets();
    let mut world = World::new();
    let (camera, pose) = facing(edge as f32 * 2.0, edge as f32);
    world.spawn((camera, pose));
    world.spawn((Camera { order: 1, ..camera }, Transform::from_xyz(pose.position.x, pose.position.y, -40.0)));
    world.spawn((Transform::IDENTITY, dense(edge * 2, edge)));
    let first = extract(&mut world, &assets);
    assert_eq!(first.stats.chunks_rebuilt, 2, "the first frame scans every chunk once");
    assert_eq!(first.stats.sprites_visible, edge * edge * 2 * 2);
    let settled = extract(&mut world, &assets);
    assert_eq!(settled.stats.chunks_rebuilt, 0);

    let map_entity = world.query::<&TileMap>().iter().next().unwrap().0;
    world.component_mut::<TileMap>(map_entity).unwrap().set(1, 1, 3);
    let edited = extract(&mut world, &assets);
    assert_eq!(edited.stats.chunks_rebuilt, 1);
    assert_eq!(edited.stats.sprites_visible, edge * edge * 2 * 2);
    assert_eq!(edited.passes.len(), 2);
}

#[test]
fn a_direct_tile_write_is_visible_immediately_and_invalidate_refreshes_rejected_counts() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((Camera::orthographic(8.0), Transform::from_xyz(1.0, 1.0, -20.0)));
    world.spawn((Camera { order: 1, ..Camera::orthographic(2.0) }, Transform::from_xyz(10_000.0, 0.0, -20.0)));
    let map_entity = world.spawn((Transform::IDENTITY, dense(4, 4)));
    let _ = extract(&mut world, &assets);
    assert_eq!(extract(&mut world, &assets).stats.chunks_rebuilt, 0);

    world.component_mut::<TileMap>(map_entity).unwrap().tiles[0] = TileMap::EMPTY;
    let stale = extract(&mut world, &assets);
    assert_eq!(stale.stats.chunks_rebuilt, 0);
    assert_eq!(stale.passes[0].stats.sprites_visible, 15, "the visible camera reads the live cells");
    assert_eq!(stale.passes[1].stats.sprites_culled, 16, "a rejected chunk still has the previous occupancy");

    world.component_mut::<TileMap>(map_entity).unwrap().invalidate();
    let fresh = extract(&mut world, &assets);
    assert_eq!(fresh.stats.chunks_rebuilt, fresh.stats.chunks_tested / 2);
    assert_eq!(fresh.passes[0].stats.sprites_visible, 15);
    assert_eq!(fresh.passes[1].stats.sprites_culled, 15);
}

#[test]
fn resizing_the_grid_rebuilds_chunks_without_a_separate_cache() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((Camera::orthographic(40.0), Transform::from_xyz(4.0, 4.0, -20.0)));
    let entity = world.spawn((Transform::IDENTITY, dense(2, 2)));
    let _ = extract(&mut world, &assets);
    {
        let mut map = world.component_mut::<TileMap>(entity).unwrap();
        map.width = 3;
        map.height = 1;
        map.tiles = vec![0, TileMap::EMPTY, 2];
    }
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.sprites_visible, 2);
    assert_eq!(scene.stats.tiles_visited, 3);
    assert!(scene.stats.chunks_rebuilt >= 1);
}

#[test]
fn parent_translation_rotation_and_scale_move_the_tile_corners() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((Camera::orthographic(80.0), Transform::from_xyz(0.0, 0.0, -30.0)));
    let parent = world.spawn((Transform::from_xyz(5.0, -4.0, 2.0).with_scale(Vec3::new(2.0, 3.0, 1.0)),));
    let child = world.spawn_child(
        parent,
        (Transform::from_rotation(Quat::from_rotation_z(FRAC_PI_2)).with_scale(Vec3::new(1.0, 1.0, 1.0)), dense(2, 2)),
    );
    let scene = extract(&mut world, &assets);
    let global = *world.component::<GlobalTransform>(child).unwrap();
    let expected = global.matrix.transform_point3(Vec3::new(1.0, 0.0, 0.0));
    let origins = origins(&scene);
    assert!(
        origins.iter().any(|origin| origin.approx_eq(expected, 1e-4)),
        "tile (1, 0) should land at {expected:?}, got {origins:?}"
    );
    assert_eq!(scene.stats.sprites_visible, 4);
}

#[test]
fn a_negative_world_position_and_a_ground_rotation_stay_visible() {
    let assets = assets();
    let mut world = World::new();
    let pose = Transform::from_xyz(-30.0, 12.0, -10.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2));
    world.spawn((
        Camera::orthographic(30.0),
        Transform::looking_at(Vec3::new(-20.0, 25.0, -10.0), Vec3::new(-20.0, 0.0, -10.0), Vec3::Z),
    ));
    let entity = world.spawn((pose, dense(4, 4)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.sprites_visible, 16);
    let global = *world.component::<GlobalTransform>(entity).unwrap();
    let expected = global.matrix.transform_point3(Vec3::new(3.0, 2.0, 0.0));
    assert!(origins(&scene).iter().any(|origin| origin.approx_eq(expected, 1e-3)));
    assert!(expected.x < 0.0, "the tile sits in negative X: {expected:?}");
    assert!(
        (expected.y - pose.position.y).abs() < 1e-3,
        "rotation about X lays the tile on the entity's height: {expected:?}"
    );
}

#[test]
fn yaw_and_non_uniform_scale_do_not_drop_a_visible_chunk() {
    let assets = assets();
    let mut world = World::new();
    let pose = Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_4)).with_scale(Vec3::new(2.0, 0.5, 4.0));
    world.spawn((Camera::orthographic(400.0), Transform::from_xyz(0.0, 0.0, -80.0)));
    let entity = world.spawn((pose, dense(TileMap::CHUNK_EDGE, 2)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.stats.chunks_rejected, 0);
    assert_eq!(scene.stats.sprites_visible, TileMap::CHUNK_EDGE * 2);
    let global = *world.component::<GlobalTransform>(entity).unwrap();
    let expected = global.matrix.transform_point3(Vec3::ZERO);
    assert!(origins(&scene).iter().any(|origin| origin.approx_eq(expected, 1e-3)));
}

#[test]
fn emitted_tiles_stay_in_row_major_order_when_another_chunk_is_rejected() {
    let edge = TileMap::CHUNK_EDGE;
    let assets = assets();
    let mut world = World::new();
    let center = edge as f32 * 0.25;
    world.spawn((Camera::orthographic(edge as f32 * 0.4), Transform::from_xyz(center, center, -30.0)));
    world.spawn((Transform::IDENTITY, dense(edge * 2, edge)));
    let scene = extract(&mut world, &assets);
    let mut previous: Option<(i32, i32)> = None;
    for origin in origins(&scene) {
        let cell = (origin.x.round() as i32, origin.y.round() as i32);
        if let Some(before) = previous {
            assert!(cell.1 > before.1 || (cell.1 == before.1 && cell.0 > before.0), "{before:?} then {cell:?}");
        }
        previous = Some(cell);
    }
    assert!(previous.is_some());
}

#[test]
fn two_textures_are_two_batches_and_one_texture_stays_one() {
    let assets = assets();
    let mut world = World::new();
    world.spawn((Camera::orthographic(20.0), Transform::from_xyz(1.0, 1.0, -10.0)));
    world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), dense(2, 2)));
    world.spawn((Transform::from_xyz(4.0, 0.0, 0.0), dense(2, 2)));
    let same = extract(&mut world, &assets);
    assert_eq!(same.sole_pass().cutout_sprites.batches.len(), 1);

    let mut split = World::new();
    split.spawn((Camera::orthographic(20.0), Transform::from_xyz(1.0, 1.0, -10.0)));
    split.spawn((Transform::IDENTITY, dense(2, 2)));
    let mut other = dense(2, 2);
    other.texture = engine_assets::AssetHandle::from_name("other");
    split.spawn((Transform::from_xyz(4.0, 0.0, 0.0), other));
    let scene = extract(&mut split, &assets);
    assert_eq!(scene.sole_pass().cutout_sprites.batches.len(), 2);
    assert_eq!(scene.stats.sprites_visible, 8);
}

#[test]
fn an_offscreen_camera_culls_the_map_with_its_own_frustum() {
    let assets = assets();
    let mut world = World::new();
    world.insert_resource(RenderTargetStore::new());
    let target = world.resource_mut::<RenderTargetStore>().create(64, 64).unwrap();
    world.spawn((
        Camera::orthographic(8.0).with_output(CameraOutput::Target(target)),
        Transform::from_xyz(1.0, 1.0, -10.0),
    ));
    world.spawn((
        Camera { order: 1, ..Camera::orthographic(2.0).with_output(CameraOutput::Surface) },
        Transform::from_xyz(10_000.0, 0.0, -10.0),
    ));
    world.spawn((Transform::IDENTITY, dense(2, 2)));
    let scene = extract(&mut world, &assets);
    assert_eq!(scene.passes.len(), 2);
    assert_eq!(scene.passes[0].stats.sprites_visible, 4);
    assert_eq!(scene.passes[1].stats.sprites_visible, 0);
    assert_eq!(scene.passes[1].stats.sprites_culled, 4);
    assert_eq!(scene.stats.chunks_rebuilt, scene.passes[0].stats.chunks_rebuilt);
    assert_eq!(scene.passes[1].stats.chunks_rebuilt, 0);
}
