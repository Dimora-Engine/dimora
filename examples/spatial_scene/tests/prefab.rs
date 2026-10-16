//! The documented lamp prefab: one function, a root, and a child.

mod common;

use common::*;
use engine::prelude::*;
use engine::world::propagate_transforms;
use spatial_scene::prefab::{place_lamp, spawn_lamp};
use spatial_scene::scene::Scene;

#[test]
fn one_lamp_is_a_post_with_a_light_child() {
    let mut game = game(0);
    let root = spawn_lamp(&mut game.world, Vec3::new(40.0, 0.0, 280.0));
    let light = only_child(&game.world, root);
    assert_eq!(game.world.component::<Name>(root).unwrap().0, "lamp");
    assert_eq!(game.world.component::<Name>(light).unwrap().0, "lamp light");
    assert_eq!(game.world.component::<Parent>(light).unwrap().0, root);
    assert_eq!(game.world.component::<Transform>(root).unwrap().position, Vec3::new(40.0, 100.0, 280.0));
    assert_eq!(game.world.component::<Transform>(light).unwrap().position, Vec3::new(0.0, 110.0, 0.0));
    assert!(game.world.scene_membership(root).unwrap().is_none());
    assert!(game.world.scene_membership(light).unwrap().is_none());
    propagate_transforms(&mut game.world);
    let global = game.world.component::<GlobalTransform>(light).unwrap().translation();
    assert_eq!(global, Vec3::new(40.0, 210.0, 280.0));
}

#[test]
fn two_lamps_are_distinct_hierarchies() {
    let mut game = game(0);
    let first = spawn_lamp(&mut game.world, Vec3::new(-20.0, 0.0, 260.0));
    let second = spawn_lamp(&mut game.world, Vec3::new(20.0, 0.0, 260.0));
    assert_ne!(first, second);
    let (first_light, second_light) = (only_child(&game.world, first), only_child(&game.world, second));
    assert_ne!(first_light, second_light);
    assert!(game.world.contains(first) && game.world.contains(second));
    assert_eq!(game.world.find_by_name("lamp").len(), 2);
    assert_eq!(game.world.find_by_name("lamp light").len(), 2);
}

#[test]
fn despawning_the_root_removes_the_light_and_leaves_the_other_lamp() {
    let mut game = game(0);
    let first = spawn_lamp(&mut game.world, Vec3::ZERO);
    let first_light = only_child(&game.world, first);
    let second = spawn_lamp(&mut game.world, Vec3::new(80.0, 0.0, 0.0));
    game.world.despawn(first).unwrap();
    assert!(!game.world.contains(first));
    assert!(!game.world.contains(first_light));
    assert!(game.world.contains(second));
    assert!(game.world.contains(only_child(&game.world, second)));
    assert!(!game.world.query::<&Parent>().iter().any(|(_, parent)| parent.0 == first));
}

#[test]
fn the_lamp_is_extracted_with_the_existing_scene() {
    let mut game = game(0);
    let before = game.extract().unwrap().stats;
    spawn_lamp(&mut game.world, Vec3::new(40.0, 0.0, 280.0));
    game.step(std::time::Duration::from_millis(16));
    let after = game.extract().unwrap().stats;
    assert_eq!(after.lights, before.lights + 1);
    assert!(after.meshes_visible >= before.meshes_visible);
    assert_eq!(after.missing_assets, 0);
    assert_eq!(game.world.resource::<Scene>().environment, spatial_scene::scene::Environment::A);
}

#[test]
fn a_scene_lamp_is_a_member_and_a_failed_build_removes_it() {
    let mut game = game(0);
    let runtime = spawn_lamp(&mut game.world, Vec3::new(0.0, 0.0, -40.0));
    let before = entity_count(&game);
    let instance = game
        .world
        .load_scene(|builder| {
            place_lamp(builder, Vec3::new(12.0, 0.0, 40.0))?;
            Ok(())
        })
        .unwrap();
    let members = game.world.scene_entities(instance).unwrap().unwrap();
    assert_eq!(members.len(), 2);
    let root = members[0].1;
    let light = only_child(&game.world, root);
    assert_eq!(members[1].1, light);
    assert!(game.world.scene_membership(runtime).unwrap().is_none());
    assert_eq!(entity_count(&game), before + 2);

    let rejected = game.world.load_scene(|builder| {
        place_lamp(builder, Vec3::new(0.0, 0.0, 80.0))?;
        Err(SceneError::BuildRejected)
    });
    assert_eq!(rejected, Err(SceneError::BuildRejected));
    assert_eq!(entity_count(&game), before + 2);
    assert!(game.world.contains(runtime));
    assert!(game.world.contains(root));

    game.world.unload_scene(instance).unwrap();
    assert!(!game.world.contains(root));
    assert!(!game.world.contains(light));
    assert!(game.world.contains(runtime));
    assert_eq!(game.world.scene_entities(instance).unwrap(), None);
    assert_eq!(entity_count(&game), before);
}

fn only_child(world: &World, root: Entity) -> Entity {
    let children = world.component::<Children>(root).unwrap().0.clone();
    assert_eq!(children.len(), 1);
    children[0]
}

fn entity_count(game: &Game) -> usize {
    game.world.query::<()>().iter().count()
}
