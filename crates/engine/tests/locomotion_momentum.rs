//! Acceleration through `Game::step`. Geometry lives in the engine-world momentum tests.
//!
//! The fixed clock is 60 Hz. A frame with no fixed iteration does not steer. Two fixed iterations
//! add two acceleration steps. A sprite child and a mesh child share the gameplay root.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use engine::core::InputEvent;
use engine::prelude::*;

fn fixed_step(game: &Game) -> Duration {
    game.world.resource::<FixedTimestep>().step()
}

fn game_with_walker() -> (Game, Entity) {
    let mut game = Game::new(GameConfig::default());
    game.world.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(400.0, 0.5, 40.0))));
    let body = game.world.spawn((
        Transform::from_xyz(0.0, 0.5, 0.0),
        Collider::sphere(0.5),
        CharacterMotor::default(),
        CharacterLocomotion::default(),
        CharacterMotion::default(),
    ));
    (game, body)
}

fn position(game: &Game, entity: Entity) -> Vec3 {
    game.world.component::<Transform>(entity).unwrap().position
}

fn motion(game: &Game, entity: Entity) -> CharacterMotion {
    *game.world.component::<CharacterMotion>(entity).unwrap()
}

#[test]
fn two_fixed_iterations_accelerate_twice_and_a_zero_frame_does_not() {
    let (mut game, body) = game_with_walker();
    game.add_fixed_system("wish", move |world| {
        let _ = world.insert(body, (LocomotionIntent::running(Vec3::X),));
    });
    let step = fixed_step(&game);
    game.step(Duration::ZERO);
    assert_eq!(position(&game, body), Vec3::new(0.0, 0.5, 0.0));
    assert_eq!(motion(&game, body).velocity, Vec3::ZERO);
    assert_eq!(motion(&game, body).support, GroundSupport::Unknown);

    game.step(step);
    let once = motion(&game, body).velocity.x;
    assert!((once - 10.0).abs() < 1e-3, "{once}");
    game.step(step * 2);
    let twice = motion(&game, body).velocity.x;
    assert!((twice - once - 20.0).abs() < 1e-2, "{once} {twice}");
    assert_eq!(motion(&game, body).velocity.y, 0.0);
    assert_eq!(motion(&game, body).phase, LocomotionPhase::Walk);
}

#[test]
fn a_sprite_and_a_mesh_and_a_camera_share_the_accelerated_root() {
    let play = |visual: &str, camera: bool| {
        let (mut game, body) = game_with_walker();
        match visual {
            "sprite" => {
                game.world.spawn_child(body, (Transform::IDENTITY, Sprite::new("hero")));
            }
            "mesh" => {
                game.world.spawn_child(body, (Transform::IDENTITY, Mesh::new("hero")));
            }
            _ => {}
        }
        if camera {
            game.world.spawn((Transform::from_xyz(0.0, 8.0, 12.0), Camera::perspective(50.0)));
            game.world.spawn((Transform::from_xyz(4.0, 6.0, 0.0), Camera::orthographic(20.0)));
        }
        game.add_fixed_system("wish", move |world| {
            let _ = world.insert(body, (LocomotionIntent::new(Vec3::new(1.0, 2.0, 1.0)),));
        });
        game.step(fixed_step(&game) * 4);
        (position(&game, body), motion(&game, body))
    };
    let plain = play("none", false);
    let sprite = play("sprite", false);
    let mesh = play("mesh", true);
    assert_eq!(sprite.0, mesh.0);
    assert_eq!(sprite.0, plain.0);
    assert_eq!(sprite.1.velocity, mesh.1.velocity);
    assert_eq!(sprite.1.phase, LocomotionPhase::Walk);
    assert!(sprite.1.velocity.x > 20.0 && (sprite.1.velocity.x - sprite.1.velocity.z).abs() < 1e-3);
}

#[test]
fn one_landing_is_visible_after_a_running_jump() {
    let (mut game, body) = game_with_walker();
    game.world.resource_mut::<Actions>().bind_button("jump", ButtonSource::Key(Key::Space));
    game.add_fixed_system("drive", move |world| {
        let jump = world.resource::<Actions>().fixed_pressed("jump");
        let _ = world.insert(body, (LocomotionIntent { wish: Vec3::X, jump, run: true },));
    });
    let seen = Rc::new(RefCell::new(Vec::new()));
    let slot = seen.clone();
    game.add_system("landings", move |world| {
        slot.borrow_mut().extend(world.events::<LocomotionEvent>());
    });
    let step = fixed_step(&game);
    for _ in 0..2 {
        game.step(step * 8);
    }
    assert!(motion(&game, body).velocity.x > 100.0, "{:?}", motion(&game, body).velocity);
    game.send_input(InputEvent::Key { key: Key::Space, pressed: true });
    game.step(step);
    game.send_input(InputEvent::Key { key: Key::Space, pressed: false });
    assert_eq!(motion(&game, body).phase, LocomotionPhase::Jump);
    assert!(seen.borrow().is_empty(), "takeoff is not a landing");
    let speed = motion(&game, body).velocity.x;
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
    assert_eq!(motion(&game, body).velocity.y, 0.0);
    assert!((motion(&game, body).velocity.x - speed).abs() < 40.0, "{:?} from {speed}", motion(&game, body).velocity);
    let count = seen.borrow().len();
    game.step(step);
    assert_eq!(seen.borrow().len(), count, "staying down landed again");
}
