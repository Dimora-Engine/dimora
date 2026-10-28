//! Headless `--collision` checks. They build the real example world and step `Game`.
//!
//! The default patrol is covered here only to show that skipping the flag leaves it alone.
//! Pixels are not hashed. Positions use the gameplay root.

mod common;

use common::game;
use engine::core::InputEvent;
use engine::prelude::*;
use spatial_scene::scene::{
    self, BodyVisual, Scene, BODY_OFFSET, BODY_RADIUS, BUMPER_X, CHARACTER_START, PACE_SPEED, PACE_START,
    PILLAR_CENTER, PILLAR_SIZE, SENSOR_CENTER, SENSOR_HALF, SLIDE_CENTER, SLIDE_HALF, WALK_SPEED, WALK_Z,
};
use std::time::Duration;

const EPS: f32 = 1e-3;

#[derive(Default)]
struct ScriptedIntents {
    once: Vec<(Entity, Vec3)>,
    consumed: bool,
    replay: bool,
    step: u32,
}

#[derive(Default)]
struct TriggerLog {
    events: Vec<TriggerEvent>,
}

#[derive(Clone, Debug, PartialEq)]
struct Sample {
    walker: Vec3,
    pace: Vec3,
    status: MoveStatus,
    events: Vec<TriggerEvent>,
}

fn fixed_dt() -> Duration {
    FixedTimestep::new(60).step()
}

fn fixed_secs() -> f32 {
    FixedTimestep::new(60).step_secs()
}

fn collision_game(config: usize, visual: BodyVisual) -> Game {
    let mut game = Game::new(GameConfig::default().with_size(common::W, common::H));
    spatial_scene::art::register(&mut game.assets());
    let scene = scene::build_world(&mut game.world, config);
    game.world.insert_resource(scene);
    {
        let mut actions = game.world.resource_mut::<Actions>();
        actions.bind_axis(scene::WALK_X_ACTION, ButtonSource::Key(Key::J), ButtonSource::Key(Key::L));
        actions.bind_axis(scene::WALK_Z_ACTION, ButtonSource::Key(Key::K), ButtonSource::Key(Key::I));
        actions.bind_button(scene::SWITCH_ENVIRONMENT, ButtonSource::Key(Key::Tab));
    }
    scene::install_collision(&mut game.world, visual);
    game.world.insert_resource(ScriptedIntents::default());
    game.world.insert_resource(TriggerLog::default());
    game.add_fixed_system("collision_motion", scene::collision_motion);
    game.add_fixed_system("scripted_motion", scripted_motion);
    game.add_system("camera", scene::camera_system);
    game.add_system("motion", scene::motion_system);
    game.add_system("rearm_script", rearm_script);
    game.add_system("watch_triggers", watch_triggers);
    game.add_system("switch_environment", scene::request_environment_switch);
    game.add_to_stage(Stage::PostUpdate, "apply_environment", scene::apply_environment_switch).unwrap();
    game
}

fn scripted_motion(world: &mut World) {
    let replay = world.resource::<ScriptedIntents>().replay;
    if replay {
        let step = world.resource::<ScriptedIntents>().step;
        let walker = world.resource::<Scene>().walker;
        let intent = replay_intent(step);
        world.resource_mut::<ScriptedIntents>().step += 1;
        world.insert(walker, (MoveIntent::new(intent),)).unwrap();
        return;
    }
    let mut script = world.resource_mut::<ScriptedIntents>();
    if script.consumed || script.once.is_empty() {
        return;
    }
    let once = std::mem::take(&mut script.once);
    script.consumed = true;
    drop(script);
    for (entity, translation) in once {
        world.insert(entity, (MoveIntent::new(translation),)).unwrap();
    }
}

fn rearm_script(world: &mut World) {
    world.resource_mut::<ScriptedIntents>().consumed = false;
}

fn watch_triggers(world: &mut World) {
    let batch = world.events::<TriggerEvent>();
    world.resource_mut::<TriggerLog>().events.extend(batch);
}

