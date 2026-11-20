//! 统一视觉主题
//!
//! 集中管理配色语义、字号层级、间距与圆角节奏，并把 egui 的全局 `Style`
//! 一次性装配好。界面代码只引用这里的语义常量，不再散落硬编码颜色/尺寸。
//!
//! 分三类：
//! - [`Palette`]：随深色/浅色模式变化的语义色（背景、控件、文字、状态色）
//! - [`overlay`]：选区覆盖层、固定窗等“浮在截图上”的元素，固定深色以保证可读
//! - [`canvas`]：编辑器画布工作区，固定中性灰，不随主题变化

use eframe::egui::{self, Color32, FontFamily, FontId, Margin, Rounding, Stroke, TextStyle, Vec2};

// ---------------- 间距节奏（4pt 基准） ----------------

pub mod spacing {
    /// 紧凑间距（控件内部、工具栏上下）
    pub const XS: f32 = 4.0;
    /// 常规间距（一行控件之间）
    pub const SM: f32 = 8.0;
    /// 分组间距（标题前后、区块之间）
    pub const MD: f32 = 12.0;
    /// 大间距（页面级留白）
    pub const LG: f32 = 16.0;
}

// ---------------- 圆角节奏 ----------------

pub mod radius {
    /// 按钮、输入框、色块
    pub const SMALL: f32 = 4.0;
    /// 菜单、浮动提示
    pub const MEDIUM: f32 = 6.0;
    /// 窗口、对话框
    pub const LARGE: f32 = 10.0;
}

// ---------------- 字号层级 ----------------

pub mod text {
    /// 窗口/页面标题（`TextStyle::Heading`）
    pub const HEADING: f32 = 18.0;
    /// 正文（`TextStyle::Body` / `TextStyle::Button`）
    pub const BODY: f32 = 14.0;
    /// 说明文字
    pub const CAPTION: f32 = 12.0;
    /// 覆盖层操作提示
    pub const HINT: f32 = 13.0;
    /// 次要提示（固定窗）
    pub const TINY: f32 = 11.0;
    /// 等宽
    pub const MONO: f32 = 12.0;
}

// ---------------- 浮层（与主题无关，保证任意截图上可读） ----------------

pub mod overlay {
    use super::Color32;

    /// 选区外的压暗遮罩
    pub const SCRIM: Color32 = Color32::from_black_alpha(120);
    /// 胶囊标签背景
    pub const PILL_BG: Color32 = Color32::from_black_alpha(200);
    /// 胶囊标签文字
    pub const TEXT: Color32 = Color32::WHITE;
    /// 次要文字
    pub const TEXT_MUTED: Color32 = Color32::from_rgba_premultiplied(220, 220, 220, 220);
    /// 十字辅助线
    pub const CROSSHAIR: Color32 = Color32::from_rgba_premultiplied(70, 70, 70, 70);
    /// 选区边框 / 主色
    pub const ACCENT: Color32 = Color32::from_rgb(0, 174, 255);
    /// 窗口悬停高亮
    pub const HOVER: Color32 = Color32::from_rgb(255, 72, 72);
}

// ---------------- 编辑器画布（固定中性灰工作区） ----------------

pub mod canvas {
    use super::Color32;

    /// 画布底色
    pub const BACKDROP: Color32 = Color32::from_gray(26);
    /// 图片加载前的占位色
    pub const PLACEHOLDER: Color32 = Color32::from_gray(60);
    /// 图片边框
    pub const BORDER: Color32 = Color32::from_gray(90);
    /// 画布提示文字
    pub const HINT: Color32 = Color32::from_gray(190);
    /// 次要说明文字
    pub const TEXT_MUTED: Color32 = Color32::from_gray(140);
}

// ---------------- 标注调色板 ----------------

/// 标注预设色：红 / 橙 / 黄 / 绿 / 蓝 / 靛 / 白 / 近黑
pub const ANNOTATION_COLORS: [Color32; 8] = [
    Color32::from_rgb(255, 59, 48),
    Color32::from_rgb(255, 149, 0),
    Color32::from_rgb(255, 214, 10),
    Color32::from_rgb(52, 199, 89),
    Color32::from_rgb(0, 174, 255),
    Color32::from_rgb(90, 120, 255),
    Color32::WHITE,
    Color32::from_rgb(30, 30, 30),
];

