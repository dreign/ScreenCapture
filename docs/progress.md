# ScreenCapture 开发进度

## 项目里程碑

| 阶段 | 状态 | 预估工期 | 完成日期 |
|------|------|---------|---------|
| M1 - 脚手架搭建 | ✅ 完成 | 0.5 天 | 2026-11-16 |
| M2 - 核心截图 | ✅ 完成 | 2 天 | 2026-11-17 |
| M3 - 区域选择 UI | ✅ 完成 | 2 天 | 2026-11-17 |
| M4 - 编辑器与标注 | ✅ 完成 | 4 天 | 2026-11-17 |
| M5 - 热键与托盘 | ✅ 完成 | 1.5 天 | 2026-11-17 |
| M6 - 输出与配置 | ✅ 完成 | 1.5 天 | 2026-11-17 |
| M7 - 测试与打包 | ✅ 完成 | 2 天 | 2026-11-17 |
| M8 - macOS 跨平台支持 | ✅ 完成（交叉编译验证） | 2 天 | 2026-11-17 |

**总计预估：约 15.5 个工作日**

---

## M1 - 脚手架搭建 ✅

**完成日期：2026-11-16 · Rust 工具链：1.100.0 stable**

### 任务清单

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 1.1 | 初始化 Cargo 项目 | ✅ 完成 | 手动搭建（无 cargo init，直接创建文件） |
| 1.2 | 编写 Cargo.toml 依赖 | ✅ 完成 | eframe/egui 0.27, windows 0.58, image 0.25 |
| 1.3 | 创建模块目录结构 | ✅ 完成 | config/capture/capture::gdi/editor/output/hotkey/tray/utils |
| 1.4 | 编写 README | ✅ 完成 | 中文说明 + 技术栈 + 快速开始 |
| 1.5 | .gitignore | ✅ 完成 | target/ Cargo.lock 等 |
| 1.6 | 配置 clippy + fmt | ✅ 完成 | rustfmt.toml (120宽 + 分模块排序) |
| 1.7 | 安装 Rust 工具链 | ✅ 完成 | rustup + stable 1.100.0 (x86_64-pc-windows-msvc) |
| 1.8 | cargo check 编译验证 | ✅ 完成 | 见下方编译日志 |

### 编译验证

```
$ cargo check
    Updating crates.io index
    Finished dev [unoptimized + debuginfo] target(s)
```

### M1 产出清单

```
ScreenCapture/
├── Cargo.toml              # 项目配置 + 依赖
├── .gitignore              # Git 忽略
├── rustfmt.toml            # 代码风格
├── README.md               # 项目说明
├── docs/
│   ├── design.md           # 软件设计文档
│   └── progress.md         # 开发进度（本文件）
└── src/
    ├── main.rs             # eframe 应用入口（占位 App）
    ├── config/mod.rs       # Config + serde 默认值
    ├── capture/mod.rs      # Capturer trait + Rect/Point 数据结构
    ├── capture/gdi.rs      # DeviceContext/GdiObject RAII 封装
    ├── editor/mod.rs       # ToolType + EditorState 撤销重做栈
    ├── output/mod.rs       # ImageFormat + OutputService
    ├── hotkey/mod.rs       # HotkeyManager + parse_combo TODO
    ├── tray/mod.rs         # TrayIcon 占位
    └── utils/mod.rs        # BGRA↔RGBA 转换 + clamp
```

---

## M2 - 核心截图 ✅

**完成日期：2026-11-17 · 环境：Windows 11 / 2560x1600（DPI 缩放 200%）**

### 任务清单

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 2.1 | GDI API 封装模块 | ✅ 完成 | `ScreenDc` / `MemDc` / `Bitmap` 全部 RAII，Drop 自动释放 |
| 2.2 | 全屏截图实现 | ✅ 完成 | BitBlt 虚拟屏幕 + GetDIBits(top-down, 32bpp) |
| 2.3 | 多显示器枚举 | ✅ 完成 | EnumDisplayMonitors + MONITORINFO，主屏标记 |
| 2.4 | 窗口截图实现 | ✅ 完成 | PrintWindow(PW_RENDERFULLCONTENT) 优先，失败回退 BitBlt |
| 2.5 | 窗口枚举 + 过滤 | ✅ 完成 | 排除最小化/隐藏/工具窗口/桌面外壳窗口 |
| 2.6 | 截图性能测试 | ✅ 完成 | 全屏 2560x1600 实测 32.8ms |
| 2.7 | 单元测试 | ✅ 完成 | 12 个测试全部通过（含真实 GDI 截图） |
| 2.8 | DPI 感知初始化 | ✅ 完成 | Per-Monitor V2，避免高 DPI 坐标错位 |
| 2.9 | UI 集成 + 预览 | ✅ 完成 | 工具栏按钮 + F1/F2/F3 快捷键 + 纹理预览 |
| 2.10 | 输出落地验证 | ✅ 完成 | 保存 PNG / 复制剪贴板（提前自 M6 实现，用于验证像素） |

### 验收结果

| 验收项 | 目标 | 实测 |
|--------|------|------|
| 全屏截图耗时 | < 100ms | ✅ 32.8ms（2560x1600，含 GetDIBits） |
| 高分辨率支持 | 2K/4K | ✅ 2560x1600 验证通过 |
| BGRA→RGBA 转换 | 正确 | ✅ 单元测试 + PNG 落盘校验 |
| 全流程截图→编码 | 可用 | ✅ PNG 文件 > 1KB，可正常打开 |

### 测试清单（`cargo test`，12 passed）

```
capture::tests::rect_empty_check
capture::tests::rect_from_win32_converts_size
capture::tests::rect_from_win32_rejects_degenerate
capture::tests::virtual_screen_is_valid
capture::tests::capture_small_region_returns_bgra_buffer
capture::tests::capture_fullscreen_matches_virtual_screen_and_saves_png   ← 端到端
capture::tests::enumerate_windows_and_capture_one                         ← 端到端
output::tests::filename_has_extension
output::tests::format_parsing
output::tests::save_png_roundtrip
utils::tests::test_bgra_to_rgba
utils::tests::test_clamp
```