fn replay_intent(step: u32) -> Vec3 {
    let d = WALK_SPEED * fixed_secs();
    match step % 200 {
        0..=50 => Vec3::new(0.0, 0.0, d),
        51..=90 => Vec3::new(d, 0.0, d).normalized() * d,
        91..=140 => Vec3::new(d, 0.0, 0.0),
        141..=170 => Vec3::new(0.0, 0.0, -d),
        171..=180 => Vec3::new(0.0, d, 0.0),
        181..=190 => Vec3::new(0.0, -d, 0.0),
        _ => Vec3::new(d * 0.5, 0.0, 0.0),
    }
}

fn queue(game: &mut Game, entity: Entity, translation: Vec3) {
    game.world.resource_mut::<ScriptedIntents>().once.push((entity, translation));
}

fn local(game: &Game, entity: Entity) -> Vec3 {
    game.world.component::<Transform>(entity).unwrap().position
}

fn named(game: &Game, name: &str) -> Entity {
    game.world.find_by_name(name).into_iter().next().unwrap_or_else(|| panic!("missing {name}"))
}

fn near(actual: Vec3, expected: Vec3) {
    assert!(actual.approx_eq(expected, EPS), "position {actual:?}, expected {expected:?}");
}

fn status_of(game: &Game, entity: Entity) -> MoveStatus {
    game.world.component::<MoveResult>(entity).unwrap().status
}

fn applied_of(game: &Game, entity: Entity) -> Vec3 {
    game.world.component::<MoveResult>(entity).unwrap().applied
}

fn take_log(game: &mut Game) -> Vec<TriggerEvent> {
    std::mem::take(&mut game.world.resource_mut::<TriggerLog>().events)
}

fn involving(events: &[TriggerEvent], entity: Entity) -> Vec<TriggerEvent> {
    events
        .iter()
        .copied()
        .filter(|event| match *event {
            TriggerEvent::Entered { a, b } | TriggerEvent::Exited { a, b } => a == entity || b == entity,
        })
        .collect()
}

fn skin_of(game: &Game, entity: Entity) -> f32 {
    game.world.component::<CharacterMotor>(entity).unwrap().skin
}

#[test]
fn the_default_walker_keeps_its_sprite_patrol_and_gains_no_collider() {
    let game = game(0);
    let walker = game.world.resource::<Scene>().walker;
    assert!(!game.world.resource::<Scene>().collision);
    assert!(game.world.component::<Sprite>(walker).is_ok());
    assert!(game.world.component::<Collider>(walker).is_err());
    assert!(game.world.component::<CharacterMotor>(walker).is_err());
    assert_eq!(local(&game, walker).z, WALK_Z);
    assert!(game.world.find_by_name("gate sensor").is_empty());
    assert!(game.world.find_by_name("slide wall").is_empty());
    assert!(game.world.find_by_name("pace").is_empty());
    assert!(game.world.find_by_name("walker visual").is_empty());
}

#[test]
fn the_motion_system_is_registered_ahead_of_the_sealed_collision_pass() {
    let game = collision_game(0, BodyVisual::Sprite);
    let order = game.system_order(Stage::FixedUpdate);
    let motion = order.iter().position(|name| *name == "collision_motion").unwrap();
    let scripted = order.iter().position(|name| *name == "scripted_motion").unwrap();
    let sync = order.iter().position(|name| *name == "sync_collision").unwrap();
    assert!(motion < scripted && scripted < sync);
}

