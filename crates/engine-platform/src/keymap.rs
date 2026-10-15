use std::collections::HashMap;

use engine_core::Key;
use winit::keyboard::{KeyCode, PhysicalKey};

/// Maps physical key positions (layout independent: WASD stays WASD on AZERTY) to engine keys.
///
/// Left and right Shift, Control, and Alt map to one [`Key`] each. [`HeldKeys`] turns those
/// physical presses into one logical down and one logical up.
#[rustfmt::skip]
pub fn map_key(key: PhysicalKey) -> Option<Key> {
    let PhysicalKey::Code(code) = key else { return None };
    use KeyCode::*;
    Some(match code {
        KeyA => Key::A, KeyB => Key::B, KeyC => Key::C, KeyD => Key::D, KeyE => Key::E, KeyF => Key::F,
        KeyG => Key::G, KeyH => Key::H, KeyI => Key::I, KeyJ => Key::J, KeyK => Key::K, KeyL => Key::L,
        KeyM => Key::M, KeyN => Key::N, KeyO => Key::O, KeyP => Key::P, KeyQ => Key::Q, KeyR => Key::R,
        KeyS => Key::S, KeyT => Key::T, KeyU => Key::U, KeyV => Key::V, KeyW => Key::W, KeyX => Key::X,
        KeyY => Key::Y, KeyZ => Key::Z,
        Digit0 => Key::Digit0, Digit1 => Key::Digit1, Digit2 => Key::Digit2, Digit3 => Key::Digit3,
        Digit4 => Key::Digit4, Digit5 => Key::Digit5, Digit6 => Key::Digit6, Digit7 => Key::Digit7,
        Digit8 => Key::Digit8, Digit9 => Key::Digit9,
        ArrowUp => Key::Up, ArrowDown => Key::Down, ArrowLeft => Key::Left, ArrowRight => Key::Right,
        Space => Key::Space, Enter => Key::Enter, Escape => Key::Escape, Tab => Key::Tab,
        ShiftLeft | ShiftRight => Key::Shift,
        ControlLeft | ControlRight => Key::Control,
        AltLeft | AltRight => Key::Alt,
        F1 => Key::F1, F2 => Key::F2, F3 => Key::F3, F4 => Key::F4, F5 => Key::F5, F6 => Key::F6,
        F7 => Key::F7, F8 => Key::F8, F9 => Key::F9, F10 => Key::F10, F11 => Key::F11, F12 => Key::F12,
        Backquote => Key::Backquote, Minus => Key::Minus, Equal => Key::Equal,
        BracketLeft => Key::BracketLeft, BracketRight => Key::BracketRight, Backslash => Key::Backslash,
        Semicolon => Key::Semicolon, Quote => Key::Quote, Comma => Key::Comma, Period => Key::Period,
        Slash => Key::Slash,
        Backspace => Key::Backspace, Delete => Key::Delete, Insert => Key::Insert, Home => Key::Home,
        End => Key::End, PageUp => Key::PageUp, PageDown => Key::PageDown,
        Numpad0 => Key::Numpad0, Numpad1 => Key::Numpad1, Numpad2 => Key::Numpad2, Numpad3 => Key::Numpad3,
        Numpad4 => Key::Numpad4, Numpad5 => Key::Numpad5, Numpad6 => Key::Numpad6, Numpad7 => Key::Numpad7,
        Numpad8 => Key::Numpad8, Numpad9 => Key::Numpad9,
        NumpadAdd => Key::NumpadAdd, NumpadSubtract => Key::NumpadSubtract,
        NumpadMultiply => Key::NumpadMultiply, NumpadDivide => Key::NumpadDivide,
        NumpadDecimal => Key::NumpadDecimal, NumpadEnter => Key::NumpadEnter,
        _ => return None,
    })
}

/// True for the three logical keys that collapse a left and a right physical key.
pub fn is_collapsed_modifier(key: Key) -> bool {
    matches!(key, Key::Shift | Key::Control | Key::Alt)
}

/// How many physical keys are currently down for each collapsed modifier.
///
/// `Key::Shift` stays down while either Shift key is down. Releasing one side does not emit a
/// logical release until the other side is up too. The same rule applies to Control and Alt.
/// Other keys are not counted: each of their events is forwarded as it arrives.
#[derive(Debug, Default)]
pub struct HeldKeys {
    counts: HashMap<Key, u32>,
}

impl HeldKeys {
    /// `Some(true)` when a logical key goes down, `Some(false)` when it goes up, `None` when a
    /// collapsed modifier is pressed or released but its logical key does not change.
    pub fn update(&mut self, key: Key, pressed: bool) -> Option<bool> {
        if !is_collapsed_modifier(key) {
            return Some(pressed);
        }
        let count = self.counts.entry(key).or_insert(0);
        if pressed {
            *count = count.saturating_add(1);
            (*count == 1).then_some(true)
        } else if *count == 0 {
            None
        } else {
            *count -= 1;
            (*count == 0).then_some(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_physical_keys() {
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::KeyW)), Some(Key::W));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::ShiftRight)), Some(Key::Shift));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::F1)), Some(Key::F1));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::F12)), Some(Key::F12));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::Backquote)), Some(Key::Backquote));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::Slash)), Some(Key::Slash));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::PageDown)), Some(Key::PageDown));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::NumpadEnter)), Some(Key::NumpadEnter));
        assert_ne!(map_key(PhysicalKey::Code(KeyCode::NumpadEnter)), Some(Key::Enter));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::F13)), None);
        assert_eq!(map_key(PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified)), None);
    }

    #[test]
    fn collapsed_modifier_stays_down_until_both_sides_release() {
        let mut held = HeldKeys::default();
        assert_eq!(held.update(Key::Shift, true), Some(true));
        assert_eq!(held.update(Key::Shift, true), None);
        assert_eq!(held.update(Key::Shift, false), None);
        assert_eq!(held.update(Key::Shift, false), Some(false));
        assert_eq!(held.update(Key::Shift, false), None);
    }

    #[test]
    fn collapsed_modifiers_are_independent_and_other_keys_pass_through() {
        let mut held = HeldKeys::default();
        assert_eq!(held.update(Key::Control, true), Some(true));
        assert_eq!(held.update(Key::Alt, true), Some(true));
        assert_eq!(held.update(Key::Control, false), Some(false));
        assert_eq!(held.update(Key::Alt, true), None);
        assert_eq!(held.update(Key::W, true), Some(true));
        assert_eq!(held.update(Key::W, false), Some(false));
    }
}