### 实现要点

- **像素管线**：`BitBlt → 兼容位图 → GetDIBits(biHeight 取负实现 top-down) → Vec<u8>(BGRA) → to_rgba()`
- **窗口枚举回调**：`LPARAM` 传 `*mut Vec<WindowInfo>` 就地收集，无需全局状态
- **窗口截图**：`PrintWindow` 可捕获被遮挡窗口；部分 GPU 直绘窗口失败时自动回退屏幕 `BitBlt`
- **中文字体**：egui 默认字体不含 CJK，启动时加载 `msyh.ttc`（微软雅黑），失败依次回退 simhei/等线/宋体
- **DPI**：`SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` 必须在创建窗口前调用

### 已知限制（M3 解决）

1. 全屏截图会把自己窗口也截进去（缺少截图前隐藏窗口的时序处理）
2. 区域截图暂用坐标输入框，无鼠标拖拽选区
3. 窗口截图对 DirectComposition 全屏独占程序可能返回黑图
4. 无选区放大镜 / 键盘微调

### 新增/修改文件

```
src/capture/mod.rs    截图入口 + 显示器/窗口枚举 + DPI + 测试（约 420 行）
src/capture/gdi.rs    GDI RAII 封装 + grab_rect/grab_window 原语
src/output/mod.rs     实现 save_image(PNG/JPG/BMP) / copy_to_clipboard / save_to_temp
src/config/mod.rs     实现 Config::load/save（%APPDATA%\ScreenCapture\config.toml）
src/main.rs           工具栏 UI + F1/F2/F3 快捷键 + CJK 字体加载 + 预览渲染
Cargo.toml            新增 Win32_Graphics_Dwm / Win32_UI_HiDpi / Win32_Storage_Xps
```

### 踩坑记录

| 问题 | 原因 | 解决 |
|------|------|------|
| `PrintWindow` 找不到 | windows 0.58 把它归到 `Win32::Storage::Xps`（user32 导出） | 加 `Win32_Storage_Xps` feature |
| `MONITORINFOF_PRIMARY` 未导出 | 该常量在 0.58 的 Gdi 模块缺失 | 用字面量 `0x1` |
| `DragValue::range` 不存在 | egui 0.27 旧 API | 改用 `clamp_range(1..=20000)` |
| 中文显示为方块 | egui 默认字体无 CJK 字形 | 启动时加载系统字体并前置到字体族 |

---

## M3 - 区域选择 UI ✅

**完成日期：2026-11-17 · 新增 `src/picker/mod.rs`（约 520 行，含 4 个单元测试）**

### 任务清单

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 3.1 | 全屏遮罩窗口 | ✅ 完成 | `show_viewport_immediate` 创建无边框/置顶/不可缩放窗口，截图为背景 |
| 3.2 | 鼠标拖拽选区 | ✅ 完成 | `drag_started/dragged/drag_stopped` + `Rect::from_two_pos`，小于 4pt 视为误触 |
| 3.3 | 选区信息面板 | ✅ 完成 | 选区左上角胶囊标签显示 `宽 × 高`，实时刷新 |
| 3.4 | 放大镜 | ✅ 完成 | 光标周围 15×15 像素、8pt/像素，显示 `(x, y) #RRGGBB RGB(r,g,b)`，贴边自动翻转 |
| 3.5 | 键盘微调 | ✅ 完成 | 方向键 1px、Shift 10px；Ctrl+方向键改右下角；Ctrl+A 全选；Enter/双击确认；Esc 取消 |
| 3.6 | 窗口截图高亮 | ✅ 完成 | 使用截图时刻的 z 序窗口列表命中测试，红色边框 + 标题与尺寸标签 |
| 3.7 | ESC 取消逻辑 | ✅ 完成 | 关闭覆盖层并恢复主窗口，状态栏提示"已取消选区" |
| 3.8 | 确认后裁剪 | ✅ 完成 | 选区按比例映射到整屏图像像素，`CapturedImage::crop` 裁剪后进入主窗口预览 |
| 3.9 | 自身窗口排除 | ✅ 完成 | 截图前把主窗口移到屏幕外(-32000,-32000)，45ms 后截图再恢复，**解决 M2 遗留问题** |

### 交互细节

- 十字辅助线贯穿全屏，便于对齐
- 选区外四块半透明黑色压暗（alpha 120），选区保持原亮度
- 选区边框 1.5pt 蓝色 + 四角白色手柄
- 顶部胶囊提示条常驻：`拖拽选择 · Enter/双击 确认 · Esc 取消 · 方向键微调(Shift×10) · Ctrl+方向键 调尺寸 · Ctrl+A 全选`
- 光标统一为十字准星

### 坐标体系

| 层级 | 单位 | 说明 |
|------|------|------|
| 覆盖层本地 | pt | egui 逻辑像素，选区与指针交互都用它 |
| 整屏图像 | px | `selection_to_image_rect()` 按 `图像尺寸 / 窗口尺寸` 比例换算并钳制边界 |
| 屏幕物理 | px | 仅用于窗口命中测试（`窗口矩形 - 虚拟屏幕原点`） |

### 测试清单（`cargo test`，18 passed）

```
picker::tests::full_screen_selection_maps_to_whole_image
picker::tests::quarter_selection_maps_proportionally
picker::tests::selection_is_clamped_to_image_bounds
picker::tests::degenerate_selection_still_yields_one_pixel
capture::tests::crop_extracts_requested_region
capture::tests::crop_rejects_invalid_region
（其余 12 个为 M1/M2 既有测试）
```

### 验证情况

**已验证（真机 2560x1600 / 200% DPI）**
- 覆盖层正常弹出并覆盖全屏，顶部提示条、十字线、放大镜信息条均正确渲染
- 放大镜读数正确显示屏幕坐标与 HEX/RGB（样例 `(670, 295) #595958`）
- 背景图为截图时刻画面，**不再包含自身主窗口**（M2 遗留问题已修复）
- 确认/取消后进程稳定，无 panic（stderr 无输出）
- 选区→像素映射、裁剪边界全部由单元测试覆盖

