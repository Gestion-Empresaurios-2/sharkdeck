# SharkDeck

Control surface for the **Attack Shark K86 (3151:4015)** — a native desktop app that unlocks the little TFT screen and the knob most vendor software leaves untouchable.

- **Screen** — push images, GIFs, live clock, built-in patterns to any of the 5 on-device image slots (RGB565, tested at 128×128)
- **Knob** — rebind turn-left / turn-right / press to keys, media controls, firmware actions or macros. On the shipped firmware (1.19) the knob lives at matrix slots **96 = turn-left, 97 = turn-right, 78 = press** (factory: volume down / volume up / mute) — the app reads the real slots before showing any binding, so it stays correct even if the firmware moves things. — hardware-verified on firmware 1.19: turn-left = slot 96, turn-right = slot 97, press = slot 78
- **Lighting** — 20 effects, brightness/speed/direction, main + side strips, presets, rainbow, and a "nothing lights up" fixer
- **Keymap** — visual 75% layout editor over Base/Fn layers and 3 profiles with read-verify-rollback writes + JSON backup/restore
- **Macros** — 50 macro slots with key/button/pointer-move events and repeat modes
- **Device** — identity, firmware, panel geometry probe, option flags, logs

Everything the app writes to flash is **read back and verified**, with automatic rollback if the device disagrees.

## Build

Requirements: Rust 1.75+, Node 18+, and on Linux the usual Tauri stack: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `build-essential`, `pkg-config`, plus `libhidapi-dev` (the `hidapi` crate bundles a copy, but keep headers handy for distro weirdness).

```bash
npm install
npm run build          # vite → dist/
cargo tauri build      #   or: cargo build --release -p sharkdeck
```

For development: `npm run tauri dev` (or start `npm run dev` and run the cargo app with `devUrl` auto-detected).

## Linux HID access

The K86 only exposes its control interface over the **USB cable with the rear switches set to WIN + USB-C** (Bluetooth lists nothing usable). Grant user access:

```bash
sudo tee /etc/udev/rules.d/91-sharkdeck.rules >/dev/null <<'EOF'
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="3151", ATTRS{idProduct}=="4015", MODE="0660", TAG+="uaccess"
EOF
sudo udevadm control --reload && sudo udevadm trigger
```

Replug the keyboard. If a slot write was interrupted, the firmware may hold stale caches — replug once and retry.

## CLI

The low-level tool is built alongside (`target/release/sharkdeck-cli`) and doubles as the end-to-end protocol test rig:

```text
sharkdeck-cli info                      # identity, firmware, profile, options, panel size
sharkdeck-cli scan                      # list candidate hidraw paths
sharkdeck-cli keymap-dump [base|fn] [profile]
sharkdeck-cli knob                      # non-empty knob-adjacent slots, decoded
sharkdeck-cli profile [get|0|1|2]
sharkdeck-cli light static [--rgb 8C5BFF --brightness 3 --side]
sharkdeck-cli screen-pattern test|orientation|ruler|band [--size 128x128 --layer 0]
sharkdeck-cli screen-img cat.gif [--mode fill|fit|stretch --layer 0 --order col|row]
sharkdeck-cli screen-clock [--frames 12 --seconds]
sharkdeck-cli display                   # display-parameter probe (width/height)
sharkdeck-cli macro-get 4
```

`--order col|row` matters beyond the verified 128×128 square: column-major big-endian was verified on hardware, row-major little-endian is the vendor convention for non-square panels.

## Hardware notes

Verified end-to-end against the K86 with firmware **1.19** (`yc500` opcode family):

- Screen writes succeeded on the verified 128×128 column-major geometry; other panel sizes can be interrogated with the built-in calibration patterns (`orientation`/`ruler`/`band`) plus Fill/Fit/Stretch scaling.
- The display-parameters probe (`0x0E`) of the vendor macOS stack answers only via a numbered-report iokit path; the same command is silent on Linux `hidraw` for this firmware, so panel geometry falls back to the stored config + calibration patterns rather than live reads.
- Which image slot the screen shows is picked from the keyboard's own menu (the knob opens it) — there is no host command for slot selection.
- Keymap/lighting/knob writes follow a read → write → read-back-verify → rollback rule; single-slot writes need the firmware's close-and-reopen dance before fresh data is served, which the app does for you.

## Architecture

- `src-tauri/` — Tauri 2 shell: `hid.rs` (hidraw scanning/open/flush per operation), `kb.rs` (command set, query-with-retry to defeat the firmware's cached-response bug, identity gating on handshake id 1168), `screen.rs` (GIF/image decode, content scaling, RGB565 packing, patterns, clock), `layout.rs` (K86 key mesh), `commands.rs` (27 IPC commands)
- `src/` — Svelte 5 + Vite UI: Screen / Knob / Lighting / Keymap / Macros / Settings
- Writes travel `read → modify → write → re-read verify → rollback on mismatch`. Knob and keymap slot writes additionally require the reopen-after-write dance the firmware demands before it serves fresh data.

## Credits & safety

Protocol reverse-engineering drawn from three MIT-licensed projects — thank you to:

- **[shark-k86-mac](https://github.com/RaphaelCaputo2/shark-k86-mac)** (Python + hidapi) — hardware-verified RGB & screen flow
- **[keyboard-studio](https://github.com/mehmetnadir/keyboard-studio)** (Swift) — device profile, knob slots, screen upload state machine
- **[sharkfin](https://dnim.dev/blog/royuan-keyboard-protocol)** (Rust) — registry, factory keymap evidence, protocol write-up

By design, SharkDeck never wires up the destructive part of the protocol: flash erase (`0xAC`), display-asset erase (`0x2C`), bootloader entry (`0x7F`/`0x30`), factory reset (`0x02`), and unknown-family keymap writes stay out of every code path (GUI and CLI).

MIT licensed.
