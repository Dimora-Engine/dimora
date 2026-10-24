//! `Game::step` clears debug lines before systems. Extract after the step still sees them. No GPU.

use std::time::Duration;

use engine::core::FixedTimestep;
use engine::prelude::*;

fn fixed_step() -> Duration {
    FixedTimestep::new(60).step()
}

fn game() -> Game {
    let mut game = Game::new(GameConfig::default().with_size(64, 64));
    game.world.spawn((Camera::orthographic(4.0), Transform::from_xyz(0.0, 0.0, -8.0)));
    game.world.spawn((
        Camera { order: 1, viewport: ViewportRect::new(0.5, 0.0, 0.5, 1.0), ..Camera::orthographic(4.0) },
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    game
}

#[test]
fn submissions_survive_extract_and_do_not_accumulate() {
    let mut game = game();
    assert!(game.world.resource::<DebugDraw>().is_empty());

    game.world.resource_mut::<DebugDraw>().line(Vec3::ZERO, Vec3::Z, Color::WHITE);
    game.add_fixed_system("fixed-ray", |world| {
        world.resource_mut::<DebugDraw>().ray(Vec3::ZERO, Vec3::X, 2.0, Color::rgb(1.0, 0.0, 0.0));
    });
    game.add_system("update-line", |world| {
        assert_eq!(world.resource::<DebugDraw>().len(), 1, "the fixed step's ray is still there");
        world.resource_mut::<DebugDraw>().line(Vec3::ZERO, Vec3::Y, Color::rgb(0.0, 1.0, 0.0));
    });
    game.add_to_stage(Stage::PostUpdate, "late-box", |world| {
        world.resource_mut::<DebugDraw>().wire_box(Vec3::ZERO, Vec3::splat(0.5), Color::WHITE);
    })
    .unwrap();

    game.step(fixed_step());
    assert_eq!(game.world.resource::<DebugDraw>().len(), 1 + 1 + 12);
    let scene = game.extract().expect("cameras");
    assert_eq!(scene.passes.len(), 2);
    assert_eq!(scene.debug_lines.len(), 14);
    assert_eq!(scene.stats.debug_segments, 14);
    let again = game.extract().expect("second extract");
    assert_eq!(again.debug_lines.len(), 14, "extract does not consume the list");

    game.step(fixed_step());
    let scene = game.extract().expect("next frame");
    assert_eq!(scene.debug_lines.len(), 14, "the previous frame was cleared before systems ran");
}

#[test]
fn two_fixed_steps_keep_both_submissions_until_the_next_frame() {
    let mut game = game();
    game.add_fixed_system("fixed-line", |world| {
        world.resource_mut::<DebugDraw>().line(Vec3::ZERO, Vec3::X, Color::WHITE);
    });
    let step = fixed_step();
    game.step(step + step);
    assert_eq!(game.world.resource::<DebugDraw>().len(), 2);
    assert_eq!(game.extract().expect("cameras").debug_lines.len(), 2);

    game.step(step);
    assert_eq!(game.world.resource::<DebugDraw>().len(), 1);
}

#[test]
fn a_removed_resource_is_replaced_for_the_step() {
    let mut game = game();
    assert!(game.world.remove_resource::<DebugDraw>().is_some());
    game.add_system("draw", |world| {
        world.resource_mut::<DebugDraw>().line(Vec3::ZERO, Vec3::X, Color::WHITE);
    });
    game.step(fixed_step());
    assert_eq!(game.extract().expect("cameras").debug_lines.len(), 1);
}