**待人工确认**
- 鼠标拖拽划选与 Enter 确认的端到端手感：当前自动化环境存在其他全局截图工具/Windows「单击以执行」抢占鼠标拖拽，注入式拖拽无法可靠触发，需人工按 F2 手动试一次

### 已知限制

1. 多显示器混合 DPI 时，覆盖层按主屏缩放因子创建，跨屏选区可能有几像素偏差
2. 选区暂不支持拖拽角点缩放（可用 Ctrl+方向键替代）
3. 未做屏幕边缘吸附

### 新增/修改文件

```
src/picker/mod.rs     新增：选区覆盖层 UI（遮罩/拖拽/放大镜/窗口高亮/键盘/确认裁剪 + 单元测试）
src/capture/mod.rs    新增 CapturedImage::crop、find_own_main_window(标题优先)、park/restore_window_position
src/main.rs           新增 start_region_picker、capture_avoiding_self、Picker 帧驱动与结果处理
Cargo.toml            无新增依赖（复用 egui 立即视口能力）
```

### 踩坑记录

| 问题 | 原因 | 解决 |
|------|------|------|
| 背景图仍包含自身窗口 | `find_own_main_window` 命中了 glutin 的 `__wglDummyWindowFodder` 辅助窗 | 改为标题优先匹配 + 面积 ≥ 400×300 过滤 |
| 截图时隐藏窗口会导致主窗口不再重绘 | 隐藏窗口收不到 redraw，立即视口可能停更 | 改用"移出屏幕外"而非隐藏，窗口仍处于可见+可绘制状态 |
| 自动化拖拽无效 | `SetCursorPos` 不产生 `WM_MOUSEMOVE`；环境中其他工具抢占拖拽 | 仅影响自动化验证，不影响真实人工操作 |

---

## M4 - 编辑器与标注 ✅

**完成日期：2026-11-17 · 新增 `src/editor/canvas.rs`（约 640 行）与 `src/editor/render.rs`（约 520 行），重写 `src/editor/mod.rs`（约 340 行）**

### 任务清单

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 4.1 | 编辑器框架 | ✅ 完成 | 截图确认后直接进入内嵌编辑视图（工具栏 + 画布 + 状态栏） |
| 4.2 | 工具栏组件 | ✅ 完成 | 两行布局：工具/颜色；粗细/字号/撤销/输出/主题 |
| 4.3 | 画笔工具 | ✅ 完成 | 拖拽采样折线（≥1px 间距），圆头粗线段拼接 |
| 4.4 | 矩形/椭圆 | ✅ 完成 | 描边 + 可选填充 |
| 4.5 | 箭头工具 | ✅ 完成 | 线段 + 实心三角箭头，头部随粗细缩放 |
| 4.6 | 文字工具 | ✅ 完成 | 弹窗输入（多行、Ctrl+Enter 确定、Esc 取消），画布即时渲染 |
| 4.7 | 序号标记 | ✅ 完成 | 点击落点自动递增，撤销后回退 |
| 4.8 | 马赛克 | ✅ 完成 | 拖拽区域按块求均值，NEAREST 纹理保持块状（以像素块化替代高斯模糊） |
| 4.9 | 取色器 | ✅ 完成 | 点击底图取色并写回当前颜色，状态栏显示 HEX/RGB |
| 4.10 | 颜色选择器 | ✅ 完成 | 8 个预设色 + egui 取色盘 + HEX 显示 |
| 4.11 | 粗细/字号调节 | ✅ 完成 | 滑块 1–24 / 10–72，实时影响预览与导出 |
| 4.12 | 撤销/重做 | ✅ 完成 | 撤销栈 + 重做栈，新标注清空重做栈（未设上限） |
| 4.13 | 画布缩放/平移 | ✅ 完成 | 滚轮以光标为锚点缩放（5%–1600%），中键/「选择」工具拖拽平移 |
| 4.14 | 主题切换 | ✅ 完成 | 工具栏按钮切换 egui 浅色/深色视觉 |

### 验收结果

| 验收项 | 目标 | 实测 |
|--------|------|------|
| 所有工具绘制流畅 | 实时预览 | ✅ 真机验证画笔 / 矩形 / 文字 / 保存路径 |
| 撤销/重做无异常 | 状态一致 | ✅ 单元测试 + 按钮启用态正确 |
| 保存/复制含标注 | 像素合成 | ✅ 2560x1600 PNG 1.1MB，红色标注像素采样 600+ |
| 内存占用 < 200MB | 含大图 | ⚠ 未做长时间压测；全屏底图约 16MB + 纹理 |

### 实现要点

- **双通道渲染**：画布用 egui painter 矢量绘制保证交互清晰；保存/复制用 `render::flatten` 软件光栅化（1px 抗锯齿）烘焙进 BGRA 像素，两套几何参数完全一致
- **文字烘焙**：引入 `ab_glyph`，运行时加载系统字体（黑体 → 雅黑 → 等线 → 宋体），字体缺失时静默跳过（画布仍用 egui 字体显示）
- **马赛克**：显示用提交时刻的像素化纹理（块均值 + NEAREST 采样），导出时对当前像素缓冲重新分块求均值
- **坐标体系**：标注统一存底图像素坐标，画布按 `fit × zoom` 映射，缩放/平移不改变标注数据
- **序号自增**：`next_number()` 由现有序号标注数量推导，撤销后自动回退

### 测试清单（`cargo test`，28 passed）

```
editor::tests::commit_and_undo_redo
editor::tests::number_counter_increments_and_rewinds
editor::tests::annotation_tool_matches_data
editor::canvas::tests::screen_to_image_maps_proportionally
editor::canvas::tests::screen_to_image_clamps_outside
editor::canvas::tests::image_to_screen_roundtrip
editor::render::tests::mosaic_averages_each_block
editor::render::tests::filled_rect_covers_area_but_not_outside
editor::render::tests::stroke_rect_keeps_center_untouched
editor::render::tests::text_and_number_are_baked_into_pixels
（其余 18 个为 M1–M3 既有测试）
```

### 验证情况

