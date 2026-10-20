//! Tauri command surface: thin serde wrappers over sharkdeck-core.

use sharkdeck_core::kb::{Keyboard, KbOptions, LightingState};
use sharkdeck_core::layout;
use sharkdeck_core::perkey::{self, KeyColour, PerKeyImage};
use sharkdeck_core::protocol::{PixelOrder, SlotEntry};
use sharkdeck_core::screen as scr;
use std::fs;
use std::sync::{Arc, Mutex};

/// Read the keyboard's battery level, if this firmware exposes one.
/// Returns None when no documented/empirical opcode answers.
#[tauri::command]
pub fn battery_probe(app: State<AppState>) -> Result<Option<u8>, String> {
    let kb = kb_of(&app)?;
    kb.battery_probe()
}

/// Live desk + device telemetry for on-panel widgets (battery, CPU, RAM,
/// uptime, host name) plus the keyboard's current state.
#[tauri::command]
pub fn host_info(app: State<AppState>) -> Result<serde_json::Value, String> {
    // Host battery
    let mut battery: Option<u8> = None;
    let mut charging = false;
    if let Ok(entries) = fs::read_dir("/sys/class/power_supply") {
        for e in entries.flatten() {
            let p = e.path();
            let typ = fs::read_to_string(p.join("type")).unwrap_or_default();
            if !typ.contains("Battery") {
                continue;
            }
            if let Ok(c) = fs::read_to_string(p.join("capacity")) {
                if let Ok(v) = c.trim().parse::<u8>() {
                    battery = Some(v);
                }
            }
            let st = fs::read_to_string(p.join("status")).unwrap_or_default();
            charging = st.trim() == "Charging";
            break;
        }
    }

    // CPU load (1 min average over available cores)
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1);
    let load1 = fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()))
        .unwrap_or(0.0);
    let cpu = ((load1 / cores as f64) * 100.0).round().clamp(0.0, 100.0) as u8;

    // RAM used
    let (total_kb, avail_kb) = (|| {
        let m = fs::read_to_string("/proc/meminfo").ok()?;
        let g = |k: &str| -> Option<u64> {
            m.lines()
                .find(|l| l.starts_with(k))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        };
        Some((g("MemTotal:")?, g("MemAvailable:")?))
    })()
    .unwrap_or((0, 0));
    let ram = if total_kb > 0 {
        (((total_kb - avail_kb.min(total_kb)) as f64 / total_kb as f64) * 100.0).round() as u8
    } else {
        0
    };

    let uptime = fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()))
        .unwrap_or(0.0) as u64;
    let hostname = fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();

    // Keyboard state (tolerates "no device")
    let (firmware, profile, effect) = match kb_of(&app) {
        Ok(kb) => (
            kb.info().map(|i| i.firmware).unwrap_or_default(),
            kb.profile_get().unwrap_or(0),
            kb.light(false).map(|l| l.effect).unwrap_or(255),
        ),
        Err(_) => (String::new(), 0, 255),
    };

    Ok(serde_json::json!({
        "battery": battery,
        "charging": charging,
        "cpu": cpu,
        "ram": ram,
        "uptime": uptime,
        "hostname": hostname,
        "firmware": firmware,
        "profile": profile,
        "effect": effect,
    }))
}
use tauri::Manager;
use tauri::State;

#[derive(Default)]
pub struct AppState {
    pub kb: Arc<Mutex<Option<Arc<Keyboard>>>>,
    pub log: Arc<Mutex<Vec<String>>>,
    pub config: Arc<Mutex<AppConfig>>,
    pub ready: std::sync::atomic::AtomicBool,
    /// Duration of the last screen push (ms) — used to time-stamp clock
    /// animations so the device starts them at the right second.
    pub last_push_ms: std::sync::atomic::AtomicU64,
    /// How many frames the last push carried — used to extrapolate the
    /// transfer time of a cartridge before it is built.
    pub last_push_frames: std::sync::atomic::AtomicU64,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub screen_width: u32,
    pub screen_height: u32,
    pub order: PixelOrder,
    pub max_frames: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            // Registry panel size for the K86 (240x135); verified 128x128 square
            // remains selectable via the panel-size campaign in Templates.
            screen_width: 240,
            screen_height: 135,
            order: PixelOrder::Column,
            max_frames: 30,
        }
    }
}

