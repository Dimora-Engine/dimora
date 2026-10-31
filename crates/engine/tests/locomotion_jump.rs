//! Jumping, landing, and gravity through `Game::step`. The fixed clock is 60 Hz.
//!
//! A jump edge lives in `Actions` until the first fixed iteration consumes it. A frame with no
//! fixed iteration does not conceal that edge and does not simulate. Events sent during the fixed
//! loop stay pending until `update`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use engine::core::{FixedTimestep, InputEvent};
use engine::prelude::*;

const GRAVITY: f32 = -400.0;
const JUMP: f32 = 200.0;

#[derive(Clone, Copy)]
struct Wish(Vec3);

/// One step still reaches the configured speed. The gameplay default accelerates.
fn kinematic() -> CharacterLocomotion {
    CharacterLocomotion {
        acceleration: 1.0e6,
        deceleration: 1.0e6,
        air_acceleration: 1.0e6,
        ..CharacterLocomotion::default()
    }
}

fn fixed_step(game: &Game) -> Duration {
    game.world.resource::<FixedTimestep>().step()
}

fn step_secs(game: &Game) -> f32 {
    game.world.resource::<FixedTimestep>().step_secs()
}

fn game_with_walker(height: f32) -> (Game, Entity) {
    let mut game = Game::new(GameConfig::default());
    game.world.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(8.0, 0.5, 8.0))));
    let body = game.world.spawn((
        Transform::from_xyz(0.0, height, 0.0),
        Collider::sphere(0.5),
        CharacterMotor::default(),
        kinematic(),
        CharacterMotion::default(),
    ));
    game.world.resource_mut::<Actions>().bind_button("jump", ButtonSource::Key(Key::Space));
    game.world.insert_resource(Wish(Vec3::ZERO));
    game.add_fixed_system("drive", move |world| {
        let wish = world.resource::<Wish>().0;
        let jump = world.resource::<Actions>().fixed_pressed("jump");
        let _ = world.insert(body, (LocomotionIntent { wish, jump, run: false },));
    });
    (game, body)
}

fn press_jump(game: &mut Game) {
    game.send_input(InputEvent::Key { key: Key::Space, pressed: true });
}

fn release_jump(game: &mut Game) {
    game.send_input(InputEvent::Key { key: Key::Space, pressed: false });
}

fn watch(game: &mut Game) -> Rc<RefCell<Vec<LocomotionEvent>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let slot = seen.clone();
    game.add_system("landings", move |world| {
        slot.borrow_mut().extend(world.events::<LocomotionEvent>());
    });
    seen
}

fn motion(game: &Game, body: Entity) -> CharacterMotion {
    *game.world.component::<CharacterMotion>(body).unwrap()
}

fn position(game: &Game, body: Entity) -> Vec3 {
    game.world.component::<Transform>(body).unwrap().position
}

fn near(actual: f32, expected: f32, tol: f32) {
    assert!((actual - expected).abs() <= tol, "{actual} vs {expected} (tol {tol})");
}

#[test]
fn one_two_and_eight_fixed_iterations_match_the_same_fall() {
    let fall = |frames: u32, span: u32| {
        let (mut game, body) = game_with_walker(12.0);
        let step = fixed_step(&game);
        for _ in 0..frames {
            game.step(step * span);
        }
        let steps = game.world.resource::<Diagnostics>().fixed_steps;
        (position(&game, body).y, motion(&game, body).velocity.y, steps)
    };
    let (one_y, one_v, one_steps) = fall(1, 1);
    let (two_y, two_v, two_steps) = fall(1, 2);
    let (eight_y, eight_v, eight_steps) = fall(1, 8);
    let (capped_y, capped_v, capped_steps) = fall(1, 100);
    assert_eq!(one_steps, 1);
    assert_eq!(two_steps, 2);
    assert_eq!(eight_steps, 8);
    assert_eq!(capped_steps, 8);
    let dt = {
        let (game, _) = game_with_walker(12.0);
        step_secs(&game)
    };
    let mut velocity = 0.0;
    let mut drop = 0.0;
    let mut at = [0.0; 3];
    let mut speeds = [0.0; 3];
    for index in 1..=8 {
        velocity += GRAVITY * dt;
        velocity = velocity.max(-800.0);
        drop += velocity * dt;
        if index == 1 {
            at[0] = 12.0 + drop;
            speeds[0] = velocity;
        }
        if index == 2 {
            at[1] = 12.0 + drop;
            speeds[1] = velocity;
        }
        if index == 8 {
            at[2] = 12.0 + drop;
            speeds[2] = velocity;
        }
    }
    near(one_y, at[0], 1e-3);
    near(one_v, speeds[0], 1e-3);
    near(two_y, at[1], 1e-3);
    near(two_v, speeds[1], 1e-3);
    near(eight_y, at[2], 1e-2);
    near(eight_v, speeds[2], 1e-2);
    near(capped_y, eight_y, 1e-4);
    near(capped_v, eight_v, 1e-4);
}