**已验证（真机 2560x1600 / 200% DPI）**
- F1 全屏截图 → 自动进入编辑器，两行工具栏完整显示（已修复单行溢出）
- 画笔拖拽、矩形拖拽、文字弹窗录入均正常渲染
- 保存 PNG：2560x1600、约 1.1MB，随机采样红色标注像素 600+，确认标注已烘焙
- 光标坐标、缩放百分比、状态栏提示实时刷新
- 28 个测试全部通过，clippy 零告警

**待人工确认**
- 马赛克、椭圆、箭头、序号、取色器的端到端手感
- 4K 以上大图的缩放/平移帧率

### 已知限制

1. 马赛克纹理采样自底图，不叠加其之前的标注（导出时按顺序作用于当前像素，显示与导出存在细微差异）
2. 文字导出依赖系统字体，字体缺失时文字不会烘焙进图片
3. 「选择」工具目前仅用于拖拽平移，尚不支持选中/移动/删除单条标注
4. 撤销/重做栈无上限，大量标注时内存随标注数增长

### 新增/修改文件

```
src/editor/mod.rs      重写：工具枚举、标注数据、EditorState（提交/撤销/重做/清除）、工具栏、单元测试
src/editor/canvas.rs   新增：底图纹理、缩放/平移、各工具交互、文字弹窗、标注预览、马赛克纹理、坐标映射
src/editor/render.rs   新增：软件光栅化 flatten（线段/矩形/椭圆/箭头/画笔/文字/序号/马赛克 + 抗锯齿）
src/main.rs            预览改为内嵌编辑器（EditorSession），编辑器工具栏/快捷键/保存复制走合成图
Cargo.toml             新增 ab_glyph 0.2（文字烘焙）
```

### 踩坑记录

| 问题 | 原因 | 解决 |
|------|------|------|
| 工具栏单行溢出、「粗细」被截断 | 工具 + 颜色 + 两组滑块总宽超过 960pt | 拆成两行固定布局 |
| 文字无法随截图导出 | egui 无离屏渲染，无法把字体绘制进像素 | 引入 ab_glyph 直接对 BGRA 缓冲光栅化 |
| 马赛克显示与导出可能不一致 | 显示用纹理、导出用像素缓冲 | 两处均按同一块大小求均值 |
| 窗口失焦时首次点击只激活窗口 | Windows 焦点策略 | 仅影响自动化验证，人工使用无感知 |

---

## M5 - 热键与托盘 ✅

**完成日期：2026-11-17 · 新增 `src/tray/mod.rs`（约 430 行）与热键解析模块，集成设置窗口**

### 任务清单

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 5.1 | 全局热键注册 | ✅ 完成 | 自研 `RegisterHotKey`（独立消息线程），未引入 `global-hotkey` |
| 5.2 | 热键配置持久化 | ✅ 完成 | 读写 `%APPDATA%\ScreenCapture\config.toml` 的 `[hotkeys]` |
| 5.3 | 热键冲突检测 | ✅ 完成 | 系统占用（错误 1409）+ 同批次重复组合提示 |
| 5.4 | 系统托盘图标 | ✅ 完成 | `Shell_NotifyIconW` + 隐藏窗口 + 后台消息循环 |
| 5.5 | 托盘右键菜单 | ✅ 完成 | `CreatePopupMenu` / `TrackPopupMenu`：全屏 / 区域 / 窗口 / 显示主窗口 / 退出 |
| 5.6 | 托盘通知气泡 | ✅ 完成 | `NIM_MODIFY + NIF_INFO`，保存 / 复制 / 截图失败时提示 |
| 5.7 | 开机自启动（可选） | ✅ 完成 | 写入 / 删除 `HKCU\...\Run` 的 `ScreenCapture` 值 |

### 验收结果

| 验收项 | 目标 | 实测 |
|--------|------|------|
| 全局热键（应用失焦） | 可触发截图 | ✅ 焦点在 Edge 时 `Ctrl+Shift+A` 打开选区、`Ctrl+Shift+F` 全屏截图 |
| 热键冲突提示 | 明确提示 | ✅ 重复组合显示"与「区域截图」热键重复" |
| 配置持久化 | 重启保留 | ✅ 单元测试 + UI 保存后文件内容校验 |
| 托盘图标 / 菜单 | 常驻可操作 | ✅ 状态栏"托盘：已启用"；右键消息弹出原生菜单 |
| 气泡通知 | 成功 / 失败提示 | ✅ 逻辑接线（保存、复制、截图失败） |
| 开机自启 | 可开可关 | ✅ 注册表值创建后删除，无残留 |

### 实现要点

- **独立消息线程**：托盘与热键都依赖 Windows 消息循环，而 eframe 独占主线程；这里在后台线程创建隐藏窗口并 `GetMessage` 泵消息，UI 通过 `mpsc` + `egui::Context::request_repaint()` 通信
- **热键解析**：`Ctrl/Shift/Alt/Win + 字母/数字/F1–F24/特殊键`，非法输入在设置窗口直接提示
- **冲突检测**：`RegisterHotKey` 失败时把系统错误翻译成中文（1409 → "已被其他程序占用"）；同一批次的重复组合提前拦截，避免静默覆盖
- **生命周期**：`TrayService::Drop` 发送 Shutdown → `PostQuitMessage` → 线程删除托盘图标、注销热键并销毁隐藏窗口
- **开机自启**：`RegSetValueExW` 写入带引号的当前 exe 路径，关闭时 `RegDeleteValueW`

### 测试清单（`cargo test`，34 passed）

```
hotkey::tests::parses_modifiers_and_letter
hotkey::tests::parses_function_key_without_modifiers
hotkey::tests::parses_aliases_and_whitespace
hotkey::tests::rejects_invalid_input
config::tests::config_save_load_roundtrip
config::tests::missing_config_returns_default
（其余 28 个为 M1–M4 既有测试）
```

### 验证情况

**已验证（真机 2560x1600 / 200% DPI）**
- 应用失焦时全局热键仍可触发选区与全屏截图，并进入编辑器
- 设置窗口：热键编辑、保存应用、状态回显（注册成功 / 重复 / 保存失败）均正常
- 配置文件写入后内容正确；重启会读取配置
- 开机自启注册表值创建与删除均验证通过
- 托盘右键菜单可弹出，UI Automation 可见"全屏截图"等菜单项
- 34 个测试全部通过，clippy 零告警

