//! Borderless fullscreen, cursor grab, and pointer-delta source selection.
//!
//! These types are the engine's window controls. They do not wrap winit or wgpu. The runner
//! applies them to the real window, and [`crate::WindowState`] records what that application
//! achieved.

use engine_core::InputEvent;
use engine_renderer_wgpu::SurfacePresent as PresentMode;

/// What the runner last applied. Requests live on [`crate::WindowControl`].
///
/// `present_mode` is `None` until a window surface has been configured. A headless game stays
/// that way. `grab_failed` stays set after a failed grab until a later attempt succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowState {
    pub fullscreen: bool,
    pub vsync_requested: bool,
    /// Effective present mode. `None` before the surface is configured.
    pub present_mode: Option<PresentMode>,
    /// Cap in frames per second after zero is treated as off.
    pub max_frame_rate: Option<u32>,
    pub cursor_visible: bool,
    pub cursor_grab: CursorGrab,
    pub grab_failed: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            fullscreen: false,
            vsync_requested: true,
            present_mode: None,
            max_frame_rate: None,
            cursor_visible: true,
            cursor_grab: CursorGrab::Free,
            grab_failed: false,
        }
    }
}

/// Borderless fullscreen request remembered across toggles.
///
/// Entering fullscreen stores the windowed logical size. Leaving asks the window to return to
/// that size. A repeated request while already in the requested state does nothing, so the
/// surface is not reconfigured for the same call. Exclusive fullscreen is not represented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullscreenToggle {
    active: bool,
    /// Logical size to restore. Kept while a leave is still waiting for the windowed resize,
    /// so a quick re-enter does not store the fullscreen framebuffer as the windowed size.
    restore: Option<(u32, u32)>,
    awaiting_windowed_size: bool,
    /// Logical size of the fullscreen framebuffer at the moment of leaving. A resize that still
    /// reports that size does not replace the saved windowed size.
    pending_fullscreen_size: Option<(u32, u32)>,
}

/// What the runner should do to the window for one fullscreen reconcile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FullscreenStep {
    /// The request already matches the applied state.
    Unchanged,
    /// Enter borderless fullscreen. The following resize supplies the new framebuffer.
    Enter,
    /// Leave fullscreen and request this logical window size.
    Exit { logical_size: (u32, u32) },
}

impl FullscreenToggle {
    /// `active` is true when the window was created already borderless.
    /// `windowed` is the logical size to restore on the first leave.
    pub fn new(active: bool, windowed: (u32, u32)) -> Self {
        Self { active, restore: Some(windowed), awaiting_windowed_size: false, pending_fullscreen_size: None }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    /// `windowed_logical` is the current logical size. It is stored only when the window is
    /// actually windowed, which [`FullscreenToggle::note_windowed_size`] records.
    pub fn reconcile(&mut self, requested: bool, windowed_logical: (u32, u32)) -> FullscreenStep {
        if self.active == requested {
            return FullscreenStep::Unchanged;
        }
        if requested {
            if !self.awaiting_windowed_size {
                self.restore = Some(windowed_logical);
            }
            self.awaiting_windowed_size = false;
            self.pending_fullscreen_size = None;
            self.active = true;
            FullscreenStep::Enter
        } else {
            self.pending_fullscreen_size = Some(windowed_logical);
            self.active = false;
            self.awaiting_windowed_size = true;
            let logical_size = self.restore.unwrap_or(windowed_logical);
            FullscreenStep::Exit { logical_size }
        }
    }

    /// A resize arrived while fullscreen is off. That size becomes the next restore size.
    ///
    /// A resize that still has the fullscreen logical size is ignored, so the saved windowed size
    /// survives until the windowed size actually arrives.
    pub fn note_windowed_size(&mut self, logical: (u32, u32)) {
        if self.active {
            return;
        }
        if self.awaiting_windowed_size && self.pending_fullscreen_size == Some(logical) {
            return;
        }
        self.restore = Some(logical);
        self.awaiting_windowed_size = false;
        self.pending_fullscreen_size = None;
    }
}

/// Logical window size from a physical framebuffer and a display scale.
///
/// Rounds to the nearest logical pixel. A non-finite or non-positive scale returns the physical
/// size, so a bad scale does not produce a zero restore size.
pub fn logical_window_size(physical_width: u32, physical_height: u32, scale: f64) -> (u32, u32) {
    if !scale.is_finite() || scale <= 0.0 {
        return (physical_width.max(1), physical_height.max(1));
    }
    let width = ((physical_width as f64) / scale).round().max(1.0) as u32;
    let height = ((physical_height as f64) / scale).round().max(1.0) as u32;
    (width, height)
}

/// How the cursor is actually grabbed. `Free` means the cursor is not grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorGrab {
    #[default]
    Free,
    /// Locked to a point. Relative motion comes from device motion.
    Locked,
    /// Confined to the window. Relative motion still comes from device motion.
    Confined,
}

/// Result of applying a grab request. `failed` stays set until a later attempt succeeds or the
/// caller asks to release from a grab that is still held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrabUpdate {
    pub grab: CursorGrab,
    pub failed: bool,
    pub changed: bool,
}