#[test]
fn the_gameplay_root_owns_the_capsule_and_the_visual_child_does_not() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let scene = game.world.resource::<Scene>();
    let (walker, visual) = (scene.walker, scene.walker_visual.unwrap());
    drop(scene);
    assert!(game.world.component::<Sprite>(walker).is_err());
    assert!(game.world.component::<Collider>(visual).is_err());
    assert!(game.world.component::<CharacterMotor>(visual).is_err());
    assert_eq!(game.world.component::<Parent>(visual).unwrap().0, walker);
    assert_eq!(game.world.component::<Sprite>(visual).unwrap().anchor, Some(Vec2::new(0.5, 0.0)));
    assert_eq!(local(&game, visual), Vec3::ZERO);
    let collider = *game.world.component::<Collider>(walker).unwrap();
    assert_eq!(collider.offset, BODY_OFFSET);
    assert!(!collider.sensor);
    game.step(fixed_dt());
    let indexed = game.world.resource::<CollisionWorld>().entities();
    assert!(indexed.contains(&walker));
    assert!(!indexed.contains(&visual));
    let center = game.world.resource::<CollisionWorld>().center(walker).unwrap();
    near(center, CHARACTER_START + BODY_OFFSET);
    let child = game.world.component::<GlobalTransform>(visual).unwrap().translation();
    near(child, local(&game, walker));
    assert!(!game.world.resource::<CollisionDebug>().enabled);
    assert!(game.world.resource::<DebugDraw>().is_empty());
}

#[test]
fn ijkl_requests_world_xz_and_a_diagonal_does_not_run_faster() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let walker = game.world.resource::<Scene>().walker;
    let start = local(&game, walker);
    game.send_input(InputEvent::Key { key: Key::L, pressed: true });
    game.step(fixed_dt());
    let step = WALK_SPEED * fixed_secs();
    near(local(&game, walker), start + Vec3::new(step, 0.0, 0.0));
    assert_eq!(status_of(&game, walker), MoveStatus::Completed);

    game.send_input(InputEvent::Key { key: Key::L, pressed: false });
    game.send_input(InputEvent::Key { key: Key::J, pressed: true });
    game.send_input(InputEvent::Key { key: Key::K, pressed: true });
    let at = local(&game, walker);
    game.step(fixed_dt());
    near(local(&game, walker), at + Vec3::new(-step, 0.0, -step).normalized() * step);
    assert_eq!(local(&game, walker).y, start.y);
}

#[test]
fn two_fixed_iterations_apply_two_steps_and_a_scripted_intent_applies_once() {
    let mut held = collision_game(0, BodyVisual::Sprite);
    let walker = held.world.resource::<Scene>().walker;
    let start = local(&held, walker);
    held.send_input(InputEvent::Key { key: Key::I, pressed: true });
    held.step(fixed_dt() * 2);
    assert_eq!(held.world.resource::<Diagnostics>().fixed_steps, 2);
    let step = WALK_SPEED * fixed_secs();
    near(local(&held, walker), start + Vec3::new(0.0, 0.0, step * 2.0));

    let mut once = collision_game(0, BodyVisual::Sprite);
    let walker = once.world.resource::<Scene>().walker;
    queue(&mut once, walker, Vec3::new(10.0, 0.0, 0.0));
    once.step(fixed_dt() * 2);
    assert_eq!(once.world.resource::<Diagnostics>().fixed_steps, 2);
    near(local(&once, walker), CHARACTER_START + Vec3::new(10.0, 0.0, 0.0));
}

#[test]
fn an_explicit_vertical_intent_still_moves_the_root() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let walker = game.world.resource::<Scene>().walker;
    queue(&mut game, walker, Vec3::new(3.0, 4.0, 5.0));
    game.step(fixed_dt());
    near(local(&game, walker), CHARACTER_START + Vec3::new(3.0, 4.0, 5.0));
    assert_eq!(status_of(&game, walker), MoveStatus::Completed);
    assert_eq!(game.world.component::<CharacterMotor>(walker).unwrap().locked_axes, BVec3::FALSE);
}

#[test]
fn a_zero_step_does_not_move_or_emit_and_drops_a_waiting_intent() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let walker = game.world.resource::<Scene>().walker;
    let start = local(&game, walker);
    game.world.insert(walker, (MoveIntent::new(Vec3::new(0.0, 0.0, 40.0)),)).unwrap();
    game.step(Duration::ZERO);
    assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 0);
    assert_eq!(local(&game, walker), start);
    assert!(take_log(&mut game).is_empty());
    assert!(game.world.component::<MoveIntent>(walker).is_err());
    assert_eq!(game.world.resource::<CollisionWorld>().generation(), 0);
}

