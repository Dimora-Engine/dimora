//! Runtime replacement of the two `spatial_scene` environments through the public scene APIs.
//!
//! The default world is environment A. Tab queues one deferred replacement. The flush at the end of
//! `update` runs it, and `post_update` records the result. The camera and the walker are not scene
//! members.

mod common;

#[allow(dead_code)]
mod capture {
    include!("../../../crates/log_capture.rs");
}

use common::*;
use engine::core::InputEvent;
use engine::prelude::*;
use spatial_scene::cameras;
use spatial_scene::scene::{self, Environment, Scene, WALK_Z};
use std::time::Duration;

const STEP: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Default)]
struct StageSeen {
    update_status: Option<SceneOpStatus>,
    update_pillar: bool,
    update_yard: bool,
    update_instance: Option<SceneInstanceId>,
    animate_status: Option<SceneOpStatus>,
    animate_pillar: bool,
    animate_yard: bool,
    animate_instance: Option<SceneInstanceId>,
}

#[derive(Clone, Copy, Default)]
struct FailureSeen {
    status: Option<SceneOpStatus>,
}

fn install(game: &mut Game) {
    game.world.resource_mut::<Actions>().bind_button(scene::SWITCH_ENVIRONMENT, ButtonSource::Key(Key::Tab));
    game.add_system("switch_environment", scene::request_environment_switch);
    game.add_to_stage(Stage::PostUpdate, "apply_environment", scene::apply_environment_switch).unwrap();
}

fn press_tab(game: &mut Game) {
    game.send_input(InputEvent::Key { key: Key::Tab, pressed: true });
}

fn release_tab(game: &mut Game) {
    game.send_input(InputEvent::Key { key: Key::Tab, pressed: false });
}

fn has(game: &Game, name: &str) -> bool {
    !game.world.find_by_name(name).is_empty()
}

fn count_named(game: &Game, name: &str) -> usize {
    game.world.find_by_name(name).len()
}

fn members(game: &Game, instance: SceneInstanceId) -> usize {
    game.world.query::<&SceneMember>().iter().filter(|(_, member)| member.instance == instance).count()
}

fn entity_count(game: &Game) -> usize {
    game.world.query::<()>().iter().count()
}

fn note_update(world: &mut World) {
    let (pending, instance) = {
        let scene = world.resource::<Scene>();
        (scene.pending, scene.instance)
    };
    let status = pending.map(|operation| world.scene_operation(operation).expect("queued operation"));
    let pillar = !world.find_by_name("pillar").is_empty();
    let yard = !world.find_by_name("yard cube").is_empty();
    let mut seen = world.resource_mut::<StageSeen>();
    seen.update_status = status;
    seen.update_pillar = pillar;
    seen.update_yard = yard;
    seen.update_instance = Some(instance);
}

fn note_animate(world: &mut World) {
    let (pending, instance) = {
        let scene = world.resource::<Scene>();
        (scene.pending, scene.instance)
    };
    let status = pending.map(|operation| world.scene_operation(operation).expect("queued operation"));
    let pillar = !world.find_by_name("pillar").is_empty();
    let yard = !world.find_by_name("yard cube").is_empty();
    let mut seen = world.resource_mut::<StageSeen>();
    seen.animate_status = status;
    seen.animate_pillar = pillar;
    seen.animate_yard = yard;
    seen.animate_instance = Some(instance);
}

fn note_failure(world: &mut World) {
    let pending = world.resource::<Scene>().pending;
    let status = pending.map(|operation| world.scene_operation(operation).expect("queued operation"));
    world.resource_mut::<FailureSeen>().status = status;
}

