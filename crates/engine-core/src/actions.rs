//! Named gameplay actions over the raw [`Input`] resource.
//!
//! An action is down when any of its bindings is down. A press is the aggregate going from up to
//! down, and a release is the aggregate going from down to up. A second binding does not press
//! again, and releasing one of several held bindings does not release. Frame edges and the
//! fixed-step latch are updated at each device event ([`Actions::sync`]) and at each rebind, so a
//! tap that is gone before the next read still leaves both edges.
//!
//! ```
//! use engine_core::{Actions, ButtonSource, Input, Key, MouseButton};
//!
//! let mut input = Input::default();
//! let mut actions = Actions::default();
//! actions.bind_button("jump", ButtonSource::Key(Key::Space));
//! actions.bind_button("jump", ButtonSource::Mouse(MouseButton::Left));
//! input.press(Key::Space);
//! actions.sync(&input);
//! assert!(actions.pressed("jump") && actions.down(&input, "jump"));
//! input.release(Key::Space);
//! actions.sync(&input);
//! assert!(actions.released("jump") && !actions.down(&input, "jump"));
//! ```

use std::collections::{HashMap, HashSet};

use engine_math::Vec2;

use crate::input::FixedLatch;
use crate::{Input, Key, MouseButton};

/// One physical control that can bind to an action or to one side of an axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ButtonSource {
    Key(Key),
    Mouse(MouseButton),
}

#[derive(Clone, Debug, Default)]
struct AxisBind {
    negative: Vec<ButtonSource>,
    positive: Vec<ButtonSource>,
}

/// Bindings from action names to keys and mouse buttons.
///
/// Names are `&'static str`. Frame edges ([`Actions::pressed`], [`Actions::released`]) clear in
/// [`Actions::end_frame`]. Fixed-step edges ([`Actions::fixed_pressed`], [`Actions::fixed_released`])
/// follow the same publish and conceal cycle as [`Input`]: readable until
/// [`Actions::conceal_fixed_edges`], then stored until [`Actions::publish_fixed_edges`].
/// `Game::step` calls those with the raw latch. [`Actions::down`] and the axis queries read the
/// current device state and do not depend on that cycle.
///
/// Device events reach this resource through [`Actions::sync`]. `Game::send_input` does that after
/// it applies the event. Rebinding uses the held snapshot from the last sync.
#[derive(Debug)]
pub struct Actions {
    buttons: HashMap<&'static str, Vec<ButtonSource>>,
    axes: HashMap<&'static str, AxisBind>,
    active: HashSet<&'static str>,
    frame_pressed: HashSet<&'static str>,
    frame_released: HashSet<&'static str>,
    latch: FixedLatch<&'static str>,
    down_keys: HashSet<Key>,
    down_buttons: HashSet<MouseButton>,
}

impl Default for Actions {
    fn default() -> Self {
        Self {
            buttons: HashMap::new(),
            axes: HashMap::new(),
            active: HashSet::new(),
            frame_pressed: HashSet::new(),
            frame_released: HashSet::new(),
            latch: FixedLatch::publishing(),
            down_keys: HashSet::new(),
            down_buttons: HashSet::new(),
        }
    }
}

impl Actions {
    /// Adds one binding. The first binding that is already down presses the action once.
    pub fn bind_button(&mut self, action: &'static str, source: ButtonSource) {
        let was = self.active.contains(action);
        self.buttons.entry(action).or_default().push(source);
        let now = was || self.source_down(source);
        self.note(action, was, now);
    }

    /// Replaces every binding for `action`, then records the aggregate change against the last sync.
    pub fn set_button(&mut self, action: &'static str, sources: &[ButtonSource]) {
        let was = self.active.contains(action);
        let now = sources.iter().copied().any(|source| self.source_down(source));
        self.buttons.insert(action, sources.to_vec());
        self.note(action, was, now);
    }

    /// Adds a negative and a positive binding. Further calls accumulate. Both sides, or neither, read as `0`.
    pub fn bind_axis(&mut self, axis: &'static str, negative: ButtonSource, positive: ButtonSource) {
        let slot = self.axes.entry(axis).or_default();
        slot.negative.push(negative);
        slot.positive.push(positive);
    }

    /// Replaces both sides of `axis`. Axes have no press or release edge.
    pub fn set_axis(&mut self, axis: &'static str, negative: &[ButtonSource], positive: &[ButtonSource]) {
        self.axes.insert(axis, AxisBind { negative: negative.to_vec(), positive: positive.to_vec() });
    }

    /// `true` while any binding is held. An unknown action is up.
    pub fn down(&self, input: &Input, action: &'static str) -> bool {
        self.buttons.get(action).is_some_and(|sources| sources.iter().any(|source| held(input, *source)))
    }

    /// Frame-scoped aggregate press. Cleared by [`Actions::end_frame`]. A read does not clear it.
    pub fn pressed(&self, action: &'static str) -> bool {
        self.frame_pressed.contains(action)
    }

