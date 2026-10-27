//! Movement through `Game::step`. No GPU.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use engine::core::FixedTimestep;
use engine::prelude::*;
use engine::renderer::PlacedCamera;

fn fixed_step() -> Duration {
    FixedTimestep::new(60).step()
}

fn body(world: &mut World) -> Entity {
    world.spawn((Transform::IDENTITY, Collider::sphere(0.3), CharacterMotor { skin: 0.0, ..CharacterMotor::default() }))
}

fn drive(entity: Entity, translation: Vec3) -> impl FnMut(&mut World) {
    move |world| {
        let _ = world.insert(entity, (MoveIntent::new(translation),));
    }
}

fn place(world: &World, entity: Entity) -> Vec3 {
    world.component::<Transform>(entity).unwrap().position
}

#[test]
fn cameras_and_visual_children_do_not_change_the_move() {
    let cameras = [
        None,
        Some(Camera::perspective(50.0)),
        Some(Camera::orthographic(12.0).with_pixel_snap(true)),
        Some(PlacedCamera::isometric(Vec3::ZERO, 16.0, 10.0).unwrap().camera),
    ];
    let bare = sample(None, false, false);
    for camera in cameras {
        assert!(sample(camera, false, false).approx_eq(bare, 1e-4), "camera changed the move");
    }
    assert!(sample(None, true, false).approx_eq(bare, 1e-4));
    assert!(sample(None, false, true).approx_eq(bare, 1e-4));
}

fn sample(camera: Option<Camera>, sprite: bool, mesh: bool) -> Vec3 {
    let mut game = Game::new(GameConfig::default());
    if let Some(camera) = camera {
        game.world.spawn((Transform::from_xyz(3.0, 4.0, -8.0), camera));
    }
    let entity = body(&mut game.world);
    if sprite {
        game.world.spawn_child(entity, (Transform::from_xyz(0.0, 1.0, 0.0), Sprite::new("hero")));
    }
    if mesh {
        game.world.spawn_child(entity, (Transform::from_xyz(0.0, 1.0, 0.0), Mesh::new("hero")));
    }
    game.world.spawn((Transform::from_xyz(2.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(0.5))));
    game.add_fixed_system("go", drive(entity, Vec3::new(5.0, 1.0, -2.0)));
    game.step(fixed_step());
    place(&game.world, entity)
}

#[test]
fn several_fixed_iterations_each_consume_one_intent() {
    let mut game = Game::new(GameConfig::default());
    let entity = body(&mut game.world);
    game.add_fixed_system("inch", drive(entity, Vec3::X));
    game.step(fixed_step() * 3);
    assert!(place(&game.world, entity).approx_eq(Vec3::new(3.0, 0.0, 0.0), 1e-4));
    let result = game.world.component::<MoveResult>(entity).unwrap();
    assert!(result.applied.approx_eq(Vec3::X, 1e-4));
    assert_eq!(result.status, MoveStatus::Completed);
    assert!(game.world.component::<MoveIntent>(entity).is_err());
}

#[test]
fn a_zero_step_drops_a_leftover_intent_and_keeps_the_old_result() {
    let mut game = Game::new(GameConfig::default());
    let entity = body(&mut game.world);
    game.world.spawn((Transform::from_xyz(1.2, 0.0, 0.0), Collider::aabb_box(Vec3::splat(0.4))));
    game.add_fixed_system("into_wall", drive(entity, Vec3::new(3.0, 0.0, 0.0)));
    game.step(fixed_step());
    let blocked = (*game.world.component::<MoveResult>(entity).unwrap()).clone();
    assert!(blocked.blocked());
    let stopped = place(&game.world, entity);

    game.world.insert(entity, (MoveIntent::new(Vec3::new(4.0, 0.0, 0.0)),)).unwrap();
    game.step(Duration::ZERO);
    assert!(game.world.component::<MoveIntent>(entity).is_err());
    assert_eq!(*game.world.component::<MoveResult>(entity).unwrap(), blocked);
    assert!(place(&game.world, entity).approx_eq(stopped, 1e-4));
    assert_eq!(game.world.resource::<CollisionWorld>().generation(), 1);
}

