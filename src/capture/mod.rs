//! 截图捕获模块（平台无关门面）
//!
//! 共享类型与裁剪逻辑在此定义，具体系统调用分派到：
//! - Windows：GDI / DWM / EnumWindows（`capture::windows`）
//! - macOS：xcap（CoreGraphics，`capture::macos`）

#[cfg(windows)]
pub mod gdi;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(windows)]
use windows as platform;

use std::time::Duration;

/// 矩形区域（虚拟屏幕坐标系）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// 右下角坐标（不含）
    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    /// 两个矩形的并集（用于多显示器虚拟屏幕）
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(x, y, (right - x).max(0) as u32, (bottom - y).max(0) as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureSource {
    FullScreen,
    Region(Rect),
    Window(isize),
}

/// 截图结果
#[derive(Clone)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    /// BGRA（top-down），每像素 4 字节
    pub pixels: Vec<u8>,
    pub source: CaptureSource,
    pub elapsed: Duration,
}

impl CapturedImage {
    fn new(width: u32, height: u32, pixels: Vec<u8>, source: CaptureSource, elapsed: Duration) -> Self {
        Self { width, height, pixels, source, elapsed }
    }

    /// 转换为 RGBA（供 egui / image crate 使用）
    pub fn to_rgba(&self) -> Vec<u8> {
        crate::utils::bgra_to_rgba(&self.pixels)
    }

    /// 转换为 image crate 的 RGBA 图像
    pub fn to_rgba_image(&self) -> anyhow::Result<image::RgbaImage> {
        image::RgbaImage::from_raw(self.width, self.height, self.to_rgba())
            .ok_or_else(|| anyhow::anyhow!("像素缓冲区与图像尺寸不匹配"))
    }

    /// 裁剪出子区域（rect 为相对于本图的像素坐标）
    pub fn crop(&self, rect: Rect) -> anyhow::Result<Self> {
        if rect.width == 0 || rect.height == 0 {
            anyhow::bail!("裁剪区域不能为空");
        }
        if rect.x < 0 || rect.y < 0 {
            anyhow::bail!("裁剪区域起点不能为负");
        }
        let x = rect.x as u32;
        let y = rect.y as u32;
        if x + rect.width > self.width || y + rect.height > self.height {
            anyhow::bail!(
                "裁剪区域超出图像范围（图 {}x{}，区域 {}x{}+{},{}）",
                self.width,
                self.height,
                rect.width,
                rect.height,
                rect.x,
                rect.y
            );
        }

        let row_bytes = rect.width as usize * 4;
        let mut pixels = Vec::with_capacity(row_bytes * rect.height as usize);
        for row in 0..rect.height as usize {
            let start = ((y as usize + row) * self.width as usize + x as usize) * 4;
            pixels.extend_from_slice(&self.pixels[start..start + row_bytes]);
        }

        Ok(Self {
            width: rect.width,
            height: rect.height,
            pixels,
            source: CaptureSource::Region(rect),
            elapsed: self.elapsed,
        })
    }
}

/// 平台窗口句柄
///
/// Windows 存放 `HWND` 指针值，macOS 存放 `CGWindowID`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle(pub isize);

/// 截图服务入口
pub struct Capturer;

impl Capturer {
    /// 全屏截图（覆盖所有显示器）
    pub fn capture_fullscreen() -> anyhow::Result<CapturedImage> {
        platform::capture_fullscreen()
    }

    /// 区域截图（虚拟屏幕坐标）
    pub fn capture_region(rect: Rect) -> anyhow::Result<CapturedImage> {
        if rect.is_empty() {
            anyhow::bail!("截图区域不能为空");
        }
        platform::capture_region(rect)
    }

    /// 窗口截图
    pub fn capture_window(handle: WindowHandle) -> anyhow::Result<CapturedImage> {
        platform::capture_window(handle)
    }

    /// 枚举当前所有可见的顶层窗口（z 序：最上层在前）
    pub fn enumerate_windows() -> anyhow::Result<Vec<WindowInfo>> {
        platform::enumerate_windows()
    }
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub handle: isize,
    pub title: String,
    pub class: String,
    pub rect: Rect,
}

