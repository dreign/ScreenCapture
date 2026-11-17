//! 系统托盘 + 全局热键服务（M5）
//!
//! 平台实现：
//! - Windows：独立后台线程 + 隐藏窗口（`Shell_NotifyIconW` / `RegisterHotKey`），
//!   通过 `std::sync::mpsc` 与 UI 通信
//! - macOS：主线程创建菜单栏图标（tray-icon）与全局热键（global-hotkey），
//!   事件通过全局 channel 在 [`TrayService::try_recv`] 中轮询

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::TrayService;
#[cfg(windows)]
pub use windows::TrayService;

use crate::hotkey::Combo;

const HOTKEY_REGION: i32 = 1;
const HOTKEY_FULLSCREEN: i32 = 2;
const HOTKEY_WINDOW: i32 = 3;

/// 截图类型（托盘 / 热键触发）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureKind {
    Fullscreen,
    Region,
    Window,
}

/// 托盘线程回传给 UI 的事件
#[derive(Debug)]
pub enum TrayAction {
    Capture(CaptureKind),
    ShowMainWindow,
    Quit,
    /// 热键批量注册结果：(名称, 结果)
    HotkeysRegistered(Vec<(String, Result<(), String>)>),
}

/// 一条热键注册请求
#[derive(Debug, Clone)]
pub struct HotkeySpec {
    pub id: i32,
    pub name: String,
    pub combo: Combo,
}

impl HotkeySpec {
    pub fn region(combo: Combo) -> Self {
        Self { id: HOTKEY_REGION, name: "区域截图".to_string(), combo }
    }

    pub fn fullscreen(combo: Combo) -> Self {
        Self { id: HOTKEY_FULLSCREEN, name: "全屏截图".to_string(), combo }
    }

    pub fn window(combo: Combo) -> Self {
        Self { id: HOTKEY_WINDOW, name: "窗口截图".to_string(), combo }
    }
}
