// ScreenCapture - 高性能跨平台桌面截图软件（Windows / macOS）
// 主入口

// M2 阶段部分数据结构尚未全部接线，先抑制未使用告警
#![allow(dead_code)]

mod capture;
mod config;
mod editor;
mod hotkey;
mod output;
mod picker;
mod pin;
mod tray;
mod utils;

use eframe::egui;

use capture::{CapturedImage, Capturer, MonitorInfo, Rect, WindowInfo};
use config::Config;
use editor::{EditorAction, EditorCanvas, EditorState};
use output::{ImageFormat, OutputService};
use picker::{Picker, PickerOutcome};
use pin::PinWindow;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tray::{CaptureKind, HotkeySpec, TrayAction, TrayService};

fn main() -> eframe::Result<()> {
    // 必须在创建任何窗口前启用 DPI 感知，否则高 DPI 下坐标会错位
    capture::init_dpi_awareness();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([960.0, 720.0])
        .with_min_inner_size([640.0, 480.0])
        .with_title("ScreenCapture");
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions { viewport, ..Default::default() };

    eframe::run_native("ScreenCapture", native_options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

/// 窗口 / 任务栏图标（打包时已生成，编译期嵌入）
fn load_window_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../assets/icon-256.png")).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData { rgba: image.into_raw(), width, height })
}

/// 一次待执行的截图动作（避免在 UI 闭包内直接持有可变借用）
#[derive(Debug, Clone, Copy)]
enum CaptureAction {
    Fullscreen,
    /// 打开交互式选区覆盖层
    Picker,
    /// 按输入框坐标精确截图
    Coordinates,
    Window,
}

/// 加载系统中文字体，否则 egui 默认字体会把中文渲染成方块
#[cfg(windows)]
const CJK_FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\msyh.ttc",   // 微软雅黑
    "C:\\Windows\\Fonts\\msyhl.ttc",  // 微软雅黑 Light
    "C:\\Windows\\Fonts\\simhei.ttf", // 黑体
    "C:\\Windows\\Fonts\\Deng.ttf",   // 等线
    "C:\\Windows\\Fonts\\simsun.ttc", // 宋体
];

/// 加载系统中文字体，否则 egui 默认字体会把中文渲染成方块
#[cfg(target_os = "macos")]
const CJK_FONT_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",                   // 苹方
    "/System/Library/Fonts/STHeiti Medium.ttc",             // 华文黑体
    "/System/Library/Fonts/Hiragino Sans GB.ttc",           // 冬青黑体简体
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf", // Arial Unicode
];

#[cfg(not(any(windows, target_os = "macos")))]
const CJK_FONT_CANDIDATES: &[&str] = &[];

fn install_cjk_font(ctx: &egui::Context) {
    for path in CJK_FONT_CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert("cjk".to_owned(), egui::FontData::from_owned(bytes));
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            list.insert(0, "cjk".to_owned());
        }
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            list.push("cjk".to_owned());
        }
        ctx.set_fonts(fonts);
        return;
    }
}

/// 一次编辑会话：原始底图 + 标注状态 + 画布视图
struct EditorSession {
    image: CapturedImage,
    state: EditorState,
    canvas: EditorCanvas,
}

/// 截图时把自身主窗口临时移出屏幕，返回后再恢复原位置
fn capture_avoiding_self<T>(f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    let own = capture::find_own_main_window("ScreenCapture");
    let saved = own.and_then(capture::park_window_offscreen);
    if saved.is_some() {
        // 等待 DWM 完成一帧合成，避免旧位置残影被截入
        std::thread::sleep(Duration::from_millis(45));
    }

    let result = f();

    if let (Some(hwnd), Some((x, y))) = (own, saved) {
        capture::restore_window_position(hwnd, x, y);
    }
    result
}

/// 把界面上的热键字符串解析为注册请求；返回（请求列表, 解析错误）
fn build_hotkey_specs(inputs: &[String; 3]) -> (Vec<HotkeySpec>, Vec<String>) {
    let mut specs = Vec::new();
    let mut errors = Vec::new();

    for (index, name) in [(0usize, "区域截图"), (1, "全屏截图"), (2, "窗口截图")] {
        match hotkey::parse(&inputs[index]) {
            Ok(combo) => specs.push(match index {
                0 => HotkeySpec::region(combo),
                1 => HotkeySpec::fullscreen(combo),
                _ => HotkeySpec::window(combo),
            }),
            Err(err) => errors.push(format!("{name}热键「{}」无效：{err}", inputs[index])),
        }
    }

    (specs, errors)
}

