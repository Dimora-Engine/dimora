use engine_core::Key;
use winit::keyboard::{KeyCode, PhysicalKey};

/// Maps physical key positions (layout independent: WASD stays WASD on AZERTY) to engine keys.
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
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_physical_keys() {
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::KeyW)), Some(Key::W));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::ShiftRight)), Some(Key::Shift));
        assert_eq!(map_key(PhysicalKey::Code(KeyCode::F13)), None);
    }
}
