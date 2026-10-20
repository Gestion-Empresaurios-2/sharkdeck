# PROTOCOL-AUDIT — Attack Shark K86 (ROYUAN family) HID protocol

**Target device.** Attack Shark K86, USB `VID:PID = 3151:4015`, ROYUAN "Gaming Keyboard".
Vendor device id reported by opcode `0x8F` = **1168** (`0x490`). Registry name
`yc3121_sg8967_3m_soc`, family **yc500**, internal layout `Common89_SG8967_v2`, 3 onboard
profiles, panel **240 × 135, mode 16 (RGB565), 5 picture slots**.

**Audit date.** 2026-10-20. **Author.** subagent, protocol research pass.

**Ground rules honoured.** Every row below is traceable to a public source URL + `file:line`,
to this repo's own captured bytes (`sharkdeck/re/PLAN.md:22-26`), or is explicitly marked
`UNVERIFIED` / `INFERRED`. No opcode, offset or layout appears here that is not in a source.

---

## 0. Executive summary

| Question | Answer |
|---|---|
| Distinct opcode values documented | **92** — 70 app-mode yc500-lineage (§2.1), 3 further app-mode values seen live but undocumented (§2.5), 8 gen2-only (§2.2), 6 receiver (§1.4), 5 boot/display-flash (§2.3) |
| Confirmed exercised on the K86 or its firmware images | **32**, plus **4** newly observed on the live board by this repo (`0x82`, `0x83`, `0x84`, `0x8D`) — of which `0x82` and `0x8D` appear in **no** public source |
| **Can we READ firmware bytes back over HID?** | **NO.** No documented opcode reads flash. Boot mode is write-and-verify only. Explicitly: *"The dispatcher has no read. Flash contents never come out over USB."* (`sharkfin/docs/PROTOCOL.md:1031`) |
| Can we back up **settings**? | **YES** — keymap, Fn layer, macros, lighting, options, sleep, debounce, auto-OS all read back faithfully. This is the only backup the protocol supports. |
| Native-OS control surface | **Partial.** 12 opcodes touch the display subsystem (frame pipeline, clock, language, weather, system-info telemetry, picture/animation index, display params). **No** documented opcode selects which stored picture the built-in menu shows, sets brightness or contrast, or controls a screensaver. |
| Highest-value unknown | `0x8D` and `0x82` — undocumented opcodes that answer on the live board (`PLAN.md:23-24`). Also `0x20`/`0xA0` "picture index", the strongest candidate for the native OS's slot selector. |

### 0.1 Corrections to the brief's "already-known" list

| Brief said | Correct value | Why |
|---|---|---|
| `0x87/0x88` light get | **correct** | `shark-k86-mac/k86/device.py:26-27` |
| `0x06/0x86` options | **correct for yc500**; `0x86` is *debounce* on gen2 | `sharkfin/docs/PROTOCOL.md:400-401` |
| `0x05/0x85` profile | **correct for yc500**; gen2 uses `0x04/0x84` | `sharkfin/docs/PROTOCOL.md:379-390` |
| `0x89` keymap page | **correct for yc500**; gen2 uses `0x8A`. `0x89` is *options* on gen2 | `sharkfin/docs/PROTOCOL.md:379-390` |
| `0x13` keymap slot set | **correct for yc500**, but not every yc500 image implements it — some drop the packet (fall back to bulk `0x09`) | `sharkfin/docs/PROTOCOL.md:186-194` |
| `0x90` fn page | **correct for yc500**; gen2's `0x90` has a *different shape* (`[os, profile, 0xFF, page]`) | `sharkfin/docs/PROTOCOL.md:212` |
| `0x16` macro set | **correct for yc500**; gen2 uses `0x0B` | `sharkfin/docs/PROTOCOL.md:445` |
| `0x8B` macro get | **correct**, shared by both families | `sharkfin/docs/PROTOCOL.md:468` |
| `0xA5` screen handshake | **correct** = 165 decimal | `shark-k86-mac/k86/screen.py:13` |
| `0x25` screen data | **correct** = 37 decimal | `shark-k86-mac/k86/screen.py:14` |
| `0x0E` display params | **real, but single-sourced**; it lives on a *numbered* report (id 13), 41-byte packet, not the 64-byte feature report | `keyboard-studio/Sources/KeyboardKit/RoyuanProtocol.swift:19,23-36` |
| — | **NEW:** `0x0C` is **not** "SET_USERPIC per-key RGB" on gen2 in the yc500 shape. On **yc500 (our K86)** the header is `[0x0C, 0, lenLo, lenHi, page, 0, 0, ck7]` + 56 data bytes at offset 8, total 384 bytes (128 keys × RGB). The gen2 header is completely different. | `sharkfin/docs/PROTOCOL.md:475-485` |
| — | **NEW:** debounce GET is **`0x91`**, not `0x81`. | `keyboard-studio/.claude/docs/feature-parity.md:69` says `0x81`; `sharkfin` says `0x91` (`cmd.rs:29`, `PROTOCOL.md:400`); the live sweep shows `0x91` answering and `0x81` absent (`PLAN.md:23-24`). **`0x81` is wrong.** |
| — | **NEW:** the K86's panel is **240 × 135**, not 128 × 128. `shark-k86-mac` and `keyboard-studio` both inherited 128×128 from an older reference; the vendor device record and the K86-specific Spotify implementation both say 240×135. | `sharkfin` `data/devices.json` (id 1168, `screen.w=240, screen.h=135`); `Xynthera/AttackShark_K86_Spotify/spotify_k86.py:18` |

---

## 1. Transport, framing and checksums

### 1.1 The wire

| Property | Value | Source |
|---|---|---|
| Control collection | usage page `0xFFFF`, usage `0x0002` (interface 2) | `shark-k86-mac/PROTOCOL.md:14`; `sharkfin/docs/PROTOCOL.md:11-13` |
| Report type | HID **feature** reports, report id `0`, 64-byte payload both directions | `shark-k86-mac/PROTOCOL.md:19-21` |
| hidapi buffer | 65 bytes: a leading `0x00` report-id byte, then the 64 payload bytes | `shark-k86-mac/PROTOCOL.md:20-24` |
| Reply echo | GET replies echo the opcode at **payload byte 0** — *except* bulk page reads (keymatrix, Fn, macro, userpic) which return **raw page bytes with no echo** | `sharkfin/docs/PROTOCOL.md:14-15` |
| Unsupported opcode | **returns the previous reply, not an error.** Always diff against a baseline captured immediately before | `sharkfin/docs/PROTOCOL.md:1041-1043`; `sharkfin/docs/PROTOCOL.md:1044-1045` |
| Reply latency | ~10 ms settle after send (`sharkfin` blog); ~16 ms measured on a real board | `dnim.dev` blog; `attackshark-r85he-linux/README.md` ("Reply latency") |
| Cached reads | after a write, a read on the *same* connection returns the **previous** reply. Re-open the device to verify a write | `keyboard-studio/Sources/KeyboardKit/Keymap.swift:88-94` |

**Byte-offset convention used in this document.** Offsets are **payload** offsets, i.e. the
64 bytes *after* the report-id byte. In the Python reference (`shark-k86-mac`) the hidapi
buffer includes the report id, so `resp[k]` there = `payload[k-1]` here. `keyboard-studio`
states this explicitly at `Sources/KeyboardKit/RoyuanProtocol.swift:9-13`.

### 1.2 Checksums

`0xFF - (sum & 0xFF)` — firmware complement, not a rolling sum.

| Mode | Covers | Stored at | Used by | Source |
|---|---|---|---|---|
| **Bit7** | payload `0..=6` | byte **7** | every app-mode command except the two lighting sets | `sharkfin/docs/PROTOCOL.md:96-104`; `shark-k86-mac/PROTOCOL.md:25-29` |
| **Bit8** | payload `0..=7` | byte **8** | `SET_LEDPARAM` (0x07), `SET_SLEDPARAM` (0x08) only | `sharkfin/docs/PROTOCOL.md:96-104` |
| **none** | — | — | USB boot data path; OLED/TFT data reports | `sharkfin/docs/PROTOCOL.md:104` |

`shark-k86-mac` states the split the same way: *"Light commands use Bit8; everything else uses
Bit7."* (`PROTOCOL.md:28-29`).

The **yc3123** firmware (the *other* K86, PID `3151:5002`) checks Bit7/Bit8 with the same
split before dispatch (`0x133b0` / `0x1357c`) — `sharkfin/docs/PROTOCOL.md:105`. Our K86
(PID 4015, id 1168) is the **yc3121 / yc500** lineage and is not covered by that image, so the
split is inherited from the family, not read out of our board's own firmware. `INFERRED`.

### 1.3 There are TWO command families, and opcodes collide

`sharkfin/docs/PROTOCOL.md:120-133`:

> *"Opcodes collide: yc500 `SET_KEYMATRIX` is gen2 `SET_KBOPTION`, yc500 `SET_KBOPTION` is gen2 `SET_DEBOUNCE`, yc500 `SET_DEBOUNCE` is gen2 `SET_SLEEPTIME`. A misaddressed write lands on a live register."*

| command | yc500 (our K86) | gen2 | payload |
|---|---|---|---|
| identify | `0x8F` | `0x8F` | same |
| revision | `0x80` | `0x80` (zeros over cable) | see §2.2 |
| LEDPARAM set/get | `0x07`/`0x87` | same | same |
| SLEDPARAM set/get | `0x08`/`0x88` | same | same |
| USERPIC set/get | `0x0C`/`0x8C` | same opcode | **different header** |
| FN set/get | `0x10`/`0x90` | same opcode | **different shape** |
| macro get | `0x8B` | same | same |
| auto-OS set/get | `0x17`/`0x97` | same | same |
| reset | `0x02` | `0x01` | bare opcode |
| profile set/get | `0x05`/`0x85` | `0x04`/`0x84` | same shape |
| keymatrix set/get | `0x09`/`0x89` | `0x0A`/`0x8A` | **different shape** |
| debounce set/get | `0x11`/`0x91` | `0x06`/`0x86` | value at a different byte |
| sleep set/get | `0x12`/`0x92` | `0x11`/`0x91` | GET offset differs |
| options set/get | `0x06`/`0x86` | `0x09`/`0x89` | yc500 bitfield; gen2 decoded fields |
| macro set | `0x16` | `0x0B` | same shape |
| single-slot key write | `0x13`/`0x15` | via keymatrix opcode | |
| report rate | absent | `0x03`/`0x83` | |

**Our board is yc500.** Evidence:
1. Registry: id 1168, `"family": "yc500"`, `"internalName": "yc3121_sg8967_3m_soc"`
   (`sharkfin/app/src-tauri/data/devices.json`).
2. Live sweep: `0x89` / `0x90` / `0x8B` answer as **echoless page reads** and `0x8A` answers
   **all-`0xFF`** — exactly the yc500 signature documented at `sharkfin/docs/PROTOCOL.md:201-203`:
   *"On yc500, `0x8A` (the gen2 keymatrix GET) answers all `0xFF`, not an echo."*
3. This repo's own `core/src/protocol.rs:20` already declares the yc500 shapes.
4. `keyboard-studio` uses `0x05/0x85`, `0x13/0x15`, `0x09`, `0x16` — all yc500.