#[test]
fn an_intent_written_in_update_waits_and_a_zero_step_discards_it() {
    let mut game = Game::new(GameConfig::default());
    let entity = body(&mut game.world);
    let wrote = Rc::new(Cell::new(false));
    let flag = wrote.clone();
    game.add_system("after", move |world| {
        if !flag.replace(true) {
            let _ = world.insert(entity, (MoveIntent::new(Vec3::X),));
        }
    });
    game.step(fixed_step());
    assert!(place(&game.world, entity).approx_eq(Vec3::ZERO, 1e-4));
    assert!(game.world.component::<MoveIntent>(entity).is_ok());

    game.step(Duration::ZERO);
    assert!(game.world.component::<MoveIntent>(entity).is_err());
    assert!(place(&game.world, entity).approx_eq(Vec3::ZERO, 1e-4));
    assert!(game.world.component::<MoveResult>(entity).is_err());
}

#[test]
fn the_next_simulating_step_consumes_an_update_intent() {
    let mut game = Game::new(GameConfig::default());
    let entity = body(&mut game.world);
    let wrote = Rc::new(Cell::new(false));
    let flag = wrote.clone();
    game.add_system("after", move |world| {
        if !flag.replace(true) {
            let _ = world.insert(entity, (MoveIntent::new(Vec3::new(0.0, 2.0, 0.0)),));
        }
    });
    game.step(fixed_step());
    game.step(fixed_step());
    assert!(place(&game.world, entity).approx_eq(Vec3::new(0.0, 2.0, 0.0), 1e-4));
    assert!(game.world.component::<MoveIntent>(entity).is_err());
}

#[test]
fn a_deferred_spawn_is_invisible_until_the_next_iteration() {
    let mut game = Game::new(GameConfig::default());
    let entity = body(&mut game.world);
    let spawned = Rc::new(Cell::new(None));
    let slot = spawned.clone();
    game.add_fixed_system("defer_wall", move |world| {
        if slot.get().is_none() {
            let id = world.commands().spawn((Transform::from_xyz(1.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(0.5))));
            slot.set(Some(id));
        }
    });
    let step_index = Rc::new(Cell::new(0u32));
    let steps = step_index.clone();
    game.add_fixed_system("go", move |world| {
        let index = steps.get();
        steps.set(index + 1);
        let translation = if index == 0 { Vec3::new(2.0, 0.0, 0.0) } else { Vec3::new(-2.0, 0.0, 0.0) };
        let _ = world.insert(entity, (MoveIntent::new(translation),));
    });
    game.step(fixed_step());
    let wall = spawned.get().unwrap();
    assert!(game.world.resource::<CollisionWorld>().center(wall).is_none());
    assert!(place(&game.world, entity).approx_eq(Vec3::new(2.0, 0.0, 0.0), 1e-4));

    game.step(fixed_step());
    assert!(game.world.resource::<CollisionWorld>().center(wall).is_some());
    let blocked = place(&game.world, entity);
    assert!(blocked.approx_eq(Vec3::new(1.8, 0.0, 0.0), 1e-3), "second move at {blocked:?}");
    assert!(game.world.component::<MoveResult>(entity).unwrap().blocked());
}

#[test]
fn a_deferred_despawn_still_blocks_during_its_iteration() {
    let mut game = Game::new(GameConfig::default());
    let entity = body(&mut game.world);
    let wall = game.world.spawn((Transform::from_xyz(1.0, 0.0, 0.0), Collider::aabb_box(Vec3::splat(0.4))));
    game.step(fixed_step());
    assert!(game.world.resource::<CollisionWorld>().center(wall).is_some());

    let once = Rc::new(Cell::new(false));
    let flag = once.clone();
    game.add_fixed_system("remove_wall", move |world| {
        if !flag.replace(true) {
            world.commands().despawn(wall);
        }
    });
    game.add_fixed_system("go", drive(entity, Vec3::new(3.0, 0.0, 0.0)));
    game.step(fixed_step());
    let blocked = place(&game.world, entity);
    assert!(blocked.x < 1.0, "despawned wall was already gone from the snapshot, at {blocked:?}");
    assert!(game.world.component::<MoveResult>(entity).unwrap().blocked());
    assert!(!game.world.contains(wall));

    game.step(fixed_step());
    assert!(!game.world.contains(wall));
    assert!(place(&game.world, entity).x > blocked.x + 0.5);
}
