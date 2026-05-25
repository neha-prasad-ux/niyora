#!/usr/bin/env python3
"""
Generate Niyora tray icons for the hybrid macOS NSStatusItem.

Two layers, composited at runtime by AppKit:

  1. **Body** (`tray-body.png` + @2x) — ring + nucleus only, white silhouette.
     Loaded as an NSImage with `isTemplate = YES`. macOS auto-tints every
     pixel against the local menu-bar background (same path Wi-Fi, battery,
     and clock take), so the body stays crisp on any wallpaper without
     needing system-Appearance detection.

  2. **Electron** (`tray-electron-{state}.png` + @2x) — just the small
     coloured dot at the electron position, transparent everywhere else.
     Loaded as an NSImage with `isTemplate = NO` and rendered as an
     NSImageView subview of the status-bar button so the green/amber state
     colour survives compositing.

States produced for the electron:

    measuring  -> green  (default running state, "we're watching")
    reminder   -> amber  (notification fired, "time to breathe")

Each layer is rendered at 22x22 (1x) and 44x44 (2x) and written to
src-tauri/icons/.

Run:
    python3 scripts/generate_tray_icons.py
"""
from __future__ import annotations

import math
import os
from dataclasses import dataclass

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
ICONS_DIR = os.path.normpath(os.path.join(HERE, "..", "src-tauri", "icons"))


# ----- Design tokens -----

# Body template uses pure white. macOS template-rendering reads the alpha
# channel and tints with the system's menu-bar tint colour; the actual RGB
# is ignored, but white keeps the file readable when inspecting.
BODY_RGB = (255, 255, 255)

# Electron colours per state. Solid, saturated; the colour itself is the
# state signal.
ELECTRON_RGB = {
    "measuring": (61, 214, 140),   # mint green, on-brand
    "reminder":  (242, 163, 61),   # warm amber (no red, stays calm)
}

# Geometry, fractions of icon pixel size. Same numbers as the brand mark.
RING_RADIUS_FRAC      = 0.42
RING_STROKE_FRAC      = 0.10
NUCLEUS_RADIUS_FRAC   = 0.20
ELECTRON_RADIUS_FRAC  = 0.11
ELECTRON_ANGLE_DEG    = 215.0   # 0 = +x; 215 = upper-left


SIZES = [
    ("",     22),
    ("@2x",  44),
]


@dataclass(frozen=True)
class IconSpec:
    kind: str           # "body" or "electron-{state}"
    suffix: str         # "" or "@2x"
    size: int


def render(spec: IconSpec) -> Image.Image:
    """Render one layer. Supersample 4x for clean edges then downscale."""
    scale = 4
    big = spec.size * scale
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    cx = cy = big / 2.0
    ring_r     = big * RING_RADIUS_FRAC
    ring_w     = big * RING_STROKE_FRAC
    nucleus_r  = big * NUCLEUS_RADIUS_FRAC
    electron_r = big * ELECTRON_RADIUS_FRAC

    if spec.kind == "body":
        body_color = BODY_RGB + (255,)
        # Outer ring (stroke only)
        draw.ellipse(
            (cx - ring_r, cy - ring_r, cx + ring_r, cy + ring_r),
            outline=body_color,
            width=int(round(ring_w)),
        )
        # Nucleus (filled)
        draw.ellipse(
            (cx - nucleus_r, cy - nucleus_r, cx + nucleus_r, cy + nucleus_r),
            fill=body_color,
        )
    elif spec.kind.startswith("electron-"):
        state = spec.kind.split("-", 1)[1]
        color = ELECTRON_RGB[state] + (255,)
        theta = math.radians(ELECTRON_ANGLE_DEG)
        ex = cx + ring_r * math.cos(theta)
        ey = cy + ring_r * math.sin(theta)
        draw.ellipse(
            (ex - electron_r, ey - electron_r, ex + electron_r, ey + electron_r),
            fill=color,
        )
    else:
        raise ValueError(f"unknown kind: {spec.kind}")

    return img.resize((spec.size, spec.size), Image.LANCZOS)


def main() -> None:
    os.makedirs(ICONS_DIR, exist_ok=True)
    # Drop earlier outputs so the icons dir doesn't accumulate stale files.
    for legacy in os.listdir(ICONS_DIR):
        if legacy.startswith("tray-") and legacy.endswith(".png"):
            os.remove(os.path.join(ICONS_DIR, legacy))

    written: list[str] = []
    kinds = ["body"] + [f"electron-{s}" for s in ELECTRON_RGB.keys()]
    for kind in kinds:
        for suffix, size in SIZES:
            spec = IconSpec(kind=kind, suffix=suffix, size=size)
            img = render(spec)
            name = f"tray-{kind}{suffix}.png"
            path = os.path.join(ICONS_DIR, name)
            img.save(path, format="PNG")
            written.append(name)

    print("wrote:")
    for name in written:
        print(f"  src-tauri/icons/{name}")


if __name__ == "__main__":
    main()
