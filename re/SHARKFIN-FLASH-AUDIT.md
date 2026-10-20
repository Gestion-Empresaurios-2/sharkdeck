# SHARKFIN FLASH / FIRMWARE AUDIT

Reverse-engineering audit of **dniminenn/sharkfin** (branch `master`, commit `b36c790`, 2026-10-16),
for the SharkDeck project (Tauri 2 + Rust + Svelte 5, Attack Shark K86, USB `3151:4015`,
HID feature reports, "ROYUAN" family).

Every opcode, byte layout and file path below is quoted from the sources named.
Anything I derived rather than read is tagged **INFERRED**.

---

## 0. HEADLINE VERDICT (read this first)

> **CAN a host read the device's firmware bytes back over HID?**
> **NO. Not today, and not with any opcode that exists.**
> The bootloader protocol has **no read command at all**:
> *"Only pass/fail and the running error count leave over USB. There is no read command in boot mode."*
> — `docs/PROTOCOL.md:1005-1007`
> *"**No USB dump** — The dispatcher has no read. Flash contents never come out over USB."*
> — `docs/PROTOCOL.md:1031`

**Closest thing sharkfin CAN do** (in descending order of usefulness):

| # | What | Where | Note |
|---|---|---|---|
| 1 | **Firmware WRITE (flash an image)** | `docs/PROTOCOL.md:927-1033` | app-mode `0x7F` boot entry → boot PID `3151:4016` → raw `0xBA` init/data/verify. **Not implemented in sharkfin**, but it was run end-to-end on a **K86 (device id 1168)** by an outside user: *"the board entered boot, took 3397 chunks and answered the verify with `0x55`, and came back up on the new image"* (`docs/PROTOCOL.md:934-939`). Reported by mail, not bench-verified by the author. |
| 2 | **Settings backup/restore (JSON)** | `commands.rs:1487-1549` (`export_config`) / `1555-1692` (`import_config`) | Keymaps (all profiles + Fn layers), LEDPARAM, side light, debounce, sleep, KBOPTION. **This is the only "backup" sharkfin actually has.** It is a *configuration* backup, not a firmware backup. |
| 3 | **Factory reset** | `commands.rs:842-849` (opcode `0x02` yc500 / `0x01` gen2) | Wipes profiles/keymaps/macros/lights. Destructive, not a backup. |
| 4 | **Flash-chip erase** | `cmd.rs:45` `GEN2_FLASH_CHIP_ERASE = 0xAC`; `mod.rs:367`; `docs/PROTOCOL.md:886-895` | Declared, **never sent** by sharkfin. Wipes every stored screen picture; ~55 s. |
| 5 | **Off-device firmware image (for *restoring*, not reading)** | Vendor channel + one GitHub archive (see §6) | A K86 **v2** (dev id 2730) image is downloadable. **Our board (dev id 1168) has NO published firmware** — the vendor channel answers `Record not found`. |

**Terminology trap — read this before quoting anyone:** in sharkfin's source, **"flash" almost always
means the board's *settings* flash (NVM), not the firmware flash.** `FLASH_COOLDOWN`, `FLASH_PAGE_GAP`,
`FLASH_SETTLE` (`commands.rs:92-107`) are about *keymap/lighting writes that land in NVM* and about the
USB endpoint stalling if you do them too fast. They have nothing to do with firmware flashing.
sharkfin's README says outright: *"sharkfin never flashes firmware."* (`README.md:30`)

---

## 1. THE COMPLETE OPCODE TABLE

### 1.0 Transport & framing (common to both families)

| item | value | source |
|---|---|---|
| Report | 64-byte HID **feature** report, report ID 0 | `protocol/mod.rs:14` (`REPORT_LEN = 64`) |
| Collection | usage page `0xFFFF`, usage `0x0002` (usage `0x0001` also accepted) | `protocol/mod.rs:9-13` |
| USB VID | `0x3151` (common; other VIDs exist) | `protocol/mod.rs:8` |
| Checksum | `0xFF - (sum & 0xFF)` | `protocol/mod.rs:24-36` |
| Bit7 mode | byte 7 = `0xFF - (sum(buf[0..=6]) & 0xFF)` | `protocol/mod.rs:26-29` |
| Bit8 mode | byte 8 = `0xFF - (sum(buf[0..=7]) & 0xFF)` | `protocol/mod.rs:30-33` |
| None mode | USB boot data path, OLED/TFT data reports | `docs/PROTOCOL.md:127` |
| Bit8 used by | `SET_LEDPARAM`, `SET_SLEDPARAM` only | `protocol/mod.rs:38`, `cmd.rs:6-7` |
| Reply convention | GET replies **echo the opcode in byte 0**; bulk page reads (keymatrix, Fn, macro, userpic) return **raw page bytes, no echo** | `protocol/mod.rs:3-5`, `docs/PROTOCOL.md:37-38` |
| Unsupported command | **replays the previous reply** — not an error. Compare against the previous response, never against zero. | `docs/PROTOCOL.md:1047-1048` |
| Minimum write gap | 12 ms (faster stalls the control endpoint until USB re-enum) | `wire.rs:66` (`MIN_WRITE_GAP_MS = 12`), `hid.rs:132-133` |
| Settle after send | 10 ms | `wire.rs:75` (`SETTLE_MS = 10`) |

### 1.1 How sharkfin decides which family a board is

**Two families, and opcodes COLLIDE.** Sending the wrong family's "write keymap" writes a live
register (on gen2, yc500's `SET_KEYMATRIX 0x09` *is* `SET_KBOPTION`) — "Nothing rejects it. The board
applies the opcode to whatever it means locally and returns a clean reply." (blog
`dnim.dev/blog/royuan-keyboard-protocol`; `docs/PROTOCOL.md:132-138`)

Resolution order in sharkfin:

1. **Registry first.** `DeviceSpec::family` from `data/devices.json` (`registry.rs:50-51`, `:171-177`).
   Only `"yc500"` and `"gen2"` are *known*; anything else is **permanently read-only**
   (`registry.rs:214-216` `writes_supported()`; `commands.rs:463-469`).
2. **Runtime derivation for unregistered boards** (`derive.rs:58-68` `detect_family`), from four
   read-only replies:
   - `0x89 [0,0]` → if it is a *keymap page* (≥8 "plain key" 4-byte entries) and `0x8A [0,0xFF,0,0]`
     is all-`0xFF` ⇒ **yc500** (`derive.rs:22-38, 59`).
   - `0x8A [0,0xFF,0,0]` is a keymap page and `0x89` is an *opcode echo* (`reply[0] == 0x89`) ⇒ **gen2**
     (`derive.rs:60`).
   - Tie-break on sleep timers: yc500 keeps them at bytes `1..9` of `0x92`; gen2 at bytes `8..16` of
     `0x91` (`derive.rs:40-53, 64-66`).
   - Anything ambiguous or unmatched ⇒ `None` ⇒ **unknown ⇒ read-only forever** (`derive.rs:61-67`).
3. **Live settle at connect**: an unregistered board's family is settled by a read sweep
   (`ops.rs:287-330` `derive_sweep`, `ops.rs:390-424` `BUNDLE_PROBES`).

> ⚠️ **Never resolve the family from the USB PID.** The vendors recycle PIDs: over a hundred yc500
> boards share `3151:4015` (`docs/PROTOCOL.md:966-970`), and our K86 is one of them.
> The authoritative per-board id is the **`0x8F` identify reply** (a u32 LE), *not* the PID.

### 1.2 Full opcode table (as implemented by sharkfin)

`F` = direction, `C` = checksum mode. Layouts are copied verbatim from the source.

#### 1.2.1 Shared opcodes (same meaning in both families)

| hex | name | dir | C | exact byte layout | reply carries | source |
|---|---|---|---|---|---|---|
| `0x8F` | GET_USB_VERSION / identify | R | Bit7 | `[0x8F]` | `[0x8F, device_id u32 LE, ...]`; gen2 also carries the keyboard fw version at bytes `7..9` | `cmd.rs:27`; `mod.rs:766-773`; `ops.rs:251-268` |
| `0x80` | GET_REVISION | R | Bit7 | `[0x80]` | `(reply[2]<<8)\|reply[1]`; **gen2 answers zeros over the cable** — use `0x8F` bytes `7..9` | `cmd.rs:32`; `docs/PROTOCOL.md:431` |
| `0x07` | SET_LEDPARAM | W | **Bit8** | `[op, mode, speed, brightness, (option<<4)\|flags, R, G, B, ck8]` — speed inverted, white `0xFFFFFF`→`0xFAFAFA`, RGB<`0x080808` renders unlit | — | `cmd.rs:6`; `mod.rs:780-901`; `docs/PROTOCOL.md:242-312` |
| `0x87` | GET_LEDPARAM | R | Bit7 | `[0x87]` | same shape as SET; `flags&0x0F` 0..6 = preset colour index (red,orange,yellow,green,cyan,blue,magenta) | `cmd.rs:22`; `mod.rs:917-958` |
| `0x08` | SET_SLEDPARAM (edge light) | W | **Bit8** | `[op, mode, speed, brightness, (option<<4)\|flags, R, G, B, ck8]` — **speed NOT inverted** | — | `cmd.rs:7`; `mod.rs:304-320`; `docs/PROTOCOL.md:369-374` |
| `0x88` | GET_SLEDPARAM | R | Bit7 | `[0x88]` | same shape; answered even on boards with no edge LEDs | `cmd.rs:23`; `mod.rs:322-354`; `docs/PROTOCOL.md:376` |
| `0x0C` | SET_USERPIC (per-key RGB) | W | Bit7 | **different header per family** — see §1.2.3 | — | `cmd.rs:10`; `mod.rs:503-517`; `gen2.rs:100-117` |
| `0x8C` | GET_USERPIC | R | Bit7 | yc500 `[0x8C, 0, page]`; gen2 `[0x8C, slot, 0xFF, page]` | **stale — does not reflect writes. Treat as write-only.** | `mod.rs:520-522`; `docs/PROTOCOL.md:496-497` |
| `0x10` | SET_FN | W | Bit7 | **different shape per family** — see §1.2.3 | — | `cmd.rs:11`; `gen2.rs:53-61` |
| `0x90` | GET_FN | R | Bit7 | yc500 `[0x90, profile, page]`; gen2 `[0x90, os, profile, 0xFF, page]` | raw 64-byte page, no echo | `cmd.rs:28`; `ops.rs:230-238`; `gen2.rs:20-22` |
| `0x8B` | GET_MACRO | R | Bit7 | `[0x8B, slot, page]`, pages 0..3 | raw 64-byte page, no echo. **Read-back is faithful.** | `cmd.rs:25`; `docs/PROTOCOL.md:468-469` |
| `0x17` | SET_AUTO_OS | W | Bit7 | `[0x17, 0|1]` | — | `cmd.rs:17` |
| `0x97` | GET_AUTO_OS | R | Bit7 | `[0x97]` | `reply[1] == 1` means on | `cmd.rs:31` |
| `0x28` | SET_OLED_CLOCK | W | Bit7 | `[0x28]` + bytes `8..14`: year BE `8..9`, month `10`, day `11`, hour `12`, minute `13`, second `14` | nothing useful; the board keeps its own time | `cmd.rs:18`; `mod.rs:478-495`; `docs/PROTOCOL.md:904-925` |
| `0xAD` | GET_OLED_VERSION | R | Bit7 | `[0xAD]` | screen firmware version; a board that answers has a display, an echo does not | `cmd.rs:36`; `docs/PROTOCOL.md:747-750` |
| `0xAE` | GET_MLED_VERSION | R | Bit7 | `[0xAE]` | second-chip version, `(reply[2]<<8)\|reply[1]`, zero for none | `cmd.rs:40`; `docs/PROTOCOL.md:752-754` |

