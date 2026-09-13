"""Generate Tauri icon set from scratch.

Produces:
  icons/32x32.png
  icons/128x128.png
  icons/128x128@2x.png  (256x256)
  icons/icon.png         (source for tauri-cli)
  icons/icon.ico         (multi-size ICO for Windows)
  icons/icon.icns        (ICNS for macOS — generated via PIL save with icns plugin if available)
  icons/Square*Logo.png  (Linux PNG set Tauri expects in the bundle dir)

Note: Tauri's `tauri icon` command would normally do this from one source PNG.
We don't have the CLI here, so we generate each required variant directly.
"""

import os
from PIL import Image, ImageDraw

OUT = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")
os.makedirs(OUT, exist_ok=True)

# WhatsApp brand green, with a white phone glyph in the middle.
BG = (37, 211, 102, 255)   # #25D366
FG = (255, 255, 255, 255)


def make_icon(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # Slight padding so the corner radius reads cleanly on small sizes.
    pad = max(1, size // 16)
    d.ellipse((pad, pad, size - pad, size - pad), fill=BG)

    # Draw a simplified chat-bubble glyph: rounded square with a tail.
    s = size
    bubble_box = (s * 0.22, s * 0.24, s * 0.78, s * 0.68)
    d.rounded_rectangle(bubble_box, radius=int(s * 0.08), fill=FG)

    # Bubble tail (bottom-left).
    tail = [
        (s * 0.30, s * 0.62),
        (s * 0.20, s * 0.78),
        (s * 0.34, s * 0.66),
    ]
    d.polygon(tail, fill=FG)

    # Three dots inside the bubble to suggest typing.
    cx = s * 0.5
    cy = s * 0.46
    r = max(1, int(s * 0.04))
    for i, dx in enumerate((-0.10, 0.0, 0.10)):
        d.ellipse((cx + dx * s - r, cy - r, cx + dx * s + r, cy + r), fill=BG)
    return img


def write(name: str, img: Image.Image) -> None:
    p = os.path.join(OUT, name)
    img.save(p, "PNG")
    print(f"  {p}")


print("Generating PNGs...")
src = make_icon(1024)
write("icon.png", src)            # source
write("32x32.png", make_icon(32))
write("128x128.png", make_icon(128))
write("128x128@2x.png", make_icon(256))
write("icon-256.png", make_icon(256))
write("icon-512.png", make_icon(512))

# Linux bundle expects a Square*Logo.png set in the bundle directory.
# Tauri copies these next to the .deb / .AppImage; not strictly required to
# exist as `icons/...`, but harmless to ship.
write("Square30x30Logo.png", make_icon(30))
write("Square44x44Logo.png", make_icon(44))
write("Square71x71Logo.png", make_icon(71))
write("Square89x89Logo.png", make_icon(89))
write("Square107x107Logo.png", make_icon(107))
write("Square142x142Logo.png", make_icon(142))
write("Square150x150Logo.png", make_icon(150))
write("Square284x284Logo.png", make_icon(284))
write("Square310x310Logo.png", make_icon(310))
write("StoreLogo.png", make_icon(50))

print("Generating ICO...")
# Multi-resolution ICO.
ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
src.save(
    os.path.join(OUT, "icon.ico"),
    format="ICO",
    sizes=ico_sizes,
)
print(f"  {os.path.join(OUT, 'icon.ico')}")

print("Generating ICNS...")
try:
    src.save(os.path.join(OUT, "icon.icns"), format="ICNS")
    print(f"  {os.path.join(OUT, 'icon.icns')}")
except (OSError, ValueError) as e:
    # PIL's ICNS support depends on the plugin being compiled in.
    # If absent, leave a placeholder file and warn — the user can re-generate
    # via `cargo tauri icon` later.
    print(f"  ICNS generation failed ({e}); leaving icon.icns for tauri-cli to produce.")

print("Done.")