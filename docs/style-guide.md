# 界面样式规范

统一主题由 [`src/theme/mod.rs`](../src/theme/mod.rs) 提供。界面代码只引用语义常量，
不再直接书写 `Color32::from_rgb(...)` 或零散尺寸。新增界面时请先查这里有没有可复用的 token。

## 1. 配色语义

随深色/浅色模式切换（`Palette`）：

| Token | 用途 | 深色 | 浅色 |
| --- | --- | --- | --- |
| `accent` | 主色：选中、链接、聚焦描边、悬停描边 | `#00AEFF` | `#007ACC` |
| `accent_soft` | 主色弱填充：按下态、文本选区 | `#145A82` | `#D2E7F9` |
| `danger` | 错误、失败提示 | `#FF7A70` | `#C43430` |
| `warning` | 警告 | `#FFB84C` | `#9E6806` |
| `success` | 成功 | `#5ACD7E` | `#1A8547` |
| `panel` | 面板背景 | `#1B1D20` | `#F6F7F9` |
| `window` | 窗口 / 卡片背景 | `#24272B` | `#FFFFFF` |
| `sunken` | 输入框等凹陷区域 | `#121416` | `#FFFFFF` |
| `faint` | 弱化分区 | `#2A2E33` | `#ECEFF3` |
| `border` | 边框、分隔线 | `#383D44` | `#D3D8DF` |
| `widget` / `hover` / `active` | 控件常规 / 悬停 / 按下底 | 见代码 | 见代码 |
| `text` / `text_strong` / `text_muted` | 正文 / 强调 / 次要文字 | 见代码 | 见代码 |

固定色组（不随主题变化）：

- `overlay`：选区覆盖层、固定窗等浮在截图上的元素。遮罩 `black 120`、胶囊底 `black 200`、
  文字纯白、次要文字 `white 220`、十字线 `white 70`、选区主色 `#00AEFF`、窗口高亮 `#FF4848`。
- `canvas`：编辑器画布工作区。底色 `gray 26`、占位 `gray 60`、边框 `gray 90`、
  提示 `gray 190`、次要文字 `gray 140`。保证截图为亮色时也有足够对比。
- `ANNOTATION_COLORS`：标注预设八色（红橙黄绿蓝靛白黑），默认取第一个。

## 2. 字号层级

| Token | 字号 | 对应 |
| --- | --- | --- |
| `text::HEADING` | 18 | `TextStyle::Heading` |
| `text::BODY` | 14 | `TextStyle::Body` / `Button` |
| `text::HINT` | 13 | 覆盖层操作提示 |
| `text::CAPTION` | 12 | 画布信息条、胶囊标签 |
| `text::TINY` | 11 | `TextStyle::Small`、固定窗提示 |
| `text::MONO` | 12 | `TextStyle::Monospace` |

自定义 `painter` 文本必须显式使用上表字号，禁止再写字面量。

## 3. 间距与圆角

- 间距：`spacing::XS=4`、`SM=8`、`MD=12`、`LG=16`，以 4pt 为基准节奏。
  全局 `item_spacing = (8, 6)`、`button_padding = (10, 4)`、窗口内边距 12。
- 圆角：`radius::SMALL=4`（按钮 / 输入框 / 色块）、`MEDIUM=6`（菜单 / 胶囊）、
  `LARGE=10`（窗口）。

## 4. 视觉层次

1. **底色**：主窗口用 `panel`，浮动窗口 / 对话框用 `window`，二者靠 `border` 1px 描边 + 阴影区分。
2. **控件**：常规无边（`widget`）→ 悬停主色描边 + 轻微外扩 → 按下主色填充（`active`）。
3. **文字**：正文 `text`，标题 / 可点击 `text_strong`，说明 `text_muted`；
   错误一律 `danger`，警告 `warning`，成功 `success`。
4. **浮层**（选区、固定窗）：固定深色胶囊，不跟随主题；窗口级浮动面板使用主题色 + 阴影。

## 5. 深色 / 浅色一致性

- 两套色板字段完全一致，仅取值不同；新增语义色必须同时补齐 `DARK` 与 `LIGHT`。
- 主题切换统一走 `theme::apply(ctx, dark)`；禁止直接调用 `ctx.set_visuals(...)`，
  否则会丢掉字号 / 间距 / 控件四态的定制。
- 自定义绘制（画布、选区）若需要随主题变化，用 `theme::palette(ui.visuals().dark_mode)`
  取色；浮在截图上的一律用 `overlay` 固定色。
