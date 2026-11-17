//! macOS 菜单栏托盘 + 全局热键
//!
//! macOS 要求状态栏图标与菜单在主线程创建，因此本实现不另起线程：
//! 菜单与热键事件通过全局 channel 回调写入队列，UI 在 [`TrayService::try_recv`]
//! 中取走，并借助 `egui::Context::request_repaint` 唤醒窗口。

use std::collections::VecDeque;
use std::sync::Arc;

use anyhow::anyhow;
use global_hotkey::hotkey::{HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use parking_lot::Mutex;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use eframe::egui;

use super::{CaptureKind, HotkeySpec, TrayAction, HOTKEY_FULLSCREEN, HOTKEY_REGION, HOTKEY_WINDOW};
use crate::hotkey::{self, Combo};

const MENU_FULLSCREEN: &str = "capture-fullscreen";
const MENU_REGION: &str = "capture-region";
const MENU_WINDOW: &str = "capture-window";
const MENU_SHOW: &str = "show-main-window";
const MENU_QUIT: &str = "quit";

/// 托盘服务句柄
pub struct TrayService {
    /// 必须持有，Drop 时才会移除菜单栏图标
    _tray: TrayIcon,
    hotkey_manager: GlobalHotKeyManager,
    /// 已注册热键（注销时需要）
    hotkeys: Mutex<Vec<HotKey>>,
    /// 热键 id → 截图类型
    hotkey_map: Arc<Mutex<Vec<(u32, CaptureKind)>>>,
    /// 回调线程写入、UI 线程取走
    pending: Arc<Mutex<VecDeque<TrayAction>>>,
}

impl TrayService {
    /// 在主线程创建菜单栏图标与热键管理器
    pub fn new(ctx: &egui::Context) -> anyhow::Result<Self> {
        let pending: Arc<Mutex<VecDeque<TrayAction>>> = Arc::new(Mutex::new(VecDeque::new()));

        let menu = Menu::new();
        for (id, label) in [(MENU_FULLSCREEN, "全屏截图"), (MENU_REGION, "区域截图"), (MENU_WINDOW, "窗口截图")]
        {
            menu.append(&MenuItem::with_id(id, label, true, None))
                .map_err(|err| anyhow!("创建托盘菜单项失败：{err}"))?;
        }
        menu.append(&PredefinedMenuItem::separator()).map_err(|err| anyhow!("创建托盘菜单失败：{err}"))?;
        menu.append(&MenuItem::with_id(MENU_SHOW, "显示主窗口", true, None))
            .map_err(|err| anyhow!("创建托盘菜单项失败：{err}"))?;
        menu.append(&PredefinedMenuItem::separator()).map_err(|err| anyhow!("创建托盘菜单失败：{err}"))?;
        menu.append(&MenuItem::with_id(MENU_QUIT, "退出", true, None))
            .map_err(|err| anyhow!("创建托盘菜单项失败：{err}"))?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ScreenCapture · 截图工具")
            .with_icon(build_icon()?)
            .with_icon_as_template(true)
            .build()
            .map_err(|err| anyhow!("创建菜单栏图标失败：{err}"))?;

        // 菜单点击 → 队列 + 唤醒 UI
        let menu_pending = pending.clone();
        let menu_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = match event.id.0.as_str() {
                MENU_FULLSCREEN => Some(TrayAction::Capture(CaptureKind::Fullscreen)),
                MENU_REGION => Some(TrayAction::Capture(CaptureKind::Region)),
                MENU_WINDOW => Some(TrayAction::Capture(CaptureKind::Window)),
                MENU_SHOW => Some(TrayAction::ShowMainWindow),
                MENU_QUIT => Some(TrayAction::Quit),
                _ => None,
            };
            if let Some(action) = action {
                menu_pending.lock().push_back(action);
                menu_ctx.request_repaint();
            }
        }));

        let hotkey_map: Arc<Mutex<Vec<(u32, CaptureKind)>>> = Arc::new(Mutex::new(Vec::new()));
        let event_map = hotkey_map.clone();
        let hotkey_pending = pending.clone();
        let hotkey_ctx = ctx.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state() != HotKeyState::Pressed {
                return;
            }
            let kind = event_map.lock().iter().find(|(id, _)| *id == event.id()).map(|(_, kind)| *kind);
            if let Some(kind) = kind {
                hotkey_pending.lock().push_back(TrayAction::Capture(kind));
                hotkey_ctx.request_repaint();
            }
        }));

        let hotkey_manager = GlobalHotKeyManager::new().map_err(|err| anyhow!("初始化全局热键失败：{err}"))?;

        Ok(Self { _tray: tray, hotkey_manager, hotkeys: Mutex::new(Vec::new()), hotkey_map, pending })
    }

    /// 弹出系统通知（通过 osascript，免去额外依赖）
    pub fn notify(&self, title: &str, body: &str, error: bool) {
        let prefix = if error { "[错误] " } else { "" };
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escape_applescript(&format!("{prefix}{body}")),
            escape_applescript(title)
        );
        // 不等待通知进程，避免阻塞 UI
        let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
    }

    /// 重新注册全局热键（会先注销旧热键）
    pub fn register_hotkeys(&self, specs: Vec<HotkeySpec>) {
        {
            let mut hotkeys = self.hotkeys.lock();
            for hotkey in hotkeys.drain(..) {
                let _ = self.hotkey_manager.unregister(hotkey);
            }
        }
        self.hotkey_map.lock().clear();

        let mut results = Vec::with_capacity(specs.len());
        for spec in specs {
            let hotkey = HotKey::new(Some(modifiers(spec.combo)), hotkey::to_code(spec.combo.key));
            if self.hotkey_map.lock().iter().any(|(id, _)| *id == hotkey.id()) {
                results.push((spec.name, Err("与同一组合的热键重复".to_string())));
                continue;
            }

            match self.hotkey_manager.register(hotkey) {
                Ok(()) => {
                    self.hotkeys.lock().push(hotkey);
                    if let Some(kind) = capture_kind(spec.id) {
                        self.hotkey_map.lock().push((hotkey.id(), kind));
                    }
                    results.push((spec.name, Ok(())));
                }
                Err(err) => results.push((spec.name, Err(err.to_string()))),
            }
        }

        self.pending.lock().push_back(TrayAction::HotkeysRegistered(results));
    }

    /// 非阻塞读取托盘 / 热键事件
    pub fn try_recv(&self) -> Option<TrayAction> {
        self.pending.lock().pop_front()
    }
}

