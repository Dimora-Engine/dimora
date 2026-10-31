//! Slope traversal through `Game::step`. Geometry lives in the engine-world slope tests.
//!
//! The fixed clock is 60 Hz. A frame with no fixed iteration does not move and still drops a
//! locomotion intent. Two fixed iterations in one frame match two frames. A sprite child and a
//! mesh child share the gameplay root. Cameras do not change the step.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use engine::core::InputEvent;
use engine::prelude::*;

const RADIUS: f32 = 0.5;

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

fn ramp(world: &mut World) -> (Entity, Vec3) {
    let rotation = Quat::from_rotation_z(30.0_f32.to_radians());
    let normal = rotation.rotate(Vec3::Y).normalized();
    let ground = world.spawn((
        Transform::from_position(-normal * 2.0).with_rotation(rotation),
        Collider::aabb_box(Vec3::new(40.0, 2.0, 16.0)),
    ));
    (ground, normal * RADIUS)
}

fn game_on_slope() -> (Game, Entity, Vec3) {
    let mut game = Game::new(GameConfig::default());
    let (_ground, rest) = ramp(&mut game.world);
    let body = game.world.spawn((
        Transform::from_position(rest),
        Collider::sphere(RADIUS),
        CharacterMotor::default(),
        kinematic(),
        CharacterMotion::default(),
    ));
    (game, body, rest)
}

fn position(game: &Game, body: Entity) -> Vec3 {
    game.world.component::<Transform>(body).unwrap().position
}

fn motion(game: &Game, body: Entity) -> CharacterMotion {
    *game.world.component::<CharacterMotion>(body).unwrap()
}

fn watch(game: &mut Game) -> Rc<RefCell<Vec<LocomotionEvent>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let slot = seen.clone();
    game.add_system("landings", move |world| {
        slot.borrow_mut().extend(world.events::<LocomotionEvent>());
    });
    seen
}

#[test]
fn a_frame_with_no_fixed_step_does_not_move_on_a_slope() {
    let (mut game, body, rest) = game_on_slope();
    game.world.insert(body, (LocomotionIntent::new(Vec3::X),)).unwrap();
    game.step(Duration::ZERO);
    assert_eq!(position(&game, body), rest);
    assert_eq!(motion(&game, body).support, GroundSupport::Unknown);
    assert_eq!(motion(&game, body).velocity, Vec3::ZERO);
    assert!(game.world.component::<LocomotionIntent>(body).is_err());
    assert!(game.world.component::<MoveIntent>(body).is_err());
}

#[test]
fn several_fixed_iterations_climb_the_same_slope() {
    let climb = |frames: u32, span: u32| {
        let (mut game, body, rest) = game_on_slope();
        game.add_fixed_system("wish", move |world| {
            let _ = world.insert(body, (LocomotionIntent::new(Vec3::X),));
        });
        let step = fixed_step(&game);
        for _ in 0..frames {
            game.step(step * span);
        }
        (position(&game, body), motion(&game, body), rest)
    };
    let (one, one_motion, rest) = climb(1, 1);
    let (two_frames, two_motion, _) = climb(2, 1);
    let (one_frame, one_frame_motion, _) = climb(1, 2);
    assert!(one.y > rest.y + 0.5, "one step did not climb {one:?}");
    assert_eq!(two_frames, one_frame);
    assert_eq!(two_motion.velocity, one_frame_motion.velocity);
    assert_eq!(two_motion.phase, one_frame_motion.phase);
    assert_eq!(two_motion.support, one_frame_motion.support);
    assert_eq!(one_motion.phase, LocomotionPhase::Walk);
    assert_eq!(one_motion.velocity.y, 0.0);
    assert!((one_motion.velocity.x - 77.9).abs() < 2.0, "{:?}", one_motion.velocity);
    let two_steps = (two_frames - rest).length();
    let one_step = (one - rest).length();
    assert!((two_steps - one_step * 2.0).abs() < 1e-3, "{one_step} {two_steps}");
}

