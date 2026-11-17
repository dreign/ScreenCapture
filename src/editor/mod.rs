//! 截图编辑器模块（M4）
//!
//! - [`EditorState`]：工具、颜色、粗细、字号、标注列表、撤销/重做
//! - [`EditorCanvas`]：底图缩放/平移 + 各工具绘制交互
//! - [`render`]：把标注叠加到底图像素（保存 / 复制用）

pub mod canvas;
pub mod render;

use eframe::egui::{self, Color32, Sense, Stroke, Vec2};

pub use canvas::EditorCanvas;

/// 工具类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    Select,
    Pen,
    Rectangle,
    Ellipse,
    Arrow,
    Text,
    Number,
    Mosaic,
    Eyedropper,
}

impl ToolType {
    pub const ALL: [ToolType; 9] = [
        ToolType::Select,
        ToolType::Pen,
        ToolType::Rectangle,
        ToolType::Ellipse,
        ToolType::Arrow,
        ToolType::Text,
        ToolType::Number,
        ToolType::Mosaic,
        ToolType::Eyedropper,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ToolType::Select => "选择",
            ToolType::Pen => "画笔",
            ToolType::Rectangle => "矩形",
            ToolType::Ellipse => "椭圆",
            ToolType::Arrow => "箭头",
            ToolType::Text => "文字",
            ToolType::Number => "序号",
            ToolType::Mosaic => "马赛克",
            ToolType::Eyedropper => "取色",
        }
    }

    /// 悬停提示（含快捷键）
    pub fn hint(self) -> &'static str {
        match self {
            ToolType::Select => "选择 / 拖拽平移画布 (V)",
            ToolType::Pen => "自由画笔 (P)",
            ToolType::Rectangle => "矩形框 (R)",
            ToolType::Ellipse => "椭圆 (E)",
            ToolType::Arrow => "箭头 (A)",
            ToolType::Text => "文字 (T)，点击位置后输入",
            ToolType::Number => "序号标记，自动递增 (N)",
            ToolType::Mosaic => "马赛克，拖拽涂抹 (M)",
            ToolType::Eyedropper => "取色器，点击拾取颜色 (C)",
        }
    }
}

/// 一条标注
#[derive(Debug, Clone)]
pub struct Annotation {
    pub id: u64,
    pub tool: ToolType,
    pub color: Color32,
    pub thickness: f32,
    pub data: AnnotationData,
}

/// 标注几何数据（坐标均为底图像素坐标）
#[derive(Debug, Clone)]
pub enum AnnotationData {
    Rect { x: f32, y: f32, w: f32, h: f32, filled: bool },
    Ellipse { x: f32, y: f32, w: f32, h: f32, filled: bool },
    Arrow { start: (f32, f32), end: (f32, f32) },
    Brush { points: Vec<(f32, f32)> },
    Text { x: f32, y: f32, content: String, font_size: f32 },
    Number { x: f32, y: f32, value: u32, radius: f32 },
    Mosaic { x: f32, y: f32, w: f32, h: f32, block: u32 },
}

impl AnnotationData {
    pub fn tool(&self) -> ToolType {
        match self {
            AnnotationData::Rect { .. } => ToolType::Rectangle,
            AnnotationData::Ellipse { .. } => ToolType::Ellipse,
            AnnotationData::Arrow { .. } => ToolType::Arrow,
            AnnotationData::Brush { .. } => ToolType::Pen,
            AnnotationData::Text { .. } => ToolType::Text,
            AnnotationData::Number { .. } => ToolType::Number,
            AnnotationData::Mosaic { .. } => ToolType::Mosaic,
        }
    }
}

/// 编辑器状态
#[derive(Debug, Clone)]
pub struct EditorState {
    pub current_tool: ToolType,
    pub color: Color32,
    pub thickness: f32,
    pub font_size: f32,
    /// 矩形 / 椭圆是否填充
    pub filled: bool,
    /// 马赛克块大小（底图像素）
    pub mosaic_block: u32,
    pub annotations: Vec<Annotation>,
    redo_stack: Vec<Annotation>,
    next_id: u64,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            current_tool: ToolType::Pen,
            color: Color32::from_rgb(255, 59, 48),
            thickness: 3.0,
            font_size: 20.0,
            filled: false,
            mosaic_block: 12,
            annotations: Vec::new(),
            redo_stack: Vec::new(),
            next_id: 1,
        }
    }
}

impl EditorState {
    /// 提交一条标注（新标注会清空重做栈）
    pub fn commit(&mut self, data: AnnotationData) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.annotations.push(Annotation { id, tool: data.tool(), color: self.color, thickness: self.thickness, data });
        self.redo_stack.clear();
        id
    }

    /// 下一个序号的数值（按现有序号标注数量 + 1）
    pub fn next_number(&self) -> u32 {
        let count = self.annotations.iter().filter(|a| matches!(a.data, AnnotationData::Number { .. })).count();
        count as u32 + 1
    }

    pub fn can_undo(&self) -> bool {
        !self.annotations.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// 撤销，返回是否有变化
    pub fn undo(&mut self) -> bool {
        match self.annotations.pop() {
            Some(ann) => {
                self.redo_stack.push(ann);
                true
            }
            None => false,
        }
    }

    /// 重做，返回是否有变化
    pub fn redo(&mut self) -> bool {
        match self.redo_stack.pop() {
            Some(ann) => {
                self.annotations.push(ann);
                true
            }
            None => false,
        }
    }

    /// 清空所有标注
    pub fn clear(&mut self) {
        self.annotations.clear();
        self.redo_stack.clear();
    }
}

