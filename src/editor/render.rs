//! 标注渲染：把编辑器中的矢量标注“烘焙”进底图像素
//!
//! 使用软件光栅化（含 1px 抗锯齿），不依赖 GPU / egui 上下文，
//! 因此保存与复制得到的图片与画布预览一致。

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use eframe::egui::Color32;

use super::{Annotation, AnnotationData};
use crate::capture::CapturedImage;

/// 把标注叠加到底图，返回合成后的截图
pub fn flatten(base: &CapturedImage, annotations: &[Annotation]) -> anyhow::Result<CapturedImage> {
    let mut out = base.clone();
    let width = out.width;
    let height = out.height;

    {
        let mut surface = Surface { buf: &mut out.pixels, w: width, h: height, font: load_font() };
        for ann in annotations {
            surface.apply(ann);
        }
    }

    Ok(out)
}

/// 加载系统字体（优先黑体，TTC 集合部分字体库不支持时自动回退）
fn load_font() -> Option<FontVec> {
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(font) = FontVec::try_from_vec(bytes) {
                return Some(font);
            }
        }
    }
    None
}

/// 一条水平扫描线的重叠长度
fn overlap(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// 点到线段的距离
fn distance_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq <= f32::EPSILON { 0.0 } else { (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0) };
    let cx = ax + dx * t;
    let cy = ay + dy * t;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// 有符号边函数（三角形填充用）
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (px - ax) * (by - ay) - (py - ay) * (bx - ax)
}

/// BGRA 像素画布 + 字体
struct Surface<'a> {
    buf: &'a mut [u8],
    w: u32,
    h: u32,
    font: Option<FontVec>,
}

