//! Windows 平台截图实现（GDI / DWM / EnumWindows）

use std::ffi::c_void;
use std::time::Instant;

use anyhow::bail;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetSystemMetrics, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetWindowPos, GWL_EXSTYLE, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    WS_EX_TOOLWINDOW,
};

use super::{gdi, CaptureSource, CapturedImage, MonitorInfo, Rect, WindowHandle, WindowInfo};

fn hwnd_of(handle: WindowHandle) -> HWND {
    HWND(handle.0 as *mut c_void)
}

fn rect_from_win32(r: RECT) -> Option<Rect> {
    let w = r.right - r.left;
    let h = r.bottom - r.top;
    if w <= 0 || h <= 0 {
        None
    } else {
        Some(Rect { x: r.left, y: r.top, width: w as u32, height: h as u32 })
    }
}

pub fn init_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub fn capture_fullscreen() -> anyhow::Result<CapturedImage> {
    let rect = virtual_screen_rect()?;
    let start = Instant::now();
    let pixels = gdi::grab_rect(rect)?;
    Ok(CapturedImage::new(rect.width, rect.height, pixels, CaptureSource::FullScreen, start.elapsed()))
}

pub fn capture_region(rect: Rect) -> anyhow::Result<CapturedImage> {
    if rect.is_empty() {
        bail!("截图区域不能为空");
    }
    let start = Instant::now();
    let pixels = gdi::grab_rect(rect)?;
    Ok(CapturedImage::new(rect.width, rect.height, pixels, CaptureSource::Region(rect), start.elapsed()))
}

pub fn capture_window(handle: WindowHandle) -> anyhow::Result<CapturedImage> {
    let hwnd = hwnd_of(handle);
    let rect = window_frame_rect(handle).ok_or_else(|| anyhow::anyhow!("无法获取窗口尺寸（窗口可能已关闭）"))?;
    let start = Instant::now();
    let pixels = gdi::grab_window(hwnd, rect)?;
    Ok(CapturedImage::new(rect.width, rect.height, pixels, CaptureSource::Window(handle.0), start.elapsed()))
}

/// 枚举当前所有可见的顶层窗口（EnumWindows 顺序，即 z 序：最上层在前）
pub fn enumerate_windows() -> anyhow::Result<Vec<WindowInfo>> {
    let mut list: Vec<WindowInfo> = Vec::new();
    let ptr = &mut list as *mut Vec<WindowInfo> as isize;
    unsafe {
        let _ = EnumWindows(Some(window_enum_proc), LPARAM(ptr));
    }
    Ok(list)
}

pub fn virtual_screen_rect() -> anyhow::Result<Rect> {
    let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    if w <= 0 || h <= 0 {
        bail!("无法获取虚拟屏幕尺寸");
    }
    Ok(Rect::new(x, y, w as u32, h as u32))
}

pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    let mut list: Vec<MonitorInfo> = Vec::new();
    let ptr = &mut list as *mut Vec<MonitorInfo> as isize;
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(monitor_enum_proc), LPARAM(ptr));
    }
    list
}

/// 获取窗口的真实外框（优先 DWM 扩展边框，回退 GetWindowRect）
pub fn window_frame_rect(handle: WindowHandle) -> Option<Rect> {
    let hwnd = hwnd_of(handle);
    let mut rect = RECT::default();
    let dwm = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut c_void,
            std::mem::size_of::<RECT>() as u32,
        )
    };
    if dwm.is_ok() {
        if let Some(r) = rect_from_win32(rect) {
            return Some(r);
        }
    }

    let mut fallback = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut fallback) }.is_ok() {
        rect_from_win32(fallback)
    } else {
        None
    }
}

/// 查找本进程的主窗口（用于截图前临时移出自身）
///
/// 本进程还存在 glutin 的 OpenGL 辅助窗口（如 `__wglDummyWindowFodder`），
/// 因此优先匹配标题，其次退回面积最大的可见窗口。
pub fn find_own_main_window(preferred_title: &str) -> Option<WindowHandle> {
    struct Search {
        preferred: String,
        preferred_hwnd: isize,
        fallback_hwnd: isize,
        fallback_area: i64,
    }

    unsafe extern "system" fn proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut Search);

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != GetCurrentProcessId() || !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }

        let mut buf = [0u16; 128];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len <= 0 {
            return BOOL(1);
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return BOOL(1);
        }
        let area = (rect.right - rect.left) as i64 * (rect.bottom - rect.top) as i64;
        // 过滤辅助小窗（IME、GL dummy 等）
        if area < 400 * 300 {
            return BOOL(1);
        }

        let title = String::from_utf16_lossy(&buf[..len as usize]);
        if title == search.preferred {
            search.preferred_hwnd = hwnd.0 as isize;
            return BOOL(0);
        }
        if area > search.fallback_area {
            search.fallback_area = area;
            search.fallback_hwnd = hwnd.0 as isize;
        }
        BOOL(1)
    }

    let mut search =
        Search { preferred: preferred_title.to_string(), preferred_hwnd: 0, fallback_hwnd: 0, fallback_area: 0 };
    unsafe {
        let _ = EnumWindows(Some(proc), LPARAM(&mut search as *mut Search as isize));
    }

    let found = if search.preferred_hwnd != 0 { search.preferred_hwnd } else { search.fallback_hwnd };
    if found == 0 {
        None
    } else {
        Some(WindowHandle(found))
    }
}

