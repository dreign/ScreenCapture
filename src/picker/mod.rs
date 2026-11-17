//! 区域选择覆盖层（M3）
//!
//! 全屏置顶无边框窗口，以整屏截图为背景，提供：
//! - 鼠标拖拽选区 + 选区外压暗 + 尺寸标签 + 角点手柄
//! - 十字辅助线 + 放大镜取色（RGB / 坐标）
//! - 悬停窗口红色高亮
//! - 键盘：方向键移动(±1，Shift ±10)、Ctrl+方向键调整尺寸、Ctrl+A 全选、Enter 确认、Esc 取消

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::capture::{self, CapturedImage, Rect as CaptureRect, WindowInfo};

/// 选区交互结果
pub enum PickerOutcome {
    /// 继续交互
    None,
    /// 用户取消（Esc）
    Cancel,
    /// 用户确认，返回相对整屏截图的像素矩形
    Confirm(CaptureRect),
}

pub struct Picker {
    /// 整屏截图（背景 + 取色数据源）
    image: CapturedImage,
    backdrop: egui::TextureHandle,
    /// 截图时刻的可见窗口（z 序，最上层在前），用于悬停高亮
    windows: Vec<WindowInfo>,
    /// 虚拟屏幕原点（物理像素）
    origin: (i32, i32),
    /// 拖拽起点（覆盖层本地坐标，单位 pt）
    drag_start: Option<Pos2>,
    /// 当前选区（覆盖层本地坐标，单位 pt）
    selection: Option<Rect>,
    /// 悬停窗口（标题 + 本地坐标矩形）
    hover: Option<(String, Rect)>,
    focus_requested: bool,
}

impl Picker {
    pub fn new(ctx: &egui::Context, image: CapturedImage, windows: Vec<WindowInfo>) -> Self {
        let origin = capture::virtual_screen_rect().map(|r| (r.x, r.y)).unwrap_or((0, 0));

        let rgba = image.to_rgba();
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &rgba);
        let backdrop = ctx.load_texture("picker_backdrop", color_image, egui::TextureOptions::LINEAR);

