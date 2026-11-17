# ScreenCapture - Rust 截图软件设计文档

## 1. 项目概述

### 1.1 项目目标
使用 Rust 语言开发一款轻量、高性能的 Windows 桌面截图软件，支持区域截图、全屏截图、窗口截图、图片标注、剪贴板操作等核心功能。

### 1.2 设计原则
- **性能优先**：利用 Rust 内存安全和零成本抽象特性，实现流畅的截图体验
- **轻量级**：单文件可执行程序，无外部运行时依赖
- **用户友好**：类 Snipaste/ShareX 的交互体验
- **可扩展性**：模块化架构，便于后续功能扩展

### 1.3 目标平台
- Windows 10/11 (x86_64)，最低 Windows 10 1809
- macOS 11+ (Apple Silicon / Intel)

> macOS 支持以代码实现 + 交叉编译检查为准（`cargo check --target aarch64-apple-darwin`），
> 屏幕录制权限、菜单栏托盘与全局热键尚未在真机运行时验证。

---

## 2. 技术选型

| 功能模块 | 技术方案 | 说明 |
|---------|---------|------|
| GUI 框架 | `eframe` / `egui` | 即时模式 GUI，跨平台、简洁高效，适合绘图标注 |
| 截图捕获（Windows） | `windows` crate + GDI API | 直接调用 Windows API，性能最优 |
| 截图捕获（macOS） | `xcap`（CoreGraphics） | 跨显示器拼接、窗口捕获与枚举 |
| 屏幕/窗口枚举（Windows） | `windows` crate (EnumDisplayMonitors, EnumWindows) | 原生 API 调用 |
| 全局热键（Windows） | `windows` crate (RegisterHotKey) | 与托盘共用后台消息线程，便于冲突检测 |
| 全局热键（macOS） | `global-hotkey` | 主线程注册，事件回调唤醒 UI |
| 系统托盘（Windows） | `windows` crate (Shell_NotifyIconW) | 隐藏窗口 + 原生右键菜单 |
| 系统托盘（macOS） | `tray-icon` + `muda` | 菜单栏状态项 + 原生菜单 |
| 图片处理 | `image` crate | 图片加载、保存、格式转换 |
| 剪贴板 | `arboard` | 跨平台剪贴板操作 |
| 文字烘焙 | `ab_glyph` | 标注文字写入像素 |
| 图标绘制 | `egui` 内置绘制 + 自定义纹理 | 箭头、矩形、文字、画笔等 |
| 序列化 | `serde` + `toml` | 配置文件读写 |
| 时间 | `chrono` | 跨平台本地时间（文件名模板） |
| 打包分发 | `scripts/package.ps1` / `scripts/package-macos.sh` | 便携 zip / .app（可选 dmg） |

### 2.1 依赖版本锁定（Cargo.toml 预览）

```toml
[dependencies]
# 只声明 eframe；egui 统一通过 `eframe::egui` 使用，避免 eframe / egui 版本分叉
eframe = "0.29"
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "bmp"] }
ab_glyph = "0.2"
arboard = "3.4"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
chrono = { version = "0.4", default-features = false, features = ["clock"] }
anyhow = "1"
parking_lot = "0.12"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_Shell",
    "Win32_System_Threading",
    "Win32_System_LibraryLoader",
    "Win32_System_Registry",
] }

[target.'cfg(target_os = "macos")'.dependencies]
xcap = "0.4"
tray-icon = "0.19"
global-hotkey = "0.6"
```

---

## 3. 系统架构

### 3.1 架构图

```
┌─────────────────────────────────────────────────────┐
│                   应用入口 main.rs                    │
└──────────────────────┬──────────────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  配置管理    │  │  热键服务    │  │  系统托盘    │
│  config/    │  │  hotkey/     │  │  tray/      │
└─────────────┘  └─────────────┘  └─────────────┘
        │              │              │
        └──────────────┼──────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│                  截图服务 capture/                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ 全屏截图  │  │ 区域截图  │  │ 窗口截图  │          │
│  └──────────┘  └──────────┘  └──────────┘          │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│                  编辑器 editor/                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ 画笔工具  │  │ 形状工具  │  │ 文字工具  │          │
│  └──────────┘  └──────────┘  └──────────┘          │
│  ┌──────────┐  ┌──────────┐                        │
│  │ 马赛克    │  │ 取色/序号 │                        │
│  └──────────┘  └──────────┘                        │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│                  输出服务 output/                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ 保存文件  │  │ 复制剪贴板│  │ 分享链接  │          │
│  └──────────┘  └──────────┘  └──────────┘          │
└─────────────────────────────────────────────────────┘
```

### 3.2 模块划分

