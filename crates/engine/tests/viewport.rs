//! Viewport scale is independent of the framebuffer size and of raw input.

use std::time::Duration;

use engine::core::InputEvent;
use engine::math::Vec2;
use engine::prelude::*;

#[test]
fn extraction_uses_the_physical_size_at_every_scale() {
    let mut game = Game::new(GameConfig::default());
    game.world.spawn((Transform::from_xyz(0.0, 0.0, -4.0), Camera::perspective(60.0)));
    game.step(Duration::from_millis(16));

    *game.world.resource_mut::<Viewport>() = Viewport::new(800, 600, 2.0).unwrap();
    let hidpi = game.extract().expect("camera");
    *game.world.resource_mut::<Viewport>() = Viewport::new(800, 600, 1.5).unwrap();
    let other = game.extract().expect("camera");

    assert_eq!(hidpi.viewport_px, [0.0, 0.0, 800.0, 600.0]);
    assert_eq!(other.viewport_px, hidpi.viewport_px);
    assert_eq!(other.view.viewport, hidpi.view.viewport);
    assert_eq!(other.view.proj, hidpi.view.proj);
}

#[test]
fn mouse_position_stays_physical_and_logical_conversion_does_not_change_input() {
    let mut game = Game::new(GameConfig::default());
    game.send_input(InputEvent::Key { key: Key::W, pressed: true });
    game.send_input(InputEvent::MouseMoved(Vec2::new(100.0, 40.0)));
    *game.world.resource_mut::<Viewport>() = Viewport::new(200, 100, 2.0).unwrap();
    game.step(Duration::from_millis(16));

    let input = game.world.resource::<Input>();
    assert!(input.key_down(Key::W));
    assert_eq!(input.mouse_position(), Vec2::new(100.0, 40.0));
    let viewport = *game.world.resource::<Viewport>();
    assert_eq!(viewport.physical_to_logical(input.mouse_position()), Vec2::new(50.0, 20.0));
    assert_eq!(input.mouse_position(), Vec2::new(100.0, 40.0));
}
