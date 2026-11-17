# ScreenCapture

一款使用 **Rust** 编写的高性能、轻量级桌面截图软件，支持 **Windows 10/11** 与 **macOS 11+**。

灵感来自 Snipaste / ShareX，目标是打造更轻、更快、更安全的替代品。

> macOS 版本已完成代码实现与交叉编译检查（`cargo check --target aarch64-apple-darwin`），
> 屏幕录制权限、菜单栏托盘与全局热键尚待在 Mac 真机上验证，详见[用户手册](docs/user-guide.md)。

## ✨ 功能特性

- 🖥️ **多种截图模式** — 全屏截图、区域截图、窗口截图
- ✂️ **区域选择** — 半透明遮罩、实时放大镜、键盘微调
- 🎨 **丰富标注工具** — 画笔、矩形、椭圆、箭头、文字、序号、马赛克、取色器
- ↩️ **撤销/重做** — Ctrl+Z / Ctrl+Y，标注实时预览
- 🔍 **画布缩放** — 滚轮以光标为锚点缩放，中键拖拽平移
- ⌨️ **全局热键** — 可自定义快捷键，冲突检测
- 📋 **灵活输出** — 保存文件（PNG/JPG/BMP）、复制剪贴板、文件名模板
- ⏱️ **延迟截图** — 0/1/3/5/10 秒倒计时
- 📌 **固定到屏幕** — 置顶悬浮窗，拖拽移动，双击/Esc 关闭
- 🎯 **系统托盘** — Windows 托盘图标 / macOS 菜单栏图标，菜单一键截图
- 🚀 **开机自启** — Windows 注册表 Run 键 / macOS LaunchAgent
- ⚡ **高性能** — Rust + 原生捕获 API，单屏截图 < 100ms
- 🧠 **内存安全** — 编译期保证，无 GC 开销

## 🛠️ 技术栈

| 类别 | 选择 |
|------|------|
| 语言 | Rust 2021 Edition |
| GUI | eframe 0.29（egui 经 `eframe::egui` 使用，单一版本） |
| 截图（Windows） | `windows` crate + GDI API |
| 截图（macOS） | `xcap`（CoreGraphics） |
| 热键（Windows） | Win32 `RegisterHotKey`（独立消息线程） |
| 热键（macOS） | `global-hotkey`（主线程注册） |
| 托盘（Windows / macOS） | `Shell_NotifyIconW` / `tray-icon` |
| 图片 | `image` crate |
| 文字烘焙 | `ab_glyph` |
| 剪贴板 | `arboard` |
| 配置 | `serde` + `toml` |
| 时间 | `chrono` |

## 🚀 快速开始

### 环境要求

