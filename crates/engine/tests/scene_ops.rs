//! Deferred scene operations through `Game::step`. No GPU.

use std::time::Duration;

use engine::core::FixedTimestep;
use engine::prelude::*;

fn fixed_step() -> Duration {
    FixedTimestep::new(60).step()
}

#[derive(Default)]
struct Seen {
    id: Option<SceneOpId>,
    statuses: Vec<SceneOpStatus>,
    counts: Vec<usize>,
}

#[test]
fn a_scene_request_waits_for_the_stage_flush_and_the_next_stage_sees_it() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Seen::default());
    game.add_system("queue", |world| {
        let id = world
            .commands()
            .load_scene(|builder| {
                builder.spawn((Name::new("deferred"),));
                Ok(())
            })
            .unwrap();
        let status = world.scene_operation(id).unwrap();
        assert_eq!(status, SceneOpStatus::Pending);
        assert!(world.scene_instances().unwrap().is_empty());
        let mut seen = world.resource_mut::<Seen>();
        seen.id = Some(id);
        seen.statuses.push(status);
    });
    game.add_system("same_stage", |world| {
        let id = world.resource::<Seen>().id.unwrap();
        let status = world.scene_operation(id).unwrap();
        assert_eq!(status, SceneOpStatus::Pending);
        assert!(world.find_by_name("deferred").is_empty());
        world.resource_mut::<Seen>().statuses.push(status);
    });
    game.add_to_stage(Stage::PostUpdate, "next", |world| {
        let id = world.resource::<Seen>().id.unwrap();
        let status = world.scene_operation(id).unwrap();
        let SceneOpStatus::Loaded { instance } = status else { panic!("expected a published load") };
        assert_eq!(world.scene_entity(instance, 0).unwrap().map(|entity| world.contains(entity)), Some(true));
        world.resource_mut::<Seen>().statuses.push(status);
    })
    .unwrap();

    game.step(fixed_step());
    let seen = game.world.resource::<Seen>();
    assert_eq!(seen.statuses.len(), 3);
    assert!(matches!(seen.statuses[2], SceneOpStatus::Loaded { .. }));
    let id = seen.id.unwrap();
    drop(seen);
    assert!(matches!(game.world.scene_operation(id).unwrap(), SceneOpStatus::Loaded { .. }));
    assert_eq!(game.world.resource::<Diagnostics>().command_failures, 0);
    assert!(game.world.command_failures().is_empty());
}

#[test]
fn fixed_iterations_flush_scene_commands_before_the_next_iteration() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Seen::default());
    game.add_fixed_system("queue", |world| {
        let count = world.scene_instances().unwrap().len();
        world.resource_mut::<Seen>().counts.push(count);
        if count == 0 {
            world
                .commands()
                .load_scene(|builder| {
                    builder.spawn((Name::new("tick"),));
                    Ok(())
                })
                .unwrap();
        }
    });
    game.add_fixed_system("same_iteration", |world| {
        world.resource_mut::<Seen>().counts.push(world.scene_instances().unwrap().len());
    });

    game.step(fixed_step() * 2);
    assert_eq!(game.world.resource::<Seen>().counts, vec![0, 0, 1, 1]);
    assert_eq!(game.world.find_by_name("tick").len(), 1);
}