#[test]
fn the_pillar_stops_a_capsule_from_every_horizontal_side_including_a_tunnel_attempt() {
    let skin = 0.01;
    let half = PILLAR_SIZE * 0.5;
    let near_z = PILLAR_CENTER.z - half.z - BODY_RADIUS - skin;
    let far_z = PILLAR_CENTER.z + half.z + BODY_RADIUS + skin;
    let pos_x = PILLAR_CENTER.x + half.x + BODY_RADIUS + skin;
    let neg_x = PILLAR_CENTER.x - half.x - BODY_RADIUS - skin;
    let (pos, status, _) = drive(Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, 80.0));
    assert_eq!(status, MoveStatus::Blocked);
    near(pos, Vec3::new(0.0, 0.0, near_z));
    let (pos, status, _) = drive(Vec3::new(0.0, 0.0, 280.0), Vec3::new(0.0, 0.0, -80.0));
    assert_eq!(status, MoveStatus::Blocked);
    near(pos, Vec3::new(0.0, 0.0, far_z));
    let (pos, status, _) = drive(Vec3::new(80.0, 0.0, 200.0), Vec3::new(-80.0, 0.0, 0.0));
    assert_eq!(status, MoveStatus::Blocked);
    near(pos, Vec3::new(pos_x, 0.0, 200.0));
    let (pos, status, _) = drive(Vec3::new(-80.0, 0.0, 200.0), Vec3::new(80.0, 0.0, 0.0));
    assert_eq!(status, MoveStatus::Blocked);
    near(pos, Vec3::new(neg_x, 0.0, 200.0));

    let mut game = collision_game(0, BodyVisual::Sprite);
    let walker = game.world.resource::<Scene>().walker;
    let beyond = game.world.spawn((
        Transform::from_xyz(0.0, 40.0, 240.0),
        Collider::aabb_box(Vec3::new(20.0, 40.0, 0.5)).sensor(),
        Name::new("beyond sensor"),
    ));
    queue(&mut game, walker, Vec3::new(0.0, 0.0, 500.0));
    game.step(fixed_dt());
    let pos = local(&game, walker);
    assert_eq!(status_of(&game, walker), MoveStatus::Blocked);
    assert!(pos.z < 170.0, "tunneled to {}", pos.z);
    assert!((pos.z - (PILLAR_CENTER.z - PILLAR_SIZE.z * 0.5 - BODY_RADIUS - skin_of(&game, walker))).abs() < EPS);
    let events = involving(&take_log(&mut game), walker);
    let gate = named(&game, "gate sensor");
    assert_eq!(events, vec![TriggerEvent::Entered { a: walker, b: gate }, TriggerEvent::Exited { a: walker, b: gate }]);
    assert!(involving(&events, beyond).is_empty());
}

#[test]
fn a_diagonal_slides_along_the_extra_wall_without_speeding_up_or_climbing() {
    let start = CHARACTER_START;
    let intent = Vec3::new(80.0, 0.0, 80.0);
    let (pos, status, applied) = drive(start, intent);
    assert_eq!(status, MoveStatus::Blocked);
    let skin = 0.01;
    let face = SLIDE_CENTER.x - SLIDE_HALF.x;
    let stop_x = face - BODY_RADIUS - skin;
    // The slide stops outside the solid, within a couple of thousandths of the skin gap.
    assert!(pos.x + BODY_RADIUS < face, "capsule penetrated the slide wall at x {}", pos.x);
    assert!((pos.x - stop_x).abs() < 2e-3, "x {} expected near {stop_x}", pos.x);
    assert!(pos.z > start.z + 70.0, "did not slide, z {}", pos.z);
    assert!(pos.y.abs() < EPS);
    assert!(applied.length() < intent.length());
    assert!(pos.x < face - BODY_RADIUS);
}