| 模块路径 | 职责 | 关键类型 |
|---------|------|---------|
| `src/main.rs` | 应用入口，初始化各服务 | `App` |
| `src/config/` | 配置加载、保存、热更新 | `Config`, `HotkeyConfig` |
| `src/capture/` | 截图门面 + 平台实现 | `Capturer`, `CapturedImage`, `capture::windows` / `capture::macos` |
| `src/editor/` | 截图编辑器、标注工具 | `EditorState`, `EditorCanvas`, `Annotation` |
| `src/pin/` | 固定到屏幕的置顶悬浮窗 | `PinWindow` |
| `src/output/` | 文件保存、剪贴板、文件名模板 | `OutputService`, `SaveOptions` |
| `src/hotkey/` | 快捷键字符串解析 + 平台键码映射 | `Combo`, `Key`, `hotkey::windows` / `hotkey::macos` |
| `src/tray/` | 托盘图标 / 菜单、全局热键注册、通知 | `TrayService`, `TrayAction`, `HotkeySpec` |
| `src/utils/` | 通用工具函数 | `Rect`, `Point`, `Result<T>` |

---

## 4. 核心功能设计

### 4.1 截图模式

#### 4.1.1 全屏截图
- 调用 `BitBlt` 捕获所有显示器内容
- 多显示器支持：按虚拟屏幕坐标拼接

#### 4.1.2 区域截图
- 半透明黑色遮罩覆盖全屏
- 鼠标拖拽选择矩形区域
- 实时显示选区尺寸（放大镜辅助定位）
- 支持方向键微调选区边界

#### 4.1.3 窗口截图
- 枚举所有可见顶层窗口
- 高亮悬停窗口
- 点击捕获指定窗口（含透明窗口支持）

### 4.2 标注工具集

| 工具 | 功能描述 | 实现方式 |
|------|---------|---------|
| 矩形框 | 绘制矩形边框/填充 | `egui` 预览 + 软件光栅化描边/填充 |
| 椭圆 | 绘制圆形/椭圆 | 椭圆折线采样 + 像素距离场填充 |
| 箭头 | 绘制带箭头直线 | 粗线段 + 实心三角箭头 |
| 画笔 | 自由手绘 | 拖拽采样折线 + 圆头线段拼接 |
| 文字 | 添加文字标注 | `egui` 画布预览；导出用 `ab_glyph` 烘焙系统字体 |
| 马赛克 | 区域打码 | 按块求均值（显示用 NEAREST 纹理，导出写回像素） |
| 取色器 | 像素颜色拾取 | 直接读取底图 BGRA 像素 |
| 序号标记 | 1, 2, 3... 序号圆 | 圆形背景 + 白色数字，自动递增 |

#### 4.2.1 工具属性
- 颜色选择器（8 个预设 + egui 取色盘 + HEX 显示）
- 线条粗细（1px ~ 24px）
- 字体大小（文字工具，10 ~ 72）
- 撤销/重做栈（无上限，新标注清空重做栈）
- 画布缩放（5% ~ 1600%，滚轮以光标为锚点）与平移（中键/选择工具拖拽）

### 4.3 全局热键（默认配置）

| 功能 | 默认热键 | 可自定义 |
|------|---------|---------|
| 区域截图 | `Ctrl + Shift + A` | ✓ |
| 全屏截图 | `Ctrl + Shift + F` | ✓ |
| 窗口截图 | `Ctrl + Shift + W` | ✓ |

实现要点：
- 使用 Win32 `RegisterHotKey`，注册到托盘线程的隐藏窗口（不依赖 `global-hotkey`）
- 字符串解析支持 `Ctrl/Shift/Alt/Win` + 字母 / 数字 / `F1–F24` / 方向键 / 空格 / PrintScreen 等
- 冲突检测：系统占用（错误码 1409）与同批次重复组合均在「设置」中提示
- 配置持久化到 `config.toml` 的 `[hotkeys]`，保存后立即重新注册

### 4.4 输出选项

- **保存到文件**：保存目录 + 文件名模板（`{date}` `{time}` `{datetime}` `{unix}` `{index}`）
- **图片格式**：PNG / JPG / BMP，JPG 支持 1–100 质量
- **重名处理**：自动追加 `_2`、`_3`，不覆盖已有文件
- **复制到剪贴板**：截图后自动复制（可关），手动保存时同时复制（可关）
- **自动保存**：点击编辑器「完成」时按配置保存
- **延迟截图**：0/1/3/5/10 秒倒计时，状态机驱动，不阻塞 UI

### 4.5 固定到屏幕

- **置顶悬浮窗**：无边框、`AlwaysOnTop`、显示合成后的截图
- **移动**：拖拽窗口内容移动（`ViewportCommand::OuterPosition`）
- **关闭**：双击或 Esc
- **性能**：静态内容以 80ms 低频轮询，只响应必要输入

### 4.6 系统托盘

- 左键单击/菜单：显示并聚焦主窗口
- 菜单：全屏截图 / 区域截图 / 窗口截图 / 显示主窗口 / 退出
- 托盘通知：保存、复制、截图失败时提示
- 开机自启：Windows 写入 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`；macOS 写入 `~/Library/LaunchAgents/com.screencapture.app.plist`
- 线程模型：
  - Windows：托盘图标与全局热键运行在独立后台消息线程，通过 `mpsc` + `request_repaint` 与 UI 通信
  - macOS：菜单栏图标与热键必须在主线程创建，事件通过全局回调写入队列 + `request_repaint` 唤醒 UI

---

## 5. 数据结构设计

### 5.1 核心结构体

```rust
// 捕获的图像
struct CapturedImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,  // BGRA 格式
    capture_time: DateTime<Local>,
    source: CaptureSource,
}

