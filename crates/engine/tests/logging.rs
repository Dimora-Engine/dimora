//! Logging and diagnostics around `Game::step`. This binary installs its own capturing logger.

use std::time::{Duration, Instant};

use engine::prelude::*;
use engine::renderer_wgpu::RenderError;

mod capture {
    include!("../../log_capture.rs");
}

#[test]
fn command_failures_log_once_and_stay_readable() {
    capture::begin();
    let mut game = Game::new(GameConfig::default());
    game.add_system("bad", |world| {
        let entity = world.spawn(());
        let mut commands = world.commands();
        commands.despawn(entity);
        commands.despawn(entity);
    });

    game.step(Duration::from_millis(16));
    let failures = game.world.command_failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].stage, Some("update"));
    let messages = capture::messages();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(messages[0].contains("command failed"));
    assert!(messages[0].contains("update"));
    assert_eq!(capture::records()[0].level, log::Level::Error);
    assert_eq!(game.world.resource::<Diagnostics>().command_failures, 1);
    assert_eq!(game.world.command_failures().len(), 1, "the list is still readable after step");

    capture::begin();
    game.step(Duration::ZERO);
    assert_eq!(game.world.command_failures().len(), 1, "the previous step's failure was cleared");
    assert_eq!(capture::messages().len(), 1, "the old failure is not logged again");
}

#[test]
fn a_failed_scene_operation_is_logged_once_and_is_not_a_command_failure() {
    capture::begin();
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Option::<SceneOpId>::None);
    game.add_system("bad_scene", |world| {
        if world.resource::<Option<SceneOpId>>().is_some() {
            return;
        }
        let id = world.commands().load_scene(|_| Err(SceneError::BuildRejected)).unwrap();
        *world.resource_mut::<Option<SceneOpId>>() = Some(id);
    });

    game.step(Duration::from_millis(16));
    let id = game.world.resource::<Option<SceneOpId>>().unwrap();
    assert_eq!(game.world.scene_operation(id).unwrap(), SceneOpStatus::Failed { error: SceneError::BuildRejected });
    assert!(game.world.command_failures().is_empty());
    assert_eq!(game.world.resource::<Diagnostics>().command_failures, 0);
    let messages = capture::messages();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(messages[0].contains("scene operation"));
    assert!(messages[0].contains("scene build was rejected"));
    assert_eq!(capture::records()[0].level, log::Level::Error);

    capture::begin();
    game.step(Duration::ZERO);
    assert_eq!(game.world.scene_operation(id).unwrap(), SceneOpStatus::Failed { error: SceneError::BuildRejected });
    assert!(capture::messages().is_empty(), "a retained scene failure is not logged again");
    assert!(game.world.command_failures().is_empty());
}

#[test]
fn discarded_events_are_counted_and_not_logged() {
    capture::begin();
    let mut game = Game::new(GameConfig::default());
    game.add_to_stage(Stage::InputClear, "late", |world| {
        world.send(1u8);
        world.send(2u8);
    })
    .unwrap();
    game.step(Duration::ZERO);
    let diagnostics = *game.world.resource::<Diagnostics>();
    assert_eq!(diagnostics.discarded_events, 2);
    assert!(capture::messages().is_empty(), "{:?}", capture::messages());
}

#[test]
fn stage_timings_are_cpu_time_and_fixed_steps_follow_the_cap() {
    let mut game = Game::new(GameConfig::default());
    game.world.spawn((Transform::default(),));
    game.add_system("burn", |_| {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(20) {}
    });
    game.step(Duration::from_secs(30));
    let diagnostics = *game.world.resource::<Diagnostics>();
    assert_eq!(diagnostics.frame, Duration::from_secs(30));
    assert_eq!(diagnostics.fixed_steps, 8);
    assert_eq!(diagnostics.entities, 1);
    assert!(diagnostics.stages.update >= Duration::from_millis(15), "{:?}", diagnostics.stages.update);
    assert!(diagnostics.stages.time < diagnostics.stages.update);
    assert!(diagnostics.stages.fixed_update < Duration::from_millis(500), "{:?}", diagnostics.stages.fixed_update);
    assert!(diagnostics.stages.total() < diagnostics.frame);
    assert_eq!(diagnostics.render, RenderStats::default());
}

#[test]
fn render_statistics_update_after_a_successful_render_and_survive_the_next_step() {
    let mut game = Game::new(GameConfig::default().with_size(32, 32));
    {
        let mut assets = game.world.resource_mut::<AssetManager>();
        let texture = assets.insert("white", Texture::solid(1, 1, [255, 0, 0, 255])).unwrap();
        assets.insert("white_sprite", SpriteAsset::whole_texture(texture, 1, 1)).unwrap();
    }
    game.world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), Sprite::new("white_sprite")));
    game.world.spawn((Transform::from_xyz(0.0, 0.0, -4.0), Camera::perspective(60.0)));
    game.step(Duration::from_millis(16));
    assert_eq!(game.world.resource::<Diagnostics>().render, RenderStats::default());

    match game.render_to_rgba(32, 32) {
        Ok(_) => {}
        Err(RenderError::NoAdapter(message)) if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() != Some("1") => {
            eprintln!("skipping GPU test: {message}");
            return;
        }
        Err(err) => panic!("render failed: {err}"),
    }

    let stats = *game.world.resource::<RenderStats>();
    assert_eq!(game.world.resource::<Diagnostics>().render, stats);
    assert!(stats.extract.sprites_visible >= 1, "{stats:?}");
    game.step(Duration::from_millis(16));
    assert_eq!(
        game.world.resource::<Diagnostics>().render,
        stats,
        "step leaves the previous render statistics in place"
    );
}
