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
    /// Cursor position in physical pixels, origin top-left, +Y down.
    ///
    /// While the cursor is free this also adds the change to [`Input::mouse_delta`]. The window
    /// loop does not deliver this for a grabbed cursor; it delivers [`InputEvent::MousePosition`]
    /// and [`InputEvent::MouseDelta`] instead.
    MouseMoved(Vec2),
    /// Sets the cursor position and does not change [`Input::mouse_delta`].
    MousePosition(Vec2),
    /// Relative motion added to [`Input::mouse_delta`]. The window loop delivers this only while
    /// the cursor is grabbed. Units are the platform's device-motion units, not a raw HID sample.
    MouseDelta(Vec2),
    /// The window lost focus. Held keys and buttons are released, and `mouse_delta` is cleared.
    FocusLost,
    /// The window gained focus. `mouse_delta` is cleared, and the next [`InputEvent::MouseMoved`]
    /// sets the position without adding the jump from the unfocused cursor.
    FocusGained,
    Scroll(f32),
}

/// Snapshot of device state for the current frame, plus the fixed-step edge latch.
///
/// The platform layer calls `press`/`release`/`set_mouse_position`/... as events arrive and
/// [`Input::end_frame`] once the frame has been simulated. Frame edges (`key_pressed`,
/// `key_released`, `mouse_pressed`, `mouse_released`) stay set until that call.
///
/// Fixed-step edges (`fixed_key_pressed`, `fixed_key_released`, `fixed_mouse_pressed`,
/// `fixed_mouse_released`) are a second pair of bits. They are published until
/// [`Input::conceal_fixed_edges`], which `Game::step` calls after the first fixed iteration.
/// [`Input::publish_fixed_edges`] shows whatever has been stored since then. [`Input::end_frame`]
/// does not touch them. A read does not clear them. Held state (`key_down`, `mouse_down`) follows
/// the device the whole time.
#[derive(Debug, Clone)]
pub struct Input {
    down: HashSet<Key>,
    just_pressed: HashSet<Key>,
    just_released: HashSet<Key>,
    buttons_down: HashSet<MouseButton>,
    buttons_pressed: HashSet<MouseButton>,
    buttons_released: HashSet<MouseButton>,
    keys_latch: FixedLatch<Key>,
    buttons_latch: FixedLatch<MouseButton>,
    mouse_position: Vec2,
    mouse_delta: Vec2,
    /// The next [`Input::set_mouse_position`] stores the position and does not add a delta.
    ignore_next_position_delta: bool,
    scroll: f32,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            down: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
            buttons_down: HashSet::new(),
            buttons_pressed: HashSet::new(),
            buttons_released: HashSet::new(),
            keys_latch: FixedLatch::publishing(),
            buttons_latch: FixedLatch::publishing(),
            mouse_position: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            ignore_next_position_delta: false,
            scroll: 0.0,
        }
    }
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
    /// Fixed-step press bit. This is the published latch, not a different answer per stage.
    ///
    /// A press stores the bit. It stays readable through [`Input::end_frame`]. After
    /// [`Input::conceal_fixed_edges`] it reads false until [`Input::publish_fixed_edges`], including
    /// when a newer press is already stored for that next publish. `Game::step` publishes at the
    /// start of every step and conceals after the first fixed iteration, so a step with no fixed
    /// iteration leaves the bit readable. Reading it does not clear it.
    pub fn fixed_key_pressed(&self, key: Key) -> bool {
        self.keys_latch.pressed(&key)
    }
    /// Fixed-step release bit. Same lifetime as [`Input::fixed_key_pressed`].
    pub fn fixed_key_released(&self, key: Key) -> bool {
        self.keys_latch.released(&key)
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
    /// Fixed-step mouse press. Same lifetime as [`Input::fixed_key_pressed`].
    pub fn fixed_mouse_pressed(&self, b: MouseButton) -> bool {
        self.buttons_latch.pressed(&b)
    }
    /// Fixed-step mouse release. Same lifetime as [`Input::fixed_key_pressed`].
    pub fn fixed_mouse_released(&self, b: MouseButton) -> bool {
        self.buttons_latch.released(&b)
    }
    /// Cursor position in physical pixels, origin top-left, +Y down.
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
            InputEvent::MousePosition(p) => self.place_mouse(p),
            InputEvent::MouseDelta(d) => self.add_mouse_delta(d),
            InputEvent::FocusLost => self.release_focus(),
            InputEvent::FocusGained => self.regain_focus(),
            InputEvent::Scroll(s) => self.add_scroll(s),
        }
    }

    pub fn press(&mut self, key: Key) {
        if self.down.insert(key) {
            self.just_pressed.insert(key);
            self.keys_latch.press(key);
        }
    }
    pub fn release(&mut self, key: Key) {
        if self.down.remove(&key) {
            self.just_released.insert(key);
            self.keys_latch.release(key);
        }
    }
    pub fn press_mouse(&mut self, b: MouseButton) {
        if self.buttons_down.insert(b) {
            self.buttons_pressed.insert(b);
            self.buttons_latch.press(b);
        }
    }
    pub fn release_mouse(&mut self, b: MouseButton) {
        if self.buttons_down.remove(&b) {
            self.buttons_released.insert(b);
            self.buttons_latch.release(b);
        }
    }
    pub fn set_mouse_position(&mut self, p: Vec2) {
        if self.ignore_next_position_delta {
            self.ignore_next_position_delta = false;
            self.mouse_position = p;
            return;
        }
        self.mouse_delta += p - self.mouse_position;
        self.mouse_position = p;
    }

    /// Stores the cursor position without changing [`Input::mouse_delta`].
    pub fn place_mouse(&mut self, p: Vec2) {
        self.mouse_position = p;
    }

    /// Adds relative motion to [`Input::mouse_delta`]. Does not move [`Input::mouse_position`].
    pub fn add_mouse_delta(&mut self, delta: Vec2) {
        self.mouse_delta += delta;
    }

    /// Releases every held key and button and clears `mouse_delta`.
    ///
    /// Each key or button that was down gets a frame release edge and a fixed-step release bit.
    /// A key that was not down is left alone. This is what focus loss applies.
    pub fn release_focus(&mut self) {
        let keys: Vec<Key> = self.down.iter().copied().collect();
        for key in keys {
            self.release(key);
        }
        let buttons: Vec<MouseButton> = self.buttons_down.iter().copied().collect();
        for button in buttons {
            self.release_mouse(button);
        }
        self.mouse_delta = Vec2::ZERO;
    }

    /// Clears `mouse_delta` and ignores the next position delta.
    ///
    /// Focus returning here does not press keys and does not grab the cursor.
    pub fn regain_focus(&mut self) {
        self.mouse_delta = Vec2::ZERO;
        self.ignore_next_position_delta = true;
    }
    pub fn add_scroll(&mut self, s: f32) {
        self.scroll += s;
    }

    /// Clears per-frame edge state. Does not change held keys or the fixed-step latch.
    pub fn end_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.mouse_delta = Vec2::ZERO;
        self.scroll = 0.0;
    }

    /// Shows fixed-step edges stored since the last [`Input::conceal_fixed_edges`].
    ///
    /// Does nothing when they are already published. `Game::step` calls this at the start of every
    /// step. Gameplay code does not.
    pub fn publish_fixed_edges(&mut self) {
        self.keys_latch.publish();
        self.buttons_latch.publish();
    }

    /// Hides the published fixed-step edges and drops the ones just shown.
    ///
    /// A transition after this stays stored until the next [`Input::publish_fixed_edges`]. Held
    /// state and frame edges stay as they are. `Game::step` calls this after the first fixed
    /// iteration's systems and command flush. Gameplay code does not.
    pub fn conceal_fixed_edges(&mut self) {
        self.keys_latch.conceal();
        self.buttons_latch.conceal();
    }

    pub(crate) fn held_keys(&self) -> &HashSet<Key> {
        &self.down
    }

    pub(crate) fn held_buttons(&self) -> &HashSet<MouseButton> {
        &self.buttons_down
    }
}

