//! 全局热键组合解析（M5）
//!
//! 把 `Ctrl+Shift+A`、`F1` 这类配置字符串解析为平台无关的按键组合，
//! 再由各平台映射为注册所需的键码：
//! - Windows：虚拟键码（`VK_*`）
//! - macOS：`global-hotkey` 的 `Code`

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::to_code;

/// 主键
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// 字母或数字（大写）
    Char(char),
    /// F1 - F24
    Function(u8),
    /// 具名按键
    Named(NamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    PrintScreen,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
    Tab,
    Enter,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Backquote,
    Minus,
    Equal,
}

/// 一个按键组合
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    pub key: Key,
}

impl Combo {
    /// Windows 虚拟键码（VK_*）
    #[cfg(windows)]
    pub fn vk(&self) -> u32 {
        windows::to_vk(self.key)
    }
}

/// 解析快捷键字符串，例如 `Ctrl+Shift+A`、`F2`、`Alt+PrintScreen`
pub fn parse(text: &str) -> Result<Combo, String> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut win = false;
    let mut key: Option<&str> = None;

    for part in text.split('+').map(str::trim).filter(|s| !s.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" | "option" | "opt" => alt = true,
            "win" | "super" | "meta" | "cmd" | "command" => win = true,
            _ => {
                if key.is_some() {
                    return Err(format!("只能包含一个主键，多余的「{part}」"));
                }
                key = Some(part);
            }
        }
    }

    let key = key.ok_or_else(|| "缺少主键（如 A、F1）".to_string())?;
    let key = key_to_key(key).ok_or_else(|| format!("无法识别的按键「{key}」"))?;
    Ok(Combo { ctrl, shift, alt, win, key })
}

/// 主键名称 → 平台无关按键
fn key_to_key(key: &str) -> Option<Key> {
    let upper = key.to_ascii_uppercase();

    // 字母 / 数字
    if upper.len() == 1 {
        let ch = upper.chars().next()?;
        if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
            return Some(Key::Char(ch));
        }
    }

    // F1 - F24
    if let Some(number) = upper.strip_prefix('F').and_then(|s| s.parse::<u32>().ok()) {
        if (1..=24).contains(&number) {
            return Some(Key::Function(number as u8));
        }
    }

    let named = match upper.as_str() {
        "PRINTSCREEN" | "PRTSC" => NamedKey::PrintScreen,
        "INSERT" | "INS" => NamedKey::Insert,
        "DELETE" | "DEL" => NamedKey::Delete,
        "HOME" => NamedKey::Home,
        "END" => NamedKey::End,
        "PAGEUP" | "PGUP" => NamedKey::PageUp,
        "PAGEDOWN" | "PGDN" => NamedKey::PageDown,
        "SPACE" => NamedKey::Space,
        "TAB" => NamedKey::Tab,
        "ENTER" | "RETURN" => NamedKey::Enter,
        "ESC" | "ESCAPE" => NamedKey::Escape,
        "UP" => NamedKey::Up,
        "DOWN" => NamedKey::Down,
        "LEFT" => NamedKey::Left,
        "RIGHT" => NamedKey::Right,
        "BACKQUOTE" | "`" => NamedKey::Backquote,
        "MINUS" | "-" => NamedKey::Minus,
        "EQUAL" | "=" => NamedKey::Equal,
        _ => return None,
    };
    Some(Key::Named(named))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_and_letter() {
        let combo = parse("Ctrl+Shift+A").expect("应能解析");
        assert!(combo.ctrl && combo.shift && !combo.alt && !combo.win);
        assert_eq!(combo.key, Key::Char('A'));
    }

    #[test]
    fn parses_function_key_without_modifiers() {
        let combo = parse("F2").expect("应能解析");
        assert!(!combo.ctrl && !combo.shift && !combo.alt && !combo.win);
        assert_eq!(combo.key, Key::Function(2));
    }

    #[test]
    fn parses_aliases_and_whitespace() {
        let combo = parse(" alt + prtsc ").expect("应能解析");
        assert!(combo.alt);
        assert_eq!(combo.key, Key::Named(NamedKey::PrintScreen));

        let combo = parse("Win+Space").expect("应能解析");
        assert!(combo.win);
        assert_eq!(combo.key, Key::Named(NamedKey::Space));

        // macOS 习惯写法
        let combo = parse("Cmd+Option+D").expect("应能解析");
        assert!(combo.win && combo.alt);
        assert_eq!(combo.key, Key::Char('D'));
    }

    #[test]
    fn rejects_invalid_input() {
        assert!(parse("Ctrl+").is_err(), "缺少主键应报错");
        assert!(parse("Ctrl+A+B").is_err(), "多个主键应报错");
        assert!(parse("Ctrl+NoSuchKey").is_err(), "未知按键应报错");
        assert!(parse("F25").is_err(), "超出 F1-F24 应报错");
    }
}