#[test]
fn the_initial_scene_is_environment_a_without_making_the_camera_or_walker_members() {
    let game = game(0);
    let scene = game.world.resource::<Scene>();
    let (camera, walker, instance) = (scene.camera, scene.walker, scene.instance);
    assert_eq!(scene.environment, Environment::A);
    assert!(scene.pending.is_none());
    assert!(scene.cube.is_some());
    drop(scene);
    assert!(has(&game, "figure A") && has(&game, "pillar") && has(&game, "cube") && has(&game, "tree 0"));
    assert!(!has(&game, "yard cube") && !has(&game, "yard light"));
    assert!(members(&game, instance) > 400);
    assert_eq!(members(&game, instance) + 2, entity_count(&game), "only the camera and walker sit outside A");
    assert!(game.world.scene_membership(camera).unwrap().is_none());
    assert!(game.world.scene_membership(walker).unwrap().is_none());
    assert!(game.world.component::<Transform>(camera).is_ok());
    assert!(game.world.component::<Transform>(walker).is_ok());
    assert_eq!(count_named(&game, "camera"), 1);
    assert_eq!(count_named(&game, "walker"), 1);
}

#[test]
fn a_queued_replacement_stays_pending_until_flush_and_the_result_is_dismissed() {
    let mut game = game(0);
    install(&mut game);
    let scene = game.world.resource::<Scene>();
    let (instance, camera, walker) = (scene.instance, scene.camera, scene.walker);
    drop(scene);
    let operation = game.world.commands().replace_scene(instance, scene::build_environment_b).unwrap();
    game.world.resource_mut::<Scene>().pending = Some(operation);
    assert_eq!(game.world.scene_operation(operation).unwrap(), SceneOpStatus::Pending);
    assert_eq!(game.world.scene_operation(operation).unwrap(), SceneOpStatus::Pending);
    assert!(has(&game, "pillar"));
    assert!(!has(&game, "yard cube"));
    assert_eq!(game.world.resource::<Scene>().instance, instance);

    game.step(STEP);

    let scene = game.world.resource::<Scene>();
    assert_eq!(scene.environment, Environment::B);
    assert_ne!(scene.instance, instance);
    assert!(scene.pending.is_none());
    let replacement = scene.instance;
    drop(scene);
    assert_eq!(game.world.scene_operation(operation), Err(SceneError::NoSuchOperation { operation }));
    assert_eq!(game.world.dismiss_scene_operation(operation), Err(SceneError::NoSuchOperation { operation }));
    assert_eq!(members(&game, instance), 0);
    assert!(members(&game, replacement) > 0);
    assert!(!has(&game, "pillar") && !has(&game, "figure A") && !has(&game, "cube"));
    assert!(has(&game, "yard cube") && has(&game, "yard figure") && has(&game, "yard light"));
    assert!(game.world.contains(camera) && game.world.contains(walker));
    assert!(game.world.scene_membership(camera).unwrap().is_none());
    assert!(game.world.scene_membership(walker).unwrap().is_none());
    assert!(game.world.component::<Transform>(walker).is_ok());
    assert!(game.world.command_failures().is_empty());
}

