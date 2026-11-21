#!/usr/bin/env python3
"""The launcher's icons and logo from assets/logo-full.png (The Aegis Oppai XC2S2 Edition emblem):
src-tauri/icons/{32x32,128x128,128x128@2x,icon}.png, ui/icon.png (title bar) and ui/logo.png (the page).  Needs Pillow."""
import os
import sys
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "src-tauri", "icons")


def main():
    src = Image.open(os.path.join(ROOT, "assets", "logo-full.png")).convert("RGBA")
    # trim the transparent margin so the emblem fills the icon
    box = src.getchannel("A").point(lambda a: 255 if a > 16 else 0).getbbox()
    img = src.crop(box)
    side = max(img.size)
    sq = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    sq.paste(img, ((side - img.width) // 2, (side - img.height) // 2))
    os.makedirs(OUT, exist_ok=True)
    for name, size in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)):
        sq.resize((size, size), Image.LANCZOS).save(os.path.join(OUT, name), optimize=True)
    sq.resize((96, 96), Image.LANCZOS).save(os.path.join(ROOT, "ui", "icon.png"), optimize=True)
    sq.resize((560, 560), Image.LANCZOS).save(os.path.join(ROOT, "ui", "logo.png"), optimize=True)
    # the page's backdrop: tiny, so stretching it is the blur (no CSS filter, which WebKitGTK redraws in software)
    from PIL import ImageFilter
    bg = sq.convert("RGBA").resize((48, 48), Image.LANCZOS).filter(ImageFilter.GaussianBlur(3)).resize((24, 24), Image.LANCZOS)
    bg.save(os.path.join(ROOT, "ui", "logo-bg.png"), optimize=True)
    print("icons in", OUT)


if __name__ == "__main__":
    sys.exit(main())
