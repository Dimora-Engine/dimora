use std::collections::HashSet;

use engine_math::Vec2;

/// Physical keys the engine cares about. The platform layer maps its native key codes to these,
/// which keeps gameplay code independent of the windowing library.
///
/// Letters, digits, and punctuation are positions on a US keyboard, so the W position stays W on
/// AZERTY. Left and right Shift, Control, and Alt share one variant. [`Key::NumpadEnter`] is not
/// [`Key::Enter`]. F13 and above are not mapped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[rustfmt::skip]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    Up, Down, Left, Right,
    Space, Enter, Escape, Tab, Shift, Control, Alt,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Backquote, Minus, Equal, BracketLeft, BracketRight, Backslash,
    Semicolon, Quote, Comma, Period, Slash,
    Backspace, Delete, Insert, Home, End, PageUp, PageDown,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal, NumpadEnter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A device event, as produced by the platform layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    Key {
        key: Key,
        pressed: bool,
    },
    MouseButton {
        button: MouseButton,
        pressed: bool,
    },
    /// Cursor position in window pixels, origin top-left.
    MouseMoved(Vec2),
    Scroll(f32),
}

/// Snapshot of device state for the current frame.
///
/// The platform layer calls `press`/`release`/`set_mouse_position`/... as events arrive and
/// [`Input::end_frame`] once the frame has been simulated. "Just pressed/released" sets are valid
/// for exactly one frame.
#[derive(Default, Debug, Clone)]
pub struct Input {
    down: HashSet<Key>,
    just_pressed: HashSet<Key>,
    just_released: HashSet<Key>,
    buttons_down: HashSet<MouseButton>,
    buttons_pressed: HashSet<MouseButton>,
    buttons_released: HashSet<MouseButton>,
    mouse_position: Vec2,
    mouse_delta: Vec2,
    scroll: f32,
}

impl Input {
    pub fn key_down(&self, key: Key) -> bool {
        self.down.contains(&key)
    }
    pub fn key_pressed(&self, key: Key) -> bool {
        self.just_pressed.contains(&key)
    }
    pub fn key_released(&self, key: Key) -> bool {
        self.just_released.contains(&key)
    }
    pub fn mouse_down(&self, b: MouseButton) -> bool {
        self.buttons_down.contains(&b)
    }
    pub fn mouse_pressed(&self, b: MouseButton) -> bool {
        self.buttons_pressed.contains(&b)
    }
    /// `true` for the rest of this frame after the button transitions up.
    ///
    /// A release of a button that was not down does not set the edge. [`Input::end_frame`] clears it.
    pub fn mouse_released(&self, b: MouseButton) -> bool {
        self.buttons_released.contains(&b)
    }
    /// Cursor position in window pixels, origin top-left.
    pub fn mouse_position(&self) -> Vec2 {
        self.mouse_position
    }
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// `+1` when `positive` is held, `-1` when `negative` is held, `0` for neither or both.
    pub fn axis(&self, negative: Key, positive: Key) -> f32 {
        self.key_down(positive) as i8 as f32 - self.key_down(negative) as i8 as f32
    }

    /// Applies one platform event to the state.
    pub fn apply(&mut self, event: InputEvent) {
        match event {
            InputEvent::Key { key, pressed: true } => self.press(key),
            InputEvent::Key { key, pressed: false } => self.release(key),
            InputEvent::MouseButton { button, pressed: true } => self.press_mouse(button),
            InputEvent::MouseButton { button, pressed: false } => self.release_mouse(button),
            InputEvent::MouseMoved(p) => self.set_mouse_position(p),
            InputEvent::Scroll(s) => self.add_scroll(s),
        }
    }

    pub fn press(&mut self, key: Key) {
        if self.down.insert(key) {
            self.just_pressed.insert(key);
        }
    }
    pub fn release(&mut self, key: Key) {
        if self.down.remove(&key) {
            self.just_released.insert(key);
        }
    }
    pub fn press_mouse(&mut self, b: MouseButton) {
        if self.buttons_down.insert(b) {
            self.buttons_pressed.insert(b);
        }
    }
    pub fn release_mouse(&mut self, b: MouseButton) {
        if self.buttons_down.remove(&b) {
            self.buttons_released.insert(b);
        }
    }
    pub fn set_mouse_position(&mut self, p: Vec2) {
        self.mouse_delta += p - self.mouse_position;
        self.mouse_position = p;
    }
    pub fn add_scroll(&mut self, s: f32) {
        self.scroll += s;
    }

    /// Clears per-frame edge state. Call after systems have run for the frame.
    pub fn end_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.mouse_delta = Vec2::ZERO;
        self.scroll = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_last_one_frame_and_repeats_are_ignored() {
        let mut i = Input::default();
        i.press(Key::W);
        i.press(Key::W);
        assert!(i.key_down(Key::W) && i.key_pressed(Key::W));
        i.end_frame();
        assert!(i.key_down(Key::W) && !i.key_pressed(Key::W));
        i.release(Key::W);
        assert!(!i.key_down(Key::W) && i.key_released(Key::W));
        i.end_frame();
        assert!(!i.key_released(Key::W));
    }

    #[test]
    fn a_tap_inside_one_frame_keeps_both_key_edges() {
        let mut i = Input::default();
        i.apply(InputEvent::Key { key: Key::Space, pressed: true });
        i.apply(InputEvent::Key { key: Key::Space, pressed: false });
        assert!(!i.key_down(Key::Space));
        assert!(i.key_pressed(Key::Space) && i.key_released(Key::Space));
        i.end_frame();
        assert!(!i.key_pressed(Key::Space) && !i.key_released(Key::Space));
    }

    #[test]
    fn mouse_release_is_an_edge_for_one_frame() {
        let mut i = Input::default();
        i.press_mouse(MouseButton::Left);
        i.press_mouse(MouseButton::Left);
        assert!(i.mouse_down(MouseButton::Left) && i.mouse_pressed(MouseButton::Left));
        assert!(!i.mouse_released(MouseButton::Left));
        i.end_frame();
        assert!(i.mouse_down(MouseButton::Left) && !i.mouse_pressed(MouseButton::Left));
        i.release_mouse(MouseButton::Left);
        assert!(!i.mouse_down(MouseButton::Left) && i.mouse_released(MouseButton::Left));
        i.end_frame();
        assert!(!i.mouse_released(MouseButton::Left));
        i.release_mouse(MouseButton::Right);
        assert!(!i.mouse_released(MouseButton::Right));
    }

    #[test]
    fn axis_cancels() {
        let mut i = Input::default();
        i.press(Key::D);
        assert_eq!(i.axis(Key::A, Key::D), 1.0);
        i.press(Key::A);
        assert_eq!(i.axis(Key::A, Key::D), 0.0);
    }
}
