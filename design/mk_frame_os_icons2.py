#!/usr/bin/env python3
"""Refined FRAME OS icon candidates: A2 (cleaner mini-OS) and E (frame corners + clock)."""
import os
from PIL import Image, ImageDraw

OUT = "/tmp/opencode/icons"
os.makedirs(OUT, exist_ok=True)

CANVAS = (0x0B, 0x0D, 0x12)
LIME = (0xCE, 0xFA, 0x83)
SOFT = (0xB8, 0xBE, 0xCA)
MUTED = (0x59, 0x5F, 0x6C)
S = 512


def plate():
    img = Image.new("RGB", (S, S), CANVAS)
    return img, ImageDraw.Draw(img)


def rr(d, box, r, fill=None, outline=None, width=0):
    d.rounded_rectangle(box, radius=r, fill=fill, outline=outline, width=width)


def cand_a2():
    """Mini OS: screen + big clock block + one widget tile + two status pixels."""
    img, d = plate()
    rr(d, (66, 116, 446, 396), 30, outline=SOFT, width=18)
    d.rectangle((104, 150, 142, 188), fill=LIME)
    d.rectangle((160, 150, 198, 188), fill=MUTED)
    rr(d, (104, 214, 322, 316), 14, fill=LIME)
    rr(d, (350, 214, 410, 316), 12, fill=MUTED)
    return img


def cand_e():
    """Frame corners + clock face: FRAME as literal brackets, OS as a clock."""
    img, d = plate()
    t, arm, m = 24, 104, 64
    lo, hi = m, S - m
    # four corner brackets
    d.rectangle((lo, lo, lo + arm, lo + t), fill=LIME)
    d.rectangle((lo, lo, lo + t, lo + arm), fill=LIME)
    d.rectangle((hi - arm, lo, hi, lo + t), fill=LIME)
    d.rectangle((hi - t, lo, hi, lo + arm), fill=LIME)
    d.rectangle((lo, hi - t, lo + arm, hi), fill=LIME)
    d.rectangle((lo, hi - arm, lo + t, hi), fill=LIME)
    d.rectangle((hi - arm, hi - t, hi, hi), fill=LIME)
    d.rectangle((hi - t, hi - arm, hi, hi), fill=LIME)
    # clock face
    cx = cy = S // 2
    d.ellipse((cx - 104, cy - 104, cx + 104, cy + 104), outline=SOFT, width=18)
    d.polygon([(cx, cy - 78), (cx + 16, cy - 6), (cx - 8, cy + 10)], fill=LIME)
    d.rectangle((cx - 7, cy - 10, cx + 7, cy + 72), fill=LIME)
    return img


def cand_f():
    """F whose arms are screen lines (FRAME as letterform + display)."""
    img, d = plate()
    d.rectangle((104, 96, 176, 416), fill=LIME)
    d.rectangle((176, 96, 408, 168), fill=LIME)
    d.rectangle((176, 236, 332, 308), fill=LIME)
    # tiny screen inset in the F's lower arm space
    rr(d, (208, 330, 400, 400), 12, outline=SOFT, width=12)
    d.rectangle((232, 358, 376, 372), fill=MUTED)
    return img


CANDS = [("a2-mini-os", cand_a2), ("e-frame-clock", cand_e), ("f-letterform", cand_f)]
for name, fn in CANDS:
    fn().resize((128, 128), Image.LANCZOS).save(f"{OUT}/cand_{name}_128.png")

sheet = Image.new("RGB", (3 * 170 + 40, 260), (0xF6, 0xF5, 0xF4))
d = ImageDraw.Draw(sheet)
for i, (name, _) in enumerate(CANDS):
    x = 20 + i * 170
    big = Image.open(f"{OUT}/cand_{name}_128.png")
    sheet.paste(big, (x + 20, 20))
    sheet.paste(big.resize((32, 32), Image.LANCZOS), (x + 72, 100))
    d.text((x + 20, 160), name, fill=(0x20, 0x20, 0x20))
    sheet.paste(big, (x + 20, 200))
    sheet.paste(big.resize((32, 32), Image.LANCZOS), (x + 72, 280 - 32))
sheet = sheet.crop((0, 0, sheet.width, 250))
sheet.save(f"{OUT}/frame_os_icons_refined.png")
print("refined candidates written")
