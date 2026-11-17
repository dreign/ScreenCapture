//! 平台无关按键 → Windows 虚拟键码（VK_*）

use super::{Key, NamedKey};

/// 主键 → 虚拟键码
pub fn to_vk(key: Key) -> u32 {
    match key {
        Key::Char(ch) => ch as u32,
        Key::Function(number) => 0x70 + u32::from(number) - 1,
        Key::Named(named) => named_to_vk(named),
    }
}

fn named_to_vk(named: NamedKey) -> u32 {
    match named {
        NamedKey::PrintScreen => 0x2C,
        NamedKey::Insert => 0x2D,
        NamedKey::Delete => 0x2E,
        NamedKey::Home => 0x24,
        NamedKey::End => 0x23,
        NamedKey::PageUp => 0x21,
        NamedKey::PageDown => 0x22,
        NamedKey::Space => 0x20,
        NamedKey::Tab => 0x09,
        NamedKey::Enter => 0x0D,
        NamedKey::Escape => 0x1B,
        NamedKey::Up => 0x26,
        NamedKey::Down => 0x28,
        NamedKey::Left => 0x25,
        NamedKey::Right => 0x27,
        NamedKey::Backquote => 0xC0,
        NamedKey::Minus => 0xBD,
        NamedKey::Equal => 0xBB,
    }
}

#[cfg(test)]
mod tests {
    use crate::hotkey::parse;

    #[test]
    fn maps_letters_function_and_named_keys() {
        assert_eq!(parse("Ctrl+Shift+A").expect("应能解析").vk(), 0x41);
        assert_eq!(parse("F2").expect("应能解析").vk(), 0x71);
        assert_eq!(parse("Alt+PrintScreen").expect("应能解析").vk(), 0x2C);
        assert_eq!(parse("Win+Space").expect("应能解析").vk(), 0x20);
        assert_eq!(parse("Ctrl+9").expect("应能解析").vk(), 0x39);
    }

    #[test]
    fn maps_all_function_keys() {
        for number in 1..=24u8 {
            let combo = parse(&format!("F{number}")).expect("应能解析");
            assert_eq!(combo.vk(), 0x70 + u32::from(number) - 1);
        }
    }
}
