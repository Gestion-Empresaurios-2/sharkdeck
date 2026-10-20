# K86 firmware image — what we have and what it is

## Where the images come from

| source | file | size | what |
|--------|------|------|------|
| `https://api2.rongyuan.tech:3816/download/fw_upgrade_file/2730_v115_oledv108_flashv101` | `k86v2.zip` | 120,107 B | **K86 v2**, dev id 2730, fw `v115_oledv108_flashv101` — the closest public image to our board |
| `subhassh/attackshark-keyboard-firmware` | `firmware/2268_v309.bin` | 46,469 B | X65HE, dev 2268, gen2 family (different lineage) |
| vendor channel for **our** dev id **1168** | — | — | **HTTP 500 "Record not found"** — no published firmware for our exact board |

Inside `k86v2.zip`:

* `firmwareFile.bin` (190,040 B) — main application firmware
* `firmwareOledFile.bin` (210,328 B) — **display-controller (OLED) firmware**
* `firmwareFlashFile.bin` (38,594 B) — all `0xFF` (blank/erased region, no strings)

## Container format (custom, vendor-specific)

All three binaries share the same 0x100-byte header:

```
0000: 00 00 03 00 cd 02 00 01  00 00 00 00 e5 03 00 01
0020: 00 00 00 00 00 00 00 00  00 00 00 00 e5 03 00 01
0038: 00 00 00 00 e5 03 00 01  01 00 15 6d 01 00 22 15
0044: 01 00 03 e5 01 00 03 e5  01 00 03 e5 01 00 03 e5
```

* magic `G1H@!:FT2` at offset `0x12f`
* the dword `0x010003e5` repeats as a **chunk-descriptor tag**; sibling dwords
  (`0x0100156d`, `0x01002215`, …) look like per-chunk offsets/lengths
* no ARM vector table is visible in the raw file at any aligned offset, so the
  payload is either packed, encrypted, or the ARM image starts after a header
  table we have not decoded yet

## Carving attempts (all negative)

* `zlib`/`raw-deflate`/`gzip` at every offset 0..128 → no stream
* `unblob 26.6.4` (with its full handler set) → **zero carved chunks**; only
  repeated "File format is invalid (lzma)" probes
* `binwalk` 2.1.0 (uv install) is broken in this environment
  (`ModuleNotFoundError: No module named 'binwalk.core'`) → needs a pip venv
  reinstall; REA's `inspect-firmware-regions` therefore cannot run yet
* `rea extract-firmware` works (unblob provider) but finds nothing to extract
  for the same reason
* `rea` itself needs **Node ≥ 22.19** (`node:zlib` `crc32` export) — the repo's
  Node 20 fails; use the toolchain's `node22`

## What the strings already prove

```
..\..\..\Libraries\sdk\yc_sysctrl.c
```

repeated throughout both binaries → the firmware is built on the **YC SDK**
(`yc_sysctrl.c` is the SDK's system-control module). That is the same vendor
lineage as the `yc500` family our board belongs to (registry id 1168 =
`yc3121_sg8967_3m_soc`), and it matches the live protocol fingerprints we
measured (`0x8A` → all `0xFF`, echoless `0x89/0x90/0x8B` page reads).

## Consequences for the three goals

1. **Backup of the factory software: impossible over HID.** The dispatcher has
   no flash-read command (see `SHARKFIN-FLASH-AUDIT.md`). The vendor can only
   *write* a signed/packed image (boot entry `0x7F 55 AA 55 AA` → boot PID
   3151:4016 → raw `0xBA` init/data/verify). So the honest backup is the
   **settings snapshot** (implemented + verified) and, for a true firmware
   restore, the *stock image* for a matching dev id.
2. **A native FRAME OS for the keyboard** requires writing our own payload in
   the vendor's packed format and replacing the **display-controller** image
   (`firmwareOledFile.bin` is the OLED chip's own firmware — the built-in
   screens and their UI live on that side of the system). That is a
   high-risk, weeks-long reverse-engineering project, and it starts with
   decoding this container.
3. **Desktop control of the keyboard's native OS** is the pragmatic path and is
   what we ship: everything the dispatcher exposes (frame pipeline, lighting,
   keymap, macros, options, profiles) plus the read-only opcode forensics
   above.

## Next steps (ordered, all safe unless stated)

1. Decode the `0x010003e5` chunk table → find the ARM payload offsets, then
   re-run the vector-table scan on the real payload.
2. Repair binwalk in a venv so `rea inspect-firmware-regions` works; re-inspect
   both K86 v2 binaries for embedded resources (the built-in screen bitmaps
   would appear as 240×135 RGB565 blobs = 64,800 B each).
3. Search `firmwareOledFile.bin` for 240×135 image blobs and for the UI strings
   (menus, "clock", "brightness") to learn how the built-in screens are drawn.
4. Only then consider writing a custom OLED image — with a settings backup and
   a confirmed restore path in hand.