enum CaptureSource {
    FullScreen,
    Region(Rect),
    Window(HWND),
}

// 标注项
struct Annotation {
    id: u64,
    tool: ToolType,
    color: Color32,
    thickness: f32,
    data: AnnotationData,
}

enum AnnotationData {
    Rect { rect: Rect, filled: bool },
    Ellipse { rect: Rect, filled: bool },
    Arrow { points: Vec<Pos2> },
    Brush { points: Vec<Pos2> },
    Text { pos: Pos2, content: String, font_size: f32 },
    Blur { rect: Rect, radius: u32 },
    Number { pos: Pos2, value: u32 },
}

// 应用配置
struct Config {
    save_path: PathBuf,
    auto_save: bool,
    copy_to_clipboard: bool,
    file_format: String,
    file_name_template: String,
    jpeg_quality: u8,
    delay_seconds: u32,
    hotkeys: HotkeyConfig,
    theme: Theme,
    show_tray: bool,
    capture_cursor: bool,
}
```

### 5.2 状态管理

```rust
enum AppState {
    Idle,           // 待机/托盘状态
    Capturing,      // 区域选择中
    Editing,        // 编辑器打开
    Settings,       // 设置窗口
    Floating(PinnedImage),  // 固定悬浮窗
}
```

---

## 6. 关键实现细节

### 6.1 高性能截图

```rust
// 使用 Windows GDI API 截图
fn capture_screen(area: Option<Rect>) -> Result<CapturedImage> {
    // 1. 获取屏幕 DC
    let screen_dc = GetDC(None)?;
    // 2. 创建兼容 DC 和位图
    let mem_dc = CreateCompatibleDC(screen_dc)?;
    let bitmap = CreateCompatibleBitmap(screen_dc, width, height)?;
    // 3. BitBlt 拷贝
    BitBlt(mem_dc, 0, 0, width, height, screen_dc, x, y, SRCCOPY)?;
    // 4. GetDIBits 读取像素数据
    // 5. 清理资源
}
```

### 6.2 区域选择遮罩

- 创建无边框、置顶、半透明的全屏窗口
- 使用 `WS_EX_LAYERED` + `SetLayeredWindowAttributes` 实现半透明
- 鼠标事件追踪选区，实时渲染

### 6.3 放大镜实现

- 在选区光标旁显示 2x~4x 放大视图
- 每 100ms 刷新一次像素数据
- 显示精确坐标和 RGB 值

### 6.4 多显示器支持

```rust
struct MonitorInfo {
    handle: HMONITOR,
    rect: Rect,        // 相对主显示器坐标
    is_primary: bool,
}

fn enumerate_monitors() -> Vec<MonitorInfo> {
    EnumDisplayMonitors(None, None, monitor_enum_proc, 0);
}
```

---

## 7. UI 设计

### 7.1 界面布局

```
┌─────────────────────────────────────────┐
│  编辑器工具栏 (顶部)                      │
│  [画笔] [矩形] [椭圆] [箭头] [文字] [...] │
├─────────────────────────────────────────┤
│                                         │
│              画布区域                    │
│           (截图内容 + 标注)              │
│                                         │
├─────────────────────────────────────────┤
│  属性面板 (右侧/底部)                    │
│  颜色: [■■■■■■]  粗细: [====]            │
│  [撤销] [重做]                          │
└─────────────────────────────────────────┘
```

### 7.2 主题
- 浅色/深色主题自动跟随系统
- 支持手动切换

---

## 8. 配置文件

### 8.1 位置
- Windows：`%APPDATA%\ScreenCapture\config.toml`
- macOS：`~/Library/Application Support/ScreenCapture/config.toml`

### 8.2 格式示例

```toml
[general]
save_path = "C:\\Users\\xxx\\Pictures\\ScreenCapture"
auto_save = false
copy_to_clipboard = true
file_format = "png"
file_name_template = "Screenshot_{date}_{time}"
jpeg_quality = 92
delay_seconds = 3
capture_cursor = false

[hotkeys]
region = "Ctrl+Shift+A"
fullscreen = "Ctrl+Shift+F"
window = "Ctrl+Shift+W"

[editor]
default_color = "#FF0000"
default_thickness = 3.0
show_magnifier = true
```

---

## 9. 安全与性能

- **内存安全**：Rust 所有权系统防止内存泄漏和悬垂指针
- **GDI 资源管理**：使用 RAII 封装确保资源及时释放
- **异常处理**：所有 FFI 调用检查返回值，错误统一通过 `anyhow::Result` 传播
- **性能目标**：截图操作 < 100ms，编辑器启动 < 500ms

---

## 10. 未来扩展

- [ ] OCR 文字识别（集成 Tesseract）
- [ ] 滚动截图（网页/长窗口）
- [ ] 录屏功能（GIF/MP4）
- [ ] 云端同步截图历史
- [ ] 插件系统
- [ ] Linux/macOS 端口
