//! Window-control requests, focus loss, and pointer deltas. No window and no GPU.

use engine::core::{FixedTimestep, InputEvent};
use engine::prelude::*;

#[test]
fn config_seeds_the_request_and_leaves_the_effective_mode_unset() {
    let game = Game::new(GameConfig::default().with_fullscreen(true).with_vsync(false).with_max_frame_rate(Some(30)));
    let control = game.world.resource::<WindowControl>();
    assert!(control.fullscreen);
    assert!(!control.vsync);
    assert_eq!(control.max_frame_rate, Some(30));
    assert!(control.cursor_visible);
    assert!(!control.cursor_grabbed);
    let state = game.world.resource::<WindowState>();
    assert!(state.fullscreen);
    assert!(!state.vsync_requested);
    assert_eq!(state.present_mode, None);
    assert_eq!(state.max_frame_rate, Some(30));
    assert_eq!(state.cursor_grab, CursorGrab::Free);
    assert!(!state.grab_failed);

    let disabled = Game::new(GameConfig::default().with_max_frame_rate(Some(0)));
    assert_eq!(disabled.world.resource::<WindowControl>().max_frame_rate, None);
    assert_eq!(disabled.world.resource::<WindowState>().max_frame_rate, None);
    assert!(disabled.world.resource::<WindowControl>().vsync);
}

#[test]
fn focus_loss_releases_actions_and_the_fixed_latch_waits_for_the_next_step() {
    let mut game = Game::new(GameConfig::default());
    game.world.insert_resource(Vec::<bool>::new());
    {
        let mut actions = game.world.resource_mut::<Actions>();
        actions.bind_button("jump", ButtonSource::Key(Key::Space));
        actions.bind_button("jump", ButtonSource::Mouse(MouseButton::Left));
    }
    game.add_fixed_system("see_release", |world| {
        let saw = world.resource::<Actions>().fixed_released("jump")
            && world.resource::<Input>().fixed_key_released(Key::Space)
            && world.resource::<Input>().fixed_mouse_released(MouseButton::Left);
        world.resource_mut::<Vec<bool>>().push(saw);
    });

    game.send_input(InputEvent::Key { key: Key::Space, pressed: true });
    game.send_input(InputEvent::MouseButton { button: MouseButton::Left, pressed: true });
    game.send_input(InputEvent::MouseMoved(Vec2::new(10.0, 4.0)));
    assert!(game.world.resource::<Actions>().down(&game.world.resource::<Input>(), "jump"));
    assert_ne!(game.world.resource::<Input>().mouse_delta(), Vec2::ZERO);

    let step = FixedTimestep::new(60).step();
    game.step(step);
    assert!(game.world.resource::<Input>().key_down(Key::Space));
    assert!(game.world.resource::<Input>().mouse_down(MouseButton::Left));
    game.world.resource_mut::<Vec<bool>>().clear();

    game.send_input(InputEvent::FocusLost);
    assert!(!game.world.resource::<Input>().key_down(Key::Space));
    assert!(!game.world.resource::<Input>().mouse_down(MouseButton::Left));
    assert!(game.world.resource::<Input>().key_released(Key::Space));
    assert!(game.world.resource::<Input>().mouse_released(MouseButton::Left));
    assert!(!game.world.resource::<Input>().fixed_key_released(Key::Space));
    assert_eq!(game.world.resource::<Input>().mouse_delta(), Vec2::ZERO);
    assert!(game.world.resource::<Actions>().released("jump"));
    assert!(!game.world.resource::<Actions>().down(&game.world.resource::<Input>(), "jump"));
    assert!(!game.world.resource::<Actions>().fixed_released("jump"));

    game.step(step);
    assert_eq!(game.world.resource::<Vec<bool>>().as_slice(), &[true]);
    assert!(!game.world.resource::<Actions>().fixed_released("jump"));
    assert!(!game.world.resource::<Input>().key_released(Key::Space));
    assert!(!game.world.resource::<Input>().key_down(Key::Space));

    game.send_input(InputEvent::FocusGained);
    game.send_input(InputEvent::MouseMoved(Vec2::new(80.0, 10.0)));
    assert_eq!(game.world.resource::<Input>().mouse_position(), Vec2::new(80.0, 10.0));
    assert_eq!(game.world.resource::<Input>().mouse_delta(), Vec2::ZERO);
    game.send_input(InputEvent::MouseMoved(Vec2::new(83.0, 12.0)));
    assert_eq!(game.world.resource::<Input>().mouse_delta(), Vec2::new(3.0, 2.0));
    assert!(!game.world.resource::<WindowControl>().cursor_grabbed);
    assert_eq!(game.world.resource::<WindowState>().cursor_grab, CursorGrab::Free);

    game.send_input(InputEvent::FocusLost);
    assert!(!game.world.resource::<Actions>().released("jump"));
    assert!(!game.world.resource::<Input>().key_released(Key::Space));
    assert!(!game.world.resource::<Input>().fixed_key_released(Key::Space));
}
