//! 固定到屏幕（M6）
//!
//! 把合成后的截图放进独立的无边框置顶窗口：
//! - 拖拽任意位置移动窗口
//! - 双击或 Esc 关闭
//!
//! 使用 egui 的立即视口，由主窗口每帧驱动。

use eframe::egui::{self, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Vec2};

use crate::capture::CapturedImage;

pub struct PinWindow {
    viewport: egui::ViewportId,
    texture: egui::TextureHandle,
    /// 窗口显示尺寸（pt）
    size: Vec2,
    /// 最近一次已知的窗口位置（pt）
    pos: Option<Pos2>,
    /// 首帧标记：设置初始位置与焦点
    first_frame: bool,
}

impl PinWindow {
    pub fn new(ctx: &egui::Context, image: &CapturedImage, index: u64) -> Self {
        let rgba = image.to_rgba();
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &rgba);
        let texture = ctx.load_texture(format!("pinned_{index}"), color_image, egui::TextureOptions::LINEAR);

        let pixel_size = Vec2::new(image.width as f32, image.height as f32);
        let scale = (900.0 / pixel_size.x).min(700.0 / pixel_size.y).min(1.0);
        let size = (pixel_size * scale).max(Vec2::splat(60.0));

        Self {
            viewport: egui::ViewportId::from_hash_of(format!("screencapture_pin_{index}")),
            texture,
            size,
            pos: initial_position(ctx, size),
            first_frame: true,
        }
    }

    /// 每帧调用；返回 false 表示窗口已关闭
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut open = true;

        let mut builder = egui::ViewportBuilder::default()
            .with_title("ScreenCapture 固定")
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false)
            .with_inner_size([self.size.x, self.size.y]);
        if self.first_frame {
            if let Some(pos) = self.pos {
                builder = builder.with_position([pos.x, pos.y]);
            }
        }

        ctx.show_viewport_immediate(self.viewport, builder, |ctx, _class| {
            if self.first_frame {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.first_frame = false;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                open = false;
            }

            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, Sense::click_and_drag());
                ui.painter().image(
                    self.texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                if response.double_clicked() {
                    open = false;
                }

                if response.dragged() {
                    let current = self
                        .pos
                        .or_else(|| ctx.input(|i| i.viewport().outer_rect).map(|r| r.min))
                        .unwrap_or(Pos2::ZERO);
                    let next = current + response.drag_delta();
                    self.pos = Some(next);
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(next));
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                } else if response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }

                // 操作提示
                let hint = "拖拽移动 · 双击 / Esc 关闭";
                let galley = ui.painter().layout_no_wrap(
                    hint.to_string(),
                    FontId::proportional(11.0),
                    Color32::from_white_alpha(220),
                );
                let pos = Pos2::new(rect.center().x - galley.size().x / 2.0, rect.max.y - galley.size().y - 6.0);
                ui.painter().rect_filled(
                    Rect::from_min_size(pos - Vec2::new(6.0, 3.0), galley.size() + Vec2::new(12.0, 6.0)),
                    4.0,
                    Color32::from_black_alpha(140),
                );
                ui.painter().galley(pos, galley, Color32::WHITE);
            });
        });

        open
    }
}

/// 初始位置：主显示器居中偏左上，避免完全盖住主窗口
fn initial_position(ctx: &egui::Context, size: Vec2) -> Option<Pos2> {
    let monitors = crate::capture::enumerate_monitors();
    let monitor = monitors.iter().find(|m| m.is_primary).or_else(|| monitors.first())?;
    let ppp = ctx.pixels_per_point().max(0.5);
    let width = monitor.rect.width as f32;
    let height = monitor.rect.height as f32;
    let x = monitor.rect.x as f32 + (width - size.x * ppp) * 0.5;
    let y = monitor.rect.y as f32 + (height - size.y * ppp) * 0.5;
    Some(Pos2::new(x / ppp, y / ppp))
}
