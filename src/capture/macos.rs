//! macOS 平台截图实现（xcap / CoreGraphics）
//!
//! 首次运行时系统会请求「屏幕录制」权限；未授权时捕获返回空画面或失败，
//! 需要在「系统设置 → 隐私与安全性 → 屏幕录制」中勾选本应用。

use std::time::Instant;

use anyhow::anyhow;
use xcap::{Monitor, Window};

use super::{CaptureSource, CapturedImage, MonitorInfo, Rect, WindowHandle, WindowInfo};

pub fn init_dpi_awareness() {
    // macOS 的坐标由系统按逻辑点管理，无需显式声明 DPI 感知
}

pub fn capture_fullscreen() -> anyhow::Result<CapturedImage> {
    let start = Instant::now();
    let monitors = Monitor::all().map_err(|err| anyhow!("枚举显示器失败：{err}"))?;
    if monitors.is_empty() {
        return Err(anyhow!("未检测到显示器"));
    }

    let bounds = union_monitors(&monitors)?;
    let (cw, ch) = (bounds.width as usize, bounds.height as usize);
    let mut canvas = vec![0u8; cw * ch * 4];

    for monitor in &monitors {
        let rect = monitor_rect(monitor)?;
        let image = monitor
            .capture_image()
            .map_err(|err| anyhow!("屏幕捕获失败（请在系统设置中授予「屏幕录制」权限）：{err}"))?;
        let (sw, sh) = (image.width() as usize, image.height() as usize);
        blit_rgba(
            &mut canvas,
            cw,
            ch,
            (rect.x - bounds.x) as isize,
            (rect.y - bounds.y) as isize,
            sw,
            sh,
            image.as_raw(),
        );
    }

    Ok(CapturedImage::new(bounds.width, bounds.height, canvas, CaptureSource::FullScreen, start.elapsed()))
}

pub fn capture_region(rect: Rect) -> anyhow::Result<CapturedImage> {
    let bounds = virtual_screen_rect()?;
    let full = capture_fullscreen()?;
    let local = Rect::new(rect.x - bounds.x, rect.y - bounds.y, rect.width, rect.height);
    let mut image = full.crop(local)?;
    image.source = CaptureSource::Region(rect);
    Ok(image)
}

pub fn capture_window(handle: WindowHandle) -> anyhow::Result<CapturedImage> {
    let target = find_window(handle)?;
    let start = Instant::now();
    let image =
        target.capture_image().map_err(|err| anyhow!("窗口捕获失败（请在系统设置中授予「屏幕录制」权限）：{err}"))?;
    let (width, height) = (image.width(), image.height());
    Ok(CapturedImage::new(
        width,
        height,
        crate::utils::rgba_to_bgra(image.as_raw()),
        CaptureSource::Window(handle.0),
        start.elapsed(),
    ))
}

pub fn enumerate_windows() -> anyhow::Result<Vec<WindowInfo>> {
    let list = Window::all().map_err(|err| anyhow!("枚举窗口失败：{err}"))?;
    let mut windows = Vec::new();
    for window in list {
        if window.is_minimized().unwrap_or(false) {
            continue;
        }
        let title = window.title().unwrap_or_default();
        if title.trim().is_empty() {
            continue;
        }
        let Ok(handle) = window.id() else {
            continue;
        };
        let Ok(rect) = window_rect(&window) else {
            continue;
        };
        if rect.is_empty() {
            continue;
        }
        windows.push(WindowInfo { handle: handle as isize, title, class: window.app_name().unwrap_or_default(), rect });
    }
    Ok(windows)
}

pub fn virtual_screen_rect() -> anyhow::Result<Rect> {
    let monitors = Monitor::all().map_err(|err| anyhow!("枚举显示器失败：{err}"))?;
    union_monitors(&monitors)
}

pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    let Ok(monitors) = Monitor::all() else {
        return Vec::new();
    };
    monitors
        .iter()
        .filter_map(|monitor| {
            let rect = monitor_rect(monitor).ok()?;
            Some(MonitorInfo { rect, is_primary: monitor.is_primary().unwrap_or(false) })
        })
        .collect()
}