> **CONFLICT (worth reading).** `sharkfin/docs/PROTOCOL.md:799-801` says *"yc3123 boards sit in
> the gen2 family"* and documents a K86 (id **2730**, PID `3151:5002`) as yc3123/gen2 with a
> **u32** frame-length field. That is a **different K86 SKU** with a different USB PID. Our
> board (`3151:4015`, id 1168) is the yc3121/yc500 K86. Both K86s report 240×135, 5 slots.
> For 240×135 RGB565 the frame is 64 800 bytes = `0xFD40`, so the u16 at `[4],[5]` holds it
> completely and the u32 high bytes `[16],[17]` are **zero**. **The length-width conflict is
> therefore moot for the K86's native resolution** — both readings agree. `INFERRED` from
> `0xFD40 < 0x10000`.

### 1.4 The 2.4 GHz receiver is a different wire

| opcode | meaning | source |
|---|---|---|
| `0xF7` | status | `sharkfin/docs/PROTOCOL.md:69` |
| `0xF6` | select relay target: `[0xF6, 10]` = keyboard, `5` = mouse, `13` = both | `sharkfin/docs/PROTOCOL.md:70` |
| `0xFC` | release a relayed reply for reading | `sharkfin/docs/PROTOCOL.md:71` |
| `0xF1` | unsupported on `4011`, replays previous reply | `sharkfin/docs/PROTOCOL.md:72` |
| `0xF0` | unsupported on `4011`, replays previous reply | `sharkfin/docs/PROTOCOL.md:73` |
| `0xFB` | not a version; carries link state | `sharkfin/docs/PROTOCOL.md:74` |

Receiver status reply: `[0]=1` reply waiting, `[1]` keyboard battery %, `[3]` keyboard online
when 0, `[5]` will accept a relayed command, `[6]` 1=kb 2=mouse 3=both, `[7],[8]` RF boot
flags — `sharkfin/docs/PROTOCOL.md:78-88`.

**Bluetooth exposes no settings collection at all** (`sharkfin/docs/PROTOCOL.md:16`).
Everything in this document is **cable-only**.

---

## 2. MASTER OPCODE TABLE

**Legend.** Dir = GET (read) / SET (write). Cksum = checksum mode.
"Applies to K86" = usable on device 1168 / PID 4015.
`HW` = verified on plugged-in hardware, `FW` = read out of a firmware image, `JS` = vendor
host code only (never exercised), per sharkfin's evidence convention
(`sharkfin/docs/PROTOCOL.md:8-13`).

### 2.1 App mode — K86's own lineage (yc500), 70 opcodes

#### 2.1.1 Identity and information

| Hex | Dec | Name | Dir | Cksum | Exact layout / reply | Applies to K86 | Source |
|---|---|---|---|---|---|---|---|
| `0x8F` | 143 | `GET_USB_VERSION` / identify | GET | Bit7 | send `[0x8F]`; reply `[0x8F, id:u32 LE, …]`. id=1168 for us. gen2's version word is at reply bytes 7..9 | yes | `sharkfin/docs/PROTOCOL.md:162-164`; `sharkfin/.../protocol/cmd.rs:27`; `keyboard-studio/.../RoyuanProtocol.swift:40` |
| `0x80` | 128 | `GET_REVISION` | GET | Bit7 | send `[0x80]`; reply `[0x80, rev_lo, rev_hi]`, version = `(reply[2]<<8)\|reply[1]`. Each nibble is a decimal digit, so `02 01` = 0x102 = "102" | yes | `sharkfin/docs/PROTOCOL.md:431,437-439`; `shark-k86-mac/PROTOCOL.md:34` |
| `0xAD` | 173 | `GET_OLED_VERSION` (display chip fw) | GET | Bit7 | send `[0xAD]`; a real version means the board *has* a display; an echo means it does not. gen2 boards without a display answer with a zero word | yes | `sharkfin/docs/PROTOCOL.md:747-750`; `sharkfin/.../protocol/cmd.rs:36` |
| `0xAE` | 174 | `GET_MLED_VERSION` (second chip) | GET | Bit7 | send `[0xAE]`; `(reply[2]<<8)\|reply[1]`, zero for none | yes | `sharkfin/docs/PROTOCOL.md:752-754`; `sharkfin/.../protocol/cmd.rs:40` |

#### 2.1.2 Profiles

| Hex | Dec | Name | Dir | Cksum | Exact layout / reply | Applies to K86 | Source |
|---|---|---|---|---|---|---|---|
| `0x05` | 5 | `SET_PROFILE` | SET | Bit7 | `[0x05, profile]` — one byte, profile index. No reply, no read-back | yes | `sharkfin/docs/PROTOCOL.md:379-390`; `sharkfin/.../protocol/cmd.rs:4`; `keyboard-studio/.../Keymap.swift:158-165` |
| `0x85` | 133 | `GET_PROFILE` | GET | Bit7 | send `[0x85]`; reply byte 1 = current index, zero-based. Board holds 3 | yes | `sharkfin/docs/PROTOCOL.md:379-390`; `sharkfin/.../protocol/cmd.rs:20` |

#### 2.1.3 Options and device settings

