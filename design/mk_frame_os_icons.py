#!/usr/bin/env python3
"""FRAME OS icon candidates — flat, full-bleed, no gradients, legible at 16px.

Renders 4 directions at 128px (4x supersampled) plus a contact sheet with a
32px legibility row.
"""
import os
from PIL import Image, ImageDraw

OUT = "/tmp/opencode/icons"
os.makedirs(OUT, exist_ok=True)

CANVAS = (0x0B, 0x0D, 0x12)   # #0b0d12 FRAME OS canvas
LIME = (0xCE, 0xFA, 0x83)     # #cefa83 accent
SOFT = (0xB8, 0xBE, 0xCA)     # #b8beca
MUTED = (0x59, 0x5F, 0x6C)    # #595f6c

S = 512  # supersampled size (128 final)


def plate():
    img = Image.new("RGB", (S, S), CANVAS)
    return img, ImageDraw.Draw(img)


def rr(d, box, r, fill=None, outline=None, width=0):
    d.rounded_rectangle(box, radius=r, fill=fill, outline=outline, width=width)


def cand_a():
    """Frame + mini OS: a small screen with a status row and a clock block."""
    img, d = plate()
    # screen body
    rr(d, (76, 128, 436, 384), 26, outline=SOFT, width=14)
    # status row: three pixels
    d.rectangle((108, 160, 132, 184), fill=LIME)
    d.rectangle((144, 160, 168, 184), fill=MUTED)
    d.rectangle((180, 160, 204, 184), fill=MUTED)
    # clock block
    rr(d, (108, 214, 300, 292), 10, fill=LIME)
    # small widget block
    rr(d, (332, 214, 404, 292), 10, fill=MUTED)
    # baseline rule
    d.rectangle((108, 324, 404, 336), fill=MUTED)
    return img


def cand_b():
    """Bold geometric F (FRAME) with a soft frame corner."""
    img, d = plate()
    d.rectangle((112, 104, 196, 408), fill=LIME)          # stem
    d.rectangle((196, 104, 400, 180), fill=LIME)          # top arm
    d.rectangle((196, 232, 344, 308), fill=LIME)          # mid arm
    # frame corner bracket (soft) bottom-right
    d.rectangle((316, 356, 404, 372), fill=SOFT)
    d.rectangle((388, 284, 404, 372), fill=SOFT)
    return img


def cand_c():
    """Pixel desktop: a 4x3 grid of tiles inside a screen outline."""
    img, d = plate()
    rr(d, (72, 116, 440, 396), 24, outline=MUTED, width=12)
    cells = [
        (100, 148, LIME), (196, 148, SOFT), (292, 148, MUTED),
        (100, 240, MUTED), (196, 240, LIME), (292, 240, SOFT),
        (100, 332, SOFT), (196, 332, MUTED), (292, 332, LIME),
    ]
    for x, y, c in cells:
        rr(d, (x, y, x + 68, y + 68), 10, fill=c)
    return img


def cand_d():
    """Knob dial + screen: the two halves of the K86 (rotary + display)."""
    img, d = plate()
    # dial
    d.ellipse((96, 96, 320, 320), outline=LIME, width=26)
    # indicator notch (12 o'clock-ish)
    d.polygon([(208, 118), (228, 118), (218, 186)], fill=LIME)
    # small screen strip bottom-right
    rr(d, (272, 300, 424, 412), 18, outline=SOFT, width=14)
    d.rectangle((300, 336, 396, 356), fill=LIME)
    return img


CANDS = [
    ("a-frame-os", cand_a),
    ("b-bold-f", cand_b),
    ("c-pixel-grid", cand_c),
    ("d-dial-screen", cand_d),
]

for name, fn in CANDS:
    img = fn().resize((128, 128), Image.LANCZOS)
    img.save(f"{OUT}/cand_{name}_128.png")

# contact sheet: light row + dark row, 128 and 32px
sheet = Image.new("RGB", (4 * 160 + 40, 2 * 200 + 60), (0xF6, 0xF5, 0xF4))
d = ImageDraw.Draw(sheet)
for i, (name, _) in enumerate(CANDS):
    x = 20 + i * 160
    big = Image.open(f"{OUT}/cand_{name}_128.png")
    sheet.paste(big, (x + 16, 20))
    small = big.resize((32, 32), Image.LANCZOS)
    sheet.paste(small, (x + 64, 100))
    d.text((x + 16, 160), name, fill=(0x20, 0x20, 0x20))
    # dark row copy
    sheet.paste(big, (x + 16, 230))
    sheet.paste(small, (x + 64, 310))
sheet.save(f"{OUT}/frame_os_icons_contact.png")
print("candidates written to", OUT)
