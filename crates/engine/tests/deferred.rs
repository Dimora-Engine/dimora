//! Command flush points and event lifetime across `Game::step`. No GPU.

use std::time::Duration;

use engine::core::FixedTimestep;
use engine::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Marker;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ping(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Late(u64);

fn fixed_step() -> Duration {
    FixedTimestep::new(60).step()
}

#[test]
fn commands_flush_between_fixed_iterations_and_failures_last_until_the_next_step() {
    let mut game = Game::new(GameConfig::default());
    let dead = game.world.spawn((Name::new("gone"),));
    game.world.despawn(dead).unwrap();
    game.world.insert_resource(Vec::<usize>::new());
    game.add_fixed_system("tick", move |world| {
        let alive = world.query::<&Marker>().iter().count();
        world.resource_mut::<Vec<usize>>().push(alive);
        world.commands().spawn((Marker,));
        world.commands().despawn(dead);
    });

    game.step(fixed_step() * 3);
    assert_eq!(*game.world.resource::<Vec<usize>>(), vec![0, 1, 2]);
    assert_eq!(game.world.query::<&Marker>().iter().count(), 3);
    assert_eq!(game.world.command_failures().len(), 3);
    assert!(game.world.command_failures().iter().all(|failure| {
        failure.stage == Some(Stage::FixedUpdate.name()) && matches!(failure.error, CommandError::NoSuchEntity { .. })
    }));

    game.step(Duration::ZERO);
    assert!(game.world.command_failures().is_empty(), "the next step starts with an empty failure list");
}

#[test]
fn events_cross_stage_boundaries_and_not_fixed_iterations_or_the_next_frame() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(0u32);
    game.world.insert_resource(Vec::<(u64, Vec<Ping>, Vec<Late>)>::new());

    game.add_fixed_system("fixed", |world| {
        assert!(world.events::<Ping>().is_empty(), "fixed iterations do not see events from this stage");
        let n = *world.resource::<u32>();
        world.send(Ping(n));
        *world.resource_mut::<u32>() += 1;
    });
    game.add_system("update", |world| {
        assert!(world.events::<Late>().is_empty());
        let frame = world.resource::<Time>().frame();
        let pings = world.events::<Ping>();
        world.resource_mut::<Vec<(u64, Vec<Ping>, Vec<Late>)>>().push((frame, pings, Vec::new()));
        world.send(Late(frame));
        assert!(world.events::<Late>().is_empty(), "a send is invisible to the rest of this stage");
    });
    game.add_system("later_in_update", |world| {
        assert!(world.events::<Late>().is_empty());
    });
    game.add_to_stage(Stage::PostUpdate, "read", |world| {
        let frame = world.resource::<Time>().frame();
        let late = world.events::<Late>();
        world.resource_mut::<Vec<(u64, Vec<Ping>, Vec<Late>)>>().push((frame, Vec::new(), late));
    })
    .unwrap();
    game.add_to_stage(Stage::InputClear, "final", |world| {
        assert_eq!(world.events::<Late>(), vec![Late(world.resource::<Time>().frame())]);
        world.send(Ping(99));
    })
    .unwrap();

    game.step(fixed_step() * 3);
    game.step(fixed_step());

    let log = game.world.resource::<Vec<(u64, Vec<Ping>, Vec<Late>)>>().clone();
    assert_eq!(
        log,
        vec![
            (1, vec![Ping(0), Ping(1), Ping(2)], Vec::new()),
            (1, Vec::new(), vec![Late(1)]),
            (2, vec![Ping(3)], Vec::new()),
            (2, Vec::new(), vec![Late(2)]),
        ]
    );
    assert!(game.world.events::<Ping>().is_empty());
    assert!(game.world.events::<Late>().is_empty());
    assert_eq!(game.world.discarded_events(), 1, "the Ping sent from input_clear was not delivered");
}

