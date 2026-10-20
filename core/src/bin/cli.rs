//! sharkdeck-cli: hardware test harness for protocol development.
//!
//! Commands:
//!   info                              device identity/firmware/profile/options/display
//!   keymap-dump [base|fn] [profile]   dump all 128 slots as hex + values
//!   knob                              decode knob-candidate slots 88..127
//!   profile <get|0|1|2>               read/write profile
//!   light <mode> [opts]               set lighting (off|static|breathing|neon|wave|…)
//!   screen-pattern <name> [opts]      upload calibration pattern
//!   screen-img <path> [opts]          upload static image or GIF animation
//!   screen-clock [opts]               upload clock animation
//!   display                           display parameter probe
//!   macro-get <slot>                  hexdump a macro slot
//!   scan                              list HID candidate paths

use sharkdeck_core::kb::{Keyboard, LightingState};
use sharkdeck_core::layout;
use sharkdeck_core::protocol::{mods, PixelOrder};
use sharkdeck_core::screen as scr;

const NO_DEVICE: &str = "no K86 found (VID:PID 3151:4015 / handshake 1168)";

/// Run `f` against a freshly identified keyboard; map to an exit code.
fn with_kb(f: impl FnOnce(&Keyboard) -> Result<String, String>) -> i32 {
    match Keyboard::open() {
        Ok(Some(kb)) => match f(&kb) {
            Ok(msg) => {
                if !msg.is_empty() {
                    println!("{msg}");
                }
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        },
        Ok(None) => {
            eprintln!("{NO_DEVICE}");
            1
        }
        Err(e) => {
            eprintln!("hid: {e}");
            1
        }
    }
}

fn parse_size(s: &str) -> (u32, u32) {
    s.split_once('x')
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .unwrap_or((128, 128))
}

fn arg_flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn hex_color(s: &str) -> (u8, u8, u8) {
    let s = s.trim_start_matches('#');
    let b = |i: usize| {
        s.get(i * 2..i * 2 + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .unwrap_or(0)
    };
    (b(0), b(1), b(2))
}

fn pixel_order(args: &[String]) -> PixelOrder {
    if arg_flag(args, "--order").as_deref() == Some("row") {
        PixelOrder::Row
    } else {
        PixelOrder::Column
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cmd = argv.first().map(|s| s.as_str()).unwrap_or("info");
    let rest: Vec<String> = argv[1..].to_vec();
    let code = run(cmd, &rest);
    if code == 2 {
        // exit only for usage errors; commands handle their own printing
    }
    std::process::exit(code);
}

fn run(cmd: &str, args: &[String]) -> i32 {
    match cmd {
        "info" => cmd_info(),
        "keymap-dump" => cmd_keymap(args),
        "knob" => cmd_knob(),
        "profile" => cmd_profile(args.first().map(|s| s.as_str()).unwrap_or("get")),
        "light" => cmd_light(args),
        "screen-pattern" => cmd_pattern(args),
        "screen-img" => cmd_image(args),
        "screen-clock" => cmd_clock(args),
        "display" => cmd_display(),
        "display-probe" => cmd_display_probe(),
        "battery" => cmd_battery(),
        "macro-get" => {
            cmd_macro_get(args.first().and_then(|s| s.parse().ok()).unwrap_or(0))
        }
        "slot-write-test" => with_kb(|kb| {
            // Rewrite a slot's CURRENT factory value (mute) and verify readback
            // — exercises the 0x13 write path with zero behavior change.
            let mute = sharkdeck_core::protocol::SlotEntry([0x03, 0x00, 0xE2, 0x00]);
            kb.write_slot(0, 78, mute, false)?;
            std::thread::sleep(std::time::Duration::from_millis(400));
            let back = kb.read_slot(0, 78, false)?;
            let match_ = back == mute;
            println!("slot 78 after 0x13 write: {:?}", back.0);
            println!("match = {match_}");
            if match_ { Ok("slot write + verify OK".into()) }
            else { Err(format!("readback != written ({:?} != {:?})", back.0, mute.0)) }
        }),
        "scan" => cmd_scan(),
        _ => {
            eprintln!("unknown command: {cmd}");
            2
        }
    }
}

// ---------------------------------------------------------------------------

fn cmd_info() -> i32 {
    with_kb(|kb| {
        let info = kb.info()?;
        let mut out = format!(
            "path={} id={} firmware={} (raw {:04x}) profile={}",
            kb.path, kb.device_id, info.firmware, info.firmware_raw, info.active_profile
        );
        out.push_str(&format!("\noptions: {:?}", info.options));
        match kb.display_params(6) {
            Ok(Some((w, h, b))) => out.push_str(&format!("\ndisplay: {w}x{h} brightness={b}")),
            Ok(None) => out.push_str("\ndisplay: (no echoed reply)"),
            Err(e) => out.push_str(&format!("\ndisplay: {e}")),
        }
        Ok(out)
    })
}

fn cmd_keymap(args: &[String]) -> i32 {
    let layer = args.first().map(|s| s.as_str()).unwrap_or("base");
    let profile: u8 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    with_kb(|kb| {
        let blob = if layer == "fn" {
            kb.fn_layer(profile)
        } else {
            kb.keymap_layer(profile)
        }?;
        let keys = layout::k86_layout();
        let mut out = String::new();
        for i in 0..128usize {
            let e = [blob[i * 4], blob[i * 4 + 1], blob[i * 4 + 2], blob[i * 4 + 3]];
            let name = keys
                .keys
                .iter()
                .find(|k| k.slot as usize == i)
                .map(|k| k.code.clone())
                .unwrap_or_else(|| "-".into());
            out.push_str(&format!(
                "slot {:3} ({:8}) [{:02X} {:02X} {:02X} {:02X}]  {}\n",
                i,
                name,
                e[0],
                e[1],
                e[2],
                e[3],
                decode_label(e)
            ));
        }
        Ok(out)
    })
}

fn decode_label(e: [u8; 4]) -> String {
    match e {
        [0, 0, 0, 0] => "(unassigned)".into(),
        [0, m, u, 0] => {
            let mut parts = Vec::new();
            if m & mods::LCTRL != 0 {
                parts.push("ctrl");
            }
            if m & mods::LSHIFT != 0 {
                parts.push("shift");
            }
            if m & mods::LALT != 0 {
                parts.push("alt");
            }
            if m & mods::LGUI != 0 {
                parts.push("gui");
            }
            let mods_s = if parts.is_empty() {
                String::new()
            } else {
                format!("{}+", parts.join("-"))
            };
            format!("{mods_s}{}", layout::usage_label(u))
        }
        [3, _, code_l, code_h] => {
            let code = code_l as u16 | ((code_h as u16) << 8);
            layout::consumer_label(code)
        }
        [9, mode, idx, _] => format!("macro {idx} mode {mode}"),
        [0x0A, action, _, _] => layout::firmware_label(action),
        _ => format!("raw {:02X?}", e),
    }
}

fn cmd_knob() -> i32 {
    with_kb(|kb| {
        let mut out = "non-empty slots 88..127 (knob-candidate range):\n".to_string();
        for slot in 88u8..128 {
            if let Ok(e) = kb.read_slot(0, slot, false) {
                if !e.is_empty() {
                    out.push_str(&format!(
                        "slot {:3}: {:02X?}  {}\n",
                        slot,
                        e.0,
                        decode_label(e.0)
                    ));
                }
            }
        }
        Ok(out)
    })
}

fn cmd_profile(arg: &str) -> i32 {
    with_kb(|kb| match arg {
        "get" => kb.profile_get().map(|p| format!("profile: {p}")),
        n => match n.parse::<u8>() {
            Ok(idx) => kb.profile_set(idx).map(|_| format!("profile set to {idx}")),
            Err(_) => Err(format!("bad profile index: {n}")),
        },
    })
}

fn cmd_light(args: &[String]) -> i32 {
    with_kb(|kb| {
        let _ = kb.leds_on();
        let name = args.first().map(|s| s.as_str()).unwrap_or("static");
        let color = arg_flag(args, "--rgb").unwrap_or_else(|| "5B8CFF".into());
        let rgb = hex_color(&color);
        let brightness: u8 = arg_flag(args, "--brightness")
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);
        let speed: u8 = arg_flag(args, "--speed")
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        let mut st = LightingState {
            effect: 1,
            speed,
            brightness,
            direction: 0,
            mode: 7,
            r: rgb.0,
            g: rgb.1,
            b: rgb.2,
        };
        st.effect = match name {
            "off" => {
                st.mode = 0;
                0
            }
            "static" => 1,
            "breathing" => 2,
            "neon" | "spectrum" => 3,
            "wave" => 4,
            "ripple" => 5,
            "raindrop" => 6,
            "snake" | "flow" => 7,
            "press" => 8,
            "converge" | "layers" => 9,
            "sine" => 10,
            "kaleidoscope" => 11,
            "linewave" => 12,
            "laser" | "radiant" => 14,
            "circlewave" => 15,
            "dazzle" => 16,
            "raindown" => 17,
            "meteor" => 18,
            "train" => 23,
            "fireworks" => 24,
            "rainbow" => {
                st.mode = 8;
                1
            }
            "preset-red" => mode_preset(&mut st, 0),
            "preset-green" => mode_preset(&mut st, 1),
            "preset-blue" => mode_preset(&mut st, 2),
            "preset-orange" => mode_preset(&mut st, 3),
            "preset-magenta" => mode_preset(&mut st, 4),
            "preset-yellow" => mode_preset(&mut st, 5),
            "preset-white" => mode_preset(&mut st, 6),
            _ => return Err(format!("unknown light mode {name}")),
        };
        let side = args.contains(&"--side".to_string());
        kb.light_set(side, st)?;
        let mut out = "light set".to_string();
        if let Ok(cur) = kb.light(side) {
            out.push_str(&format!(" read-back: {cur:?}"));
        }
        Ok(out)
    })
}

fn mode_preset(st: &mut LightingState, mode: u8) -> u8 {
    st.mode = mode;
    1
}

fn cmd_pattern(args: &[String]) -> i32 {
    let name = args.first().map(|s| s.as_str()).unwrap_or("test");
    let (w, h) = parse_size(&arg_flag(args, "--size").unwrap_or_else(|| "128x128".into()));
    let layer: u8 = arg_flag(args, "--layer")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let order = pixel_order(args);
    with_kb(|kb| {
        let frame = match name {
            "test" => scr::test_pattern(w, h),
            "ruler" => scr::ruler_pattern(w, h),
            "orientation" => scr::orientation_pattern(w, h),
            "band" => scr::band_pattern(w, h, 32),
            "solid-red" => scr::solid((255, 0, 0), w, h),
            "solid-green" => scr::solid((0, 255, 0), w, h),
            "solid-blue" => scr::solid((0, 0, 255), w, h),
            "solid-white" => scr::solid((255, 255, 255), w, h),
            "blank" => scr::solid((0, 0, 0), w, h),
            _ => return Err(format!("unknown pattern {name}")),
        };
        let bytes = sharkdeck_core::protocol::encode565(
            &frame.rgba,
            frame.width as usize,
            frame.height as usize,
            4,
            order,
        );
        kb.screen_write_image(&bytes, layer, frame.width as u16, frame.height as u16)?;
        Ok(format!(
            "uploaded {name} {}x{} to slot {layer}",
            frame.width, frame.height
        ))
    })
}

fn cmd_image(args: &[String]) -> i32 {
    let Some(path) = args.first().cloned() else {
        eprintln!("usage: sharkdeck-cli screen-img <path> [--mode fill|fit|stretch] [--layer N] [--size WxH] [--max-frames N] [--order col|row]");
        return 2;
    };
    let mode = match arg_flag(args, "--mode").as_deref() {
        Some("fit") => scr::ContentMode::Fit,
        Some("stretch") => scr::ContentMode::Stretch,
        _ => scr::ContentMode::Fill,
    };
    let (w, h) = parse_size(&arg_flag(args, "--size").unwrap_or_else(|| "128x128".into()));
    let layer: u8 = arg_flag(args, "--layer")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let max_frames: usize = arg_flag(args, "--max-frames")
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let order = pixel_order(args);

    with_kb(|kb| {
        // Animated path: GIF decode succeeds and yields >1 frame -> ONE
        // handshake carries all frames; never loop write_animation.
        if let Ok((frames, animated)) = scr::decode_gif(&path, mode, w, h, max_frames) {
            let (bytes565, delays) = scr::frames_to_565(&frames, order);
            if animated {
                let refs: Vec<(&[u8], u8)> = bytes565
                    .iter()
                    .map(|b| b.as_slice())
                    .zip(delays.iter().copied())
                    .collect();
                println!("decoded {} animation frames, uploading…", frames.len());
                kb.screen_write_animation(&refs, w as u16, h as u16)?;
                return Ok(format!("animation uploaded ({} frames)", frames.len()));
            }
            let bytes = &bytes565[0];
            kb.screen_write_image(bytes, layer, w as u16, h as u16)?;
            return Ok(format!("single-frame GIF {}x{} to slot {layer}", w, h));
        }
        let frame = scr::decode_static(&path, mode, w, h)?;
        let bytes = sharkdeck_core::protocol::encode565(
            &frame.rgba,
            frame.width as usize,
            frame.height as usize,
            4,
            order,
        );
        kb.screen_write_image(&bytes, layer, frame.width as u16, frame.height as u16)?;
        Ok(format!(
            "uploaded image {}x{} to slot {layer}",
            frame.width, frame.height
        ))
    })
}

fn cmd_clock(args: &[String]) -> i32 {
    let n: usize = arg_flag(args, "--frames")
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
        .clamp(1, 255);
    let seconds = args.contains(&"--seconds".to_string());
    let fg = hex_color(&arg_flag(args, "--rgb").unwrap_or_else(|| "E6E9F2".into()));
    with_kb(|kb| {
        let frames = scr::clock_frames(n, seconds, fg, (10, 12, 18), 128, 128);
        let (bytes565, delays) = scr::frames_to_565(&frames, PixelOrder::Column);
        let refs: Vec<(&[u8], u8)> = bytes565
            .iter()
            .map(|b| b.as_slice())
            .zip(delays.iter().copied())
            .collect();
        kb.screen_write_animation(&refs, 128, 128)?;
        Ok(format!("clock uploaded ({n} frames)"))
    })
}

fn cmd_display() -> i32 {
    with_kb(|kb| {
        let r = kb.display_params(8);
        match r {
            Ok(Some((w, h, b))) => Ok(format!("display: {w}x{h} brightness {b}")),
            Ok(None) => Err("display: no valid reply".into()),
            Err(e) => Err(format!("display: {e}")),
        }
    })
}

/// Empirical battery probe: tries undocumented read-only opcodes and reports
/// the first reply that looks like a percentage.
fn cmd_battery() -> i32 {
    with_kb(|kb| {
        // Raw dumps first so a false positive can be spotted by eye.
        for op in [0x81u8, 0x82, 0x83, 0x84, 0x8C, 0x8D, 0x8E] {
            match kb.debug_query(op) {
                Ok(r) => println!("op {op:#04x} -> {:02x?}", &r[..8.min(r.len())]),
                Err(e) => println!("op {op:#04x} -> err {e}"),
            }
        }
        let r = kb.battery_probe()?;
        Ok(match r {
            Some(p) => format!("battery: {p}%"),
            None => "battery: no readable percentage (device may be USB-powered)".into(),
        })
    })
}

/// Raw matrix probe for the 0x0E display family: paths × send × read × lens.
fn cmd_display_probe() -> i32 {
    use sharkdeck_core::hid::ApiBox;
    let api = match ApiBox::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let paths: Vec<String> = api
        .list()
        .into_iter()
        .filter(|c| c.usage_page == 0xFFFF)
        .map(|c| c.path)
        .collect();
    let mut wire = vec![0u8; 42];
    wire[0] = 13;
    wire[1] = 0x0E;
    wire[2] = 39;
    let sum: u8 = wire[1..41].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    wire[41] = 85u8.wrapping_sub(sum).wrapping_sub(13);
    let mut code = 1;
    for path in &paths {
        for sn in ["write", "sout"] {
            {
                // reopen per send/read combo (device cache)
                let dt = match hidapi::HidApi::new() {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("api: {e}");
                        return 2;
                    }
                };
                let dev = match dt.open_path(
                    std::ffi::CString::new(path.as_str()).unwrap().as_c_str(),
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("open {path}: {e}");
                        continue;
                    }
                };
                let sent: Result<(), String> = match sn {
                    "write" => dev.write(&wire).map(|_| ()).map_err(|e| e.to_string()),
                    _ => dev.send_output_report(&wire).map_err(|e| e.to_string()),
                };
                if sent.is_err() {
                    println!("{path} {sn}: send failed");
                    continue;
                }
                std::thread::sleep(std::time::Duration::from_millis(60));
                for len in [33usize, 42, 65] {
                    for rn in ["gin", "tread"] {
                        let mut buf = vec![0u8; len];
                        buf[0] = 13;
                        let got: Result<usize, String> = match rn {
                            "gin" => {
                                dev.get_input_report(&mut buf).map_err(|e| e.to_string())
                            }
                            _ => dev.read_timeout(&mut buf, 60).map_err(|e| e.to_string()),
                        };
                        match got {
                            Ok(n) if n >= 22 => {
                                println!("{path} {sn} {rn} len={len}: {n}B {:02X?}", &buf[..16]);
                                code = 0;
                            }
                            Ok(n) => println!("{path} {sn} {rn} len={len}: {n}B"),
                            Err(e) => println!("{path} {sn} {rn} len={len}: ERR {e}"),
                        }
                    }
                }
            }
            // fresh api so the next send lands on a clean open
            let _ = &api;
        }
    }
    code
}

fn cmd_macro_get(slot: u8) -> i32 {
    with_kb(|kb| {
        let blob = kb.macro_read(slot)?;
        let mut out = format!("macro {slot}: {} bytes", blob.len());
        for (i, chunk) in blob.chunks(32).enumerate() {
            out.push_str(&format!("\n  {:3}: {:02X?}", i * 32, chunk));
        }
        Ok(out)
    })
}

/// List every HID path (for plumbing debug).
fn cmd_scan() -> i32 {
    match sharkdeck_core::hid::find_candidates() {
        Ok(list) => {
            println!("{} candidate paths:", list.len());
            for c in &list {
                println!(
                    "  {} intr={} upage={:#06x} usage={:#06x} serial={} product={}",
                    c.path, c.interface, c.usage_page, c.usage, c.serial, c.product
                );
            }
            0
        }
        Err(e) => {
            eprintln!("scan: {e}");
            1
        }
    }
}
