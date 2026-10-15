//! Window creation, the event loop, input translation and frame timing.
//!
//! This crate knows about winit and nothing about rendering or gameplay. It turns OS events into
//! [`engine_core::InputEvent`]s and calls an [`App`] once per frame with the elapsed time.

mod keymap;

use std::sync::Arc;
use std::time::{Duration, Instant};

use engine_core::{InputEvent, MouseButton};
use engine_math::Vec2;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::window::{Fullscreen, Window, WindowId};

pub use winit;

#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    /// Initial inner size in logical pixels.
    pub width: u32,
    pub height: u32,
    /// Create the window borderless fullscreen on the current monitor.
    pub fullscreen: bool,
    /// Request vertical sync. The surface may still present with Fifo when it cannot disable it.
    pub vsync: bool,
    /// Target frames per second. `None` or `Some(0)` leaves the loop polling every frame.
    ///
    /// This is a scheduling target for the event loop. It does not change `Game::step` and it
    /// does not guarantee the display presents at that rate. VSync can still hold a frame for a
    /// blanking interval.
    pub max_frame_rate: Option<u32>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "2.5D engine".into(),
            width: 1280,
            height: 720,
            fullscreen: false,
            vsync: true,
            max_frame_rate: None,
        }
    }
}

/// When the next frame should run.
///
/// `Continue` polls and requests a redraw immediately. `WaitUntil` waits without a blocking
/// sleep, and input events still wake the loop. `Exit` stops the loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    WaitUntil(Instant),
    Exit,
}

/// Deadline for the next frame, or `None` when the cap is off.
///
/// Zero and `None` both disable the cap. The deadline is `now + 1/hz`, truncated to a whole
/// number of nanoseconds. It is a scheduling target, not a presentation guarantee.
pub fn frame_deadline(now: Instant, cap_hz: Option<u32>) -> Option<Instant> {
    let hz = cap_hz.filter(|hz| *hz > 0)?;
    Some(now + Duration::from_secs(1) / hz)
}

/// What the platform layer drives. Everything happens on the main thread.
pub trait App {
    /// Called once, when the window exists. `display` is needed by some graphics backends
    /// (Wayland) to create their instance.
    fn init(&mut self, window: Arc<Window>, display: OwnedDisplayHandle);
    fn input(&mut self, event: InputEvent);
    /// Inner size in physical pixels changed (also reported once after `init`).
    /// A zero width or height is a minimized window; the implementor decides whether to keep
    /// the previous framebuffer.
    fn resize(&mut self, width: u32, height: u32);
    /// The display scale changed. This does not itself change the physical size. The following
    /// [`App::resize`] carries the framebuffer the OS chose. Do not multiply that size by the scale.
    fn scale_factor(&mut self, scale: f64);
    /// Once per display frame. `dt` is the wall-clock time since the previous frame.
    fn frame(&mut self, dt: Duration) -> Flow;
}

#[derive(Debug)]
pub struct PlatformError(pub String);

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "platform error: {}", self.0)
    }
}
impl std::error::Error for PlatformError {}

/// Opens a window and runs `app` until it asks to exit or the window is closed.
pub fn run(config: WindowConfig, app: &mut dyn App) -> Result<(), PlatformError> {
    let event_loop = EventLoop::new().map_err(|e| PlatformError(e.to_string()))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut runner = Runner {
        config,
        app,
        window: None,
        last_frame: None,
        next_frame: None,
        keys: keymap::HeldKeys::default(),
        error: None,
    };
    event_loop.run_app(&mut runner).map_err(|e| PlatformError(e.to_string()))?;
    runner.error.map_or(Ok(()), |e| Err(PlatformError(e)))
}

struct Runner<'a> {
    config: WindowConfig,
    app: &'a mut dyn App,
    window: Option<Arc<Window>>,
    last_frame: Option<Instant>,
    /// `None` polls continuously. `Some` is the earliest time to request the next redraw.
    next_frame: Option<Instant>,
    keys: keymap::HeldKeys,
    error: Option<String>,
}

impl ApplicationHandler for Runner<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height));
        if self.config.fullscreen {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                self.app.init(window.clone(), event_loop.owned_display_handle());
                let size = window.inner_size();
                self.app.resize(size.width, size.height);
                window.request_redraw();
                self.window = Some(window);
            }
            Err(e) => {
                self.error = Some(format!("could not create window: {e}"));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.app.resize(size.width, size.height),
            // Leave the suggested size in place. The OS then emits `Resized` with that physical
            // size. Requesting a size here would apply the scale a second time.
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.app.scale_factor(scale_factor),
            WindowEvent::KeyboardInput { event, .. } if !event.repeat => {
                if let Some(key) = keymap::map_key(event.physical_key) {
                    let pressed = event.state == ElementState::Pressed;
                    if let Some(pressed) = self.keys.update(key, pressed) {
                        self.app.input(InputEvent::Key { key, pressed });
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                self.app.input(InputEvent::MouseButton { button, pressed: state == ElementState::Pressed });
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.app.input(InputEvent::MouseMoved(Vec2::new(position.x as f32, position.y as f32)));
            }
            WindowEvent::Focused(false) => {
                // Both physical sides of a collapsed modifier drop together, so the logical
                // key cannot stay down after the matching `FocusLost` release.
                self.keys.clear();
                self.app.input(InputEvent::FocusLost);
            }
            WindowEvent::Focused(true) => self.app.input(InputEvent::FocusGained),
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.app.input(InputEvent::Scroll(y));
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                // Clamp so a debugger pause or window drag does not teleport the simulation.
                let dt = self.last_frame.map_or(Duration::ZERO, |t| now - t).min(Duration::from_millis(250));
                self.last_frame = Some(now);
                match self.app.frame(dt) {
                    Flow::Exit => event_loop.exit(),
                    Flow::Continue => self.next_frame = None,
                    Flow::WaitUntil(deadline) => self.next_frame = Some(deadline),
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: DeviceId, event: DeviceEvent) {
        // Relative motion only. The engine drops this while the cursor is free, so it is not
        // added to cursor-position deltas. Units are whatever winit reports for this device.
        if let DeviceEvent::MouseMotion { delta: (x, y) } = event {
            self.app.input(InputEvent::MouseDelta(Vec2::new(x as f32, y as f32)));
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = &self.window else {
            return;
        };
        match self.next_frame {
            Some(deadline) if Instant::now() < deadline => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
            Some(_) => {
                self.next_frame = None;
                event_loop.set_control_flow(ControlFlow::Poll);
                window.request_redraw();
            }
            None => {
                event_loop.set_control_flow(ControlFlow::Poll);
                window.request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_cap_waits_one_period_and_zero_disables_it() {
        let now = Instant::now();
        assert_eq!(frame_deadline(now, None), None);
        assert_eq!(frame_deadline(now, Some(0)), None);
        assert_eq!(frame_deadline(now, Some(60)), Some(now + Duration::from_secs(1) / 60));
        let waited = frame_deadline(now, Some(30)).unwrap() - now;
        assert_eq!(waited, Duration::from_secs(1) / 30);
    }
}
