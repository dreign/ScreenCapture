//! 编辑器画布
//!
//! 负责底图显示（自动适配 + 滚轮缩放 + 中键/选择工具拖拽平移）、
//! 各工具的鼠标交互，以及标注的即时预览。

use std::collections::HashMap;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

use super::{Annotation, AnnotationData, EditorState, ToolType};
use crate::capture::CapturedImage;

/// 画布返回给上层的信息
#[derive(Default)]
pub struct CanvasFeedback {
    /// 状态栏提示（如取色结果）
    pub message: Option<String>,
}

pub struct EditorCanvas {
    texture: egui::TextureHandle,
    /// 用户缩放倍数（相对适配尺寸）
    zoom: f32,
    /// 相对居中的平移量（屏幕 pt）
    offset: Vec2,
    drag: Option<Drag>,
    text_draft: Option<TextDraft>,
    /// 已提交马赛克标注的像素化纹理
    mosaic_textures: HashMap<u64, egui::TextureHandle>,
    /// 当前光标所在的底图像素坐标
    cursor: Option<Pos2>,
    /// 本帧的显示缩放（屏幕 pt / 底图像素）
    scale: f32,
}

enum Drag {
    Brush { points: Vec<Pos2> },
    Shape { start: Pos2, current: Pos2 },
}

struct TextDraft {
    pos: Pos2,
    text: String,
    request_focus: bool,
}

impl EditorCanvas {
    pub fn new(ctx: &egui::Context, image: &CapturedImage) -> Self {
        let rgba = image.to_rgba();
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &rgba);
        let texture = ctx.load_texture("editor_base", color_image, egui::TextureOptions::LINEAR);

