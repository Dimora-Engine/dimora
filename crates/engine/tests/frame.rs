//! Stage order, fixed-step delta and input-edge lifetime. No GPU.

use std::time::Duration;

use engine::core::{FixedTimestep, InputEvent};
use engine::prelude::*;
use engine::world::ScheduleError;

fn push(label: &'static str) -> impl FnMut(&mut World) {
    move |world| world.resource_mut::<Vec<&'static str>>().push(label)
}

fn log(game: &Game) -> Vec<&'static str> {
    game.world.resource::<Vec<&str>>().clone()
}

fn fixed_step() -> Duration {
    FixedTimestep::new(60).step()
}

fn err<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(error) => error,
    }
}

#[test]
fn stages_run_in_order_and_builtins_stay_at_their_anchors() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Vec::<&'static str>::new());
    let parent = game.world.spawn((Transform::IDENTITY,));
    let child = game.world.spawn_child(parent, (Transform::from_xyz(0.0, 0.0, 1.0),));

    game.add_to_stage(Stage::Time, "after_time", |world| {
        assert_eq!(world.resource::<Time>().frame(), 1);
        world.resource_mut::<Vec<&str>>().push("after_time");
    })
    .unwrap();
    game.add_to_stage(Stage::Loads, "loads", push("loads")).unwrap();
    game.add_fixed_system("fixed", push("fixed"));
    game.add_system("update", move |world| {
        world.component_mut::<Transform>(parent).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
        world.resource_mut::<Vec<&str>>().push("update");
    });
    game.add_to_stage_before(Stage::Animate, "before_animate", Stage::ANIMATE, push("before_animate")).unwrap();
    game.add_to_stage(Stage::Animate, "after_animate", push("after_animate")).unwrap();
    game.add_to_stage(Stage::PostUpdate, "before_propagate", move |world| {
        assert!(world.component::<GlobalTransform>(child).is_err());
        world.resource_mut::<Vec<&str>>().push("before_propagate");
    })
    .unwrap();
    game.add_to_stage(Stage::InputClear, "before_clear", push("before_clear")).unwrap();

    assert_eq!(game.system_order(Stage::Time), [Stage::ADVANCE_TIME, "after_time"]);
    assert_eq!(game.system_order(Stage::Loads), [Stage::PROCESS_LOADS, "loads"]);
    assert_eq!(game.system_order(Stage::Animate), ["before_animate", Stage::ANIMATE, "after_animate"]);
    assert_eq!(game.system_order(Stage::PostUpdate), ["before_propagate", Stage::PROPAGATE_TRANSFORMS]);
    assert_eq!(game.system_order(Stage::InputClear), ["before_clear", Stage::END_INPUT_FRAME]);

    let step = fixed_step();
    game.step(step);
    assert_eq!(
        log(&game),
        [
            "after_time",
            "loads",
            "fixed",
            "update",
            "before_animate",
            "after_animate",
            "before_propagate",
            "before_clear",
        ]
    );
    let child_pos = game.world.component::<GlobalTransform>(child).unwrap().translation();
    assert!(child_pos.approx_eq(Vec3::new(5.0, 0.0, 1.0), 1e-5), "{child_pos:?}");
}

#[test]
fn fixed_systems_see_the_fixed_step_and_the_frame_delta_returns() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Vec::<Duration>::new());
    game.add_fixed_system("fixed", |world| {
        let delta = world.resource::<Time>().delta();
        world.resource_mut::<Vec<Duration>>().push(delta);
    });
    game.add_system("update", |world| {
        let delta = world.resource::<Time>().delta();
        world.resource_mut::<Vec<Duration>>().push(delta);
    });
    game.add_to_stage(Stage::Animate, "animate_delta", |world| {
        let delta = world.resource::<Time>().delta();
        world.resource_mut::<Vec<Duration>>().push(delta);
    })
    .unwrap();

    let step = fixed_step();
    let frame = step * 3;
    game.step(frame);

    let seen = game.world.resource::<Vec<Duration>>().clone();
    assert_eq!(seen, vec![step, step, step, frame, frame]);
    let time = game.world.resource::<Time>();
    assert_eq!(time.delta(), frame);
    assert_eq!(time.frame(), 1);
    assert!((time.elapsed_secs() - frame.as_secs_f32()).abs() < 1e-5);

    drop(time);
    game.step(frame);
    let time = game.world.resource::<Time>();
    assert_eq!(time.frame(), 2);
    assert_eq!(time.delta(), frame);
    assert!((time.elapsed_secs() - (frame * 2).as_secs_f32()).abs() < 1e-5);
    assert_eq!(game.world.resource::<Vec<Duration>>().len(), 10);
}