| Hex | Dec | Name | Dir | Cksum | Exact layout / reply | Applies to K86 | Source |
|---|---|---|---|---|---|---|---|
| `0x06` | 6 | `SET_KBOPTION` | SET | Bit7 | `[0x06, 0, flags, fnMatrix, powerSave]`. flags bit0=Win lock, bit1=Mac mode, bit2 unused, bit3=WASD swap, bit4=main LED off, bit5=side LED off, bit6=keyboard mode, bit7=keyboard lock. byte3 bit0 = Fn-matrix flag. byte4 = power save. **Read-modify-write** | yes | `sharkfin/docs/PROTOCOL.md:417-425`; `shark-k86-mac/PROTOCOL.md:78-91`; `sharkfin/.../protocol/cmd.rs:5` |
| `0x86` | 134 | `GET_KBOPTION` | GET | Bit7 | send `[0x86, 0]`; reply byte2=flags, byte3=Fn matrix, byte4=power save. **Bit 128 locks the whole keyboard** | yes | `sharkfin/docs/PROTOCOL.md:417-425`; `shark-k86-mac/k86/device.py:104-107` |
| `0x11` | 17 | `SET_DEBOUNCE` | SET | Bit7 | `[0x11, 0, ms]` — value at **byte 2** (yc500 pads a zero at byte 1). Range 1..10 | yes | `sharkfin/docs/PROTOCOL.md:394-404`; `sharkfin/.../protocol/cmd.rs:12` |
| `0x91` | 145 | `GET_DEBOUNCE` | GET | Bit7 | send `[0x91]`; reply **byte 2** = value | yes | `sharkfin/docs/PROTOCOL.md:400`; `sharkfin/.../protocol/cmd.rs:29` |
| `0x12` | 18 | `SET_SLEEPTIME` | SET | Bit7 | `[0x12]` + four **u16 LE** seconds at bytes 8..16: sleep BT, sleep 2.4G, deep BT, deep 2.4G | yes | `sharkfin/docs/PROTOCOL.md:406-415`; `sharkfin/.../protocol/mod.rs` (`SleepTimes::to_packet`) |
| `0x92` | 146 | `GET_SLEEPTIME` | GET | Bit7 | send `[0x92]`; **four u16 LE at reply bytes 1..9** (NOT the SET offsets — the vendor's encoder and decoder disagree) | yes | `sharkfin/docs/PROTOCOL.md:412`; `sharkfin/.../protocol/cmd.rs:31` |
| `0x17` | 23 | `SET_AUTO_OS` | SET | Bit7 | `[0x17, 0\|1]` | yes | `sharkfin/docs/PROTOCOL.md:433`; `sharkfin/.../protocol/cmd.rs:17` |
| `0x97` | 151 | `GET_AUTO_OS` | GET | Bit7 | send `[0x97]`; `reply[1] == 1` when auto-OS is on | yes | `sharkfin/docs/PROTOCOL.md:433`; `sharkfin/.../protocol/cmd.rs:31` |
| `0x02` | 2 | `SET_RESET` (factory reset) | SET | Bit7 | **bare opcode**, no payload. Needs ~4 s to settle. Wipes every profile, keymap, macro and light setting | yes — **DESTRUCTIVE** | `sharkfin/docs/PROTOCOL.md:432`; `sharkfin/.../protocol/cmd.rs:3` |

#### 2.1.4 Lighting

| Hex | Dec | Name | Dir | Cksum | Exact layout / reply | Applies to K86 | Source |
|---|---|---|---|---|---|---|---|
| `0x07` | 7 | `SET_LEDPARAM` (main) | SET | **Bit8** | `[0x07, mode, speed, brightness, (option<<4)\|flags, R, G, B, ck8]`. brightness 0..4 (>4 rejected). speed on the wire is `5 - host` (host 0..4 → wire 5..1). flags low nibble: 0..6 = preset colour index (red, green, blue, orange, magenta, yellow, white), 7 = fixed colour, 8 = rainbow | yes | `sharkfin/docs/PROTOCOL.md:238-250`; `shark-k86-mac/PROTOCOL.md:43-61`; `shark-k86-mac/k86/device.py:29,122-136` |
| `0x87` | 135 | `GET_LEDPARAM` | GET | Bit7 | send `[0x87]`; reply mirrors the SET layout: `[0x87, mode, speed, brightness, flags, R, G, B]`. A flags nibble of 0..6 is a preset that **overrides** the RGB | yes | `shark-k86-mac/PROTOCOL.md:34`; `sharkfin/docs/PROTOCOL.md:250-251` |
| `0x08` | 8 | `SET_SLEDPARAM` (side/edge) | SET | **Bit8** | same layout as `0x07`. **Speed is NOT inverted here.** Modes: 0 off, 1 static, 2 breathing, 3 neon, 4 wave, 5 snake. Pure white `(255,255,255)` is transmitted as `(250,255,250)` | partly — registry says `sideLight: false` for id 1168, but the K86 physically has side strips and `0x88` answers | `sharkfin/docs/PROTOCOL.md:340-346`; `shark-k86-mac/PROTOCOL.md:63-66`; `shark-k86-mac/k86/device.py:30,137-152` |
| `0x88` | 136 | `GET_SLEDPARAM` | GET | Bit7 | send `[0x88]`; reply mirrors SET. **A board with no edge LEDs still answers this with a plausible state** | yes (answers) | `shark-k86-mac/PROTOCOL.md:35`; `sharkfin/docs/PROTOCOL.md:347-348` |
| `0x0C` | 12 | `SET_USERPIC` (per-key RGB) | SET | Bit7 | **yc500 header**: `[0x0C, 0, lenLo, lenHi, page(0..6), 0, 0, ck7]` + 56 data bytes at offset 8. Total 384 bytes = 128 slots × RGB, 7 pages. Matrix order. The `page` byte is the index; data is copied at `page × 56` | yes | `sharkfin/docs/PROTOCOL.md:475-485`; `sharkfin/.../protocol/mod.rs:503-518`; `keyboard-studio/.../PerKeyLighting.swift:15,79-116` |
| `0x8C` | 140 | `GET_USERPIC` | GET | Bit7 | send `[0x8C, 0, page]` (page index at **byte 2**), six raw 64-byte pages back. **Returns stable data that does not reflect writes — treat as write-only** | yes (but useless) | `sharkfin/docs/PROTOCOL.md:496-497`; `sharkfin/.../protocol/mod.rs:520-523` |
| `0x18` | 24 | animated per-key light | SET | Bit7 | — | UNVERIFIED, no layout published | `keyboard-studio/.../DangerousCommands.swift:23-30` |
| `0x19` | 25 | animated per-key light | SET | Bit7 | — | UNVERIFIED, no layout published | `keyboard-studio/.../DangerousCommands.swift:23-30` |

**Per-key RGB pacing** (this is a flash write and it will wedge the control endpoint):

| Cadence | Result | Source |
|---|---|---|
| 7 reports / 500 ms | endpoint dead after ~13 reports | `sharkfin/docs/PROTOCOL.md:499-505` |
| 7 reports / 3 s | 42 reports, still responsive | same |
| 2 uploads / 5 s, pages 40 ms apart, mode switch 600 ms after | stalls on the second | same |
| **2 uploads / 10 s, pages 100 ms apart, 2 s settle, no redundant mode switch** | **survives — this is the shipping rate limit** | same |

`sharkfin` hard-codes `FLASH_COOLDOWN = 10 s`, `FLASH_PAGE_GAP = 100 ms`, `FLASH_SETTLE = 2 s`
(`sharkfin/.../commands.rs:94-107`); `keyboard-studio` uses the same numbers
(`Sources/KeyboardKit/PerKeyLighting.swift:23-40`).

**Showing the pattern**: light mode **13**, with the pattern slot in the option nibble and RGB
forced to `(0, 200, 200)` — `sharkfin/docs/PROTOCOL.md:487-488`; `keyboard-studio` writes
`slot << 4` into byte 4 and RGB `(0,200,200)`
(`Sources/KeyboardKit/PerKeyLighting.swift:118-129`). On **yc500 the slot nibble is ignored**
(*"That slot is the gen2 USERPIC slot; yc500 USERPIC has no slot byte"* —
`sharkfin/docs/PROTOCOL.md:487-488`); writing it anyway is harmless.

**Colour floor**: any RGB below `0x080808` renders as unlit; "off" is an options bit, not a
black write — `sharkfin/docs/PROTOCOL.md:249`; `sharkfin/.../protocol/mod.rs` (`COLOR_FLOOR`).

#### 2.1.5 Keymap, Fn layer, macros

| Hex | Dec | Name | Dir | Cksum | Exact layout / reply | Applies to K86 | Source |
|---|---|---|---|---|---|---|---|
| `0x89` | 137 | `GET_KEYMATRIX` (page) | GET | Bit7 | send `[0x89, profile, page]`, page 0..7 → 8 raw 64-byte pages, **no opcode echo**. 128 slots × 4 bytes = 512 per layer. Slot values: `[type, mod/arg, code, 0]` where type `0x00`=HID usage, `0x01`=mouse button, `0x03`=consumer usage, `0x09`=macro `[mode,index]`, `0x0A`=special, all-zero=Fn fall-through / dead key | yes | `sharkfin/docs/PROTOCOL.md:168-175,225-236`; `sharkfin/.../protocol/cmd.rs:24`; `keyboard-studio/.../Keymap.swift:13-19` |
| `0x13` | 19 | `SET_KEY_ONE` (single slot) | SET | Bit7 | `[0x13, profile, slot, 0,0,0,0, ck7]` + the 4 slot bytes at **8..12**. Slot 0..255. No unlock, no commit; the vendor blocks its own polling for ~100 ms after, so wait 120 ms | yes, but see caveat | `sharkfin/docs/PROTOCOL.md:177,186-194`; `keyboard-studio/.../Keymap.swift:71-96`; `keyboard-studio/.../DangerousCommands.swift:12-20` |
| `0x09` | 9 | `SET_KEYMATRIX` (bulk layer) | SET | Bit7 | `[0x09, profile, 0xF8, 1, page, 0, 0, ck7]` + 56 data bytes at offset 8, **9 pages, commit on page 8**. Sends 9×56 = 504 of 512 bytes — **slots 126 and 127 never transmit** | yes (fallback) | `sharkfin/docs/PROTOCOL.md:179,182-184`; `sharkfin/.../protocol/mod.rs:171-195` |
| `0x90` | 144 | `GET_FN` (Fn layer page) | GET | Bit7 | send `[0x90, profile, page]`, page 0..7 → raw 64-byte pages, no echo. Same slot encoding as the base layer | yes | `sharkfin/docs/PROTOCOL.md:176`; `keyboard-studio/.../Keymap.swift:140-146` |
| `0x15` | 21 | `SET_FN_ONE` (single Fn slot) | SET | Bit7 | `[0x15, profile, slot, 0,0,0,0, ck7]` + 4 slot bytes at 8..12 | yes | `sharkfin/docs/PROTOCOL.md:178`; `keyboard-studio/.../Keymap.swift:122-137` |
| `0x10` | 16 | `SET_FN` (bulk Fn layer) | SET | Bit7 | same shape as `0x09` bulk, on opcode `0x10` | yes | `sharkfin/docs/PROTOCOL.md:180` |
| `0x16` | 22 | `SET_MACRO` | SET | Bit7 | `[0x16, slot, page, 56, last(0/1), 0, 0, ck7]` + 56 payload bytes at offset 8, zero-padded. 50 slots × 256 bytes. Blob = u16 LE repeat count, then alternating event/delay: key/mouse `[code, pressed<<7\|delay]` (or `[code, pressed?0x80:0, delay u16 LE]` when delay is 0 or >127 ms), mouse move `[0xF9, delay, dx i8, dy i8]`, mouse buttons `0xF0`..`0xF4`, four zero bytes terminate | yes | `sharkfin/docs/PROTOCOL.md:443-466`; `sharkfin/.../protocol/mod.rs:686-700`; `sharkfin/.../protocol/cmd.rs:16` |
| `0x8B` | 139 | `GET_MACRO` | GET | Bit7 | send `[0x8B, slot, page]`, pages 0..3 → raw 64-byte pages. **Read-back is faithful** | yes | `sharkfin/docs/PROTOCOL.md:468-469`; `sharkfin/.../protocol/cmd.rs:25` |

**Caveat on `0x13`/`0x15`.** *"Not every yc500 image has the single-slot writes… `0x13` and `0x15`
fall to the default, which drops the packet."* (`sharkfin/docs/PROTOCOL.md:186-190`). Boards with
`bulkKeymap` in the registry write whole layers on `0x09`/`0x10` instead. **Our K86's single-slot
write is confirmed working** — `keyboard-studio` round-trips it with a re-open to verify
(`Sources/KeyboardKit/Keymap.swift:88-113`, `Sources/kstudio/KeymapCommand.swift` `--set-slot`).

**Keymap → physical key mapping is per board, not per protocol** (`sharkfin/docs/PROTOCOL.md:236`).
On the K86 the knob's three actions live at global slots **96 (turn-left, volume down 0xEA)** and
**97 (turn-right, volume up 0xE9)**, with **mute (0xE2) at slot 78** according to
`keyboard-studio/.claude/docs/knob-screen-menu.md:38-44`; the shipped JSON profile
`keyboard-studio/Sources/KeyboardKit/Resources/Devices/attackshark-k86.json` instead records
`page 6, turnLeft 0, turnRight 1, press 2` = global slots **96/97/98**.
**CONFLICT — 78 vs 98 for the knob press slot. UNRESOLVED.** `keyboard-studio` prefers
*discovery* over hard-coding: it scans pages for `0x03`-type slots carrying consumer usages
0xE9/0xEA/0xE2 (`Sources/KeyboardKit/Keymap.swift:42-63`).

#### 2.1.6 Screen and display subsystem — **read §3 for the full discussion**

| Hex | Dec | Name | Dir | Cksum | Exact layout / reply | Applies to K86 | Source |
|---|---|---|---|---|---|---|---|
| `0xA5` | 165 | screen **announce** (mode 16, RGB565) | SET (answers) | Bit7 | `[0xA5, frame, frames, delay, lenLo, lenHi, 0, ck7, left, top, right, bottom, leftHi, topHi, rightHi, bottomHi, lenB16, lenB24, layer]`. `len` = total RGB565 byte count. Reply: **payload byte 1 == 1 means ready**. Reply echoes the opcode at byte 0 | yes | `shark-k86-mac/PROTOCOL.md:98-117`; `shark-k86-mac/k86/screen.py:34-53`; `keyboard-studio/.../Screen.swift:384-414`; `sharkfin/.../protocol/mod.rs:409-444`; `Xynthera/.../spotify_k86.py:64-80` |
| `0x25` | 37 | screen **data page** (mode 16) | SET | Bit7 | `[0x25, frame, frames, delay, pageLo, pageHi, pageLen, ck7]` + 56 data bytes at offset 8, zero-padded on the last chunk. `pageLen` = bytes actually in this chunk. `frame` doubles as the **picture slot** on a static image | yes | `shark-k86-mac/PROTOCOL.md:119-133`; `shark-k86-mac/k86/screen.py:56-74`; `keyboard-studio/.../Screen.swift:420-441`; `Xynthera/.../spotify_k86.py:82-95` |
| `0xA9` | 169 | screen announce (mode 24, 3 B/px) | SET | Bit7 | as `0xA5` | no (K86 is mode 16) | `sharkfin/docs/PROTOCOL.md:778` |
| `0x29` | 41 | screen data (mode 24) | SET | Bit7 | as `0x25` | no | `sharkfin/docs/PROTOCOL.md:779` |
| `0x0E` | 14 | `GetDisplayParam` | GET | **custom** | **NOT** the 64-byte feature report. 41-byte packet on **numbered report id 13**: `[0x0E, 0x27, 0×38, ck]` where the checksum makes the whole report (report id included) sum to 85. Reply parse: **byte 2 = brightness**, **bytes 17-18 = width (big-endian)**, **bytes 19-20 = height (big-endian)** | yes — but single-sourced, and keyboard-studio reports it usually does not answer | `keyboard-studio/.../RoyuanProtocol.swift:19,23-36`; `keyboard-studio/.../Screen.swift:118-150` |
| `0x28` | 40 | `SET_OLED_CLOCK` | SET | Bit7 | `[0x28, 0×7, ck7]` then **past the checksum**: byte 8-9 = year big-endian, 10 = month (1..12), 11 = day, 12 = hour, 13 = minute, 14 = second. **No read**; the board keeps time itself from there | yes | `sharkfin/docs/PROTOCOL.md:904-925`; `sharkfin/.../protocol/mod.rs:478-501` |
| `0x27` | 39 | display language | SET | Bit7 | — layout not published | yes | `sharkfin/docs/PROTOCOL.md:900` |
| `0x22` | 34 | display options / system-info telemetry | SET | Bit7 | payload = disk free/total, memory used/total, CPU %, CPU temperature, network totals — a one-way feed for the panel's system-info widget. **The vendor app never sends it to this board** | yes, but unused | `keyboard-studio/.claude/docs/knob-screen-menu.md:84-88`; `sharkfin/docs/PROTOCOL.md:900` |
| `0x2A` | 42 | weather | SET | Bit7 | — layout not published. Pairs with the vendor client's "sync weather" screen API | yes | `sharkfin/docs/PROTOCOL.md:897-899`; `keyboard-studio/.claude/docs/knob-screen-menu.md:60-66` |
| `0x2B` | 43 | effect | SET | Bit7 | — layout not published | yes | `sharkfin/docs/PROTOCOL.md:899` |
| `0xAB` | 171 | effect GET | GET | Bit7 | — layout not published | yes | `sharkfin/docs/PROTOCOL.md:899` |
| `0x20` | 32 | picture index | SET | Bit7 | — layout not published. **Strongest candidate for the native OS's "which picture is shown" selector** | yes | `sharkfin/docs/PROTOCOL.md:897` |
| `0xA0` | 160 | picture index GET | GET | Bit7 | — | yes | `sharkfin/docs/PROTOCOL.md:897` |
| `0x21` | 33 | picture data | SET | Bit7 | — layout not published (older picture pipeline, predates `0xA5`) | yes | `sharkfin/docs/PROTOCOL.md:898` |
| `0xA1` | 161 | picture data GET | GET | Bit7 | — | yes | `sharkfin/docs/PROTOCOL.md:898` |
| `0x24` | 36 | animation data | SET | Bit7 | — | yes | `sharkfin/docs/PROTOCOL.md:898` |
| `0xA4` | 164 | animation data GET | GET | Bit7 | — | yes | `sharkfin/docs/PROTOCOL.md:898` |
| `0x26` | 38 | animation index | SET | Bit7 | — | yes | `sharkfin/docs/PROTOCOL.md:898` |
| `0xA6` | 166 | animation index GET | GET | Bit7 | — | yes | `sharkfin/docs/PROTOCOL.md:899` |
| `0x2F` | 47 | `SET_MENU` | SET | Bit7 | payload `[0x2F, loop?1:0, interval]` — the vendor labels it *"Playback Method / Press to Play / Loop / Image Interval"*, i.e. a **slideshow** setting for stored images. **Only implemented by a driver class declaring `type: "dongle"`** — every board carrying the capability flag is a wireless dongle, never a keyboard | **no** | `keyboard-studio/.claude/docs/knob-screen-menu.md:77-82` |
| `0x36` | 54 | image slot addresses | GET | Bit7 | *"0x0E asks the panel for its own resolution and 0x36 for the image slot addresses; both are reads in the vendor's own client."* Layout not published | yes — high-value probe | `keyboard-studio/.../DangerousCommands.swift:36-38` |

#### 2.1.7 Destructive and boot-entry opcodes

| Hex | Dec | Name | Dir | Cksum | Exact layout / reply | Applies to K86 | Source |
|---|---|---|---|---|---|---|---|
| `0x2C` | 44 | display erase | SET | Bit7 | bare-ish. On yc500 images `0x2C` and `0xAC` set the **same** flag; `0xAC` additionally preloads an `AA AA 55 55` reply | yes — **DESTRUCTIVE** | `sharkfin/docs/PROTOCOL.md:743,886-895` |
| `0xAC` | 172 | flash-chip erase | SET | Bit7 | bare-ish. On gen2 it erases every picture and **takes about 55 seconds**. Sits inside the `0x80+` range that otherwise holds only reads | yes — **DESTRUCTIVE, ~55 s** | `sharkfin/docs/PROTOCOL.md:742,888-889`; `keyboard-studio/.../DangerousCommands.swift:7-9` |
| `0x7F` | 127 | USB boot entry | SET | Bit7 | **`[0x7F, 0x55, 0xAA, 0x55, 0xAA]`** — the trailer is mandatory. Board re-enumerates as a boot PID | yes — **DESTRUCTIVE/IRREVERSIBLE** | `sharkfin/docs/PROTOCOL.md:744,941-973`; `keyboard-studio/.../DangerousCommands.swift:12-20` |
| `0x30` | 48 | display-chip boot entry | SET | Bit7 | `[0x30, 0x55, 0xAA, 0x55, 0xAA, 0, 0, 0]` | yes — **DESTRUCTIVE** | `sharkfin/docs/PROTOCOL.md:745,1011-1023` |
| `0x31` | 49 | OLED start (display-chip flash) | SET | Bit7 | `0x31` + the 16-bit chunk count; stays in app mode | yes — **DESTRUCTIVE** | `sharkfin/docs/PROTOCOL.md:901,1019` |
| `0x43`/`0x44`/`0xC3`/`0xC4` | 67/68/195/196 | Nordic bootloader entry / start | SET | ? | — | UNVERIFIED on this protocol | `keyboard-studio/.../DangerousCommands.swift:14` |
| `0x1C`/`0x1E` | 28/30 | magnetic-switch travel calibration | SET | Bit7 | `0x1C` zeroes the stored travel tables in RAM; `0x1E` saves them to flash. On-then-off without pressing every key commits **zeroed** tables | no (K86 is not magnetic) — **DESTRUCTIVE** | `sharkfin/docs/PROTOCOL.md:691`; `keyboard-studio/.../DangerousCommands.swift:15` |

#### 2.1.8 Magnetic-switch family (present in the protocol, NOT on the K86)

The K86 registry record has `"magnetic": false` (`sharkfin/.../data/devices.json`, id 1168).
Listed for completeness / to avoid mis-probing.

| Hex | Dec | Name | Dir | Cksum | Layout | Source |
|---|---|---|---|---|---|---|
| `0xE5` | 229 | read a travel column | GET | Bit7 | `[0xE5, subop, 1, page]`; reply is the raw page, no echo. u16 columns = 4 pages of 32; byte columns = 2 pages of 64 | `sharkfin/docs/PROTOCOL.md:524-536` |
| `0x65` | 101 | write a travel column | SET | Bit7 | `[0x65, subop, 0, slot, last]` value at 8..9 (u16) — or `[0x65, subop, 1, page, last]` + 56 bytes for bulk | `sharkfin/docs/PROTOCOL.md:524-536` |
| `0x1B` | 27 | live travel stream | SET | Bit7 | `0x1B [flag]`; any nonzero starts the stream. Report id 5 on the `0xFFFF` input collection: `[0x1B, lo, hi, slot]` | `sharkfin/docs/PROTOCOL.md:642` |
| `0x1D` | 29 | travel preset | SET | Bit7 | `0x1D [preset]`: 0 comfort, 1 sensitive, 2 gaming, 3 = the columns | `sharkfin/docs/PROTOCOL.md:666` |
| `0x9D` | 157 | travel preset GET | GET | Bit7 | reply `[0x9D, preset, 0×6]` | `sharkfin/docs/PROTOCOL.md:667` |
| `0x1A` | 26 | one record for every key / one slot | SET | Bit7 | byte 5 = all-keys flag, byte 6 = slot; values at 8 (release), 9/10 (rapid trigger), 11 (first point), 12..15 (DKS actions), 16 (mod-tap), 25 (dead zone) | `sharkfin/docs/PROTOCOL.md:668` |
| `0x9F` | 159 | reply of the eight bytes `0x1F` set | GET | Bit7 | unrelated to travel | `sharkfin/docs/PROTOCOL.md:669` |

### 2.2 gen2-family opcodes — same protocol, other boards (8 new values)

> These are **not** for the K86 at 4015. They are listed because the same wire carries them on
> other ROYUAN boards, and because **a mis-addressed write lands on a live register**
> (`sharkfin/docs/PROTOCOL.md:120-133`). The K86's registry family is `yc500`.

| Hex | Dec | Name | Dir | Cksum | Layout | Source |
|---|---|---|---|---|---|---|
| `0x01` | 1 | gen2 factory reset | SET | Bit7 | bare opcode | `sharkfin/docs/PROTOCOL.md:432`; `keyboard-studio/.../DangerousCommands.swift:13` |
| `0x03` | 3 | gen2 report rate SET | SET | Bit7 | — | `sharkfin/docs/PROTOCOL.md:434` |
| `0x04` | 4 | gen2 SET_PROFILE | SET | Bit7 | one byte | `sharkfin/docs/PROTOCOL.md:379-390` |
| `0x0A` | 10 | gen2 keymatrix SET | SET | Bit7 | single slot `[0x0A, profile, slot, 0, 0, apply, os]`, value at 8..12; bulk `[0x0A, profile, 0xFF, page, len, last, os]` + ≤56 bytes, 10 pages. `os` = host layer 0 win, 1 mac, 2 android, 3 ios | `sharkfin/docs/PROTOCOL.md:209-213` |
| `0x0B` | 11 | gen2 SET_MACRO | SET | Bit7 | same packet shape as yc500 `0x16` | `sharkfin/docs/PROTOCOL.md:445` |
| `0x83` | 131 | gen2 report rate GET | GET | Bit7 | — **also answers on our board** (see §2.5) | `sharkfin/docs/PROTOCOL.md:434` |
| `0x84` | 132 | gen2 GET_PROFILE | GET | Bit7 | reply byte 1 = index. **also answers on our board** (see §2.5) | `sharkfin/docs/PROTOCOL.md:379-390` |
| `0x8A` | 138 | gen2 GET_KEYMATRIX | GET | Bit7 | `[0x8A, profile, 0xFF, page, os]` → raw pages. **On yc500 this opcode answers all-`0xFF`** | `sharkfin/docs/PROTOCOL.md:201-203,209` |

### 2.3 Boot mode and display-chip flashing (5 new values)

| Hex | Dec | Name | Dir | Cksum | Layout / reply | Source |
|---|---|---|---|---|---|---|
| `0xBA` | 186 | bootloader init / verify | SET | **none** | init: `[0xBA, 0xC0, cntLo, cntHi, len0, len1, len2]`; verify: `[0xBA, 0xC2, cntLo, cntHi, s0, s1, s2, s3, len0, len1, len2, len3]`. `cnt = ceil(len/64)`, `len` 24-bit, checksum = plain byte sum. Reply opcode `0xAB`, second byte echoes `0xC0`/`0xC2`; **verify sets byte 4 to `0x55` on success, `0xAA` on failure** | `sharkfin/docs/PROTOCOL.md:987-1007` |
| `0xB0` | 176 | OLED poll (display-chip flash) | GET | Bit7 | poll until it echoes and `reply[1] == 1` | `sharkfin/docs/PROTOCOL.md:1018` |
| `0xB1` | 177 | OLED verify | SET | Bit7 | `0xB1` + byte sum; success is `reply[1] == 0x55` | `sharkfin/docs/PROTOCOL.md:1021` |
| `0xB2` | 178 | TFT probe | GET | Bit7 | `0xB2` + page count; continue only if `reply[1] == 1` | `sharkfin/docs/PROTOCOL.md:1022` |
| `0x32` | 50 | TFT data (display-chip flash) | SET | none | 56-byte pages from file offset 65536 | `sharkfin/docs/PROTOCOL.md:1023` |

### 2.4 2.4 GHz receiver — see §1.4 (6 values: `0xF0`, `0xF1`, `0xF6`, `0xF7`, `0xFB`, `0xFC`)

### 2.5 Opcode census

| Bucket | Count | Values |
|---|---|---|
| App mode, K86's yc500 lineage (public sources) | **70** | §2.1.1–2.1.8 |
| App mode, seen live but **undocumented in any public source** | **3** | `0x8D` `0x82` (answer) + `0x81` (disputed, see §2.5) |
| gen2-only (other boards) | **8** | `0x01 0x03 0x04 0x0A 0x0B 0x83 0x84 0x8A` |
| 2.4 GHz receiver | **6** | `0xF0 0xF1 0xF6 0xF7 0xFB 0xFC` |
| Boot / display-chip flash | **5** | `0xBA 0xB0 0xB1 0xB2 0x32` |
| **Total distinct opcode values** | **92** | |

Of these, **32** have been exercised against the K86 (1168) or against a firmware image
identified as a K86: `0x8F 0x80 0xAD 0xAE 0x05 0x85 0x06 0x86 0x11 0x91 0x12 0x92 0x17 0x97
0x07 0x87 0x08 0x88 0x0C 0x89 0x13 0x09 0x90 0x15 0x10 0x16 0x8B 0xA5 0x25 0x28 0x7F 0xBA`.
(`0x09` and `0x10` are read out of firmware, not exercised on hardware; `0x7F` and `0xBA`
were exercised on a K86 by a user, not on a bench — `sharkfin/docs/PROTOCOL.md:934-937`.)
Plus **4** observed live by this repo (`PLAN.md:23-24`): `0x82`, `0x83`, `0x84`, `0x8D`.

**Undocumented / unexplained opcodes (high priority).** All three are **app-mode** values and are
counted separately in the census above; none appears in any public source's opcode table.

| Hex | Observed | Analysis |
|---|---|---|
| `0x8D` | answers on the live board (`PLAN.md:23-24`) | Appears in **no** public source. Not in sharkfin's `cmd.rs`, not in keyboard-studio's `safeToProbe` list, not in the vendor-client opcode census. Highest-value unknown. |
| `0x82` | answers on the live board | Same. Adjacent to `0x80` (revision) / `0x83` (gen2 report rate) / `0x84` (gen2 profile). |
| `0x81` | listed as "safe to probe" by keyboard-studio; **did not** answer on the live board | `keyboard-studio/.claude/docs/feature-parity.md:69` claims debounce GET is `0x81`. sharkfin says `0x91` (`cmd.rs:29`, `PROTOCOL.md:400`). The live sweep found `0x91` answering and `0x81` absent. **`0x81` is very likely a doc error.** |
| `0x83`, `0x84` | answer on the live board | Documented only as gen2 opcodes (report rate GET, profile GET). On a yc500 board they are unassigned, so an echo is surprising. Two readings: (a) the K86's firmware is more gen2-ish than the registry says, or (b) the sweep's "implemented" test is fooled by the previous-reply behaviour. **Must be re-measured with a baseline diff.** |

---

## 3. FLASH / FIRMWARE

### 3.1 The verdict

> ### **No documented opcode can read firmware bytes back from the device.**
>
> Direct quote — `sharkfin/docs/PROTOCOL.md:1031` (Hazards table, "No USB dump"):
> *"The dispatcher has no read. Flash contents never come out over USB."*
>
> And again at `sharkfin/docs/PROTOCOL.md:1005-1007`:
> *"Only pass/fail and the running error count leave over USB. **There is no read command in boot mode.**"*

**What this means for the plan (PLAN.md §1).** Route **R1 (HID flash-read opcode) does not
exist** for this protocol. R1 must be closed, not pursued. Routes R2 (bootloader) and R3
(SWD/JTAG) remain the only per-unit firmware-read routes, and R2 is **write-only** — entering
the bootloader lets you *write* an image, never *read* the current one.

### 3.2 The complete flash-related opcode inventory

| Hex | Dec | Name | Dir | What it does | Timing / danger |
|---|---|---|---|---|---|
| `0xAC` | 172 | flash-chip erase | SET | Erases the flash chip. On gen2 it erases **every stored picture** and **takes about 55 seconds**. `0xAC` additionally preloads an `AA AA 55 55` reply | ~55 s, **irreversible**, wipes screen images and settings. DANGER: 5/5 |
| `0x2C` | 44 | display erase | SET | yc500 display erase; on the images read it sets the **same flag** as `0xAC` | Same flag → same outcome. DANGER: 5/5 |
| `0x0C` | 12 | `SET_USERPIC` | SET | Writes 384 bytes (128 keys × RGB) of per-key colour into flash, 7 pages | Flash write. Pace: 100 ms between pages, 2 s settle, **10 s between uploads**. Endpoint wedges if exceeded. DANGER: 2/5 (boring) |
| `0x09` | 9 | `SET_KEYMATRIX` bulk | SET | Writes a whole 512-byte keymap layer to flash; commit on page 8 | Flash write, same pacing class |
| `0x10` | 16 | `SET_FN` bulk | SET | Same, for the Fn layer | Flash write |
| `0x13` / `0x15` | 19/21 | `SET_KEY_ONE` / `SET_FN_ONE` | SET | Writes one 4-byte slot to flash | Flash write. 120 ms settle after. DANGER: 2/5 |
| `0x16` | 22 | `SET_MACRO` | SET | Writes a 256-byte macro slot to flash | Flash write |
| `0x0C` (gen2) | 12 | `SET_USERPIC` gen2 header | SET | 378 bytes (126 keys), 7 pages, staged at `page × 56`, committed to flash at `0x0802F800 + slot × 384` when page 6 carries the final flag | Flash write |
| `0x18` / `0x19` | 24/25 | animated per-key light | SET | — | UNVERIFIED; listed as flash-writing by `keyboard-studio/.../DangerousCommands.swift:23-30` |
| `0x22` / `0x27` / `0x28` | 34/39/40 | display options / language / clock | SET | `0x22` is a one-way telemetry feed for the system-info widget. `0x28` sets the panel's clock | `keyboard-studio/.../DangerousCommands.swift:23-30` lists `0x22`, `0x27`, `0x28` as flash-writing |
| `0x1C` / `0x1E` | 28/30 | magnetic calibration on / save | SET | `0x1C` zeroes the travel tables in RAM; `0x1E` saves them to flash | DANGER: 4/5 (commits zeroed calibration) — not applicable to the K86 (not magnetic) |
| `0x02` | 2 | factory reset (yc500) | SET | Wipes every onboard profile, keymap, macro and light setting | ~4 s. DANGER: 4/5 (recoverable, but destroys user config) |
| `0x01` | 1 | factory reset (gen2) | SET | same | DANGER: 4/5. **Not the K86's opcode** — sending `0x01` to a yc500 board lands somewhere else |
| `0x7F` | 127 | USB boot entry | SET | `[0x7F, 0x55, 0xAA, 0x55, 0xAA]`. Board re-enumerates as a boot PID | DANGER: 5/5 |
| `0x30` | 48 | display-chip boot entry | SET | `[0x30, 0x55, 0xAA, 0x55, 0xAA, 0, 0, 0]` | DANGER: 5/5 |
| `0x31` | 49 | OLED start (display-chip flash) | SET | stays in app mode; `0x31` + 16-bit chunk count | DANGER: 5/5 |
| `0xBA` | 186 | bootloader init/verify | SET | Write-only image transfer | DANGER: 5/5 |
| `0xB0`/`0xB1`/`0xB2`/`0x32` | 176/177/178/50 | display-chip / TFT flashing | SET | Write-only | DANGER: 5/5 |

### 3.3 The bootloader, in detail (the only firmware-write path)

**Entering boot** — `sharkfin/docs/PROTOCOL.md:941-973`:

| report | bytes |
|---|---|
| USB | `[0x7F, 0x55, 0xAA, 0x55, 0xAA]` |
| RF | `[0xF8, 0x55, 0xAA, 0x55, 0xAA, 0, 0, 0x82]` |
| display chip | `[0x30, 0x55, 0xAA, 0x55, 0xAA, 0, 0, 0]` |

**The K86 has been flashed end-to-end, once, by a user** — `sharkfin/docs/PROTOCOL.md:934-939`:

> *"Boot entry and the USB image transfer have since been run end to end on a K86 (1168): the
> board entered boot, took 3397 chunks and answered the verify with `0x55`, and came back up on
> the new image. That was a user recovering their own board with a tool built from this file,
> reported by mail, not a round-trip on a bench here."*

- 3397 chunks × 64 B = **217 408 bytes** of payload. The yc500 host convention skips 65 536
  bytes of the file, so the on-flash image is roughly **151 872 bytes (≈148 KB)**. `INFERRED`
  from `PROTOCOL.md:934-937` + `:984`.
- The K86 **re-enumerates as `3151:4016`**, which is in *neither* vendor boot-PID table
  (`sharkfin/docs/PROTOCOL.md:1030`). Whether boot PID = app PID + 1 in general is untested.
- **No read path.** The bootloader dispatcher at `0x1000676` handles only `0xC0BA` (init) and
  `0xC2BA` (verify). Chunks are written at the flash base + `index << 6` with a 64-byte
  read-back compare. *"Only pass/fail and the running error count leave over USB."*
  (`sharkfin/docs/PROTOCOL.md:1001-1007`)

**Hazards** (`sharkfin/docs/PROTOCOL.md:1027-1033`):

| | |
|---|---|
| Sibling images | *"Same USB PID does not mean the same PCB. A wrong image that fails to run can leave the board unable to answer `0x7F`, so boot is unreachable without SWD."* |
| Boot PID table | The vendor tables are incomplete |
| No USB dump | The dispatcher has no read |
| Bootloader is kept | *"A board that can still enter boot can be written again."* |

### 3.4 What was searched for a flash-read opcode

| Searched | Result |
|---|---|
| `sharkfin/docs/PROTOCOL.md` — 1053 lines, the full protocol bible with 1409 devices | No read opcode. Explicit "No USB dump" hazard row (`:1031`) |
| `sharkfin/app/src-tauri/src/protocol/` — `cmd.rs` (46 lines), `mod.rs` (963), `gen2.rs`, `hall.rs`, `receiver.rs` | No flash-read opcode in any builder |
| `sharkfin/app/src-tauri/data/devices*.json` (1.26 MB registry), `led-flags.json`, `light-layouts.json` | No flash-read opcode referenced |
| `keyboard-studio/Sources/KeyboardKit/` — `RoyuanProtocol.swift`, `Screen.swift`, `Knob.swift`, `Keymap.swift`, `Shortcuts.swift`, `Keyboard.swift`, `HIDTransport.swift`, `PerKeyLighting.swift`, `DangerousCommands.swift` | No flash-read opcode. `DangerousCommands.safeToProbe` lists 19 opcodes, all reads of state, none of firmware |
| `shark-k86-mac/` — `PROTOCOL.md`, `k86/device.py`, `k86/screen.py`, `k86/cli.py` | No flash-read opcode. README: *"it does not flash firmware"* |
| `dnim.dev` blog (`/md/blog/royuan-keyboard-protocol.md`) — how the author got firmware | The firmware came from a **community archive of rescued images**, inflated with raw-deflate, not from the device (`blog`, "When there is no software to capture") |
| `Xynthera/AttackShark_K86_Spotify` | Screen-only, no flash |
| `TarikTopalovic/attackshark-r85he-linux` | *"Firmware upgrade ⛔ blocked on purpose: the driver refuses to fetch firmware."* |
| This repo's live sweep, `0x80..=0x9F` (`PLAN.md:22-26`) | *"nothing in the GET family returns a block of firmware bytes"* |
| GitHub repo search: `royuan`, `3151 4015`, `attack shark keyboard driver`, `K86 keyboard tft hid`, `keyboard hid protocol royuan` | Only the 5 repos cited in this document + `attackshark-r85he-linux` |

### 3.5 What *can* be backed up (settings, not firmware)

Everything the board stores as **configuration** reads back faithfully:

| What | How |
|---|---|
| Keymap, 3 profiles × 512 B | `0x89 [profile, page 0..7]` → 8 raw pages |
| Fn layer, 3 profiles | `0x90 [profile, page 0..7]` |
| Macros, 50 slots × 256 B | `0x8B [slot, page 0..3]` → 4 raw pages; *"Read-back is faithful"* |
| Main + side lighting | `0x87`, `0x88` |
| Options bitfield | `0x86` |
| Debounce / sleep / auto-OS | `0x91`, `0x92`, `0x97` |
| Profile index, device id, revision | `0x85`, `0x8F`, `0x80` |

That is exactly what `sharkfin`'s "Backup" feature does — *"Export everything to a file and
restore it later"* (`sharkfin/README.md`, Features). **The honest product framing is therefore:
"Back up and restore the keyboard's *settings*", not "back up the firmware".** The firmware
restore source is the vendor's stock image (PLAN.md route R4).

---

## 4. NATIVE OS / BUILT-IN SCREEN UI

### 4.1 What the panel is

A **dumb RGB565 framebuffer**. 240 × 135. The host pushes frames; the board shows the last one
and loops an animation with a per-frame delay byte. The K86's own firmware draws a UI *into*
that same framebuffer, and the host cannot talk to that UI directly — only overwrite it.

### 4.2 The five picture slots

| Fact | Value | Source |
|---|---|---|
| Number of stored image slots | **5** | `sharkfin/.../data/devices.json` id 1168 `"screen": {"layers": 5}`; `keyboard-studio/.../Screen.swift:66` (`public static let imageSlots = 5`); `keyboard-studio/.../RoyuanProtocol.swift` (5 in the profile) |
| The layer is a **parameter of the write**, not a display selector | *"The five `screen.layer` entries are five static-image slots, not pages. The layer is a parameter of the image write, not a display selector"* | `keyboard-studio/.claude/docs/knob-screen-menu.md:101-103` |
| Where the layer goes | announce `0xA5` **byte 18**; data `0x25` **byte 1** | `keyboard-studio/.claude/docs/knob-screen-menu.md:104-105` |
| Competing placement | `sharkfin` puts the slot in announce **byte 1** (`frame`) and leaves byte 18 = 0; `screen_announce_packet(announce, slot, 1, 0, len, bbox, 0)` | `sharkfin/.../commands.rs:1091-1099` |
| **Resolution of the conflict** | `keyboard-studio` writes the layer into **both** byte 1 and byte 18 of the announce, so it is compatible with either reading (`Sources/KeyboardKit/Screen.swift:384-414`; `writeImage` passes `frameIndex: layer` *and* `layer: layer`). **Do the same.** | — |
| Is there a command to *display* a given slot? | **No.** *"There is no command to display a given slot. Which means the on-screen menu is very likely how a user picks between stored images."* | `keyboard-studio/.../Screen.swift:60-66`; `keyboard-studio/.claude/docs/knob-screen-menu.md:106-108` |
| Who picks the slot then? | *"the keyboard's own keys pick which slot is shown"* (ry5088 lineage) | `sharkfin/docs/PROTOCOL.md:880-881` |
| Alternative pipeline (older) | `0x20`/`0xA0` **picture index**, `0x21`/`0xA1` picture data, `0x24`/`0xA4` animation data, `0x26`/`0xA6` animation index — yc500-only in the vendor table | `sharkfin/docs/PROTOCOL.md:897-899` |

> **LEAD (not proven).** `0x20`/`0xA0` "picture index" is the only opcode in the entire corpus
> whose *name* matches the missing capability ("which stored picture is the OS showing?"). If
> the K86's native OS selects a slot through a picture-index command, `0x20` is very likely it.
> This is the single most promising native-OS control lead and it is **completely unprobed**.

### 4.3 The knob's on-screen menu — the "native OS"

| Fact | Evidence |
|---|---|
| Turning the knob opens the keyboard's own settings menu on the screen | `keyboard-studio/.claude/docs/knob-screen-menu.md:19-24` |
| The knob has **two firmware modes**: screen mode (turn walks the settings menu) and volume mode (turn does what the keymap slots say) | `keyboard-studio/.claude/docs/knob-screen-menu.md:13-16` |
| The switch is **Fn + knob press**, and it is a **key combination, not a command** | *"Fn + knob press — switches between screen operation and volume control. — Attack Shark K86 manual"*, quoted at `keyboard-studio/.claude/docs/knob-screen-menu.md:8-11` |
| **The mode is not stored in `SET_KBOPTION`** — reading it before and after a mode change returns the same bytes (`00 02 00 …`), so no bit can be written | `keyboard-studio/.claude/docs/knob-screen-menu.md:64-70` |
| The knob's volume slots are correct (96/97) and the menu still wins | `keyboard-studio/.claude/docs/knob-screen-menu.md:19-24` |
| **No command disables the menu.** The vendor's own client was searched exhaustively | `keyboard-studio/.claude/docs/knob-screen-menu.md:58-92` |

**What the vendor's own client was searched for** (`keyboard-studio/.claude/docs/knob-screen-menu.md:58-92`):

- *"All 76 `FEA_CMD_*` opcodes. Nothing selects a screen page, suppresses an overlay, or rebinds the encoder."*
- *"The complete abstract device API — 142 members. Its entire screen surface is: switch language, clear all images, sync clock, sync weather, sync system info, write image, and get/set screen menu. Nothing else exists."*
- *"Every bit of `SET_KBOPTION` (0x06). bit0 win-key lock, bit1 mac/win, bit3 WASD swap, bit4 main LED off, bit5 side LED off, bit6 keyboard mode, bit7 keyboard lock; byte3 bit0 Fn matrix; byte4 power save. Bit2 unused. No screen bit anywhere."*
- *"The whole special-key action catalogue. Exactly three OLED actions exist (`OLED_FontColor`, `OLED_BackColor`, `OLED_Confirm`). No menu toggle, no page next, no brightness."*
- `SET_MENU` (`0x2F`) *"exists but not on this keyboard. Only one driver class implements it, and that class declares `type: "dongle"`."*
- `SET_OLEDOPTION` (`0x22`) *"is not a settings command at all"* — it is a telemetry feed.
- *"The vendor's own app refuses to touch this knob."* In the K86's key-mapping page
  `AudioVolumeUp`, `AudioVolumeDown` and `AudioVolumeMute` are an explicit exclusion set, and
  the knob's labels are stripped from the SVG. The device record has no `knobKeyCodes`.

**Cross-reference:** the abstract API's screen surface maps neatly onto the opcodes we have:

| Vendor abstract-API call | Best-matching opcode | Status |
|---|---|---|
| switch language | `0x27` | layout unpublished |
| clear all images | `0x2C` / `0xAC`? | **unmapped** — which opcode clears images without erasing all flash? UNKNOWN |
| sync clock | `0x28` | **known**, byte-exact |
| sync weather | `0x2A` | layout unpublished |
| sync system info | `0x22` | payload fields known semantically, offsets unpublished |
| write image | `0xA5` + `0x25` | **known**, byte-exact |
| get/set screen menu | `0x2F`? | `0x2F` is dongle-only. UNKNOWN for a keyboard |

**This mapping is `INFERRED`** from the two lists, not stated in any source. It is, however, a
tight 7↔7 correspondence and it is the best available picture of the native OS's control
surface.

### 4.4 Brightness, contrast, screensaver, clock

| Thing | Status |
|---|---|
| **Brightness** | **Read only**, via `0x0E` reply byte 2 (single-sourced, `keyboard-studio/.../Screen.swift:118-150`). **No SET opcode for brightness exists in any source.** The vendor's special-key catalogue has *no* brightness entry (`knob-screen-menu.md:70-72`). The native menu almost certainly does it in firmware. |
| **Contrast** | **No opcode, no mention, anywhere.** Unknown. |
| **Screensaver** | **No opcode, no mention, anywhere.** Unknown. |
| **Clock** | `0x28` SET, byte-exact: year BE at 8..9, month 10, day 11, hour 12, minute 13, second 14. *"The vendor's app sends the host's local time on every connect, both families, the same builder."* *"No read. The board answers nothing useful and keeps time itself from there."* Round-tripped on a K86 — but the **yc3123** K86 (id 2730), not ours (`sharkfin/docs/PROTOCOL.md:914`). |
| **Display resolution** | `0x0E` reply bytes 17-18 (w) / 19-20 (h) big-endian, single-sourced. **Conflicts with** `keyboard-studio`'s own statement that *"the panel's real resolution is not something the protocol reports (the firmware accepts a write handshake for any size)"* (`Sources/KeyboardKit/Screen.swift:8-10`) and with sharkfin's *"Nothing may assume a default size"* — sharkfin takes the geometry from the vendor registry record, never from the device (`sharkfin/.../registry.rs:289-308`). **Treat `0x0E` as unproven on the K86 and use 240×135.** |
| **Sleep timers** | `0x12` / `0x92` — these are *keyboard* sleep timers, not screen sleep. `sharkfin/docs/PROTOCOL.md:406-415` |
| **Power save** | `SET_KBOPTION` byte 4. `sharkfin/docs/PROTOCOL.md:419-421` |

### 4.5 Other native-OS facts

- **The panel's own images are flash resources.** `0x25` pages *"stream through ten 4096-byte RAM
  banks with the announced length as a countdown… The frame goes to flash."* (yc3123 lineage,
  `sharkfin/docs/PROTOCOL.md:830-833`; yc500: *"The announce erases nothing. It resets counters."*
  `:792`). This is the factual basis for PLAN.md §2 question 3 — *are the built-in screens flash
  resources?* — and it is now **partially answered**: the host-writable images are definitely
  flash-backed.
