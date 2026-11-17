//! 平台无关按键 → `global-hotkey` 的 `Code`

use global_hotkey::hotkey::Code;

use super::{Key, NamedKey};

/// 主键 → macOS 键码
pub fn to_code(key: Key) -> Code {
    match key {
        Key::Char(ch) => char_to_code(ch),
        Key::Function(number) => function_to_code(number),
        Key::Named(named) => named_to_code(named),
    }
}

fn char_to_code(ch: char) -> Code {
    match ch {
        'A' => Code::KeyA,
        'B' => Code::KeyB,
        'C' => Code::KeyC,
        'D' => Code::KeyD,
        'E' => Code::KeyE,
        'F' => Code::KeyF,
        'G' => Code::KeyG,
        'H' => Code::KeyH,
        'I' => Code::KeyI,
        'J' => Code::KeyJ,
        'K' => Code::KeyK,
        'L' => Code::KeyL,
        'M' => Code::KeyM,
        'N' => Code::KeyN,
        'O' => Code::KeyO,
        'P' => Code::KeyP,
        'Q' => Code::KeyQ,
        'R' => Code::KeyR,
        'S' => Code::KeyS,
        'T' => Code::KeyT,
        'U' => Code::KeyU,
        'V' => Code::KeyV,
        'W' => Code::KeyW,
        'X' => Code::KeyX,
        'Y' => Code::KeyY,
        'Z' => Code::KeyZ,
        '0' => Code::Digit0,
        '1' => Code::Digit1,
        '2' => Code::Digit2,
        '3' => Code::Digit3,
        '4' => Code::Digit4,
        '5' => Code::Digit5,
        '6' => Code::Digit6,
        '7' => Code::Digit7,
        '8' => Code::Digit8,
        '9' => Code::Digit9,
        // 解析阶段已保证只出现字母数字
        _ => Code::KeyA,
    }
}

fn function_to_code(number: u8) -> Code {
    match number {
        1 => Code::F1,
        2 => Code::F2,
        3 => Code::F3,
        4 => Code::F4,
        5 => Code::F5,
        6 => Code::F6,
        7 => Code::F7,
        8 => Code::F8,
        9 => Code::F9,
        10 => Code::F10,
        11 => Code::F11,
        12 => Code::F12,
        13 => Code::F13,
        14 => Code::F14,
        15 => Code::F15,
        16 => Code::F16,
        17 => Code::F17,
        18 => Code::F18,
        19 => Code::F19,
        20 => Code::F20,
        21 => Code::F21,
        22 => Code::F22,
        23 => Code::F23,
        _ => Code::F24,
    }
}

fn named_to_code(named: NamedKey) -> Code {
    match named {
        NamedKey::PrintScreen => Code::PrintScreen,
        NamedKey::Insert => Code::Insert,
        NamedKey::Delete => Code::Delete,
        NamedKey::Home => Code::Home,
        NamedKey::End => Code::End,
        NamedKey::PageUp => Code::PageUp,
        NamedKey::PageDown => Code::PageDown,
        NamedKey::Space => Code::Space,
        NamedKey::Tab => Code::Tab,
        NamedKey::Enter => Code::Enter,
        NamedKey::Escape => Code::Escape,
        NamedKey::Up => Code::ArrowUp,
        NamedKey::Down => Code::ArrowDown,
        NamedKey::Left => Code::ArrowLeft,
        NamedKey::Right => Code::ArrowRight,
        NamedKey::Backquote => Code::Backquote,
        NamedKey::Minus => Code::Minus,
        NamedKey::Equal => Code::Equal,
    }
}
