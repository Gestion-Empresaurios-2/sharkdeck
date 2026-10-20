# SharkDeck

Control surface for the **Attack Shark K86 (3151:4015)** — a native desktop app that unlocks the little TFT screen and the knob most vendor software leaves untouchable.

- **Screen** — push images, GIFs, live clock, built-in patterns to any of the 5 on-device image slots (RGB565, tested at 128×128)
- **Templates** — **20 designed screens** (dashboard, retro terminal, matrix rain, waves, equalizer, SharkDeck, four watch faces, plus nine lock-screen-style layouts) built from composable widgets: clock (5 sizes + seconds), date, ticker, wave, rain, frame, equalizer, scanlines, brand fin, meter, analog watch dial, date window, battery (classic segments or iOS style), ring, steps, and **live desk data** (host battery %, CPU load, RAM used, uptime, keyboard firmware/profile). **HyperOS-style layout system**: layout-driven templates ship several layouts (Classic / Split / Corner, Big / Compact, Art / Minimal, Cell / Row…), each with its own clock size + position and its own default widget set — pick a layout, then move any widget onto one of eight anchors (TL, TR, C, BL, BR, BAR, L, R). Every template ships a real canvas preview, and the panel shows a **live mirror of exactly what the keyboard is displaying**.
- **Knob** — rebind turn-left / turn-right / press to keys, media controls, firmware actions or macros. On the shipped firmware (1.19) the knob lives at matrix slots **96 = turn-left, 97 = turn-right, 78 = press** (factory: volume down / volume up / mute) — the app reads the real slots before showing any binding, so it stays correct even if the firmware moves things. — hardware-verified on firmware 1.19: turn-left = slot 96, turn-right = slot 97, press = slot 78
- **Lighting** — 20 effects, brightness/speed/direction, main + side strips, presets, rainbow, and a "nothing lights up" fixer
- **Keymap** — visual 75% layout editor over Base/Fn layers and 3 profiles with read-verify-rollback writes + JSON backup/restore
- **Macros** — 50 macro slots with key/button/pointer-move events and repeat modes
- **Device** — identity, firmware, panel geometry probe, option flags, logs, plus a "Desk & keyboard" panel with host battery/CPU/RAM/uptime and the keyboard's own state

Everything the app writes to flash is **read back and verified**, with automatic rollback if the device disagrees.

## Notes & honest limits

- **The keyboard has no screen read-back.** The protocol exposes no opcode to read the panel, so the app's "on the keyboard now" panel mirrors exactly the pixel stream SharkDeck sends (and says so). Panel geometry is therefore verified visually with the ruler/orientation/band patterns in the panel-size campaign.
- **The K86 has no battery read command.** The firmware only offers a *key action* (0x08 "checkBattery"), not a read; probing every plausible read opcode returns unrelated/stale frames, so the app reports the keyboard battery as "not reported" instead of inventing a number. Host battery comes from `/sys/class/power_supply`.
- **Animation delays are centiseconds** on the wire (1 s = 100, max 2.55 s per frame); GIF delays are converted from the file's 10 ms units.
- **Which image slot is on screen is chosen on the keyboard itself** (its own menu opens with the knob) — the app writes into slots 1–5 but cannot switch the display.
- **Theme**: GNOME/Yaru-flat light and Adwaita-dark pair, no gradients, keyboard focus rings, `prefers-reduced-motion` respected.

## Cartridges — the keyboard's own animation engine

The K86's panel is a dumb RGB565 framebuffer, but its firmware **plays a stored animation on its own timer**, honouring a per-frame delay (centiseconds, max 2.55 s). That player is the only execution engine the hardware has, so SharkDeck uses it as one:

- A **cartridge** is a scene compiled into *frames + per-frame delays*, uploaded once over the existing `0xA5` handshake / `0x25` chunk stream.
- Once the cartridge is on the keyboard it **loops on its own** — the screen keeps animating and the clock keeps ticking with the laptop closed and the keyboard on battery. No host, no pushes, no daemon.
- The frames are stamped with the wall-clock second they will be shown at (the device only starts playing once every frame has landed), so the on-keyboard clock is phase-correct instead of being late by the transfer time.
- **Verified on hardware (fw 1.19, 240×135):** 30 / 60 / 90 / 120 / 200-frame cartridges all upload and play; 255 frames is the protocol ceiling. A 1 s dwell gives a 1-minute (60-frame) or 3m20s (200-frame) window of *fully correct* clock, at ~0.55 s per frame of upload time.
- **Honest limit:** the hour and minute are baked pixels. A 60-frame cartridge keeps the *seconds* exact forever (the device loops them) but freezes the minute/hour at the values of the last push; a longer cartridge keeps the whole clock exact for longer. With the app open, "re-anchor while connected" re-pushes the cartridge before the window ends, so the clock is always exact.
- The firmware has **no scripting, vector or text primitives** (and no native clock screensaver) — every pixel comes from the host, so the host stays the "engine" and the keyboard stays the "player".

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

### Native knob events (optional)

The firmware exposes the knob as a **mouse-style HID device** — the kernel
creates `ROYUAN Gaming Keyboard Mouse` (`/dev/input/event12`) with relative
axes (X/Y/Z/RY/DIAL/WHEEL) plus buttons. Reading those events (so the app can
show live knob turns, or bind them to actions) needs membership of the
`input` group:

```bash
sudo gpasswd -a "$USER" input   # then log out and back in (or: newgrp input)
```

Without it, the *Device → Native knob* panel reports a permission error; every
other feature works fine. The raw `hidraw` path also works but only while the
device is actively sending reports.


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

## Auto-update

SharkDeck ships with a signed in-app updater (`tauri-plugin-updater`): **Device → Updates → Check for updates**, then *Install & restart*. Artifacts are signed, and the app refuses anything whose signature does not match the embedded public key.

One-time setup (run it in your terminal — the private key is deliberately never handled by an agent and never committed):

```bash
sh scripts/setup-updater.sh    # POSIX-sh safe (works with sh/dash/bash)
```

It generates the keypair, writes the public key into `src-tauri/tauri.conf.json`, and tells you to add `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as repository secrets. After that, tagging releases is enough:

```bash
git tag v0.2.0 && git push origin v0.2.0
```

The workflow (`.github/workflows/release.yml`) builds the AppImage + deb, signs them, publishes `latest.json` and the artifacts on the GitHub release, and the running app picks the new version up from its update feed. The feed URL must be reachable without authentication — make the repository public, or point `plugins.updater.endpoints` at your own host.

## Credits & safety

Protocol reverse-engineering drawn from three MIT-licensed projects — thank you to:

- **[shark-k86-mac](https://github.com/RaphaelCaputo2/shark-k86-mac)** (Python + hidapi) — hardware-verified RGB & screen flow
- **[keyboard-studio](https://github.com/mehmetnadir/keyboard-studio)** (Swift) — device profile, knob slots, screen upload state machine
- **[sharkfin](https://dnim.dev/blog/royuan-keyboard-protocol)** (Rust) — registry, factory keymap evidence, protocol write-up

By design, SharkDeck never wires up the destructive part of the protocol: flash erase (`0xAC`), display-asset erase (`0x2C`), bootloader entry (`0x7F`/`0x30`), factory reset (`0x02`), and unknown-family keymap writes stay out of every code path (GUI and CLI).

MIT licensed.