        Self {
            texture,
            zoom: 1.0,
            offset: Vec2::ZERO,
            drag: None,
            text_draft: None,
            mosaic_textures: HashMap::new(),
            cursor: None,
            scale: 1.0,
        }
    }

    /// 每帧绘制画布并处理交互
    pub fn show(&mut self, ui: &mut egui::Ui, state: &mut EditorState, image: &CapturedImage) -> CanvasFeedback {
        let mut feedback = CanvasFeedback::default();

        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, Sense::click_and_drag());
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, Color32::from_gray(26));

        let (iw, ih) = (image.width as f32, image.height as f32);
        let fit = (area.width() / iw).min(area.height() / ih).max(0.0001);

        // ---- 滚轮缩放（以光标为锚点） ----
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.01 {
                let old_zoom = self.zoom;
                self.zoom = (self.zoom * (scroll * 0.0015).exp()).clamp(0.05, 16.0);
                if let Some(cursor) = response.hover_pos() {
                    let old_scale = fit * old_zoom;
                    let new_scale = fit * self.zoom;
                    let old_min = area.center() + self.offset - Vec2::new(iw, ih) * old_scale * 0.5;
                    let anchor = (cursor - old_min) / old_scale;
                    let new_min = cursor - anchor * new_scale;
                    self.offset = new_min + Vec2::new(iw, ih) * new_scale * 0.5 - area.center();
                }
            }
        }

        // ---- 中键拖拽平移 ----
        if ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle)) {
            self.offset += ui.input(|i| i.pointer.delta());
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        let scale = fit * self.zoom;
        self.scale = scale;
        let image_size = Vec2::new(iw, ih) * scale;
        let image_rect = Rect::from_center_size(area.center() + self.offset, image_size);

        // ---- 光标位置（底图像素坐标） ----
        let pointer_img = response.hover_pos().map(|p| screen_to_image(p, image_rect, scale));
        self.cursor = pointer_img;

        // ---- 底图 ----
        painter.rect_filled(image_rect, 0.0, Color32::from_gray(60));
        painter.image(
            self.texture.id(),
            image_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        // ---- 交互 ----
        if let Some(message) = self.handle_interaction(ui.ctx(), &response, state, image, pointer_img) {
            feedback.message = Some(message);
        }

        // ---- 已提交标注 ----
        let clip = image_rect.intersect(area);
        let ann_painter = ui.painter_at(clip);
        for ann in &state.annotations {
            self.draw_annotation(&ann_painter, ann, image, image_rect, scale);
        }

        // ---- 进行中的预览 ----
        self.draw_preview(&ann_painter, state, image, pointer_img, image_rect, scale);

        // ---- 图片边框 ----
        painter.rect_stroke(image_rect, 0.0, Stroke::new(1.0_f32, Color32::from_gray(90)));

        // ---- 光标样式 ----
        if response.hovered() {
            let icon = match state.current_tool {
                ToolType::Select => egui::CursorIcon::Grab,
                ToolType::Text => egui::CursorIcon::Text,
                ToolType::Eyedropper => egui::CursorIcon::Crosshair,
                _ => egui::CursorIcon::Crosshair,
            };
            ui.ctx().set_cursor_icon(icon);
        }

        // ---- 左下角信息 ----
        let cursor_text = match self.cursor {
            Some(p) => format!("({}, {})", p.x.round() as i32, p.y.round() as i32),
            None => "(-, -)".to_string(),
        };
        let info =
            format!("{}x{}  ·  缩放 {:.0}%  ·  光标 {}", image.width, image.height, self.zoom * 100.0, cursor_text);
        draw_pill(&painter, area.left_bottom() + Vec2::new(10.0, -30.0), &info);

        let hint = "滚轮缩放 · 中键/选择工具 拖拽平移 · Ctrl+Z 撤销";
        let galley = painter.layout_no_wrap(hint.to_string(), FontId::proportional(12.0), Color32::from_gray(190));
        let pos = Pos2::new(area.max.x - galley.size().x - 10.0, area.max.y - 24.0);
        painter.galley(pos, galley, Color32::from_gray(190));

        // ---- 文字输入弹窗 ----
        self.show_text_draft(ui.ctx(), state);

        feedback
    }

    fn handle_interaction(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        state: &mut EditorState,
        image: &CapturedImage,
        pointer: Option<Pos2>,
    ) -> Option<String> {
        let tool = state.current_tool;
        let mut message = None;

        match tool {
            ToolType::Select => {
                if response.dragged() {
                    self.offset += response.drag_delta();
                }
            }
            ToolType::Pen => {
                if response.drag_started() {
                    if let Some(p) = pointer {
                        self.drag = Some(Drag::Brush { points: vec![p] });
                    }
                }
                if response.dragged() {
                    if let (Some(Drag::Brush { points }), Some(p)) = (self.drag.as_mut(), pointer) {
                        if points.last().is_none_or(|q| q.distance(p) >= 1.0) {
                            points.push(p);
                        }
                    }
                }
                if response.drag_stopped() {
                    self.finish_drag(ctx, state, image);
                }
            }
            ToolType::Rectangle | ToolType::Ellipse | ToolType::Arrow | ToolType::Mosaic => {
                if response.drag_started() {
                    if let Some(p) = pointer {
                        self.drag = Some(Drag::Shape { start: p, current: p });
                    }
                }
                if response.dragged() {
                    if let (Some(Drag::Shape { current, .. }), Some(p)) = (self.drag.as_mut(), pointer) {
                        *current = p;
                    }
                }
                if response.drag_stopped() {
                    self.finish_drag(ctx, state, image);
                }
            }
            ToolType::Text => {
                if response.clicked() {
                    if let Some(p) = pointer {
                        self.text_draft = Some(TextDraft { pos: p, text: String::new(), request_focus: true });
                    }
                }
            }
            ToolType::Number => {
                if response.clicked() {
                    if let Some(p) = pointer {
                        let value = state.next_number();
                        let radius = (state.font_size * 0.8).max(12.0);
                        state.commit(AnnotationData::Number { x: p.x, y: p.y, value, radius });
                    }
                }
            }
            ToolType::Eyedropper => {
                if response.clicked() {
                    if let Some(p) = pointer {
                        let color = sample_pixel(image, p.x as i32, p.y as i32);
                        state.color = color;
                        message = Some(format!(
                            "已拾取颜色 #{:02X}{:02X}{:02X} RGB({}, {}, {})",
                            color.r(),
                            color.g(),
                            color.b(),
                            color.r(),
                            color.g(),
                            color.b()
                        ));
                    }
                }
            }
        }

        message
    }

    /// 提交进行中的拖拽（画笔 / 矩形 / 椭圆 / 箭头 / 马赛克）
    fn finish_drag(&mut self, ctx: &egui::Context, state: &mut EditorState, image: &CapturedImage) {
        let Some(drag) = self.drag.take() else {
            return;
        };

        match drag {
            Drag::Brush { points } => {
                if points.len() >= 2 {
                    state.commit(AnnotationData::Brush { points: points.iter().map(|p| (p.x, p.y)).collect() });
                }
            }
            Drag::Shape { start, current } => {
                let (dx, dy) = (current.x - start.x, current.y - start.y);
                if dx.abs() < 2.0 && dy.abs() < 2.0 {
                    return;
                }
                let (x, y) = (start.x.min(current.x), start.y.min(current.y));
                let (w, h) = (dx.abs(), dy.abs());

                let data = match state.current_tool {
                    ToolType::Rectangle => AnnotationData::Rect { x, y, w, h, filled: state.filled },
                    ToolType::Ellipse => AnnotationData::Ellipse { x, y, w, h, filled: state.filled },
                    ToolType::Arrow => AnnotationData::Arrow { start: (start.x, start.y), end: (current.x, current.y) },
                    ToolType::Mosaic => AnnotationData::Mosaic { x, y, w, h, block: state.mosaic_block },
                    _ => return,
                };

                let is_mosaic = matches!(data, AnnotationData::Mosaic { .. });
                let id = state.commit(data);
                if is_mosaic {
                    self.build_mosaic_texture(ctx, image, id, x, y, w, h, state.mosaic_block);
                }
            }
        }
    }

    /// 为已提交的马赛克标注生成像素化纹理（每块取均值，NEAREST 采样保持块状）
    #[allow(clippy::too_many_arguments)]
    fn build_mosaic_texture(
        &mut self,
        ctx: &egui::Context,
        image: &CapturedImage,
        id: u64,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        block: u32,
    ) {
        let block = block.max(2) as i32;
        let x0 = (x.floor() as i32).max(0);
        let y0 = (y.floor() as i32).max(0);
        let x1 = ((x + w).ceil() as i32).min(image.width as i32);
        let y1 = ((y + h).ceil() as i32).min(image.height as i32);
        if x1 <= x0 || y1 <= y0 {
            return;
        }

        let gw = ((x1 - x0 + block - 1) / block) as usize;
        let gh = ((y1 - y0 + block - 1) / block) as usize;
        let mut pixels = vec![Color32::TRANSPARENT; gw * gh];

        for gy in 0..gh {
            for gx in 0..gw {
                let bx0 = x0 + gx as i32 * block;
                let by0 = y0 + gy as i32 * block;
                let bx1 = (bx0 + block).min(x1);
                let by1 = (by0 + block).min(y1);
                let (mut sb, mut sg, mut sr, mut n) = (0u32, 0u32, 0u32, 0u32);
                for py in by0..by1 {
                    for px in bx0..bx1 {
                        let idx = ((py as u32 * image.width + px as u32) * 4) as usize;
                        sb += image.pixels[idx] as u32;
                        sg += image.pixels[idx + 1] as u32;
                        sr += image.pixels[idx + 2] as u32;
                        n += 1;
                    }
                }
                let n = n.max(1);
                pixels[gy * gw + gx] = Color32::from_rgb((sr / n) as u8, (sg / n) as u8, (sb / n) as u8);
            }
        }

        let color_image = egui::ColorImage { size: [gw, gh], pixels };
        let texture = ctx.load_texture(format!("mosaic_{id}"), color_image, egui::TextureOptions::NEAREST);
        self.mosaic_textures.insert(id, texture);
    }

    fn draw_annotation(
        &self,
        painter: &egui::Painter,
        ann: &Annotation,
        image: &CapturedImage,
        image_rect: Rect,
        scale: f32,
    ) {
        draw_annotation(painter, ann, image, image_rect, scale, &self.mosaic_textures);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_preview(
        &self,
        painter: &egui::Painter,
        state: &EditorState,
        image: &CapturedImage,
        pointer: Option<Pos2>,
        image_rect: Rect,
        scale: f32,
    ) {
        let thickness = (state.thickness * scale).max(1.0);

        if let Some(drag) = &self.drag {
            match drag {
                Drag::Brush { points } => {
                    let pts: Vec<Pos2> = points.iter().map(|p| image_to_screen(*p, image_rect, scale)).collect();
                    if pts.len() >= 2 {
                        painter.add(Shape::line(pts, Stroke::new(thickness, state.color)));
                    }
                }
                Drag::Shape { start, current } => {
                    let data = match state.current_tool {
                        ToolType::Rectangle => AnnotationData::Rect {
                            x: start.x.min(current.x),
                            y: start.y.min(current.y),
                            w: (current.x - start.x).abs(),
                            h: (current.y - start.y).abs(),
                            filled: state.filled,
                        },
                        ToolType::Ellipse => AnnotationData::Ellipse {
                            x: start.x.min(current.x),
                            y: start.y.min(current.y),
                            w: (current.x - start.x).abs(),
                            h: (current.y - start.y).abs(),
                            filled: state.filled,
                        },
                        ToolType::Arrow => {
                            AnnotationData::Arrow { start: (start.x, start.y), end: (current.x, current.y) }
                        }
                        ToolType::Mosaic => AnnotationData::Mosaic {
                            x: start.x.min(current.x),
                            y: start.y.min(current.y),
                            w: (current.x - start.x).abs(),
                            h: (current.y - start.y).abs(),
                            block: state.mosaic_block,
                        },
                        _ => return,
                    };
                    let preview =
                        Annotation { id: 0, tool: data.tool(), color: state.color, thickness: state.thickness, data };
                    // 马赛克预览没有纹理，走 block 采样回退
                    draw_annotation(painter, &preview, image, image_rect, scale, &self.mosaic_textures);
                }
            }
        } else if let (Some(p), ToolType::Text) = (pointer, state.current_tool) {
            let pos = image_to_screen(p, image_rect, scale);
            let font = FontId::proportional((state.font_size * scale).max(10.0));
            painter.text(pos + Vec2::new(0.0, 2.0), Align2::LEFT_TOP, "|", font, state.color);
        }
    }

    /// 文字输入弹窗
    fn show_text_draft(&mut self, ctx: &egui::Context, state: &mut EditorState) {
        let mut submitted: Option<(Pos2, String)> = None;
        let mut cancelled = false;

        if let Some(draft) = self.text_draft.as_mut() {
            egui::Window::new("添加文字")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::multiline(&mut draft.text)
                            .desired_rows(2)
                            .desired_width(260.0)
                            .hint_text("输入文字，支持换行"),
                    );
                    if draft.request_focus {
                        edit.request_focus();
                        draft.request_focus = false;
                    }

                    ui.horizontal(|ui| {
                        if ui.button("确定").clicked() {
                            submitted = Some((draft.pos, draft.text.clone()));
                        }
                        if ui.button("取消").clicked() {
                            cancelled = true;
                        }
                    });
                    ui.label(egui::RichText::new("Ctrl+Enter 确定 · Esc 取消").small().color(Color32::from_gray(140)));

                    if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter)) {
                        submitted = Some((draft.pos, draft.text.clone()));
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancelled = true;
                    }
                });
        }

        if let Some((pos, content)) = submitted {
            let content = content.trim().to_string();
            if !content.is_empty() {
                state.commit(AnnotationData::Text { x: pos.x, y: pos.y, content, font_size: state.font_size });
            }
            self.text_draft = None;
        } else if cancelled {
            self.text_draft = None;
        }
    }
}