pub fn window_frame_rect(handle: WindowHandle) -> Option<Rect> {
    let window = find_window(handle).ok()?;
    let rect = window_rect(&window).ok()?;
    if rect.is_empty() {
        None
    } else {
        Some(rect)
    }
}

/// macOS 下没有便捷方式在截图前隐藏自身窗口，暂不做移屏处理
pub fn find_own_main_window(_preferred_title: &str) -> Option<WindowHandle> {
    None
}

pub fn park_window_offscreen(_handle: WindowHandle) -> Option<(i32, i32)> {
    None
}

pub fn restore_window_position(_handle: WindowHandle, _x: i32, _y: i32) {}

fn union_monitors(monitors: &[Monitor]) -> anyhow::Result<Rect> {
    let mut iter = monitors.iter();
    let first = iter.next().ok_or_else(|| anyhow!("未检测到显示器"))?;
    let mut bounds = monitor_rect(first)?;
    for monitor in iter {
        bounds = bounds.union(monitor_rect(monitor)?);
    }
    Ok(bounds)
}

fn monitor_rect(monitor: &Monitor) -> anyhow::Result<Rect> {
    let x = monitor.x().map_err(|err| anyhow!("读取显示器坐标失败：{err}"))?;
    let y = monitor.y().map_err(|err| anyhow!("读取显示器坐标失败：{err}"))?;
    let width = monitor.width().map_err(|err| anyhow!("读取显示器尺寸失败：{err}"))?;
    let height = monitor.height().map_err(|err| anyhow!("读取显示器尺寸失败：{err}"))?;
    Ok(Rect::new(x, y, width, height))
}

fn window_rect(window: &Window) -> anyhow::Result<Rect> {
    let x = window.x().map_err(|err| anyhow!("读取窗口坐标失败：{err}"))?;
    let y = window.y().map_err(|err| anyhow!("读取窗口坐标失败：{err}"))?;
    let width = window.width().map_err(|err| anyhow!("读取窗口尺寸失败：{err}"))?;
    let height = window.height().map_err(|err| anyhow!("读取窗口尺寸失败：{err}"))?;
    Ok(Rect::new(x, y, width, height))
}

fn find_window(handle: WindowHandle) -> anyhow::Result<Window> {
    let list = Window::all().map_err(|err| anyhow!("枚举窗口失败：{err}"))?;
    list.into_iter()
        .find(|window| window.id().map(|id| id as isize).ok() == Some(handle.0))
        .ok_or_else(|| anyhow!("无法找到窗口（窗口可能已关闭）"))
}

/// 把 RGBA 源图叠加到 BGRA 画布的指定位置（自动裁剪越界部分）
#[allow(clippy::too_many_arguments)]
fn blit_rgba(
    canvas: &mut [u8],
    canvas_width: usize,
    canvas_height: usize,
    offset_x: isize,
    offset_y: isize,
    src_width: usize,
    src_height: usize,
    src: &[u8],
) {
    if src_width == 0 || src_height == 0 || canvas_width == 0 || canvas_height == 0 {
        return;
    }

    // 负偏移时跳过源图左侧 / 上侧
    let skip_x = (-offset_x).max(0) as usize;
    let skip_y = (-offset_y).max(0) as usize;
    let dst_x = offset_x.max(0) as usize;
    let dst_y = offset_y.max(0) as usize;

    if skip_x >= src_width || skip_y >= src_height || dst_x >= canvas_width || dst_y >= canvas_height {
        return;
    }

    let cols = (src_width - skip_x).min(canvas_width - dst_x);
    for row in 0..(src_height - skip_y).min(canvas_height - dst_y) {
        let src_row = (skip_y + row) * src_width * 4;
        let dst_row = (dst_y + row) * canvas_width * 4;
        for col in 0..cols {
            let s = src_row + (skip_x + col) * 4;
            let d = dst_row + (dst_x + col) * 4;
            canvas[d] = src[s + 2];
            canvas[d + 1] = src[s + 1];
            canvas[d + 2] = src[s];
            canvas[d + 3] = src[s + 3];
        }
    }
}