/// Press and release bits that stay stored until a fixed iteration can see them.
///
/// While `visible` is set, a new edge is readable immediately. [`FixedLatch::conceal`] clears the
/// readable set and anything already delivered. Edges recorded after that wait in `pending` until
/// [`FixedLatch::publish`].
#[derive(Debug, Clone)]
pub(crate) struct FixedLatch<T> {
    pending_pressed: HashSet<T>,
    pending_released: HashSet<T>,
    visible_pressed: HashSet<T>,
    visible_released: HashSet<T>,
    visible: bool,
}

impl<T> FixedLatch<T>
where
    T: Copy + Eq + std::hash::Hash,
{
    pub(crate) fn publishing() -> Self {
        Self {
            pending_pressed: HashSet::new(),
            pending_released: HashSet::new(),
            visible_pressed: HashSet::new(),
            visible_released: HashSet::new(),
            visible: true,
        }
    }

    pub(crate) fn press(&mut self, value: T) {
        self.pending_pressed.insert(value);
        if self.visible {
            self.visible_pressed.insert(value);
        }
    }

    pub(crate) fn release(&mut self, value: T) {
        self.pending_released.insert(value);
        if self.visible {
            self.visible_released.insert(value);
        }
    }

    pub(crate) fn pressed(&self, value: &T) -> bool {
        self.visible_pressed.contains(value)
    }

    pub(crate) fn released(&self, value: &T) -> bool {
        self.visible_released.contains(value)
    }

    pub(crate) fn publish(&mut self) {
        if self.visible {
            return;
        }
        self.visible_pressed.clone_from(&self.pending_pressed);
        self.visible_released.clone_from(&self.pending_released);
        self.visible = true;
    }

    pub(crate) fn conceal(&mut self) {
        self.pending_pressed.clear();
        self.pending_released.clear();
        self.visible_pressed.clear();
        self.visible_released.clear();
        self.visible = false;
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
        assert!(i.fixed_key_pressed(Key::Space) && i.fixed_key_released(Key::Space));
    }

    #[test]
    fn the_fixed_latch_survives_end_frame_and_a_read_does_not_clear_it() {
        let mut i = Input::default();
        i.press(Key::Space);
        i.release(Key::Space);
        assert!(i.fixed_key_pressed(Key::Space) && i.fixed_key_released(Key::Space));
        assert!(i.fixed_key_pressed(Key::Space));
        i.end_frame();
        assert!(!i.key_pressed(Key::Space) && !i.key_released(Key::Space));
        assert!(i.fixed_key_pressed(Key::Space) && i.fixed_key_released(Key::Space));
        assert!(!i.key_down(Key::Space));
        i.conceal_fixed_edges();
        assert!(!i.fixed_key_pressed(Key::Space) && !i.fixed_key_released(Key::Space));
        i.press(Key::W);
        assert!(!i.fixed_key_pressed(Key::W));
        i.publish_fixed_edges();
        assert!(i.fixed_key_pressed(Key::W) && i.key_down(Key::W));
    }

    #[test]
    fn repeated_presses_collapse_to_one_fixed_bit() {
        let mut i = Input::default();
        i.press(Key::W);
        i.press(Key::W);
        i.release(Key::W);
        i.press(Key::W);
        i.release(Key::W);
        assert!(i.fixed_key_pressed(Key::W) && i.fixed_key_released(Key::W));
        assert!(!i.key_down(Key::W));
        i.conceal_fixed_edges();
        i.press(Key::W);
        assert!(i.key_pressed(Key::W) && i.key_down(Key::W));
        assert!(!i.fixed_key_pressed(Key::W));
    }

    #[test]
    fn mouse_fixed_edges_follow_the_key_latch() {
        let mut i = Input::default();
        i.press_mouse(MouseButton::Right);
        i.release_mouse(MouseButton::Right);
        i.release_mouse(MouseButton::Left);
        assert!(i.fixed_mouse_pressed(MouseButton::Right) && i.fixed_mouse_released(MouseButton::Right));
        assert!(!i.fixed_mouse_released(MouseButton::Left));
        i.end_frame();
        assert!(i.fixed_mouse_pressed(MouseButton::Right));
        assert!(!i.mouse_pressed(MouseButton::Right));
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
    fn focus_loss_releases_held_input_and_focus_gain_drops_the_next_jump() {
        let mut input = Input::default();
        input.press(Key::W);
        input.press(Key::Shift);
        input.press_mouse(MouseButton::Left);
        input.set_mouse_position(Vec2::new(12.0, 8.0));
        input.apply(InputEvent::FocusLost);
        assert!(!input.key_down(Key::W) && input.key_released(Key::W));
        assert!(!input.key_down(Key::Shift) && input.key_released(Key::Shift));
        assert!(input.fixed_key_released(Key::W) && input.fixed_key_released(Key::Shift));
        assert!(!input.mouse_down(MouseButton::Left) && input.mouse_released(MouseButton::Left));
        assert!(input.fixed_mouse_released(MouseButton::Left));
        assert_eq!(input.mouse_delta(), Vec2::ZERO);
        input.apply(InputEvent::FocusGained);
        input.apply(InputEvent::MouseMoved(Vec2::new(400.0, 20.0)));
        assert_eq!(input.mouse_position(), Vec2::new(400.0, 20.0));
        assert_eq!(input.mouse_delta(), Vec2::ZERO);
        input.apply(InputEvent::MouseMoved(Vec2::new(403.0, 22.0)));
        assert_eq!(input.mouse_delta(), Vec2::new(3.0, 2.0));
        assert!(!input.key_down(Key::W));
        input.press(Key::W);
        assert!(input.key_down(Key::W) && input.key_pressed(Key::W));
    }

    #[test]
    fn a_second_focus_loss_does_not_repeat_the_release() {
        let mut input = Input::default();
        input.press(Key::Shift);
        input.press_mouse(MouseButton::Right);
        input.add_mouse_delta(Vec2::new(5.0, 1.0));
        input.apply(InputEvent::FocusLost);
        assert!(input.key_released(Key::Shift));
        assert!(input.mouse_released(MouseButton::Right));
        assert!(input.fixed_key_released(Key::Shift));
        input.end_frame();
        input.conceal_fixed_edges();
        input.apply(InputEvent::FocusLost);
        assert!(!input.key_released(Key::Shift));
        assert!(!input.mouse_released(MouseButton::Right));
        assert!(!input.fixed_key_released(Key::Shift));
        assert!(!input.fixed_mouse_released(MouseButton::Right));
        assert!(!input.key_down(Key::Shift));
        assert_eq!(input.mouse_delta(), Vec2::ZERO);
    }

    #[test]
    fn a_concealed_focus_release_waits_for_the_next_publish() {
        let mut input = Input::default();
        input.press(Key::A);
        input.conceal_fixed_edges();
        input.apply(InputEvent::FocusLost);
        assert!(input.key_released(Key::A));
        assert!(!input.fixed_key_released(Key::A));
        input.publish_fixed_edges();
        assert!(input.fixed_key_released(Key::A));
        assert!(!input.key_down(Key::A));
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