fn modifiers(combo: Combo) -> Modifiers {
    let mut modifiers = Modifiers::empty();
    if combo.ctrl {
        modifiers |= Modifiers::CONTROL;
    }
    if combo.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if combo.alt {
        modifiers |= Modifiers::ALT;
    }
    if combo.win {
        modifiers |= Modifiers::SUPER;
    }
    modifiers
}

fn capture_kind(id: i32) -> Option<CaptureKind> {
    match id {
        HOTKEY_REGION => Some(CaptureKind::Region),
        HOTKEY_FULLSCREEN => Some(CaptureKind::Fullscreen),
        HOTKEY_WINDOW => Some(CaptureKind::Window),
        _ => None,
    }
}

/// 生成 32x32 的四角取景框模板图标（菜单栏按 alpha 着色）
fn build_icon() -> anyhow::Result<Icon> {
    const SIZE: i32 = 32;
    const THICK: i32 = 3;
    const INSET: i32 = 3;
    const ARM: i32 = 9;

    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    {
        let mut fill = |x0: i32, y0: i32, x1: i32, y1: i32| {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if x < 0 || y < 0 || x >= SIZE || y >= SIZE {
                        continue;
                    }
                    let index = ((y * SIZE + x) * 4) as usize;
                    rgba[index..index + 4].copy_from_slice(&[0, 0, 0, 255]);
                }
            }
        };

        let right = SIZE - 1 - INSET;
        let bottom = SIZE - 1 - INSET;
        // 左上
        fill(INSET, INSET, INSET + ARM, INSET + THICK - 1);
        fill(INSET, INSET, INSET + THICK - 1, INSET + ARM);
        // 右上
        fill(right - ARM, INSET, right, INSET + THICK - 1);
        fill(right - THICK + 1, INSET, right, INSET + ARM);
        // 左下
        fill(INSET, bottom - THICK + 1, INSET + ARM, bottom);
        fill(INSET, bottom - ARM, INSET + THICK - 1, bottom);
        // 右下
        fill(right - ARM, bottom - THICK + 1, right, bottom);
        fill(right - THICK + 1, bottom - ARM, right, bottom);
    }

    Icon::from_rgba(rgba, SIZE as u32, SIZE as u32).map_err(|err| anyhow!("生成托盘图标失败：{err}"))
}

/// 转义 AppleScript 字符串字面量
fn escape_applescript(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}