#[test]
fn input_clear_events_are_counted_when_dropped_and_delivered_events_are_not() {
    let mut game = Game::new(GameConfig::default());
    game.add_system("delivered", |world| world.send(Ping(1)));
    game.add_to_stage(Stage::PostUpdate, "read", |world| {
        assert_eq!(world.events::<Ping>(), vec![Ping(1)]);
    })
    .unwrap();
    game.step(fixed_step());
    assert_eq!(game.world.discarded_events(), 0);
    assert!(game.world.events::<Ping>().is_empty());

    game.add_to_stage(Stage::InputClear, "final", |world| {
        world.send(Late(1));
        world.send(Late(2));
    })
    .unwrap();
    game.step(fixed_step());
    assert!(game.world.events::<Late>().is_empty());
    assert!(game.world.events::<Ping>().is_empty());
    assert_eq!(game.world.discarded_events(), 2);
}

#[test]
fn command_failures_stay_readable_through_the_end_of_the_step() {
    let mut game = Game::new(GameConfig::default());
    let parent = game.world.spawn((Transform::IDENTITY,));
    let child = game.world.spawn_child(parent, (Transform::IDENTITY,));
    let dead = game.world.spawn((Name::new("gone"),));
    game.world.despawn(dead).unwrap();
    game.world.insert_resource(Vec::<CommandFailure>::new());

    game.add_system("bad_parent", move |world| {
        world.commands().set_parent(parent, child);
    });
    game.add_to_stage(Stage::PostUpdate, "sees_update_failure", move |world| {
        let failures = world.command_failures();
        assert_eq!(world.component::<Parent>(parent).ok().map(|p| p.0), None);
        world.resource_mut::<Vec<CommandFailure>>().extend(failures);
    })
    .unwrap();
    game.add_to_stage(Stage::InputClear, "bad_despawn", move |world| {
        world.commands().despawn(dead);
    })
    .unwrap();

    game.step(fixed_step());
    let during = game.world.resource::<Vec<CommandFailure>>().clone();
    assert_eq!(during.len(), 1);
    assert_eq!(during[0].stage, Some(Stage::Update.name()));
    assert!(matches!(during[0].error, CommandError::Cycle { .. }));
    assert!(game.world.component::<Parent>(child).is_ok());

    let after = game.world.command_failures();
    assert_eq!(after.len(), 2);
    assert_eq!(after[1].stage, Some(Stage::InputClear.name()));
    assert!(matches!(after[1].error, CommandError::NoSuchEntity { .. }));

    game.step(Duration::ZERO);
    let second = game.world.command_failures();
    assert_eq!(second.len(), 2, "the previous step's two failures were cleared before this step recorded its own");
    assert_eq!(second[0].stage, Some(Stage::Update.name()));
    assert_eq!(second[1].stage, Some(Stage::InputClear.name()));
    let seen = game.world.resource::<Vec<CommandFailure>>();
    assert_eq!(seen.len(), 2);
    assert!(seen.iter().all(|failure| failure.stage == Some(Stage::Update.name())));
    assert_eq!(game.world.discarded_events(), 0);
}

#[test]
fn a_queued_spawn_stays_invisible_until_the_stage_ends() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Vec::<usize>::new());
    game.add_system("queue", |world| {
        world.commands().spawn((Marker,));
        world.resource_mut::<Vec<usize>>().push(world.query::<&Marker>().iter().count());
    });
    game.add_system("same_stage", |world| {
        world.resource_mut::<Vec<usize>>().push(world.query::<&Marker>().iter().count());
    });
    game.add_to_stage(Stage::Animate, "next_stage", |world| {
        world.resource_mut::<Vec<usize>>().push(world.query::<&Marker>().iter().count());
    })
    .unwrap();
    game.step(fixed_step());
    assert_eq!(*game.world.resource::<Vec<usize>>(), vec![0, 0, 1]);
}

#[test]
fn a_direct_spawn_in_update_is_visible_to_the_same_system() {
    let mut game = Game::new(GameConfig::default());
    game.add_system("play", |world| {
        let entity = world.spawn((Marker,));
        assert!(world.contains(entity));
        assert_eq!(world.query::<&Marker>().iter().count(), 1);
    });
    game.step(fixed_step());
    assert_eq!(game.world.query::<&Marker>().iter().count(), 1);
}