#### 1.2.2 yc500 family (chip lineage `yc3121_*` — **this is our K86's family**)

| hex | name | dir | C | exact byte layout | reply carries | source |
|---|---|---|---|---|---|---|
| `0x02` | SET_RESET (factory reset) | W | Bit7 | `[0x02]` bare opcode. Firmware needs ~4 s after. | — | `cmd.rs:3`; `commands.rs:842-849`; `docs/PROTOCOL.md:432` |
| `0x05` | SET_PROFILE | W | Bit7 | `[0x05, profile]` | — | `cmd.rs:4`; `docs/PROTOCOL.md:384-387` |
| `0x85` | GET_PROFILE | R | Bit7 | `[0x85]` | `reply[1]` = current profile, zero-based | `cmd.rs:20`; `docs/PROTOCOL.md:381-387` |
| `0x06` | SET_KBOPTION | W | Bit7 | `[op, 0, flags, fnMatrix, powerSave]` — reply flags byte 2: `1` Win-lock, `2` Mac mode, `8` WASD swap, `16` backlight off, `32` side light off, `64` kb mode, `128` keyboard lock | — | `cmd.rs:5`; `mod.rs:742-755`; `docs/PROTOCOL.md:417-425` |
| `0x86` | GET_KBOPTION | R | Bit7 | `[0x86, 0]` | flags in `reply[2]`, fn matrix `reply[3]`, power save `reply[4]` | `cmd.rs:21`; `mod.rs:723-735` |
| `0x11` | SET_DEBOUNCE | W | Bit7 | `[op, 0, value]` — **value at byte 2** (byte 1 is a zero pad) | — | `cmd.rs:12`; `mod.rs:105`; `docs/PROTOCOL.md:400-401` |
| `0x91` | GET_DEBOUNCE | R | Bit7 | `[0x91]` | `reply[2]` | `cmd.rs:29`; `docs/PROTOCOL.md:400` |
| `0x12` | SET_SLEEPTIME | W | Bit7 | `[op]` + 4×u16 LE at bytes `8..16` (sleep BT, sleep 2.4G, deep BT, deep 2.4G) | — | `cmd.rs:13`; `mod.rs:221-230`; `docs/PROTOCOL.md:411` |
| `0x92` | GET_SLEEPTIME | R | Bit7 | `[0x92]` | 4×u16 LE at bytes **`1..9`** (NOT the SET offsets) — the vendor's encoder and decoder disagree | `cmd.rs:30`; `mod.rs:106`; `docs/PROTOCOL.md:412` |
| `0x09` | SET_KEYMATRIX (bulk) | W | Bit7 | `[0x09, profile, 0xF8, 1, page, 0, 0, ck7]` + 56 bytes at byte 8; **9 pages, commit on page 8** | — | `cmd.rs:8`; `mod.rs:171-195`; `docs/PROTOCOL.md:179-184` |
| `0x89` | GET_KEYMATRIX | R | Bit7 | `[0x89, profile, page]`, page 0..7 | raw 64-byte page, no echo | `cmd.rs:24`; `ops.rs:236` |
| `0x13` | SET_KEY_ONE (single slot) | W | Bit7 | `[0x13, profile, slot]`, 4-byte value at bytes `8..12` | — | `cmd.rs:14`; `mod.rs:108-110`; `docs/PROTOCOL.md:177` |
| `0x15` | SET_FN_ONE | W | Bit7 | `[0x15, profile, slot]`, 4-byte value at bytes `8..12` | — | `cmd.rs:15`; `docs/PROTOCOL.md:178` |
| `0x10` | SET_FN (bulk) | W | Bit7 | same shape as `0x09` (page commit on page 8) | — | `cmd.rs:11`; `mod.rs:179-180`; `docs/PROTOCOL.md:180` |
| `0x16` | SET_MACRO | W | Bit7 | `[op, slot, page, 56, last, 0]` + 56 bytes at byte 8 (zero-padded) | — | `cmd.rs:16`; `mod.rs:686-704`; `docs/PROTOCOL.md:445-454` |
| `0x1D` | SET_SWITCH_PRESET (magnetic, yc500) | W | Bit7 | `[0x1D, preset]`, preset 0..3 (0 comfort, 1 sensitive, 2 gaming, 3 = use the columns) | — | `protocol/hall.rs:63`; `docs/PROTOCOL.md:721` |
| `0x9D` | GET_SWITCH_PRESET | R | Bit7 | `[0x9D]` | `[9D, preset, 0 x6]` | `protocol/hall.rs:64`; `docs/PROTOCOL.md:722` |
| `0x1A` | SET_SWITCH_GLOBAL (yc500, fw < 2.00) | W | Bit7 | one record per key (byte 5 set) or one slot (byte 6): bytes `3`=mode, `4`=travel, `8`=release, `9`/`10`=RT steps, `11`=first point, `12..15`=actions, `16`=mod-tap, `25`=dead zone. Saves to flash, forces preset 3. | — | `protocol/hall.rs:70`; `docs/PROTOCOL.md:723` |
| `0xE5` | HALL read (columns) | R | Bit7 | `[0xE5, subop, 1, page]` | raw page, no echo. u16 columns = 4 pages of 32; byte columns = 2 pages of 64 | `protocol/hall.rs:6`; `docs/PROTOCOL.md:534` |
| `0x65` | HALL write (columns) | W | Bit7 | one: `[0x65, subop, 0, slot, last]`, value at bytes 8..9; all: `[0x65, subop, 1, page, last]` + 56 bytes at 8 | — | `protocol/hall.rs:7`; `docs/PROTOCOL.md:535-536` |
| `0x1B` | TRAVEL_STREAM (gen2 only) | W | Bit7 | `[0x1B, flag]` — any nonzero starts the stream, `0` stops it. Not saved. | stream arrives as an **input** report: report ID 5, `[0x1B, travel_lo, travel_hi, slot]` on the `0xFFFF` input collection | `protocol/hall.rs:31-34`; `docs/PROTOCOL.md:642-666` |

**yc500 sub-op codes (HALL columns)** — `protocol/hall.rs:10-21`, `docs/PROTOCOL.md:538-553`:

| sub-op | column | gen2 type | yc500 offset (1618) |
|---|---|---|---|
| 0 | actuation travel | u16 | `0x82d` |
| 1 | release travel | u16 | `0x8ab` |
| 2 | rapid-trigger press step | u16 | `0x929` |
| 3 | rapid-trigger release step | u16 | `0x9a7` |
| 4 | dynamic keystroke first point | u16 | `0xa25` |
| 5 | mod-tap hold time (10 ms units) | u8 | `0xc9b` |
| 6 | bottom dead zone | u16 | `0xd19` |
| 7 | key mode (bit7 = rapid trigger, bits 0..6 = kind) | u8 | `0x7af` |
| 8 | dynamic keystroke actions (4 bytes, one slot) | 4×u8 | `0xaa3` |
| 9 | snap partner slot | u8 | `0xd97` |
| 10 | every key's DKS actions (read only, 8 pages) | 4×128 u8 | 4×126 u8 |
| 251 | top dead zone (3708 only) | u8 | absent |
| 252 | switch model | u8 | absent |
| 254 | live press travel (read only) | u16 | absent |

**Never sent** (destructive, `docs/PROTOCOL.md:691-693`): `0x1C` and `0x1E` are travel-table
*calibration* — "On, they zero the stored travel tables in RAM; off, they save them to flash, so
on-then-off without pressing every key commits zeroed tables."

#### 1.2.3 gen2 family (chip lineage `ry5088_*`, `yc3123_*`)