#[test]
fn the_gate_emits_enter_then_exit_and_does_not_repeat_while_overlapped() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let walker = game.world.resource::<Scene>().walker;
    let gate = named(&game, "gate sensor");
    queue(&mut game, walker, Vec3::new(0.0, 0.0, 25.0));
    game.step(fixed_dt());
    let entered = involving(&take_log(&mut game), walker);
    assert_eq!(entered, vec![TriggerEvent::Entered { a: walker, b: gate }]);
    assert_eq!(status_of(&game, walker), MoveStatus::Completed);
    queue(&mut game, walker, Vec3::new(0.0, 0.0, 2.0));
    game.step(fixed_dt());
    assert!(involving(&take_log(&mut game), walker).is_empty());
    let pos = local(&game, walker);
    let capsule_z = pos.z;
    assert!(capsule_z > SENSOR_CENTER.z - SENSOR_HALF.z - BODY_RADIUS);
    assert!(capsule_z < SENSOR_CENTER.z + SENSOR_HALF.z + BODY_RADIUS);
}

#[test]
fn a_slide_reports_the_traveled_segment_and_not_the_chord_or_the_far_side() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let y = 500.0;
    let wall = game.world.spawn((Transform::from_xyz(0.05, y, 0.0), Collider::aabb_box(Vec3::new(0.05, 5.0, 20.0))));
    let character = game.world.spawn((
        Transform::from_xyz(-2.5, y, 0.0),
        Collider::sphere(0.5),
        CharacterMotor { skin: 0.0, ..CharacterMotor::default() },
    ));
    let on_slide =
        game.world.spawn((Transform::from_xyz(-0.5, y, 3.2), Collider::aabb_box(Vec3::new(0.2, 0.5, 0.05)).sensor()));
    let on_chord =
        game.world.spawn((Transform::from_xyz(-1.15, y, 3.0), Collider::aabb_box(Vec3::splat(0.05)).sensor()));
    let beyond = game.world.spawn((Transform::from_xyz(2.0, y, 1.0), Collider::aabb_box(Vec3::splat(0.3)).sensor()));
    let _ = wall;
    queue(&mut game, character, Vec3::new(4.0, 0.0, 4.0));
    game.step(fixed_dt());
    near(applied_of(&game, character), Vec3::new(2.0, 0.0, 4.0));
    assert_eq!(status_of(&game, character), MoveStatus::Blocked);
    let events = involving(&take_log(&mut game), character);
    assert_eq!(
        events,
        vec![TriggerEvent::Entered { a: character, b: on_slide }, TriggerEvent::Exited { a: character, b: on_slide }]
    );
    assert!(events.iter().all(|event| !mentions(event, on_chord) && !mentions(event, beyond)));
    assert!(events.iter().all(canonical));
}

#[test]
fn a_sensor_motor_and_a_solid_motor_both_pass_through_the_other_kind() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let solid = game.world.spawn((Transform::from_xyz(0.0, 0.0, -800.0), Collider::aabb_box(Vec3::new(1.0, 2.0, 2.0))));
    let ghost = game.world.spawn((
        Transform::from_xyz(-8.0, 0.0, -800.0),
        Collider::sphere(0.5).sensor(),
        CharacterMotor { skin: 0.0, ..CharacterMotor::default() },
    ));
    let volume = game
        .world
        .spawn((Transform::from_xyz(0.0, 0.0, -900.0), Collider::aabb_box(Vec3::new(0.2, 2.0, 2.0)).sensor()));
    let body = game.world.spawn((
        Transform::from_xyz(8.0, 0.0, -900.0),
        Collider::sphere(0.5),
        CharacterMotor { skin: 0.0, ..CharacterMotor::default() },
    ));
    queue(&mut game, ghost, Vec3::new(16.0, 0.0, 0.0));
    queue(&mut game, body, Vec3::new(-16.0, 0.0, 0.0));
    game.step(fixed_dt());
    near(local(&game, ghost), Vec3::new(8.0, 0.0, -800.0));
    near(local(&game, body), Vec3::new(-8.0, 0.0, -900.0));
    assert_eq!(status_of(&game, ghost), MoveStatus::Completed);
    assert_eq!(status_of(&game, body), MoveStatus::Completed);
    let events = take_log(&mut game);
    let (ghost_a, ghost_b) = ordered(solid, ghost);
    let (body_a, body_b) = ordered(volume, body);
    assert!(events.contains(&TriggerEvent::Entered { a: ghost_a, b: ghost_b }));
    assert!(events.contains(&TriggerEvent::Exited { a: ghost_a, b: ghost_b }));
    assert!(events.contains(&TriggerEvent::Entered { a: body_a, b: body_b }));
    assert!(events.contains(&TriggerEvent::Exited { a: body_a, b: body_b }));
    assert!(events.iter().all(canonical));
}

