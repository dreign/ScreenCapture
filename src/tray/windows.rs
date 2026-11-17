//! Windows 托盘实现
//!
//! eframe/winit 独占主线程消息循环，因此托盘图标与全局热键放在独立后台线程：
//! 该线程创建隐藏窗口并自行泵消息，负责：
//! - `Shell_NotifyIconW` 托盘图标（左键显示主窗口，右键弹出菜单）
//! - `RegisterHotKey` 全局热键，`WM_HOTKEY` 转发为 [`TrayAction::Capture`]
//! - `NIM_MODIFY + NIF_INFO` 气泡通知
//!
//! 线程与 UI 之间通过 `std::sync::mpsc` 通道通信，动作到达后调用
//! `egui::Context::request_repaint` 唤醒界面。

use std::ffi::c_void;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_ERROR, NIIF_INFO, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    GetCursorPos, GetMessageW, GetWindowLongPtrW, LoadCursorW, LoadIconW, PostMessageW, PostQuitMessage,
    RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, TrackPopupMenu, TranslateMessage, GWLP_USERDATA, HMENU,
    IDC_ARROW, IDI_APPLICATION, MENU_ITEM_FLAGS, MF_SEPARATOR, MF_STRING, MSG, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_HOTKEY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};

use eframe::egui;

use super::{CaptureKind, HotkeySpec, TrayAction, HOTKEY_FULLSCREEN, HOTKEY_REGION, HOTKEY_WINDOW};
use crate::hotkey::Combo;

const WM_TRAYICON: u32 = WM_APP + 1;
const WM_WAKE: u32 = WM_APP + 2;

const TRAY_UID: u32 = 1;
const CLASS_NAME: PCWSTR = w!("ScreenCaptureTrayWindow");

const MENU_FULLSCREEN: usize = 1001;
const MENU_REGION: usize = 1002;
const MENU_WINDOW: usize = 1003;
const MENU_SHOW: usize = 1004;
const MENU_QUIT: usize = 1005;

/// 发给托盘线程的命令
#[derive(Debug)]
enum TrayCommand {
    Notify { title: String, body: String, error: bool },
    RegisterHotkeys(Vec<HotkeySpec>),
    Shutdown,
}

/// 托盘服务句柄（Drop 时清理图标、热键并退出线程）
pub struct TrayService {
    cmd_tx: Sender<TrayCommand>,
    action_rx: Receiver<TrayAction>,
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

impl TrayService {
    /// 启动后台托盘线程并添加托盘图标
    pub fn new(ctx: &egui::Context) -> anyhow::Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (action_tx, action_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();
        let ctx = ctx.clone();

        let thread = std::thread::Builder::new()
            .name("screencapture-shell".to_string())
            .spawn(move || run_tray_thread(ctx, action_tx, cmd_rx, ready_tx))?;

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(hwnd)) => Ok(Self { cmd_tx, action_rx, hwnd, thread: Some(thread) }),
            Ok(Err(message)) => {
                let _ = thread.join();
                anyhow::bail!(message)
            }
            Err(err) => anyhow::bail!("托盘线程启动失败：{err}"),
        }
    }

    /// 弹出托盘气泡通知
    pub fn notify(&self, title: &str, body: &str, error: bool) {
        let _ = self.cmd_tx.send(TrayCommand::Notify { title: title.to_string(), body: body.to_string(), error });
        self.wake();
    }

    /// 重新注册全局热键（会先注销旧热键）
    pub fn register_hotkeys(&self, specs: Vec<HotkeySpec>) {
        let _ = self.cmd_tx.send(TrayCommand::RegisterHotkeys(specs));
        self.wake();
    }

    /// 非阻塞读取托盘 / 热键事件
    pub fn try_recv(&self) -> Option<TrayAction> {
        self.action_rx.try_recv().ok()
    }

    fn wake(&self) {
        unsafe {
            let _ = PostMessageW(HWND(self.hwnd as *mut c_void), WM_WAKE, WPARAM(0), LPARAM(0));
        }
    }
}