#[test]
fn a_short_frame_runs_no_fixed_step() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(0u32);
    game.add_fixed_system("fixed", |world| *world.resource_mut::<u32>() += 1);
    let dt = Duration::from_millis(1);
    game.step(dt);
    assert_eq!(*game.world.resource::<u32>(), 0);
    assert_eq!(game.world.resource::<Time>().delta(), dt);
    assert_eq!(game.world.resource::<Time>().frame(), 1);
}

#[test]
fn a_stall_runs_eight_fixed_steps_and_restores_the_frame_delta() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(0u32);
    game.add_fixed_system("fixed", |world| *world.resource_mut::<u32>() += 1);
    let stall = Duration::from_secs(10);
    game.step(stall);
    assert_eq!(*game.world.resource::<u32>(), 8);
    assert_eq!(game.world.resource::<Time>().delta(), stall);
    assert_eq!(game.world.resource::<Time>().frame(), 1);
    game.step(Duration::ZERO);
    assert_eq!(*game.world.resource::<u32>(), 8);
    assert_eq!(game.world.resource::<Time>().delta(), Duration::ZERO);
    assert_eq!(game.world.resource::<Time>().frame(), 2);
}

#[test]
fn input_edges_survive_fixed_and_update_and_clear_at_the_end() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Vec::<bool>::new());
    game.send_input(InputEvent::Key { key: Key::W, pressed: true });
    game.add_fixed_system("fixed", |world| {
        let pressed = world.resource::<Input>().key_pressed(Key::W);
        world.resource_mut::<Vec<bool>>().push(pressed);
    });
    game.add_system("update", |world| {
        let pressed = world.resource::<Input>().key_pressed(Key::W);
        world.resource_mut::<Vec<bool>>().push(pressed);
    });
    game.add_to_stage(Stage::InputClear, "before_clear", |world| {
        let pressed = world.resource::<Input>().key_pressed(Key::W);
        world.resource_mut::<Vec<bool>>().push(pressed);
    })
    .unwrap();

    game.step(fixed_step() * 2);
    assert_eq!(*game.world.resource::<Vec<bool>>(), vec![true, true, true, true]);
    let input = game.world.resource::<Input>();
    assert!(input.key_down(Key::W));
    assert!(!input.key_pressed(Key::W));
    assert!(!input.fixed_key_pressed(Key::W));
}

#[test]
fn fixed_edges_are_visible_once_and_a_zero_step_frame_keeps_them() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Vec::<&'static str>::new());
    game.add_fixed_system("first", |world| {
        let input = world.resource::<Input>();
        if input.fixed_key_pressed(Key::Space) {
            world.resource_mut::<Vec<&str>>().push("fixed-press");
        }
        if input.fixed_key_released(Key::W) {
            world.resource_mut::<Vec<&str>>().push("fixed-release");
        }
        if input.fixed_key_pressed(Key::Space) {
            world.resource_mut::<Vec<&str>>().push("fixed-press-again");
        }
    });
    game.add_fixed_system("second", |world| {
        if world.resource::<Input>().fixed_key_pressed(Key::Space) {
            world.resource_mut::<Vec<&str>>().push("second-saw-press");
        }
    });
    game.add_system("update", |world| {
        let input = world.resource::<Input>();
        if input.fixed_key_pressed(Key::Space) {
            world.resource_mut::<Vec<&str>>().push("update-fixed");
        }
        if input.key_pressed(Key::Space) {
            world.resource_mut::<Vec<&str>>().push("update-frame");
        }
    });

    game.send_input(InputEvent::Key { key: Key::Space, pressed: true });
    game.send_input(InputEvent::Key { key: Key::Space, pressed: false });
    game.send_input(InputEvent::Key { key: Key::W, pressed: true });
    game.send_input(InputEvent::Key { key: Key::W, pressed: false });
    game.step(Duration::ZERO);
    assert_eq!(log(&game), ["update-fixed", "update-frame"]);
    assert!(game.world.resource::<Input>().fixed_key_pressed(Key::Space));
    assert!(!game.world.resource::<Input>().key_pressed(Key::Space));
    assert!(!game.world.resource::<Input>().key_down(Key::W));

    game.step(fixed_step() * 2);
    assert_eq!(
        log(&game),
        ["update-fixed", "update-frame", "fixed-press", "fixed-release", "fixed-press-again", "second-saw-press"]
    );
    assert!(!game.world.resource::<Input>().fixed_key_pressed(Key::Space));
    assert!(!game.world.resource::<Input>().fixed_key_released(Key::W));
}