#[test]
fn three_motors_resolve_in_ascending_entity_order() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let spawn_lane = |game: &mut Game, x: f32, name: &str| {
        game.world.spawn((
            Transform::from_xyz(x, 0.0, -400.0),
            scene::character_collider(),
            CharacterMotor::default(),
            Name::new(name),
        ))
    };
    let a = spawn_lane(&mut game, 0.0, "lane a");
    let b = spawn_lane(&mut game, 30.0, "lane b");
    let c = spawn_lane(&mut game, 60.0, "lane c");
    assert!(a < b && b < c);
    queue(&mut game, a, Vec3::new(40.0, 0.0, 0.0));
    queue(&mut game, b, Vec3::new(40.0, 0.0, 0.0));
    queue(&mut game, c, Vec3::new(40.0, 0.0, 0.0));
    game.step(fixed_dt());
    let skin = skin_of(&game, a);
    let gap = BODY_RADIUS * 2.0 + skin;
    near(local(&game, a), Vec3::new(30.0 - gap, 0.0, -400.0));
    near(local(&game, b), Vec3::new(60.0 - gap, 0.0, -400.0));
    near(local(&game, c), Vec3::new(100.0, 0.0, -400.0));
    assert_ne!(local(&game, a).x, 40.0);
}

#[test]
fn the_scripted_character_moves_with_the_controlled_one() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let scene = game.world.resource::<Scene>();
    let (walker, pace) = (scene.walker, scene.pace.unwrap());
    drop(scene);
    assert!(game.world.component::<Collider>(named(&game, "pace visual")).is_err());
    queue(&mut game, walker, Vec3::new(0.0, 0.0, 4.0));
    game.step(fixed_dt());
    near(local(&game, walker), CHARACTER_START + Vec3::new(0.0, 0.0, 4.0));
    near(local(&game, pace), PACE_START + Vec3::new(PACE_SPEED * fixed_secs(), 0.0, 0.0));
    assert!(game.world.resource::<CollisionWorld>().entities().contains(&pace));
    assert_eq!(local(&game, named(&game, "bumper +x")).x, BUMPER_X);
}

#[test]
fn sprite_and_mesh_children_leave_the_same_gameplay_trace() {
    let sprite = run_sequence(0, BodyVisual::Sprite, true);
    let mesh = run_sequence(0, BodyVisual::Mesh, true);
    assert_eq!(sprite, mesh);
    let mut check = collision_game(0, BodyVisual::Mesh);
    let visual = check.world.resource::<Scene>().walker_visual.unwrap();
    assert!(check.world.component::<Mesh>(visual).is_ok());
    assert!(check.world.component::<Sprite>(visual).is_err());
    check.step(fixed_dt());
    let walker = check.world.resource::<Scene>().walker;
    assert!(!check.world.resource::<CollisionWorld>().entities().contains(&visual));
    assert!(check.world.resource::<CollisionWorld>().entities().contains(&walker));
}

#[test]
fn cameras_do_not_change_positions_statuses_or_trigger_order() {
    let reference = run_sequence(0, BodyVisual::Sprite, true);
    for name in ["orthographic", "isometric", "side"] {
        let index = spatial_scene::cameras::by_name(name).unwrap();
        assert_eq!(run_sequence(index, BodyVisual::Sprite, true), reference, "{name}");
    }
    assert_eq!(run_sequence(0, BodyVisual::Sprite, false), reference, "no active camera");
}