impl WindowInfo {
    pub fn handle(&self) -> WindowHandle {
        WindowHandle(self.handle)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MonitorInfo {
    pub rect: Rect,
    pub is_primary: bool,
}

/// 高 DPI 适配（Windows 启用 Per-Monitor V2；macOS 由系统自动处理）
pub fn init_dpi_awareness() {
    platform::init_dpi_awareness();
}

/// 虚拟屏幕（所有显示器并集）对应的矩形
pub fn virtual_screen_rect() -> anyhow::Result<Rect> {
    platform::virtual_screen_rect()
}

/// 枚举所有显示器
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    platform::enumerate_monitors()
}

/// 获取窗口的真实外框
pub fn window_frame_rect(handle: WindowHandle) -> Option<Rect> {
    platform::window_frame_rect(handle)
}

/// 查找本进程的主窗口（用于截图前临时移出自身）
pub fn find_own_main_window(preferred_title: &str) -> Option<WindowHandle> {
    platform::find_own_main_window(preferred_title)
}

/// 把窗口临时移出屏幕外（截图时排除自身），返回原位置
pub fn park_window_offscreen(handle: WindowHandle) -> Option<(i32, i32)> {
    platform::park_window_offscreen(handle)
}

/// 恢复窗口位置
pub fn restore_window_position(handle: WindowHandle, x: i32, y: i32) {
    platform::restore_window_position(handle, x, y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_empty_check() {
        assert!(Rect::new(0, 0, 0, 10).is_empty());
        assert!(Rect::new(0, 0, 10, 0).is_empty());
        assert!(!Rect::new(0, 0, 10, 10).is_empty());
    }

    #[test]
    fn rect_union_covers_both() {
        let left = Rect::new(-1920, 0, 1920, 1080);
        let right = Rect::new(0, -200, 2560, 1440);
        let union = left.union(right);
        assert_eq!(union, Rect::new(-1920, -200, 4480, 1440));
    }

    #[test]
    fn crop_extracts_requested_region() {
        // 构造 4x4 图像，像素 BGRA = (x, y, 0, 255)
        let mut pixels = Vec::new();
        for y in 0..4u8 {
            for x in 0..4u8 {
                pixels.extend_from_slice(&[x, y, 0, 255]);
            }
        }
        let image = CapturedImage::new(4, 4, pixels, CaptureSource::FullScreen, Duration::ZERO);
        let cropped = image.crop(Rect::new(1, 2, 2, 2)).expect("裁剪应成功");

        assert_eq!(cropped.width, 2);
        assert_eq!(cropped.height, 2);
        assert_eq!(cropped.pixels.len(), 16);
        // 首行：源图 (1,2) (2,2)
        assert_eq!(&cropped.pixels[0..4], &[1, 2, 0, 255]);
        assert_eq!(&cropped.pixels[4..8], &[2, 2, 0, 255]);
        // 次行：源图 (1,3) (2,3)
        assert_eq!(&cropped.pixels[8..12], &[1, 3, 0, 255]);
        assert_eq!(&cropped.pixels[12..16], &[2, 3, 0, 255]);
        assert_eq!(cropped.source, CaptureSource::Region(Rect::new(1, 2, 2, 2)));
    }

    #[test]
    fn crop_rejects_invalid_region() {
        let image = CapturedImage::new(2, 2, vec![0; 16], CaptureSource::FullScreen, Duration::ZERO);
        assert!(image.crop(Rect::new(1, 1, 2, 2)).is_err(), "越界应报错");
        assert!(image.crop(Rect::new(0, 0, 0, 1)).is_err(), "空区域应报错");
        assert!(image.crop(Rect::new(-1, 0, 1, 1)).is_err(), "负坐标应报错");
        assert!(image.crop(Rect::new(0, 0, 2, 2)).is_ok(), "整图应可裁剪");
    }

    #[test]
    fn crop_accepts_exact_edges() {
        let image = CapturedImage::new(4, 3, vec![7; 4 * 3 * 4], CaptureSource::FullScreen, Duration::ZERO);

        let edge = image.crop(Rect::new(2, 1, 2, 2)).expect("右下贴边裁剪应成功");
        assert_eq!((edge.width, edge.height), (2, 2));
        assert_eq!(edge.pixels, vec![7; 2 * 2 * 4]);

        let corner = image.crop(Rect::new(3, 2, 1, 1)).expect("单像素角应可裁剪");
        assert_eq!(corner.pixels, vec![7; 4]);
    }
}