**待人工确认**
- 托盘气泡在桌面右下角的实际显示效果
- 托盘菜单各项点击（全屏 / 区域 / 窗口 / 显示 / 退出）的手感

### 已知限制

1. 托盘图标使用系统默认应用图标，尚未制作专属图标
2. 关闭主窗口即退出程序（未实现"关闭到托盘"）；最小化后热键仍可用
3. 托盘菜单不显示当前热键文本
4. 开机自启写入当前 exe 路径，移动程序后需重新勾选
5. Windows 11 默认可能把新托盘图标折叠进"隐藏的图标"溢出区

### 新增/修改文件

```
src/hotkey/mod.rs   重写：快捷键字符串解析（修饰键 + VK）+ 4 个单元测试
src/tray/mod.rs     新增：托盘线程（隐藏窗口 / 图标 / 菜单 / 气泡 / 热键注册 / 冲突检测）
src/config/mod.rs   新增：开机自启读写、Config::save_to/load_from + 2 个单元测试
src/main.rs         集成：托盘与热键事件轮询、设置窗口、气泡通知、托盘状态显示
Cargo.toml          新增 feature：Win32_System_LibraryLoader / Win32_System_Registry / Win32_UI_Input_KeyboardAndMouse
```

### 踩坑记录

| 问题 | 原因 | 解决 |
|------|------|------|
| `RegisterHotKey` 找不到 | 它在 `Win32::UI::Input::KeyboardAndMouse`，不在 `WindowsAndMessaging` | 增加对应 feature 并调整导入 |
| 同一组合可注册两次 | 同进程内不同 id 时系统行为不稳定 | 注册前自行按（修饰键+VK）去重 |
| 测试环境写 `%APPDATA%` 被拒 | 文件写入受沙箱限制（非程序问题） | 单元测试写临时目录；UI 验证时重定向 APPDATA 到工作区 |
| 窗口未激活时 UIA 点击无效 | Windows 焦点策略 | 验证前先 `AppActivate` 激活窗口 |
| `FindWindow` 偶发找不到隐藏窗口 | 隐藏窗口检索时序 | 改用 `EnumWindows` 按进程枚举定位 |

---

## M6 - 输出与配置 ✅

**完成日期：2026-11-17 · 新增 `src/pin/mod.rs`，扩展输出、配置与设置窗口**

### 任务清单

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 6.1 | 文件保存（PNG/JPG/BMP） | ✅ 完成 | M2 已实现，M6 接入配置格式与 JPEG 质量 |
| 6.2 | 剪贴板复制 | ✅ 完成 | 手动复制 + 截图后自动复制（可关） |
| 6.3 | 文件名模板 | ✅ 完成 | `{date}` `{time}` `{datetime}` `{unix}` `{index}`，非法字符过滤、重名追加序号 |
| 6.4 | 延迟截图 | ✅ 完成 | 工具栏菜单 + 倒计时状态机（0/1/3/5/10 秒） |
| 6.5 | 固定到屏幕 | ✅ 完成 | 置顶无边框窗口，拖拽移动，双击/Esc 关闭 |
| 6.6 | 配置文件读写 | ✅ 完成 | 新增字段带 `serde(default)`，兼容旧配置 |
| 6.7 | 设置窗口 UI | ✅ 完成 | 保存目录/模板/格式/质量/延迟/自动保存/自动复制/热键/自启/主题 |
| 6.8 | 配置变更实时生效 | ✅ 完成 | 保存行为与延迟直接读取配置，保存后写盘 |

### 验收结果

| 验收项 | 目标 | 实测 |
|--------|------|------|
| 自定义目录 + 模板保存 | 文件名符合模板 | ✅ 生成 `M6_20261117_021318.png`（1.9MB） |
| 延迟截图 | 延时后自动截图 | ✅ 3 秒菜单触发后自动全屏截图并进入编辑器 |
| 固定到屏幕 | 置顶悬浮窗 | ✅ 出现「ScreenCapture 固定」窗口（visible），Esc 可关闭 |
| 旧配置兼容 | 缺字段可加载 | ✅ 新字段默认值 + 单元测试 |
| 设置项实时生效 | 无需重启 | ✅ 修改后立即影响保存与延迟 |

### 实现要点

- **文件名模板**：纯函数渲染 + 非法字符替换 + 空模板回退；目标文件重名时追加 `_2`、`_3`，避免覆盖
- **延迟截图**：`(CaptureAction, Instant)` 状态机，倒计时期间以 100ms 间隔重绘，不阻塞 UI
- **固定到屏幕**：egui 立即视口 + `with_always_on_top`，拖拽通过 `ViewportCommand::OuterPosition` 移动；静态内容用 80ms 低频轮询降低 CPU
- **配置兼容**：新增 `file_name_template` / `jpeg_quality` / `delay_seconds` 均带默认值，旧 `config.toml` 可直接加载

### 测试清单（`cargo test`，37 passed）

```
output::tests::template_renders_date_time_and_index
output::tests::template_sanitizes_invalid_filename_chars
output::tests::unique_path_avoids_overwrite
config::tests::config_save_load_roundtrip
config::tests::missing_config_returns_default
（其余 32 个为 M1–M5 既有测试）
```

### 验证情况

**已验证（真机 2560x1600 / 200% DPI）**
- 设置窗口布局完整（保存与输出 / 全局热键 / 自启 / 配置路径 / 主题）
- 修改保存目录与模板并应用后，配置文件正确写入
- 全屏截图 → 保存，文件按模板命名落在指定目录
- 延迟截图菜单可用，延时结束后自动截图
- 「固定」生成置顶窗口，Esc 关闭后窗口销毁
- 37 个测试通过，clippy 零告警

**待人工确认**
- 固定窗口拖拽手感与多显示器下的初始位置
- JPEG/BMP 格式保存与质量滑块效果

### 已知限制