/// Tries [`CursorGrab::Locked`], then [`CursorGrab::Confined`].
///
/// `try_mode` reports whether the window accepted that mode. It is not called when the grab is
/// already in effect or when the request is to release. A failure leaves the grab free and sets
/// [`GrabUpdate::failed`]. The caller clears the request so the next frame does not retry.
pub fn reconcile_grab(
    active: CursorGrab,
    failed: bool,
    requested: bool,
    mut try_mode: impl FnMut(CursorGrab) -> bool,
) -> GrabUpdate {
    if requested {
        if active != CursorGrab::Free {
            return GrabUpdate { grab: active, failed: false, changed: false };
        }
        if try_mode(CursorGrab::Locked) {
            return GrabUpdate { grab: CursorGrab::Locked, failed: false, changed: true };
        }
        if try_mode(CursorGrab::Confined) {
            return GrabUpdate { grab: CursorGrab::Confined, failed: false, changed: true };
        }
        return GrabUpdate { grab: CursorGrab::Free, failed: true, changed: false };
    }
    if active != CursorGrab::Free {
        return GrabUpdate { grab: CursorGrab::Free, failed: false, changed: true };
    }
    GrabUpdate { grab: CursorGrab::Free, failed, changed: false }
}

/// Decides which pointer event reaches [`engine_core::Input`] for the current grab.
///
/// A free cursor keeps [`InputEvent::MouseMoved`] and drops [`InputEvent::MouseDelta`]. A grabbed
/// cursor keeps device motion and turns cursor-position events into [`InputEvent::MousePosition`],
/// so a lock warp is not added to `mouse_delta`. `suppress_next_move` does the same for one
/// position event after a grab change, then clears itself.
pub fn accept_pointer(grabbed: bool, suppress_next_move: &mut bool, event: InputEvent) -> Option<InputEvent> {
    match event {
        InputEvent::MouseDelta(_) if !grabbed => None,
        InputEvent::MouseMoved(position) if grabbed || *suppress_next_move => {
            *suppress_next_move = false;
            Some(InputEvent::MousePosition(position))
        }
        other => Some(other),
    }
}

/// Where `mouse_delta` is taken from. Device motion is winit's `MouseMotion` delta. Its units
/// are the backend's, and this is not a claim of unaccelerated HID input.
pub fn pointer_delta_source(grab: CursorGrab) -> PointerDeltaSource {
    match grab {
        CursorGrab::Free => PointerDeltaSource::CursorPosition,
        CursorGrab::Locked | CursorGrab::Confined => PointerDeltaSource::DeviceMotion,
    }
}

/// Which stream feeds `mouse_delta`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerDeltaSource {
    CursorPosition,
    DeviceMotion,
}

/// Applies accepted pointer events and returns the position and delta. Tests use this.
#[cfg(test)]
fn delta_after(
    grabbed: bool,
    suppress_next_move: &mut bool,
    events: &[InputEvent],
) -> (engine_math::Vec2, engine_math::Vec2) {
    use engine_core::Input;
    use engine_math::Vec2;
    let mut input = Input::default();
    let mut position = Vec2::ZERO;
    for event in events {
        if let Some(event) = accept_pointer(grabbed, suppress_next_move, *event) {
            input.apply(event);
            position = input.mouse_position();
        }
    }
    (position, input.mouse_delta())
}

#[cfg(test)]
mod tests {
    use engine_math::Vec2;

    use super::*;

    #[test]
    fn fullscreen_enter_is_once_and_exit_restores_the_windowed_size() {
        let mut toggle = FullscreenToggle::new(false, (1280, 720));
        assert_eq!(toggle.reconcile(false, (1280, 720)), FullscreenStep::Unchanged);
        assert_eq!(toggle.reconcile(true, (1280, 720)), FullscreenStep::Enter);
        assert_eq!(toggle.reconcile(true, (2560, 1440)), FullscreenStep::Unchanged);
        assert_eq!(toggle.reconcile(false, (2560, 1440)), FullscreenStep::Exit { logical_size: (1280, 720) });
        assert_eq!(toggle.reconcile(false, (800, 600)), FullscreenStep::Unchanged);
        toggle.note_windowed_size((800, 600));
        assert_eq!(toggle.reconcile(true, (800, 600)), FullscreenStep::Enter);
        assert_eq!(toggle.reconcile(false, (800, 600)), FullscreenStep::Exit { logical_size: (800, 600) });
    }

