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
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

pub use winit;

#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    /// Initial inner size in logical pixels.
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { title: "2.5D engine".into(), width: 1280, height: 720 }
    }
}

/// Whether the loop should keep going after a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Exit,
}

/// What the platform layer drives. Everything happens on the main thread.
pub trait App {
    /// Called once, when the window exists. `display` is needed by some graphics backends
    /// (Wayland) to create their instance.
    fn init(&mut self, window: Arc<Window>, display: OwnedDisplayHandle);
    fn input(&mut self, event: InputEvent);
    /// Inner size in physical pixels changed (also reported once after `init`).
    fn resize(&mut self, width: u32, height: u32);
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
    let mut runner =
        Runner { config, app, window: None, last_frame: None, keys: keymap::HeldKeys::default(), error: None };
    event_loop.run_app(&mut runner).map_err(|e| PlatformError(e.to_string()))?;
    runner.error.map_or(Ok(()), |e| Err(PlatformError(e)))
}

struct Runner<'a> {
    config: WindowConfig,
    app: &'a mut dyn App,
    window: Option<Arc<Window>>,
    last_frame: Option<Instant>,
    keys: keymap::HeldKeys,
    error: Option<String>,
}

impl ApplicationHandler for Runner<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height));
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
                if self.app.frame(dt) == Flow::Exit {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