#[test]
fn changing_the_camera_between_steps_leaves_the_gameplay_root() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let walker = game.world.resource::<Scene>().walker;
    queue(&mut game, walker, Vec3::new(0.0, 6.0, 8.0));
    game.step(fixed_dt());
    let at = local(&game, walker);
    {
        let mut scene = game.world.resource_mut::<Scene>();
        scene::select_config(&mut scene, spatial_scene::cameras::by_name("side").unwrap());
    }
    game.step(fixed_dt());
    assert_eq!(local(&game, walker), at);
    assert_eq!(game.world.resource::<Scene>().config_name(), "side");
}

#[test]
fn a_direct_teleport_across_the_gate_is_endpoint_only() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let walker = game.world.resource::<Scene>().walker;
    game.step(fixed_dt());
    let _ = take_log(&mut game);
    game.world.component_mut::<Transform>(walker).unwrap().position = Vec3::new(0.0, 0.0, 160.0);
    game.step(fixed_dt());
    assert_eq!(local(&game, walker), Vec3::new(0.0, 0.0, 160.0));
    assert!(involving(&take_log(&mut game), walker).is_empty());
}

#[test]
fn replacing_the_environment_drops_the_pillar_and_a_return_blocks_again() {
    let mut game = collision_game(0, BodyVisual::Sprite);
    game.step(fixed_dt());
    let pillar = named(&game, "pillar");
    let sensor = named(&game, "gate sensor");
    let pace = game.world.resource::<Scene>().pace.unwrap();
    let walker = game.world.resource::<Scene>().walker;
    assert!(game.world.resource::<CollisionWorld>().center(pillar).is_some());

    game.send_input(InputEvent::Key { key: Key::Tab, pressed: true });
    game.step(fixed_dt());
    assert_eq!(game.world.resource::<Scene>().environment, scene::Environment::B);
    assert!(!game.world.contains(pillar));
    assert!(game.world.contains(sensor) && game.world.contains(pace) && game.world.contains(walker));
    assert!(game.world.command_failures().is_empty());

    game.step(fixed_dt());
    let indexed = game.world.resource::<CollisionWorld>().entities();
    assert!(!indexed.contains(&pillar));
    assert!(indexed.contains(&sensor));
    assert!(game.world.find_by_name("pillar").is_empty());
    let yard = named(&game, "yard pillar");
    assert!(game.world.component::<Collider>(yard).is_err());

    game.send_input(InputEvent::Key { key: Key::Tab, pressed: false });
    game.step(fixed_dt());
    game.send_input(InputEvent::Key { key: Key::Tab, pressed: true });
    game.step(fixed_dt());
    assert_eq!(game.world.resource::<Scene>().environment, scene::Environment::A);
    let pillar = named(&game, "pillar");
    assert!(game.world.component::<Collider>(pillar).is_ok());
    assert!(game.world.resource::<CollisionWorld>().center(pillar).is_none());
    game.step(fixed_dt());
    assert!(game.world.resource::<CollisionWorld>().center(pillar).is_some());

    queue(&mut game, walker, Vec3::new(0.0, 0.0, 80.0));
    game.step(fixed_dt());
    assert_eq!(status_of(&game, walker), MoveStatus::Blocked);
    assert!(local(&game, walker).z < 170.0);
}

#[test]
fn one_thousand_steps_hash_the_same_bits_across_two_fresh_runs() {
    let first = hash_run();
    let second = hash_run();
    assert_eq!(first.hash, second.hash);
    assert!(first.blocked);
    assert!(first.entered && first.exited);
    assert!(first.max_y > 1.0);
    assert_ne!(first.hash, 0);
}

struct HashRun {
    hash: u64,
    blocked: bool,
    entered: bool,
    exited: bool,
    max_y: f32,
}