/// 默认标注色（预设第一个）
pub const ANNOTATION_DEFAULT: Color32 = ANNOTATION_COLORS[0];

/// 未选中色块的描边
pub const SWATCH_BORDER: Color32 = Color32::from_gray(120);

// ---------------- 语义色 ----------------

/// 一套完整的语义色板
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// 主色：选中、链接、聚焦描边
    pub accent: Color32,
    /// 主色低饱和填充：按下态、文本选区背景
    pub accent_soft: Color32,
    /// 危险 / 错误
    pub danger: Color32,
    /// 警告
    pub warning: Color32,
    /// 成功
    pub success: Color32,
    /// 面板底色
    pub panel: Color32,
    /// 窗口 / 卡片底色
    pub window: Color32,
    /// 输入框等凹陷区域底色
    pub sunken: Color32,
    /// 弱化分区底色
    pub faint: Color32,
    /// 边框 / 分隔线
    pub border: Color32,
    /// 常规控件底色
    pub widget: Color32,
    /// 悬停态
    pub hover: Color32,
    /// 按下 / 激活态
    pub active: Color32,
    /// 正文文字
    pub text: Color32,
    /// 强调文字
    pub text_strong: Color32,
    /// 次要文字
    pub text_muted: Color32,
}

/// 深色模式色板
pub const DARK: Palette = Palette {
    accent: Color32::from_rgb(0, 174, 255),
    accent_soft: Color32::from_rgb(20, 90, 130),
    danger: Color32::from_rgb(255, 122, 112),
    warning: Color32::from_rgb(255, 184, 76),
    success: Color32::from_rgb(90, 205, 126),
    panel: Color32::from_rgb(27, 29, 32),
    window: Color32::from_rgb(36, 39, 43),
    sunken: Color32::from_rgb(18, 20, 22),
    faint: Color32::from_rgb(42, 46, 51),
    border: Color32::from_rgb(56, 61, 68),
    widget: Color32::from_rgb(44, 48, 54),
    hover: Color32::from_rgb(57, 63, 71),
    active: Color32::from_rgb(24, 84, 120),
    text: Color32::from_rgb(215, 219, 224),
    text_strong: Color32::WHITE,
    text_muted: Color32::from_rgb(140, 148, 157),
};

/// 浅色模式色板
pub const LIGHT: Palette = Palette {
    accent: Color32::from_rgb(0, 122, 204),
    accent_soft: Color32::from_rgb(210, 231, 249),
    danger: Color32::from_rgb(196, 52, 48),
    warning: Color32::from_rgb(158, 104, 6),
    success: Color32::from_rgb(26, 133, 71),
    panel: Color32::from_rgb(246, 247, 249),
    window: Color32::WHITE,
    sunken: Color32::WHITE,
    faint: Color32::from_rgb(236, 239, 243),
    border: Color32::from_rgb(211, 216, 223),
    widget: Color32::WHITE,
    hover: Color32::from_rgb(232, 243, 252),
    active: Color32::from_rgb(210, 231, 249),
    text: Color32::from_rgb(35, 38, 42),
    text_strong: Color32::from_rgb(16, 18, 20),
    text_muted: Color32::from_rgb(108, 116, 126),
};

/// 按深色/浅色取色板
pub fn palette(dark: bool) -> Palette {
    if dark {
        DARK
    } else {
        LIGHT
    }
}

/// 当前界面对应的危险色（用于错误提示文字）
pub fn danger(visuals: &egui::Visuals) -> Color32 {
    palette(visuals.dark_mode).danger
}

// ---------------- 装配 egui Style ----------------

/// 按上下文当前主题重新装配样式（保留深色/浅色的既有选择）
pub fn install(ctx: &egui::Context) {
    let dark = ctx.style().visuals.dark_mode;
    apply(ctx, dark);
}