- **Windows**：Windows 10 1809+ / Windows 11，[Rust](https://rustup.rs) stable，Visual Studio C++ Build Tools（MSVC）
- **macOS**：macOS 11+，[Rust](https://rustup.rs) stable，Xcode Command Line Tools（`xcode-select --install`）

### 构建

```bash
# 开发构建
cargo build

# Release 优化构建（LTO + strip）
cargo build --release

# 运行
cargo run
```

macOS 首次截图时需在「系统设置 → 隐私与安全性 → 屏幕录制」中授予权限。

Release 构建体积约 **5-8 MB**（静态链接 + strip）。

## 📁 项目结构

```
screencapture/
├── docs/
│   ├── design.md        # 软件设计文档
│   ├── progress.md      # 开发进度追踪
│   └── user-guide.md    # 用户手册
├── scripts/
│   ├── package.ps1      # Windows 便携包
│   ├── package-macos.sh # macOS .app / zip（可选 dmg）
│   └── make_icon.py     # 生成 assets 中的应用图标
├── assets/
│   ├── icon.png         # 1024px 主图标
│   ├── icon-256.png     # 窗口图标
│   ├── icon.ico         # Windows（16–256 多尺寸）
│   └── icon.icns        # macOS（32–1024 多尺寸）
├── src/
│   ├── main.rs          # 应用入口
│   ├── config/          # 配置加载/保存、开机自启
│   ├── capture/         # 截图门面 + windows.rs / macos.rs / gdi.rs
│   ├── picker/          # 区域选择覆盖层
│   ├── editor/          # 标注编辑器（状态 / 画布 / 像素合成）
│   ├── pin/             # 固定到屏幕（置顶悬浮窗）
│   ├── output/          # 保存/剪贴板/文件名模板
│   ├── hotkey/          # 热键解析 + 平台键码映射
│   ├── tray/            # 托盘 + 热键注册（windows.rs / macos.rs）
│   └── utils/           # 工具函数
├── Cargo.toml
├── build.rs             # Windows 版本资源嵌入（作者 / 版本 / 版权）
├── rustfmt.toml
└── .gitignore
```

## ⌨️ 默认热键

| 功能 | 全局热键 | 应用内 |
|------|---------|--------|
| 区域截图 | `Ctrl+Shift+A` | `F2` |
| 全屏截图 | `Ctrl+Shift+F` | `F1` |
| 窗口截图 | `Ctrl+Shift+W` | `F3` |

全局热键可在主界面「设置」中修改（格式如 `Ctrl+Shift+A`、`F2`），保存后立即生效并写入配置文件；被其他程序占用时会给出提示。

## 💾 保存与输出

| 设置项 | 说明 |
|--------|------|
| 保存目录 | 截图保存位置，目录不存在会自动创建 |
| 文件名模板 | 默认 `Screenshot_{date}_{time}`，支持 `{date}` `{time}` `{datetime}` `{unix}` `{index}` |
| 图片格式 | PNG / JPG / BMP（JPG 可调 1–100 质量） |
| 延迟截图 | 工具栏「延迟截图」菜单，延时秒数在设置中选择（0/1/3/5/10 秒） |
| 自动保存 | 开启后点击编辑器「完成」自动保存 |
| 自动复制 | 截图后立即复制到剪贴板；保存时同时复制 |

重名文件会自动追加 `_2`、`_3`，不会覆盖已有截图。

## 🎯 托盘操作

| 操作 | Windows | macOS |
|------|---------|-------|
| 左键单击图标 | 显示并聚焦主窗口 | 打开菜单栏菜单 |
| 右键 / 菜单 | 全屏 / 区域 / 窗口 / 显示主窗口 / 退出 | 同左 |
| 通知 | 气泡提示 | 系统通知（osascript） |

> 关闭主窗口即退出程序；Windows 11 可能把新托盘图标折叠到「隐藏的图标」溢出区。

## 📝 配置文件

- Windows：`%APPDATA%\ScreenCapture\config.toml`
- macOS：`~/Library/Application Support/ScreenCapture/config.toml`

```toml
[general]
save_path = "D:\\Pictures\\ScreenCapture"   # macOS 默认 ~/Pictures/ScreenCapture
auto_save = false
copy_to_clipboard = true
file_format = "png"
capture_cursor = false
file_name_template = "Screenshot_{date}_{time}"
jpeg_quality = 92
delay_seconds = 3

[hotkeys]
region = "Ctrl+Shift+A"
fullscreen = "Ctrl+Shift+F"
window = "Ctrl+Shift+W"
```

## 🛠️ 构建、测试与打包

```bash
cargo build --release        # Windows: target\release\screencapture.exe
                             # macOS:   target/release/screencapture
cargo test                   # 47 个测试
cargo clippy --all-targets   # 静态检查
```

**Windows 便携包**：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
# 生成 dist\ScreenCapture-0.8.0-win64.zip（exe + 文档）
```

发布包为免安装便携版：解压后运行 `ScreenCapture.exe`。也可在解压目录执行 `install.ps1` 安装到 `%LOCALAPPDATA%\Programs\ScreenCapture` 并创建开始菜单快捷方式；卸载使用同目录的 `uninstall.ps1`。

**macOS 应用包**：

```bash
bash scripts/package-macos.sh
# 生成 dist/ScreenCapture.app 与 dist/ScreenCapture-0.8.0-macos-<arch>.zip
# 安装 create-dmg 后同时生成 dmg：brew install create-dmg
```

产物未签名，首次打开需右键 →「打开」，或执行 `xattr -dr com.apple.quarantine ScreenCapture.app`。

CI 配置见 `.github/workflows/ci.yml`（Windows + macOS 双平台 fmt / clippy / test / release）。

## 🗺️ 开发路线图

见 [progress.md](docs/progress.md)。

当前版本：**v0.8.0 — 跨平台支持（macOS）**

详细操作说明见 [用户手册](docs/user-guide.md)。

## 🖌️ 编辑器操作

| 操作 | 说明 |
|------|------|
| 工具快捷键 | `V` 选择、`P` 画笔、`R` 矩形、`E` 椭圆、`A` 箭头、`T` 文字、`N` 序号、`M` 马赛克、`C` 取色 |
| 绘制 | 在画布上拖拽；矩形/椭圆/马赛克为框选，文字点击后弹窗输入（Ctrl+Enter 确定） |
| `Ctrl`+`Z` / `Ctrl`+`Y` | 撤销 / 重做（`Ctrl`+`Shift`+`Z` 也可重做） |
| 滚轮 | 以光标为锚点缩放画布 |
| 中键拖拽 / 「选择」工具拖拽 | 平移画布 |
| 保存 PNG / 复制 | 自动把标注合成进图片 |

## 🔍 选区模式操作

| 操作 | 说明 |
|------|------|
| 拖拽 | 划出矩形选区 |
| `Enter` / 双击 | 确认并裁剪 |
| `Esc` | 取消 |
| 方向键 / `Shift`+方向键 | 移动选区 1px / 10px |
| `Ctrl`+方向键 | 调整选区尺寸 |
| `Ctrl`+`A` | 全选整屏 |

## ⌨️ 应用内快捷键

| 功能 | 快捷键 |
|------|--------|
| 全屏截图 | `F1` |
| 区域截图 | `F2` |
| 窗口截图 | `F3` |

## 👤 开发者

| 项目 | 信息 |
|------|------|
| 作者 | 风不休 |
| 邮箱 | [dreign@qq.com](mailto:dreign@qq.com) |
| 网站 | [https://hefeng.chat/](https://hefeng.chat/) |

Windows 产物已嵌入版本资源，右键 `ScreenCapture.exe` →「属性 → 详细信息」可查看作者、版本与版权信息；macOS 应用包的 `Info.plist` 同样包含版权与网站信息。

## 📄 License

MIT License · © 2026 风不休
