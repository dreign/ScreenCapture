//! 配置管理模块
//! 负责加载、保存应用配置，以及开机自启动

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub hotkeys: HotkeyConfig,
    pub editor: EditorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub save_path: PathBuf,
    pub auto_save: bool,
    pub copy_to_clipboard: bool,
    pub file_format: String,
    pub capture_cursor: bool,
    /// 文件名模板，支持 {date} {time} {datetime} {unix} {index}
    #[serde(default = "default_file_name_template")]
    pub file_name_template: String,
    /// JPEG 质量（1-100）
    #[serde(default = "default_jpeg_quality")]
    pub jpeg_quality: u8,
    /// 延迟截图秒数
    #[serde(default = "default_delay_seconds")]
    pub delay_seconds: u32,
}

fn default_file_name_template() -> String {
    "Screenshot_{date}_{time}".to_string()
}

fn default_jpeg_quality() -> u8 {
    92
}

fn default_delay_seconds() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub region: String,
    pub fullscreen: String,
    pub window: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub default_color: String,
    pub default_thickness: f32,
    pub show_magnifier: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                save_path: default_save_path(),
                auto_save: false,
                copy_to_clipboard: true,
                file_format: "png".to_string(),
                capture_cursor: false,
                file_name_template: default_file_name_template(),
                jpeg_quality: default_jpeg_quality(),
                delay_seconds: default_delay_seconds(),
            },
            hotkeys: HotkeyConfig {
                region: "Ctrl+Shift+A".to_string(),
                fullscreen: "Ctrl+Shift+F".to_string(),
                window: "Ctrl+Shift+W".to_string(),
            },
            editor: EditorConfig { default_color: "#FF0000".to_string(), default_thickness: 3.0, show_magnifier: true },
        }
    }
}

impl Config {
    /// 配置文件路径
    pub fn config_path() -> PathBuf {
        config_path()
    }

    /// 加载配置；文件不存在时返回默认配置
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&config_path())
    }

    /// 从指定路径加载
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    /// 保存配置
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&config_path())
    }

    /// 保存到指定路径
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        use anyhow::Context;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("创建配置目录 {}", parent.display()))?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)
            .with_context(|| format!("写入配置文件 {}", path.display()))?;
        Ok(())
    }
}

// ---------------- 平台路径 ----------------

#[cfg(windows)]
fn default_save_path() -> PathBuf {
    PathBuf::from(r"C:\Users\Public\Pictures\ScreenCapture")
}

#[cfg(target_os = "macos")]
fn default_save_path() -> PathBuf {
    home_dir().join("Pictures").join("ScreenCapture")
}

#[cfg(not(any(windows, target_os = "macos")))]
fn default_save_path() -> PathBuf {
    std::env::temp_dir().join("ScreenCapture")
}

/// Windows：%APPDATA%\ScreenCapture\config.toml
#[cfg(windows)]
fn config_path() -> PathBuf {
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    base.join("ScreenCapture").join("config.toml")
}

/// macOS：~/Library/Application Support/ScreenCapture/config.toml
#[cfg(target_os = "macos")]
fn config_path() -> PathBuf {
    home_dir().join("Library").join("Application Support").join("ScreenCapture").join("config.toml")
}

#[cfg(not(any(windows, target_os = "macos")))]
fn config_path() -> PathBuf {
    std::env::temp_dir().join("ScreenCapture").join("config.toml")
}

#[cfg(target_os = "macos")]
fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

// ---------------- 开机自启动 ----------------

/// 开机启动是否已启用
#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    const AUTOSTART_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const AUTOSTART_VALUE: &str = "ScreenCapture";

    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    };

    let subkey = wide(AUTOSTART_KEY);
    let value_name = wide(AUTOSTART_VALUE);
    unsafe {
        let mut key = HKEY::default();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), 0, KEY_QUERY_VALUE, &mut key);
        if status.0 != 0 {
            return false;
        }
        let mut size = 0u32;
        let status = RegQueryValueExW(key, PCWSTR(value_name.as_ptr()), None, None, None, Some(&mut size));
        let _ = RegCloseKey(key);
        status.0 == 0 && size > 0
    }
}