        Self {
            image,
            backdrop,
            windows,
            origin,
            drag_start: None,
            selection: None,
            hover: None,
            focus_requested: false,
        }
    }

    pub fn viewport_id() -> egui::ViewportId {
        egui::ViewportId::from_hash_of("screencapture_region_picker")
    }

    /// 按确认的选区裁剪出截图
    pub fn crop(&self, rect: CaptureRect) -> anyhow::Result<CapturedImage> {
        self.image.crop(rect)
    }

    /// 每帧调用，绘制覆盖层并返回交互结果
    pub fn show(&mut self, ctx: &egui::Context) -> PickerOutcome {
        let mut outcome = PickerOutcome::None;

        // 用主窗口的缩放因子估算覆盖层尺寸（混用多 DPI 时会由覆盖层自行拉伸背景）
        let main_ppp = ctx.pixels_per_point().max(0.5);
        let size_pts = Vec2::new(self.image.width as f32 / main_ppp, self.image.height as f32 / main_ppp);

        let builder = egui::ViewportBuilder::default()
            .with_title("ScreenCapture 选区")
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false)
            .with_position([self.origin.0 as f32, self.origin.1 as f32])
            .with_inner_size([size_pts.x, size_pts.y]);

        ctx.show_viewport_immediate(Self::viewport_id(), builder, |ctx, _class| {
            if !self.focus_requested {
                // 覆盖层需要键盘焦点才能接收 Enter / Esc / 方向键
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.focus_requested = true;
            }
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);

            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                let screen = ui.max_rect();
                let response = ui.allocate_rect(screen, Sense::click_and_drag());
                let painter = ui.painter().clone();
                let scale = ctx.pixels_per_point().max(0.5);

                // 背景：整屏截图
                painter.image(
                    self.backdrop.id(),
                    screen,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                let to_local = |p: Pos2| Pos2::new(p.x - screen.min.x, p.y - screen.min.y);
                let to_screen = |p: Pos2| Pos2::new(screen.min.x + p.x, screen.min.y + p.y);
                let pointer = ctx.input(|i| i.pointer.hover_pos()).map(to_local);

                // ---------------- 鼠标：拖拽选区 ----------------
                if response.drag_started() {
                    if let Some(p) = pointer {
                        self.drag_start = Some(p);
                        self.selection = Some(Rect::from_min_max(p, p));
                    }
                }
                if response.dragged() {
                    if let (Some(start), Some(p)) = (self.drag_start, pointer) {
                        self.selection = Some(Rect::from_two_pos(start, p));
                    }
                }
                if response.drag_stopped() {
                    self.drag_start = None;
                    if let Some(sel) = self.selection {
                        if sel.width() < 4.0 || sel.height() < 4.0 {
                            self.selection = None;
                        }
                    }
                }

                // ---------------- 键盘 ----------------
                let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
                let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                let select_all = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A));
                let (shift, ctrl) = ctx.input(|i| (i.modifiers.shift, i.modifiers.ctrl));
                let (left, right, up, down) = ctx.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowLeft),
                        i.key_pressed(egui::Key::ArrowRight),
                        i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::ArrowDown),
                    )
                });

                if select_all {
                    self.selection = Some(Rect::from_min_size(Pos2::ZERO, screen.size()));
                }

                if let Some(sel) = self.selection.as_mut() {
                    let step = if shift { 10.0 } else { 1.0 };
                    let dx = (right as i32 - left as i32) as f32 * step;
                    let dy = (down as i32 - up as i32) as f32 * step;
                    if dx != 0.0 || dy != 0.0 {
                        let bounds = Rect::from_min_size(Pos2::ZERO, screen.size());
                        let mut r = *sel;
                        if ctrl {
                            // Ctrl+方向键：调整右下角（尺寸）
                            r.max.x = (r.max.x + dx).clamp(r.min.x + 1.0, bounds.max.x);
                            r.max.y = (r.max.y + dy).clamp(r.min.y + 1.0, bounds.max.y);
                        } else {
                            // 方向键：整体平移
                            r = r.translate(Vec2::new(dx, dy));
                            let mut fix = Vec2::ZERO;
                            if r.min.x < bounds.min.x {
                                fix.x = bounds.min.x - r.min.x;
                            }
                            if r.min.y < bounds.min.y {
                                fix.y = bounds.min.y - r.min.y;
                            }
                            if r.max.x > bounds.max.x {
                                fix.x = bounds.max.x - r.max.x;
                            }
                            if r.max.y > bounds.max.y {
                                fix.y = bounds.max.y - r.max.y;
                            }
                            r = r.translate(fix);
                        }
                        *sel = r;
                    }
                }

                if esc {
                    outcome = PickerOutcome::Cancel;
                }
                if enter {
                    if let Some(sel) = self.selection {
                        outcome = self.confirm(sel, screen.size());
                    }
                }
                if response.double_clicked() {
                    if let Some(sel) = self.selection {
                        outcome = self.confirm(sel, screen.size());
                    }
                }

                // ---------------- 悬停窗口高亮 ----------------
                self.hover = None;
                if self.drag_start.is_none() && self.selection.is_none() {
                    if let Some(p) = pointer {
                        let sx = (p.x * scale).round() as i32 + self.origin.0;
                        let sy = (p.y * scale).round() as i32 + self.origin.1;
                        for info in &self.windows {
                            let r = info.rect;
                            if sx >= r.x && sy >= r.y && sx < r.x + r.width as i32 && sy < r.y + r.height as i32 {
                                let min = Pos2::new(
                                    (r.x - self.origin.0) as f32 / scale,
                                    (r.y - self.origin.1) as f32 / scale,
                                );
                                let max = Pos2::new(
                                    (r.x - self.origin.0 + r.width as i32) as f32 / scale,
                                    (r.y - self.origin.1 + r.height as i32) as f32 / scale,
                                );
                                self.hover = Some((info.title.clone(), Rect::from_min_max(min, max)));
                                break;
                            }
                        }
                    }
                }

                // ---------------- 绘制 ----------------
                if let Some(sel) = self.selection {
                    let s = Rect::from_min_max(to_screen(sel.min), to_screen(sel.max));
                    let dim = Color32::from_black_alpha(120);
                    // 四块压暗（选区外）
                    painter.rect_filled(Rect::from_min_max(screen.min, Pos2::new(screen.max.x, s.min.y)), 0.0, dim);
                    painter.rect_filled(Rect::from_min_max(Pos2::new(screen.min.x, s.max.y), screen.max), 0.0, dim);
                    painter.rect_filled(
                        Rect::from_min_max(Pos2::new(screen.min.x, s.min.y), Pos2::new(s.min.x, s.max.y)),
                        0.0,
                        dim,
                    );
                    painter.rect_filled(
                        Rect::from_min_max(Pos2::new(s.max.x, s.min.y), Pos2::new(screen.max.x, s.max.y)),
                        0.0,
                        dim,
                    );
                    // 选区边框 + 角点
                    painter.rect_stroke(s, 0.0, Stroke::new(1.5_f32, Color32::from_rgb(0, 174, 255)));
                    for corner in [s.left_top(), s.right_top(), s.left_bottom(), s.right_bottom()] {
                        painter.rect_filled(Rect::from_center_size(corner, Vec2::splat(6.0)), 1.0, Color32::WHITE);
                    }
                    // 尺寸标签
                    let label = format!("{} × {}", sel.width().round() as i32, sel.height().round() as i32);
                    let label_pos = Pos2::new(s.min.x, (s.min.y - 26.0).max(screen.min.y));
                    draw_pill(&painter, label_pos, &label);
                } else if let Some((title, rect)) = &self.hover {
                    let s = Rect::from_min_max(to_screen(rect.min), to_screen(rect.max));
                    painter.rect_stroke(s, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(255, 72, 72)));
                    let label = format!("{}  {:.0} × {:.0}", title, rect.width(), rect.height());
                    draw_pill(&painter, Pos2::new(s.min.x + 2.0, (s.min.y - 26.0).max(screen.min.y)), &label);
                }

                // 十字辅助线 + 放大镜
                if let Some(p) = pointer {
                    let pp = to_screen(p);
                    let line = Color32::from_white_alpha(70);
                    painter.line_segment(
                        [Pos2::new(screen.min.x, pp.y), Pos2::new(screen.max.x, pp.y)],
                        Stroke::new(1.0_f32, line),
                    );
                    painter.line_segment(
                        [Pos2::new(pp.x, screen.min.y), Pos2::new(pp.x, screen.max.y)],
                        Stroke::new(1.0_f32, line),
                    );
                    self.draw_magnifier(&painter, &screen, p, &to_screen, scale);
                }

                // 顶部操作提示
                let hint =
                    "拖拽选择 · Enter/双击 确认 · Esc 取消 · 方向键微调(Shift×10) · Ctrl+方向键 调尺寸 · Ctrl+A 全选";
                let galley = painter.layout_no_wrap(hint.to_string(), FontId::proportional(13.0), Color32::WHITE);
                let hint_pos = Pos2::new(screen.center().x - galley.size().x / 2.0, screen.min.y + 14.0);
                painter.rect_filled(
                    Rect::from_min_size(hint_pos - Vec2::new(10.0, 5.0), galley.size() + Vec2::new(20.0, 10.0)),
                    6.0,
                    Color32::from_black_alpha(180),
                );
                painter.galley(hint_pos, galley, Color32::WHITE);
            });
        });

        outcome
    }

    /// 将覆盖层本地坐标选区映射为图像像素矩形
    fn confirm(&self, selection: Rect, screen_size: Vec2) -> PickerOutcome {
        PickerOutcome::Confirm(selection_to_image_rect(selection, screen_size, self.image.width, self.image.height))
    }

    /// 放大镜：光标周围 15×15 像素，8pt/像素
    fn draw_magnifier(
        &self,
        painter: &egui::Painter,
        screen: &Rect,
        cursor: Pos2,
        to_screen: &impl Fn(Pos2) -> Pos2,
        scale: f32,
    ) {
        const N: i32 = 15;
        const CELL: f32 = 8.0;

        let box_size = Vec2::splat(N as f32 * CELL);
        let cursor_pt = to_screen(cursor);
        let mut origin = cursor_pt + Vec2::new(22.0, 22.0);
        if origin.x + box_size.x > screen.max.x {
            origin.x = cursor_pt.x - 22.0 - box_size.x;
        }
        if origin.y + box_size.y + 34.0 > screen.max.y {
            origin.y = cursor_pt.y - 22.0 - box_size.y - 34.0;
        }

        // 光标所在的图像像素
        let img_x = (cursor.x * scale).round() as i32;
        let img_y = (cursor.y * scale).round() as i32;

        painter.rect_filled(
            Rect::from_min_size(origin - Vec2::splat(2.0), box_size + Vec2::splat(4.0)),
            3.0,
            Color32::from_black_alpha(220),
        );

        let half = N / 2;
        let mut center_color = Color32::BLACK;
        for j in 0..N {
            for i in 0..N {
                let sx = img_x + i - half;
                let sy = img_y + j - half;
                if sx < 0 || sy < 0 || sx >= self.image.width as i32 || sy >= self.image.height as i32 {
                    continue;
                }
                let idx = ((sy as usize * self.image.width as usize) + sx as usize) * 4;
                let px = &self.image.pixels[idx..idx + 4];
                let color = Color32::from_rgb(px[2], px[1], px[0]);
                if i == half && j == half {
                    center_color = color;
                }
                let cell = Rect::from_min_size(origin + Vec2::new(i as f32 * CELL, j as f32 * CELL), Vec2::splat(CELL));
                painter.rect_filled(cell, 0.0, color);
            }
        }

        // 中心像素描边
        let center_cell =
            Rect::from_min_size(origin + Vec2::new(half as f32 * CELL, half as f32 * CELL), Vec2::splat(CELL));
        painter.rect_stroke(center_cell, 0.0, Stroke::new(1.0_f32, Color32::WHITE));

        // 信息条：屏幕坐标 + HEX + RGB
        let text = format!(
            "({}, {})  #{:02X}{:02X}{:02X}  RGB({}, {}, {})",
            img_x + self.origin.0,
            img_y + self.origin.1,
            center_color.r(),
            center_color.g(),
            center_color.b(),
            center_color.r(),
            center_color.g(),
            center_color.b()
        );
        draw_pill(painter, origin + Vec2::new(0.0, box_size.y + 6.0), &text);
    }
}