#[test]
fn a_sprite_and_a_mesh_climb_the_same_slope() {
    let climb = |visual: &str| {
        let (mut game, body, _) = game_on_slope();
        match visual {
            "sprite" => {
                game.world.spawn_child(body, (Transform::IDENTITY, Sprite::new("hero")));
            }
            "mesh" => {
                game.world.spawn_child(body, (Transform::IDENTITY, Mesh::new("hero")));
            }
            _ => {}
        }
        game.add_fixed_system("wish", move |world| {
            let _ = world.insert(body, (LocomotionIntent::new(Vec3::X),));
        });
        game.step(fixed_step(&game));
        (position(&game, body), motion(&game, body))
    };
    let sprite = climb("sprite");
    let mesh = climb("mesh");
    assert_eq!(sprite.0, mesh.0);
    assert_eq!(sprite.1.velocity, mesh.1.velocity);
    assert_eq!(sprite.1.phase, mesh.1.phase);
    assert_eq!(sprite.1.support, mesh.1.support);
    assert!(sprite.0.y > RADIUS);
    assert_eq!(sprite.1.phase, LocomotionPhase::Walk);
}

#[test]
fn cameras_do_not_change_slope_movement() {
    let climb = |cameras: bool| {
        let (mut game, body, _) = game_on_slope();
        if cameras {
            game.world.spawn((Transform::from_xyz(0.0, 8.0, 12.0), Camera::perspective(50.0)));
            game.world.spawn((Transform::from_xyz(4.0, 6.0, 0.0), Camera::orthographic(20.0)));
        }
        game.add_fixed_system("wish", move |world| {
            let _ = world.insert(body, (LocomotionIntent::new(Vec3::new(0.5, 3.0, -1.0)),));
        });
        game.step(fixed_step(&game) * 3);
        (position(&game, body), motion(&game, body))
    };
    let plain = climb(false);
    let viewed = climb(true);
    assert_eq!(plain.0, viewed.0);
    assert_eq!(plain.1.velocity, viewed.1.velocity);
    assert_eq!(plain.1.phase, viewed.1.phase);
    assert_eq!(plain.1.support, viewed.1.support);
    assert_eq!(plain.1.phase, LocomotionPhase::Walk);
    assert_eq!(plain.1.velocity.y, 0.0);
    assert!(plain.1.velocity.length() > 1.0, "{:?}", plain.1.velocity);
}

#[test]
fn a_jump_from_a_slope_lands_once_through_the_frame() {
    let (mut game, body, rest) = game_on_slope();
    game.world.resource_mut::<Actions>().bind_button("jump", ButtonSource::Key(Key::Space));
    game.add_fixed_system("drive", move |world| {
        let jump = world.resource::<Actions>().fixed_pressed("jump");
        let _ = world.insert(body, (LocomotionIntent { wish: Vec3::ZERO, jump, run: false },));
    });
    let seen = watch(&mut game);
    let step = fixed_step(&game);
    game.step(step);
    assert!(matches!(motion(&game, body).support, GroundSupport::Supported { .. }));
    game.send_input(InputEvent::Key { key: Key::Space, pressed: true });
    game.step(step);
    let takeoff = position(&game, body);
    assert!(takeoff.y > rest.y + 2.0, "{takeoff:?}");
    assert_eq!(motion(&game, body).phase, LocomotionPhase::Jump);
    assert_eq!(motion(&game, body).support, GroundSupport::Airborne);
    assert!((motion(&game, body).velocity.y - 200.0).abs() < 1e-3);
    assert!(seen.borrow().is_empty(), "takeoff is not a landing");

    game.send_input(InputEvent::Key { key: Key::Space, pressed: false });
    let mut landed = false;
    for _ in 0..20 {
        game.step(step * 4);
        if matches!(motion(&game, body).support, GroundSupport::Supported { .. }) {
            landed = true;
            break;
        }
    }
    assert!(landed, "never landed at {:?}", position(&game, body));
    assert_eq!(*seen.borrow(), vec![LocomotionEvent::Landed { entity: body }]);
    assert_eq!(motion(&game, body).velocity, Vec3::ZERO);
    assert_ne!(motion(&game, body).phase, LocomotionPhase::Fall);
    let gap = position(&game, body).dot(rest.normalized()) - RADIUS;
    assert!(gap.abs() < 0.08, "settled off the slope, gap {gap}");
    let count = seen.borrow().len();
    game.step(step);
    assert_eq!(seen.borrow().len(), count, "staying on the slope landed again");
}