#[test]
fn a_zero_fixed_step_frame_keeps_the_jump_edge_and_does_not_simulate() {
    let (mut game, body) = game_with_walker(0.5);
    let seen = watch(&mut game);
    game.step(fixed_step(&game));
    assert!(seen.borrow().is_empty(), "spawn support is not a landing");
    assert!(matches!(motion(&game, body).support, GroundSupport::Supported { .. }));
    let rested = position(&game, body);

    press_jump(&mut game);
    game.step(Duration::ZERO);
    assert_eq!(position(&game, body), rested);
    assert_eq!(motion(&game, body).velocity, Vec3::ZERO);
    assert!(seen.borrow().is_empty());
    assert!(game.world.resource::<Actions>().fixed_pressed("jump"));
    assert!(game.world.component::<LocomotionIntent>(body).is_err());

    game.step(fixed_step(&game));
    near(position(&game, body).y, rested.y + JUMP * step_secs(&game), 1e-2);
    near(motion(&game, body).velocity.y, JUMP, 1e-3);
    assert_eq!(motion(&game, body).phase, LocomotionPhase::Jump);
    assert_eq!(motion(&game, body).support, GroundSupport::Airborne);
    assert!(seen.borrow().is_empty(), "takeoff does not land");
}

#[test]
fn holding_jump_across_a_multi_iteration_frame_takes_off_once() {
    let (mut game, body) = game_with_walker(0.5);
    let samples = Rc::new(RefCell::new(Vec::new()));
    let slot = samples.clone();
    game.add_fixed_system("sample", move |world| {
        let velocity = world.component::<CharacterMotion>(body).unwrap().velocity.y;
        slot.borrow_mut().push(velocity);
    });
    game.step(fixed_step(&game));
    samples.borrow_mut().clear();

    press_jump(&mut game);
    let dt = step_secs(&game);
    game.step(fixed_step(&game) * 4);
    let sampled = samples.borrow().clone();
    assert_eq!(sampled.len(), 4, "{sampled:?}");
    near(sampled[1], JUMP, 1e-3);
    near(sampled[2], JUMP + GRAVITY * dt, 1e-2);
    near(sampled[3], JUMP + GRAVITY * dt * 2.0, 1e-2);
    near(motion(&game, body).velocity.y, JUMP + GRAVITY * dt * 3.0, 1e-2);
    assert!(sampled.iter().filter(|velocity| (**velocity - JUMP).abs() < 1e-2).count() <= 1);

    let after_first = motion(&game, body).velocity.y;
    game.step(fixed_step(&game) * 4);
    let later = motion(&game, body).velocity.y;
    assert!(later < after_first - 1.0, "a held jump took off again: {after_first} -> {later}");
    assert!((later - JUMP).abs() > 1.0);

    release_jump(&mut game);
    press_jump(&mut game);
    let before = motion(&game, body).velocity.y;
    game.step(fixed_step(&game));
    assert!((motion(&game, body).velocity.y - JUMP).abs() > 1.0, "an airborne edge jumped from {before}");
    assert!(motion(&game, body).velocity.y < before);
}

#[test]
fn a_landing_on_the_first_iteration_stays_visible_to_update_and_animate() {
    let (mut game, body) = game_with_walker(0.65);
    let update_seen = watch(&mut game);
    let animate_seen = Rc::new(RefCell::new(Vec::new()));
    let slot = animate_seen.clone();
    game.add_to_stage(Stage::Animate, "animate-landings", move |world| {
        slot.borrow_mut().extend(world.events::<LocomotionEvent>());
    })
    .unwrap();
    let leaked = Rc::new(Cell::new(false));
    let flag = leaked.clone();
    game.add_fixed_system("peek", move |world| {
        if !world.events::<LocomotionEvent>().is_empty() {
            flag.set(true);
        }
    });
    *game.world.resource_mut::<Wish>() = Wish(Vec3::X);

    game.step(fixed_step(&game) * 3);
    assert!(!leaked.get(), "a fixed iteration saw the landing");
    assert_eq!(*update_seen.borrow(), vec![LocomotionEvent::Landed { entity: body }]);
    assert_eq!(*animate_seen.borrow(), vec![LocomotionEvent::Landed { entity: body }]);
    assert!(matches!(motion(&game, body).support, GroundSupport::Supported { .. }));
    assert_eq!(motion(&game, body).velocity.y, 0.0);
    assert!(position(&game, body).x > 1.0, "later iterations of the landing frame still walked");
    assert_eq!(motion(&game, body).phase, LocomotionPhase::Walk);

    let again = update_seen.borrow().len();
    game.step(fixed_step(&game));
    assert_eq!(update_seen.borrow().len(), again, "staying grounded must not land again");
}