struct App {
    editor: Option<EditorSession>,
    status: String,
    is_error: bool,
    monitors: Vec<MonitorInfo>,
    windows: Vec<WindowInfo>,
    selected_window: Option<usize>,
    // 区域截图参数
    region_x: i32,
    region_y: i32,
    region_w: u32,
    region_h: u32,
    // 最近一次截图耗时（用于性能统计）
    last_elapsed: Duration,
    // 交互式选区覆盖层
    picker: Option<Picker>,
    // 配置 / 托盘 / 全局热键
    config: Config,
    save_path_input: String,
    tray: Option<TrayService>,
    settings_open: bool,
    hotkey_inputs: [String; 3],
    hotkey_status: Vec<String>,
    settings_status: Vec<String>,
    autostart: bool,
    // 固定到屏幕的悬浮窗
    pins: Vec<PinWindow>,
    next_pin_id: u64,
    // 延迟截图倒计时
    pending_capture: Option<(CaptureAction, Instant)>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_cjk_font(&cc.egui_ctx);

        let monitors = capture::enumerate_monitors();
        let (region_x, region_y, region_w, region_h) = monitors
            .iter()
            .find(|m| m.is_primary)
            .or_else(|| monitors.first())
            .map(|m| (m.rect.x, m.rect.y, m.rect.width, m.rect.height))
            .unwrap_or((0, 0, 800, 600));

        let config = Config::load().unwrap_or_default();
        let autostart = config::autostart_enabled();
        let save_path_input = config.general.save_path.display().to_string();
        let hotkey_inputs =
            [config.hotkeys.region.clone(), config.hotkeys.fullscreen.clone(), config.hotkeys.window.clone()];

        let mut status = "就绪 · 选择一种截图方式".to_string();
        let mut is_error = false;
        let mut settings_status = Vec::new();

        let tray = match TrayService::new(&cc.egui_ctx) {
            Ok(service) => {
                let (specs, errors) = build_hotkey_specs(&hotkey_inputs);
                settings_status = errors;
                if !specs.is_empty() {
                    service.register_hotkeys(specs);
                }
                Some(service)
            }
            Err(err) => {
                status = format!("托盘服务启动失败：{err:#}");
                is_error = true;
                settings_status.push(format!("托盘服务启动失败：{err:#}"));
                None
            }
        };