    /// Frame-scoped aggregate release. Cleared by [`Actions::end_frame`].
    pub fn released(&self, action: &'static str) -> bool {
        self.frame_released.contains(action)
    }

    /// Fixed-step aggregate press. Same publish and conceal rules as [`Input::fixed_key_pressed`].
    ///
    /// After `Game::step` conceals the latch, this is false until the next step publishes it, even
    /// when a press is already stored. A step that runs no fixed iteration does not conceal, so the
    /// bit stays readable. A read does not clear it.
    pub fn fixed_pressed(&self, action: &'static str) -> bool {
        self.latch.pressed(&action)
    }

    /// Fixed-step aggregate release. Same lifetime as [`Actions::fixed_pressed`].
    pub fn fixed_released(&self, action: &'static str) -> bool {
        self.latch.released(&action)
    }

    /// `+1` when any positive binding is held, `-1` when any negative binding is held, `0` for both or neither.
    ///
    /// An unknown axis is `0`.
    pub fn axis(&self, input: &Input, axis: &'static str) -> f32 {
        let Some(bind) = self.axes.get(axis) else {
            return 0.0;
        };
        let negative = bind.negative.iter().any(|source| held(input, *source));
        let positive = bind.positive.iter().any(|source| held(input, *source));
        match (negative, positive) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    }

    /// `x` and `y` are two [`Actions::axis`] values.
    pub fn axis2(&self, input: &Input, x: &'static str, y: &'static str) -> Vec2 {
        Vec2::new(self.axis(input, x), self.axis(input, y))
    }

    /// Compares each button action with `input` and records aggregate edges.
    ///
    /// Call this after every device change that did not go through `Game::send_input`. Rebinding
    /// reads the held keys and buttons saved here.
    pub fn sync(&mut self, input: &Input) {
        let names: Vec<&'static str> = self.buttons.keys().copied().collect();
        for name in names {
            let now = self.down(input, name);
            let was = self.active.contains(name);
            self.note(name, was, now);
        }
        self.down_keys.clone_from(input.held_keys());
        self.down_buttons.clone_from(input.held_buttons());
    }

    /// Clears frame edges. Does not change bindings, held actions, or the fixed-step latch.
    pub fn end_frame(&mut self) {
        self.frame_pressed.clear();
        self.frame_released.clear();
    }

    /// Shows fixed-step action edges stored since the last [`Actions::conceal_fixed_edges`].
    ///
    /// `Game::step` calls this with [`Input::publish_fixed_edges`]. Gameplay code does not.
    pub fn publish_fixed_edges(&mut self) {
        self.latch.publish();
    }

    /// Hides the published fixed-step action edges and drops the ones just shown.
    ///
    /// `Game::step` calls this with [`Input::conceal_fixed_edges`] after the first fixed iteration.
    /// Gameplay code does not.
    pub fn conceal_fixed_edges(&mut self) {
        self.latch.conceal();
    }

    fn source_down(&self, source: ButtonSource) -> bool {
        match source {
            ButtonSource::Key(key) => self.down_keys.contains(&key),
            ButtonSource::Mouse(button) => self.down_buttons.contains(&button),
        }
    }

    fn note(&mut self, action: &'static str, was: bool, now: bool) {
        if now == was {
            return;
        }
        if now {
            self.frame_pressed.insert(action);
            self.latch.press(action);
            self.active.insert(action);
        } else {
            self.frame_released.insert(action);
            self.latch.release(action);
            self.active.remove(action);
        }
    }
}

