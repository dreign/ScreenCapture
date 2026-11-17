"""生成 ScreenCapture 应用图标（Windows .ico / macOS .icns / PNG）。

设计：蓝色圆角方块背景 + 白色显示器轮廓 + 屏幕内的四角选取框，
同时表达「屏幕」与「框选截取」。

用法（项目根目录）：
    python scripts/make_icon.py
"""

from __future__ import annotations

import struct
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"

# 背景渐变色（上 → 下）
TOP = (86, 148, 255)
BOTTOM = (29, 92, 219)
WHITE = (255, 255, 255, 255)

# 圆角方块半径占比
CORNER_RADIUS = 0.22


def _bar(draw: ImageDraw.ImageDraw, x0: float, y0: float, x1: float, y1: float, color=WHITE) -> None:
    """画一个圆头胶囊（用于选取框的转角笔段）。"""
    draw.rounded_rectangle([x0, y0, x1, y1], radius=min(x1 - x0, y1 - y0) / 2, fill=color)


def _linear_vertical(size: int, top: tuple[int, int, int], bottom: tuple[int, int, int]) -> Image.Image:
    """生成竖向渐变的背景层。"""
    layer = Image.new("RGBA", (size, size))
    draw = ImageDraw.Draw(layer)
    for y in range(size):
        t = y / max(1, size - 1)
        color = tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,)
        draw.line([(0, y), (size, y)], fill=color)
    return layer


def render(size: int) -> Image.Image:
    """渲染指定尺寸的图标（4 倍超采样后缩小，保证边缘平滑）。"""
    scale = 4
    canvas = size * scale
    u = float(canvas)

    image = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    mask = Image.new("L", (canvas, canvas), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, canvas - 1, canvas - 1], radius=int(CORNER_RADIUS * u), fill=255
    )
    image.paste(_linear_vertical(canvas, TOP, BOTTOM), (0, 0), mask)

    draw = ImageDraw.Draw(image)
    detailed = size >= 48
    stroke = max(u * 0.052, 4.0 if detailed else 5.0)

    # 显示器外框
    sx0, sy0 = u * 0.185, u * 0.205
    sx1, sy1 = u * 0.815, u * 0.625 if detailed else u * 0.70
    radius = u * 0.055
    draw.rounded_rectangle([sx0, sy0, sx1, sy1], radius=radius, outline=WHITE, width=round(stroke))

    # 底座（小尺寸省略，避免糊成一团）
    if detailed:
        stem_w = u * 0.056
        cx = u * 0.5
        _bar(draw, cx - stem_w / 2, sy1, cx + stem_w / 2, sy1 + u * 0.085)
        base_w, base_h = u * 0.29, u * 0.05
        _bar(draw, cx - base_w / 2, sy1 + u * 0.085, cx + base_w / 2, sy1 + u * 0.085 + base_h)

    # 屏幕内的四角选取框
    inset = u * (0.065 if detailed else 0.08)
    arm = u * (0.088 if detailed else 0.06)
    thick = max(stroke * 0.9, 5.0)
    bx0, by0 = sx0 + inset + stroke / 2, sy0 + inset + stroke / 2
    bx1, by1 = sx1 - inset - stroke / 2, sy1 - inset - stroke / 2
    half = thick / 2

    for cx, cy, dx, dy in (
        (bx0, by0, 1, 1),
        (bx1, by0, -1, 1),
        (bx0, by1, 1, -1),
        (bx1, by1, -1, -1),
    ):
        # 水平臂
        x0, x1 = sorted((cx, cx + dx * arm))
        y0, y1 = sorted((cy - half, cy + half))
        draw.rounded_rectangle([x0, y0, x1, y1], radius=half, fill=WHITE)
        # 竖直臂
        x0, x1 = sorted((cx - half, cx + half))
        y0, y1 = sorted((cy, cy + dy * arm))
        draw.rounded_rectangle([x0, y0, x1, y1], radius=half, fill=WHITE)

    return image.resize((size, size), Image.LANCZOS)


def build_ico(png_by_size: dict[int, bytes]) -> bytes:
    """按 ICO 容器格式打包 PNG 帧（Vista+ 支持 PNG 压缩条目）。"""
    sizes = sorted(png_by_size)
    header = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    entries = b""
    payloads = b""
    for size in sizes:
        payload = png_by_size[size]
        entries += struct.pack(
            "<BBBBHHII",
            0 if size >= 256 else size,
            0 if size >= 256 else size,
            0,
            0,
            1,
            32,
            len(payload),
            offset,
        )
        offset += len(payload)
        payloads += payload
    return header + entries + payloads


def build_icns(png_by_size: dict[int, bytes]) -> bytes:
    """按 ICNS 容器格式打包 PNG 帧。"""
    # icns 类型 → 像素尺寸
    entries = {
        b"ic07": 128,
        b"ic08": 256,
        b"ic09": 512,
        b"ic10": 1024,
        b"ic11": 32,
        b"ic12": 64,
        b"ic13": 256,
        b"ic14": 512,
    }
    body = b""
    for kind, size in entries.items():
        payload = png_by_size[size]
        body += kind + struct.pack(">I", len(payload) + 8) + payload
    return b"icns" + struct.pack(">I", len(body) + 8) + body


def main() -> None:
    ASSETS.mkdir(exist_ok=True)

    ico_sizes = [16, 24, 32, 48, 64, 128, 256]
    icns_sizes = [32, 64, 128, 256, 512, 1024]

    rendered = {size: render(size) for size in sorted(set(ico_sizes + icns_sizes + [256, 1024]))}

    master = rendered[1024]
    master.save(ASSETS / "icon.png")
    rendered[256].save(ASSETS / "icon-256.png")

    ico_png = {}
    for size in ico_sizes:
        buffer = ASSETS / f".icon-{size}.png"
        rendered[size].save(buffer, format="PNG")
        ico_png[size] = buffer.read_bytes()
        buffer.unlink()
    (ASSETS / "icon.ico").write_bytes(build_ico(ico_png))

    png_by_size = {}
    for size in icns_sizes:
        buffer = ASSETS / f".icon-{size}.png"
        rendered[size].save(buffer, format="PNG")
        png_by_size[size] = buffer.read_bytes()
        buffer.unlink()
    (ASSETS / "icon.icns").write_bytes(build_icns(png_by_size))

    print("generated:", ", ".join(p.name for p in sorted(ASSETS.iterdir())))


if __name__ == "__main__":
    main()