pub fn log(app: &AppState, line: String) {
    let mut l = app.log.lock().unwrap();
    l.push(line);
    while l.len() > 200 {
        l.remove(0);
    }
}

fn kb_of(app: &AppState) -> Result<Arc<Keyboard>, String> {
    app.kb
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "no device connected".into())
}

pub fn config_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("sharkdeck.json"))
}

pub fn load_config(path: Option<std::path::PathBuf>) -> AppConfig {
    let cfg: AppConfig = path
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // Migrate stale 128x128 defaults to the registry panel (240x135) so old
    // configs don't pin the app to the square that ignores the panel edge.
    if cfg.screen_width == 128 && cfg.screen_height == 128 {
        AppConfig {
            screen_width: 240,
            screen_height: 135,
            ..cfg
        }
    } else {
        cfg
    }
}

#[tauri::command]
pub fn probe(app: tauri::State<'_, AppState>) -> Result<Option<serde_json::Value>, String> {
    // Cold-start races (webview booting while hidraw settles) can make the
    // first enumeration come up empty; retry briefly before giving up.
    let mut fresh = Keyboard::open()?;
    eprintln!("[sharkdeck] probe: first open -> {}", if fresh.is_some() { "found" } else { "none" });
    if fresh.is_none() {
        for i in 0..6 {
            std::thread::sleep(std::time::Duration::from_millis(800));
            fresh = Keyboard::open()?;
            eprintln!("[sharkdeck] probe: retry {} -> {}", i + 1, if fresh.is_some() { "found" } else { "none" });
            if fresh.is_some() {
                break;
            }
        }
    }
    let mut guard = app.kb.lock().unwrap();
    match fresh {
        Some(kb) => {
            let info = kb.info();
            let out = match info {
                Ok(i) => Some(serde_json::json!({
                    "path": i.path,
                    "deviceId": format!("{:x}", i.device_id),
                    "handshakeId": i.device_id,
                    "firmware": i.firmware,
                    "profile": i.active_profile,
                    "options": i.options,
                })),
                Err(e) => {
                    log(&app, format!("info failed: {e}"));
                    Some(serde_json::json!({"path": kb.path, "handshakeId": kb.device_id, "error": e}))
                }
            };
            let _ = guard.get_or_insert_with(|| Arc::new(kb));
            let _ = app.ready.swap(true, std::sync::atomic::Ordering::SeqCst);
            Ok(out)
        }
        None => {
            *guard = None;
            Ok(None)
        }
    }
}