fn held(input: &Input, source: ButtonSource) -> bool {
    match source {
        ButtonSource::Key(key) => input.key_down(key),
        ButtonSource::Mouse(button) => input.mouse_down(button),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jump() -> (Input, Actions) {
        let input = Input::default();
        let mut actions = Actions::default();
        actions.bind_button("jump", ButtonSource::Key(Key::Space));
        actions.bind_button("jump", ButtonSource::Key(Key::W));
        (input, actions)
    }

    #[test]
    fn the_first_binding_presses_once_and_the_second_does_not() {
        let (mut input, mut actions) = jump();
        input.press(Key::Space);
        actions.sync(&input);
        assert!(actions.pressed("jump") && actions.down(&input, "jump"));
        assert!(actions.fixed_pressed("jump"));
        input.press(Key::W);
        actions.sync(&input);
        assert!(actions.pressed("jump"));
        assert!(!actions.released("jump"));
        input.release(Key::Space);
        actions.sync(&input);
        assert!(actions.down(&input, "jump"));
        assert!(!actions.released("jump"));
        input.release(Key::W);
        actions.sync(&input);
        assert!(actions.released("jump") && !actions.down(&input, "jump"));
        assert!(actions.pressed("jump") && actions.fixed_released("jump"));
    }

    #[test]
    fn a_same_frame_tap_keeps_both_edges() {
        let (mut input, mut actions) = jump();
        input.press(Key::Space);
        actions.sync(&input);
        input.release(Key::Space);
        actions.sync(&input);
        assert!(!actions.down(&input, "jump"));
        assert!(actions.pressed("jump") && actions.released("jump"));
        assert!(actions.fixed_pressed("jump") && actions.fixed_released("jump"));
        actions.end_frame();
        input.end_frame();
        assert!(!actions.pressed("jump") && !actions.released("jump"));
        assert!(actions.fixed_pressed("jump") && actions.fixed_released("jump"));
    }

    #[test]
    fn two_taps_collapse_to_one_press_bit_and_one_release_bit() {
        let (mut input, mut actions) = jump();
        for _ in 0..2 {
            input.press(Key::Space);
            actions.sync(&input);
            input.release(Key::Space);
            actions.sync(&input);
        }
        assert!(actions.fixed_pressed("jump") && actions.fixed_released("jump"));
        assert!(!actions.down(&input, "jump"));
        actions.conceal_fixed_edges();
        assert!(!actions.fixed_pressed("jump") && !actions.fixed_released("jump"));
    }

    #[test]
    fn a_repeat_press_does_not_press_the_action_again() {
        let (mut input, mut actions) = jump();
        input.press(Key::Space);
        actions.sync(&input);
        actions.end_frame();
        input.press(Key::Space);
        actions.sync(&input);
        assert!(actions.down(&input, "jump"));
        assert!(!actions.pressed("jump"));
    }

    #[test]
    fn rebinding_follows_the_aggregate() {
        let (mut input, mut actions) = jump();
        input.press(Key::Space);
        input.press(Key::J);
        actions.sync(&input);
        actions.end_frame();
        actions.conceal_fixed_edges();
        actions.publish_fixed_edges();

        actions.set_button("jump", &[ButtonSource::Key(Key::J)]);
        assert!(!actions.pressed("jump") && !actions.released("jump"));
        assert!(actions.down(&input, "jump"));
        assert!(!actions.fixed_pressed("jump"));

        actions.set_button("jump", &[ButtonSource::Key(Key::A)]);
        assert!(actions.released("jump") && actions.fixed_released("jump"));
        assert!(!actions.down(&input, "jump"));
        assert!(!actions.pressed("jump"));

        actions.set_button("jump", &[ButtonSource::Key(Key::Space), ButtonSource::Mouse(MouseButton::Left)]);
        assert!(actions.pressed("jump") && actions.fixed_pressed("jump"));
        assert!(actions.released("jump"));
        assert!(actions.down(&input, "jump"));
    }

    #[test]
    fn opposing_axis_bindings_cancel_and_mouse_buttons_bind() {
        let mut input = Input::default();
        let mut actions = Actions::default();
        actions.bind_axis("move_x", ButtonSource::Key(Key::A), ButtonSource::Key(Key::D));
        actions.bind_axis("move_x", ButtonSource::Key(Key::Left), ButtonSource::Key(Key::Right));
        actions.bind_axis("move_y", ButtonSource::Key(Key::S), ButtonSource::Key(Key::W));
        actions.bind_button("fire", ButtonSource::Mouse(MouseButton::Left));
        input.press(Key::A);
        input.press(Key::D);
        actions.sync(&input);
        assert_eq!(actions.axis(&input, "move_x"), 0.0);
        input.release(Key::A);
        assert_eq!(actions.axis(&input, "move_x"), 1.0);
        input.press(Key::Left);
        assert_eq!(actions.axis(&input, "move_x"), 0.0);
        input.release(Key::D);
        input.release(Key::Left);
        input.press(Key::W);
        assert_eq!(actions.axis2(&input, "move_x", "move_y"), Vec2::new(0.0, 1.0));
        actions.set_axis("move_x", &[ButtonSource::Key(Key::A)], &[ButtonSource::Key(Key::D)]);
        input.press(Key::Right);
        assert_eq!(actions.axis(&input, "move_x"), 0.0);
        input.press_mouse(MouseButton::Left);
        actions.sync(&input);
        assert!(actions.pressed("fire") && actions.down(&input, "fire"));
        assert_eq!(actions.axis(&input, "missing"), 0.0);
        assert!(!actions.down(&input, "missing"));
    }

    #[test]
    fn a_concealed_action_waits_for_the_next_publish() {
        let (mut input, mut actions) = jump();
        actions.conceal_fixed_edges();
        input.press(Key::Space);
        actions.sync(&input);
        assert!(actions.pressed("jump"));
        assert!(!actions.fixed_pressed("jump"));
        actions.end_frame();
        assert!(!actions.pressed("jump"));
        actions.publish_fixed_edges();
        assert!(actions.fixed_pressed("jump"));
        assert!(actions.fixed_pressed("jump"));
    }
}
