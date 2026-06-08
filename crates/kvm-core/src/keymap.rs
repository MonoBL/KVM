/// Cross-platform key code used on the wire.
/// u32 values are USB HID usage codes (usage page 0x07), with the
/// 0xE0..0xE7 range used for the modifier keys.
use enigo::Key as EnigoKey;
use rdev::Key as RdevKey;

/// Convert an rdev key to a wire code.
/// Returns None for keys that have no useful mapping.
pub fn rdev_to_code(key: &RdevKey) -> Option<u32> {
    use RdevKey::*;
    let code = match key {
        // modifiers (HID 0xE0..0xE7)
        ControlLeft => 0xe0,
        ShiftLeft => 0xe1,
        Alt => 0xe2, // left alt / option
        MetaLeft => 0xe3,
        ControlRight => 0xe4,
        ShiftRight => 0xe5,
        AltGr => 0xe6, // right alt
        MetaRight => 0xe7,
        // control keys
        Backspace => 0x2a,
        CapsLock => 0x39,
        Delete => 0x4c,
        DownArrow => 0x51,
        End => 0x4d,
        Escape => 0x29,
        F1 => 0x3a,
        F2 => 0x3b,
        F3 => 0x3c,
        F4 => 0x3d,
        F5 => 0x3e,
        F6 => 0x3f,
        F7 => 0x40,
        F8 => 0x41,
        F9 => 0x42,
        F10 => 0x43,
        F11 => 0x44,
        F12 => 0x45,
        Home => 0x4a,
        LeftArrow => 0x50,
        PageDown => 0x4e,
        PageUp => 0x4b,
        Return => 0x28,
        RightArrow => 0x4f,
        Space => 0x2c,
        Tab => 0x2b,
        UpArrow => 0x52,
        PrintScreen => 0x46,
        ScrollLock => 0x47,
        Pause => 0x48,
        NumLock => 0x53,
        Insert => 0x49,
        // number row
        Num1 => 0x1e,
        Num2 => 0x1f,
        Num3 => 0x20,
        Num4 => 0x21,
        Num5 => 0x22,
        Num6 => 0x23,
        Num7 => 0x24,
        Num8 => 0x25,
        Num9 => 0x26,
        Num0 => 0x27,
        // letters
        KeyA => 0x04,
        KeyB => 0x05,
        KeyC => 0x06,
        KeyD => 0x07,
        KeyE => 0x08,
        KeyF => 0x09,
        KeyG => 0x0a,
        KeyH => 0x0b,
        KeyI => 0x0c,
        KeyJ => 0x0d,
        KeyK => 0x0e,
        KeyL => 0x0f,
        KeyM => 0x10,
        KeyN => 0x11,
        KeyO => 0x12,
        KeyP => 0x13,
        KeyQ => 0x14,
        KeyR => 0x15,
        KeyS => 0x16,
        KeyT => 0x17,
        KeyU => 0x18,
        KeyV => 0x19,
        KeyW => 0x1a,
        KeyX => 0x1b,
        KeyY => 0x1c,
        KeyZ => 0x1d,
        // punctuation
        Minus => 0x2d,
        Equal => 0x2e,
        LeftBracket => 0x2f,
        RightBracket => 0x30,
        BackSlash => 0x31,
        SemiColon => 0x33,
        Quote => 0x34,
        BackQuote => 0x35,
        Comma => 0x36,
        Dot => 0x37,
        Slash => 0x38,
        IntlBackslash => 0x64,
        // keypad
        KpReturn => 0x58,
        KpMinus => 0x56,
        KpPlus => 0x57,
        KpMultiply => 0x55,
        KpDivide => 0x54,
        Kp0 => 0x62,
        Kp1 => 0x59,
        Kp2 => 0x5a,
        Kp3 => 0x5b,
        Kp4 => 0x5c,
        Kp5 => 0x5d,
        Kp6 => 0x5e,
        Kp7 => 0x5f,
        Kp8 => 0x60,
        Kp9 => 0x61,
        KpDelete => 0x63,
        Function => 0xff01,
        Unknown(n) => *n,
        #[allow(unreachable_patterns)]
        _ => return None,
    };
    Some(code)
}

