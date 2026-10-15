use std::sync::Arc;
use std::time::{Duration, Instant};

use engine_assets::AssetManager;
use engine_core::InputEvent;
use engine_platform::winit::dpi::LogicalSize;
use engine_platform::winit::event_loop::OwnedDisplayHandle;
use engine_platform::winit::window::{CursorGrabMode, Fullscreen, Window};
use engine_platform::{frame_deadline, App, Flow};
use engine_renderer_wgpu::WindowRenderer;

use crate::diagnostics::Diagnostics;
use crate::game::{Game, RenderStats, WindowControl};
use crate::logging::{log_render_failure, log_window_init_failure};
use crate::viewport::Viewport;
use crate::window::{
    accept_pointer, logical_window_size, pointer_delta_source, reconcile_grab, CursorGrab, FullscreenStep,
    FullscreenToggle, PointerDeltaSource,
};
use crate::PresentMode;

pub fn run(game: Game) -> Result<(), Box<dyn std::error::Error>> {
    let window_config = game.config.window.clone();
    let fullscreen = FullscreenToggle::new(window_config.fullscreen, (window_config.width, window_config.height));
    let mut app = WindowedGame {
        game,
        window: None,
        renderer: None,
        error: None,
        fullscreen,
        applied_vsync: None,
        applied_cursor_visible: None,
        suppress_cursor_delta: false,
        present_fallback_logged: false,
    };
    engine_platform::run(window_config, &mut app)?;
    app.error.map_or(Ok(()), Err)
}

struct WindowedGame {
    game: Game,
    window: Option<Arc<Window>>,
    renderer: Option<WindowRenderer>,
    error: Option<Box<dyn std::error::Error>>,
    fullscreen: FullscreenToggle,
    /// `None` until the surface has been asked for a present mode.
    applied_vsync: Option<bool>,
    applied_cursor_visible: Option<bool>,
    /// One cursor-position event after a grab change is stored without a delta.
    suppress_cursor_delta: bool,
    present_fallback_logged: bool,
}

impl WindowedGame {
    fn apply_window(&mut self) {
        let (title, fullscreen, vsync, cap, cursor_visible, cursor_grabbed) = {
            let mut control = self.game.world.resource_mut::<WindowControl>();
            let title = control.title.take();
            (
                title,
                control.fullscreen,
                control.vsync,
                control.max_frame_rate,
                control.cursor_visible,
                control.cursor_grabbed,
            )
        };
        if let (Some(title), Some(window)) = (title, &self.window) {
            window.set_title(&title);
        }
        self.apply_fullscreen(fullscreen);
        self.apply_present(vsync);
        self.apply_cursor(cursor_visible, cursor_grabbed, true, false);
        self.game.world.resource_mut::<crate::WindowState>().max_frame_rate = cap.filter(|hz| *hz > 0);
    }