#[test]
fn the_same_stage_still_sees_the_old_scene_and_the_next_stage_sees_the_replacement() {
    let mut game = game(0);
    install(&mut game);
    game.world.insert_resource(StageSeen::default());
    game.add_system("note_switch_update", note_update);
    game.add_to_stage(Stage::Animate, "note_switch_animate", note_animate).unwrap();
    let before = game.world.resource::<Scene>();
    let (instance, camera, walker, rig) = (before.instance, before.camera, before.walker, before.rig);
    drop(before);
    let entities = entity_count(&game);
    assert!(game.world.resource::<Scene>().pending.is_none());

    press_tab(&mut game);
    game.step(STEP);

    let seen = *game.world.resource::<StageSeen>();
    assert_eq!(seen.update_status, Some(SceneOpStatus::Pending));
    assert!(seen.update_pillar && !seen.update_yard);
    assert_eq!(seen.update_instance, Some(instance));
    assert!(matches!(seen.animate_status, Some(SceneOpStatus::Replaced { old, .. }) if old == instance));
    assert!(!seen.animate_pillar && seen.animate_yard);
    assert_eq!(seen.animate_instance, Some(instance), "the tracked id waits for the result to be applied");

    let scene = game.world.resource::<Scene>();
    assert_eq!(scene.environment, Environment::B);
    assert_ne!(scene.instance, instance);
    assert!(scene.pending.is_none());
    assert_eq!((scene.camera, scene.walker), (camera, walker));
    assert_eq!((scene.rig.position, scene.rig.yaw, scene.rig.pitch), (rig.position, rig.yaw, rig.pitch));
    let replacement = scene.instance;
    drop(scene);
    assert_eq!(members(&game, instance), 0);
    assert_eq!(members(&game, replacement), entity_count(&game) - 2);
    assert!(game.world.resource::<Scene>().pending.is_none());

    release_tab(&mut game);
    game.step(STEP);
    press_tab(&mut game);
    game.step(STEP);
    let scene = game.world.resource::<Scene>();
    assert_eq!(scene.environment, Environment::A);
    assert_ne!(scene.instance, replacement);
    assert_eq!((scene.camera, scene.walker), (camera, walker));
    let restored = scene.instance;
    drop(scene);
    assert_eq!(members(&game, replacement), 0);
    assert_eq!(members(&game, instance), 0);
    assert!(members(&game, restored) > 400);
    assert!(has(&game, "pillar") && has(&game, "figure A"));
    assert!(!has(&game, "yard cube"));
    assert_eq!(entity_count(&game), entities);
    assert_eq!(count_named(&game, "camera"), 1);
    assert_eq!(count_named(&game, "walker"), 1);
}

#[test]
fn tab_switches_once_per_press_and_a_held_key_does_not_switch_again() {
    let mut game = game(0);
    install(&mut game);
    let camera = game.world.resource::<Scene>().camera;
    let start = *game.world.component::<Transform>(game.world.resource::<Scene>().walker).unwrap();

    press_tab(&mut game);
    game.step(STEP);
    let on_b = game.world.resource::<Scene>().instance;
    assert_eq!(game.world.resource::<Scene>().environment, Environment::B);
    game.step(STEP);
    game.step(STEP);
    assert_eq!(game.world.resource::<Scene>().instance, on_b);
    assert_eq!(game.world.resource::<Scene>().environment, Environment::B);
    assert!(game.world.resource::<Scene>().pending.is_none());

    release_tab(&mut game);
    game.step(STEP);
    assert_eq!(game.world.resource::<Scene>().instance, on_b);

    press_tab(&mut game);
    game.step(STEP);
    assert_eq!(game.world.resource::<Scene>().environment, Environment::A);
    assert_ne!(game.world.resource::<Scene>().instance, on_b);
    assert_eq!(game.world.resource::<Scene>().camera, camera);
    let walker = *game.world.component::<Transform>(game.world.resource::<Scene>().walker).unwrap();
    assert_eq!(walker.position.z, WALK_Z);
    assert_ne!(walker.position.x, start.position.x);
    assert!(game.world.command_failures().is_empty());
}

#[test]
fn requesting_the_switch_twice_in_one_press_queues_one_operation() {
    let mut game = game(0);
    game.world.resource_mut::<Actions>().bind_button(scene::SWITCH_ENVIRONMENT, ButtonSource::Key(Key::Tab));
    game.add_system("switch_twice", request_twice);
    press_tab(&mut game);
    game.step(STEP);
    let scene = game.world.resource::<Scene>();
    let operation = scene.pending.expect("the first request stored its id");
    assert_eq!(scene.environment, Environment::A, "nothing applies the result in this test");
    drop(scene);
    assert!(matches!(game.world.scene_operation(operation).unwrap(), SceneOpStatus::Replaced { .. }));
    assert!(has(&game, "yard cube"));
    assert!(!has(&game, "pillar"));
    assert_eq!(game.world.clear_scene_operations().unwrap(), 1);
    assert_eq!(game.world.clear_scene_operations().unwrap(), 0);
    assert_eq!(game.world.scene_operation(operation), Err(SceneError::NoSuchOperation { operation }));
}