- **The built-in screens' own bitmaps are in the same flash** — `INFERRED` from the above plus
  PLAN.md's own note that *"the device's own UI (image slots, menus) is firmware code + flash
  resources"* (`PLAN.md:20`). Still unproven.
- **The K86's firmware draws a clock widget** — proven only in the sense that `0x28` sets a
  clock the panel then shows (`sharkfin/docs/PROTOCOL.md:914`).
- **The panel's own menu is knob-driven.** `keyboard-studio` could not find any opcode for it
  (§4.3).

### 4.6 What remains UNKNOWN about the native OS

1. The **content and structure** of the knob's settings menu (pages, items, icons).
2. How a **slot is selected** from the menu, and which physical keys do it.
3. Whether `0x20`/`0xA0` (picture index) is that selector.
4. **Brightness** — is there any writable path at all? Does the menu own it exclusively?
5. **Contrast, screensaver, idle timeout, theme, language** — no opcode, no source.
6. The **"clear all images"** command in the vendor API — which opcode?
7. Whether the native OS's clock is *derived* from `0x28` or merely *displayed* by it.
8. Whether the five slots and the native menu share a resource pool, and whether the menu's
   built-in pictures can be read back (there is **no** read for the `0xA5`/`0x25` pipeline).
9. The exact payloads of `0x22`, `0x27`, `0x2A`, `0x2B`, `0x20`, `0x21`, `0x24`, `0x26`.
10. Whether `0x0E` answers on the K86 at all.

