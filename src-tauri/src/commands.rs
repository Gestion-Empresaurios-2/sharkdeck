//! Tauri command surface: thin serde wrappers over sharkdeck-core.

use sharkdeck_core::kb::{Keyboard, KbOptions, LightingState};
use sharkdeck_core::layout;
use sharkdeck_core::protocol::{PixelOrder, SlotEntry};
use sharkdeck_core::screen as scr;
use std::fs;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[derive(Default)]
pub struct AppState {
    pub kb: Arc<Mutex<Option<Arc<Keyboard>>>>,
    pub log: Arc<Mutex<Vec<String>>>,
    pub config: Arc<Mutex<AppConfig>>,
    pub ready: std::sync::atomic::AtomicBool,
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
    let fresh = Keyboard::open()?;
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
    let packed = scr::frames_to_565(
        &scr::clock_frames(
            n,
            with_seconds,
            color.unwrap_or([230, 233, 242]).into(),
            background.unwrap_or([10, 12, 18]).into(),
            cfg.screen_width,
            cfg.screen_height,
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
    if frames.len() > 90 {
        return Err("max 90 frames".into());
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
    kb.screen_write_animation(&refs, width as u16, height as u16)
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
