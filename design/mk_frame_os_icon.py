#!/usr/bin/env python3
"""FRAME OS app icon set — "mini OS" mark (candidate A2).

A small display with a status pixel, the big clock block and a widget tile.
Flat, full-bleed (no transparent padding), FRAME OS palette:
  canvas #0b0d12 · accent lime #cefa83 · soft #b8beca · muted #595f6c
No gradients, legible down to 16px.
"""
import os
import struct
from PIL import Image, ImageDraw

OUT = "/home/teo/Documents/Default Project/sharkdeck/src-tauri/icons"
os.makedirs(OUT, exist_ok=True)

CANVAS = (0x0B, 0x0D, 0x12)
LIME = (0xCE, 0xFA, 0x83)
SOFT = (0xB8, 0xBE, 0xCA)
MUTED = (0x59, 0x5F, 0x6C)

SS = 4  # supersample factor


def draw(size):
    S = size * SS
    img = Image.new("RGB", (S, S), CANVAS)
    d = ImageDraw.Draw(img)
    u = S / 512.0  # design units -> pixels

    def rr(box, r, fill=None, outline=None, width=0):
        d.rounded_rectangle(
            [box[0] * u, box[1] * u, box[2] * u, box[3] * u],
            radius=r * u, fill=fill, outline=outline, width=max(1, int(width * u)),
        )

    def rect(box, fill):
        d.rectangle([box[0] * u, box[1] * u, box[2] * u, box[3] * u], fill=fill)

    # screen
    rr((66, 116, 446, 396), 30, outline=SOFT, width=18)
    # status pixels
    rect((104, 150, 142, 188), LIME)
    rect((160, 150, 198, 188), MUTED)
    # clock block + widget tile
    rr((104, 214, 322, 316), 14, fill=LIME)
    rr((350, 214, 410, 316), 12, fill=MUTED)
    return img.resize((size, size), Image.LANCZOS)


def write_ico(path, sizes):
    images = [draw(s) for s in sizes]
    blobs = []
    for im in images:
        import io
        buf = io.BytesIO()
        im.convert("RGBA").save(buf, format="PNG")
        blobs.append(buf.getvalue())
    n = len(blobs)
    header = struct.pack("<HHH", 0, 1, n)
    offset = 6 + 16 * n
    entries, data = b"", b""
    for size, blob in zip(sizes, blobs):
        w = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(blob), offset)
        data += blob
        offset += len(blob)
    with open(path, "wb") as f:
        f.write(header + entries + data)


for name, size in [
    ("32x32.png", 32),
    ("128x128.png", 128),
    ("128x128@2x.png", 256),
    ("icon.png", 512),
]:
    draw(size).convert("RGBA").save(os.path.join(OUT, name))
    print("wrote", name, size)

write_ico(os.path.join(OUT, "icon.ico"), [16, 32, 48, 64, 128, 256])
print("wrote icon.ico")