---

## 5. WHAT THE FACTORY APP DOES

This is the feature-parity target for the desktop app.

### 5.1 Architecture (two vintages, both public)

| Vint | Shape | Source |
|---|---|---|
| **Driver v4 (Electron, pure WebHID)** | *"The vendor tool is an Electron app, and nothing in its device path is native. Every byte it sends to the keyboard goes out over WebHID, from JavaScript. Three patches to the bundle force the pure-WebHID path."* | `dnim.dev/blog/royuan-keyboard-protocol` |
| **Current Windows build (web UI + closed helper)** | *"Attack Shark only ships a Windows app. That app is a web UI that talks to a small closed helper, `iot_driver.exe`, over gRPC-web on `127.0.0.1:3814`, and that helper is what touches the USB device."* | `attackshark-r85he-linux/README.md:5` |
| The app is a reskin | *"The 'Attack Shark Driver' is a reskin of a universal ROYUAN app… swapping brand logos off a JSON file over a registry of more than five hundred devices."* | `dnim.dev` blog |

The v4 bundle was the source of the whole opcode census in §2 (76 `FEA_CMD_*` opcodes, a
142-member abstract device API). It is the closest thing to a public spec of the factory app.

### 5.2 Feature list of the official Attack Shark app

From `attackshark-r85he-linux/README.md` (status column = tested on real hardware there, on a
R85HE, 2026-10-10):