#[test]
fn action_edges_follow_the_fixed_latch_and_aggregate_bindings() {
    let mut game = Game::new(GameConfig::default());
    {
        let mut actions = game.world.resource_mut::<Actions>();
        actions.bind_button("jump", ButtonSource::Key(Key::Space));
        actions.bind_button("jump", ButtonSource::Key(Key::W));
        actions.bind_button("jump", ButtonSource::Mouse(MouseButton::Left));
    }
    game.world.insert_resource(Vec::<&'static str>::new());
    game.add_fixed_system("one", |world| {
        let actions = world.resource::<Actions>();
        if actions.fixed_pressed("jump") {
            world.resource_mut::<Vec<&str>>().push("press");
        }
        if actions.fixed_released("jump") {
            world.resource_mut::<Vec<&str>>().push("release");
        }
    });
    game.add_fixed_system("two", |world| {
        if world.resource::<Actions>().fixed_pressed("jump") {
            world.resource_mut::<Vec<&str>>().push("two");
        }
    });
    game.add_system("update", |world| {
        let actions = world.resource::<Actions>();
        let input = world.resource::<Input>();
        if actions.fixed_pressed("jump") {
            world.resource_mut::<Vec<&str>>().push("update-fixed");
        }
        if actions.pressed("jump") {
            world.resource_mut::<Vec<&str>>().push("update-frame");
        }
        if actions.down(&input, "jump") {
            world.resource_mut::<Vec<&str>>().push("down");
        }
    });

    game.send_input(InputEvent::Key { key: Key::Space, pressed: true });
    game.send_input(InputEvent::Key { key: Key::W, pressed: true });
    game.step(Duration::ZERO);
    assert_eq!(log(&game), ["update-fixed", "update-frame", "down"]);

    game.send_input(InputEvent::Key { key: Key::Space, pressed: false });
    game.step(fixed_step() * 2);
    assert_eq!(log(&game), ["update-fixed", "update-frame", "down", "press", "two", "down"]);
    assert!(!game.world.resource::<Actions>().fixed_pressed("jump"));
    assert!(game.world.resource::<Actions>().down(&game.world.resource::<Input>(), "jump"));

    game.send_input(InputEvent::Key { key: Key::W, pressed: false });
    game.send_input(InputEvent::MouseButton { button: MouseButton::Left, pressed: true });
    game.send_input(InputEvent::MouseButton { button: MouseButton::Left, pressed: false });
    game.step(fixed_step());
    assert!(log(&game).ends_with(&["press", "release", "two", "update-frame"]));
    let input = game.world.resource::<Input>();
    assert!(!game.world.resource::<Actions>().down(&input, "jump"));
}

fn take_log(game: &mut Game) -> Vec<&'static str> {
    std::mem::take(&mut *game.world.resource_mut::<Vec<&'static str>>())
}

fn watch_jump(world: &mut World, stage: &'static str) {
    let actions = world.resource::<Actions>();
    let fixed_press = actions.fixed_pressed("jump");
    let fixed_release = actions.fixed_released("jump");
    let frame_press = actions.pressed("jump");
    let frame_release = actions.released("jump");
    drop(actions);
    let mut log = world.resource_mut::<Vec<&'static str>>();
    if fixed_press {
        log.push(stage);
        log.push("fixed-press");
    }
    if fixed_release {
        log.push(stage);
        log.push("fixed-release");
    }
    if frame_press {
        log.push(stage);
        log.push("frame-press");
    }
    if frame_release {
        log.push(stage);
        log.push("frame-release");
    }
}

#[test]
fn zero_steps_keep_the_latch_and_rebinding_waits_until_publish() {
    let mut game = Game::new(GameConfig::default());
    game.world.resource_mut::<Actions>().bind_button("jump", ButtonSource::Key(Key::Space));
    game.world.insert_resource(Vec::<&'static str>::new());
    game.add_fixed_system("one", |world| watch_jump(world, "fixed"));
    game.add_fixed_system("two", |world| watch_jump(world, "fixed-2"));
    game.add_system("update", |world| watch_jump(world, "update"));
    game.add_to_stage(Stage::PostUpdate, "post", |world| watch_jump(world, "post")).unwrap();
    game.add_to_stage(Stage::InputClear, "clear", |world| watch_jump(world, "clear")).unwrap();

    game.send_input(InputEvent::Key { key: Key::Space, pressed: true });
    game.step(Duration::ZERO);
    assert_eq!(
        take_log(&mut game),
        [
            "update",
            "fixed-press",
            "update",
            "frame-press",
            "post",
            "fixed-press",
            "post",
            "frame-press",
            "clear",
            "fixed-press",
            "clear",
            "frame-press",
        ]
    );
    assert!(game.world.resource::<Actions>().fixed_pressed("jump"));
    assert!(!game.world.resource::<Actions>().pressed("jump"));

    game.step(Duration::ZERO);
    assert_eq!(take_log(&mut game), ["update", "fixed-press", "post", "fixed-press", "clear", "fixed-press"]);
    assert!(game.world.resource::<Actions>().fixed_pressed("jump"));
    assert!(!game.world.resource::<Actions>().pressed("jump"));
    assert!(game.world.resource::<Actions>().down(&game.world.resource::<Input>(), "jump"));

    game.step(fixed_step() * 2);
    assert_eq!(take_log(&mut game), ["fixed", "fixed-press", "fixed-2", "fixed-press"]);
    assert!(!game.world.resource::<Actions>().fixed_pressed("jump"));
    assert!(game.world.resource::<Input>().key_down(Key::Space));

    game.send_input(InputEvent::Key { key: Key::J, pressed: true });
    game.world.resource_mut::<Actions>().set_button("jump", &[ButtonSource::Key(Key::J)]);
    assert!(!game.world.resource::<Actions>().pressed("jump"));
    assert!(!game.world.resource::<Actions>().released("jump"));
    assert!(!game.world.resource::<Actions>().fixed_pressed("jump"));
    assert!(!game.world.resource::<Actions>().fixed_released("jump"));
    game.step(fixed_step());
    assert!(take_log(&mut game).is_empty());
    assert!(game.world.resource::<Actions>().down(&game.world.resource::<Input>(), "jump"));

    game.send_input(InputEvent::Key { key: Key::J, pressed: false });
    game.world.resource_mut::<Actions>().set_button("jump", &[ButtonSource::Key(Key::A)]);
    game.send_input(InputEvent::Key { key: Key::A, pressed: true });
    assert!(game.world.resource::<Actions>().pressed("jump") && game.world.resource::<Actions>().released("jump"));
    assert!(!game.world.resource::<Actions>().fixed_pressed("jump"));
    assert!(!game.world.resource::<Actions>().fixed_released("jump"));
    game.step(fixed_step() * 2);
    assert_eq!(
        take_log(&mut game),
        [
            "fixed",
            "fixed-press",
            "fixed",
            "fixed-release",
            "fixed",
            "frame-press",
            "fixed",
            "frame-release",
            "fixed-2",
            "fixed-press",
            "fixed-2",
            "fixed-release",
            "fixed-2",
            "frame-press",
            "fixed-2",
            "frame-release",
            "fixed",
            "frame-press",
            "fixed",
            "frame-release",
            "fixed-2",
            "frame-press",
            "fixed-2",
            "frame-release",
            "update",
            "frame-press",
            "update",
            "frame-release",
            "post",
            "frame-press",
            "post",
            "frame-release",
            "clear",
            "frame-press",
            "clear",
            "frame-release",
        ]
    );
}

#[test]
fn duplicate_stage_names_and_missing_targets_fail_without_changing_the_order() {
    let mut game = Game::new(GameConfig::default());
    game.add_system("player", |_| {});
    let duplicate = err(game.add_to_stage(Stage::Update, "player", |_| {}));
    assert_eq!(duplicate, ScheduleError::DuplicateName("player"));
    assert_eq!(game.system_order(Stage::Update), ["player"]);

    let missing = err(game.add_to_stage_before(Stage::Update, "early", "nobody", |_| {}));
    assert_eq!(missing, ScheduleError::MissingTarget { system: "early", target: "nobody" });
    assert_eq!(game.system_order(Stage::Update), ["player"]);
}

#[test]
fn lifecycle_builtins_stay_at_the_ends_of_their_stages() {
    let mut game = Game::new(GameConfig::default());

    let before_time = err(game.add_to_stage_before(Stage::Time, "too_early", Stage::ADVANCE_TIME, |_| {}));
    assert_eq!(before_time, ScheduleError::MustRunAfter { system: "too_early", anchor: Stage::ADVANCE_TIME });
    assert_eq!(game.system_order(Stage::Time), [Stage::ADVANCE_TIME]);

    let before_loads = err(game.add_to_stage_before(Stage::Loads, "before_loads", Stage::PROCESS_LOADS, |_| {}));
    assert_eq!(before_loads, ScheduleError::MustRunAfter { system: "before_loads", anchor: Stage::PROCESS_LOADS });
    assert_eq!(game.system_order(Stage::Loads), [Stage::PROCESS_LOADS]);

    let after_propagate =
        err(game.add_to_stage_after(Stage::PostUpdate, "too_late", Stage::PROPAGATE_TRANSFORMS, |_| {}));
    assert_eq!(
        after_propagate,
        ScheduleError::MustRunBefore { system: "too_late", anchor: Stage::PROPAGATE_TRANSFORMS }
    );
    assert_eq!(game.system_order(Stage::PostUpdate), [Stage::PROPAGATE_TRANSFORMS]);

    let after_input = err(game.add_to_stage_after(Stage::InputClear, "after_clear", Stage::END_INPUT_FRAME, |_| {}));
    assert_eq!(after_input, ScheduleError::MustRunBefore { system: "after_clear", anchor: Stage::END_INPUT_FRAME });
    assert_eq!(game.system_order(Stage::InputClear), [Stage::END_INPUT_FRAME]);

    game.add_to_stage_before(Stage::Animate, "pose", Stage::ANIMATE, |_| {}).unwrap();
    game.add_to_stage_after(Stage::Animate, "read_pose", Stage::ANIMATE, |_| {}).unwrap();
    assert_eq!(game.system_order(Stage::Animate), ["pose", Stage::ANIMATE, "read_pose"]);

    game.add_to_stage(Stage::Time, "sees_frame", |_| {}).unwrap();
    game.add_to_stage_before(Stage::Time, "still_after_time", "sees_frame", |_| {}).unwrap();
    assert_eq!(game.system_order(Stage::Time), [Stage::ADVANCE_TIME, "still_after_time", "sees_frame"]);

    game.add_to_stage(Stage::Loads, "after_assets", |_| {}).unwrap();
    game.add_to_stage_before(Stage::Loads, "still_after_loads", "after_assets", |_| {}).unwrap();
    assert_eq!(game.system_order(Stage::Loads), [Stage::PROCESS_LOADS, "still_after_loads", "after_assets"]);

    game.add_to_stage(Stage::PostUpdate, "moves", |_| {}).unwrap();
    game.add_to_stage_after(Stage::PostUpdate, "moves_later", "moves", |_| {}).unwrap();
    assert_eq!(game.system_order(Stage::PostUpdate), ["moves", "moves_later", Stage::PROPAGATE_TRANSFORMS]);

    game.add_to_stage(Stage::InputClear, "reads_edges", |_| {}).unwrap();
    game.add_to_stage_before(Stage::InputClear, "reads_earlier", "reads_edges", |_| {}).unwrap();
    assert_eq!(game.system_order(Stage::InputClear), ["reads_earlier", "reads_edges", Stage::END_INPUT_FRAME]);

    game.send_input(InputEvent::Key { key: Key::W, pressed: true });
    game.step(fixed_step());
    assert_eq!(game.world.resource::<Time>().frame(), 1);
    assert!(game.world.resource::<Input>().key_down(Key::W));
    assert!(!game.world.resource::<Input>().key_pressed(Key::W));
}