    #[test]
    fn a_fullscreen_sized_resize_after_exit_does_not_replace_the_windowed_size() {
        let mut toggle = FullscreenToggle::new(false, (1280, 720));
        assert_eq!(toggle.reconcile(true, (1280, 720)), FullscreenStep::Enter);
        assert_eq!(toggle.reconcile(false, (2560, 1440)), FullscreenStep::Exit { logical_size: (1280, 720) });
        toggle.note_windowed_size((2560, 1440));
        assert_eq!(toggle.reconcile(true, (2560, 1440)), FullscreenStep::Enter);
        assert_eq!(toggle.reconcile(false, (2560, 1440)), FullscreenStep::Exit { logical_size: (1280, 720) });
        toggle.note_windowed_size((1280, 720));
        toggle.note_windowed_size((900, 600));
        assert_eq!(toggle.reconcile(false, (900, 600)), FullscreenStep::Unchanged);
        assert_eq!(toggle.reconcile(true, (900, 600)), FullscreenStep::Enter);
        assert_eq!(toggle.reconcile(false, (1920, 1080)), FullscreenStep::Exit { logical_size: (900, 600) });
    }

    #[test]
    fn reentering_before_the_windowed_resize_keeps_the_saved_size() {
        let mut toggle = FullscreenToggle::new(false, (1000, 700));
        assert_eq!(toggle.reconcile(true, (1000, 700)), FullscreenStep::Enter);
        assert_eq!(toggle.reconcile(false, (1920, 1080)), FullscreenStep::Exit { logical_size: (1000, 700) });
        assert_eq!(toggle.reconcile(true, (1920, 1080)), FullscreenStep::Enter);
        assert_eq!(toggle.reconcile(false, (1920, 1080)), FullscreenStep::Exit { logical_size: (1000, 700) });
    }

    #[test]
    fn logical_window_size_rounds_and_rejects_a_bad_scale() {
        assert_eq!(logical_window_size(2000, 1000, 2.0), (1000, 500));
        assert_eq!(logical_window_size(1500, 751, 1.5), (1000, 501));
        assert_eq!(logical_window_size(800, 600, f64::NAN), (800, 600));
    }

    #[test]
    fn grab_tries_locked_then_confined_and_a_failure_does_not_stick_the_request() {
        let mut tried = Vec::new();
        let failed = reconcile_grab(CursorGrab::Free, false, true, |mode| {
            tried.push(mode);
            false
        });
        assert_eq!(tried, vec![CursorGrab::Locked, CursorGrab::Confined]);
        assert_eq!(failed, GrabUpdate { grab: CursorGrab::Free, failed: true, changed: false });

        let locked = reconcile_grab(CursorGrab::Free, true, true, |mode| mode == CursorGrab::Locked);
        assert_eq!(locked.grab, CursorGrab::Locked);
        assert!(!locked.failed && locked.changed);
        let again = reconcile_grab(CursorGrab::Locked, false, true, |_| panic!("already grabbed"));
        assert!(!again.changed && !again.failed);

        let mut tried = Vec::new();
        let confined = reconcile_grab(CursorGrab::Free, false, true, |mode| {
            tried.push(mode);
            mode == CursorGrab::Confined
        });
        assert_eq!(tried, vec![CursorGrab::Locked, CursorGrab::Confined]);
        assert_eq!(confined.grab, CursorGrab::Confined);

        let released = reconcile_grab(CursorGrab::Confined, false, false, |_| panic!("release"));
        assert_eq!(released, GrabUpdate { grab: CursorGrab::Free, failed: false, changed: true });
    }

    #[test]
    fn a_failed_grab_stays_visible_until_the_next_attempt() {
        let sitting = reconcile_grab(CursorGrab::Free, true, false, |_| panic!("no attempt"));
        assert!(sitting.failed && !sitting.changed);
        let recovered = reconcile_grab(CursorGrab::Free, true, true, |_| true);
        assert_eq!(recovered.grab, CursorGrab::Locked);
        assert!(!recovered.failed);
    }

    #[test]
    fn pointer_events_use_one_delta_source_and_a_transition_drops_the_warp() {
        assert_eq!(pointer_delta_source(CursorGrab::Free), PointerDeltaSource::CursorPosition);
        assert_eq!(pointer_delta_source(CursorGrab::Locked), PointerDeltaSource::DeviceMotion);
        assert_eq!(pointer_delta_source(CursorGrab::Confined), PointerDeltaSource::DeviceMotion);

        let events = [InputEvent::MouseMoved(Vec2::new(4.0, 6.0)), InputEvent::MouseDelta(Vec2::new(30.0, 0.0))];
        let mut suppress = false;
        let (position, delta) = delta_after(false, &mut suppress, &events);
        assert_eq!(position, Vec2::new(4.0, 6.0));
        assert_eq!(delta, Vec2::new(4.0, 6.0));

        let mut suppress = false;
        let (position, delta) = delta_after(true, &mut suppress, &events);
        assert_eq!(position, Vec2::new(4.0, 6.0));
        assert_eq!(delta, Vec2::new(30.0, 0.0));

        let mut suppress = true;
        let warp = [InputEvent::MouseMoved(Vec2::new(400.0, 300.0)), InputEvent::MouseMoved(Vec2::new(408.0, 300.0))];
        let (position, delta) = delta_after(false, &mut suppress, &warp);
        assert!(!suppress);
        assert_eq!(position, Vec2::new(408.0, 300.0));
        assert_eq!(delta, Vec2::new(8.0, 0.0));
    }
}