| hex | name | dir | C | exact byte layout | reply carries | source |
|---|---|---|---|---|---|---|
| `0x01` | SET_RESET (factory reset) | W | Bit7 | `[0x01]` bare opcode | — | `mod.rs:128`; `docs/PROTOCOL.md:432` |
| `0x04` | SET_PROFILE | W | Bit7 | `[0x04, profile]` | — | `mod.rs:115` |
| `0x84` | GET_PROFILE | R | Bit7 | `[0x84]` | `reply[1]` | `mod.rs:116` |
| `0x09` | SET_KBOPTION | W | Bit7 | decoded fields, **meanings unestablished** — sharkfin withholds the whole feature on gen2 | — | `mod.rs:125, 139`; `docs/PROTOCOL.md:154` |
| `0x89` | GET_KBOPTION | R | Bit7 | `[0x89]` | decoded fields at bytes 1..4, meanings unestablished | `mod.rs:126`; `docs/PROTOCOL.md:435` |
| `0x06` | SET_DEBOUNCE | W | Bit7 | `[0x06, value]` — **value at byte 1** (yc500's `[op,0,value]` would write debounce 0!) | — | `mod.rs:118, 135`; `docs/PROTOCOL.md:401-404` |
| `0x86` | GET_DEBOUNCE | R | Bit7 | `[0x86]` | `reply[1]` | `mod.rs:119`; `docs/PROTOCOL.md:401` |
| `0x11` | SET_SLEEPTIME | W | Bit7 | `[0x11]` + 4×u16 LE at bytes `8..16` | — | `mod.rs:123`; `docs/PROTOCOL.md:414` |
| `0x91` | GET_SLEEPTIME | R | Bit7 | `[0x91]` | 4×u16 LE at bytes `8..16` (symmetric with SET) | `mod.rs:122, 138`; `docs/PROTOCOL.md:413` |
| `0x0A` | SET_KEYMATRIX (slot) | W | Bit7 | `[0x0A, profile, slot, 0, 0, apply, sublayer]`, 4-byte value at bytes `8..12` | — | `gen2.rs:38-51`; `docs/PROTOCOL.md:210, 596` |
| `0x0A` | SET_KEYMATRIX (bulk) | W | Bit7 | `[0x0A, profile, 0xFF, page, len, last, sublayer]` + ≤56 bytes at byte 8; 10 pages | — | `gen2.rs:65-82`; `docs/PROTOCOL.md:211, 597` |
| `0x8A` | GET_KEYMATRIX | R | Bit7 | `[0x8A, profile, 0xFF, page, sublayer]` (sub-layer 0 = the keymap, 1..3 = DKS / mod-tap / toggle) | raw 64-byte page, no echo | `gen2.rs:8-18`; `ops.rs:231-234` |
| `0x10` | SET_FN | W | Bit7 | `[0x10, os, profile, slot]`, value at bytes 8..12; os = 0 win, 1 mac, 2 android, 3 ios | — | `gen2.rs:53-61`; `docs/PROTOCOL.md:213` |
| `0x0B` | SET_MACRO | W | Bit7 | same shape as yc500 `0x16` | — | `mod.rs:127`; `docs/PROTOCOL.md:155` |
| `0x03` | SET_REPORT_RATE | W | Bit7 | (yc500 has no equivalent; the vendor's yc500 setter is a stub) | — | `mod.rs:129`; `docs/PROTOCOL.md:434` |
| `0x83` | GET_REPORT_RATE | R | Bit7 | `[0x83]` | — | `mod.rs:129`; `docs/PROTOCOL.md:434` |

**gen2 per-key colour header** (`gen2.rs:100-117`): `[0x0C, slot, 0xFF, page, len, last, 0, ck7]`
+ data at byte 8; 378 bytes (126 slots × RGB) in 7 pages. A page is copied **only when byte 2 is
`0xFF`**; otherwise the copy is skipped but the commit still fires (burns a flash cycle on stale
data). The firmware bounds-checks neither page nor length.

**yc500 per-key colour header** (`mod.rs:503-517`): `[0x0C, 0, len_lo, len_hi, page, 0, 0, ck7]`
+ data at byte 8; 384 bytes (128 slots × RGB) in 7 pages. **No slot byte on yc500.**

#### 1.2.4 Slot value encoding (both families)

`docs/PROTOCOL.md:225-236`:

| byte 0 | meaning | bytes 1..3 |
|---|---|---|
| `0x00` | HID usage | `[0, usage, 0]`; combos use bytes 1 and 3 |
| `0x01` | mouse button | |
| `0x03` | consumer usage | `[0, lo, hi]`; Vol+ = `[3,0,0xE9,0]` |
| `0x09` | macro | `[mode, index, 0]`; 0 repeat, 1 toggle, 2 hold |
| `0x0A` | special | `[1,0,0]` Fn, `[12,0,0]` power save, `[17,0,0]` Caps LED Swap |
| all zero | Fn layer: fall through to base. Base layer: the key is dead | |

#### 1.2.5 Receiver (2.4 GHz dongle) opcodes — separate device, same collection

`protocol/receiver.rs:5-53`; `docs/PROTOCOL.md:40-95`:

| hex | name | dir | C | layout | reply |
|---|---|---|---|---|---|
| `0xF7` | STATUS | R | Bit7 | `[0xF7]` | `[0]=1` a relayed reply is waiting, `[1]` keyboard battery %, `[3]=0` keyboard online, `[5]=1` will accept a command, `[6]` 1=kb/2=mouse/3=both, `[7],[8]` RF boot flags (both 1 = in boot) |
| `0xF6` | SELECT relay target | W | Bit7 | `[0xF6, 10]` keyboard, `5` mouse, `13` both | — |
| `0xFC` | RELEASE a relayed reply | W | Bit7 | `[0xFC]` bare | hands the buffered reply to the next read |
| `0xF0`, `0xF1` | none | — | — | unsupported on `4011`, replay the previous reply | — |
| `0xFB` | not a version | R | Bit7 | — | carries link state, not a version |

Relay exchange: poll `0xF7` until `[5]==1` → `[0xF6,10]` → the keyboard packet → poll `0xF7` until
`[0]==1` → `0xFC` → the GET reply. sharkfin implements this in `wire.rs:136-184`.

### 1.3 The yc500 / gen2 family table (`FamilyCmds`)

`protocol/mod.rs:50-140` — the machine-readable per-family opcode table sharkfin uses:

| field | yc500 | gen2 | src |
|---|---|---|---|
| `name` | `"yc500"` | `"gen2"` | `mod.rs:86, 114` |
| `set_profile` / `get_profile` | `0x05` / `0x85` | `0x04` / `0x84` | `mod.rs:87-88, 115-116` |
| `set_keymatrix` / `get_keymatrix` | `0x09` / `0x89` | `0x0A` / `0x8A` | `mod.rs:89-90, 117-118` |
| `set_key_one` / `set_fn_one` | `0x13` / `0x15` | `None` (uses `0x0A`) | `mod.rs:91-92, 119-120` |
| `set_debounce` / `get_debounce` | `0x11` / `0x91` | `0x06` / `0x86` | `mod.rs:93-94, 121-122` |
| `set_sleeptime` / `get_sleeptime` | `0x12` / `0x92` | `0x11` / `0x91` | `mod.rs:95-96, 123-124` |
| `set_kboption` / `get_kboption` | `0x06` / `0x86` | `0x09` / `0x89` | `mod.rs:97-98, 125-126` |
| `set_macro` | `0x16` | `0x0B` | `mod.rs:99, 127` |
| `set_reset` | `0x02` | `0x01` | `mod.rs:100, 128` |
| `report_rate` | `None` | `(0x03, 0x83)` | `mod.rs:101, 129` |
| `get_revision` | `0x80` | `0x80` (shared) | `mod.rs:102, 130` |
| `auto_os` | `(0x17, 0x97)` | same | `mod.rs:103, 131` |
| `sled` | `(0x08, 0x88)` | same | `mod.rs:104, 132` |
| `debounce_at` | 2 | **1** | `mod.rs:105, 135` |
| `sleep_reply_at` | **1** | **8** | `mod.rs:106, 138` |
| `kboption` | `Some` | `None` (withheld) | `mod.rs:107, 139` |

---

## 2. FLASH / FIRMWARE SECTION

### 2.1 Destructive opcodes (app mode) — the complete list

`docs/PROTOCOL.md:735-754`. **sharkfin sends none of these** except the factory reset.

| opcode | what | danger | source |
|---|---|---|---|
| `0xAC` | **flash-chip erase**, both families. On gen2 it erases every stored picture and takes ~55 s. `0xAC` preloads an `AA AA 55 55` reply. | 🔴 **DESTROYS DATA** (pictures; possibly more) | `cmd.rs:42-45`; `mod.rs:367`; `docs/PROTOCOL.md:742, 886-895` |
| `0x2C` | yc500 **display erase** — sets the *same* flag as `0xAC` on every image that was read (bit `0x20` at `0x2050C+2` on the RT100; bit `0x80` at `+4` on the Dynatab75X). | 🔴 **DESTROYS DATA** | `docs/PROTOCOL.md:743, 891-895` |
| `0x7F` | **USB boot entry** — but *only* with the trailer `55 AA 55 AA`. `0x7F` alone erases the keymap block and the switch-settings block and enters the bootloader. | 🟠 **RISKY / DESTRUCTIVE** | `docs/PROTOCOL.md:744, 691-694, 941-949` |
| `0x30` | **display-chip boot entry**, same trailer. NOT USB boot. | 🟠 **RISKY** | `docs/PROTOCOL.md:745, 901, 1033` |
| `0x01` (gen2) / `0x02` (yc500) | factory reset — wipes profiles, keymaps, macros, lights | 🟠 **DESTROYS SETTINGS** | `cmd.rs:3`; `mod.rs:128`; `commands.rs:842-849` |
| `0x1C`, `0x1E` | magnetic travel-table **calibration** — on, they zero the stored travel tables in RAM; off, they **save them to flash**. On-then-off without pressing every key commits zeroed tables. | 🔴 **DESTROYS SETTINGS** | `docs/PROTOCOL.md:691-693` |

**Third-party corroboration** — `mehmetnadir/keyboard-studio`, `Sources/KeyboardKit/DangerousCommands.swift`
(same author, same protocol, independent implementation):

```swift
public static let blocked: Set<UInt8> = [
  0xAC,  // flash chip erase — wipes stored screen images, blocks ~55 s
  0x02,  // factory reset (this protocol lineage)
  0x01,  // factory reset (the other lineage)
  0x43, 0x44, 0xC3, 0xC4,  // Nordic bootloader entry and start
  0x30, 0x31, 0x40, 0x41,  // further reported bootloader entries, unconfirmed
  0x1C, 0x1E,  // magnetic-switch travel calibration — destroys calibration
]
public static let flashWriting: Set<UInt8> = [
  0x0C,        // per-key colour pattern
  0x18, 0x19,  // animated per-key light
  0x25, 0x29,  // screen image, 16-bit and 24-bit
  0x22, 0x27, 0x28,  // screen host stats, language, clock
]
```
The extra opcodes (`0x43/0x44/0xC3/0xC4` Nordic boot, `0x40/0x41`) are **not in sharkfin** — mark as
**third-party, unconfirmed**.

### 2.2 Boot-mode (firmware) protocol — the write path

`docs/PROTOCOL.md:927-1033`. "A second HID dialect on a different product ID. **sharkfin never sends
any of this.**" Host framing is **[JS]** (vendor JS); the dispatcher is **[FW]** (read from the 1724
bootloader, first 20 KB, load `0x01000000`).

**Step 1 — enter boot (app mode, Bit7):**

| target | report | source |
|---|---|---|
| USB | `[0x7F, 0x55, 0xAA, 0x55, 0xAA]` | `docs/PROTOCOL.md:947` |
| RF | `[0xF8, 0x55, 0xAA, 0x55, 0xAA, 0, 0, 0x82]` | `docs/PROTOCOL.md:948` |
| display chip | `[0x30, 0x55, 0xAA, 0x55, 0xAA, 0, 0, 0]` | `docs/PROTOCOL.md:949` |

**Step 2 — the board re-enumerates on a boot PID.** The vendor's boot table
(`docs/PROTOCOL.md:951-963`): `3151:4036`, `3151:502a`, `3151:504d`, `046a:012e`, `046a:0130`,
and **`3151:4016` = K86 (1168), app PID `3151:4015`**. A second vendor table adds `3151:5024`.
"Which PID a given board lands on is not established beyond that table."
⚠️ *"Over a hundred yc500 boards share `3151:4015`, so a tool that assumes the vendor tables are
complete will not find them."* (`docs/PROTOCOL.md:966-970`)
⚠️ *"A K86 (1168) boots to `3151:4016`, which is in neither. A tool that only looks for the listed
PIDs will report no board while one sits in the bootloader."* (`docs/PROTOCOL.md:1030`)

**Step 3 — USB image transfer (raw 64-byte reports, NO Bit7 checksum):**

| step | report | notes |
|---|---|---|
| init | `[0xBA, 0xC0, cntLo, cntHi, len0, len1, len2]` | `cnt = ceil(len/64)`, `len` 24-bit |
| data | 64-byte chunks of the payload | written on arrival at flash-base + `index << 6` (`0x01010200` on the 1724 bootloader) **with a 64-byte read-back compare** |
| verify | `[0xBA, 0xC2, cntLo, cntHi, s0, s1, s2, s3, len0, len1, len2, len3]` | checksum is the plain byte-sum of the payload; the host sends four bytes, the bootloader compares the low 24 bits |

Replies use opcode **`0xAB`**, second byte echoing `0xC0` or `0xC2`. **Verify sets byte 4 to `0x55`
when the sum matches and the on-chip read-back found no errors, `0xAA` otherwise.**
Dispatcher at `0x1000676`: `cmd = report[0] | report[1]<<8`; `0xC0BA` → init (`0x10005ac`),
`0xC2BA` → verify (`0x1000552`).
(`docs/PROTOCOL.md:975-1007`)

**File prefix the host skips** (a host convention, *not* the write address):

| class | skip |
|---|---|
| gen2 keyboard | 20480 |
| **yc500 keyboard (our K86)** | **65536** |
| second vendor table (`3151:5024`) | 65536 |

(`docs/PROTOCOL.md:977-985`)

**Timing:** the end-to-end K86 run **"took 3397 chunks"** (`docs/PROTOCOL.md:936`).
3397 × 64 = 217,408 bytes of payload. **INFERRED:** the K86's `firmwareFile.bin` is 190,040 bytes and
the K86 v2 vendor package contains a 210,328-byte OLED image; 3397 chunks is closest to
`0x40000 - 0x10000 = 196,608` (yc500 skip 65536 of a 256 KiB image ⇒ 190,464 bytes of payload
⇒ 2976 chunks) — the two numbers do not reconcile exactly, so treat 3397 as a reported observation,
not a derived constant. (Do not build on this number.)

**Step 4 — display chip / TFT path (app mode, Bit7 on the command reports, no checksum on data):**
`0x30` enter OLED · `0xB0` poll until it echoes and `reply[1] == 1` · `0x31` start + 16-bit chunk
count · OLED data = 64-byte chunks **from file offset 65536** · `0xB1` verify with the byte-sum,
success = `reply[1] == 0x55` · `0xB2` TFT probe (continue only if `reply[1] == 1`) · `0x32` TFT data
in 56-byte pages. (`docs/PROTOCOL.md:1011-1023`) *"The display-chip path below is still unexercised,
and so is RF."* (`docs/PROTOCOL.md:938-939`)

### 2.3 The hazards sharkfin itself lists

`docs/PROTOCOL.md:1025-1033`, verbatim:

| | |
|---|---|
| Sibling images | Same USB PID does not mean the same PCB. A wrong image that fails to run can leave the board unable to answer `0x7F`, so boot is unreachable without SWD. |
| Boot PID table | The vendor tables are incomplete. A K86 (1168) boots to `3151:4016`, which is in neither. A tool that only looks for the listed PIDs will report no board while one sits in the bootloader. |
| **No USB dump** | **The dispatcher has no read. Flash contents never come out over USB.** |
| Bootloader is kept | The host never sends the file prefix above. A board that can still enter boot can be written again. The 1724 write base is `0x01010200`, which is not either skip. |
| `0x30` | Display-chip boot, not USB boot. |

### 2.4 Danger rating summary

| operation | verdict |
|---|---|
| read keymap / macro / LED / settings over HID | ✅ **SAFE** |
| read the **firmware** bytes | ⛔ **IMPOSSIBLE — no opcode exists** |
| `0xAC` flash-chip erase | 🔴 **DESTROYS DATA** (~55 s, all stored pictures) |
| `0x2C` display erase | 🔴 **DESTROYS DATA** (same flag as `0xAC`) |
| `0x7F` boot entry | 🟠 **RISKY** (with trailer: erases keymap + switch-settings blocks, then enters boot) |
| `0x30` display-chip boot | 🟠 **RISKY** |
| `0x01`/`0x02` factory reset | 🟠 **DESTROYS SETTINGS** (4 s on yc500) |
| `0x1C`/`0x1E` travel calibration | 🔴 **DESTROYS SETTINGS** (commits zeroed tables) |
| boot-mode `0xBA` image write | 🟠 **RISKY → BRICK RISK** (wrong image ⇒ board won't answer `0x7F` ⇒ needs SWD) |
| `0x0C`/`0x09`/`0x10`/`0x0B`/`0x65` writes | ✅ **SAFE-ish** — they write NVM; the hazard is the **USB endpoint stalling** (§3), not data loss |

---

## 3. PAINT PROBE / PER-KEY RGB PATH, AND THE SAFETY GATING

### 3.1 The per-key RGB path

**Opcode `0x0C` (SET_USERPIC).** Header differs per family:

| byte | yc500 | gen2 |
|---|---|---|
| 1 | `0` | pattern slot |
| 2 | length low | `0xFF` (bulk sentinel) |
| 3 | length high | page (0..6) |
| 4 | page (0..6) | page length (56, then 42) |
| 5 | `0` | `1` on the final page |
| total | **384 bytes** (128 slots × RGB, matrix order) | **378 bytes** (126 slots) |

(`docs/PROTOCOL.md:473-485`; `mod.rs:498, 503-517`; `gen2.rs:85, 100-117`)

**Chunking:** 7 pages × 56 data bytes, data starting at byte 8.
**Commit:** gen2 commits the slot to flash at `0x0802f800 + slot × 384` when page 6 carries the final
flag (`docs/PROTOCOL.md:490-494`).
**Read-back: none.** `GET_USERPIC 0x8C` "returns stable data that does not reflect writes. Treat as
write-only." (`docs/PROTOCOL.md:496-497`)

**`examples/paint_probe.rs`** (`app/src-tauri/examples/paint_probe.rs`):
- builds a 128-slot rainbow, `PER_KEY_BYTES = 384` (`:57-61`);
- sends **7 pages** via `userpic_write_packet(page, &colors)` with **100 ms between pages** (`:62-65`);
- sleeps **2 s** (`:66`);
- switches to **light mode 13** (the pattern mode) with `speed 2, brightness 4, option 0, dazzle false,
  r=0 g=200 b=200` (`:67-80`);
- `--restore <mode>` puts the lighting mode back (`:35-55`);
- the doc comment states the pattern **overwrites what the board holds** and is **not read back** (`:3-7`).

**The app's own path** — `commands.rs:939-996` `write_per_key`:
- requires exactly `PER_KEY_BYTES` (384) bytes (`:944-950`);
- `flash_cooldown()` first (`:951`);
- decides the mode switch *before* the upload — "asking afterwards means talking to a board that is
  still writing flash" (`:955-960`);
- yc500: 7 × `userpic_write_packet` with `FLASH_PAGE_GAP` (100 ms) between; gen2: `gen2::userpic_packets(0, &colors)` (`:962-973`);
- `FLASH_SETTLE` (2 s) after the batch (`:974`);
- then one LEDPARAM mode-13 write if needed (`:976-990`).

### 3.2 Rate limits (all in `commands.rs:92-120`)

| constant | value | rationale (source comment) |
|---|---|---|
| `MIN_WRITE_GAP_MS` (`wire.rs:66`) | 12 ms | "Faster stalls the endpoint until re-enum; 12 ms per report is the sustainable rate on an X86." |
| `FLASH_PAGE_GAP` | 100 ms | gap between pages of one upload |
| `FLASH_SETTLE` | 2 s | idle after a flash batch; "Vendor waits 500 ms; two uploads at that pace stalled an X86" |
| `FLASH_COOLDOWN` | 10 s | "X86: 7 reports / 500 ms dies after ~13; 3 s survives. Enforced here so no caller can skip it." |
| `KEY_GAP` | 400 ms | single-slot key writes (flash) |
| `LIGHT_GAP` | 1000 ms | lighting writes |
| `SETTING_GAP` | 1000 ms | profile / debounce / sleep / auto-OS / reset |
| `SCREEN_PAGE_GAP` | 5 ms | screen frames (a 240×135 frame is >1000 pages) |

**Measured cadence table** (`docs/PROTOCOL.md:499-510`, reproduced at `commands.rs:92-102`):

| cadence | result |
|---|---|
| 7 reports / 500 ms | endpoint dies after ~13 reports |
| 7 reports / 3 s | 42 reports, still responsive |
| 2 uploads / 5 s, pages 40 ms apart, mode switch 600 ms after | stalled on the second |
| **2 uploads / 10 s, pages 100 ms apart, 2 s settle, no redundant mode switch** | **survives** ← the shipping rate limit |

A stalled endpoint fails every report (`ioctl (SFEATURE): Protocol error`) and **does not recover on
reopen** — the device needs USB re-enumeration.

### 3.3 The safety / permission gating mechanism

There is **no single "permissions" object for destructive ops**. Gating is layered:

**(a) `src/permissions.rs` is only about OS access, not destructive ops.**
It is a macOS **Input Monitoring** consent check (`IOHIDCheckAccess` / `IOHIDRequestAccess`,
`permissions.rs:4-67`) plus `open_input_monitoring_settings` (`:69-86`) and `reveal_current_app`
(`:88-111`). On Linux the equivalent problem (the hidraw node is root-owned) is solved with a **udev
rule** that the app prints for copy-paste (`app/src/components/PermissionNotice.tsx:22-34`,
`:92-116`). Neither is a write-permission gate.

**(b) Backend write gate — `with_writable` vs `with_open`.** Every mutating Tauri command runs
through `run(state, true, …)` (`commands.rs:506-511`); reads run `run(state, false, …)` (`:497-502`).
Inside `run` (`:443-495`):

```rust
if require_writable && !open.spec.writes_supported() {
    return Err(format!("sharkfin doesn't know the {} command set, so it will not write to {}",
        open.spec.family, open.spec.label()));
}
if require_writable && open.spec.unregistered && !unregistered_ok {
    return Err("This keyboard is not in sharkfin's list. Allow changes on the notice above first.".into());
}
```
(`commands.rs:463-475`) — **two hard refusals**: an unknown family, and an unregistered board that
has not been consented to.

**(c) The consent grant — `allow_unregistered`.**
`commands.rs:361-372`: a session-scoped boolean `Inner::unregistered_ok` (`:32-33`), set by a
one-click **"Allow changes"** button (`app/src/components/UnregisteredNotice.tsx:58-62`) and **cleared
with the handle** (i.e. it does not survive a disconnect). It **sends nothing** — it only lifts the
refusal in `run`. The board's family was derived from its own answers (`derive.rs:126-180`), so the
owner is shown *which* family was detected before consenting (`UnregisteredNotice.tsx:46-53`).

**(d) `read_only` surfaced to the UI.** `ConnectedDevice.read_only` (`commands.rs:147-149`) is computed
as `!spec.writes_supported() || (spec.unregistered && !unregistered_ok)` (`commands.rs:66-78`) and
drives a `ShieldAlert`/`ShieldCheck` badge and a `ReadOnlyNotice` component.

**(e) The owner record.** `apply_owner_record` (`commands.rs:376-402`) stores, per device id, the
owner's answers (magnetic? side light? profile count?) and may set `unregistered_ok` **both ways** —
"the derived spec says no; the record is the only other word" (`:384-396`). A withdrawn yes goes back
to no.

**(f) The felt-test slot.** `set_switch_trial` (`commands.rs:404-419`) opens **exactly one key slot**
for the switch felt test. The test writes, reads back, asks, and restores; only then may the record
allow switch writes (`:405-406`).

**(g) Cable-only destructive ops.** `require_cable` (`commands.rs:428-439`) — *"Factory reset and
screen frames: cable only. Bare opcode, no read-back; the receiver drops a packet if the board
dozed."* Used by `factory_reset` (`:843`) and `set_clock` (`:863`).

**(h) Wire-level pacing gates that "no caller can skip".** `flash_cooldown` / `write_gap` /
`stamp_write` (`commands.rs:882-933`) are called *inside* the command bodies, not in the frontend —
*"a limit the frontend enforces is one a bug can skip"* (blog). The read path also waits out a
write's quiet window before touching the wire (`commands.rs:449-460`): *"Reads share the wire with
flash writes. A read in a write's quiet window stalls the same way."*

**(i) Family-scoped write refusal inside `ops`.** `need(fc)` (`ops.rs:20-22`) refuses to resolve an
opcode at all when the family is unknown. `bulk_fallback_allowed` (`ops.rs:174-176`) refuses the
whole-layer upload on magnetic boards, sub-layers, and slots ≥126.

**(j) Read-only sweep discipline.** `BUNDLE_PROBES` (`ops.rs:390-424`) contains only GET opcodes
precisely so that an unknown board can be identified: *"Do not sweep `0x80..` blind. gen2 `0xAC` erases
the flash chip; on yc500 the same byte is the `0x2C` display erase. The sweep runs before the family
is known. Every entry must be a harmless read in both families."* (`ops.rs:385-388`)
`probe_sweep` (`ops.rs:433-458`) is read-only and aborts on a stall.

**(k) The escape hatch.** `raw_command` (`commands.rs:1828-1844`) lets the frontend send **any**
opcode with any payload and any checksum mode. It runs through `with_open` (not `with_writable`), so
it bypasses the write gate — **it is a debugging tool, and a dangerous one for destructive opcodes**
(it also does a `roundtrip`, i.e. it *expects* a reply, which is wrong for `0xAC`/`0x7F`).

**What sharkfin does NOT gate:** there is no per-opcode allow-list at the backend. `0xAC` and `0x2C`
are simply never called from any command; the only protection is that nobody wrote a command for them.
`GEN2_FLASH_CHIP_ERASE` (`cmd.rs:45`) is a `pub const` that is **never referenced** anywhere in the
crate (verified by grep across the whole `app/src-tauri/src` tree).

---

## 4. `devices.json` — THE DEVICE MODEL, AND THE K86 ENTRY

### 4.1 The model

`app/src-tauri/data/devices.json` is a **JSON array of 1409 `DeviceSpec` objects**, one per board.
The Rust type is `registry.rs:30-96`:

| field | type | meaning |
|---|---|---|
| `id` | u32 | the board's **handshake id** — the u32 the board returns in the `0x8F` identify reply. **This is the key to everything vendor-side** (the firmware channel is keyed on it). |
| `name` | String | internal slug, e.g. `yc3121_sg8967_3m_soc` |
| `displayName` | String | marketing name, e.g. `K86` |
| `company` / `vendor` | String | brand as sold / brand as the vendor table files it |
| `vendorId` / `productId` | u16 | USB ids. **Not unique** — over 100 boards share `3151:4015` |
| `internalName` | String | the vendor driver's class name; the **prefix encodes the chip lineage** (`yc3121_`, `yc3123_`, `ry5088_`, `ry6602_`, `ry6609_`, `pan1086_`, `yc500_`) — this is what gates screen-draw and hall-write support (`registry.rs:240-247, 289-308`) |
| `keyLayout` | String | the picture/layout file in `app/src/lib/layouts/` |
| `lightLayout` | String | key into `data/light-layouts.json` (effects, speed/brightness ranges) |
| `sideLightLayout` | String | key into the side-light layout table |
| `profiles` | u8 | number of onboard profiles |
| `magnetic` | bool | Hall-effect switches |
| `family` | String | `"yc500"` \| `"gen2"` \| `"unknown"` — **the single most important field**; unknown ⇒ read-only |
| `bulkKeymap` | bool | this board's firmware has no single-slot key write (`0x13`/`0x15`); use the whole-layer `0x09`/`0x10` upload |
| `screen` | `Option<ScreenSpec>` | `{w, h, mode, layers}`; `mode` `"16"` = RGB565, `"24"` = 3 bytes/pixel |
| `travel` | `Option<TravelSpec>` | vendor travel ranges in mm |
| `switchReplaceable` | bool | |
| `switchTypes` | `Vec<SwitchType>` | `{code, name}` — the switch models the vendor UI offers |
| `features` | `DeviceFeatures` | `knob[]`, `debounce`, `sleep24`, `sleepBT`, `magneticSwitches`, `screen`, `sideLight` (`registry.rs:9-28`) |
| `unregistered` | bool | true only for a board sharkfin derived from its own answers |
| `ledFlagsSwapped` | bool | this board reads the LEDPARAM flags nibble the other way round (8 = fixed, 7 = rainbow) |
| `ledFlagsSource` | &str | `"hardware"` / `"firmware"` / `"vendor driver"` / `""` (not serialised; attached at load) |

### 4.2 The K86 entry — **our board, VID `3151` / PID `4015`, handshake id 1168**

Verbatim from `app/src-tauri/data/devices.json`:

```json
{
  "id": 1168,
  "name": "yc3121_sg8967_3m_soc",
  "displayName": "K86",
  "company": "AttackShark",
  "vendor": "AttackShark",
  "vendorId": 12625,
  "productId": 16405,
  "internalName": "yc3121_sg8967_3m_soc",
  "keyLayout": "Common89_SG8967_v2",
  "lightLayout": "se",
  "sideLightLayout": "",
  "profiles": 3,
  "magnetic": false,
  "family": "yc500",
  "screen": {
    "w": 240,
    "h": 135,
    "mode": "16",
    "layers": 5
  },
  "travel": null,
  "switchReplaceable": false,
  "switchTypes": [],
  "features": {
    "knob": [],
    "debounce": true,
    "sleep24": true,
    "sleepBT": true,
    "magneticSwitches": false,
    "screen": true,
    "sideLight": false
  }
}
```

- `vendorId 12625 = 0x3151`, `productId 16405 = 0x4015` ✔ matches our board.
- **family `yc500`** → the *first* column of every table in §1.2.2.
- `internalName` prefix `yc3121_` → the yc3121 chip lineage (firmware base `0x01000000`, per
  `docs/PROTOCOL.md:317-318`).
- `screen` 240×135, RGB565 (`mode "16"`), **5 picture slots**.
- `profiles: 3`, no magnetic switches, no knob, no side light, debounce + 24h sleep + BT sleep on.
- `keyLayout: Common89_SG8967_v2` → `app/src/lib/layouts/vendor/Common89_SG8967_v2.json` (13,909 B).

> **Two other K86s exist in the registry** (do not confuse them with ours):
> `id 2730` = `yc3123_sg_sg8967_3m_1k`, PID `0x5002`, **gen2**, screen 240×135×5, knob + side light,
> layout `Common89_SG8967_v3`; and `id 4094` = `ry6602_sg_sg8967_3m_1k_1k`, PID `0x5056`, gen2,
> layout `Common87_SG8967`.
> ⚠️ **Discrepancy to note:** `mehmetnadir/keyboard-studio`
> (`Sources/KeyboardKit/Resources/Devices/attackshark-k86.json`) describes handshake id 1168 with a
> **128×128** screen, `"verified": false` — sharkfin says 240×135. sharkfin's number comes from the
> vendor record and the yc3121/yc3123 firmware images; trust sharkfin, but the screen geometry is worth
> re-verifying on our actual unit before shipping a frame.

---

## 5. `keymap-evidence.json` — WHAT THE "offset" AND "firmware version" MEAN

### 5.1 Structure

`app/src-tauri/data/keymap-evidence.json` (899,691 B) is a **JSON object of 230 records keyed by the
device id as a string**. Each record:

| field | type | meaning |
|---|---|---|
| `board` | String | the board's name at the time of the record ("(not in the registry)" for boards the registry has not caught up with) |
| `company` | String | **only present** for boards not in the registry (the channel record's only description of them) |
| `firmware` | String | **the vendor package's version string**, e.g. `v115_oledv108_flashv101`. It is a *compound* name: `v115` = main keyboard MCU firmware, `oledv108` = display-chip firmware, `flashv101` = flash-controller (NVM) firmware, `mledv105` = second LED chip, `rfv200` = RF firmware, `nordicv105` = Nordic radio, `v113` etc. = the main firmware. |
| `image` | String | **which image inside the vendor package the table was found in**: `"raw"` (the package is a single raw-deflate stream and the whole thing is the image), `"firmwareFile.bin"` (the keyboard MCU image inside the ZIP), `"firmwareRFFile.bin"` (the RF image) |
| `offset` | int | **byte offset of the factory keymap table inside that image** |
| `copies` | int | how many times the table is repeated in the image (9 for yc3121/8-page+settings copies, 3 for gen2 profiles, 1 for some) |
| `method` | String | `"anchored"` = located via the vendor JS `defaultMatrix` bytes; `"escape"` = located by scanning for the Escape entry |
| `matrix` | int[] | the **extracted keymap table** — 4-byte slot entries, exactly the bytes `GET_KEYMATRIX` returns |

Distribution across the 230 records: `image` = `raw` 154 / `firmwareFile.bin` 63 / `firmwareRFFile.bin` 13;
`method` = `escape` 146 / `anchored` 84; `copies` = 9 (123 boards), 1 (78), 3 (14), 10 (12), 7 (2), 6 (1).

**The K86 record (device 2730):**

```json
"2730": {
  "board": "K86",
  "firmware": "v115_oledv108_flashv101",
  "image": "firmwareFile.bin",
  "offset": 163324,
  "copies": 1,
  "method": "anchored",
  "matrix": [ ... ]
}
```

### 5.2 Does sharkfin ever have an actual firmware IMAGE?

**NO. Not one byte of firmware image is committed to the repository.**

- The GitHub tree of `dniminenn/sharkfin` (master, `b36c790`) contains **zero** files ending in
  `.bin`, `.hex`, `.img`, `.dfu`, `.elf`, `.uf2`, `.fw`, `.dat`, `.zip`, `.tar`, `.gz`, `.rar`, `.7z`.
  Verified programmatically against the recursive tree API.
- `keymap-evidence.json` stores only the **extracted tables** plus metadata pointing at an offset
  *inside* an image the repo never holds.
- The tool that produces it, `tools/firmware_keymaps.py`, **downloads** the packages from the vendor
  channel and **caches them outside the repo**:
  `~/vendor-builds/firmware/pkg` (`tools/firmware_keymaps.py:51-52` for the URLs, `:195` for the cache
  default, `:157-181` `fetch()` writing `<dev_id>_<version>.pkg`).
- `CONTRIBUTING.md:185-215` says so in words: *"The vendor publishes firmware per board … It downloads
  each board's package once (cached under `~/vendor-builds/firmware/pkg` …) and records the board,
  firmware version, image member, offset, copy count, how the table was located and the table itself
  in `app/src-tauri/data/keymap-evidence.json`, **which is committed, so regenerating the registry
  downloads nothing**."*
- The `sharkfin` GitHub **releases** (v0.6.0 … v0.10.0, 15 releases) carry only app artifacts
  (`.rpm`, `.deb`, `.AppImage`, `.dmg`, `.msi`, `.exe`, web `.tar.gz`). **No firmware asset.**

**Is a K86 firmware image publicly available in that repo?** **NO.** The K86 is present in
`keymap-evidence.json` only as an *extracted keymap table* for device 2730 (the gen2 K86 v2).
**Our board, device 1168, is absent from `keymap-evidence.json` entirely** — because the vendor
channel has no record for 1168, so there is no image to read a table out of. (Verified: `1168` is not a
key in that file. `matrix-evidence.json` (16 records) and `devices.legacy.json` (122 records) also have
no entry for 1168, 2730 or 4094.)

**What IS publicly downloadable:** see §6.

---

## 6. FIRMWARE BINARY HUNT

### 6.1 What I found and verified (downloadable today)

| # | what | URL | size | claims to be | verified |
|---|---|---|---|---|---|
| **F1** | **Attack Shark X65HE keyboard firmware** (device id **2268**, gen2 / `ry5088` lineage) | `https://github.com/subhassh/attackshark-keyboard-firmware` → `firmware/2268_v309.bin` (raw: `https://raw.githubusercontent.com/subhassh/attackshark-keyboard-firmware/main/firmware/2268_v309.bin`) | **46,469 bytes** | "**Firmware Version: v309, Device ID: 2268**" — the official vendor update image, archived "to help recover keyboards that become stuck in bootloader mode when the vendor's backend fails to serve firmware" | ✅ downloaded. SHA256 `185034679bf6dbc141d106fddbe5b1c7fc414b3261d9429d71501e5724f75fe7` (matches the repo's `hashes.txt`). Content is **raw-deflate** → inflates to **117,852 bytes**, ARM Cortex-M image at base `0x08000000` (head `b04e0020 c1020008 77040008 6f040008` = vector table). **This is the exact image sharkfin's blog disassembled to reverse the gen2 protocol.** It is *not* a K86 image. |
| **F2** | **Attack Shark K86 v2 (yc3123, gen2, device id 2730, PID `0x5002`) vendor firmware package** | `https://api2.rongyuan.tech:3816/download/fw_upgrade_file/2730_v115_oledv108_flashv101` | **120,107 bytes** (ZIP) | The vendor's own update package, version `v115_oledv108_flashv101`, `lowest_app_version_str: 3.0.28` | ✅ downloaded and opened. ZIP members: **`firmwareFile.bin` 190,040 B** (keyboard MCU firmware), **`firmwareFlashFile.bin` 38,594 B** (flash-controller / settings firmware), **`firmwareOledFile.bin` 210,328 B** (display-chip firmware). The keymap table lives in `firmwareFile.bin` at offset **163,324** (= the `offset` in `keymap-evidence.json`). |
| **F3** | The **vendor firmware channel API** (metadata + the file path for any device id) | `POST https://api2.rongyuan.tech:3816/api/v2/get_fw_version` with body `{"dev_id": <u32>}` → `{"code":0,"data":{"version_str":…,"file_path":"fw_upgrade_file/<id>_<version>",…}}`; then `GET https://api2.rongyuan.tech:3816/download/<file_path>` | — | The vendor's own per-board update channel (the one the Attack Shark / GearHub updater uses) | ✅ live. Source: `tools/firmware_keymaps.py:51-52, 148-176` |

### 6.2 Our board specifically — **NO firmware is published for it**

```
POST https://api2.rongyuan.tech:3816/api/v2/get_fw_version   {"dev_id": 1168}
→ HTTP 500   "Record not found"
```

`tools/firmware_keymaps.py:133-155` documents this exact answer as the "no firmware published" case:
*"A missing record arrives as HTTP 500 with 'Record not found' in the body."*

Full AttackShark survey against the channel (all 73 AttackShark device ids in `devices.json`):

| dev id | board | channel answer |
|---|---|---|
| 1025 | X65HE (yc500) | HTTP 500 — no record |
| **1168** | **K86 v1 (yc3121) — OUR BOARD** | **HTTP 500 — no record** |
| 1343 | K75RGB | `v101` → `fw_upgrade_file/1343_v101` |
| 1466 | K85 (mag) | `v107_mledv105` → `fw_upgrade_file/1466_v107_mledv105` |
| 1679 | K75RGB (dm) | HTTP 500 |
| 1724 | X85PRO | `v104_oledv104` → `fw_upgrade_file/1724_v104_oledv104` |
| 1967 | X86 | HTTP 500 |
| 1996 | X98PRO | `v113_oledv102` → `fw_upgrade_file/1996_v113_oledv102` |
| 2268 | X65HE (gen2) | `v309` → `fw_upgrade_file/2268_v309` |
| 2730 | **K86 v2 (yc3123)** | `v115_oledv108_flashv101` → `fw_upgrade_file/2730_v115_oledv108_flashv101` |
| 2755 | X68MAX | `v504` → `fw_upgrade_file/2755_v504` |
| 2935 | X82PRO HE | `v503` → `fw_upgrade_file/2935_v503` |
| all others | — | HTTP 500 (no record) |

> ⚠️ The channel **rate-limits**: a burst of requests is answered with a bare **HTTP 400**, which is
> *not* an answer about the board (`tools/firmware_keymaps.py:135-140, 212-215`). If you script a
> survey, sleep ≥1 s between calls and treat HTTP 400 as "retry later", never as "no firmware".
> (My own survey got HTTP 400 from id 3743 onward until it stopped.)

### 6.3 Repos checked for firmware binaries — all negative except F1

| repo | result |
|---|---|
| `dniminenn/sharkfin` | tree: **0** binary/firmware files. 15 releases, app artifacts only. |
| `tsensei/Attack-Shark-K86` | 1 file: `Readme.md` (1270 B). No code, no firmware. |
| `TheElevatedOne/K86-Software-Linux` | full C++/GTK source (`src/`, `docs/PROTOCOL.md`). No firmware. **Its `docs/PROTOCOL.md` is a valuable second K86-specific protocol doc** (device table for `yc3121_sg8967_3m_soc` = firmware id **1168**, VID:PID `3151:4015`; and `yc3123_sg_sg8967_3m_1k` = 2730, `3151:5002`) — but it also documents **no** firmware read-back path. |
| `Xynthera/AttackShark_K86_Spotify` | 1 Python script + README. No firmware. |
| `OhAleeh/AttackShark_K86_Media` | 1 Python script + README. No firmware. |
| `RaphaelCaputo2/shark-k86-mac` | Python package + `PROTOCOL.md`. No firmware. |
| `mehmetnadir/keyboard-studio` | Swift app. **No firmware.** But `Sources/KeyboardKit/DangerousCommands.swift` is the best independent destructive-opcode blocklist found — see §2.1. |
| `cykorr/AttackSharkX68HE` | README only. |
| **`subhassh/attackshark-keyboard-firmware`** | ✅ **F1 — the only publicly downloadable ROYUAN keyboard firmware image found.** |

### 6.4 Official vendor download hosts (exact paths)

| host | what | URL |
|---|---|---|
| ROYUAN update channel | per-board firmware packages | `https://api2.rongyuan.tech:3816/download/fw_upgrade_file/<dev_id>_<version_str>` |
| Attack Shark CN driver page | per-model driver/firmware `.zip`/`.rar` (model↔file mapping is rendered client-side; the static HTML only has the `upload/<timestamp>.zip|rar` links) | `https://www.attackshark.cn/downs.aspx?ClassID=19` → files at `https://www.attackshark.cn/upload/<timestamp>.zip` |
| Attack Shark CN driver list | another class | `https://www.attackshark.cn/downs.aspx?ClassID=22` |
| Attack Shark file CDN (Shopify) | **manuals** (e.g. `https://download.attackshark.pro/ATTACKSHARK/K86/K86UserManual.pdf`), not firmware | `https://download.attackshark.pro/` |
| Attack Shark in-app file host | the driver's own update file host | `https://controlhub.top/AttackShark/` (returns a JS shell) |
| Attack Shark global driver page | user-facing download index | `https://attackshark.com/pages/driver-download` |

**Bottom line for §6: exactly ONE public ROYUAN keyboard firmware image exists on GitHub
(`2268_v309.bin`, X65HE, 46,469 B) plus a live vendor channel that serves a K86 **v2** package.
There is NO public image for our K86 v1 (dev id 1168).**

---

## 7. RECOMMENDED IMPLEMENTATION PLAN FOR SHARKDECK

### 7.0 The honest summary first

| Feature | Feasible? | Notes |
|---|---|---|
| **"Backup firmware" (read the board's firmware bytes out)** | ❌ **NOT POSSIBLE over HID.** No read opcode exists in the bootloader; flash contents never leave the chip over USB. | If we want a *usable* backup, the only honest option is (a) back up the **settings** (keymaps/macros/lights) — sharkfin already does this; or (b) archive the **vendor's** image for our board id and store it locally as the "factory" reference. Option (b) is impossible for dev id **1168** today because the vendor publishes nothing for it. |
| **"Restore firmware" (write an image back)** | 🟠 **Possible but high-risk.** The protocol is fully documented in `docs/PROTOCOL.md:927-1033` and was run end-to-end on a K86 (1168) by an outside user. Bricking risk is real. | Requires: the exact image, the exact prefix skip, the boot PID discovery, and a rescue path. |
| **"Backup / restore settings"** | ✅ **Fully possible and safe** — this is the feature to ship. | Mirror sharkfin's `export_config`/`import_config`. |

**Therefore: ship "Backup & Restore **settings**" now, label the firmware paths as an explicit
"Experimental / Advanced" feature, and never call a settings backup a firmware backup.**

### 7.1 Phase 1 — Settings backup & restore (SAFE; ship first)

1. **Model the config** on sharkfin's `SavedConfig` (`session.rs`; `commands.rs:1528-1544`):
   `{version: 1, device_id, family, board, profiles: Vec<[512]>, fn_layers: Vec<[512]>, led, side_light, debounce, sleep, options}`.
2. **Read** (all read-only, all proven):
   - keymaps: yc500 `0x89 [profile, page]` ×8 pages; gen2 `0x8A [profile, 0xFF, page, sublayer]` ×8.
     (`ops.rs:221-243`) — note magnetic yc500 uses `profile * 4 + sublayer` (`mod.rs:150-156`).
   - Fn layers: `0x90` (`ops.rs:235, 237`).
   - LEDPARAM `0x87`, side light `0x88`, debounce `0x91`/`0x86`, sleep `0x92`/`0x91`,
     options `0x86`/`0x89`, auto-OS `0x97`, profile `0x85`/`0x84`, revision `0x80` (+ `0x8F` bytes 7..9).
   - **macros** `0x8B` are readable and read back faithfully (`docs/PROTOCOL.md:468-469`) — but
     sharkfin does **not** export them. Adding macro export is a genuine improvement over sharkfin.
3. **Write** with the exact per-family shapes from §1.2 (yc500 vs gen2 differ on debounce byte,
   sleep reply offset, keymap header, USERPIC header, macro opcode, reset opcode).
   Prefer single-slot writes (`0x13`/`0x15` on yc500, `0x0A` on gen2) and **fall back to the
   whole-layer bulk upload** only when a slot write fails its read-back check — exactly sharkfin's
   `write_slot_checked` → `bulk_fallback_allowed` logic (`ops.rs:185-218`, `:174-176`).
4. **Verify after write**: read the layer back and diff. sharkfin's `write_slot_checked` reads at
   `slot % 16` and waits `settle, 50 ms, 150 ms` (`ops.rs:204-217`).
5. **Safety rails** (borrow sharkfin's exact numbers):
   - 12 ms floor between any two feature reports (`wire.rs:66`);
   - **10 s** cooldown after any flash-backed write (`commands.rs:94`);
   - **100 ms** between pages of an upload, **2 s** settle after the batch (`commands.rs:98, 102`);
   - **1 s** after any single-slot key write (`commands.rs:112` KEY_GAP = 400 ms — use 1 s to be safe);
   - **1 s** after any lighting or settings write (`commands.rs:116, 120`);
   - **never read on the wire inside a write's quiet window** — sharkfin explicitly waits it out
     (`commands.rs:449-460`), because a read 120 ms after a profile switch wedged an X86's endpoint.
6. **Never send `0xAC`, `0x2C`, `0x7F`, `0x30`, `0x01`/`0x02`, `0x1C`, `0x1E` from a settings path.**
   Put a hard denylist in the transport layer (keyboard-studio's `DangerousCommands` is a good
   starting list) so a future feature cannot trip over them.
7. **Wipe protection:** before any bulk write, take the backup first and verify it is on disk.
   Refuse to write if the backup file cannot be written and re-read.

### 7.2 Phase 2 — Firmware WRITE (the "Restore firmware" button) — READ-ONLY PROBES FIRST

**Step 2.0 — read-only experiments to run against the real K86 (dev id 1168) BEFORE any write.**
These are all *reads* or *harmless*:

| # | experiment | exact opcodes | what it tells you | risk |
|---|---|---|---|---|
| **E1** | Identify + family confirm | `0x8F` (read) → expect `1168`; `0x80` (read) → expect the version word; `0x89`/`0x8A`/`0x91`/`0x92` (reads) to confirm **yc500** per `derive.rs:58-68` | confirms we are on the yc500 family and not a mis-identified board | ✅ safe |
| **E2** | Version + chip inventory | `0xAD` (screen fw ver), `0xAE` (2nd chip ver) | the K86 has a display; tells you which of the three images in the vendor package is live. **Caveat: "a reply is not evidence"** — a board without a display echoes the previous reply; compare against the previous response, not zero (`docs/PROTOCOL.md:1047-1048`) | ✅ safe (read-only) |
| **E3** | Screen announce handshake, **without pages** | `0xA5` announce (yc500 mode-16 pair) with a tiny bbox, then **stop** | tells you the yc500 screen-parse lineage and whether the announce is accepted (`reply[1] == 1`). **Do NOT send `0xAC`/`0x2C` first** — the announce "erases nothing. It resets counters." (`docs/PROTOCOL.md:792`) | 🟡 low risk: resets the frame counters only |
| **E4** | Boot-PID discovery (read-only side effects only) | send `[0x7F, 0x55, 0xAA, 0x55, 0xAA]` and then **poll the USB bus, do not send any `0xBA`** | discovers the real boot PID for our board (the vendor table says `3151:4016` for the K86, but "Which PID a given board lands on is not established beyond that table"). **This is the single most valuable unknown.** | 🟠 **RISKY** — `0x7F` with the trailer *"erases the keymap block and the switch-settings block and enters the bootloader"* (`docs/PROTOCOL.md:691-694`). It puts the board in a state we cannot read back out of. **Only do this with a settings backup already on disk, and with a known-good rescue path.** |

**Step 2.1 — the write itself.** Only after E1–E3 pass and E4 has been planned:

1. Obtain the image. **Our board id 1168 has no vendor firmware.** Options, in order of preference:
   a. Ask the owner of a K86 (1168) to run the vendor updater once and share the downloaded package
      (the vendor's updater on a board with a record downloads `fw_upgrade_file/<id>_<ver>` — but 1168
      has no record, so this will not work either);
   b. **Capture the factory image from the board's own flash** — **not possible over HID**; it would
      require SWD/scope on the chip, which is out of scope for a desktop app;
   c. Use a **sibling board's image** (e.g. the X86 `1967`, X98PRO `1996`, X85PRO `1724` — all
      `yc3121_*`, same chip lineage, same PID `0x4015`). ⚠️ **This is exactly the "Sibling images"
      hazard** (`docs/PROTOCOL.md:1029`): *"Same USB PID does not mean the same PCB. A wrong image
      that fails to run can leave the board unable to answer `0x7F`, so boot is unreachable without
      SWD."* — **Treat sibling images as a last resort, and require the user to type a scary
      confirmation.**
2. **Compute the payload correctly.** The host skips a **file prefix**, not a write address:
   - yc500 keyboard: skip **65536**; gen2 keyboard: skip **20480**; `3151:5024`: skip 65536
     (`docs/PROTOCOL.md:977-985`).
   - ⚠️ *"The prefix is a host convention, not the 1724 write address."* — and *"the 1724 write base
     is `0x01010200`, which is not either skip."* (`docs/PROTOCOL.md:978, 1032`)
   - The package we can download (F2) is a **ZIP with three members** — you must pick the right
     member (`firmwareFile.bin` for the keyboard MCU, `firmwareOledFile.bin` for the display chip,
     `firmwareFlashFile.bin` for the flash controller) and know which one the boot loader expects.
     **INFERRED, and unverified:** the boot transfer writes a *single* linear image, so the OLED and
     flash-controller images are probably flashed over a different path (the `0x30`/`0xB0`/`0xB1`
     display-chip path in `docs/PROTOCOL.md:1011-1023`, and possibly the `0x7F` RF path with
     `0xF8`). **This is not established and must be probed, not assumed.**
3. **Enter boot:** `[0x7F, 0x55, 0xAA, 0x55, 0xAA]`, Bit7 checksum over bytes 0..6 into byte 7
   (`docs/PROTOCOL.md:947`; `mod.rs:24-36`).
4. **Wait for re-enumeration on the boot PID.** Enumerate **all** PIDs, not just the vendor's table —
   our board may land on `3151:4016` (app PID + 1) or somewhere not in any table
   (`docs/PROTOCOL.md:960-970, 1030`).
5. **Transfer (no Bit7 checksum in boot mode):**
   - init `[0xBA, 0xC0, cntLo, cntHi, len0, len1, len2]`, `cnt = ceil(len/64)`, `len` 24-bit;
   - data: 64-byte chunks, paced (the boot endpoint tolerates far less than 12 ms? — **unknown**;
     the safe default is 12 ms, the same as app mode, until measured);
   - verify `[0xBA, 0xC2, cntLo, cntHi, s0, s1, s2, s3, len0, len1, len2, len3]`, checksum = plain
     byte-sum of the payload (4 bytes, low 24 bits compared);
   - read the reply (`0xAB`, second byte `0xC0`/`0xC2`): **verify byte 4 must be `0x55`**. `0xAA` = fail.
6. **Re-enumerate and check** the board comes back: `0x8F` must answer with id 1168, `0x80` must
   report the new version, and the keymap must read back.
7. **Verify-after-write:** compare the flashed version against the target, and confirm the board
   answers `0x7F`-able again (i.e. it did not brick into a non-bootable image).

**Step 2.2 — the safety rails for a firmware write.** Non-negotiable:

| rail | rule |
|---|---|
| **Read-only first** | Never let the user reach the flash button before the read path (§7.1) works and the device is identified. |
| **Mandatory backup before any write** | A firmware write must be blocked unless a **settings backup file exists on disk AND re-reads successfully**. The settings backup is not a firmware backup — say so in the UI. |
| **Image provenance** | Store the image's SHA256, its source URL, its vendor version string, and the dev id it was taken from. Refuse an image whose `dev id` ≠ the connected board's id unless the user explicitly overrides for a known sibling lineage. |
| **Checksums** | Verify the host-side byte-sum before sending anything. Verify `0x55` in the boot verify reply. Refuse to proceed on `0xAA`. |
| **Verify-after-write** | Re-identify (`0x8F`), re-read the version (`0x80`/`0x8F[7..9]`), re-read the keymap, and diff against the pre-write backup. |
| **No sibling-image flashing by default** | Disabled behind an explicit typed confirmation, and only for boards in the same `internalName` lineage prefix. |
| **Cable only** | Force USB cable for any boot/flash operation. sharkfin's `require_cable` pattern (`commands.rs:428-439`). |
| **No RF flashing** | The RF path (`0xF8` boot, `0xF7` status) is entirely unexercised (`docs/PROTOCOL.md:938-939, 1009`). |
| **Hard denylist** | `0xAC, 0x2C, 0x7F, 0x30, 0x31, 0x01, 0x02, 0x1C, 0x1E, 0x43, 0x44, 0xC3, 0xC4, 0x40, 0x41` blocked by default in the transport; unlockable only by a named "advanced" path. |
| **Bricking escape hatch** | State plainly in the UI: a wrong image can make the board unable to answer `0x7F` and **require SWD** (`docs/PROTOCOL.md:1029`). There is no undo except re-flashing a good image, which requires the board to still enumerate. |

### 7.3 The honest risks, in one place

| risk | severity | detail |
|---|---|---|
| **Bricking** | 🔴 high | A wrong/partial image can leave the board unbootable without SWD. The bootloader has **no read**, so you cannot inspect what went wrong over HID. |
| **"Backup" is a lie if we call a settings dump a firmware backup** | 🔴 high (product/honesty) | There is no firmware read path. The UI must say "Backup **settings**" and "Restore **settings**", and a separate "Flash firmware image" with its own warnings. |
| **Wrong board id** | 🔴 high | `3151:4015` is shared by 100+ boards. The **handshake id (`0x8F`)** is the only discriminator. |
| **Wrong family** | 🔴 high | The two families' opcodes collide and a misaddressed write lands on a live register. Never write before the family is resolved *and* the board is in the registry or the owner has consented. |
| **Encryption / obfuscation** | 🟡 medium | The X65HE image "reads as encrypted, entropy 7.99" but is actually **raw-deflate** (the vendor's JS names the format with `inflateRaw`); inflated it is a 115 KB ARM Cortex-M image at `0x08000000` (blog). **INFERRED: other lineages may use a different container** — `tools/firmware_keymaps.py:121-131` `images()` handles three shapes (raw-deflate, ZIP, none), and one of the 230 records has `image: "firmwareRFFile.bin"` with 10 copies, so packages are not uniform. Never hard-code one container. |
| **MCU family / lineage** | 🟡 medium | Our board is `yc3121_` (firmware base `0x01000000`, `docs/PROTOCOL.md:317-318`); the gen2 images are base `0x08000000`; the 1724 bootloader loads at `0x01000000` and writes at `0x01010200`. **The boot image's load address and the app image's base differ by lineage — a mismatch here is a brick.** |
| **Flash wear** | 🟢 low | NVM writes (keymap, lighting, macros) have a finite cycle budget. A firmware write is a one-shot; a settings backup/restore cycle should be diff-based (only write what differs) to minimise wear. |
| **The vendor channel can vanish** | 🟡 medium | `subhassh/attackshark-keyboard-firmware` exists precisely because *"the official updater can't find firmware"* after the vendor's backend broke (`README.md` of that repo). **Mirror any image we obtain into SharkDeck's own storage.** |
| **Rate limiting / flakiness** | 🟡 medium | The vendor channel answers a burst with a bare HTTP 400. Any downloader we ship must back off and treat 400 as "retry", never as "absent". |
| **Screen geometry uncertainty** | 🟢 low | sharkfin says 240×135 for id 1168; keyboard-studio says 128×128 (`verified: false`). Re-measure on our unit before shipping frames. |
| **No SWD in the product** | 🔴 high | If a flash goes wrong there is no field-recoverable path. Consider a "never flash unless the user has a known-good image + a working recovery plan" policy, and make the flash feature opt-in behind an explicit expert flag. |

---

## APPENDIX A — Source index (all fetched from `raw.githubusercontent.com/dniminenn/sharkfin/master/`)

| file | lines | used for |
|---|---|---|
| `app/src-tauri/src/protocol/mod.rs` | 962 | families, opcodes, layouts, checksums, userpic/macro/led/clock packets |
| `app/src-tauri/src/protocol/cmd.rs` | 45 | the shared opcode constant table |
| `app/src-tauri/src/protocol/gen2.rs` | 117 | gen2 keymatrix/userpic/fn packet builders |
| `app/src-tauri/src/protocol/hall.rs` | ~560 | magnetic-switch sub-ops, `0x65`/`0xE5`/`0x1B`/`0x1A`/`0x1D`/`0x9D` |
| `app/src-tauri/src/protocol/receiver.rs` | 88 | 2.4 GHz receiver opcodes |
| `app/src-tauri/src/ops.rs` | 475 | read/write conversations, `BUNDLE_PROBES`, `FlashPace` |
| `app/src-tauri/src/commands.rs` | 1844 | rate limits, permission gating, export/import config, `raw_command` |
| `app/src-tauri/src/registry.rs` | 35,936 B | `DeviceSpec`, `KNOWN_FAMILIES`, `screen_draw` |
| `app/src-tauri/src/derive.rs` | 293 | `detect_family`, `derive_spec` |
| `app/src-tauri/src/permissions.rs` | 126 | macOS Input Monitoring (NOT destructive-op gating) |
| `app/src-tauri/src/wire.rs` | 314 | transport primitives, 12 ms floor, relay |
| `app/src-tauri/src/hid.rs` | 18,103 B | hidapi transport, usage-page filter |
| `app/src-tauri/src/session.rs` | 4,247 B | `SavedConfig` (`:84`), `DeviceSettings` (`:66`), `switch_access` (`:31`) |
| `app/src-tauri/data/devices.json` | 1,263,918 B | 1409 device records (K86 = id 1168) |
| `app/src-tauri/data/keymap-evidence.json` | 899,691 B | 230 keymap records (K86 v2 = id 2730) |
| `app/src-tauri/data/devices.extra.json` | 430,832 B | 234 hand-added records (no K86) |
| `app/src-tauri/data/matrix-evidence.json` | 76,630 B | 16 records (no K86) |
| `app/src-tauri/data/devices.legacy.json` | 77,865 B | 122 records (no K86) |
| `app/src-tauri/examples/*.rs` | 10 files | `paint_probe`, `probe`, `profile_probe`, `gen2_roundtrip`, `led_speed_probe`, … |
| `docs/PROTOCOL.md` | 1,052 lines | **the protocol spec** — flash/boot §927-1033, destructive §735-754, screens §756-902 |
| `docs/BOARDS.md` | 83,700 B | board list |
| `tools/firmware_keymaps.py` | 12,784 B | the vendor channel API + cache layout |
| `tools/led_flags.py`, `tools/verify_matrix.py`, `tools/extract_vendor_data.py` | — | firmware → table extraction pipeline |
| `README.md`, `CONTRIBUTING.md`, `AGENTS.md` | — | "sharkfin never flashes firmware"; the firmware-reading method |
| `app/src/components/{PermissionNotice,ReadOnlyNotice,UnregisteredNotice}.tsx` | — | the consent UX |

## APPENDIX B — Second-source protocol docs (independent of sharkfin)

| doc | value |
|---|---|
| `TheElevatedOne/K86-Software-Linux` → `docs/PROTOCOL.md` | K86-specific. Device table: `yc3121_sg8967_3m_soc` = 3151:4015, **firmware id 1168**, 3 profiles, 240×135 LCD + side light; `yc3123_sg_sg8967_3m_1k` = 3151:5002, id 2730. Documents the **write-commit window** on a YC3121: after a write the firmware keeps committing to flash; during that window every SET/GET takes ~100 ms instead of ~1 ms, and **reads return stale pages** — so a read-modify-write right after a write silently writes old data back. Busy windows: single key `13` ~0.5 s, backlight `07` ~1.1 s, per-key colours `0x0C` ×7 ~1.7 s, key/Fn matrix `09`/`10` ×9 ~2.1 s. Also: **reading ~10 ms after a write rebooted the keyboard** (USB re-enum, unsaved write lost) — polling from 500 ms on was safe. Vendor sleep 500 ms (YC3121) / 100 ms (YC3123). "Erase screen flash: `AC` → `AC AA AA 55 55`". **No firmware read-back path is documented.** |
| `RaphaelCaputo2/shark-k86-mac` → `PROTOCOL.md` | 3151:4015, HID **interface 2**, usage page 0xFFFF usage 0x0002; 65-byte hidapi buffer (leading `0x00` report id). No flash path. |
| `mehmetnadir/keyboard-studio` → `Sources/KeyboardKit/DangerousCommands.swift` | The best independent denylist; also `Sources/KeyboardKit/Resources/Devices/attackshark-k86.json` (id 1168, family "royuan", screen 128×128 `verified: false` — conflicts with sharkfin's 240×135). |
| `dnim.dev/blog/royuan-keyboard-protocol` | The origin story: two colliding command sets; the X65HE image "reads as encrypted, entropy 7.99" but is raw-deflate → 115 KB ARM Cortex-M at `0x08000000`; *"It also names the opcodes to avoid: one erases the flash chip, two pairs are bootloader entry points."* |
| `subhassh/attackshark-keyboard-firmware` → `README.md` | The recovery-firmware archive pattern; confirms the vendor backend going dark and the board landing in an unflashable bootloader state. |

## APPENDIX C — Commands used to reproduce the §6 findings (all read-only, no HID access)

```bash
# vendor channel metadata for our board (expect HTTP 500 "Record not found")
curl -s -X POST https://api2.rongyuan.tech:3816/api/v2/get_fw_version \
     -H 'Content-Type: application/json' -d '{"dev_id": 1168}'

# vendor channel metadata for the gen2 K86 (expect a file_path)
curl -s -X POST https://api2.rongyuan.tech:3816/api/v2/get_fw_version \
     -H 'Content-Type: application/json' -d '{"dev_id": 2730}'

# download + list the K86 v2 package members
curl -s -o fw.pkg https://api2.rongyuan.tech:3816/download/fw_upgrade_file/2730_v115_oledv108_flashv101
python3 -c "import zipfile;print([(i.filename,i.file_size) for i in zipfile.ZipFile('fw.pkg').infolist()])"

# the one public ROYUAN firmware image
curl -sLo 2268_v309.bin https://raw.githubusercontent.com/subhassh/attackshark-keyboard-firmware/main/firmware/2268_v309.bin
sha256sum 2268_v309.bin   # 185034679bf6dbc141d106fddbe5b1c7fc414b3261d9429d71501e5724f75fe7
python3 -c "import zlib;print(len(zlib.decompress(open('2268_v309.bin','rb').read(),-15)))"   # 117852

# prove sharkfin ships no firmware binary
curl -s "https://api.github.com/repos/dniminenn/sharkfin/git/trees/master?recursive=1" \
 | python3 -c "import json,sys;print([t['path'] for t in json.load(sys.stdin)['tree'] if t['path'].endswith(('.bin','.hex','.img','.dfu','.uf2','.elf','.zip','.rar'))])"
# -> []
```
