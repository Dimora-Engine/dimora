use std::sync::Arc;
use std::time::Duration;

use engine_assets::AssetManager;
use engine_core::InputEvent;
use engine_platform::winit::event_loop::OwnedDisplayHandle;
use engine_platform::winit::window::Window;
use engine_platform::{App, Flow};
use engine_renderer_wgpu::WindowRenderer;

use crate::diagnostics::Diagnostics;
use crate::game::{Game, RenderStats, WindowControl};
use crate::logging::{log_render_failure, log_window_init_failure};
use crate::viewport::Viewport;

pub fn run(game: Game) -> Result<(), Box<dyn std::error::Error>> {
    let window_config = game.config.window.clone();
    let mut app = WindowedGame { game, window: None, renderer: None, error: None };
    engine_platform::run(window_config, &mut app)?;
    app.error.map_or(Ok(()), Err)
}

struct WindowedGame {
    game: Game,
    window: Option<Arc<Window>>,
    renderer: Option<WindowRenderer>,
    error: Option<Box<dyn std::error::Error>>,
}

impl App for WindowedGame {
    fn init(&mut self, window: Arc<Window>, display: OwnedDisplayHandle) {
        let size = window.inner_size();
        let scale = window.scale_factor();
        match WindowRenderer::new(window.clone(), Some(Box::new(display)), (size.width, size.height)) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                log_window_init_failure(&e);
                self.error = Some(Box::new(e));
            }
        }
        self.window = Some(window);
        self.scale_factor(scale);
    }

    fn input(&mut self, event: InputEvent) {
        self.game.send_input(event);
    }

    fn resize(&mut self, width: u32, height: u32) {
        if let Some(r) = &mut self.renderer {
            r.resize(width, height);
        }
        self.game.world.resource_mut::<Viewport>().apply_physical_size(width, height);
    }

    fn scale_factor(&mut self, scale: f64) {
        let mut viewport = self.game.world.resource_mut::<Viewport>();
        // `false` is also "already this scale". A duplicate event must not warn or resize.
        if !viewport.apply_scale_factor(scale) && viewport.scale_factor() != scale {
            log::warn!("ignored window scale factor {scale}");
        }
    }

    fn frame(&mut self, dt: Duration) -> Flow {
        if self.error.is_some() {
            return Flow::Exit;
        }
        self.game.step(dt);

        let (exit, title) = {
            let mut control = self.game.world.resource_mut::<WindowControl>();
            (control.exit, control.title.take())
        };
        if let (Some(title), Some(window)) = (title, &self.window) {
            window.set_title(&title);
        }

        if let (Some(renderer), Some(scene)) = (&mut self.renderer, self.game.extract()) {
            let assets = self.game.world.resource::<AssetManager>();
            if let Err(e) = renderer.render(&scene, &assets) {
                log_render_failure(&e);
                self.error = Some(Box::new(e));
                return Flow::Exit;
            }
            drop(assets);
            let stats = RenderStats {
                extract: scene.stats,
                frame: renderer.scene.stats(),
                cache: renderer.scene.cache_stats(),
            };
            *self.game.world.resource_mut::<RenderStats>() = stats;
            self.game.world.resource_mut::<Diagnostics>().render = stats;
        }
        if exit {
            Flow::Exit
        } else {
            Flow::Continue
        }
    }
}