1. 保存目录需手动输入，尚未接入系统目录选择对话框
2. 固定窗口不支持缩放，重启程序后不会恢复之前的固定内容
3. 默认开启"截图后自动复制"，会覆盖剪贴板内容
4. 倒计时仅显示在状态栏，没有全屏倒计时动画
5. JPEG 质量只在保存 JPG/JPEG 时生效

### 新增/修改文件

```
src/pin/mod.rs      新增：固定到屏幕的置顶悬浮窗（拖拽 / 双击 / Esc 关闭）
src/output/mod.rs   新增：文件名模板、唯一路径、save_with_template + 4 个测试
src/config/mod.rs   新增：file_name_template / jpeg_quality / delay_seconds（serde 默认值）
src/editor/mod.rs   工具栏新增「固定」按钮
src/main.rs         设置窗口扩展、延迟截图状态机、固定窗管理、自动保存/复制
Cargo.toml          新增 feature：Win32_System_SystemInformation
```

### 踩坑记录

| 问题 | 原因 | 解决 |
|------|------|------|
| `GetLocalTime` 参数不匹配 | windows 0.58 直接返回 `SYSTEMTIME` | 改为无参调用 |
| 固定窗口不出现 | 立即视口需要每帧驱动 | 主循环持续调用 `show`，并以低频重绘平衡 CPU |
| 保存配置在测试环境被拒 | 沙箱限制 `%APPDATA%` 写入（环境问题） | 验证时重定向 `APPDATA` 到工作区 |
| 旧配置加载失败风险 | 新增字段缺省值 | 字段级 `serde(default = ...)` |

---

## M7 - 测试与打包 ✅

**完成日期：2026-11-17 · 44 个测试、Release 构建、便携包、CI 与用户手册**

### 任务清单

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 7.1 | 单元测试 | ✅ 完成 | 44 个测试，覆盖截图裁剪、标注烘焙、模板、配置、热键 |
| 7.2 | 集成测试 | ✅ 完成 | 截图数据 → 标注合成 → 按模板保存 → 解码校验像素 |
| 7.3 | 边界测试 | ✅ 完成 | 越界标注、裁剪边界、非法热键、非法文件名、缺失配置 |
| 7.4 | 多显示器测试 | ⚠ 未验证 | 当前环境仅单显示器 2560x1600，多屏需人工验证 |
| 7.5 | 性能测试 | ✅ 完成 | Release 基准：200 条标注合成 15.0ms、马赛克 9.6ms、PNG 12.1ms、JPG 59.2ms |
| 7.6 | Release 构建 | ✅ 完成 | `cargo build --release`，exe 约 4.1MB，启动冒烟通过 |
| 7.7 | 安装包 | ✅ 完成 | 便携 zip + `install.ps1` / `uninstall.ps1`（开始菜单快捷方式、自启清理） |
| 7.8 | Defender 误报检查 | ⚠ 未签名 | 未做代码签名，文档已提示添加信任 |
| 7.9 | 用户手册 | ✅ 完成 | `docs/user-guide.md` |
| 7.10 | CI | ✅ 完成 | GitHub Actions：fmt / clippy / test / release build |

### 验收结果

| 验收项 | 目标 | 实测 |
|--------|------|------|
| 全量测试 | 全部通过 | ✅ 44 passed，0 failed |
| 静态检查 | 零告警 | ✅ `cargo clippy --all-targets` 无告警 |
| Release 产物 | 可启动 | ✅ `screencapture.exe` 4.1MB，进程正常存活 |
| 发布包 | 结构完整 | ✅ `ScreenCapture-0.7.0-win64.zip` 含 exe / README / 安装卸载脚本 / 用户手册 |
| 完整流程 | 截图→标注→保存 | ✅ 集成测试解码 PNG 校验红/黑像素 |
| 性能基准 | 大图不卡顿 | ✅ 2560x1600 + 200 条标注合成 15.0ms |
| 安装脚本 | 安装/卸载可回滚 | ✅ 安装后 exe + 快捷方式存在；卸载后全部清理 |

### 性能基准（Release，2560x1600）

| 场景 | 耗时 | 备注 |
|------|------|------|
| 标注合成（200 条混合标注） | 15.0 ms | 含 16MB 像素缓冲克隆与光栅化 |
| 马赛克（800×600 / block 16） | 9.6 ms | 按块均值 |
| 保存 PNG | 12.1 ms | 460 KB |
| 保存 JPG（质量 92） | 59.2 ms | 600 KB |
| 全屏截图（真机） | 36–46 ms | 含 DWM 合成等待 |

复现：`cargo test --release -- --ignored --nocapture performance_snapshot`

### 实现要点

- **集成测试**：在 `main.rs` 的 `integration_tests` 中串联 `flatten → save_with_template → image::open`，验证标注烘焙与模板命名的真实结果
- **边界测试**：完全越界的标注不改变像素、部分越界只影响图像内区域；裁剪贴边/单像素；模板未知变量保留、非法字符替换、带扩展名不重复追加
- **打包脚本**：`scripts/package.ps1` 自动读取 Cargo.toml 版本、构建 release、复制文档与安装脚本并压缩为便携 zip
- **安装脚本**：`install.ps1` 复制到 `%LOCALAPPDATA%\Programs\ScreenCapture`、创建开始菜单/可选桌面快捷方式；`uninstall.ps1` 清理快捷方式、开机自启项与程序文件
- **CI**：Windows runner 上执行格式检查、clippy（`-D warnings`）、测试与 release 构建

### 已知遗留

1. 多显示器（2×4K / 混合 DPI）未实测
2. 未做代码签名，可能触发 SmartScreen / Defender 提示
3. 安装为 PowerShell 脚本（非 MSI/EXE 安装器），卸载需运行脚本

---

## M8 - macOS 跨平台支持 ✅

**完成日期：2026-11-17 · 平台拆分重构 + macOS 实现 + 双平台 CI/打包**

### 任务清单