impl Surface<'_> {
    /// 按覆盖率混合一个像素（BGRA，源色不透明）
    fn blend(&mut self, x: i32, y: i32, color: Color32, coverage: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let alpha = coverage.clamp(0.0, 1.0) * (color.a() as f32 / 255.0);
        if alpha <= 0.0 {
            return;
        }
        let idx = ((y as u32 * self.w + x as u32) * 4) as usize;
        let inv = 1.0 - alpha;
        let b = self.buf[idx] as f32 * inv + color.b() as f32 * alpha;
        let g = self.buf[idx + 1] as f32 * inv + color.g() as f32 * alpha;
        let r = self.buf[idx + 2] as f32 * inv + color.r() as f32 * alpha;
        self.buf[idx] = b.round().clamp(0.0, 255.0) as u8;
        self.buf[idx + 1] = g.round().clamp(0.0, 255.0) as u8;
        self.buf[idx + 2] = r.round().clamp(0.0, 255.0) as u8;
        self.buf[idx + 3] = 255;
    }

    /// 抗锯齿粗线段
    fn stroke_segment(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: Color32) {
        let half = (width * 0.5).max(0.5);
        let pad = half + 1.0;
        let x0 = ((a.0.min(b.0) - pad).floor() as i32).max(0);
        let y0 = ((a.1.min(b.1) - pad).floor() as i32).max(0);
        let x1 = ((a.0.max(b.0) + pad).ceil() as i32).min(self.w as i32 - 1);
        let y1 = ((a.1.max(b.1) + pad).ceil() as i32).min(self.h as i32 - 1);

        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = distance_to_segment(x as f32 + 0.5, y as f32 + 0.5, a.0, a.1, b.0, b.1);
                let cov = (half + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend(x, y, color, cov);
                }
            }
        }
    }

    fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Color32) {
        let (x0, y0, x1, y1) = (x, y, x + w, y + h);
        self.stroke_segment((x0, y0), (x1, y0), width, color);
        self.stroke_segment((x0, y1), (x1, y1), width, color);
        self.stroke_segment((x0, y0), (x0, y1), width, color);
        self.stroke_segment((x1, y0), (x1, y1), width, color);
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        let (rx0, ry0, rx1, ry1) = (x.min(x + w), y.min(y + h), x.max(x + w), y.max(y + h));
        let x0 = (rx0.floor() as i32).max(0);
        let y0 = (ry0.floor() as i32).max(0);
        let x1 = (rx1.ceil() as i32).min(self.w as i32 - 1);
        let y1 = (ry1.ceil() as i32).min(self.h as i32 - 1);

        for py in y0..=y1 {
            for px in x0..=x1 {
                let cx = overlap(px as f32, px as f32 + 1.0, rx0, rx1);
                let cy = overlap(py as f32, py as f32 + 1.0, ry0, ry1);
                if cx > 0.0 && cy > 0.0 {
                    self.blend(px, py, color, cx * cy);
                }
            }
        }
    }

    fn stroke_ellipse(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Color32) {
        let (cx, cy, rx, ry) = (x + w * 0.5, y + h * 0.5, w * 0.5, h * 0.5);
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let steps = (((rx + ry) * 0.5) as usize).clamp(32, 512);
        let mut prev = (cx + rx, cy);
        for i in 1..=steps {
            let t = i as f32 / steps as f32 * std::f32::consts::TAU;
            let next = (cx + rx * t.cos(), cy + ry * t.sin());
            self.stroke_segment(prev, next, width, color);
            prev = next;
        }
    }

    fn fill_ellipse(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        let (cx, cy, rx, ry) = (x + w * 0.5, y + h * 0.5, w * 0.5, h * 0.5);
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let x0 = ((cx - rx).floor() as i32).max(0);
        let y0 = ((cy - ry).floor() as i32).max(0);
        let x1 = ((cx + rx).ceil() as i32).min(self.w as i32 - 1);
        let y1 = ((cy + ry).ceil() as i32).min(self.h as i32 - 1);
        let scale = rx.min(ry);

        for py in y0..=y1 {
            for px in x0..=x1 {
                let nx = (px as f32 + 0.5 - cx) / rx;
                let ny = (py as f32 + 0.5 - cy) / ry;
                let dist = ((nx * nx + ny * ny).sqrt() - 1.0) * scale;
                let cov = (0.5 - dist).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend(px, py, color, cov);
                }
            }
        }
    }

    fn fill_triangle(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), color: Color32) {
        let area = edge(a.0, a.1, b.0, b.1, c.0, c.1);
        if area.abs() < 0.001 {
            return;
        }
        let sign = area.signum();
        let len_ab = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(0.001);
        let len_bc = ((c.0 - b.0).powi(2) + (c.1 - b.1).powi(2)).sqrt().max(0.001);
        let len_ca = ((a.0 - c.0).powi(2) + (a.1 - c.1).powi(2)).sqrt().max(0.001);

        let x0 = (a.0.min(b.0).min(c.0).floor() as i32).max(0);
        let y0 = (a.1.min(b.1).min(c.1).floor() as i32).max(0);
        let x1 = (a.0.max(b.0).max(c.0).ceil() as i32).min(self.w as i32 - 1);
        let y1 = (a.1.max(b.1).max(c.1).ceil() as i32).min(self.h as i32 - 1);

        for py in y0..=y1 {
            for px in x0..=x1 {
                let (sx, sy) = (px as f32 + 0.5, py as f32 + 0.5);
                let d0 = edge(a.0, a.1, b.0, b.1, sx, sy) * sign / len_ab;
                let d1 = edge(b.0, b.1, c.0, c.1, sx, sy) * sign / len_bc;
                let d2 = edge(c.0, c.1, a.0, a.1, sx, sy) * sign / len_ca;
                let cov = (d0.min(d1).min(d2) + 0.5).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend(px, py, color, cov);
                }
            }
        }
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, radius: f32, color: Color32) {
        if radius <= 0.0 {
            return;
        }
        let x0 = ((cx - radius).floor() as i32).max(0);
        let y0 = ((cy - radius).floor() as i32).max(0);
        let x1 = ((cx + radius).ceil() as i32).min(self.w as i32 - 1);
        let y1 = ((cy + radius).ceil() as i32).min(self.h as i32 - 1);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let d = ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - cy).powi(2)).sqrt();
                let cov = (radius + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend(px, py, color, cov);
                }
            }
        }
    }

    /// 马赛克：按块求均值后回填
    fn mosaic(&mut self, x: f32, y: f32, w: f32, h: f32, block: u32) {
        let block = block.max(2) as i32;
        let x0 = (x.floor() as i32).max(0);
        let y0 = (y.floor() as i32).max(0);
        let x1 = ((x + w).ceil() as i32).min(self.w as i32);
        let y1 = ((y + h).ceil() as i32).min(self.h as i32);
        if x1 <= x0 || y1 <= y0 {
            return;
        }

        let mut by = y0;
        while by < y1 {
            let bh = (by + block).min(y1) - by;
            let mut bx = x0;
            while bx < x1 {
                let bw = (bx + block).min(x1) - bx;
                let (mut sb, mut sg, mut sr, mut n) = (0u32, 0u32, 0u32, 0u32);
                for py in by..by + bh {
                    for px in bx..bx + bw {
                        let idx = ((py as u32 * self.w + px as u32) * 4) as usize;
                        sb += self.buf[idx] as u32;
                        sg += self.buf[idx + 1] as u32;
                        sr += self.buf[idx + 2] as u32;
                        n += 1;
                    }
                }
                let n = n.max(1);
                let (b, g, r) = ((sb / n) as u8, (sg / n) as u8, (sr / n) as u8);
                for py in by..by + bh {
                    for px in bx..bx + bw {
                        let idx = ((py as u32 * self.w + px as u32) * 4) as usize;
                        self.buf[idx] = b;
                        self.buf[idx + 1] = g;
                        self.buf[idx + 2] = r;
                    }
                }
                bx += block;
            }
            by += block;
        }
    }

    /// 测量单行文字宽度
    fn text_width(font: &FontVec, size: f32, text: &str) -> f32 {
        let scaled = font.as_scaled(PxScale::from(size));
        text.chars().filter(|c| *c != '\n' && *c != '\r').map(|c| scaled.h_advance(font.glyph_id(c))).sum()
    }

    /// 以基线为锚点绘制文字（支持 `\n` 换行）
    fn text_at_baseline(&mut self, text: &str, x: f32, baseline: f32, size: f32, color: Color32) {
        let Some(font) = self.font.take() else {
            return;
        };
        let scale = PxScale::from(size.max(1.0));
        let scaled = font.as_scaled(scale);
        let line_height = (scaled.ascent() - scaled.descent()).max(size);
        let mut caret = x;
        let mut line_baseline = baseline;

        for ch in text.chars() {
            match ch {
                '\n' => {
                    caret = x;
                    line_baseline += line_height;
                }
                '\r' => {}
                _ => {
                    let id = font.glyph_id(ch);
                    let glyph = id.with_scale_and_position(scale, ab_glyph::point(caret, line_baseline));
                    if let Some(outlined) = font.outline_glyph(glyph) {
                        let bounds = outlined.px_bounds();
                        let (ox, oy) = (bounds.min.x, bounds.min.y);
                        outlined.draw(|gx, gy, cov| {
                            self.blend(ox as i32 + gx as i32, oy as i32 + gy as i32, color, cov);
                        });
                    }
                    caret += scaled.h_advance(id);
                }
            }
        }

        self.font = Some(font);
    }

    /// 以左上角为锚点绘制文字
    fn text(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color32) {
        let ascent = {
            let Some(font) = self.font.as_ref() else {
                return;
            };
            let scaled = font.as_scaled(PxScale::from(size.max(1.0)));
            scaled.ascent()
        };
        self.text_at_baseline(text, x, y + ascent, size, color);
    }

    /// 以中心为锚点绘制序号（圆形底 + 白色数字）
    fn number(&mut self, cx: f32, cy: f32, value: u32, radius: f32, color: Color32) {
        self.fill_circle(cx, cy, radius, color);
        let text = value.to_string();
        let size = (radius * 1.2).max(8.0);
        let Some(font) = self.font.as_ref() else {
            return;
        };
        let width = Self::text_width(font, size, &text);
        // 数字视觉中心约在基线上方 0.35em
        self.text_at_baseline(&text, cx - width * 0.5, cy + size * 0.35, size, Color32::WHITE);
    }

    fn apply(&mut self, ann: &Annotation) {
        let color = ann.color;
        let thickness = ann.thickness.max(1.0);
        match &ann.data {
            AnnotationData::Rect { x, y, w, h, filled } => {
                if *filled {
                    self.fill_rect(*x, *y, *w, *h, color);
                } else {
                    self.stroke_rect(*x, *y, *w, *h, thickness, color);
                }
            }
            AnnotationData::Ellipse { x, y, w, h, filled } => {
                if *filled {
                    self.fill_ellipse(*x, *y, *w, *h, color);
                } else {
                    self.stroke_ellipse(*x, *y, *w, *h, thickness, color);
                }
            }
            AnnotationData::Arrow { start, end } => {
                let (sx, sy) = *start;
                let (ex, ey) = *end;
                let dx = ex - sx;
                let dy = ey - sy;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 0.001 {
                    return;
                }
                let (ux, uy) = (dx / len, dy / len);
                let head = (thickness * 4.0).max(9.0).min(len);
                let half = head * 0.42;
                let base = (ex - ux * head, ey - uy * head);
                self.stroke_segment((sx, sy), base, thickness, color);
                self.fill_triangle(
                    (ex, ey),
                    (base.0 - uy * half, base.1 + ux * half),
                    (base.0 + uy * half, base.1 - ux * half),
                    color,
                );
            }
            AnnotationData::Brush { points } => {
                if points.len() == 1 {
                    self.fill_circle(points[0].0, points[0].1, thickness * 0.5, color);
                }
                for pair in points.windows(2) {
                    self.stroke_segment(pair[0], pair[1], thickness, color);
                }
            }
            AnnotationData::Text { x, y, content, font_size } => {
                self.text(content, *x, *y, *font_size, color);
            }
            AnnotationData::Number { x, y, value, radius } => {
                self.number(*x, *y, *value, *radius, color);
            }
            AnnotationData::Mosaic { x, y, w, h, block } => {
                self.mosaic(*x, *y, *w, *h, *block);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureSource, CapturedImage};
    use std::time::Duration;

    fn image_with_two_halves() -> CapturedImage {
        // 4x4：左半黑、右半白（BGRA）
        let mut pixels = Vec::new();
        for _y in 0..4 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 255 };
                pixels.extend_from_slice(&[v, v, v, 255]);
            }
        }
        CapturedImage { width: 4, height: 4, pixels, source: CaptureSource::FullScreen, elapsed: Duration::ZERO }
    }

    fn sample(image: &CapturedImage, x: u32, y: u32) -> [u8; 3] {
        let idx = ((y * image.width + x) * 4) as usize;
        [image.pixels[idx + 2], image.pixels[idx + 1], image.pixels[idx]]
    }

    fn annotation(id: u64, data: AnnotationData) -> Annotation {
        Annotation { id, tool: data.tool(), color: Color32::from_rgb(255, 0, 0), thickness: 2.0, data }
    }

    #[test]
    fn fully_outside_annotations_leave_pixels_unchanged() {
        let base = image_with_two_halves();
        let annotations = vec![
            annotation(1, AnnotationData::Rect { x: -20.0, y: -20.0, w: 10.0, h: 10.0, filled: true }),
            annotation(2, AnnotationData::Ellipse { x: 10.0, y: 10.0, w: 8.0, h: 8.0, filled: true }),
            annotation(3, AnnotationData::Text { x: -50.0, y: -50.0, content: "X".to_string(), font_size: 20.0 }),
            annotation(4, AnnotationData::Mosaic { x: -10.0, y: -10.0, w: 8.0, h: 8.0, block: 4 }),
            annotation(5, AnnotationData::Arrow { start: (-30.0, -30.0), end: (-5.0, -5.0) }),
        ];

        let out = flatten(&base, &annotations).expect("越界标注不应失败");
        assert_eq!(out.pixels, base.pixels, "完全在图像外的标注不应改变像素");
    }

    #[test]
    fn partially_visible_annotation_only_affects_inside() {
        let base = image_with_two_halves();
        let out =
            flatten(&base, &[annotation(1, AnnotationData::Rect { x: -2.0, y: -2.0, w: 4.0, h: 4.0, filled: true })])
                .expect("部分越界标注不应失败");

        assert_eq!(sample(&out, 0, 0), [255, 0, 0], "图像内部分应被填充");
        assert_eq!(sample(&out, 3, 3), [255, 255, 255], "图像外部分不应影响像素");
    }

    #[test]
    fn mosaic_averages_each_block() {
        let base = image_with_two_halves();
        let ann = Annotation {
            id: 1,
            tool: super::super::ToolType::Mosaic,
            color: Color32::RED,
            thickness: 3.0,
            data: AnnotationData::Mosaic { x: 0.0, y: 0.0, w: 4.0, h: 4.0, block: 2 },
        };
        let out = flatten(&base, &[ann]).expect("合成应成功");
        // 左上块全黑、右上块全白
        assert_eq!(sample(&out, 0, 0), [0, 0, 0]);
        assert_eq!(sample(&out, 3, 0), [255, 255, 255]);
        // 中间块（x=2..4）应保持纯白，不受左半影响
        assert_eq!(sample(&out, 2, 2), [255, 255, 255]);
    }

    #[test]
    fn filled_rect_covers_area_but_not_outside() {
        let base = image_with_two_halves();
        let ann = Annotation {
            id: 1,
            tool: super::super::ToolType::Rectangle,
            color: Color32::from_rgb(255, 0, 0),
            thickness: 2.0,
            data: AnnotationData::Rect { x: 0.0, y: 0.0, w: 2.0, h: 2.0, filled: true },
        };
        let out = flatten(&base, &[ann]).expect("合成应成功");
        assert_eq!(sample(&out, 0, 0), [255, 0, 0]);
        assert_eq!(sample(&out, 1, 1), [255, 0, 0]);
        // 区域外像素保持原样
        assert_eq!(sample(&out, 3, 3), [255, 255, 255]);
    }

    #[test]
    fn stroke_rect_keeps_center_untouched() {
        let base = image_with_two_halves();
        let ann = Annotation {
            id: 1,
            tool: super::super::ToolType::Rectangle,
            color: Color32::from_rgb(0, 255, 0),
            thickness: 1.0,
            data: AnnotationData::Rect { x: 0.5, y: 0.5, w: 3.0, h: 3.0, filled: false },
        };
        let out = flatten(&base, &[ann]).expect("合成应成功");
        // 中心像素不被描边覆盖（黑半区）
        assert_eq!(sample(&out, 2, 2), [255, 255, 255]);
    }

    #[test]
    fn text_and_number_are_baked_into_pixels() {
        // 无系统字体时跳过（本测试目标平台为 Windows）
        if load_font().is_none() {
            return;
        }
        let size = 64u32;
        let black = CapturedImage {
            width: size,
            height: size,
            pixels: [0u8, 0, 0, 255].repeat((size * size) as usize),
            source: CaptureSource::FullScreen,
            elapsed: Duration::ZERO,
        };

        let text = Annotation {
            id: 1,
            tool: super::super::ToolType::Text,
            color: Color32::WHITE,
            thickness: 2.0,
            data: AnnotationData::Text { x: 4.0, y: 4.0, content: "A".to_string(), font_size: 40.0 },
        };
        let out = flatten(&black, &[text]).expect("文字合成应成功");
        let lit = out.pixels.as_chunks::<4>().0.iter().filter(|p| p[0] > 128).count();
        assert!(lit > 20, "文字应留下可见像素，实际 {lit}");

        let number = Annotation {
            id: 2,
            tool: super::super::ToolType::Number,
            color: Color32::from_rgb(255, 0, 0),
            thickness: 3.0,
            data: AnnotationData::Number { x: 32.0, y: 32.0, value: 1, radius: 12.0 },
        };
        let out = flatten(&black, &[number]).expect("序号合成应成功");
        let red = out.pixels.as_chunks::<4>().0.iter().filter(|p| p[2] > 180 && p[1] < 100 && p[0] < 100).count();
        assert!(red > 100, "序号圆点应可见，实际 {red}");
    }
}