#[tauri::command]
pub fn disconnect(app: tauri::State<'_, AppState>) -> Result<(), String> {
    *app.kb.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub fn light_get(app: tauri::State<'_, AppState>, side: bool) -> Result<LightingState, String> {
    kb_of(&app)?.light(side)
}

#[tauri::command]
pub fn light_set(app: tauri::State<'_, AppState>, side: bool, state: LightingState) -> Result<(), String> {
    let kb = kb_of(&app)?;
    kb.light_set(side, state)
}

#[tauri::command]
pub fn leds_on(app: tauri::State<'_, AppState>) -> Result<KbOptions, String> {
    kb_of(&app)?.leds_on()
}

#[tauri::command]
pub fn options_get(app: tauri::State<'_, AppState>) -> Result<KbOptions, String> {
    kb_of(&app)?.options_get()
}

#[tauri::command]
pub fn options_set(app: tauri::State<'_, AppState>, options: KbOptions) -> Result<(), String> {
    kb_of(&app)?.options_set(options)
}

#[tauri::command]
pub fn profile_get(app: tauri::State<'_, AppState>) -> Result<u8, String> {
    kb_of(&app)?.profile_get()
}

#[tauri::command]
pub fn profile_set(app: tauri::State<'_, AppState>, index: u8) -> Result<(), String> {
    kb_of(&app)?.profile_set(index)
}

#[tauri::command]
pub fn keymap_read(
    app: tauri::State<'_, AppState>,
    profile: u8,
    fn_layer: bool,
) -> Result<Vec<u8>, String> {
    let kb = kb_of(&app)?;
    let blob = if fn_layer {
        kb.fn_layer(profile)
    } else {
        kb.keymap_layer(profile)
    }?;
    Ok(blob.to_vec())
}

#[tauri::command]
pub fn keymap_write(
    app: tauri::State<'_, AppState>,
    profile: u8,
    fn_layer: bool,
    slot: u8,
    entry: [u8; 4],
) -> Result<serde_json::Value, String> {
    let kb = kb_of(&app)?;
    let before = kb.read_slot(profile, slot, fn_layer)?;
    kb.write_slot(profile, slot, SlotEntry(entry), fn_layer)?;
    let after = kb.read_slot(profile, slot, fn_layer)?;
    let ok = after.0 == entry;
    if !ok {
        // Roll back: firmware may have silently dropped the write.
        let orig = before.0;
        kb.write_slot(profile, slot, before, fn_layer)?;
        let after2 = kb.read_slot(profile, slot, fn_layer)?;
        if after2.0 != orig {
            log(&app, format!("slot {slot}: restore failed; device changed"));
        }
        return Ok(serde_json::json!({
            "ok": false,
            "restored": after2.0 == orig,
            "before": orig,
            "got": after.0
        }));
    }
    Ok(serde_json::json!({"ok": true, "value": entry}))
}

#[tauri::command]
pub fn macro_read(app: tauri::State<'_, AppState>, slot: u8) -> Result<Vec<u8>, String> {
    Ok(kb_of(&app)?.macro_read(slot)?.to_vec())
}

#[tauri::command]
pub fn macro_write(app: tauri::State<'_, AppState>, slot: u8, blob: Vec<u8>) -> Result<(), String> {
    let mut b = [0u8; 256];
    if blob.len() > 256 {
        return Err("macro too long".into());
    }
    b[..blob.len()].copy_from_slice(&blob);
    kb_of(&app)?.macro_write(slot, &b)
}

#[tauri::command]
pub fn display_params(app: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    match kb_of(&app)?.display_params(8) {
        Ok(Some((w, h, b))) => Ok(serde_json::json!({"width": w, "height": h, "brightness": b})),
        Ok(None) => Ok(serde_json::json!({"width": null, "height": null})),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub fn config_get(app: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(*app.config.lock().unwrap())
}

#[tauri::command]
pub fn config_set(
    app: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    config: AppConfig,
) -> Result<(), String> {
    *app.config.lock().unwrap() = config;
    if let Some(p) = config_path(&app_handle) {
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&p, serde_json::to_string(&config).unwrap_or_default());
    }
    Ok(())
}

#[tauri::command]
pub fn logs_get(app: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(app.log.lock().unwrap().clone())
}

#[tauri::command]
pub fn layout_get() -> Result<serde_json::Value, String> {
    serde_json::to_value(layout::k86_layout()).map_err(|e| e.to_string())
}

/// Per-key RGB grid: the K86 layout projected onto a colour grid (rows are
/// the distinct key `y`, columns the distinct key `x`) plus the canvas
/// rectangles the painter draws with. Also reports the flash-upload limiter
/// state so the UI can show how many uploads are left.
#[tauri::command]
pub fn perkey_grid() -> Result<serde_json::Value, String> {
    let layout = layout::k86_layout();
    let map = perkey::grid_map(layout);
    let mut cells: Vec<serde_json::Value> = Vec::with_capacity(layout.keys.len());
    for (i, key) in layout.keys.iter().enumerate() {
        let Some((col, row)) = map.cell_of_key(i) else {
            continue;
        };
        cells.push(serde_json::json!({
            "slot": key.slot,
            "code": key.code,
            "text": key.text.clone().unwrap_or_default(),
            "col": col,
            "row": row,
            "cell": map.cell_index(col, row),
            "x": key.x,
            "y": key.y,
            "w": key.w,
            "h": key.h,
        }));
    }
    let status = perkey::userpic_status();
    Ok(serde_json::json!({
        "width": map.width,
        "height": map.height,
        "canvas": { "width": layout.canvas.width, "height": layout.canvas.height },
        "keyCount": map.occupied(),
        "cells": cells,
        "unusedSlots": layout.unused.iter().map(|u| u.matrix_index).collect::<Vec<u8>>(),
        "uploads": {
            "allowed": status.allowed,
            "remaining": status.remaining,
            "max": status.max,
            "windowS": status.window_s,
            "nextAllowedInS": status.next_allowed_in_s,
        }
    }))
}

/// Push a per-key colour image to the keyboard.
///
/// `cells` carries one `[r, g, b]` triple per grid cell (row-major, exactly
/// `width * height` entries). With `dry_run` the payload is only encoded —
/// nothing is written to the device. A real upload is a FLASH write limited
/// to 2 per 10 s by the limiter inside `Keyboard::set_userpic`.
#[tauri::command]
pub fn perkey_push(
    app: tauri::State<'_, AppState>,
    cells: Vec<[u8; 3]>,
    dry_run: bool,
) -> Result<serde_json::Value, String> {
    let layout = layout::k86_layout();
    let map = perkey::grid_map(layout);
    let mut image = PerKeyImage::new(map.width, map.height);
    if cells.len() != image.pixels.len() {
        return Err(format!(
            "expected {} cells, got {}",
            image.pixels.len(),
            cells.len()
        ));
    }
    for (i, c) in cells.iter().enumerate() {
        image.pixels[i] = KeyColour::new(c[0], c[1], c[2]);
    }
    let blob = perkey::encode_with(&image, layout);
    let chunks = (blob.len() + perkey::USERPIC_CHUNK - 1) / perkey::USERPIC_CHUNK;
    if dry_run {
        let status = perkey::userpic_status();
        return Ok(serde_json::json!({
            "sent": false,
            "dry_run": true,
            "bytes": blob.len(),
            "chunks": chunks,
            "keys": map.occupied(),
            "remaining": status.remaining,
            "next_allowed_in_s": status.next_allowed_in_s,
        }));
    }
    let kb = kb_of(&app)?;
    kb.set_userpic(&blob)?;
    let status = perkey::userpic_status();
    log(
        &app,
        format!("perkey upload: {} bytes in {} chunks", blob.len(), chunks),
    );
    Ok(serde_json::json!({
        "sent": true,
        "dry_run": false,
        "bytes": blob.len(),
        "chunks": chunks,
        "keys": map.occupied(),
        "remaining": status.remaining,
        "next_allowed_in_s": status.next_allowed_in_s,
    }))
}

/// Upload a frame prepared in the frontend canvas. `rgba_b64` holds 4 bytes
/// per pixel in row-major order.
#[tauri::command]
pub fn screen_push(
    app: tauri::State<'_, AppState>,
    rgba_b64: String,
    width: u32,
    height: u32,
    mode: scr::ContentMode,
    order: Option<PixelOrder>,
    delay: Option<u8>,
    slot: Option<u8>,
) -> Result<(), String> {
    use base64::Engine;
    let cfg = *app.config.lock().unwrap();
    let kb = kb_of(&app)?;
    let rgba = base64::engine::general_purpose::STANDARD
        .decode(rgba_b64)
        .map_err(|e| e.to_string())?;
    let frame = scr::RgbaFrame {
        width,
        height,
        rgba,
    };
    let scaled = scr::scale(&frame, mode, cfg.screen_width, cfg.screen_height);
    let bytes = sharkdeck_core::protocol::encode565(
        &scaled.rgba,
        scaled.width as usize,
        scaled.height as usize,
        4,
        order.unwrap_or(cfg.order),
    );
    kb.screen_write_image(
        &bytes,
        slot.unwrap_or(0),
        scaled.width as u16,
        scaled.height as u16,
    )
    .map_err(|e| {
        log(&app, format!("screen push failed: {e}"));
        e
    })?;
    let _ = delay;
    Ok(())
}

/// File import: returns frames (base64 rgb565, delays, preview base64 rgba).
#[tauri::command]
pub fn screen_import(
    app: tauri::State<'_, AppState>,
    path: String,
    mode: scr::ContentMode,
    order: Option<PixelOrder>,
    slot: Option<u8>,
    slot_count: Option<usize>,
) -> Result<serde_json::Value, String> {
    use base64::Engine;
    let cfg = *app.config.lock().unwrap();
    let kb = kb_of(&app)?;
    let (w, h) = (cfg.screen_width, cfg.screen_height);
    let order = order.unwrap_or(cfg.order);
    let lower = path.to_lowercase();
    let (frames, animated) = if lower.ends_with(".gif") {
        scr::decode_gif(&path, mode, w, h, cfg.max_frames)?
    } else {
        (vec![(scr::decode_static(&path, mode, w, h)?, 10u8)], false)
    };
    let slot_count = slot_count.unwrap_or(1).max(1);
    let _ = slot_count;
    if animated {
        let bytes_pack = scr::frames_to_565(&frames, order);
        let refs: Vec<(&[u8], u8)> = bytes_pack
            .0
            .iter()
            .zip(bytes_pack.1.iter())
            .map(|(b, d)| (b.as_slice(), *d))
            .collect();
        kb.screen_write_animation(&refs, w as u16, h as u16)?;
    } else {
        for (i, (f, _)) in frames.iter().enumerate().take(slot_count) {
            let (bytes_pack, _) = scr::frames_to_565(&[(f.clone(), 10)], order);
            kb.screen_write_image(&bytes_pack[0], slot.unwrap_or(0) + i as u8, w as u16, h as u16)?;
        }
    }
    // previews for the UI (scaled rgba, base64)
    let previews: Vec<String> = frames[0..1]
        .iter()
        .map(|(f, _)| {
            let mut p = f.rgba.clone();
            for b in p.chunks_exact_mut(4) {
                b[3] = 255;
            }
            base64::engine::general_purpose::STANDARD.encode(&p)
        })
        .collect();
    Ok(serde_json::json!({
        "animated": animated,
        "count": frames.len(),
        "previews": previews,
        "delays": frames.iter().map(|(_, d)| d).collect::<Vec<_>>(),
        "width": w,
        "height": h,
        "slot": slot.unwrap_or(0)
    }))
}

/// Upload a builtin pattern directly (wizard can override panel size).
#[tauri::command]
pub fn screen_pattern(
    app: tauri::State<'_, AppState>,
    pattern: String,
    slot: Option<u8>,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<serde_json::Value, String> {
    use base64::Engine;
    let cfg = *app.config.lock().unwrap();
    let kb = kb_of(&app)?;
    let (w, h) = (width.unwrap_or(cfg.screen_width), height.unwrap_or(cfg.screen_height));
    let frame = match pattern.as_str() {
        "test" => scr::test_pattern(w, h),
        "ruler" => scr::ruler_pattern(w, h),
        "orientation" => scr::orientation_pattern(w, h),
        "band" => scr::band_pattern(w, h, 32),
        "solid-red" => scr::solid((255, 0, 0), w, h),
        "solid-green" => scr::solid((0, 255, 0), w, h),
        "solid-blue" => scr::solid((0, 0, 255), w, h),
        "solid-white" => scr::solid((255, 255, 255), w, h),
        "blank" => scr::solid((0, 0, 0), w, h),
        _ => return Err(format!("unknown pattern {pattern}")),
    };
    let bytes = sharkdeck_core::protocol::encode565(
        &frame.rgba, frame.width as usize, frame.height as usize, 4, cfg.order,
    );
    kb.screen_write_image(&bytes, slot.unwrap_or(0), frame.width as u16, frame.height as u16)?;
    Ok(serde_json::json!({
        "preview": base64::engine::general_purpose::STANDARD.encode(&frame.rgba),
        "width": frame.width,
        "height": frame.height
    }))
}

/// Upload a builtin clock animation (n frames each 1s).
#[tauri::command]
pub fn screen_clock(
    app: tauri::State<'_, AppState>,
    frames: Option<usize>,
    with_seconds: bool,
    color: Option<[u8; 3]>,
    background: Option<[u8; 3]>,
) -> Result<(), String> {
    let cfg = *app.config.lock().unwrap();
    let kb = kb_of(&app)?;
    let n = frames.unwrap_or(cfg.max_frames).max(1).min(255);
    // The device only starts playing the animation once the upload finished,
    // so stamp the frames from the moment the device will actually begin —
    // estimated from the duration of the previous push (self-correcting).
    let offset_secs = (app.last_push_ms.load(std::sync::atomic::Ordering::Relaxed) as i64 / 1000) + 1;
    let t0 = std::time::Instant::now();
    let packed = scr::frames_to_565(
        &scr::clock_frames_from(
            n,
            with_seconds,
            color.unwrap_or([230, 233, 242]).into(),
            background.unwrap_or([10, 12, 18]).into(),
            cfg.screen_width,
            cfg.screen_height,
            offset_secs,
        ),
        cfg.order,
    );
    let refs: Vec<(&[u8], u8)> = packed
        .0
        .iter()
        .zip(packed.1.iter())
        .map(|(b, d)| (b.as_slice(), *d))
        .collect();
    kb.screen_write_animation(&refs, cfg.screen_width as u16, cfg.screen_height as u16)?;
    app.last_push_ms
        .store(t0.elapsed().as_millis() as u64, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// Upload a composed template animation: each frame arrives as base64 RGBA
/// plus a delay byte (10ms units), 2..90 frames.
#[derive(serde::Deserialize)]
pub struct AnimFrame {
    pub rgba_b64: String,
    pub delay: u8,
}

#[tauri::command]
pub fn screen_anim_push(
    app: tauri::State<'_, AppState>,
    frames: Vec<AnimFrame>,
    width: u32,
    height: u32,
    order: Option<PixelOrder>,
) -> Result<(), String> {
    use base64::Engine;
    if frames.is_empty() {
        return Err("no frames".into());
    }
    if frames.len() > 255 {
        return Err("max 255 frames (protocol limit)".into());
    }
    let cfg = *app.config.lock().unwrap();
    let kb = kb_of(&app)?;
    let order = order.unwrap_or(cfg.order);
    let mut rgba_frames = Vec::with_capacity(frames.len());
    for f in &frames {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&f.rgba_b64)
            .map_err(|e| format!("bad frame b64: {e}"))?;
        if raw.len() != (width * height * 4) as usize {
            return Err(format!(
                "frame is {} bytes, expected {} ({w}x{h}x4)",
                raw.len(),
                width * height * 4,
                w = width,
                h = height
            ));
        }
        rgba_frames.push((raw, f.delay));
    }
    let packed: Vec<(Vec<u8>, u8)> = rgba_frames
        .iter()
        .map(|(raw, d)| {
            (
                sharkdeck_core::protocol::encode565(&raw, width as usize, height as usize, 4, order),
                *d,
            )
        })
        .collect();
    let refs: Vec<(&[u8], u8)> = packed
        .iter()
        .map(|(b, d)| (b.as_slice(), *d))
        .collect();
    let t0 = std::time::Instant::now();
    kb.screen_write_animation(&refs, width as u16, height as u16)?;
    app.last_push_ms
        .store(t0.elapsed().as_millis() as u64, std::sync::atomic::Ordering::Relaxed);
    app.last_push_frames
        .store(frames.len() as u64, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// How long a cartridge of `frames` frames will take to reach the keyboard.
///
/// The device only starts playing an animation once *every* frame has landed,
/// so the front-end has to know the transfer time in advance: the frames are
/// stamped with the wall-clock second they will be shown at. The estimate is
/// extrapolated from the last real push (or from the theoretical chunk
/// pacing) plus the handshake slack.
#[tauri::command]
pub fn screen_cartridge_estimate(
    app: tauri::State<'_, AppState>,
    frames: usize,
    frame_bytes: usize,
) -> u64 {
    use std::sync::atomic::Ordering::Relaxed;
    let measured = app.last_push_ms.load(Relaxed);
    let last_frames = app.last_push_frames.load(Relaxed);
    let measured_per_frame = if last_frames > 0 {
        measured / last_frames
    } else {
        0
    };
    // 56-byte chunks, ~250µs of transfer each (plus a floor of 1ms/frame)
    let chunks = ((frame_bytes + 55) / 56) as u64;
    let theoretical_per_frame = (chunks * 250 / 1000).max(1);
    let per_frame = measured_per_frame.max(theoretical_per_frame).max(1);
    per_frame * frames as u64 + 900
}

/// File dialog via rfd; must run on the main thread (GTK). Returns the path.
fn pick_file(
    app: &tauri::AppHandle,
    title: &str,
    save_name: Option<String>,
    filters: Vec<(&'static str, Vec<&'static str>)>,
) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    let title = title.to_string();
    app.run_on_main_thread(move || {
        let mut d = rfd::FileDialog::new().set_title(&title);
        for (name, exts) in filters {
            d = d.add_filter(name, &exts);
        }
        if let Some(ref n) = save_name {
            d = d.set_file_name(n);
        }
        let picked = if save_name.is_some() {
            d.save_file()
        } else {
            d.pick_file()
        }
        .map(|p| p.to_string_lossy().into_owned());
        let _ = tx.send(picked);
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())
}

/// File dialog via rfd; returns absolute path picked by user.
#[tauri::command]
pub fn pick_open_image(app_handle: tauri::AppHandle) -> Result<Option<String>, String> {
    pick_file(
        &app_handle,
        "Pick an image or GIF for the screen",
        None,
        vec![(
            "Image or GIF",
            vec!["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico"],
        )],
    )
}

#[tauri::command]
pub fn pick_save_file(
    app_handle: tauri::AppHandle,
    name: String,
) -> Result<Option<String>, String> {
    pick_file(&app_handle, "Save backup", Some(name), vec![])
}

#[tauri::command]
pub fn pick_open_json(app_handle: tauri::AppHandle) -> Result<Option<String>, String> {
    pick_file(
        &app_handle,
        "Pick a backup file",
        None,
        vec![("SharkDeck / JSON", vec!["json"])],
    )
}

#[tauri::command]
pub fn keymap_backup(app: tauri::State<'_, AppState>, path: String, profile: u8) -> Result<(), String> {
    let kb = kb_of(&app)?;
    let base = kb.keymap_layer(profile)?;
    let fns = kb.fn_layer(profile)?;
    let doc = serde_json::json!({
        "format": "sharkdeck-keymap",
        "version": 1,
        "device": kb.device_id,
        "profile": profile,
        "base": base64_encode(&base),
        "fn": base64_encode(&fns),
    });
    fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).map_err(|e| e.to_string())
}

/// Full settings snapshot — the only thing that can be backed up over HID.
///
/// The firmware never reads its own flash out over USB (verified: no GET
/// opcode returns firmware bytes; the boot loader is write-and-verify only),
/// so a per-unit *firmware* image is not obtainable. Everything that IS
/// readable is captured here: the keymap + Fn layer of all three profiles,
/// all 50 macro slots, the option bytes and both lighting states.
#[tauri::command]
pub fn settings_backup(app: tauri::State<'_, AppState>, path: String) -> Result<String, String> {
    let kb = kb_of(&app)?;
    let info = kb.info()?;
    let mut profiles = Vec::new();
    for p in 0..3u8 {
        let base = kb.keymap_layer(p)?;
        let fns = kb.fn_layer(p)?;
        profiles.push(serde_json::json!({
            "index": p,
            "base": base64_encode(&base),
            "fn": base64_encode(&fns),
        }));
    }
    let mut macros = Vec::new();
    for s in 0..50u8 {
        let data = kb.macro_read(s)?;
        macros.push(serde_json::json!({
            "slot": s,
            "data": base64_encode(&data),
        }));
    }
    let doc = serde_json::json!({
        "format": "sharkdeck-settings",
        "version": 1,
        "created": chrono::Local::now().to_rfc3339(),
        "device": {
            "path": info.path,
            "device_id": info.device_id,
            "firmware": info.firmware,
            "active_profile": info.active_profile,
        },
        "profiles": profiles,
        "macros": macros,
        "options": kb.options_get()?,
        "lighting": { "main": kb.light(false)?, "side": kb.light(true)? },
    });
    fs::write(&path, serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(path)
}

/// Restore a settings snapshot. Only slots that actually differ are written,
/// and every keymap write is verified by read-back.
#[tauri::command]
pub fn settings_restore(app: tauri::State<'_, AppState>, path: String) -> Result<String, String> {
    let kb = kb_of(&app)?;
    let doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&path).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if doc["format"].as_str() != Some("sharkdeck-settings") {
        return Err("not a SharkDeck settings backup".into());
    }
    let mut report = String::new();

    if let Some(profiles) = doc["profiles"].as_array() {
        let mut slots = 0usize;
        for p in profiles {
            let idx = p["index"].as_u64().unwrap_or(0) as u8;
            for (layer_flag, field) in [(false, "base"), (true, "fn")] {
                let Some(b) = p[field].as_str() else { continue };
                let bytes = base64_decode(b)?;
                if bytes.len() != 512 {
                    return Err(format!("profile {idx} {field} wrong size"));
                }
                let current = if layer_flag { kb.fn_layer(idx)? } else { kb.keymap_layer(idx)? };
                for slot in 0..128u8 {
                    let o = slot as usize * 4;
                    if current[o..o + 4] == bytes[o..o + 4] {
                        continue;
                    }
                    let entry = SlotEntry([
                        bytes[o],
                        bytes[o + 1],
                        bytes[o + 2],
                        bytes[o + 3],
                    ]);
                    kb.write_slot(idx, slot, entry, layer_flag)?;
                    slots += 1;
                }
            }
        }
        report.push_str(&format!("keymap: {slots} slots written"));
    }

    if let Some(macros) = doc["macros"].as_array() {
        let mut written = 0usize;
        for m in macros {
            let slot = m["slot"].as_u64().unwrap_or(0) as u8;
            let Some(d) = m["data"].as_str() else { continue };
            let bytes = base64_decode(d)?;
            if bytes.len() != 256 {
                continue;
            }
            let cur = kb.macro_read(slot)?;
            if cur[..] == bytes[..] {
                continue;
            }
            let mut blob = [0u8; 256];
            blob.copy_from_slice(&bytes);
            kb.macro_write(slot, &blob)?;
            written += 1;
        }
        report.push_str(&format!(" · macros: {written} slots written"));
    }

    if let Some(o) = doc["options"].as_object() {
        let cur = kb.options_get()?;
        let want = KbOptions {
            win_lock: o.get("win_lock").and_then(|v| v.as_bool()).unwrap_or(cur.win_lock),
            mac_mode: o.get("mac_mode").and_then(|v| v.as_bool()).unwrap_or(cur.mac_mode),
            wasd_swap: o.get("wasd_swap").and_then(|v| v.as_bool()).unwrap_or(cur.wasd_swap),
            main_led_off: o.get("main_led_off").and_then(|v| v.as_bool()).unwrap_or(cur.main_led_off),
            side_led_off: o.get("side_led_off").and_then(|v| v.as_bool()).unwrap_or(cur.side_led_off),
            keyboard_mode: o.get("keyboard_mode").and_then(|v| v.as_bool()).unwrap_or(cur.keyboard_mode),
            keyboard_lock: o.get("keyboard_lock").and_then(|v| v.as_bool()).unwrap_or(cur.keyboard_lock),
            fn_matrix: o.get("fn_matrix").and_then(|v| v.as_u64()).unwrap_or(cur.fn_matrix as u64) as u8,
            power_save: o.get("power_save").and_then(|v| v.as_u64()).unwrap_or(cur.power_save as u64) as u8,
        };
        if want != cur {
            kb.options_set(want)?;
            report.push_str(" · options restored");
        }
    }

    if let Some(l) = doc["lighting"].as_object() {
        if let Some(m) = l.get("main") {
            if let Ok(state) = serde_json::from_value::<LightingState>(m.clone()) {
                let cur = kb.light(false)?;
                if state != cur {
                    kb.light_set(false, state)?;
                    report.push_str(" · main lighting restored");
                }
            }
        }
        if let Some(s) = l.get("side") {
            if let Ok(state) = serde_json::from_value::<LightingState>(s.clone()) {
                let cur = kb.light(true)?;
                if state != cur {
                    kb.light_set(true, state)?;
                    report.push_str(" · side lighting restored");
                }
            }
        }
    }

    Ok(if report.is_empty() {
        "already identical — nothing written".to_string()
    } else {
        report
    })
}

fn base64_encode(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn keymap_restore(app: tauri::State<'_, AppState>, path: String) -> Result<usize, String> {
    let kb = kb_of(&app)?;
    let doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&path).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if doc["format"].as_str() != Some("sharkdeck-keymap") {
        return Err("not a sharkdeck keymap backup".into());
    }
    let profile = doc["profile"].as_u64().unwrap_or(0) as u8;
    let mut restored = 0usize;
    for (layer_flag, field) in [(false, "base"), (true, "fn")] {
        let Some(b) = doc[field].as_str() else {
            continue;
        };
        let bytes = base64_decode(b)?;
        if bytes.len() != 512 {
            return Err(format!("{} layer wrong size", field));
        }
        for slot in 0..128u8 {
            let cur = kb.read_slot(profile, slot, layer_flag)?;
            let target = SlotEntry([
                bytes[slot as usize * 4],
                bytes[slot as usize * 4 + 1],
                bytes[slot as usize * 4 + 2],
                bytes[slot as usize * 4 + 3],
            ]);
            if cur == target {
                continue;
            }
            kb.write_slot(profile, slot, target, layer_flag)?;
            let check = kb.read_slot(profile, slot, layer_flag)?;
            if check != target {
                log(&app, format!("restore slot {slot} mismatch"));
            } else {
                restored += 1;
            }
        }
    }
    Ok(restored)
}
