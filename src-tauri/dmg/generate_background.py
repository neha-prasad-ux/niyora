"""
Generates the DMG installer background for Niyora.

Run from repo root:
    python3 src-tauri/dmg/generate_background.py

Outputs:
    src-tauri/dmg/background.png       (660x400)
    src-tauri/dmg/background@2x.png    (1320x800)
    src-tauri/dmg/background.tiff      (multi-resolution, used by Tauri bundler)

Design notes (see DESIGN.md):
- Near-black with faint indigo cast.
- Soft rose/violet glow (Glow tier hue ~335) top-center.
- Source Serif heading "Welcome." + quiet subline "Drag Niyora to Applications".
- Subtle drop targets: ghost circle on the left for the app, ghost folder square on the right.
- Faint hairline arrow between them.
- No exclamations, no em dashes.
"""

from __future__ import annotations
import math
import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

# The Tauri DMG window opens at 660x400. We render a larger canvas with
# bleed so when users resize the Finder window wider/taller they see more
# of the same gradient rather than tiled or empty space. Design (text and
# the Applications target) lives inside the visible 660x400 region in the
# top-left corner of the canvas; everything outside that is bleed.
WINDOW_W, WINDOW_H = 660, 400
BLEED_X, BLEED_Y = 440, 250
W, H = WINDOW_W + BLEED_X, WINDOW_H + BLEED_Y
SCALES = (1, 2)

REPO = Path(__file__).resolve().parents[2]
OUT_DIR = Path(__file__).resolve().parent

BG_TOP = (12, 12, 24)
BG_BOTTOM = (6, 6, 14)

GLOW_RGB = (140, 90, 180)

TEXT_PRIMARY = (248, 244, 252)
TEXT_SECONDARY = (200, 196, 218)
HAIRLINE = (130, 128, 158)

APP_CENTER = (165, 245)
APPS_CENTER = (495, 245)
ICON_R = 64


FONTS_DIR = Path(__file__).resolve().parent / "fonts"


def find_sans_font(size: int, bold: bool = False) -> ImageFont.ImageFont:
    """Use Poppins to match the rest of the app UI."""
    bold_candidates = [
        FONTS_DIR / "Poppins-SemiBold.ttf",
        Path("/System/Library/Fonts/HelveticaNeue.ttc"),
        Path("/Library/Fonts/Arial Bold.ttf"),
    ]
    regular_candidates = [
        FONTS_DIR / "Poppins-Regular.ttf",
        Path("/System/Library/Fonts/SFNS.ttf"),
        Path("/System/Library/Fonts/HelveticaNeue.ttc"),
        Path("/Library/Fonts/Arial.ttf"),
    ]
    candidates = bold_candidates if bold else regular_candidates
    for path in candidates:
        if path.exists():
            try:
                return ImageFont.truetype(str(path), size=size)
            except OSError:
                continue
    return ImageFont.load_default()


def find_serif_font(size: int, bold: bool = False) -> ImageFont.ImageFont:
    """Heading font. Uses Poppins (same as rest of UI) — DESIGN.md mentions
    Source Serif for in-app headings, but the installer pairs better with
    the panel's actual typeface. Falls back through SourceSerif/Times only
    if Poppins isn't on disk.
    """
    return find_sans_font(size, bold=bold)


def vertical_gradient(size: tuple[int, int], top: tuple[int, int, int], bottom: tuple[int, int, int]) -> Image.Image:
    w, h = size
    img = Image.new("RGB", size, top)
    px = img.load()
    for y in range(h):
        t = y / max(1, h - 1)
        r = int(top[0] + (bottom[0] - top[0]) * t)
        g = int(top[1] + (bottom[1] - top[1]) * t)
        b = int(top[2] + (bottom[2] - top[2]) * t)
        for x in range(w):
            px[x, y] = (r, g, b)
    return img


def add_glow(base: Image.Image, center: tuple[int, int], radius: int, color: tuple[int, int, int], intensity: float) -> Image.Image:
    overlay = Image.new("RGB", base.size, (0, 0, 0))
    draw = ImageDraw.Draw(overlay)
    cx, cy = center
    draw.ellipse([cx - radius, cy - radius, cx + radius, cy + radius], fill=color)
    overlay = overlay.filter(ImageFilter.GaussianBlur(radius=radius * 0.6))
    return Image.blend(base, Image.eval(overlay, lambda v: int(v * intensity)).convert("RGB"), 0.0).point(lambda v: v) if False else _screen_blend(base, overlay, intensity)


def _screen_blend(base: Image.Image, overlay: Image.Image, intensity: float) -> Image.Image:
    base_px = base.load()
    over_px = overlay.load()
    w, h = base.size
    for y in range(h):
        for x in range(w):
            br, bg, bb = base_px[x, y]
            orr, og, ob = over_px[x, y]
            r = 255 - int((255 - br) * (255 - orr * intensity) / 255)
            g = 255 - int((255 - bg) * (255 - og * intensity) / 255)
            b = 255 - int((255 - bb) * (255 - ob * intensity) / 255)
            base_px[x, y] = (max(0, min(255, r)), max(0, min(255, g)), max(0, min(255, b)))
    return base