| # | 任务 | 状态 | 备注 |
|---|------|------|------|
| 8.1 | 截图模块平台拆分 | ✅ 完成 | `capture/mod.rs` 共享门面；Windows 逻辑迁入 `capture/windows.rs`，新增 `capture/macos.rs`（xcap） |
| 8.2 | 托盘模块平台拆分 | ✅ 完成 | Windows 保留后台消息线程；新增 `tray/macos.rs`（tray-icon + global-hotkey，主线程创建） |
| 8.3 | 热键平台抽象 | ✅ 完成 | `Combo` 改为平台无关 `Key`；`hotkey/windows.rs`（VK）、`hotkey/macos.rs`（Code） |
| 8.4 | 配置路径与自启动 | ✅ 完成 | macOS：`~/Library/Application Support/...`、LaunchAgent plist；默认保存目录 `~/Pictures/ScreenCapture` |
| 8.5 | 跨平台时间 | ✅ 完成 | `GetLocalTime` → `chrono::Local` |
| 8.6 | 依赖按目标拆分 | ✅ 完成 | `windows` 仅 Windows；`xcap` / `tray-icon` / `global-hotkey` 仅 macOS |
| 8.7 | GUI 依赖升级 | ✅ 完成 | eframe/egui 0.27 → 0.29（旧 objc2 无法在当前 Rust 下编译） |
| 8.8 | macOS 打包脚本 | ✅ 完成 | `scripts/package-macos.sh`：.app bundle + zip（可选 dmg） |
| 8.9 | CI 双平台 | ✅ 完成 | 新增 macOS job：fmt / clippy / test / release / package |
| 8.10 | 文档更新 | ✅ 完成 | README / design / user-guide 增 macOS 章节并标注未实机验证 |

### 验收结果

| 验收项 | 目标 | 实测 |
|--------|------|------|
| Windows 功能与测试 | 不回归 | ✅ 47 passed，0 failed；clippy `-D warnings` 零告警 |
| macOS 编译 | 通过交叉检查 | ✅ `cargo check --target aarch64-apple-darwin --all-targets` 通过 |
| macOS 运行时 | 真机验证 | ⚠ 无设备，未验证（权限、托盘、热键、多屏） |
| 打包 / CI | 双平台可用 | ✅ 脚本与 workflow 已补齐（macOS 未实际执行） |

### 实现要点

- **平台门面**：`capture` / `tray` / `hotkey` 通过 `#[cfg]` 选择 `windows` / `macos` 实现，`main.rs` 与 UI 层不感知平台
- **窗口句柄抽象**：新增 `WindowHandle(isize)`（Windows 存 HWND，macOS 存 CGWindowID），公开 API 保持一致
- **macOS 全屏拼接**：逐个 `Monitor::capture_image()`（RGBA）按虚拟屏幕并集贴图并转换为 BGRA，区域截图在其上裁剪
- **macOS 托盘线程模型**：tray-icon / global-hotkey 要求主线程，事件用全局回调写入队列，`try_recv` 轮询 + `request_repaint`
- **macOS 自启动**：写 `~/Library/LaunchAgents/com.screencapture.app.plist`；在 .app 内运行时用 `open -a` 启动整个应用包
- **通知**：macOS 用 `osascript display notification`，避免引入额外通知依赖

### 已知限制

1. macOS 未在真机运行验证（屏幕录制权限、菜单栏图标、全局热键、区域覆盖层、固定窗）
2. macOS 全屏截图不会排除自身窗口（Windows 有移屏逻辑）
3. 多显示器 / 混合缩放比例下坐标换算未验证
4. 产物未签名、未公证

### 新增/修改文件

```
src/capture/mod.rs       重写为平台门面（共享类型 + crop + 分派）
src/capture/windows.rs   新增：Windows GDI 截图 / 枚举 / 窗口移屏（原 logic）
src/capture/macos.rs     新增：xcap 截图 / 多屏拼接 / 窗口枚举
src/tray/mod.rs          重写为共享类型 + 平台选择
src/tray/windows.rs      新增：原 Windows 托盘线程实现
src/tray/macos.rs        新增：tray-icon 菜单栏 + global-hotkey
src/hotkey/mod.rs        重写：平台无关 Key / NamedKey + 解析
src/hotkey/windows.rs    新增：Key → VK 映射
src/hotkey/macos.rs      新增：Key → Code 映射
src/config/mod.rs        平台路径 / 默认保存目录 / Windows 注册表与 macOS LaunchAgent
src/output/mod.rs        local_now 改用 chrono
src/main.rs              字体候选按平台、WindowHandle、egui 0.29 API 适配
scripts/package-macos.sh 新增：.app / zip / 可选 dmg
.github/workflows/ci.yml 新增 macOS job
Cargo.toml               版本 0.8.0；eframe/egui 0.29；依赖按 target 拆分
```

### 踩坑记录

| 问题 | 原因 | 解决 |
|------|------|------|
| `cargo check --target aarch64-apple-darwin` 在 objc2 0.4.1 / 0.3-beta 报 E0277 | eframe 0.27（winit 0.29 / accesskit）依赖的旧 objc2 与当前 rustc 1.100 不兼容 | 升级 eframe/egui 到 0.29（winit 0.30，objc2 0.5/0.6） |
| egui 0.29 编译报错 / 弃用 | `run_native` 闭包返回 `Result`；`from_id_source`→`from_id_salt`；`clamp_range`→`range` | 逐项适配 |
| tray-icon 在 macOS 只能主线程创建 | 依赖 `MainThreadMarker` | macOS 托盘不走后台线程，改为 UI 线程创建 + 事件队列 |

---

## 变更日志

### v0.8.0 - 2026-11-17
- 完成 M8：macOS 跨平台支持
- `capture` / `tray` / `hotkey` 按平台拆分，新增 macOS 实现（xcap / tray-icon / global-hotkey）
- 窗口句柄抽象为 `WindowHandle`，UI 层无需区分平台
- macOS 配置路径 `~/Library/Application Support/ScreenCapture/config.toml`，默认保存目录 `~/Pictures/ScreenCapture`
- macOS 开机自启改用 LaunchAgent plist
- 本地时间改用 `chrono`，移除 Win32 `GetLocalTime`
- eframe / egui 升级 0.27 → 0.29（旧 objc2 与当前 Rust 不兼容）
- 新增 `scripts/package-macos.sh`（.app bundle + zip，可选 dmg）
- CI 新增 macOS job（fmt / clippy / test / release / package）
- 测试 47 个通过；Windows 功能无回归；macOS 仅交叉编译验证，运行时待真机验证
- GUI 依赖改为单一来源：仅声明 `eframe`，egui 统一经 `eframe::egui` 使用，杜绝 eframe / egui 版本分叉
- 新增 `build.rs`：为 Windows 可执行文件嵌入版本资源（作者 / 版权 / 版本 / 网站），其他平台自动跳过
- 新增应用图标：蓝色圆角底 + 白色显示器轮廓 + 四角取景框，`scripts/make_icon.py` 生成 PNG / ICO / ICNS
- 图标接入：Windows exe 资源 + 窗口/任务栏图标、Windows 托盘图标、macOS 菜单栏模板图标与 .app 图标