/// 应用指定主题
pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_style(style(dark));
}

/// 构造主题样式
pub fn style(dark: bool) -> egui::Style {
    let pal = palette(dark);
    let mut style = if dark {
        egui::Style { visuals: egui::Visuals::dark(), ..Default::default() }
    } else {
        egui::Style { visuals: egui::Visuals::light(), ..Default::default() }
    };

    // 字号层级
    style.text_styles.insert(TextStyle::Heading, FontId::new(text::HEADING, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Body, FontId::new(text::BODY, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Button, FontId::new(text::BODY, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Small, FontId::new(text::TINY, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Monospace, FontId::new(text::MONO, FontFamily::Monospace));

    // 间距节奏
    style.spacing.item_spacing = Vec2::new(spacing::SM, spacing::XS + 2.0);
    style.spacing.button_padding = Vec2::new(10.0, spacing::XS);
    style.spacing.window_margin = Margin::same(spacing::MD);
    style.spacing.menu_margin = Margin::same(spacing::SM);
    style.spacing.indent = 18.0;
    style.spacing.interact_size = Vec2::new(40.0, 22.0);
    style.spacing.slider_width = 100.0;
    style.spacing.combo_width = 110.0;
    style.spacing.text_edit_width = 200.0;

    // 面板与浮动层
    let v = &mut style.visuals;
    v.panel_fill = pal.panel;
    v.window_fill = pal.window;
    v.extreme_bg_color = pal.sunken;
    v.faint_bg_color = pal.faint;
    v.window_stroke = Stroke::new(1.0_f32, pal.border);
    v.window_rounding = Rounding::same(radius::LARGE);
    v.menu_rounding = Rounding::same(radius::MEDIUM);
    let shadow_alpha = if dark { 70 } else { 36 };
    v.window_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 6.0),
        blur: 18.0,
        spread: 0.0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 3.0),
        blur: 10.0,
        spread: 0.0,
        color: Color32::from_black_alpha(shadow_alpha),
    };

    // 语义色
    v.hyperlink_color = pal.accent;
    v.warn_fg_color = pal.warning;
    v.error_fg_color = pal.danger;
    v.text_cursor.stroke = Stroke::new(1.5_f32, pal.accent);
    v.selection = egui::style::Selection {
        bg_fill: with_alpha(pal.accent, if dark { 90 } else { 60 }),
        stroke: Stroke::new(1.0_f32, pal.accent),
    };

    // 控件四态：常规 → 悬停（主色描边）→ 按下（主色填充）
    let rounding = Rounding::same(radius::SMALL);
    let widgets = &mut v.widgets;
    widgets.noninteractive = egui::style::WidgetVisuals {
        bg_fill: pal.window,
        weak_bg_fill: pal.window,
        bg_stroke: Stroke::new(1.0_f32, pal.border),
        rounding,
        fg_stroke: Stroke::new(1.0_f32, pal.text),
        expansion: 0.0,
    };
    widgets.inactive = egui::style::WidgetVisuals {
        bg_fill: pal.widget,
        weak_bg_fill: pal.widget,
        bg_stroke: Stroke::new(1.0_f32, pal.border),
        rounding,
        fg_stroke: Stroke::new(1.0_f32, pal.text),
        expansion: 0.0,
    };
    widgets.hovered = egui::style::WidgetVisuals {
        bg_fill: pal.hover,
        weak_bg_fill: pal.hover,
        bg_stroke: Stroke::new(1.0_f32, pal.accent),
        rounding,
        fg_stroke: Stroke::new(1.0_f32, pal.text_strong),
        expansion: 1.0,
    };
    widgets.active = egui::style::WidgetVisuals {
        bg_fill: pal.active,
        weak_bg_fill: pal.active,
        bg_stroke: Stroke::new(1.0_f32, pal.accent),
        rounding,
        fg_stroke: Stroke::new(1.0_f32, pal.text_strong),
        expansion: 0.0,
    };
    widgets.open = widgets.hovered;

    style
}

/// 给颜色叠加不透明度
fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}