fn request_twice(world: &mut World) {
    scene::request_environment_switch(world);
    scene::request_environment_switch(world);
}

#[test]
fn repeated_switches_do_not_accumulate_entities_or_asset_registrations() {
    let mut game = game(0);
    install(&mut game);
    let assets = game.world.resource::<AssetManager>();
    let figure = assets.sprite("figure");
    let panel = assets.texture("panel");
    let cube = assets.mesh("cube");
    let (figure_rev, panel_rev, cube_rev) = (assets.revision(figure), assets.revision(panel), assets.revision(cube));
    drop(assets);
    let entities = entity_count(&game);
    let members_a = members(&game, game.world.resource::<Scene>().instance);

    for expected in [Environment::B, Environment::A, Environment::B, Environment::A] {
        release_tab(&mut game);
        game.step(STEP);
        press_tab(&mut game);
        game.step(STEP);
        assert_eq!(game.world.resource::<Scene>().environment, expected);
        assert!(game.world.resource::<Scene>().pending.is_none());
    }
    assert_eq!(entity_count(&game), entities);
    assert_eq!(members(&game, game.world.resource::<Scene>().instance), members_a);
    assert_eq!(count_named(&game, "camera"), 1);
    assert_eq!(count_named(&game, "walker"), 1);
    let assets = game.world.resource::<AssetManager>();
    assert_eq!(assets.revision(figure), figure_rev);
    assert_eq!(assets.revision(panel), panel_rev);
    assert_eq!(assets.revision(cube), cube_rev);
    assert!(assets.get(figure).is_some());
    assert!(assets.get(panel).is_some());
    assert!(assets.get(cube).is_some());
    assert!(assets.get(assets.mesh("floor_tile")).is_some());
    assert!(assets.get(assets.sprite("tree")).is_some());
}

#[test]
fn a_failed_replacement_keeps_the_active_scene_and_is_not_a_command_failure() {
    let mut game = game(0);
    game.world.insert_resource(FailureSeen::default());
    game.add_system("fail_switch", queue_rejected_replace);
    game.add_to_stage(Stage::Animate, "note_failure", note_failure).unwrap();
    game.add_to_stage(Stage::PostUpdate, "apply_environment", scene::apply_environment_switch).unwrap();
    let instance = game.world.resource::<Scene>().instance;
    let camera = game.world.resource::<Scene>().camera;

    game.step(STEP);

    assert_eq!(
        game.world.resource::<FailureSeen>().status,
        Some(SceneOpStatus::Failed { error: SceneError::BuildRejected })
    );
    let scene = game.world.resource::<Scene>();
    assert_eq!(scene.environment, Environment::A);
    assert_eq!(scene.instance, instance);
    assert!(scene.pending.is_none());
    drop(scene);
    assert!(has(&game, "pillar") && has(&game, "figure A"));
    assert!(!has(&game, "yard cube"));
    assert_eq!(game.world.resource::<Scene>().camera, camera);
    assert!(game.world.command_failures().is_empty());
    assert_eq!(game.world.resource::<Diagnostics>().command_failures, 0);
}

fn queue_rejected_replace(world: &mut World) {
    let pending = world.resource::<Scene>().pending;
    if pending.is_some() {
        return;
    }
    let instance = world.resource::<Scene>().instance;
    let operation = world.commands().replace_scene(instance, |_| Err(SceneError::BuildRejected)).unwrap();
    world.resource_mut::<Scene>().pending = Some(operation);
}