/// 绘制一条标注
fn draw_annotation(
    painter: &egui::Painter,
    ann: &Annotation,
    image: &CapturedImage,
    image_rect: Rect,
    scale: f32,
    mosaics: &HashMap<u64, egui::TextureHandle>,
) {
    let stroke = Stroke::new((ann.thickness * scale).max(1.0), ann.color);
    let to_screen = |p: Pos2| image_to_screen(p, image_rect, scale);
    let rect_of = |x: f32, y: f32, w: f32, h: f32| {
        Rect::from_min_max(to_screen(Pos2::new(x, y)), to_screen(Pos2::new(x + w, y + h)))
    };

    match &ann.data {
        AnnotationData::Rect { x, y, w, h, filled } => {
            let rect = rect_of(*x, *y, *w, *h);
            if *filled {
                painter.rect_filled(rect, 0.0, ann.color);
            } else {
                painter.rect_stroke(rect, 0.0, stroke);
            }
        }
        AnnotationData::Ellipse { x, y, w, h, filled } => {
            let (cx, cy, rx, ry) = (x + w * 0.5, y + h * 0.5, w * 0.5, h * 0.5);
            if *filled {
                let points = ellipse_points(cx, cy, rx, ry, scale);
                painter.add(Shape::convex_polygon(points, ann.color, Stroke::NONE));
            } else {
                let points = ellipse_points(cx, cy, rx, ry, scale);
                painter.add(Shape::closed_line(points, stroke));
            }
        }
        AnnotationData::Arrow { start, end } => {
            let from = to_screen(Pos2::new(start.0, start.1));
            let to = to_screen(Pos2::new(end.0, end.1));
            paint_arrow(painter, from, to, stroke);
        }
        AnnotationData::Brush { points } => {
            let pts: Vec<Pos2> = points.iter().map(|(x, y)| to_screen(Pos2::new(*x, *y))).collect();
            if pts.len() == 1 {
                painter.circle_filled(pts[0], stroke.width * 0.5, ann.color);
            } else if pts.len() >= 2 {
                painter.add(Shape::line(pts, stroke));
            }
        }
        AnnotationData::Text { x, y, content, font_size } => {
            painter.text(
                to_screen(Pos2::new(*x, *y)),
                Align2::LEFT_TOP,
                content,
                FontId::proportional((font_size * scale).max(1.0)),
                ann.color,
            );
        }
        AnnotationData::Number { x, y, value, radius } => {
            let center = to_screen(Pos2::new(*x, *y));
            let r = (radius * scale).max(2.0);
            painter.circle_filled(center, r, ann.color);
            painter.text(
                center,
                Align2::CENTER_CENTER,
                value.to_string(),
                FontId::proportional((r * 1.2).max(8.0)),
                Color32::WHITE,
            );
        }
        AnnotationData::Mosaic { x, y, w, h, block } => {
            let rect = rect_of(*x, *y, *w, *h);
            if let Some(texture) = mosaics.get(&ann.id) {
                painter.image(texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            } else {
                draw_mosaic_blocks(painter, image, rect, *x, *y, *w, *h, *block);
            }
        }
    }
}

/// 马赛克块采样预览（未生成纹理时使用，限制块数量防止过慢）
#[allow(clippy::too_many_arguments)]
fn draw_mosaic_blocks(
    painter: &egui::Painter,
    image: &CapturedImage,
    screen_rect: Rect,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    block: u32,
) {
    let mut block = block.max(2) as i32;
    loop {
        let bw = ((w.ceil() as i32) + block - 1) / block;
        let bh = ((h.ceil() as i32) + block - 1) / block;
        if bw * bh <= 4096 {
            break;
        }
        block *= 2;
    }

    let x0 = (x.floor() as i32).max(0);
    let y0 = (y.floor() as i32).max(0);
    let x1 = ((x + w).ceil() as i32).min(image.width as i32);
    let y1 = ((y + h).ceil() as i32).min(image.height as i32);
    let sx = screen_rect.width() / w.max(1.0);
    let sy = screen_rect.height() / h.max(1.0);

    let mut by = y0;
    while by < y1 {
        let mut bx = x0;
        while bx < x1 {
            let color = sample_pixel(image, bx + block / 2, by + block / 2);
            let cell = Rect::from_min_max(
                Pos2::new(screen_rect.min.x + (bx as f32 - x) * sx, screen_rect.min.y + (by as f32 - y) * sy),
                Pos2::new(
                    screen_rect.min.x + ((bx + block) as f32 - x) * sx,
                    screen_rect.min.y + ((by + block) as f32 - y) * sy,
                ),
            );
            painter.rect_filled(cell, 0.0, color);
            bx += block;
        }
        by += block;
    }
}

/// 椭圆折线采样（屏幕坐标）
fn ellipse_points(cx: f32, cy: f32, rx: f32, ry: f32, scale: f32) -> Vec<Pos2> {
    let steps = (((rx + ry) * scale * 0.5) as usize).clamp(32, 512);
    (0..steps)
        .map(|i| {
            let t = i as f32 / steps as f32 * std::f32::consts::TAU;
            Pos2::new(cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
}

/// 画箭头（线段 + 实心三角箭头）
fn paint_arrow(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    painter.line_segment([from, to], stroke);
    let dir = to - from;
    let len = dir.length();
    if len < 1.0 {
        return;
    }
    let unit = dir / len;
    let head = (stroke.width * 4.0).max(10.0).min(len);
    let half = head * 0.42;
    let perp = Vec2::new(-unit.y, unit.x);
    let base = to - unit * head;
    painter.add(Shape::convex_polygon(vec![to, base + perp * half, base - perp * half], stroke.color, Stroke::NONE));
}

/// 屏幕坐标 → 底图像素坐标（结果钳制在图像范围内）
pub fn screen_to_image(pos: Pos2, image_rect: Rect, scale: f32) -> Pos2 {
    let scale = scale.max(0.0001);
    Pos2::new(
        ((pos.x - image_rect.min.x) / scale).clamp(0.0, image_rect.width() / scale),
        ((pos.y - image_rect.min.y) / scale).clamp(0.0, image_rect.height() / scale),
    )
}

/// 底图像素坐标 → 屏幕坐标
pub fn image_to_screen(pos: Pos2, image_rect: Rect, scale: f32) -> Pos2 {
    Pos2::new(image_rect.min.x + pos.x * scale, image_rect.min.y + pos.y * scale)
}

/// 读取底图某个像素的颜色（越界返回黑色）
fn sample_pixel(image: &CapturedImage, x: i32, y: i32) -> Color32 {
    if x < 0 || y < 0 || x >= image.width as i32 || y >= image.height as i32 {
        return Color32::BLACK;
    }
    let idx = ((y as u32 * image.width + x as u32) * 4) as usize;
    let px = &image.pixels[idx..idx + 4];
    Color32::from_rgb(px[2], px[1], px[0])
}

/// 深色胶囊背景标签
fn draw_pill(painter: &egui::Painter, pos: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(12.0), Color32::WHITE);
    let bg = Rect::from_min_size(pos - Vec2::new(7.0, 4.0), galley.size() + Vec2::new(14.0, 8.0));
    painter.rect_filled(bg, 5.0, Color32::from_black_alpha(190));
    painter.galley(pos, galley, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(min: (f32, f32), size: (f32, f32)) -> Rect {
        Rect::from_min_size(Pos2::new(min.0, min.1), Vec2::new(size.0, size.1))
    }

    #[test]
    fn screen_to_image_maps_proportionally() {
        let area = rect((0.0, 0.0), (1000.0, 800.0));
        let scale = 2.0;
        let p = screen_to_image(Pos2::new(140.0, 90.0), area, scale);
        assert!((p.x - 70.0).abs() < 0.001);
        assert!((p.y - 45.0).abs() < 0.001);
    }

    #[test]
    fn screen_to_image_clamps_outside() {
        let area = rect((100.0, 50.0), (400.0, 300.0));
        let scale = 2.0;
        // 左侧外部钳制到 0
        let p = screen_to_image(Pos2::new(0.0, 0.0), area, scale);
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 0.0);
        // 右下外部钳制到图像边界
        let p = screen_to_image(Pos2::new(9999.0, 9999.0), area, scale);
        assert!((p.x - 200.0).abs() < 0.001);
        assert!((p.y - 150.0).abs() < 0.001);
    }

    #[test]
    fn image_to_screen_roundtrip() {
        let area = rect((80.0, 40.0), (640.0, 480.0));
        let scale = 1.5;
        let original = Pos2::new(120.0, 66.0);
        let screen = image_to_screen(original, area, scale);
        let back = screen_to_image(screen, area, scale);
        assert!((back.x - original.x).abs() < 0.001);
        assert!((back.y - original.y).abs() < 0.001);
    }
}