/// 画一个深色胶囊背景的文字标签
fn draw_pill(painter: &egui::Painter, pos: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(12.0), Color32::WHITE);
    let bg = Rect::from_min_size(pos - Vec2::new(5.0, 3.0), galley.size() + Vec2::new(10.0, 6.0));
    painter.rect_filled(bg, 4.0, Color32::from_black_alpha(200));
    painter.galley(pos, galley, Color32::WHITE);
}

/// 选区（覆盖层本地 pt）→ 整屏截图内的像素矩形
///
/// 覆盖层窗口与图像可能因 DPI 存在尺寸差异，这里按比例映射并做边界钳制。
fn selection_to_image_rect(selection: Rect, screen_size: Vec2, image_width: u32, image_height: u32) -> CaptureRect {
    let iw = image_width as f32;
    let ih = image_height as f32;
    let sx = iw / screen_size.x.max(1.0);
    let sy = ih / screen_size.y.max(1.0);

    let x = (selection.min.x * sx).round().clamp(0.0, iw - 1.0);
    let y = (selection.min.y * sy).round().clamp(0.0, ih - 1.0);
    let w = (selection.width() * sx).round().clamp(1.0, iw - x);
    let h = (selection.height() * sy).round().clamp(1.0, ih - y);

    CaptureRect::new(x as i32, y as i32, w as u32, h as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_screen() -> Vec2 {
        Vec2::new(1280.0, 800.0)
    }

    #[test]
    fn full_screen_selection_maps_to_whole_image() {
        let sel = Rect::from_min_size(Pos2::ZERO, full_screen());
        let rect = selection_to_image_rect(sel, full_screen(), 2560, 1600);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 2560);
        assert_eq!(rect.height, 1600);
    }

    #[test]
    fn quarter_selection_maps_proportionally() {
        // 覆盖层 1280x800 pt，选区从 (320,200) 起、800x400 pt → 图像 2x 缩放
        let sel = Rect::from_min_size(Pos2::new(320.0, 200.0), Vec2::new(800.0, 400.0));
        let rect = selection_to_image_rect(sel, full_screen(), 2560, 1600);
        assert_eq!(rect.x, 640);
        assert_eq!(rect.y, 400);
        assert_eq!(rect.width, 1600);
        assert_eq!(rect.height, 800);
    }

    #[test]
    fn selection_is_clamped_to_image_bounds() {
        // 选区超出窗口范围（如窗口比图像大）时应被钳制
        let sel = Rect::from_min_size(Pos2::new(1200.0, 750.0), Vec2::new(500.0, 500.0));
        let rect = selection_to_image_rect(sel, full_screen(), 2560, 1600);
        assert!(rect.x >= 0 && rect.y >= 0);
        assert!(rect.x as u32 + rect.width <= 2560);
        assert!(rect.y as u32 + rect.height <= 1600);
        assert!(rect.width >= 1 && rect.height >= 1);
    }

    #[test]
    fn degenerate_selection_still_yields_one_pixel() {
        let sel = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::ZERO);
        let rect = selection_to_image_rect(sel, full_screen(), 2560, 1600);
        assert_eq!(rect.width, 1);
        assert_eq!(rect.height, 1);
    }
}