#[test]
fn an_unexpected_load_stays_published_and_the_active_environment_stays() {
    capture::begin();
    let mut game = game(0);
    game.world.insert_resource(FailureSeen::default());
    game.add_system("load_instead", queue_empty_load);
    game.add_to_stage(Stage::Animate, "note_failure", note_failure).unwrap();
    game.add_to_stage(Stage::PostUpdate, "apply_environment", scene::apply_environment_switch).unwrap();
    let instance = game.world.resource::<Scene>().instance;

    game.step(STEP);

    let SceneOpStatus::Loaded { instance: published } = game.world.resource::<FailureSeen>().status.unwrap() else {
        panic!("animate saw {:?}", game.world.resource::<FailureSeen>().status);
    };
    assert_eq!(game.world.resource::<Scene>().environment, Environment::A);
    assert_eq!(game.world.resource::<Scene>().instance, instance);
    assert!(game.world.resource::<Scene>().pending.is_none());
    assert!(has(&game, "pillar"));
    assert_eq!(game.world.scene_entities(published).unwrap(), Some(Vec::new()));
    assert!(game.world.scene_instances().unwrap().contains(&published));
    assert!(game.world.scene_instances().unwrap().contains(&instance));
    assert_eq!(game.world.clear_scene_operations().unwrap(), 0);
    assert_error_mentions(&["does not own", "leaving it published", &published.to_string()]);
}

#[test]
fn an_unexpected_unload_is_left_in_place_and_the_active_environment_stays() {
    capture::begin();
    let mut game = game(0);
    let extra = game.world.load_scene(|_| Ok(())).unwrap();
    game.world.insert_resource(FailureSeen::default());
    game.add_system("unload_instead", move |world| queue_unload(world, extra));
    game.add_to_stage(Stage::Animate, "note_failure", note_failure).unwrap();
    game.add_to_stage(Stage::PostUpdate, "apply_environment", scene::apply_environment_switch).unwrap();
    let instance = game.world.resource::<Scene>().instance;

    game.step(STEP);

    assert_eq!(game.world.resource::<FailureSeen>().status, Some(SceneOpStatus::Unloaded { instance: extra }));
    assert_eq!(game.world.resource::<Scene>().environment, Environment::A);
    assert_eq!(game.world.resource::<Scene>().instance, instance);
    assert!(game.world.resource::<Scene>().pending.is_none());
    assert!(has(&game, "pillar"));
    assert_eq!(game.world.scene_entities(extra).unwrap(), None);
    assert!(!game.world.scene_instances().unwrap().contains(&extra));
    assert!(game.world.scene_instances().unwrap().contains(&instance));
    assert_error_mentions(&["does not own", "leaving the world", &extra.to_string()]);
}

fn assert_error_mentions(parts: &[&str]) {
    let records = capture::records();
    let shown: Vec<String> = records.iter().map(|record| format!("{:?} {}", record.level, record.message)).collect();
    assert!(
        records.iter().any(|record| {
            record.level == log::Level::Error && parts.iter().all(|part| record.message.contains(part))
        }),
        "{shown:?}"
    );
}

fn queue_unload(world: &mut World, instance: SceneInstanceId) {
    if world.resource::<Scene>().pending.is_some() {
        return;
    }
    let operation = world.commands().unload_scene(instance).unwrap();
    world.resource_mut::<Scene>().pending = Some(operation);
}

fn queue_empty_load(world: &mut World) {
    if world.resource::<Scene>().pending.is_some() {
        return;
    }
    let operation = world.commands().load_scene(|_| Ok(())).unwrap();
    world.resource_mut::<Scene>().pending = Some(operation);
}

