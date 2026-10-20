# Opcode findings — measured on the live K86 (fw 1.19)

All probes are **read-only**: they only use GET-family opcodes and never touch
`0xAC` (flash erase), `0x2C` (display erase), `0x7F`/`0x30` (boot entry) or
`0x02` (factory reset).

## Method

`sharkdeck-cli opprobe [ops…]` classifies each opcode by diffing against a
poison baseline: it sends `0x8F` (device id = 1168 → `8f 90 04 00 …`) first, then
the probed opcode, then `0x8F` again.

* reply **== baseline** → the opcode is unimplemented, the firmware just replayed
  the previous answer ("stale").
* reply **starts with the opcode** → implemented.
* reply without the opcode echo → echoless / raw data payload.

`sharkdeck-cli opfull [ops…]` dumps the full 64-byte reply as hex plus 32-bit
big-endian words.

## Results (fw 1.19, 240×135 board)

| opcode | reply (bytes 0..11) | verdict | reading |
|--------|---------------------|---------|---------|
| 0x80 | `80 13 01 …` | implemented | firmware version (major 1, minor 19) |
| 0x82 | `82 00 00 00 00 00 00 7d` | implemented | all-zero payload, no data yet |
| 0x83 | `83 00 01 00 00 00 00 7c` | implemented | counter/status = 1 |
| 0x84 | `84 00 01 00 00 00 00 7b` | implemented | counter/status = 1 |
| 0x85 | `85 <profile> …` | implemented | active profile (0..2) |
| 0x86 | `86 …` | implemented | option bytes (win lock, mac, wasd, LED-off, …) |
| 0x87 / 0x88 | `87/88 …` | implemented | main / side lighting state |
| 0x89 / 0x90 / 0x8B | raw page data, no echo | echoless | keymap page / Fn page / macro page |
| 0x8A | all `0xFF` | implemented | yc500-family fingerprint (matches sharkfin) |
| 0x8D | `8d 00 00 00 00 00 00 72` | implemented | all-zero payload |
| 0x8F | `8f 90 04 00 …` | implemented | device id 1168 |
| 0x91 | `91 00 03 00 00 00 00 6e` | implemented | debounce family (audit says GET) — value 3 |
| 0x92 | `92 b4 00 b4 00 54 06 54 06 54 06` | implemented | **undocumented**: 180, 0, 180, 0, then three (0x54, 0x06) pairs |
| 0x97 | `97 00 00 00 00 00 00 68` | implemented | all-zero payload |
| 0x36 | == baseline | **unimplemented** | contradicts the vendor client's slot read |
| 0x81, 0x8E, 0x93–0x96, 0x98–0x9F | == baseline | unimplemented | replay the previous reply |

## Checksum convention (confirmed on replies)

`reply[7] = 0xFF - (sum(reply[0..7]) & 0xFF)` — verified for 0x82 (0x7d),
0x83 (0x7c), 0x84 (0x7b), 0x8D (0x72), 0x91 (0x6e), 0x97 (0x68).
`0x92` does **not** follow that rule (its byte 7 is 0x54, data-like), so its
payload layout is different from the standard 7-byte reply.

## Consequences

* **No HID read returns firmware bytes.** A per-unit flash image backup is
  impossible over this transport (audit: "the dispatcher has no read"). Only
  the *settings* can be backed up — and that is now implemented and verified
  (3 profiles × base+Fn layers, 50 macros, options, lighting).
* The 5 image slots are a **write parameter**, not a display selector, and no
  opcode selects the shown picture, sets brightness/contrast, or drives a
  screensaver. The only remaining "show picture N" candidate is `0x20`/`0xA0`
  (picture index) — reserved until the payload format is confirmed.
* `0x0E` (display parameters) is silent on hidraw: the Swift app's "report id 13"
  is an **IOKit global report index**, not a HID report ID.
* The knob is exposed by the firmware as a **mouse-style device**: the kernel
  creates `ROYUAN Gaming Keyboard Mouse` (event12) with REL axes
  (X/Y/Z/RY/DIAL/WHEEL) plus buttons, i.e. the knob is a relative-axis HID
  device. Reading those events needs the `input` group (or the raw `hidraw3`
  path). The raw hidraw listener produced **no** reports while idle.

## Next safe experiments

1. Characterise `0x92` (looks like a telemetry/config block: 180, 0, 180, 0, 84, 6 …).
2. `0x91` with an index argument (debounce slots) — read-only in intent, but
   treat as write-until-proven.
3. `0x20` / `0xA0` picture index — the last candidate for "show slot N";
   attempt with a valid slot only, and only after the payload format is known.
4. `0x8D` / `0x82` with arguments.
5. Baseline-diff `0x83` / `0x84` to see what their single byte tracks.