/// 把窗口临时移出屏幕外（截图时排除自身），返回原位置
pub fn park_window_offscreen(handle: WindowHandle) -> Option<(i32, i32)> {
    let hwnd = hwnd_of(handle);
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
    unsafe {
        // 移出虚拟屏幕范围，同时不改 z 序、不改尺寸、不激活
        let _ = SetWindowPos(hwnd, HWND::default(), -32000, -32000, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
    Some((rect.left, rect.top))
}

/// 恢复窗口位置
pub fn restore_window_position(handle: WindowHandle, x: i32, y: i32) {
    let hwnd = hwnd_of(handle);
    unsafe {
        let _ = SetWindowPos(hwnd, HWND::default(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

unsafe extern "system" fn monitor_enum_proc(hmonitor: HMONITOR, _hdc: HDC, _rect: *mut RECT, lparam: LPARAM) -> BOOL {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
        if let Some(rect) = rect_from_win32(info.rcMonitor) {
            let list = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
            list.push(MonitorInfo {
                rect,
                // MONITORINFOF_PRIMARY = 0x1
                is_primary: (info.dwFlags & 0x1) != 0,
            });
        }
    }
    BOOL(1)
}

unsafe extern "system" fn window_enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return BOOL(1);
    }

    // 跳过工具窗口（托盘图标、浮动面板等）
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
        return BOOL(1);
    }

    let mut title_buf = [0u16; 256];
    let title_len = GetWindowTextW(hwnd, &mut title_buf);
    if title_len <= 0 {
        return BOOL(1);
    }
    let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

    let mut class_buf = [0u16; 256];
    let class_len = GetClassNameW(hwnd, &mut class_buf);
    let class = String::from_utf16_lossy(&class_buf[..class_len.max(0) as usize]);
    // 过滤桌面、任务栏等系统外壳窗口
    if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Windows.UI.Core.CoreWindow") {
        return BOOL(1);
    }

    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() {
        return BOOL(1);
    }
    let Some(rect) = rect_from_win32(rect) else {
        return BOOL(1);
    };

    let list = &mut *(lparam.0 as *mut Vec<WindowInfo>);
    list.push(WindowInfo { handle: hwnd.0 as isize, title, class, rect });
    BOOL(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{ImageFormat, OutputService, SaveOptions};

    #[test]
    fn rect_from_win32_converts_size() {
        let raw = RECT { left: 5, top: 6, right: 25, bottom: 36 };
        let rect = rect_from_win32(raw).expect("应为有效矩形");
        assert_eq!(rect.x, 5);
        assert_eq!(rect.y, 6);
        assert_eq!(rect.width, 20);
        assert_eq!(rect.height, 30);
    }

    #[test]
    fn rect_from_win32_rejects_degenerate() {
        let raw = RECT { left: 10, top: 10, right: 10, bottom: 30 };
        assert!(rect_from_win32(raw).is_none());
    }

    #[test]
    fn virtual_screen_is_valid() {
        let rect = virtual_screen_rect().expect("应能获取虚拟屏幕");
        assert!(rect.width >= 640 && rect.height >= 480);
    }

    #[test]
    fn capture_small_region_returns_bgra_buffer() {
        let image = capture_region(Rect::new(0, 0, 32, 32)).expect("区域截图应成功");
        assert_eq!(image.width, 32);
        assert_eq!(image.height, 32);
        assert_eq!(image.pixels.len(), 32 * 32 * 4);
        assert_eq!(image.to_rgba().len(), 32 * 32 * 4);
    }

    #[test]
    fn capture_fullscreen_matches_virtual_screen_and_saves_png() {
        let expected = virtual_screen_rect().expect("应能获取虚拟屏幕");
        let image = capture_fullscreen().expect("全屏截图应成功");
        assert_eq!(image.width, expected.width);
        assert_eq!(image.height, expected.height);
        assert_eq!(image.pixels.len(), (image.width * image.height * 4) as usize);

        let path = std::env::temp_dir().join("screencapture_fullscreen_test.png");
        let saved = OutputService::save_image(
            &image.pixels,
            image.width,
            image.height,
            &SaveOptions { path, format: ImageFormat::Png, quality: 90 },
        )
        .expect("全屏截图保存 PNG 应成功");
        let size = std::fs::metadata(&saved).expect("文件应存在").len();
        assert!(size > 1024, "PNG 文件应有实际内容，实际 {size} 字节");
        let _ = std::fs::remove_file(saved);
    }

    #[test]
    fn enumerate_windows_and_capture_one() {
        let windows = enumerate_windows().expect("枚举窗口应成功");
        assert!(!windows.is_empty(), "至少应枚举到一个可见窗口");

        let info = &windows[0];
        assert!(!info.title.is_empty());
        assert!(info.rect.width > 0 && info.rect.height > 0);

        // 某些窗口（如受保护内容）可能截取失败，此处只校验成功路径的缓冲区尺寸
        if let Ok(image) = capture_window(info.handle()) {
            assert_eq!(image.pixels.len(), (image.width * image.height * 4) as usize);
        }
    }
}