/// 开启 / 关闭开机自启动（写入 HKCU\...\Run）
#[cfg(windows)]
pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    const AUTOSTART_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const AUTOSTART_VALUE: &str = "ScreenCapture";

    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
        KEY_SET_VALUE, REG_SZ,
    };

    let subkey = wide(AUTOSTART_KEY);
    let value_name = wide(AUTOSTART_VALUE);
    unsafe {
        let mut key = HKEY::default();
        let status =
            RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), 0, KEY_SET_VALUE | KEY_QUERY_VALUE, &mut key);
        if status.0 != 0 {
            anyhow::bail!("打开注册表 Run 键失败（错误码 {}）", status.0);
        }

        let result = if enabled {
            let exe = std::env::current_exe()?;
            let command = format!("\"{}\"", exe.display());
            let data = wide(&command);
            let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
            RegSetValueExW(key, PCWSTR(value_name.as_ptr()), 0, REG_SZ, Some(bytes))
        } else {
            RegDeleteValueW(key, PCWSTR(value_name.as_ptr()))
        };
        let _ = RegCloseKey(key);

        // 删除不存在的值时返回 ERROR_FILE_NOT_FOUND，视为成功
        if result.0 != 0 && !(result.0 == 2 && !enabled) {
            anyhow::bail!("写入注册表失败（错误码 {}）", result.0);
        }
        Ok(())
    }
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "macos")]
const LAUNCH_AGENT_LABEL: &str = "com.screencapture.app";

#[cfg(target_os = "macos")]
fn launch_agent_path() -> PathBuf {
    home_dir().join("Library").join("LaunchAgents").join(format!("{LAUNCH_AGENT_LABEL}.plist"))
}

/// 开机启动是否已启用（存在 LaunchAgent plist）
#[cfg(target_os = "macos")]
pub fn autostart_enabled() -> bool {
    launch_agent_path().exists()
}

/// 开启 / 关闭开机自启动（写入 ~/Library/LaunchAgents）
#[cfg(target_os = "macos")]
pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    let path = launch_agent_path();
    if !enabled {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let arguments = launch_arguments();
    let program_arguments = arguments
        .iter()
        .map(|arg| format!("        <string>{}</string>", xml_escape(arg)))
        .collect::<Vec<_>>()
        .join("\n");
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCH_AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
{program_arguments}
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#
    );
    std::fs::write(&path, plist)?;
    Ok(())
}

/// 位于 .app 包内时通过 `open -a` 启动整个应用包，保证 Dock / 菜单栏行为正常
#[cfg(target_os = "macos")]
fn launch_arguments() -> Vec<String> {
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_text = exe.display().to_string();
    if let Some(index) = exe_text.find(".app/Contents/MacOS/") {
        let bundle = &exe_text[..index + 4];
        return vec!["/usr/bin/open".to_string(), "-a".to_string(), bundle.to_string()];
    }
    vec![exe_text]
}

#[cfg(target_os = "macos")]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn autostart_enabled() -> bool {
    false
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn set_autostart(_enabled: bool) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_dir() -> PathBuf {
        std::env::temp_dir().join("screencapture_config_test")
    }

    #[test]
    fn config_save_load_roundtrip() {
        let dir = temp_config_dir();
        let path = dir.join("config.toml");

        let mut config = Config::default();
        config.hotkeys.region = "Ctrl+Alt+Q".to_string();
        config.editor.default_thickness = 7.0;
        config.save_to(&path).expect("保存配置应成功");

        let loaded = Config::load_from(&path).expect("加载配置应成功");
        assert_eq!(loaded.hotkeys.region, "Ctrl+Alt+Q");
        assert_eq!(loaded.editor.default_thickness, 7.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_config_returns_default() {
        let path = std::env::temp_dir().join("screencapture_missing_config_xyz.toml");
        let _ = std::fs::remove_file(&path);
        let loaded = Config::load_from(&path).expect("缺失配置应返回默认值");
        assert_eq!(loaded.general.file_format, "png");
    }
}