impl Drop for TrayService {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(TrayCommand::Shutdown);
        self.wake();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// 托盘线程主体
fn run_tray_thread(
    ctx: egui::Context,
    action_tx: Sender<TrayAction>,
    cmd_rx: Receiver<TrayCommand>,
    ready: Sender<Result<isize, String>>,
) {
    unsafe {
        let hinstance = match GetModuleHandleW(PCWSTR::null()) {
            Ok(module) => HINSTANCE(module.0),
            Err(err) => {
                let _ = ready.send(Err(format!("获取模块句柄失败：{err}")));
                return;
            }
        };

        let wc = WNDCLASSW {
            lpfnWndProc: Some(tray_wndproc),
            hInstance: hinstance,
            lpszClassName: CLASS_NAME,
            hCursor: LoadCursorW(HINSTANCE::default(), IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            let _ = ready.send(Err("注册托盘窗口类失败".to_string()));
            return;
        }

        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            w!("ScreenCapture"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(err) => {
                let _ = ready.send(Err(format!("创建托盘窗口失败：{err}")));
                return;
            }
        };

        // 优先使用 build.rs 嵌入的应用图标（资源 id = 1），失败时回退系统默认图标
        // MAKEINTRESOURCEW(1)：把整数 1 直接解释为资源 id
        let icon = LoadIconW(hinstance, PCWSTR(std::ptr::without_provenance::<u16>(1)))
            .or_else(|_| LoadIconW(HINSTANCE::default(), IDI_APPLICATION))
            .unwrap_or_default();
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: icon,
            ..Default::default()
        };
        copy_wide(&mut nid.szTip, "ScreenCapture · 截图工具");

        if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            let _ = ready.send(Err("添加托盘图标失败（Shell_NotifyIconW）".to_string()));
            let _ = DestroyWindow(hwnd);
            return;
        }

        let state = Box::new(TrayState { ctx, action_tx, cmd_rx, nid, hotkey_ids: Vec::new() });
        let state_ptr = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

        let _ = ready.send(Ok(hwnd.0 as isize));

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // 清理：先断开窗口关联，避免 DestroyWindow 期间访问已释放状态
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let state = Box::from_raw(state_ptr);
        let _ = Shell_NotifyIconW(NIM_DELETE, &state.nid);
        for id in &state.hotkey_ids {
            let _ = UnregisterHotKey(hwnd, *id);
        }
        drop(state);
        let _ = DestroyWindow(hwnd);
    }
}

struct TrayState {
    ctx: egui::Context,
    action_tx: Sender<TrayAction>,
    cmd_rx: Receiver<TrayCommand>,
    nid: NOTIFYICONDATAW,
    hotkey_ids: Vec<i32>,
}