fn hash_run() -> HashRun {
    let mut game = collision_game(0, BodyVisual::Sprite);
    game.world.resource_mut::<ScriptedIntents>().replay = true;
    let walker = game.world.resource::<Scene>().walker;
    let pace = game.world.resource::<Scene>().pace.unwrap();
    let mut hash = 0u64;
    let mut blocked = false;
    let mut entered = false;
    let mut exited = false;
    let mut max_y = 0.0f32;
    for _ in 0..1000 {
        let _ = take_log(&mut game);
        game.step(fixed_dt());
        assert_eq!(game.world.resource::<Diagnostics>().fixed_steps, 1);
        let walker_pos = local(&game, walker);
        let pace_pos = local(&game, pace);
        max_y = max_y.max(walker_pos.y.abs());
        hash = mix_vec(hash, walker_pos);
        hash = mix_vec(hash, pace_pos);
        hash = mix_vec(hash, applied_of(&game, walker));
        let walker_status = status_of(&game, walker);
        let pace_status = status_of(&game, pace);
        blocked |= walker_status == MoveStatus::Blocked || pace_status == MoveStatus::Blocked;
        hash = mix(hash, status_bits(walker_status));
        hash = mix(hash, status_bits(pace_status));
        for event in game.world.resource::<TriggerLog>().events.iter().copied() {
            match event {
                TriggerEvent::Entered { .. } => entered = true,
                TriggerEvent::Exited { .. } => exited = true,
            }
            hash = mix(hash, event_bits(event));
        }
    }
    HashRun { hash, blocked, entered, exited, max_y }
}

fn mix(hash: u64, bits: u64) -> u64 {
    hash.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(bits)
}

fn mix_vec(hash: u64, value: Vec3) -> u64 {
    mix(mix(mix(hash, value.x.to_bits() as u64), value.y.to_bits() as u64), value.z.to_bits() as u64)
}

fn status_bits(status: MoveStatus) -> u64 {
    match status {
        MoveStatus::Completed => 1,
        MoveStatus::Blocked => 2,
        MoveStatus::Rejected => 3,
    }
}

fn event_bits(event: TriggerEvent) -> u64 {
    match event {
        TriggerEvent::Entered { a, b } => mix(mix(1, a.to_bits().get()), b.to_bits().get()),
        TriggerEvent::Exited { a, b } => mix(mix(2, a.to_bits().get()), b.to_bits().get()),
    }
}

fn drive(start: Vec3, intent: Vec3) -> (Vec3, MoveStatus, Vec3) {
    let mut game = collision_game(0, BodyVisual::Sprite);
    let pace = game.world.resource::<Scene>().pace.unwrap();
    game.world.component_mut::<Transform>(pace).unwrap().position = Vec3::new(0.0, 0.0, 2000.0);
    let walker = game.world.resource::<Scene>().walker;
    game.world.component_mut::<Transform>(walker).unwrap().position = start;
    queue(&mut game, walker, intent);
    game.step(fixed_dt());
    (local(&game, walker), status_of(&game, walker), applied_of(&game, walker))
}

fn ordered(a: Entity, b: Entity) -> (Entity, Entity) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn run_sequence(config: usize, visual: BodyVisual, camera_active: bool) -> Vec<Sample> {
    let mut game = collision_game(config, visual);
    if !camera_active {
        let camera = game.world.resource::<Scene>().camera;
        game.world.component_mut::<Camera>(camera).unwrap().active = false;
    }
    let walker = game.world.resource::<Scene>().walker;
    let pace = game.world.resource::<Scene>().pace.unwrap();
    let moves = [
        Vec3::new(0.0, 0.0, 30.0),
        Vec3::new(0.0, 0.0, 80.0),
        Vec3::new(40.0, 0.0, 0.0),
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::new(0.0, -5.0, -20.0),
    ];
    let mut samples = Vec::new();
    for intent in moves {
        let _ = take_log(&mut game);
        queue(&mut game, walker, intent);
        game.step(fixed_dt());
        samples.push(Sample {
            walker: local(&game, walker),
            pace: local(&game, pace),
            status: status_of(&game, walker),
            events: involving(&game.world.resource::<TriggerLog>().events, walker),
        });
    }
    samples
}

fn mentions(event: &TriggerEvent, entity: Entity) -> bool {
    match *event {
        TriggerEvent::Entered { a, b } | TriggerEvent::Exited { a, b } => a == entity || b == entity,
    }
}

fn canonical(event: &TriggerEvent) -> bool {
    match *event {
        TriggerEvent::Entered { a, b } | TriggerEvent::Exited { a, b } => a < b,
    }
}