#[test]
fn every_camera_configuration_still_extracts_after_switching_both_ways() {
    for (config, name) in config_names() {
        let mut game = game(config);
        install(&mut game);
        let camera = game.world.resource::<Scene>().camera;
        let projection = game.world.resource::<Scene>().projection;
        press_tab(&mut game);
        game.step(STEP);
        assert_scene_camera(&game, camera, projection, name);
        let extracted = game.extract().unwrap_or_else(|| panic!("{name}: no camera"));
        assert_eq!(extracted.stats.missing_assets, 0, "{name}");
        assert!(extracted.stats.sprites_visible > 0 && extracted.stats.meshes_visible > 0, "{name}");
        assert_eq!(extracted.stats.lights, 1, "{name}");
        assert!(has(&game, "yard cube"), "{name}");
        release_tab(&mut game);
        game.step(STEP);
        press_tab(&mut game);
        game.step(STEP);
        assert_eq!(game.world.resource::<Scene>().environment, Environment::A);
        assert_scene_camera(&game, camera, projection, name);
        let extracted = game.extract().unwrap();
        assert_eq!(extracted.stats.lights, 0, "{name}");
        assert_eq!(extracted.stats.missing_assets, 0, "{name}");
        assert!(has(&game, "figure A") && has(&game, "pillar"), "{name}");
    }
}

fn assert_scene_camera(game: &Game, camera: Entity, projection: CameraProjection, name: &str) {
    assert_eq!(game.world.resource::<Scene>().camera, camera, "{name}");
    assert!(game.world.contains(camera), "{name}");
    assert!(game.world.scene_membership(camera).unwrap().is_none(), "{name}");
    let component = *game.world.component::<Camera>(camera).unwrap();
    assert_eq!(component.projection, projection, "{name}");
    let global = *game.world.component::<GlobalTransform>(camera).unwrap();
    let extracted = game.extract().unwrap();
    let expected = CameraView::new(&component, &global, Vec2::new(W as f32, H as f32));
    assert_eq!(extracted.sole_pass().view.position, expected.position, "{name}");
    assert_eq!(extracted.sole_pass().view.proj, expected.proj, "{name}");
}

#[test]
fn environment_b_renders_in_perspective_and_orthographic() {
    render_environment_b(0, "perspective");
    render_environment_b(cameras::by_name("orthographic").unwrap(), "orthographic");
}

fn render_environment_b(config: usize, name: &str) {
    let mut game = game(config);
    install(&mut game);
    let Some(before) = render(&mut game) else { return };
    let stats_a = game.extract().unwrap().stats;
    press_tab(&mut game);
    game.step(STEP);
    let Some(after) = render(&mut game) else { return };
    let stats_b = game.extract().unwrap().stats;
    assert_ne!(before, after, "{name}");
    assert_eq!(stats_a.lights, 0, "{name}");
    assert!(stats_a.sprites_visible > 0 && stats_a.meshes_visible > 0, "{name}");
    assert_eq!(stats_b.lights, 1, "{name}");
    assert!(stats_b.sprites_visible > 0 && stats_b.meshes_visible > 0, "{name}");
    assert!(stats_b.meshes_visible < stats_a.meshes_visible, "{name}");
    assert_eq!(stats_b.missing_assets, 0, "{name}");
    assert_eq!(game.world.resource::<Scene>().environment, Environment::B);
}

#[test]
fn the_launch_helper_can_publish_b_without_a_second_camera_or_walker() {
    let mut game = game(0);
    let mut scene = game.world.remove_resource::<Scene>().unwrap();
    let (camera, walker) = (scene.camera, scene.walker);
    scene::replace_environment(&mut game.world, &mut scene, Environment::B).unwrap();
    scene::replace_environment(&mut game.world, &mut scene, Environment::B).unwrap();
    assert_eq!(scene.environment, Environment::B);
    game.world.insert_resource(scene);
    assert_eq!(count_named(&game, "camera"), 1);
    assert_eq!(count_named(&game, "walker"), 1);
    assert_eq!(game.world.resource::<Scene>().camera, camera);
    assert_eq!(game.world.resource::<Scene>().walker, walker);
    assert!(has(&game, "yard tree"));
    assert!(!has(&game, "figure A"));
}