| Feature | Official app | Source line |
|---|---|---|
| Device detection, model id, firmware version | ✅ | :57 |
| **Key remap (Custom Keys), Reset Key** | ✅ write and read-back verified | :60-61 |
| **Fn layer remap** | ✅ write and read-back verified | :62 |
| **Macros: record, save, assign to a key** | ✅ macro bytes read back exactly | :62 |
| **Magnetic switch: actuation per key, rapid trigger** | ✅ | :63 |
| Advanced keys (Toggle/TGL; DKS, MT, SnapKey same command path) | ✅ | :64 |
| **Profiles 1-4** | ✅ switch verified | :65 |
| **Report rate (Other Settings)** | ✅ | :66 |
| **Lighting: effects, brightness, speed, colour (live picker)** | ✅ | :67 |
| **Side light** | ✅ | :68 |
| **Light Edit (per-key colours, custom layers)** | ✅ read and write verified | :69 |
| Simulation Test (live key travel) | ✅ live data reaches the app | :70 |
| Local Configuration / saved macros | ✅ stored by the driver | :71 |
| Community Share, Login | ✅ | :72 |
| Travel Calibration | ⚠️ not run (destructive) | :73 |
| **Restore Factory Settings** | ⚠️ not run (wipes settings) | :74 |
| **Music Follow lighting** | ❌ not implemented | :75 |
| **Firmware upgrade** | ⛔ **blocked on purpose** — *"A failed flash can brick the keyboard; update from Windows with the official tool"* | :76 |

