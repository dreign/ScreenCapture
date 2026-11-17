//! GDI 底层封装（RAII）
//!
//! 提供屏幕 DC、内存 DC、位图的安全获取与释放，以及像素读取原语。

use std::ffi::c_void;
use std::mem::size_of;

use anyhow::{bail, Context};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC,
    SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, RGBQUAD, SRCCOPY,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};

use super::Rect;

/// 屏幕 DC（GetDC(NULL)），Drop 时自动 ReleaseDC
pub struct ScreenDc(HDC);

impl ScreenDc {
    pub fn new() -> anyhow::Result<Self> {
        let hdc = unsafe { GetDC(HWND::default()) };
        if hdc.0.is_null() {
            bail!("GetDC 失败");
        }
        Ok(Self(hdc))
    }

    pub fn handle(&self) -> HDC {
        self.0
    }
}

impl Drop for ScreenDc {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseDC(HWND::default(), self.0);
        }
    }
}

/// 内存 DC，Drop 时自动 DeleteDC
pub struct MemDc(HDC);

impl MemDc {
    pub fn new_compatible(src: HDC) -> anyhow::Result<Self> {
        let hdc = unsafe { CreateCompatibleDC(src) };
        if hdc.0.is_null() {
            bail!("CreateCompatibleDC 失败");
        }
        Ok(Self(hdc))
    }

    pub fn handle(&self) -> HDC {
        self.0
    }
}

impl Drop for MemDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

/// 兼容位图，Drop 时自动 DeleteObject
pub struct Bitmap(HBITMAP);

impl Bitmap {
    pub fn new_compatible(dc: HDC, width: i32, height: i32) -> anyhow::Result<Self> {
        let hbm = unsafe { CreateCompatibleBitmap(dc, width, height) };
        if hbm.0.is_null() {
            bail!("CreateCompatibleBitmap 失败（{width}x{height}）");
        }
        Ok(Self(hbm))
    }

    pub fn handle(&self) -> HBITMAP {
        self.0
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.0 .0));
        }
    }
}

/// 将位图内容读为 BGRA（top-down，每条像素 4 字节）像素缓冲区
fn read_bitmap_pixels(dc: HDC, bitmap: HBITMAP, width: i32, height: i32) -> anyhow::Result<Vec<u8>> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // 负高度表示 top-down，避免图像上下翻转
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        bmiColors: [RGBQUAD::default()],
    };

    let mut buffer = vec![0u8; width as usize * height as usize * 4];
    let lines = unsafe {
        GetDIBits(dc, bitmap, 0, height as u32, Some(buffer.as_mut_ptr() as *mut c_void), &mut info, DIB_RGB_COLORS)
    };
    if lines == 0 {
        bail!("GetDIBits 失败（返回 0 行）");
    }
    Ok(buffer)
}

/// 抓取虚拟屏幕上的任意矩形区域，返回 BGRA 像素
pub fn grab_rect(rect: Rect) -> anyhow::Result<Vec<u8>> {
    let width = rect.width as i32;
    let height = rect.height as i32;
    if width <= 0 || height <= 0 {
        bail!("无效的截图区域（{}x{}）", rect.width, rect.height);
    }

    let screen = ScreenDc::new()?;
    let mem = MemDc::new_compatible(screen.handle())?;
    let bitmap = Bitmap::new_compatible(screen.handle(), width, height)?;

    let old = unsafe { SelectObject(mem.handle(), HGDIOBJ(bitmap.handle().0)) };
    let blt = unsafe { BitBlt(mem.handle(), 0, 0, width, height, screen.handle(), rect.x, rect.y, SRCCOPY) };
    unsafe {
        SelectObject(mem.handle(), old);
    }
    blt.context("BitBlt 屏幕拷贝失败")?;

    read_bitmap_pixels(screen.handle(), bitmap.handle(), width, height)
}

/// 抓取指定窗口内容，返回 BGRA 像素
///
/// 优先使用 PrintWindow（可捕获被遮挡窗口），失败时回退为屏幕区域拷贝。
pub fn grab_window(hwnd: HWND, rect: Rect) -> anyhow::Result<Vec<u8>> {
    let width = rect.width as i32;
    let height = rect.height as i32;
    if width <= 0 || height <= 0 {
        bail!("无效的窗口区域（{}x{}）", rect.width, rect.height);
    }

    let screen = ScreenDc::new()?;
    let mem = MemDc::new_compatible(screen.handle())?;
    let bitmap = Bitmap::new_compatible(screen.handle(), width, height)?;

    let old = unsafe { SelectObject(mem.handle(), HGDIOBJ(bitmap.handle().0)) };

    // PW_RENDERFULLCONTENT = 0x2，可捕获部分 DirectComposition 渲染内容
    let printed = unsafe { PrintWindow(hwnd, mem.handle(), PRINT_WINDOW_FLAGS(2)) }.as_bool();
    let mut copy_result = Ok(());
    if !printed {
        copy_result = unsafe { BitBlt(mem.handle(), 0, 0, width, height, screen.handle(), rect.x, rect.y, SRCCOPY) };
    }

    unsafe {
        SelectObject(mem.handle(), old);
    }
    copy_result.context("窗口截图失败（PrintWindow 与 BitBlt 均不可用）")?;

    read_bitmap_pixels(screen.handle(), bitmap.handle(), width, height)
}