/// Map an ASCII character to the best enigo key for the current platform.
///
/// On Windows, letters and digits must use the named virtual-key variants
/// (`Key::A`, `Key::Num0`, ...) so they combine with held modifiers
/// (e.g. Ctrl+C). `Key::Unicode` on Windows uses KEYEVENTF_UNICODE which
/// ignores modifiers. On macOS/Linux `Key::Unicode` resolves to a real
/// layout-dependent keycode that does combine with modifiers.
#[cfg(target_os = "windows")]
fn char_key(c: char) -> Option<EnigoKey> {
    use EnigoKey::*;
    let k = match c {
        'a' => A, 'b' => B, 'c' => C, 'd' => D, 'e' => E, 'f' => F,
        'g' => G, 'h' => H, 'i' => I, 'j' => J, 'k' => K, 'l' => L,
        'm' => M, 'n' => N, 'o' => O, 'p' => P, 'q' => Q, 'r' => R,
        's' => S, 't' => T, 'u' => U, 'v' => V, 'w' => W, 'x' => X,
        'y' => Y, 'z' => Z,
        '0' => Num0, '1' => Num1, '2' => Num2, '3' => Num3, '4' => Num4,
        '5' => Num5, '6' => Num6, '7' => Num7, '8' => Num8, '9' => Num9,
        other => Unicode(other),
    };
    Some(k)
}

#[cfg(not(target_os = "windows"))]
fn char_key(c: char) -> Option<EnigoKey> {
    Some(EnigoKey::Unicode(c))
}