*"Opening **Light Edit** switches the keyboard to the 'User Picture' lighting mode"* — that is
light mode 13, our `0x0C` pattern. (:69)

### 5.3 The K86-specific surface (from the vendor's device record)

From `sharkfin/.../data/devices.json`, id 1168:

| Field | Value |
|---|---|
| `profiles` | 3 |
| `screen` | `{w:240, h:135, mode:"16", layers:5}` |
| `features.debounce` / `sleep24` / `sleepBT` | true / true / true |
| `features.screen` | true |
| `features.sideLight` | false |
| `features.magneticSwitches` | false |
| `features.knob` | `[]` — **no knob key codes in the vendor record** |
| `lightLayout` | `"se"` |

**`lightLayout: "se"` is the K86's exact lighting preset list** — this is the app's
"Lighting" page, verbatim (`sharkfin/.../data/light-layouts.json`):

| | |
|---|---|
| `rgb` | true |
| `brightnessMax` | 4 |
| effects (18) | mode 4 (opts: Right/Left/Down/Up), 5, 6, 7 (opts: Zigzag/Spiral), 8, 9, 10, 11 (opts: Outward/Inward), **19**, 12 (opts: Right/Left), 14, 15 (opts: CCW/CW), 16, 17, 18, 1, 2, 3 |

So the K86 has **18 firmware lighting effects** (modes 1-12, 14-19, skipping 13) with
direction options on modes 4, 7, 11, 12, 15. Mode 13 is the per-key "User Picture" mode, and
mode 20/21/22/23/24 are not offered on this board.

> **CONFLICT.** `keyboard-studio/.../RoyuanProtocol.swift` (`LightEffect.index`) lists
> `train = 23` and `fireworks = 24` for the family, and says *"13 and 19-22 are unused slots"* —
> but the K86's own vendor light layout **includes mode 19** and **excludes 23 and 24**. The
> `keyboard-studio` list is the family-wide LIGHT_LIST, not the K86's list. **Use `light-layouts.json`
> "se" for the K86.**

