//! Locomotion through `Game::step`. The fixed clock is 60 Hz.

use std::time::Duration;

use engine::prelude::*;

/// One step still reaches the configured speed. The gameplay default accelerates.
fn kinematic() -> CharacterLocomotion {
    CharacterLocomotion {
        acceleration: 1.0e6,
        deceleration: 1.0e6,
        air_acceleration: 1.0e6,
        ..CharacterLocomotion::default()
    }
}

fn game_with_walker() -> (Game, Entity) {
    let mut game = Game::new(GameConfig::default());
    game.world.spawn((Transform::from_xyz(0.0, -0.5, 0.0), Collider::aabb_box(Vec3::new(8.0, 0.5, 8.0))));
    let body = game.world.spawn((
        Transform::from_xyz(0.0, 0.5, 0.0),
        Collider::sphere(0.5),
        CharacterMotor::default(),
        kinematic(),
        CharacterMotion::default(),
    ));
    (game, body)
}

fn fixed_step(game: &Game) -> Duration {
    game.world.resource::<FixedTimestep>().step()
}

#[test]
fn several_fixed_iterations_apply_the_fixed_delta_once_each() {
    let (mut once, once_body) = game_with_walker();
    once.add_fixed_system("wish", move |world| {
        let _ = world.insert(once_body, (LocomotionIntent::new(Vec3::X),));
    });
    once.step(fixed_step(&once));

    let (mut twice, twice_body) = game_with_walker();
    twice.add_fixed_system("wish", move |world| {
        let _ = world.insert(twice_body, (LocomotionIntent::new(Vec3::X),));
    });
    twice.step(fixed_step(&twice) * 2);

    let one = once.world.component::<Transform>(once_body).unwrap().position.x;
    let two = twice.world.component::<Transform>(twice_body).unwrap().position.x;
    assert!((one - 1.5).abs() < 1e-3, "{one}");
    assert!((two - one * 2.0).abs() < 1e-3, "{one} {two}");
}

#[test]
fn a_frame_with_no_fixed_step_does_not_change_support() {
    let (mut game, body) = game_with_walker();
    game.world.insert(body, (LocomotionIntent::new(Vec3::X),)).unwrap();
    game.step(Duration::ZERO);
    let position = game.world.component::<Transform>(body).unwrap().position;
    assert_eq!(position, Vec3::new(0.0, 0.5, 0.0));
    assert_eq!(game.world.component::<CharacterMotion>(body).unwrap().support, GroundSupport::Unknown);
    assert!(game.world.component::<LocomotionIntent>(body).is_err());
    assert!(game.world.component::<MoveIntent>(body).is_err());
}

#[test]
fn a_sprite_child_and_a_mesh_child_trace_the_same_root() {
    let (mut sprites, sprite_body) = game_with_walker();
    sprites.world.spawn_child(sprite_body, (Transform::IDENTITY, Sprite::new("hero")));
    sprites.add_fixed_system("wish", move |world| {
        let _ = world.insert(sprite_body, (LocomotionIntent::new(Vec3::X),));
    });
    sprites.step(fixed_step(&sprites));

    let (mut meshes, mesh_body) = game_with_walker();
    meshes.world.spawn_child(mesh_body, (Transform::IDENTITY, Mesh::new("cube")));
    meshes.add_fixed_system("wish", move |world| {
        let _ = world.insert(mesh_body, (LocomotionIntent::new(Vec3::X),));
    });
    meshes.step(fixed_step(&meshes));

    let sprite = sprites.world.component::<Transform>(sprite_body).unwrap().position;
    let mesh = meshes.world.component::<Transform>(mesh_body).unwrap().position;
    assert!((sprite.x - mesh.x).abs() < 1e-5 && (sprite.y - mesh.y).abs() < 1e-5 && (sprite.z - mesh.z).abs() < 1e-5);
    assert!((sprite.x - 1.5).abs() < 1e-3);
}

#[test]
fn a_camera_does_not_change_ground_support() {
    let (mut plain, plain_body) = game_with_walker();
    plain.add_fixed_system("wish", move |world| {
        let _ = world.insert(plain_body, (LocomotionIntent::new(Vec3::X),));
    });
    plain.step(fixed_step(&plain));

    let (mut viewed, viewed_body) = game_with_walker();
    viewed.world.spawn((Transform::from_xyz(0.0, 8.0, 12.0), Camera::perspective(50.0)));
    viewed.world.spawn((Transform::from_xyz(4.0, 6.0, 0.0), Camera::orthographic(20.0)));
    viewed.add_fixed_system("wish", move |world| {
        let _ = world.insert(viewed_body, (LocomotionIntent::new(Vec3::X),));
    });
    viewed.step(fixed_step(&viewed));

    let left = plain.world.component::<Transform>(plain_body).unwrap().position;
    let right = viewed.world.component::<Transform>(viewed_body).unwrap().position;
    assert_eq!(left, right);
    assert_eq!(
        plain.world.component::<CharacterMotion>(plain_body).unwrap().support,
        viewed.world.component::<CharacterMotion>(viewed_body).unwrap().support
    );
}