#[test]
fn queued_scene_operations_run_in_order_and_a_failure_does_not_stop_the_queue() {
    let mut game = Game::new(GameConfig::default());
    let original = game
        .world
        .load_scene(|builder| {
            builder.spawn((Name::new("original"), Transform::IDENTITY));
            Ok(())
        })
        .unwrap();
    let entity = game.world.scene_entity(original, 0).unwrap().unwrap();
    let runtime = game.world.spawn((Name::new("runtime"), Transform::from_xyz(2.0, 0.0, 0.0)));
    game.world.set_parent(runtime, entity).unwrap();
    let other = game
        .world
        .load_scene(|builder| {
            builder.spawn((Name::new("other"),));
            Ok(())
        })
        .unwrap();
    let other_entity = game.world.scene_entity(other, 0).unwrap().unwrap();
    let gone = game.world.load_scene(|_| Ok(())).unwrap();
    game.world.unload_scene(gone).unwrap();

    game.add_system("queue", move |world| {
        let failed = world.commands().load_scene(|_| Err(SceneError::BuildRejected)).unwrap();
        let spawned = world
            .commands()
            .load_scene(|builder| {
                let root = builder.spawn((Name::new("rolled-back"),));
                builder.spawn_child(root, (Name::new("rolled-back-child"),))?;
                Err(SceneError::BuildRejected)
            })
            .unwrap();
        let first = world
            .commands()
            .load_scene(|builder| {
                builder.spawn((Name::new("first"),));
                Ok(())
            })
            .unwrap();
        let second = world
            .commands()
            .load_scene(|builder| {
                builder.spawn((Name::new("second"),));
                Ok(())
            })
            .unwrap();
        let unload = world.commands().unload_scene(original).unwrap();
        let bad = world.commands().unload_scene(gone).unwrap();
        let replaced = world
            .commands()
            .replace_scene(other, |builder| {
                builder.spawn((Name::new("replaced"),));
                Ok(())
            })
            .unwrap();
        let rejected = world.commands().replace_scene(other, |_| Err(SceneError::BuildRejected)).unwrap();
        world.commands().spawn((Name::new("command-after"),));
        world.insert_resource([failed, spawned, first, second, unload, bad, replaced, rejected]);
    });

    game.step(fixed_step());
    let ids = *game.world.resource::<[SceneOpId; 8]>();
    let failed = game.world.scene_operation(ids[0]).unwrap();
    let spawned = game.world.scene_operation(ids[1]).unwrap();
    let SceneOpStatus::Loaded { instance: first } = game.world.scene_operation(ids[2]).unwrap() else {
        panic!("first load");
    };
    let SceneOpStatus::Loaded { instance: second } = game.world.scene_operation(ids[3]).unwrap() else {
        panic!("second load");
    };
    assert!(first < second);
    assert_eq!(
        *game.world.component::<Name>(game.world.scene_entity(first, 0).unwrap().unwrap()).unwrap(),
        Name::new("first")
    );
    assert_eq!(failed, SceneOpStatus::Failed { error: SceneError::BuildRejected });
    assert_eq!(spawned, SceneOpStatus::Failed { error: SceneError::BuildRejected });
    assert!(game.world.find_by_name("rolled-back").is_empty());
    assert!(game.world.find_by_name("rolled-back-child").is_empty());
    assert_eq!(game.world.scene_operation(ids[4]).unwrap(), SceneOpStatus::Unloaded { instance: original });
    assert!(!game.world.contains(entity));
    assert!(game.world.contains(runtime));
    assert!(game.world.component::<Parent>(runtime).is_err());
    assert_eq!(
        game.world.scene_operation(ids[5]).unwrap(),
        SceneOpStatus::Failed { error: SceneError::NoSuchInstance { instance: gone } }
    );
    let SceneOpStatus::Replaced { old, new } = game.world.scene_operation(ids[6]).unwrap() else {
        panic!("replace");
    };
    assert_eq!(old, other);
    assert!(!game.world.contains(other_entity));
    assert_eq!(game.world.scene_entities(new).unwrap().unwrap().len(), 1);
    // The rejected replace was queued after the successful one, so `other` is already gone.
    assert!(matches!(
        game.world.scene_operation(ids[7]).unwrap(),
        SceneOpStatus::Failed { error: SceneError::NoSuchInstance { .. } }
    ));
    assert_eq!(game.world.find_by_name("command-after").len(), 1);
    assert!(game.world.command_failures().is_empty());
    assert_eq!(game.world.resource::<Diagnostics>().command_failures, 0);
    assert!(game.world.scene_instances().is_ok());
}

#[test]
fn a_failed_deferred_replace_keeps_the_original_when_it_is_still_published() {
    let mut game = Game::new(GameConfig::default());
    let original = game
        .world
        .load_scene(|builder| {
            builder.spawn((Name::new("stay"),));
            Ok(())
        })
        .unwrap();
    let entity = game.world.scene_entity(original, 0).unwrap().unwrap();
    let other = game
        .world
        .load_scene(|builder| {
            builder.spawn((Name::new("untouched"),));
            Ok(())
        })
        .unwrap();
    let other_entity = game.world.scene_entity(other, 0).unwrap().unwrap();
    let dead = game.world.spawn((Name::new("dead"),));
    game.world.despawn(dead).unwrap();
    game.add_system("queue", move |world| {
        let id = world
            .commands()
            .replace_scene(original, |builder| {
                builder.spawn((Name::new("temporary"),));
                Err(SceneError::BuildRejected)
            })
            .unwrap();
        world.commands().despawn(dead);
        world.insert_resource(id);
    });
    game.step(fixed_step());
    let id = *game.world.resource::<SceneOpId>();
    assert_eq!(game.world.scene_operation(id).unwrap(), SceneOpStatus::Failed { error: SceneError::BuildRejected });
    assert_eq!(game.world.scene_entity(original, 0).unwrap(), Some(entity));
    assert!(game.world.find_by_name("temporary").is_empty());
    assert_eq!(game.world.scene_entity(other, 0).unwrap(), Some(other_entity));
    assert_eq!(
        game.world.command_failures().len(),
        1,
        "the despawn is a command failure, separate from the scene result"
    );
    assert!(game.world.scene_instances().is_ok());
}