### v0.7.0 - 2026-11-17
- 完成 M7：测试与打包
- 测试 37 → 44 个：新增越界标注、裁剪贴边、模板未知变量/扩展名、PNG 回读、端到端集成测试
- 新增 `scripts/package.ps1`：一键构建并生成 `dist/ScreenCapture-0.7.0-win64.zip`
- 新增 `install.ps1` / `uninstall.ps1`：安装到用户目录、创建快捷方式、清理自启与程序文件
- 新增性能基准（ignored 测试）：200 条标注合成 15.0ms、PNG 保存 12.1ms、JPG 59.2ms
- 新增 GitHub Actions CI（fmt / clippy / test / release）
- 新增用户手册 `docs/user-guide.md`
- Cargo 版本号更新为 0.7.0
- Release 构建冒烟通过（exe 约 4.1MB，压缩包约 2.1MB）

### v0.6.0 - 2026-11-17
- 完成 M6：输出与配置
- 文件名模板：`{date}` `{time}` `{datetime}` `{unix}` `{index}`，非法字符过滤，重名自动追加序号
- 保存支持配置目录、PNG/JPG/BMP 格式与 JPEG 质量
- 延迟截图：工具栏菜单，0/1/3/5/10 秒倒计时
- 固定到屏幕：置顶无边框悬浮窗，拖拽移动，双击/Esc 关闭
- 设置窗口：保存目录、模板、格式、质量、延迟、自动保存、自动复制、热键、自启、主题
- 截图后自动复制、点击「完成」自动保存（可在设置关闭）
- 配置新增字段带默认值，兼容旧配置文件
- 测试 34 → 37 个，全部通过；clippy 零告警

### v0.5.0 - 2026-11-17
- 完成 M5：系统托盘 + 全局热键
- 托盘图标常驻（独立后台消息线程），左键显示主窗口，右键菜单：全屏 / 区域 / 窗口 / 显示 / 退出
- 全局热键：默认 `Ctrl+Shift+A`（区域）、`Ctrl+Shift+F`（全屏）、`Ctrl+Shift+W`（窗口）
- 热键字符串解析（Ctrl/Shift/Alt/Win + 字母/数字/F1–F24/特殊键）
- 热键冲突检测：系统占用与同批次重复均给出中文提示
- 设置窗口：热键编辑、保存应用、状态回显、配置路径展示
- 热键配置持久化到 `config.toml`，启动时自动读取
- 托盘气泡通知：保存 / 复制 / 截图失败
- 开机自启动（HKCU Run）可开关
- 测试 28 → 34 个，全部通过；clippy 零告警

### v0.4.0 - 2026-11-17
- 完成 M4：截图后自动进入内嵌编辑器
- 标注工具：画笔 / 矩形 / 椭圆 / 箭头 / 文字 / 序号 / 马赛克 / 取色器
- 颜色预设 + 取色盘，粗细/字号/填充/马赛克块大小可调
- 撤销/重做（Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z）与清除标注
- 画布滚轮缩放（以光标为锚点）+ 中键/选择工具平移
- 保存 PNG 与复制剪贴板均自动合成标注（软件光栅化，含 1px 抗锯齿）
- 文字烘焙使用 ab_glyph + 系统字体；马赛克按块均值像素化
- 浅色/深色主题切换
- 测试 18 → 28 个，全部通过；clippy 零告警

### v0.3.0 - 2026-11-17
- 完成 M3：全屏选区覆盖层（拖拽选区 / 四边压暗 / 尺寸标签 / 角点手柄）
- 放大镜取色：15×15 像素，显示屏幕坐标 + HEX + RGB
- 键盘操作：方向键微调、Shift 加速、Ctrl+方向键改尺寸、Ctrl+A 全选、Enter/双击确认、Esc 取消
- 悬停窗口红色高亮（基于截图时刻 z 序窗口列表）
- 确认后按比例裁剪并回填主窗口预览
- 截图时自身窗口移出屏幕，修复 M2「截到自己」问题
- 测试 14 → 18 个，全部通过

### v0.2.0 - 2026-11-17
- 完成 M2：全屏 / 区域 / 窗口截图全部可用
- GDI RAII 封装 + top-down 像素读取
- 显示器与窗口枚举（含过滤规则）
- Per-Monitor V2 DPI 感知
- 截图预览 UI、F1/F2/F3 本地快捷键
- 保存 PNG/JPG/BMP、复制到剪贴板
- 配置读写（%APPDATA%\ScreenCapture\config.toml）
- 12 个测试全部通过，全屏 2560x1600 耗时 32.8ms

### v0.1.0 - 2026-11-16
- 项目初始化
- 完成设计文档与 M1 脚手架

---

## 风险与阻塞

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| `windows` crate API 变化 | 编译失败 | 锁定版本号 |
| 高 DPI 屏幕坐标偏移 | 选区不准 | EnableHighDpiMode |
| DirectX 全屏独占窗口无法 BitBlt | 黑屏 | 回退方案 + PrintWindow |
| 杀毒软件拦截全局热键 | 功能失效 | 申请代码签名 |

---

## 技术债务追踪

| 债务 | 影响范围 | 计划偿还 |
|------|---------|---------|
| - | - | - |

---

## 构建命令备忘

```powershell
# 开发构建
cargo build

# Release 构建（带优化）
cargo build --release --features release

# 运行
cargo run

# 格式化
cargo fmt

# 静态检查
cargo clippy --all-targets --all-features
```