> **CONFLICT (names).** `keyboard-studio` names mode 3 "neon", 6 "raindrop", 7 "snake",
> 9 "converge", 11 "kaleidoscope", 12 "linewave", 14 "laser", 15 "circlewave", 16 "dazzle",
> 17 "raindown", 19 unused, 23 "train", 24 "fireworks". `sharkfin` names the same numbers
> 3 "spectrum", 6 "star dots", 7 "flow", 8 "key shadow", 9 "layers", 11 "spring", 12 "neon",
> 14 "radiant", 15 "loop", 16 "colour grid", 17 "snowfall", 19 "silent snow", 23 "train",
> 24 "endless" (`sharkfin/docs/PROTOCOL.md:255-263`, from the firmware's own renderers).
> **The numbers are agreed; the names are not.** Ship the vendor's own UI names from
> `light-layouts.json` where available, and the mode number as the stable key.

**LED flags for the K86**: `sharkfin/.../data/led-flags.json` has **no entry for 1168**, so the
default applies — **nibble 7 = fixed colour, 8 = rainbow**. (33 of 133 boards read them
swapped, including the AttackShark X65HE; `sharkfin/docs/PROTOCOL.md:252-258`.)

### 5.4 Parity target, restated

Our desktop app must be able to do, at minimum:

1. **Lighting**: 18 effects, brightness 0..4, speed 0..4, full RGB, per-effect direction
   options, side strip. ✅ mostly done.
2. **Per-key RGB** (Light Edit): 128 keys, 7 pages, flash-paced. ✅ protocol done, UI pending.
3. **Key remap**: base layer + Fn layer, per profile, with the knob's three actions. ✅ write path
   proven on the K86.
4. **Macros**: 50 slots × 256 B, record/play/repeat/toggle/hold, bind to a key. Protocol known
   (`0x16`/`0x8B`).
5. **Profiles**: 3, switchable (`0x05`).
6. **Screen**: image + GIF upload to 5 slots at 240×135, 30 frames max, 10 ms delay units. ✅
   protocol done.
7. **Device settings**: debounce (`0x11`), sleep timers (`0x12`), Win lock / WASD swap / LED-off
   / power save (`0x06`), auto-OS (`0x17`).
8. **Settings backup + restore** — the full read-back set in §3.5.
9. **Firmware upgrade** — the official app has it; we should refuse it (§3.3 hazards) and say why.
10. **Native OS control** — the aspirational one; see §4.

---

## 6. OPEN QUESTIONS / NEXT EXPERIMENTS

### 6.1 Method (applies to all of them)

- **Baseline-diff every probe.** *"An unsupported command returns the previous reply, not an
  error. Compare against the preceding response, not against zero."*
  (`sharkfin/docs/PROTOCOL.md:1041-1043`). Send a known-good GET, capture it, send the probe,
  and require the reply to (a) echo the probe opcode **and** (b) differ from the baseline.
- **Re-open the device before verifying a write.** A read on the same connection returns the
  cached previous reply (`keyboard-studio/.../Keymap.swift:88-94`).
- **Cable only.** Bluetooth has no settings collection; the 2.4 GHz receiver relays but adds
  `0xF6`/`0xF7`/`0xFC` handshaking and drops packets otherwise.
- **Never sweep a range blindly.** `keyboard-studio` was four opcodes away from `0xAC` with a
  naive `0x80..0x8F` sweep (`keyboard-studio/.claude/docs/feature-parity.md:81-88`).
- **Timing:** ≥10 ms after a send before reading the reply; 120 ms after a keymap write; 100 ms
  between flash pages; 2 s settle; 10 s between flash uploads.

### 6.2 SAFE — read-only probes (no state change)

| # | Experiment | What to send | What a positive result looks like | Value |
|---|---|---|---|---|
| **E1** | **Characterise `0x8D`** | `[0x8D]` (Bit7), then `[0x8D, 0]`, `[0x8D, 0, 0]`, `[0x8D, 1]` | A reply that echoes `0x8D` **and** differs from the baseline | **Highest.** It answers on the live board (`PLAN.md:23-24`) and exists in no public source. It may be a display/flash/OS read. |
| **E2** | **Characterise `0x82`** | same ladder as E1 | ditto | High — answers on the live board, undocumented |
| **E3** | **Re-test `0x83` / `0x84` with a baseline diff** | `[0x83]`, `[0x84]` after a captured `[0x85]` baseline | If `0x84` really answers with a profile index, the K86 is partly gen2 and the family table needs a per-board override. If it merely echoes the previous reply, `0x83`/`0x84` are dead on yc500 | High — resolves a family-classification ambiguity that would change several layouts |
| **E4** | **`0x0E` on report id 13** | 41-byte packet `[0x0E, 0x27, 0×38, ck]`, then the same opcode on the unnumbered feature report | A reply with a plausible width in 8..1024 at bytes 17-18 and height at 19-20, and brightness ≤ 255 at byte 2 | High — would settle the panel geometry question and give a brightness read. `keyboard-studio` says it usually does *not* answer; the K86 has never been asked on the numbered path |
| **E5** | **`0x36` (image slot addresses)** | `[0x36]`, then `[0x36, 0]`, `[0x36, 0..4]` | A reply containing 5 addresses / offsets, or a count of 5 | High — this is the vendor client's own "image slot addresses" read. If it returns flash offsets of the 5 slots, that is the **map of the native OS's picture resources**, which is exactly PLAN.md question 3 |
| **E6** | **`0xAD` / `0xAE`** | `[0xAD]`, `[0xAE]` | A real display-chip version (not an echo, not a zero word) | Medium-high — identifies the panel controller, which tells us what the native OS's drawing primitives are |
| **E7** | **`0x20` / `0xA0` picture index** | `[0x20]`, `[0x20, 0]`, `[0xA0]` — **read-shaped first** | A reply naming a current picture index / count | **Highest for native-OS control.** This is the only opcode whose name matches the missing "select the shown picture" capability |
| **E8** | **`0x8C` GET_USERPIC** | `[0x8C, 0, page 0..5]` | Documented to return stale data, but worth capturing: does it return the **factory** per-key pattern? If the board ships a per-key pattern, its bytes show the shipped colour map | Medium |
| **E9** | **Full settings backup** | `0x8F`, `0x80`, `0x85`, `0x86`, `0x87`, `0x88`, `0x91`, `0x92`, `0x97`, `0x89`×(3 profiles × 8 pages), `0x90`×(3 × 8), `0x8B`×(50 slots × 4 pages) | A complete, restorable settings snapshot | **High** — this is the deliverable "backup" that the protocol actually supports |
| **E10** | **Dead-opcode sweep with baseline diff** | `0x00`, `0x01`…`0x3F` and `0x8D`–`0xFF` one at a time, each preceded by a known-good baseline, **excluding the blocked set** `{0x01, 0x02, 0x1C, 0x1E, 0x30, 0x31, 0x40, 0x41, 0x43, 0x44, 0x7F, 0xC3, 0xC4, 0xAC}` | Any opcode that answers with fresh, structured data | Medium — a disciplined way to complete the census without a blind range sweep |
| **E11** | **Clock round-trip on OUR K86** | `0x28` with a known timestamp, then read the panel | The panel shows the sent time | Low-medium — proves the vendor's clock widget is live on the 4015 SKU (currently only proven on the 5002 SKU) |
| **E12** | **Side-light reality check** | `0x88` | Registry says `sideLight: false` for id 1168, but `0x88` answers with a plausible state. **A reply is not evidence of hardware** (`sharkfin/docs/PROTOCOL.md:1041`) — check visually | Low |

### 6.3 SAFE-BUT-WRITING (state change, reversible, low blast radius)

| # | Experiment | What it changes | Why it is acceptable |
|---|---|---|---|
| **E13** | **`0x28` clock** | the panel's clock | Round-tripped on a sibling K86; overwritten on the next vendor-app connect. It is a *write* though, so it is not in §6.2. Verify by eye on the panel. |
| **E14** | **`0x20`/`0xA0` picture index, write** | which stored picture the native OS shows | **Only if E7 shows the opcode exists and is readable.** If the index is currently 0 and writing 1 changes the panel's displayed picture, that is the native-OS selector. Revert afterwards. |
| **E15** | **Slot round-trip** | writes one picture to slot 1, reads the panel | Reversible; overwrites a stored image only. Also proves which of the 5 slots is which. |
| **E16** | **Light-mode 13 / USERPIC** | a per-key pattern in flash | Flash-paced; recoverable by re-uploading. Confirms `0x0C` works on the 4015 SKU with the yc500 header. |

### 6.4 RISKY — **DO NOT RUN WITHOUT THE OWNER'S EXPLICIT CONSENT**

| # | Experiment | Why it is dangerous |
|---|---|---|
| **R1** | **`0xAC` flash-chip erase** | Erases stored images and settings; blocks ~55 s; irreversible. `keyboard-studio` blocks it at the transport layer (`DangerousCommands.swift:7-9`) |
| **R2** | **`0x2C` display erase** | Sets the same flag as `0xAC` on the images read |
| **R3** | **`0x7F 55 AA 55 AA` (USB boot entry)** | The board re-enumerates as `3151:4016` (observed on a K86, `PROTOCOL.md:1030`). If the image you then write is wrong, *"the board [is] unable to answer `0x7F`, so boot is unreachable without SWD."* **This is the only route to the flashing path, and it is write-only — it cannot help with a backup.** |
| **R4** | **`0x30 55 AA 55 AA` (display-chip boot entry)** | Same class of risk, applied to the panel controller. The display-chip path is *"still unexercised"* (`PROTOCOL.md:938`) |
| **R5** | **`0x02` factory reset** | Wipes every profile, keymap, macro and light setting; ~4 s. Recoverable only if a settings backup exists (E9) |
| **R6** | **`0x1C` / `0x1E` calibration** | Commits zeroed travel tables. Not applicable to the K86 (not magnetic), so there is no reason to send it |
| **R7** | **Any write to an opcode whose family is uncertain** | *"A misaddressed write lands on a live register"* (`PROTOCOL.md:122-123`). The `0x06`/`0x09`/`0x11`/`0x12` collision set is exactly this hazard. |

### 6.5 Explicitly closed

- **R1 from PLAN.md §1 (HID flash-read opcode): CLOSED — it does not exist.** See §3.1.
- **Reading back the 5 stored pictures: NO OPCODE.** The `0xA5`/`0x25` pipeline is write-only and
  the announce *"erases nothing. It resets counters."* (`PROTOCOL.md:792`). A per-slot backup of
  the user's images is **impossible** over HID; the host must keep the originals.

---

## 7. SOURCES

Every URL used, with a one-line note.

| # | URL | Note |
|---|---|---|
| S1 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/docs/PROTOCOL.md` | **Primary source.** 1053-line protocol bible for the whole ROYUAN family: opcode tables, checksums, families, screens, flashing, hazards. |
| S2 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/app/src-tauri/src/protocol/cmd.rs` | Opcode constant list with the family split (46 lines). |
| S3 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/app/src-tauri/src/protocol/mod.rs` | Packet builders: checksum, bulk keymap, screen announce/page, clock, userpic, macro. |
| S4 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/app/src-tauri/src/protocol/gen2.rs` | gen2 keymatrix/userpic layouts (for the collision table). |
| S5 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/app/src-tauri/src/registry.rs` | `screen_draw()` — which lineages may draw, and the length/dimension limits. |
| S6 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/app/src-tauri/src/commands.rs` | Reference implementation: screen write pacing, announce polling, flash cooldowns, clock. |
| S7 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/app/src-tauri/data/devices.json` | 1409-device registry; **id 1168 = our K86** (`3151:4015`, yc500, 240×135, 5 layers, 3 profiles, no knob codes). |
| S8 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/app/src-tauri/data/light-layouts.json` | Vendor lighting preset list; **layout "se" = the K86's 18 effects** with direction options. |
| S9 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/app/src-tauri/data/led-flags.json` | Per-board LED flags-nibble swap list (no 1168 entry ⇒ 7=fixed, 8=rainbow). |
| S10 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/README.md` | Feature list, "sharkfin never flashes firmware", recovery steps. |
| S11 | `https://raw.githubusercontent.com/dniminenn/sharkfin/master/AGENTS.md` | Project's own safety rules (never write to an unknown family, rate limits in the backend). |
| S12 | `https://dnim.dev/md/blog/royuan-keyboard-protocol.md` | Author's write-up: two colliding command sets, the rescued-firmware route, the per-key flash rate limits. |
| S13 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/Sources/KeyboardKit/RoyuanProtocol.swift` | Opcode constants, checksum encoder, `0x0E` display-packet builder, effect index table. |
| S14 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/Sources/KeyboardKit/Screen.swift` | Screen announce/data packet layout, 5 slots, `0x0E` reply parse, pixel-order conflict note, calibration patterns. |
| S15 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/Sources/KeyboardKit/DangerousCommands.swift` | Blocked / flash-writing / safe-to-probe opcode sets (incl. `0x0E`, `0x36`). |
| S16 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/Sources/KeyboardKit/Keymap.swift` | Keymap/Fn read+write, knob slot discovery, profile select, the "re-open to verify" rule. |
| S17 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/Sources/KeyboardKit/PerKeyLighting.swift` | `0x0C` yc500 header, flash pacing, mode-13 show command. |
| S18 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/Sources/KeyboardKit/Knob.swift` | Knob slot encoding and the 16 firmware actions (incl. `wheelSwap` = 14). |
| S19 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/Sources/KeyboardKit/Keyboard.swift` | `0x8F`/`0x80` parsing, LED master read-modify-write, `queryDisplay` on report id 13. |
| S20 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/.claude/docs/knob-screen-menu.md` | **The native-OS document.** Vendor client census, `SET_MENU`/`SET_OLEDOPTION` findings, the 5-slot finding, the Fn+knob-press manual quote. |
| S21 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/.claude/docs/feature-parity.md` | Device-settings table (source of the `0x81` debounce error), 240×135 panel statement, dangerous-command history. |
| S22 | `https://raw.githubusercontent.com/mehmetnadir/keyboard-studio/main/Sources/KeyboardKit/Resources/Devices/attackshark-k86.json` | K86 device profile: id 1168, knob slots, screen 128×128 `verified:false`. |
| S23 | `https://raw.githubusercontent.com/RaphaelCaputo2/shark-k86-mac/main/PROTOCOL.md` | The original clean-room byte tables (128×128 era). |
| S24 | `https://raw.githubusercontent.com/RaphaelCaputo2/shark-k86-mac/main/k86/device.py` | Opcode constants, checksum encoder, LED master read-modify-write, light setters. |
| S25 | `https://raw.githubusercontent.com/RaphaelCaputo2/shark-k86-mac/main/k86/screen.py` | Screen announce/page packets, column-major big-endian RGB565, GIF path. |
| S26 | `https://raw.githubusercontent.com/RaphaelCaputo2/shark-k86-mac/main/k86/cli.py` | Reference CLI; confirms the "LEDs off / power-save" gotcha. |
| S27 | `https://raw.githubusercontent.com/Xynthera/AttackShark_K86_Spotify/master/spotify_k86.py` | **Independent K86 implementation** on our exact VID:PID: 240×135, 0xA5 announce with the layer/length bytes, 0x25 pages, column-major big-endian. |
| S28 | `https://raw.githubusercontent.com/Xynthera/AttackShark_K86_Spotify/master/README.md` | Same author's protocol summary (0xA5/0x25, Bit7 checksum at byte 7). |
| S29 | `https://raw.githubusercontent.com/TarikTopalovic/attackshark-r85he-linux/main/README.md` | **The official Attack Shark app's feature list** (vetted on real hardware) and the `iot_driver.exe` architecture. |
| S30 | `https://api.github.com/repos/dniminenn/sharkfin/git/trees/master?recursive=1` | Repo tree used to locate `docs/PROTOCOL.md`, `data/devices.json`, `protocol/*`. |
| S31 | `https://api.github.com/search/repositories?q=…` | Repo search for other implementations (`royuan`, `3151 4015`, `K86 keyboard tft hid`, `attack shark keyboard driver`). No further implementations exist. |
| S32 | `https://dnim.dev/blog/royuan-keyboard-protocol` | Canonical URL of the blog post (the `.md` variant S12 is the machine-readable version). |
| S33 | *(this repo)* `sharkdeck/re/PLAN.md:12-26` | Our own live-board opcode sweep: implemented handlers `0x80 0x82 0x83 0x84 0x85 0x86 0x87 0x88 0x8D 0x91 0x92 0x97`; `0x89/0x90/0x8B` echoless page reads; `0x8A` all-`0xFF`; *"nothing in the GET family returns a block of firmware bytes."* |
| S34 | *(this repo)* `sharkdeck/core/src/protocol.rs` | Our current implementation's opcode constants and slot encodings (used to cross-check, not changed). |

### 7.1 Sources that were checked and produced nothing usable

| Source | Result |
|---|---|
| `https://api.github.com/search/code?q=0xA5+0x25+royuan+keyboard` | 401 — GitHub code search requires authentication |
| GitHub repo search: `3151+4015`, `keyboard+hid+protocol+royuan`, `royuan+firmware+hid`, `3151+4015+keyboard` | zero results |
| `https://github.com/search?q=royuan+hid` (via web) | nothing beyond S1–S29 |
| Any public firmware image for the K86 (id 1168 / yc3121_sg8967_3m_soc) | **none found.** The firmware images cited by `sharkfin` for the K86 are for the **yc3123** SKU (id 2730, `v115_oledv108`) and the Hator (id 2936, `v113_oledv106`). Route **R4** (PLAN.md) still needs work, but the *yc3121* K86 image is a plausible find. |

---

*Every claim in this document is traceable to a file:line, a URL, or this repo's own captured
bytes. Anything inferred is marked `INFERRED`; anything a single source asserts without
corroboration is marked `UNVERIFIED`.*