/// 编辑器工具栏返回的动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    Save,
    Copy,
    /// 固定到屏幕（置顶悬浮窗）
    Pin,
    Close,
    ClearAll,
}

const PRESET_COLORS: [Color32; 8] = [
    Color32::from_rgb(255, 59, 48),
    Color32::from_rgb(255, 149, 0),
    Color32::from_rgb(255, 214, 10),
    Color32::from_rgb(52, 199, 89),
    Color32::from_rgb(0, 174, 255),
    Color32::from_rgb(90, 120, 255),
    Color32::WHITE,
    Color32::from_rgb(30, 30, 30),
];

/// 绘制编辑器工具栏（工具 / 颜色 / 粗细 / 字号 / 撤销重做 / 输出）
pub fn draw_toolbar(ui: &mut egui::Ui, state: &mut EditorState) -> Option<EditorAction> {
    let mut action = None;

    // 第一行：工具 + 颜色
    ui.horizontal(|ui| {
        ui.label("工具");
        for tool in ToolType::ALL {
            let selected = state.current_tool == tool;
            if ui.selectable_label(selected, tool.label()).on_hover_text(tool.hint()).clicked() {
                state.current_tool = tool;
            }
        }

        ui.separator();
        ui.label("颜色");
        for color in PRESET_COLORS {
            if color_swatch(ui, color, state.color == color) {
                state.color = color;
            }
        }
        ui.color_edit_button_srgba(&mut state.color);
        ui.label(format!("#{:02X}{:02X}{:02X}", state.color.r(), state.color.g(), state.color.b()));
    });

    // 第二行：参数 + 编辑 + 输出
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut state.thickness, 1.0..=24.0).text("粗细").fixed_decimals(0));
        ui.add(egui::Slider::new(&mut state.font_size, 10.0..=72.0).text("字号").fixed_decimals(0));
        match state.current_tool {
            ToolType::Rectangle | ToolType::Ellipse => {
                ui.checkbox(&mut state.filled, "填充");
            }
            ToolType::Mosaic => {
                ui.add(egui::Slider::new(&mut state.mosaic_block, 4..=40).text("马赛克块").fixed_decimals(0));
            }
            _ => {}
        }

        ui.separator();
        if ui.add_enabled(state.can_undo(), egui::Button::new("撤销")).on_hover_text("Ctrl+Z").clicked() {
            state.undo();
        }
        if ui.add_enabled(state.can_redo(), egui::Button::new("重做")).on_hover_text("Ctrl+Y / Ctrl+Shift+Z").clicked()
        {
            state.redo();
        }
        if ui.button("清除标注").clicked() {
            action = Some(EditorAction::ClearAll);
        }

        ui.separator();
        if ui.button("保存").clicked() {
            action = Some(EditorAction::Save);
        }
        if ui.button("复制").clicked() {
            action = Some(EditorAction::Copy);
        }
        if ui.button("固定").on_hover_text("固定到屏幕（置顶）").clicked() {
            action = Some(EditorAction::Pin);
        }
        if ui.button("完成").clicked() {
            action = Some(EditorAction::Close);
        }
    });

    action
}

/// 一个可点击的色块
fn color_swatch(ui: &mut egui::Ui, color: Color32, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
    ui.painter().rect_filled(rect, 3.0, color);
    if selected {
        ui.painter().rect_stroke(rect.expand(1.0), 3.0, Stroke::new(1.5_f32, Color32::WHITE));
    } else {
        ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0_f32, Color32::from_gray(120)));
    }
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_data() -> AnnotationData {
        AnnotationData::Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0, filled: false }
    }

    #[test]
    fn commit_and_undo_redo() {
        let mut state = EditorState::default();
        state.commit(rect_data());
        state.commit(rect_data());
        assert_eq!(state.annotations.len(), 2);
        assert!(state.can_undo());
        assert!(!state.can_redo());

        assert!(state.undo());
        assert_eq!(state.annotations.len(), 1);
        assert!(state.can_redo());

        assert!(state.redo());
        assert_eq!(state.annotations.len(), 2);

        // 新标注应清空重做栈
        state.undo();
        assert!(state.can_redo());
        state.commit(rect_data());
        assert!(!state.can_redo());
    }

    #[test]
    fn number_counter_increments_and_rewinds() {
        let mut state = EditorState::default();
        assert_eq!(state.next_number(), 1);

        state.commit(AnnotationData::Number { x: 1.0, y: 1.0, value: 1, radius: 12.0 });
        assert_eq!(state.next_number(), 2);

        state.undo();
        assert_eq!(state.next_number(), 1);
    }

    #[test]
    fn annotation_tool_matches_data() {
        assert_eq!(rect_data().tool(), ToolType::Rectangle);
        assert_eq!(AnnotationData::Mosaic { x: 0.0, y: 0.0, w: 1.0, h: 1.0, block: 8 }.tool(), ToolType::Mosaic);
    }
}
