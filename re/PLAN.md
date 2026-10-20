# PLAN — from "pixels on top" to a real native OS for the K86

Owner request (paraphrased): stop pushing images over the built-in UI. Use the
REA reverse-engineering toolkit to understand the keyboard's **own** firmware,
so that (a) the factory software can be **backed up and restored**, and (b) we
can ship a genuine **FRAME OS** for the keyboard — with its own screens,
widgets, colours and shortcuts — and drive the keyboard's **native** OS from the
desktop app.

---

## 0. Ground truth about the hardware (measured, not assumed)

| Fact | Evidence |
|---|---|
| The TFT is a dumb RGB565 framebuffer, 240×135 (128×128 also verified) | `sharkdeck-cli screen-pattern`, `core/src/screen.rs` |
| The keyboard's *only* runtime is a frame/loop player with per-frame delay bytes (centiseconds, max 255 = 2.55 s) | PROTOCOL.md, keyboard-studio `Screen.swift` |
| There is **no** scripting, no vector/text primitive, no native clock screensaver exposed over HID | opcode sweep below |
| The keyboard exposes **one** USB device (3151:4015) with 3 HID interfaces; no DFU interface is published | `lsusb`, `/sys/class/hidraw/*/uevent` |
| The device's own UI (image slots, menus) is firmware code + flash resources | `sharkfin` keymap-evidence offsets into a firmware image |

**Opcode sweep on the live board** (`sharkdeck-cli opdump`, GET family only,
`0x80..=0x9F`): implemented handlers are `0x80 0x82 0x83 0x84 0x85 0x86 0x87
0x88 0x8D 0x91 0x92 0x97`; `0x89/0x90/0x8B` are echoless page reads; `0x8A`
answers all-`0xFF`; **nothing in the GET family returns a block of firmware
bytes.** So the backup cannot be done with a plain read command today.

## 1. The three ways to get the firmware out (and back in)

| Route | What it needs | Risk | Yields |
|---|---|---|---|
| **R1 — HID flash-read opcode** | the exact opcode+layout (sharkfin audit in flight) | low if it is a pure read | the unit's *current* firmware; enables true backup |
| **R2 — bootloader / DFU** | owner's consent; entering the boot entry (0x7F/0x30) | medium: DFU *read* is safe, a bad *write* bricks | dump + restore path, possibly custom flash |
| **R3 — SWD/JTAG clip** | opening the board, a ~$15 probe, the MCU part number | low for reading, high for writing | the real deal: read, patch, flash |
| **R4 — stock firmware image (public)** | finding the vendor's firmware file (official app / CDN) | none | an image to analyse *right now* with REA, and a restore source for a stock unit |

R4 unblocks analysis today; R1/R2/R3 unlock per-unit backup. R1 is tried first
because it is a normal HID command and needs no hardware work.

## 2. The REA analysis pipeline (once an image exists)

```bash
export PATH=/tmp/opencode/rea-toolchain/npm/bin:$HOME/.local/bin:$PATH
export GHIDRA_INSTALL_DIR=/tmp/opencode/rea-toolchain/ghidra

rea inspect-firmware-regions --input k86-firmware.bin   # binwalk + unblob: carve regions
rea extract-firmware      --input k86-firmware.bin      # verified extraction + provenance
rea doctor                                             # provider readiness (Ghidra, Hopper)
```

Then, in Ghidra (ARM Cortex-M), the questions to answer — in this order, each
one is a concrete analysis task, not a vibe:

1. **Command dispatcher.** Locate the HID feature-report handler and the switch
   over opcodes `0x80..0x9F` / `0xA5` / `0x25` / `0x0C` / `0xAC`. Every opcode
   the desktop app uses becomes a named function in the evidence bundle.
2. **Screen-UI layer.** Find the code that draws the built-in lock screen,
   widgets and menus: which draws the clock, which handles the knob menu, what
   palette it uses, where its bitmaps live.
3. **Resource tables.** Locate the picture/slot storage that the `0xA5`/`0x25`
   image pipeline writes into, and the format of the built-in screens' assets.
   *If the built-in screens are stored as resources in flash, replacing those
   resources with FRAME OS artwork is a full "custom OS" without writing a new
   MCU application* — this is the decisive question of the whole project.
4. **Rendering primitives.** The available drawing API (rect/line/blit/alpha/
   text bitmap) tells us what a native FRAME OS could look like.
5. **Shortcut/menu plumbing.** The knob handler and the Fn-layer dispatch —
   this is where "new shortcuts to leave the built-in screens" gets implemented
   natively instead of by pushing images.

## 3. What the desktop app becomes

* **Firmware page** — `Backup now` (R1/R2/R3), `Restore…` (with mandatory
  "a verified backup exists" guard), `Verify`, firmware version/ID, and a
  "custom OS package" installer (FRAME OS image = assets + palette + layout
  descriptors).
* **Native OS page** — drives the keyboard's *own* settings (profile, theme,
  screen brightness, knob mode, keymap) rather than painting over them; shows
  which parts are native vs emulated.
* **FRAME OS designer** — the screen/widget system we already have, but exported
  as a *native* package (assets + layout descriptors) instead of a video.
* Everything shipped so far (screen push, templates, layout system, lighting,
  per-key RGB, keymap, macros, live mirror) stays, but is reframed as
  "emulated mode" for boards that are not yet flashed.

## 4. Honest scope

* A **new MCU firmware** is a multi-week project (toolchain, bootloader, flash
  driver, UI runtime). The realistic "own OS" milestone is
  **FRAME OS as a flashable resource image + a native loader** — and the
  decisive unknown is #3 above.
* A **backup** is only meaningful once we can read the unit's current flash.
  Until then the honest position is: *stock firmware from the vendor is the
  restore source; per-unit backup is blocked on R1/R2/R3.*
* Every flash write is destructive-if-wrong. The app will refuse to write
  without a verified backup, and the UI will say exactly what it is about to do.

## 5. Next actions (in order)

1. Read `PROTOCOL-AUDIT.md` + `SHARKFIN-FLASH-AUDIT.md` (agents in flight).
2. Probe (read-only) any flash-read opcode they identify; extend `opdump` with
   a targeted, argument-carrying probe.
3. Download the stock firmware image (R4) and run it through REA
   (`inspect-firmware-regions` → Ghidra).
4. Answer question #3 (are the built-in screens flash resources?) — that
   decides the whole project shape.
5. Implement the Firmware/Native-OS pages in the desktop app, and switch the
   shell to the Notion design system.