        Self {
            editor: None,
            status,
            is_error,
            monitors,
            windows: Vec::new(),
            selected_window: None,
            region_x,
            region_y,
            region_w,
            region_h,
            last_elapsed: Duration::ZERO,
            picker: None,
            config,
            save_path_input,
            tray,
            settings_open: false,
            hotkey_inputs,
            hotkey_status: Vec::new(),
            settings_status,
            autostart,
            pins: Vec::new(),
            next_pin_id: 1,
            pending_capture: None,
        }
    }

    fn refresh_monitors(&mut self) {
        self.monitors = capture::enumerate_monitors();
    }

    fn refresh_windows(&mut self) {
        match Capturer::enumerate_windows() {
            Ok(mut list) => {
                self.status = format!("已枚举 {} 个可见窗口", list.len());
                self.is_error = false;
                // 下拉列表按标题排序，便于查找（枚举本身保持 z 序供选区层使用）
                list.sort_by_key(|w| w.title.to_lowercase());
                self.windows = list;
                self.selected_window = None;
            }
            Err(err) => self.set_error(format!("枚举窗口失败：{err:#}")),
        }
    }

    fn set_error(&mut self, message: String) {
        self.status = message;
        self.is_error = true;
    }

    /// 通过托盘弹出气泡通知
    fn notify(&self, body: &str, error: bool) {
        if let Some(tray) = &self.tray {
            tray.notify("ScreenCapture", body, error);
        }
    }

    /// 保存配置并重新注册热键
    fn apply_settings(&mut self) {
        let (specs, errors) = build_hotkey_specs(&self.hotkey_inputs);
        // 解析错误保留在配置状态里；热键注册结果由托盘线程异步回填
        self.hotkey_status.clear();
        self.settings_status = errors;

        if let Some(tray) = &self.tray {
            tray.register_hotkeys(specs);
        } else {
            self.settings_status.push("托盘服务未启动，无法注册全局热键".to_string());
        }

        self.config.hotkeys.region = self.hotkey_inputs[0].clone();
        self.config.hotkeys.fullscreen = self.hotkey_inputs[1].clone();
        self.config.hotkeys.window = self.hotkey_inputs[2].clone();
        if self.save_path_input.trim().is_empty() {
            self.settings_status.push("保存目录不能为空，已保留原路径".to_string());
        } else {
            self.config.general.save_path = PathBuf::from(self.save_path_input.trim());
        }

        let mut has_error = false;
        match self.config.save() {
            Ok(()) => self.settings_status.push("配置已保存".to_string()),
            Err(err) => {
                has_error = true;
                self.settings_status.push(format!("保存配置失败：{err:#}"));
            }
        }
        match config::set_autostart(self.autostart) {
            Ok(()) => self.settings_status.push(if self.autostart {
                "已开启开机自启动".to_string()
            } else {
                "已关闭开机自启动".to_string()
            }),
            Err(err) => {
                has_error = true;
                self.settings_status.push(format!("设置开机自启失败：{err:#}"));
            }
        }

        if has_error {
            self.status = "设置保存失败，详见「设置」窗口".to_string();
            self.is_error = true;
        } else {
            self.status = "设置已保存，热键已重新注册".to_string();
            self.is_error = false;
        }
    }

    /// 设置窗口：全局热键 + 开机自启
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }

        let mut open = true;
        let mut apply = false;

        egui::Window::new("设置").open(&mut open).resizable(false).default_width(470.0).show(ctx, |ui| {
            ui.heading("保存与输出");
            egui::Grid::new("save_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("保存目录");
                ui.add(egui::TextEdit::singleline(&mut self.save_path_input).desired_width(300.0));
                ui.end_row();

                ui.label("文件名模板");
                ui.add(egui::TextEdit::singleline(&mut self.config.general.file_name_template).desired_width(300.0));
                ui.end_row();

                ui.label("图片格式");
                egui::ComboBox::from_id_salt("format_combo")
                    .selected_text(self.config.general.file_format.to_uppercase())
                    .show_ui(ui, |ui| {
                        for value in ["png", "jpg", "bmp"] {
                            ui.selectable_value(
                                &mut self.config.general.file_format,
                                value.to_string(),
                                value.to_uppercase(),
                            );
                        }
                    });
                ui.end_row();

                ui.label("JPEG 质量");
                let is_jpeg = self.config.general.file_format.eq_ignore_ascii_case("jpg")
                    || self.config.general.file_format.eq_ignore_ascii_case("jpeg");
                ui.add_enabled(is_jpeg, egui::Slider::new(&mut self.config.general.jpeg_quality, 1..=100));
                ui.end_row();

                ui.label("延迟截图");
                egui::ComboBox::from_id_salt("delay_combo")
                    .selected_text(format!("{} 秒", self.config.general.delay_seconds))
                    .show_ui(ui, |ui| {
                        for seconds in [0u32, 1, 3, 5, 10] {
                            ui.selectable_value(
                                &mut self.config.general.delay_seconds,
                                seconds,
                                format!("{seconds} 秒"),
                            );
                        }
                    });
                ui.end_row();
            });
            ui.label("模板变量：{date} {time} {datetime} {unix} {index}");
            ui.checkbox(&mut self.config.general.auto_save, "点击「完成」时自动保存");
            ui.checkbox(&mut self.config.general.copy_to_clipboard, "截图后自动复制，保存时同时复制");

            ui.separator();
            ui.heading("全局热键");
            ui.label("格式：修饰键+主键，例如 Ctrl+Shift+A、F2（留空则不注册）");
            ui.add_space(6.0);

            egui::Grid::new("hotkey_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("区域截图");
                ui.add(egui::TextEdit::singleline(&mut self.hotkey_inputs[0]).desired_width(200.0));
                ui.end_row();

                ui.label("全屏截图");
                ui.add(egui::TextEdit::singleline(&mut self.hotkey_inputs[1]).desired_width(200.0));
                ui.end_row();

                ui.label("窗口截图");
                ui.add(egui::TextEdit::singleline(&mut self.hotkey_inputs[2]).desired_width(200.0));
                ui.end_row();
            });

            ui.separator();
            ui.checkbox(&mut self.autostart, "开机自动启动");
            ui.label(format!("配置路径：{}", Config::config_path().display()));

            ui.separator();
            ui.heading("关于");
            ui.label("作者：和风");
            ui.label("邮箱：dreign@qq.com");
            ui.horizontal(|ui| {
                ui.label("网站：");
                ui.hyperlink("https://hefeng.chat/");
            });
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("保存并应用").clicked() {
                    apply = true;
                }
                let light = !ui.visuals().dark_mode;
                if ui.button(if light { "浅色主题" } else { "深色主题" }).clicked() {
                    ui.ctx().set_visuals(if light { egui::Visuals::dark() } else { egui::Visuals::light() });
                }
            });

            if !self.hotkey_status.is_empty() {
                ui.separator();
                ui.label("热键状态");
                for line in &self.hotkey_status {
                    let color = if line.contains("失败") || line.contains("无效") || line.contains("占用") {
                        egui::Color32::from_rgb(220, 100, 80)
                    } else {
                        ui.style().visuals.text_color()
                    };
                    ui.colored_label(color, line);
                }
            }

            if !self.settings_status.is_empty() {
                ui.separator();
                for line in &self.settings_status {
                    let color = if line.contains("失败") {
                        egui::Color32::from_rgb(220, 100, 80)
                    } else {
                        ui.style().visuals.text_color()
                    };
                    ui.colored_label(color, line);
                }
            }
        });

        self.settings_open = open;
        if apply {
            self.apply_settings();
        }
    }

    /// 处理托盘 / 全局热键事件
    fn handle_tray_action(&mut self, ctx: &egui::Context, action: TrayAction) {
        match action {
            TrayAction::Capture(kind) => {
                let capture = match kind {
                    CaptureKind::Fullscreen => CaptureAction::Fullscreen,
                    CaptureKind::Region => CaptureAction::Picker,
                    CaptureKind::Window => CaptureAction::Window,
                };
                self.execute(ctx, capture);
            }
            TrayAction::ShowMainWindow => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            TrayAction::Quit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            TrayAction::HotkeysRegistered(results) => {
                self.hotkey_status.clear();
                let mut failed = false;
                for (name, result) in results {
                    match result {
                        Ok(()) => self.hotkey_status.push(format!("{name}热键已注册")),
                        Err(err) => {
                            failed = true;
                            self.hotkey_status.push(format!("{name}热键注册失败：{err}"));
                        }
                    }
                }
                if failed {
                    self.set_error("部分全局热键注册失败，可在「设置」中修改".to_string());
                } else {
                    self.status = "全局热键已生效".to_string();
                    self.is_error = false;
                }
            }
        }
    }

    /// 打开交互式选区：先把自己移出屏幕外截整屏作为背景，再弹出全屏覆盖层
    fn start_region_picker(&mut self, ctx: &egui::Context) {
        let result = capture_avoiding_self(|| {
            let image = Capturer::capture_fullscreen()?;
            // 背景时刻的可见窗口（z 序），供悬停高亮使用
            let windows = Capturer::enumerate_windows().unwrap_or_default();
            Ok((image, windows))
        });

        match result {
            Ok((image, windows)) => {
                self.picker = Some(Picker::new(ctx, image, windows));
                self.status = "选区模式 · 拖拽选择区域，Enter 确认，Esc 取消".to_string();
                self.is_error = false;
            }
            Err(err) => self.set_error(format!("选区背景截图失败：{err:#}")),
        }
    }

    fn execute(&mut self, ctx: &egui::Context, action: CaptureAction) {
        let result = match action {
            // 全屏截图时把自身窗口移出屏幕，避免把自己截进画面
            CaptureAction::Fullscreen => capture_avoiding_self(Capturer::capture_fullscreen),
            CaptureAction::Picker => {
                self.start_region_picker(ctx);
                return;
            }
            CaptureAction::Coordinates => {
                Capturer::capture_region(Rect::new(self.region_x, self.region_y, self.region_w, self.region_h))
            }
            CaptureAction::Window => match self.selected_window {
                Some(index) => match self.windows.get(index) {
                    Some(info) => Capturer::capture_window(info.handle()),
                    None => {
                        self.set_error("请先选择一个窗口".to_string());
                        return;
                    }
                },
                None => {
                    self.set_error("请先选择一个窗口".to_string());
                    return;
                }
            },
        };

        match result {
            Ok(image) => self.open_editor(ctx, image),
            Err(err) => {
                self.set_error(format!("截图失败：{err:#}"));
                self.notify("截图失败", true);
            }
        }
    }

    /// 截图成功后进入编辑器
    fn open_editor(&mut self, ctx: &egui::Context, image: CapturedImage) {
        let source = match image.source {
            capture::CaptureSource::FullScreen => "全屏".to_string(),
            capture::CaptureSource::Region(r) => {
                format!("区域 {}+{},{}+{}", r.x, r.y, r.width, r.height)
            }
            capture::CaptureSource::Window(_) => "窗口".to_string(),
        };

        self.last_elapsed = image.elapsed;
        self.status = format!(
            "编辑中 · {} · {}x{} · 截图耗时 {:.1} ms",
            source,
            image.width,
            image.height,
            image.elapsed.as_secs_f64() * 1000.0
        );
        self.is_error = false;
        let canvas = EditorCanvas::new(ctx, &image);
        // 配置开启时，截图后立即复制一份原始画面
        if self.config.general.copy_to_clipboard {
            let _ = OutputService::copy_to_clipboard(&image.pixels, image.width, image.height);
        }
        self.editor = Some(EditorSession { image, state: EditorState::default(), canvas });
    }

    /// 合成标注后的图像
    fn flatten_editor(&self) -> Option<anyhow::Result<CapturedImage>> {
        self.editor.as_ref().map(|session| editor::render::flatten(&session.image, &session.state.annotations))
    }

    /// 按配置目录 / 模板 / 格式保存，返回是否成功
    fn save_editor(&mut self) -> bool {
        let Some(result) = self.flatten_editor() else {
            self.set_error("没有可保存的截图".to_string());
            return false;
        };
        let image = match result {
            Ok(image) => image,
            Err(err) => {
                self.set_error(format!("合成标注失败：{err:#}"));
                return false;
            }
        };

        let format = ImageFormat::from_ext(&self.config.general.file_format).unwrap_or(ImageFormat::Png);
        let dir = self.config.general.save_path.clone();
        let template = self.config.general.file_name_template.clone();
        let quality = self.config.general.jpeg_quality;

        match OutputService::save_with_template(
            &image.pixels,
            image.width,
            image.height,
            &dir,
            &template,
            format,
            quality,
        ) {
            Ok(path) => {
                if self.config.general.copy_to_clipboard
                    && OutputService::copy_to_clipboard(&image.pixels, image.width, image.height).is_ok()
                {
                    self.status = format!("已保存并复制：{}", path.display());
                } else {
                    self.status = format!("已保存：{}", path.display());
                }
                self.is_error = false;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                self.notify(&format!("已保存 {name}"), false);
                true
            }
            Err(err) => {
                self.set_error(format!("保存失败：{err:#}"));
                self.notify("保存失败", true);
                false
            }
        }
    }

    fn copy_editor(&mut self) {
        let Some(result) = self.flatten_editor() else {
            self.set_error("没有可复制的截图".to_string());
            return;
        };
        let result =
            result.and_then(|image| OutputService::copy_to_clipboard(&image.pixels, image.width, image.height));
        match result {
            Ok(()) => {
                self.status = "已复制到剪贴板（含标注）".to_string();
                self.is_error = false;
                self.notify("已复制到剪贴板", false);
            }
            Err(err) => {
                self.set_error(format!("复制失败：{err:#}"));
                self.notify("复制失败", true);
            }
        }
    }

    fn close_editor(&mut self) {
        self.editor = None;
        self.status = "已退出编辑".to_string();
        self.is_error = false;
    }

    /// 开始延迟截图倒计时
    fn start_delayed(&mut self, action: CaptureAction) {
        let seconds = self.config.general.delay_seconds;
        self.pending_capture = Some((action, Instant::now() + Duration::from_secs(seconds as u64)));
        if seconds == 0 {
            self.status = "正在截图…".to_string();
        } else {
            self.status = format!("{seconds} 秒后截图…");
        }
        self.is_error = false;
    }

    fn handle_editor_action(&mut self, ctx: &egui::Context, action: EditorAction) {
        match action {
            EditorAction::Save => {
                self.save_editor();
            }
            EditorAction::Copy => self.copy_editor(),
            EditorAction::Pin => {
                let Some(result) = self.flatten_editor() else {
                    self.set_error("没有可固定的截图".to_string());
                    return;
                };
                match result {
                    Ok(image) => {
                        let pin = PinWindow::new(ctx, &image, self.next_pin_id);
                        self.next_pin_id += 1;
                        self.pins.push(pin);
                        self.status = "已固定到屏幕 · 拖拽移动，双击/Esc 关闭".to_string();
                        self.is_error = false;
                    }
                    Err(err) => self.set_error(format!("固定失败：{err:#}")),
                }
            }
            EditorAction::Close => {
                if self.config.general.auto_save && !self.save_editor() {
                    return;
                }
                self.close_editor();
            }
            EditorAction::ClearAll => {
                if let Some(session) = &mut self.editor {
                    session.state.clear();
                    self.status = "已清除全部标注".to_string();
                    self.is_error = false;
                }
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<CaptureAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            ui.heading("ScreenCapture");
            ui.separator();

            if ui.button("全屏截图 (F1)").clicked() {
                action = Some(CaptureAction::Fullscreen);
            }
            if ui.button("选区截图 (F2)").clicked() {
                action = Some(CaptureAction::Picker);
            }
            ui.menu_button("延迟截图", |ui| {
                ui.label(format!("{} 秒后执行", self.config.general.delay_seconds));
                ui.separator();
                for (label, delayed) in [
                    ("全屏截图", CaptureAction::Fullscreen),
                    ("区域截图", CaptureAction::Picker),
                    ("窗口截图", CaptureAction::Window),
                ] {
                    if ui.button(label).clicked() {
                        self.start_delayed(delayed);
                        ui.close_menu();
                    }
                }
            });

            ui.separator();
            ui.label("坐标：");
            ui.add(egui::DragValue::new(&mut self.region_x).prefix("x=").speed(1.0));
            ui.add(egui::DragValue::new(&mut self.region_y).prefix("y=").speed(1.0));
            ui.add(egui::DragValue::new(&mut self.region_w).prefix("w=").speed(1.0).range(1..=20000));
            ui.add(egui::DragValue::new(&mut self.region_h).prefix("h=").speed(1.0).range(1..=20000));
            if ui.button("按坐标截取").clicked() {
                action = Some(CaptureAction::Coordinates);
            }
        });

        ui.horizontal(|ui| {
            if ui.button("刷新窗口列表").clicked() {
                self.refresh_windows();
            }

            let selected_text =
                self.selected_window.and_then(|i| self.windows.get(i)).map(|w| w.title.clone()).unwrap_or_else(|| {
                    if self.windows.is_empty() {
                        "（列表为空，请先刷新）".to_string()
                    } else {
                        "（选择窗口）".to_string()
                    }
                });

            egui::ComboBox::from_id_salt("window_picker").selected_text(selected_text).width(360.0).show_ui(ui, |ui| {
                for (index, info) in self.windows.iter().enumerate() {
                    let label = format!("{} — {}x{}", info.title, info.rect.width, info.rect.height);
                    ui.selectable_value(&mut self.selected_window, Some(index), label);
                }
            });

            if ui.button("截取窗口").clicked() {
                action = Some(CaptureAction::Window);
            }

            ui.separator();
            if ui.button("保存").clicked() {
                self.save_editor();
            }
            if ui.button("复制到剪贴板").clicked() {
                self.copy_editor();
            }
            ui.separator();
            if ui.button("设置").clicked() {
                self.settings_open = true;
            }
        });

        ui.horizontal(|ui| {
            if ui.button("刷新显示器信息").clicked() {
                self.refresh_monitors();
            }
            let tray_text = if self.tray.is_some() { "托盘：已启用" } else { "托盘：未启用" };
            ui.label(tray_text);
            ui.separator();
            let monitor_text = if self.monitors.is_empty() {
                "未检测到显示器".to_string()
            } else {
                self.monitors
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        format!(
                            "#{}{} {}+{},{}x{}",
                            i + 1,
                            if m.is_primary { "(主)" } else { "" },
                            m.rect.x,
                            m.rect.y,
                            m.rect.width,
                            m.rect.height
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("  |  ")
            };
            ui.label(monitor_text);
        });

        action
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut action = None;

        // ---- 延迟截图倒计时 ----
        if let Some((pending, deadline)) = self.pending_capture {
            let now = Instant::now();
            if now >= deadline {
                self.pending_capture = None;
                self.execute(ctx, pending);
            } else {
                let remain = (deadline - now).as_secs_f32();
                self.status = format!("{remain:.1} 秒后截图…");
                self.is_error = false;
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }

        // ---- 托盘 / 全局热键事件 ----
        let mut tray_actions = Vec::new();
        if let Some(tray) = &self.tray {
            while let Some(tray_action) = tray.try_recv() {
                tray_actions.push(tray_action);
            }
        }
        if !tray_actions.is_empty() {
            // 保证事件到达时界面能及时刷新（窗口最小化时也能被唤醒）
            ctx.request_repaint();
        }

        // 本地快捷键（全局热键由托盘线程注册）
        ctx.input(|input| {
            if input.key_pressed(egui::Key::F1) {
                action = Some(CaptureAction::Fullscreen);
            } else if input.key_pressed(egui::Key::F2) {
                action = Some(CaptureAction::Picker);
            } else if input.key_pressed(egui::Key::F3) {
                action = Some(CaptureAction::Window);
            }
        });

        // ---- 编辑器快捷键（文字输入框获得焦点时不拦截） ----
        if self.editor.is_some() && !ctx.wants_keyboard_input() {
            let (ctrl, shift) = ctx.input(|i| (i.modifiers.ctrl, i.modifiers.shift));
            let (undo, redo, tool) = ctx.input(|i| {
                let tool = [
                    (egui::Key::V, editor::ToolType::Select),
                    (egui::Key::P, editor::ToolType::Pen),
                    (egui::Key::R, editor::ToolType::Rectangle),
                    (egui::Key::E, editor::ToolType::Ellipse),
                    (egui::Key::A, editor::ToolType::Arrow),
                    (egui::Key::T, editor::ToolType::Text),
                    (egui::Key::N, editor::ToolType::Number),
                    (egui::Key::M, editor::ToolType::Mosaic),
                    (egui::Key::C, editor::ToolType::Eyedropper),
                ]
                .into_iter()
                .find(|(key, _)| i.key_pressed(*key))
                .map(|(_, tool)| tool);
                (i.key_pressed(egui::Key::Z), i.key_pressed(egui::Key::Y), tool)
            });

            if let Some(session) = &mut self.editor {
                if ctrl && undo {
                    if shift {
                        session.state.redo();
                    } else {
                        session.state.undo();
                    }
                }
                if ctrl && redo {
                    session.state.redo();
                }
                if !ctrl {
                    if let Some(tool) = tool {
                        session.state.current_tool = tool;
                    }
                }
            }
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            if let Some(from_toolbar) = self.toolbar(ui) {
                action = Some(from_toolbar);
            }
            ui.add_space(4.0);
        });

        // ---- 编辑器工具栏 ----
        let mut editor_action = None;
        if self.editor.is_some() {
            egui::TopBottomPanel::top("editor_toolbar").show(ctx, |ui| {
                ui.add_space(4.0);
                if let Some(session) = &mut self.editor {
                    editor_action = editor::draw_toolbar(ui, &mut session.state);
                }
                ui.add_space(4.0);
            });
        }

        let status = self.status.clone();
        let is_error = self.is_error;
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let color =
                    if is_error { egui::Color32::from_rgb(220, 80, 80) } else { ui.style().visuals.text_color() };
                ui.colored_label(color, status);
            });
        });

        let mut canvas_message = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(session) = &mut self.editor {
                let EditorSession { image, state, canvas } = session;
                canvas_message = canvas.show(ui, state, image).message;
            } else {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.heading("ScreenCapture 已就绪");
                        ui.add_space(8.0);
                        ui.label("· 保存：使用设置中的目录 / 文件名模板 / PNG·JPG·BMP 格式，自动避免重名");
                        ui.label("· 模板变量：{date} {time} {datetime} {unix} {index}");
                        ui.label("· 延迟截图：工具栏「延迟截图」菜单，延时秒数在设置中调整");
                        ui.label("· 固定到屏幕：截图后在编辑器点「固定」，拖拽移动，双击/Esc 关闭");
                        ui.add_space(12.0);
                        ui.label("本地快捷键：F1 全屏 / F2 选区 / F3 窗口；全局热键与托盘保持可用");
                        ui.label("截图后自动进入编辑器，Ctrl+Z 撤销、Ctrl+Y 重做");
                    });
                });
            }
        });
        if let Some(message) = canvas_message {
            self.status = message;
            self.is_error = false;
        }

        if let Some(action) = action {
            self.execute(ctx, action);
        }
        if let Some(editor_action) = editor_action {
            self.handle_editor_action(ctx, editor_action);
        }

        // ---- 托盘 / 全局热键事件 ----
        for tray_action in tray_actions {
            self.handle_tray_action(ctx, tray_action);
        }

        // ---- 设置窗口 ----
        self.settings_window(ctx);

        // ---- 固定到屏幕的悬浮窗 ----
        if !self.pins.is_empty() {
            // 静态内容无需满帧重绘，低频轮询即可响应拖拽与关闭
            ctx.request_repaint_after(Duration::from_millis(80));
            let mut closed = Vec::new();
            for (index, pin) in self.pins.iter_mut().enumerate() {
                if !pin.show(ctx) {
                    closed.push(index);
                }
            }
            for index in closed.into_iter().rev() {
                self.pins.remove(index);
            }
        }

        // ---- 选区覆盖层：每帧驱动，直到确认 / 取消 ----
        if self.picker.is_some() {
            // 拖拽与放大镜需要连续重绘
            ctx.request_repaint();

            let mut picker = self.picker.take().expect("picker 已判定存在");
            match picker.show(ctx) {
                PickerOutcome::None => self.picker = Some(picker),
                PickerOutcome::Cancel => {
                    ctx.send_viewport_cmd_to(Picker::viewport_id(), egui::ViewportCommand::Close);
                    self.status = "已取消选区".to_string();
                    self.is_error = false;
                }
                PickerOutcome::Confirm(rect) => {
                    ctx.send_viewport_cmd_to(Picker::viewport_id(), egui::ViewportCommand::Close);
                    match picker.crop(rect) {
                        Ok(image) => self.open_editor(ctx, image),
                        Err(err) => self.set_error(format!("裁剪选区失败：{err:#}")),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use editor::{Annotation, AnnotationData, ToolType};

    #[test]
    fn capture_annotate_export_pipeline() {
        // 3x2 黑底（BGRA）
        let base = CapturedImage {
            width: 3,
            height: 2,
            pixels: [0u8, 0, 0, 255].repeat(6),
            source: capture::CaptureSource::FullScreen,
            elapsed: Duration::ZERO,
        };

        let annotation = Annotation {
            id: 1,
            tool: ToolType::Rectangle,
            color: egui::Color32::from_rgb(255, 0, 0),
            thickness: 1.0,
            data: AnnotationData::Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0, filled: true },
        };

        let composed = editor::render::flatten(&base, &[annotation]).expect("标注合成应成功");

        let dir = std::env::temp_dir().join("screencapture_pipeline_test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = OutputService::save_with_template(
            &composed.pixels,
            composed.width,
            composed.height,
            &dir,
            "pipeline_{index}",
            ImageFormat::Png,
            92,
        )
        .expect("按模板保存应成功");
        assert!(path.to_string_lossy().contains("pipeline_000"), "文件名应包含序号：{}", path.display());

        let decoded = image::open(&path).expect("应能解码导出文件").to_rgba8();
        assert_eq!(decoded.dimensions(), (3, 2));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255], "标注区域应为红色");
        assert_eq!(decoded.get_pixel(2, 1).0, [0, 0, 0, 255], "未标注区域应保持黑色");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 性能基准：`cargo test --release -- --ignored --nocapture`
    #[test]
    #[ignore = "性能基准，手动运行：cargo test --release -- --ignored --nocapture"]
    fn performance_snapshot() {
        use std::time::Instant;

        let (width, height) = (2560u32, 1600u32);
        let base = CapturedImage {
            width,
            height,
            pixels: [30u8, 30, 30, 255].repeat((width * height) as usize),
            source: capture::CaptureSource::FullScreen,
            elapsed: Duration::ZERO,
        };

        // 200 条混合标注
        let mut annotations = Vec::new();
        for i in 0..200u32 {
            let x = (i % 20) as f32 * 120.0 + 20.0;
            let y = (i / 20) as f32 * 140.0 + 20.0;
            let data = match i % 4 {
                0 => AnnotationData::Rect { x, y, w: 100.0, h: 80.0, filled: false },
                1 => AnnotationData::Ellipse { x, y, w: 100.0, h: 80.0, filled: true },
                2 => AnnotationData::Arrow { start: (x, y), end: (x + 100.0, y + 60.0) },
                _ => AnnotationData::Text { x, y, content: "性能基准 Text".to_string(), font_size: 24.0 },
            };
            annotations.push(Annotation {
                id: i as u64 + 1,
                tool: data.tool(),
                color: egui::Color32::from_rgb(255, 60, 60),
                thickness: 3.0,
                data,
            });
        }

        let start = Instant::now();
        let composed = editor::render::flatten(&base, &annotations).expect("标注合成应成功");
        let flatten_ms = start.elapsed().as_secs_f64() * 1000.0;

        let mosaic = Annotation {
            id: 999,
            tool: ToolType::Mosaic,
            color: egui::Color32::WHITE,
            thickness: 2.0,
            data: AnnotationData::Mosaic { x: 200.0, y: 200.0, w: 800.0, h: 600.0, block: 16 },
        };
        let start = Instant::now();
        let _ = editor::render::flatten(&base, &[mosaic]).expect("马赛克合成应成功");
        let mosaic_ms = start.elapsed().as_secs_f64() * 1000.0;

        let dir = std::env::temp_dir().join("screencapture_bench");
        let _ = std::fs::remove_dir_all(&dir);

        let start = Instant::now();
        let png_path = OutputService::save_with_template(
            &composed.pixels,
            composed.width,
            composed.height,
            &dir,
            "bench_{index}",
            ImageFormat::Png,
            92,
        )
        .expect("保存 PNG 应成功");
        let png_ms = start.elapsed().as_secs_f64() * 1000.0;

        let start = Instant::now();
        let jpg_path = OutputService::save_with_template(
            &composed.pixels,
            composed.width,
            composed.height,
            &dir,
            "bench_jpg_{index}",
            ImageFormat::Jpeg,
            92,
        )
        .expect("保存 JPG 应成功");
        let jpg_ms = start.elapsed().as_secs_f64() * 1000.0;

        let png_kb = std::fs::metadata(&png_path).map(|m| m.len() / 1024).unwrap_or(0);
        let jpg_kb = std::fs::metadata(&jpg_path).map(|m| m.len() / 1024).unwrap_or(0);

        println!("=== ScreenCapture 性能基准 2560x1600 ===");
        println!("标注合成（200 条混合标注）: {flatten_ms:.1} ms");
        println!("马赛克（800x600 / block 16）: {mosaic_ms:.1} ms");
        println!("保存 PNG: {png_ms:.1} ms（{png_kb} KB）");
        println!("保存 JPG: {jpg_ms:.1} ms（{jpg_kb} KB）");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(flatten_ms < 10_000.0, "合成耗时异常：{flatten_ms} ms");
    }
}