unsafe extern "system" fn tray_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    match msg {
        WM_TRAYICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            match event {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                    send_action(&*state_ptr, TrayAction::ShowMainWindow);
                }
                WM_RBUTTONUP => {
                    show_menu(hwnd, &*state_ptr);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            if let Some(kind) = hotkey_kind(wparam.0 as i32) {
                send_action(&*state_ptr, TrayAction::Capture(kind));
            }
            LRESULT(0)
        }
        WM_WAKE => {
            let state = &mut *state_ptr;
            while let Ok(command) = state.cmd_rx.try_recv() {
                handle_command(hwnd, state, command);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn handle_command(hwnd: HWND, state: &mut TrayState, command: TrayCommand) {
    match command {
        TrayCommand::Notify { title, body, error } => unsafe {
            state.nid.uFlags = NIF_INFO;
            copy_wide(&mut state.nid.szInfoTitle, &title);
            copy_wide(&mut state.nid.szInfo, &body);
            state.nid.dwInfoFlags = if error { NIIF_ERROR } else { NIIF_INFO };
            let _ = Shell_NotifyIconW(NIM_MODIFY, &state.nid);
        },
        TrayCommand::RegisterHotkeys(specs) => unsafe {
            for id in state.hotkey_ids.drain(..) {
                let _ = UnregisterHotKey(hwnd, id);
            }

            let mut results = Vec::with_capacity(specs.len());
            let mut registered: Vec<(HOT_KEY_MODIFIERS, u32, String)> = Vec::new();
            for spec in specs {
                let modifiers = hotkey_modifiers(spec.combo);
                let vk = spec.combo.vk();
                // 同一进程内 Windows 允许重复组合，需要自行拦截
                if let Some((_, _, owner)) = registered.iter().find(|(m, v, _)| *m == modifiers && *v == vk) {
                    results.push((spec.name, Err(format!("与「{owner}」热键重复"))));
                    continue;
                }

                match RegisterHotKey(hwnd, spec.id, modifiers, vk) {
                    Ok(()) => {
                        state.hotkey_ids.push(spec.id);
                        registered.push((modifiers, vk, spec.name.clone()));
                        results.push((spec.name, Ok(())));
                    }
                    Err(err) => results.push((spec.name, Err(format_register_error(&err)))),
                }
            }
            send_action(state, TrayAction::HotkeysRegistered(results));
        },
        TrayCommand::Shutdown => unsafe {
            PostQuitMessage(0);
        },
    }
}

fn format_register_error(err: &windows::core::Error) -> String {
    let code = err.code().0 as u32;
    // ERROR_HOTKEY_ALREADY_REGISTERED = 1409
    if code == 1409 {
        "已被其他程序占用".to_string()
    } else {
        err.message()
    }
}

fn hotkey_modifiers(combo: Combo) -> HOT_KEY_MODIFIERS {
    let mut modifiers = HOT_KEY_MODIFIERS(0);
    if combo.ctrl {
        modifiers |= MOD_CONTROL;
    }
    if combo.shift {
        modifiers |= MOD_SHIFT;
    }
    if combo.alt {
        modifiers |= MOD_ALT;
    }
    if combo.win {
        modifiers |= MOD_WIN;
    }
    modifiers
}

fn hotkey_kind(id: i32) -> Option<CaptureKind> {
    match id {
        HOTKEY_REGION => Some(CaptureKind::Region),
        HOTKEY_FULLSCREEN => Some(CaptureKind::Fullscreen),
        HOTKEY_WINDOW => Some(CaptureKind::Window),
        _ => None,
    }
}

fn send_action(state: &TrayState, action: TrayAction) {
    let _ = state.action_tx.send(action);
    state.ctx.request_repaint();
}

/// 弹出右键菜单（返回值即命令 ID）
unsafe fn show_menu(hwnd: HWND, state: &TrayState) {
    let Ok(menu) = CreatePopupMenu() else {
        return;
    };
    let separator = MENU_ITEM_FLAGS(0) | MF_SEPARATOR;
    let _ = AppendMenuW(menu, MF_STRING, MENU_FULLSCREEN, w!("全屏截图"));
    let _ = AppendMenuW(menu, MF_STRING, MENU_REGION, w!("区域截图"));
    let _ = AppendMenuW(menu, MF_STRING, MENU_WINDOW, w!("窗口截图"));
    let _ = AppendMenuW(menu, separator, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, MENU_SHOW, w!("显示主窗口"));
    let _ = AppendMenuW(menu, separator, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, w!("退出"));

    let mut point = POINT::default();
    let _ = GetCursorPos(&mut point);
    let _ = SetForegroundWindow(hwnd);

    let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, None).0 as usize;

    let _ = DestroyMenu(menu);
    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));

    match command {
        MENU_FULLSCREEN => send_action(state, TrayAction::Capture(CaptureKind::Fullscreen)),
        MENU_REGION => send_action(state, TrayAction::Capture(CaptureKind::Region)),
        MENU_WINDOW => send_action(state, TrayAction::Capture(CaptureKind::Window)),
        MENU_SHOW => send_action(state, TrayAction::ShowMainWindow),
        MENU_QUIT => send_action(state, TrayAction::Quit),
        _ => {}
    }
}

/// UTF-8 → 以 0 结尾的 UTF-16，写入固定长度缓冲区
fn copy_wide<const N: usize>(dst: &mut [u16; N], text: &str) {
    let mut index = 0;
    for unit in text.encode_utf16().take(N - 1) {
        dst[index] = unit;
        index += 1;
    }
    dst[index] = 0;
}