    fn apply_fullscreen(&mut self, requested: bool) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let logical = {
            let viewport = self.game.world.resource::<Viewport>();
            logical_window_size(viewport.physical_width(), viewport.physical_height(), viewport.scale_factor())
        };
        match self.fullscreen.reconcile(requested, logical) {
            FullscreenStep::Unchanged => {}
            FullscreenStep::Enter => window.set_fullscreen(Some(Fullscreen::Borderless(None))),
            FullscreenStep::Exit { logical_size } => {
                window.set_fullscreen(None);
                // `Some` means the size was applied immediately and `Resized` may not follow.
                let applied = window.request_inner_size(LogicalSize::new(logical_size.0 as f64, logical_size.1 as f64));
                if let Some(size) = applied {
                    self.resize(size.width, size.height);
                }
            }
        }
        self.game.world.resource_mut::<crate::WindowState>().fullscreen = self.fullscreen.active();
    }

    fn apply_present(&mut self, requested: bool) {
        let configured = self.game.world.resource::<crate::WindowState>().present_mode.is_some();
        if self.applied_vsync == Some(requested) && configured {
            return;
        }
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let effective = renderer.apply_vsync(requested);
        let request_changed = self.applied_vsync != Some(requested);
        self.applied_vsync = Some(requested);
        if request_changed {
            self.present_fallback_logged = false;
        }
        let preferred = if requested { PresentMode::Fifo } else { PresentMode::Immediate };
        if effective != preferred && !self.present_fallback_logged {
            log::info!(
                "presentation for vsync {requested} is {effective:?}; preferred {preferred:?} was not available"
            );
            self.present_fallback_logged = true;
        }
        let mut state = self.game.world.resource_mut::<crate::WindowState>();
        state.vsync_requested = requested;
        state.present_mode = Some(effective);
    }

    /// `suppress_warp` is set for a grab change the application asked for. Focus loss passes
    /// `false`: [`InputEvent::FocusGained`] already ignores the next position delta, and setting
    /// both would drop two moves.
    fn apply_cursor(&mut self, visible: bool, requested: bool, suppress_warp: bool, force_visible: bool) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let previous = *self.game.world.resource::<crate::WindowState>();
        let update = reconcile_grab(previous.cursor_grab, previous.grab_failed, requested, |mode| {
            set_grab(&window, mode).is_ok()
        });
        if requested && update.failed {
            log::warn!("cursor grab failed; locked and confined were both rejected");
            self.game.world.resource_mut::<WindowControl>().release_grab_request();
        }
        let mut grab = update.grab;
        let mut changed = update.changed;
        if changed && grab == CursorGrab::Free && set_grab(&window, CursorGrab::Free).is_err() {
            log::warn!("releasing the cursor grab failed");
            grab = previous.cursor_grab;
            changed = false;
        }
        if changed && suppress_warp {
            self.suppress_cursor_delta = true;
        }
        if force_visible || changed || self.applied_cursor_visible != Some(visible) {
            window.set_cursor_visible(visible);
            self.applied_cursor_visible = Some(visible);
        }
        let mut state = self.game.world.resource_mut::<crate::WindowState>();
        state.cursor_grab = grab;
        state.grab_failed = if requested || changed { update.failed } else { previous.grab_failed };
        state.cursor_visible = visible;
    }
}

fn set_grab(window: &Window, mode: CursorGrab) -> Result<(), engine_platform::winit::error::ExternalError> {
    let mode = match mode {
        CursorGrab::Free => CursorGrabMode::None,
        CursorGrab::Locked => CursorGrabMode::Locked,
        CursorGrab::Confined => CursorGrabMode::Confined,
    };
    window.set_cursor_grab(mode)
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
        match event {
            InputEvent::FocusLost => {
                let visible = {
                    let mut control = self.game.world.resource_mut::<WindowControl>();
                    control.release_grab_request();
                    control.cursor_visible
                };
                self.apply_cursor(visible, false, false, true);
                self.game.send_input(event);
            }
            InputEvent::FocusGained => self.game.send_input(event),
            other => {
                let grabbed = pointer_delta_source(self.game.world.resource::<crate::WindowState>().cursor_grab)
                    == PointerDeltaSource::DeviceMotion;
                if let Some(event) = accept_pointer(grabbed, &mut self.suppress_cursor_delta, other) {
                    self.game.send_input(event);
                }
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(width, height);
        }
        let applied = {
            let mut viewport = self.game.world.resource_mut::<Viewport>();
            let applied = viewport.apply_physical_size(width, height);
            let scale = viewport.scale_factor();
            (applied, scale)
        };
        if applied.0 {
            self.fullscreen.note_windowed_size(logical_window_size(width, height, applied.1));
        }
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
        let frame_start = Instant::now();
        self.game.step(dt);
        self.apply_window();

        let exit = self.game.world.resource::<WindowControl>().exit;
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
            return Flow::Exit;
        }
        let cap = self.game.world.resource::<crate::WindowState>().max_frame_rate;
        match frame_deadline(frame_start, cap) {
            Some(deadline) => Flow::WaitUntil(deadline),
            None => Flow::Continue,
        }
    }
}