def draw_ghost_app_target(draw: ImageDraw.ImageDraw, center: tuple[int, int], r: int) -> None:
    cx, cy = center
    box = [cx - r, cy - r, cx + r, cy + r]
    for i, (alpha_r, alpha_g, alpha_b, a) in enumerate([(60, 58, 80, 32), (90, 88, 110, 64)]):
        offset = i * 1
        draw.ellipse([box[0] - offset, box[1] - offset, box[2] + offset, box[3] + offset],
                     outline=(alpha_r, alpha_g, alpha_b), width=1)
    label = "Niyora"
    font = find_sans_font(13)
    bbox = draw.textbbox((0, 0), label, font=font)
    tw = bbox[2] - bbox[0]
    draw.text((cx - tw / 2, cy + r + 14), label, font=font, fill=TEXT_SECONDARY)


def draw_ghost_apps_target(draw: ImageDraw.ImageDraw, center: tuple[int, int], r: int) -> None:
    """Ghost outline of the Applications shortcut. We don't draw a label
    here because Finder renders the actual shortcut name underneath the
    icon — drawing it on the bg image would double up.
    """
    cx, cy = center
    box = [cx - r, cy - r, cx + r, cy + r]
    radius = int(r * 0.22)
    draw.rounded_rectangle(box, radius=radius, outline=(110, 108, 138), width=1)


def draw_arrow(draw: ImageDraw.ImageDraw, start: tuple[int, int], end: tuple[int, int]) -> None:
    sx, sy = start
    ex, ey = end
    draw.line([(sx, sy), (ex, ey)], fill=HAIRLINE, width=1)
    head = 8
    angle = math.atan2(ey - sy, ex - sx)
    a1 = angle + math.pi - math.pi / 7
    a2 = angle + math.pi + math.pi / 7
    draw.line([(ex, ey), (ex + head * math.cos(a1), ey + head * math.sin(a1))], fill=HAIRLINE, width=1)
    draw.line([(ex, ey), (ex + head * math.cos(a2), ey + head * math.sin(a2))], fill=HAIRLINE, width=1)


def render(scale: int) -> Image.Image:
    """Render the DMG background.

    Visible window region is the top-left WINDOW_W x WINDOW_H block; the rest
    is bleed so a user widening the Finder window sees more of the same
    gradient instead of empty space or tiled artifacts.
    """
    canvas_w, canvas_h = W * scale, H * scale
    img = vertical_gradient((canvas_w, canvas_h), BG_TOP, BG_BOTTOM)

    glow_cx = int((WINDOW_W / 2) * scale)
    glow_cy = int(WINDOW_H * 0.18 * scale)
    img = add_glow(img, (glow_cx, glow_cy), int(220 * scale), GLOW_RGB, intensity=0.18)

    draw = ImageDraw.Draw(img)

    heading_font = find_sans_font(44 * scale, bold=True)
    sub_font = find_sans_font(16 * scale)

    heading = "Welcome"
    sub = "Drag Niyora to Applications."

    h_bbox = draw.textbbox((0, 0), heading, font=heading_font)
    hw = h_bbox[2] - h_bbox[0]
    draw.text(((WINDOW_W * scale - hw) / 2, 60 * scale), heading, font=heading_font, fill=TEXT_PRIMARY)

    s_bbox = draw.textbbox((0, 0), sub, font=sub_font)
    sw = s_bbox[2] - s_bbox[0]
    draw.text(((WINDOW_W * scale - sw) / 2, 130 * scale), sub, font=sub_font, fill=TEXT_SECONDARY)

    app_c = (APP_CENTER[0] * scale, APP_CENTER[1] * scale)
    apps_c = (APPS_CENTER[0] * scale, APPS_CENTER[1] * scale)
    r = ICON_R * scale
    draw_ghost_apps_target(draw, apps_c, r)

    arrow_start = (app_c[0] + r + 16 * scale, app_c[1])
    arrow_end = (apps_c[0] - r - 16 * scale, apps_c[1])
    draw_arrow(draw, arrow_start, arrow_end)

    return img


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    pngs: list[Path] = []
    for scale in SCALES:
        img = render(scale)
        suffix = "" if scale == 1 else f"@{scale}x"
        out = OUT_DIR / f"background{suffix}.png"
        img.save(out, format="PNG", optimize=True)
        pngs.append(out)
        print(f"wrote {out} ({img.size[0]}x{img.size[1]})")

    tiffutil = shutil.which("tiffutil")
    if tiffutil is None:
        print("tiffutil not found, skipping multi-resolution tiff (macOS only).")
        return
    tiff_out = OUT_DIR / "background.tiff"
    subprocess.run(
        [tiffutil, "-cathidpicheck", *map(str, pngs), "-out", str(tiff_out)],
        check=True,
    )
    print(f"wrote {tiff_out}")


if __name__ == "__main__":
    main()