/// Convert a wire code back to an enigo key.
/// Returns None for codes that have no safe mapping on this platform; the
/// injector must skip those rather than fabricate a wrong keypress.
///
/// IMPORTANT: never pass a HID code into `Key::Other`. `Other` expects a
/// raw platform virtual keycode, not a HID usage, so it would inject the
/// wrong key.
pub fn code_to_enigo(code: u32) -> Option<EnigoKey> {
    use EnigoKey::*;
    let key = match code {
        // modifiers
        0xe0 => LControl,
        0xe1 => LShift,
        0xe2 => Alt,
        0xe3 => Meta,
        0xe4 => RControl,
        0xe5 => RShift,
        0xe6 => Alt, // right alt / AltGr
        0xe7 => Meta,
        // control keys
        0x28 => Return,
        0x2a => Backspace,
        0x2b => Tab,
        0x2c => Space,
        0x29 => Escape,
        0x39 => CapsLock,
        0x4c => Delete,
        0x4a => Home,
        0x4d => End,
        0x4b => PageUp,
        0x4e => PageDown,
        0x52 => UpArrow,
        0x51 => DownArrow,
        0x50 => LeftArrow,
        0x4f => RightArrow,
        0x3a => F1,
        0x3b => F2,
        0x3c => F3,
        0x3d => F4,
        0x3e => F5,
        0x3f => F6,
        0x40 => F7,
        0x41 => F8,
        0x42 => F9,
        0x43 => F10,
        0x44 => F11,
        0x45 => F12,
        // Insert: only exists on Windows/Linux in enigo
        #[cfg(not(target_os = "macos"))]
        0x49 => Insert,
        // letters
        0x04 => return char_key('a'),
        0x05 => return char_key('b'),
        0x06 => return char_key('c'),
        0x07 => return char_key('d'),
        0x08 => return char_key('e'),
        0x09 => return char_key('f'),
        0x0a => return char_key('g'),
        0x0b => return char_key('h'),
        0x0c => return char_key('i'),
        0x0d => return char_key('j'),
        0x0e => return char_key('k'),
        0x0f => return char_key('l'),
        0x10 => return char_key('m'),
        0x11 => return char_key('n'),
        0x12 => return char_key('o'),
        0x13 => return char_key('p'),
        0x14 => return char_key('q'),
        0x15 => return char_key('r'),
        0x16 => return char_key('s'),
        0x17 => return char_key('t'),
        0x18 => return char_key('u'),
        0x19 => return char_key('v'),
        0x1a => return char_key('w'),
        0x1b => return char_key('x'),
        0x1c => return char_key('y'),
        0x1d => return char_key('z'),
        // number row + keypad digits
        0x1e | 0x59 => return char_key('1'),
        0x1f | 0x5a => return char_key('2'),
        0x20 | 0x5b => return char_key('3'),
        0x21 | 0x5c => return char_key('4'),
        0x22 | 0x5d => return char_key('5'),
        0x23 | 0x5e => return char_key('6'),
        0x24 | 0x5f => return char_key('7'),
        0x25 | 0x60 => return char_key('8'),
        0x26 | 0x61 => return char_key('9'),
        0x27 | 0x62 => return char_key('0'),
        // punctuation + keypad ops (Unicode on every platform)
        0x2d => return char_key('-'),
        0x2e => return char_key('='),
        0x2f => return char_key('['),
        0x30 => return char_key(']'),
        0x31 => return char_key('\\'),
        0x33 => return char_key(';'),
        0x34 => return char_key('\''),
        0x35 => return char_key('`'),
        0x36 | 0x63 => return char_key(','),
        0x37 => return char_key('.'),
        0x38 | 0x54 => return char_key('/'),
        0x55 => return char_key('*'),
        0x56 => return char_key('-'),
        0x57 => return char_key('+'),
        0x64 => return char_key('\\'),
        0x58 => Return, // keypad enter
        // unmapped (PrintScreen, ScrollLock, Pause, NumLock, Function, Insert
        // on macOS, anything else): skip rather than inject garbage.
        // NOTE: fully qualified because `use EnigoKey::*` brings `Key::None`
        // into scope on Windows and `Key::Option` on macOS, shadowing both a
        // bare `None` and a bare `Option`.
        _ => return ::std::option::Option::None,
    };
    ::std::option::Option::Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_alpha_keys() {
        let pairs = [
            (RdevKey::KeyA, 0x04u32),
            (RdevKey::KeyZ, 0x1d),
            (RdevKey::Return, 0x28),
            (RdevKey::Space, 0x2c),
            (RdevKey::Escape, 0x29),
            (RdevKey::ShiftLeft, 0xe1),
            (RdevKey::ControlLeft, 0xe0),
            (RdevKey::F1, 0x3a),
            (RdevKey::F12, 0x45),
        ];
        for (key, expected_code) in pairs {
            let code = rdev_to_code(&key).expect("should have code");
            assert_eq!(code, expected_code, "wrong code for {:?}", key);
            assert!(code_to_enigo(code).is_some(), "no enigo key for {:?}", key);
        }
    }

    /// C3 regression: Alt and Slash must NOT share a wire code.
    #[test]
    fn alt_and_slash_differ() {
        let alt = rdev_to_code(&RdevKey::Alt).unwrap();
        let slash = rdev_to_code(&RdevKey::Slash).unwrap();
        assert_ne!(alt, slash, "Alt and Slash collided");
        assert_eq!(alt, 0xe2);
        assert_eq!(slash, 0x38);
    }

    /// No two distinct rdev keys may map to the same wire code.
    #[test]
    fn no_code_collisions() {
        use RdevKey::*;
        let keys = [
            ControlLeft, ShiftLeft, Alt, MetaLeft, ControlRight, ShiftRight,
            AltGr, MetaRight, Backspace, CapsLock, Delete, DownArrow, End,
            Escape, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, Home,
            LeftArrow, PageDown, PageUp, Return, RightArrow, Space, Tab,
            UpArrow, PrintScreen, ScrollLock, Pause, NumLock, Insert,
            Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9, Num0,
            KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK,
            KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV,
            KeyW, KeyX, KeyY, KeyZ, Minus, Equal, LeftBracket, RightBracket,
            BackSlash, SemiColon, Quote, BackQuote, Comma, Dot, Slash,
            IntlBackslash,
        ];
        let mut seen = std::collections::HashMap::new();
        for k in keys {
            let code = rdev_to_code(&k).unwrap();
            if let Some(prev) = seen.insert(code, format!("{:?}", k)) {
                panic!("code 0x{:x} shared by {:?} and {}", code, k, prev);
            }
        }
    }

    /// C4 regression: every code emitted for a non-keypad printable/named
    /// key must have a reverse mapping (no silent garbage).
    #[test]
    fn primary_keys_reverse_map() {
        use RdevKey::*;
        for k in [KeyA, Num0, Minus, Slash, Return, Tab, Space, F5, Home, Delete] {
            let code = rdev_to_code(&k).unwrap();
            assert!(code_to_enigo(code).is_some(), "no reverse for {:?}", k);
        }
    }

    #[test]
    fn unmapped_codes_return_none() {
        // Function key has no enigo equivalent.
        assert!(code_to_enigo(0xff01).is_none());
    }
}
