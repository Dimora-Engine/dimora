//! Facade sample-count request. Activation is checked when a renderer exists.

use engine::prelude::*;
use engine::renderer_wgpu::RenderError;

#[test]
fn the_request_is_not_reported_as_active_before_a_renderer_exists() {
    let mut game = Game::new(GameConfig::default());
    assert_eq!(game.msaa(), MsaaSamples::One);
    assert_eq!(game.msaa4_available(), None);
    game.set_msaa(MsaaSamples::Four).unwrap();
    assert_eq!(game.msaa(), MsaaSamples::One, "a stored request is not an active 4× renderer");
    game.set_msaa(MsaaSamples::One).unwrap();
    assert_eq!(game.msaa(), MsaaSamples::One);
}

#[test]
fn the_headless_renderer_applies_a_supported_four_sample_request() {
    let mut game = Game::new(GameConfig::default());
    game.world.spawn((Transform::from_xyz(0.0, 0.0, -4.0), Camera::orthographic(2.0)));
    game.step(std::time::Duration::from_millis(16));
    game.set_msaa(MsaaSamples::Four).unwrap();
    match game.render_to_rgba(32, 32) {
        Ok(_) => {
            assert_eq!(game.msaa(), MsaaSamples::Four);
            assert_eq!(game.msaa4_available(), Some(true));
            game.set_msaa(MsaaSamples::One).unwrap();
            assert_eq!(game.msaa(), MsaaSamples::One);
        }
        Err(RenderError::NoAdapter(message)) if std::env::var("DIMORA_REQUIRE_GPU").ok().as_deref() != Some("1") => {
            eprintln!("skipping Game MSAA apply test: {message}");
        }
        Err(err) => panic!("render failed: {err}"),
    }
}