#[test]
fn one_frame_can_cross_the_jump_apex() {
    let (mut game, body) = game_with_walker(0.5);
    game.world.insert(body, (CharacterLocomotion { jump_speed: 40.0, ..kinematic() },)).unwrap();
    game.step(fixed_step(&game));
    let start = position(&game, body).y;
    press_jump(&mut game);
    game.step(fixed_step(&game) * 8);
    let end = motion(&game, body);
    assert!(end.velocity.y < 0.0, "{:?}", end.velocity);
    assert_eq!(end.phase, LocomotionPhase::Fall);
    assert_eq!(end.support, GroundSupport::Airborne);
    assert!(position(&game, body).y > start);
}

#[test]
fn a_fall_with_no_input_does_not_depend_on_the_camera_or_a_visual_child() {
    let fall = |camera: bool, visual: Option<&'static str>| {
        let (mut game, body) = game_with_walker(9.0);
        if camera {
            game.world.spawn((Transform::from_xyz(0.0, 8.0, 12.0), Camera::perspective(50.0)));
            game.world.spawn((Transform::from_xyz(4.0, 6.0, 0.0), Camera::orthographic(20.0)));
        }
        match visual {
            Some("sprite") => {
                game.world.spawn_child(body, (Transform::IDENTITY, Sprite::new("hero")));
            }
            Some("mesh") => {
                game.world.spawn_child(body, (Transform::IDENTITY, Mesh::new("cube")));
            }
            _ => {}
        }
        game.step(fixed_step(&game) * 2);
        (position(&game, body), motion(&game, body))
    };
    let plain = fall(false, None);
    let viewed = fall(true, None);
    let sprite = fall(false, Some("sprite"));
    let mesh = fall(false, Some("mesh"));
    assert_eq!(plain.0, viewed.0);
    assert_eq!(plain.1.velocity, viewed.1.velocity);
    assert_eq!(plain.1.phase, viewed.1.phase);
    assert_eq!(plain.1.support, viewed.1.support);
    assert_eq!(sprite.0, mesh.0);
    assert_eq!(sprite.1, mesh.1);
    assert!(plain.0.y < 9.0);
    assert_eq!(plain.1.phase, LocomotionPhase::Fall);
}

#[test]
fn two_games_replay_a_jump_fall_landing_and_walk() {
    fn play(camera: bool) -> (Vec3, CharacterMotion, Vec<LocomotionEvent>) {
        let (mut game, body) = game_with_walker(0.5);
        if camera {
            game.world.spawn((Transform::from_xyz(-4.0, 3.0, 9.0), Camera::perspective(35.0)));
        }
        let seen = watch(&mut game);
        let step = fixed_step(&game);
        game.step(step);
        press_jump(&mut game);
        game.step(Duration::ZERO);
        game.step(step);
        release_jump(&mut game);
        for _ in 0..16 {
            game.step(step * 8);
            if seen.borrow().iter().any(|event| matches!(event, LocomotionEvent::Landed { .. })) {
                break;
            }
        }
        *game.world.resource_mut::<Wish>() = Wish(Vec3::X);
        game.step(step * 3);
        let events = seen.borrow().clone();
        (position(&game, body), motion(&game, body), events)
    }

    let left = play(false);
    let right = play(true);
    assert_eq!(left.0, right.0);
    assert_eq!(left.1.velocity, right.1.velocity);
    assert_eq!(left.1.phase, right.1.phase);
    assert_eq!(left.1.support, right.1.support);
    assert_eq!(left.2.len(), 1, "{:?}", left.2);
    assert_eq!(left.2.len(), right.2.len());
    assert!(left.0.x > 1.0, "{:?}", left.0);
    assert_eq!(left.1.phase, LocomotionPhase::Walk);
    assert_eq!(left.1.velocity.y, 0.0);
    near(left.0.y, 0.5, 0.05);
}
