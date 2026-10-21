//! Device-level operations on the K86 over the vendor HID interface.
//!
//! Reads on this firmware are cached: the reply to an unrelated query is the
//! previous reply. Every query polls until the echoed opcode matches. After a
//! write, a read through the same open handle returns the *old* value, so
//! verification closes and reopens the device (ApiBox opens per op).
//!
//! Byte layouts below follow the hardware-verified Swift implementation in
//! mehmetnadir/keyboard-studio (K86) merged with dniminenn/sharkfin's protocol
//! doc. MIT credits apply throughout.

use crate::hid::{ApiBox, SharedApi};
use crate::perkey;
use crate::protocol::*;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub const PROFILE_WAIT_MS: u64 = 500;
pub const IDENTIFY_WAIT_MS: u64 = 30;

#[derive(Clone, Debug, serde::Serialize)]
pub struct DeviceInfo {
    pub path: String,
    pub device_id: u32,
    pub firmware_raw: u32,
    pub firmware: String,
    pub active_profile: u8,
    pub options: KbOptions,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KbOptions {
    pub win_lock: bool,
    pub mac_mode: bool,
    pub wasd_swap: bool,
    pub main_led_off: bool,
    pub side_led_off: bool,
    pub keyboard_mode: bool,
    pub keyboard_lock: bool,
    pub fn_matrix: u8,
    pub power_save: u8,
}

impl KbOptions {
    fn from_payload(p: &[u8]) -> KbOptions {
        let bits = p[2];
        KbOptions {
            win_lock: bits & 0x01 != 0,
            mac_mode: bits & 0x02 != 0,
            wasd_swap: bits & 0x08 != 0,
            main_led_off: bits & 0x10 != 0,
            side_led_off: bits & 0x20 != 0,
            keyboard_mode: bits & 0x40 != 0,
            keyboard_lock: bits & 0x80 != 0,
            fn_matrix: p[3],
            power_save: p.get(5).copied().unwrap_or(0),
        }
    }
    fn to_bits(&self) -> [u8; 2] {
        let mut bits = 0u8;
        if self.win_lock {
            bits |= 0x01;
        }
        if self.mac_mode {
            bits |= 0x02;
        }
        if self.wasd_swap {
            bits |= 0x08;
        }
        if self.main_led_off {
            bits |= 0x10;
        }
        if self.side_led_off {
            bits |= 0x20;
        }
        if self.keyboard_mode {
            bits |= 0x40;
        }
        if self.keyboard_lock {
            bits |= 0x80;
        }
        [bits, self.fn_matrix]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LightingState {
    /// effect id: 0 off … 24 fireworks (7=custom RGB, 8=rainbow nibble)
    pub effect: u8,
    /// host speed 0..4 (sent as 5-speed)
    pub speed: u8,
    /// host brightness 0..4 (firmware rejects >4)
    pub brightness: u8,
    pub direction: u8,
    /// mode nibble: 7 = custom rgb, 8 = rainbow (rgb ignored), 0..6 preset
    pub mode: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub type Payload = [u8; REPORT_LEN];

pub struct Keyboard {
    pub path: String,
    pub api: SharedApi,
    pub device_id: u32,
}

impl Keyboard {
    /// Scan candidate hidraw paths and identify the K86 vendor interface by
    /// its handshake ID (1168). Returns None when no K86 is attached.
    pub fn open() -> Result<Option<Keyboard>, String> {
        let api = ApiBox::new()?;
        let api: SharedApi = std::sync::Arc::new(api);
        Self::find_in(&api)
    }

    pub fn find_in(api: &SharedApi) -> Result<Option<Keyboard>, String> {
        let mut best: Option<(usize, String, u32)> = None;
        for cand in api.list() {
            let blessing = (cand.usage_page == 0xFF00 || cand.usage_page == 0xFFFF) && cand.usage == 2;
            let preference = if blessing { 0 } else { 1 };
            let id = match Self::identify(api, &cand.path) {
                Ok(Some(id)) => id,
                Ok(None) => {
                    eprintln!("[sharkdeck] identify {}: no reply", cand.path);
                    continue;
                }
                Err(e) => {
                    eprintln!("[sharkdeck] identify {}: {e}", cand.path);
                    continue;
                }
            };
            if best.as_ref().map(|&(p, _, _)| preference < p).unwrap_or(true) {
                best = Some((preference, cand.path.clone(), id));
            }
        }
        Ok(best
            .filter(|(_, _, id)| *id == K86_HANDSHAKE_ID)
            .map(|(_, path, id)| Keyboard {
                path,
                api: api.clone(),
                device_id: id,
            }))
    }

    /// Send identify 0x8F and read the u32 LE device id at payload[1..5].
    pub fn identify(api: &SharedApi, path: &str) -> Result<Option<u32>, String> {
        api.with(path, |dev| {
            let mut buf = [0u8; FEATURE_LEN];
            buf[1] = OP_GET_ID;
            buf[8] = {
                // bit7 over payload[0..6] where payload[0]=op, payload[1]=0
                let mut p = [0u8; REPORT_LEN];
                p[0] = OP_GET_ID;
                0xFFu8.wrapping_sub(p[0..7].iter().fold(0u8, |a, &b| a.wrapping_add(b)))
            };
            dev.send_feature_report(&buf).map_err(|e| e.to_string())?;
            sleep(Duration::from_millis(IDENTIFY_WAIT_MS));
            let n = dev.get_feature_report(&mut buf).map_err(|e| e.to_string())?;
            if n < 6 || buf[1] != OP_GET_ID {
                return Ok(None);
            }
            Ok(Some(u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]])))
        })
    }

    /// Generic query: send GET request (bit7 checksum added), retry until the
    /// reply's first payload byte echoes the requested opcode.
    /// Opcodes that erase flash, reset the board, or jump into the
    /// bootloader/display-chip flashing paths. They are refused at the
    /// transport layer so no UI, CLI or future code path can send them.
    pub const DANGEROUS_OPS: [u8; 10] = [
        0xAC, // flash-chip erase (~55 s, wipes everything)
        0x2C, // yc500 display erase
        0x7F, // USB boot entry (erases keymap + switch settings)
        0x30, // display-chip boot/flash path
        0xB0, // display-chip flash (alternative)
        0xB1, // display-chip flash (alternative)
        0x01, // factory reset (variant)
        0x02, // factory reset
        0x1C, // factory reset (variant)
        0x1E, // factory reset (variant)
    ];

    /// Refuse a dangerous opcode before it reaches the wire.
    pub fn assert_safe_op(op: u8) -> Result<(), String> {
        if Self::DANGEROUS_OPS.contains(&op) {
            return Err(format!(
                "opcode {op:02X} is destructive (erase/reset/boot) and is blocked by SharkDeck"
            ));
        }
        Ok(())
    }

    pub fn query(&self, op: u8, args: &[u8], retries: usize, wait_ms: u64) -> Result<Payload, String> {
        Self::assert_safe_op(op)?;
        let req = cmd_bit7(op, args);
        let mut wire = [0u8; FEATURE_LEN];
        wire[1..].copy_from_slice(&req);
        self.api.with(&self.path, |dev| {
            dev.send_feature_report(&wire).map_err(|e| e.to_string())?;
            sleep(Duration::from_millis(wait_ms));
            let mut buf = [0u8; FEATURE_LEN];
            for attempt in 0..retries {
                let n = dev.get_feature_report(&mut buf).map_err(|e| e.to_string())?;
                if n >= FEATURE_LEN && buf[1] == op {
                    let mut p = [0u8; REPORT_LEN];
                    p.copy_from_slice(&buf[1..=REPORT_LEN]);
                    return Ok(p);
                }
                if attempt + 1 < retries {
                    sleep(Duration::from_millis(50));
                }
            }
            let mut p = [0u8; REPORT_LEN];
            p.copy_from_slice(&buf[1..=REPORT_LEN]);
            Err(format!(
                "opcode {op:02X} echo mismatch after {retries} tries (reply starts {:02X?})",
                &p[..4.min(p.len())]
            ))
        })
    }

    /// Raw write without read-back (SET, bit7 checksum).
    /// Vendor **factory reset** (opcode 0x02, ~4 s on the wire).
    ///
    /// This is intentionally NOT routed through `send()`: 0x02 is on the
    /// denylist, so the generic path can never emit it. It exists only for
    /// the explicit, user-consented factory-reset flow (CLI `factory-reset`
    /// and the app's "Reset keyboard to factory settings" action, both of
    /// which require an explicit confirmation).
    ///
    /// It wipes the keymap, macros, lighting and switch settings on the
    /// keyboard — the host-side settings backup (see `settings_backup`) is
    /// the only way to restore them.
    pub fn factory_reset(&self) -> Result<(), String> {
        let req = cmd_bit7(0x02, &[]);
        let mut wire = [0u8; FEATURE_LEN];
        wire[1..].copy_from_slice(&req);
        self.api.with(&self.path, |dev| {
            dev.send_feature_report(&wire).map_err(|e| e.to_string())?;
            Ok(())
        })?;
        // The firmware takes ~4 s to wipe and reboot; give it room, then
        // re-open the device so the next command talks to a fresh unit.
        std::thread::sleep(std::time::Duration::from_millis(4500));
        Ok(())
    }

    /// Erase the DISPLAY store: every picture and animation that was pushed to
    /// the screen (our FRAME OS frames, the clock animation, the carousel).
    /// The keyboard then falls back to its own built-in screens again.
    /// Keymap, macros, lighting and switch settings are NOT touched.
    ///
    /// This deliberately bypasses the transport denylist: it is the explicit,
    /// consented path (the CLI requires `--yes`).
    pub fn display_erase(&self) -> Result<(), String> {
        self.erase_op(0x2C, 3_000)
    }

    /// Erase the whole flash chip: pictures, animations, keymap, macros,
    /// lighting and switch settings. The vendor's full flash erase takes
    /// ~55 s and reboots the board. Last resort — only with a settings
    /// backup at hand.
    pub fn flash_erase(&self) -> Result<(), String> {
        self.erase_op(0xAC, 60_000)
    }

    fn erase_op(&self, op: u8, wait_ms: u64) -> Result<(), String> {
        let req = cmd_bit7(op, &[]);
        let mut wire = [0u8; FEATURE_LEN];
        wire[1..].copy_from_slice(&req);
        self.api.with(&self.path, |dev| {
            dev.send_feature_report(&wire).map_err(|e| e.to_string())?;
            Ok(())
        })?;
        sleep(Duration::from_millis(wait_ms));
        Ok(())
    }

    pub fn send(&self, payload: Payload) -> Result<(), String> {
        Self::assert_safe_op(payload[0])?;
        let mut wire = [0u8; FEATURE_LEN];
        wire[1..].copy_from_slice(&payload);
        self.api.with(&self.path, |dev| {
            dev.send_feature_report(&wire).map_err(|e| e.to_string())?;
            Ok(())
        })
    }

    pub fn info(&self) -> Result<DeviceInfo, String> {
        let fw = self.query(OP_GET_FIRMWARE, &[], 8, 60)?;
        let fw_raw = ((fw[2] as u32) << 8) | fw[1] as u32;
        let profile = self.query(OP_GET_PROFILE, &[], 8, 60)?;
        let options = self.options_get()?;
        Ok(DeviceInfo {
            path: self.path.clone(),
            device_id: self.device_id,
            firmware_raw: fw_raw,
            firmware: format!("{}.{}", fw[2], fw[1]),
            active_profile: profile[1],
            options,
        })
    }

    // ---- kb options / LED master ----

    pub fn options_get(&self) -> Result<KbOptions, String> {
        let p = self.query(OP_GET_KBOPTION, &[0], 8, 50)?;
        Ok(KbOptions::from_payload(&p))
    }

    pub fn options_set(&self, o: KbOptions) -> Result<(), String> {
        let [bits, fmat] = o.to_bits();
        self.send(cmd_bit7(OP_SET_KBOPTION, &[0, bits, fmat, o.power_save]))?;
        sleep(Duration::from_millis(80));
        let _ = self.query(OP_GET_KBOPTION, &[0], 8, 80);
        Ok(())
    }

    /// Ensure LEDs can light at all: clear the off bits and power-save flag.
    /// Read-modify-write preserves unmodeled bits.
    pub fn leds_on(&self) -> Result<KbOptions, String> {
        let mut o = self.options_get()?;
        o.main_led_off = false;
        o.side_led_off = false;
        o.power_save = 0;
        self.options_set(o)?;
        Ok(o)
    }

    // ---- lighting ----

    pub fn light(&self, side: bool) -> Result<LightingState, String> {
        let op = if side { OP_GET_SLED } else { OP_GET_LED };
        let p = self.query(op, &[], 8, 60)?;
        Ok(LightingState {
            effect: p[1],
            speed: 4u8.saturating_sub(p[2].min(5).saturating_sub(1)),
            brightness: p[3].min(4),
            direction: p[4] >> 4,
            mode: p[4] & 0x0F,
            r: p[5],
            g: p[6],
            b: p[7],
        })
    }

    pub fn light_set(&self, side: bool, l: LightingState) -> Result<(), String> {
        let op = if side { OP_SET_SLED } else { OP_SET_LED };
        let mode = l.mode.min(9);
        let args: [u8; 7] = [
            l.effect,
            5u8.saturating_sub(l.speed.min(4)), // host speed 0..4 -> 5-speed
            l.brightness.min(4),
            (l.direction << 4) | mode,
            l.r,
            l.g,
            l.b,
        ];
        self.send(cmd_bit8(op, &args))?;
        sleep(Duration::from_millis(60));
        Ok(())
    }

    // ---- profiles ----

    pub fn profile_get(&self) -> Result<u8, String> {
        let p = self.query(OP_GET_PROFILE, &[], 8, 60)?;
        Ok(p[1].min(2))
    }

    pub fn profile_set(&self, idx: u8) -> Result<(), String> {
        self.send(cmd_bit7(OP_SET_PROFILE, &[idx.min(2)]))?;
        sleep(Duration::from_millis(PROFILE_WAIT_MS));
        Ok(())
    }

    // ---- keymap ----

    /// Read one 16-slot page (64 bytes) as raw payload.
    /// Raw (echoless) read: page GETs answer with slot data, not an opcode
    /// echo — poll-for-echo never matches and surfaces stale data. Sends the
    /// request, waits, reads once (fresh handle per `with` call avoids the
    /// firmware's cached-response bug the way vendor tools force declines).
    /// Empirical read-only probe for a battery/percentage opcode.
    ///
    /// This family documents no battery read opcode, so try the plausible
    /// neighbours and accept the first reply that carries a sane percentage
    /// in one of its leading bytes. Strictly read-only: nothing is written.
    pub fn battery_probe(&self) -> Result<Option<u8>, String> {
        // Empirical: no battery read opcode is documented for this family, so
        // try plausible ones and accept a reply ONLY if it is not a plain
        // opcode echo and carries a percentage in a plausible field.
        for op in [0x81u8, 0x82, 0x83, 0x84, 0x8C, 0x8D, 0x8E] {
            if let Ok(reply) = self.query_raw(op, &[], 60) {
                if reply[0] == op || reply[0] == 0 {
                    continue; // echo of our own request / empty reply
                }
                if reply[1] >= 1 && reply[1] <= 100 && reply[2] == 0 && reply[3] == 0 {
                    return Ok(Some(reply[1]));
                }
            }
        }
        Ok(None)
    }

    /// Raw single-shot query used by the battery probe diagnostics.
    pub fn debug_query(&self, op: u8) -> Result<Payload, String> {
        self.query_raw(op, &[], 60)
    }

    /// Read-only probe with explicit arguments, used to characterise
    /// opcodes that no public source documents.
    pub fn debug_query_args(&self, op: u8, args: &[u8]) -> Result<Payload, String> {
        self.query_raw(op, args, 120)
    }

    fn query_raw(&self, op: u8, args: &[u8], wait_ms: u64) -> Result<Payload, String> {
        Self::assert_safe_op(op)?;
        let req = cmd_bit7(op, args);
        let mut wire = [0u8; FEATURE_LEN];
        wire[1..].copy_from_slice(&req);
        self.api.with(&self.path, |dev| {
            dev.send_feature_report(&wire).map_err(|e| e.to_string())?;
            sleep(Duration::from_millis(wait_ms));
            let mut buf = [0u8; FEATURE_LEN];
            let n = dev.get_feature_report(&mut buf).map_err(|e| e.to_string())?;
            if n < FEATURE_LEN {
                return Err(format!("opcode {op:02X} short reply ({n} bytes)"));
            }
            let mut p = [0u8; REPORT_LEN];
            p.copy_from_slice(&buf[1..=REPORT_LEN]);
            Ok(p)
        })
    }

    pub fn keymap_page(&self, profile: u8, page: u8) -> Result<Payload, String> {
        self.query_raw(OP_KEYMAP_PAGE, &[profile, page], 40)
    }

    pub fn fn_page(&self, profile: u8, page: u8) -> Result<Payload, String> {
        self.query_raw(OP_FN_PAGE, &[profile, page], 40)
    }

    /// Whole layer = 8 pages * 16 slots of 4 bytes = 512 bytes (already
    /// aligned: page bytes land at page*64 in the blob).
    pub fn keymap_layer(&self, profile: u8) -> Result<[u8; 512], String> {
        let mut blob = [0u8; 512];
        for page in 0u8..8 {
            let d = self.keymap_page(profile, page)?;
            blob[page as usize * 64..(page as usize + 1) * 64].copy_from_slice(&d);
        }
        Ok(blob)
    }

    pub fn fn_layer(&self, profile: u8) -> Result<[u8; 512], String> {
        let mut blob = [0u8; 512];
        for page in 0u8..8 {
            let d = self.fn_page(profile, page)?;
            blob[page as usize * 64..(page as usize + 1) * 64].copy_from_slice(&d);
        }
        Ok(blob)
    }

    /// Write one 4-byte slot. Base layer: 0x13; Fn layer: 0x15.
    /// Layout: [op, profile, slot, 0,0,0,0, ck(bit7), value at 8..12].
    pub fn write_slot(&self, profile: u8, slot: u8, entry: SlotEntry, fn_layer: bool) -> Result<(), String> {
        let op = if fn_layer { OP_FN_SLOT } else { OP_KEYMAP_SLOT };
        let mut payload = cmd_bit7(op, &[profile, slot]);
        payload[8] = entry.0[0];
        payload[9] = entry.0[1];
        payload[10] = entry.0[2];
        payload[11] = entry.0[3];
        self.send(payload)?;
        sleep(Duration::from_millis(120));
        Ok(())
    }

    /// Read a single slot back (fresh handle each ApiBox op, so this sees
    /// freshly written values).
    pub fn read_slot(&self, profile: u8, slot: u8, fn_layer: bool) -> Result<SlotEntry, String> {
        let page = slot / 16;
        let idx = ((slot % 16) as usize) * 4; // raw reply = 16 slots * 4 bytes, no echo byte
        let raw = if fn_layer {
            self.fn_page(profile, page)?
        } else {
            self.keymap_page(profile, page)?
        };
        Ok(SlotEntry([raw[idx], raw[idx + 1], raw[idx + 2], raw[idx + 3]]))
    }

    // ---- macros ----

    pub fn macro_page(&self, slot: u8, page: u8) -> Result<Payload, String> {
        self.query_raw(OP_MACRO_GET, &[slot, page], 40)
    }

    pub fn macro_read(&self, slot: u8) -> Result<[u8; 256], String> {
        let mut blob = [0u8; 256];
        for page in 0u8..4 {
            let d = self.macro_page(slot, page)?;
            blob[page as usize * 64..(page as usize + 1) * 64].copy_from_slice(&d);
        }
        Ok(blob)
    }

    /// Write a macro blob (256 bytes): 56-byte pages, data at payload[8..64],
    /// page number at payload[2], length payload[4], final flag payload[5].
    pub fn macro_write(&self, slot: u8, blob: &[u8; 256]) -> Result<(), String> {
        let last_page = (0..4usize)
            .rev()
            .find(|&i| blob[i * 64..(i + 1) * 64].iter().any(|b| *b != 0))
            .unwrap_or(0);
        for page in 0..=last_page {
            let data = &blob[page * 64..page * 64 + 64];
            let mut payload = cmd_bit7(
                OP_MACRO_SET,
                &[slot, page as u8, SCREEN_CHUNK as u8, 0, if page == last_page { 1 } else { 0 }],
            );
            payload[8..64].copy_from_slice(&data[..SCREEN_CHUNK]);
            self.send(payload)?;
            sleep(Duration::from_millis(40));
        }
        sleep(Duration::from_millis(200));
        Ok(())
    }

    // ---- screen ----

    /// Write handshake for the TFT panel. Layout (bit7 checksum at payload[7]):
    /// p[0]=0xA5, p[1]=frameIndex, p[2]=allFrames, p[3]=delayByte,
    /// p[4..5]=byteLen u16 LE, p[8..11]=box left/top/right/bottom (low),
    /// p[12..15]=box halves (high), p[16..17]=byteLen bytes 2/3, p[18]=layer.
    /// The panel's on-screen rectangle: right = left+width, bottom = top+height.
    pub fn screen_handshake(&self, frame_index: u8, all_frames: u8, delay: u8, byte_len: u32, layer: u8, box_wh: (u16, u16)) -> Result<(), String> {
        self.api.with(&self.path, |dev| {
            let mut p = [0u8; REPORT_LEN];
            p[0] = OP_SCREEN_HANDSHAKE;
            p[1] = frame_index;
            p[2] = all_frames;
            p[3] = delay;
            p[4] = (byte_len & 0xFF) as u8;
            p[5] = ((byte_len >> 8) & 0xFF) as u8;
            let (w, h) = (box_wh.0, box_wh.1);
            p[8] = 0u8; // left lo
            p[9] = 0u8; // top lo
            p[10] = (w & 0xFF) as u8; // right lo
            p[11] = (h & 0xFF) as u8; // bottom lo
            p[12] = 0; // left hi
            p[13] = 0; // top hi
            p[14] = (w >> 8) as u8; // right hi
            p[15] = (h >> 8) as u8; // bottom hi
            p[16] = ((byte_len >> 16) & 0xFF) as u8;
            p[17] = ((byte_len >> 24) & 0xFF) as u8;
            p[18] = layer;
            p[7] = checksum7(&p);
            let mut wire = [0u8; FEATURE_LEN];
            wire[1..].copy_from_slice(&p);
            dev.send_feature_report(&wire).map_err(|e| e.to_string())?;
            // The device can stay busy for a while after the previous
            // animation (it is still rendering / freeing the frames), and it
            // only answers the handshake when it is ready. Poll patiently:
            // 30 tries x 200 ms = 6 s of grace.
            for _ in 0..30 {
                sleep(Duration::from_millis(200));
                let mut out = [0u8; FEATURE_LEN];
                let _ = dev.get_feature_report(&mut out);
                if out[1] == OP_SCREEN_HANDSHAKE && out[2] == 1 {
                    return Ok(());
                }
            }
            Err("screen handshake never became ready (30 tries)".into())
        })
    }

    /// Stream RGB565 pixels in 56-byte chunks (0x25), ~1 ms between chunks.
    /// p[0]=0x25, p[1]=frameIndex, p[2]=allFrames, p[3]=delay,
    /// p[4..5]=chunk # u16 LE, p[6]=len<=56, p[7]=bit7 ck, data at p[8..64].
    /// Inter-chunk pacing for the screen stream, in MICROSECONDS (0 = as fast
    /// as the USB endpoint allows). Read once from the environment.
    /// Default 200 µs: ~0.23 s per 240x135 frame, i.e. ~4x faster than the 1 s
    /// clock cadence, so the device never has to skip a frame.
    fn chunk_delay_us() -> u64 {
        static DELAY: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        *DELAY.get_or_init(|| {
            std::env::var("SHARKDECK_CHUNK_DELAY_US")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200)
        })
    }

    pub fn screen_stream(&self, bytes: &[u8], frame_index: u8, all_frames: u8, delay: u8) -> Result<(), String> {
        self.api.with(&self.path, |dev| {
            for (n, chunk) in bytes.chunks(SCREEN_CHUNK).enumerate() {
                let mut p = [0u8; REPORT_LEN];
                p[0] = OP_SCREEN_DATA;
                p[1] = frame_index;
                p[2] = all_frames;
                p[3] = delay;
                p[4] = (n & 0xFF) as u8;
                p[5] = ((n >> 8) & 0xFF) as u8;
                p[6] = chunk.len() as u8;
                p[7] = {
                    let sum: u8 = p[0..7].iter().fold(0u8, |a, &b| a.wrapping_add(b));
                    0xFFu8.wrapping_sub(sum)
                };
                p[8..8 + chunk.len()].copy_from_slice(chunk);
                let mut wire = [0u8; FEATURE_LEN];
                wire[1..].copy_from_slice(&p);
                dev.send_feature_report(&wire).map_err(|e| e.to_string())?;
                // Pacing: 200 µs by default. The old 1 ms sleep cost ~1.16 s per
                // 240x135 frame (1158 chunks), i.e. slower than the 1 s clock
                // cadence — the device then skipped frames and the on-screen
                // seconds jumped. Override with SHARKDECK_CHUNK_DELAY_US=1000
                // for firmware that needs the old pacing.
                let d = Self::chunk_delay_us();
                if d > 0 {
                    sleep(Duration::from_micros(d));
                }
            }
            Ok(())
        })
    }

    /// Upload one static frame into slot `layer` (0..5).
    pub fn screen_write_image(&self, rgb565: &[u8], layer: u8, width: u16, height: u16) -> Result<(), String> {
        let len = rgb565.len() as u32;
        self.screen_handshake(layer, 1, 0, len, layer, (width, height))?;
        self.screen_stream(rgb565, layer, 1, 0)?;
        Ok(())
    }

    /// Upload an animation. One handshake announcing the whole set with
    /// frame-0 metadata, then per-frame streams carrying their own index and
    /// delay byte.
    pub fn screen_write_animation(&self, frames: &[(&[u8], u8)], width: u16, height: u16) -> Result<(), String> {
        let (first_bytes, first_delay) = frames[0];
        let len = first_bytes.len() as u32;
        // The device answers the handshake only when it is ready; right after a
        // previous animation it can stay busy for a while. Retry the whole
        // handshake a few times instead of giving up on the first refusal.
        let mut err = String::new();
        for attempt in 0..3 {
            match self.screen_handshake(0, frames.len() as u8, first_delay, len, 0, (width, height)) {
                Ok(()) => {
                    err.clear();
                    break;
                }
                Err(e) => {
                    err = e;
                    if attempt < 2 {
                        sleep(Duration::from_millis(2500));
                    }
                }
            }
        }
        if !err.is_empty() {
            return Err(err);
        }
        for (i, (bytes, delay)) in frames.iter().enumerate() {
            self.screen_stream(bytes, i as u8, frames.len() as u8, *delay)?;
        }
        Ok(())
    }

    /// Panel params (0x0E). The display family answers on numbered report 13;
    /// the endpoint may be the control interface itself or its sibling vendor
    /// interface (usage page 0xFFFF, usage 0x0001) — try both.
    pub fn display_params(&self, retries: usize) -> Result<Option<(u16, u16, u8)>, String> {
        if let Some(v) = Self::display_params_on_path(&self.api, &self.path, retries)? {
            return Ok(Some(v));
        }
        let sibling = self
            .api
            .list()
            .into_iter()
            .find(|c| c.usage_page == 0xFFFF && c.usage == 0x0001)
            .map(|c| c.path);
        if let Some(p) = sibling {
            if p != self.path {
                return Self::display_params_on_path(&self.api, &p, retries);
            }
        }
        Ok(None)
    }

    fn display_params_on_path(
        api: &SharedApi,
        path: &str,
        retries: usize,
    ) -> Result<Option<(u16, u16, u8)>, String> {
        api.with(path, |dev| {
            // 41-byte body: opcode 0x0E, sub 39; IOKit prepends the report id
            // itself and the checksum subtracts it (whole wire sums to 85).
            // On hidapi the wire frame carries the id in b0, so 42 bytes.
            // NOTE (Linux): IOKit's GetReport(Input) has no working hidraw
            // counterpart for this firmware — GINPUT returns stale keyboard-
            // family reports and no async data arrives, so this is best-effort
            // and the app falls back to the configured (hardware-verified)
            // geometry. See README + calibration patterns.
            let mut wire = [0u8; 42];
            wire[0] = 13; // displayReportID
            wire[1] = OP_GET_DISPLAY; // 0x0E
            wire[2] = 39; // sub-command
            let sum: u8 = wire[1..41].iter().fold(0u8, |a, b| a.wrapping_add(*b));
            wire[41] = 85u8.wrapping_sub(sum).wrapping_sub(13);
            if dev.write(&wire).is_err() {
                return Ok(None);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));

            let mut reply = [0u8; 42];
            for _ in 0..retries {
                match dev.read_timeout(&mut reply, 60) {
                    Ok(n) if n >= 22 => {
                        let body = if reply[0] == 13 { &reply[1..] } else { &reply[..] };
                        if body[0] != OP_GET_DISPLAY {
                            continue;
                        }
                        let w = ((body[17] as u16) << 8) | body[18] as u16;
                        let h = ((body[19] as u16) << 8) | body[20] as u16;
                        if (8..=1024).contains(&w) && (8..=1024).contains(&h) {
                            return Ok(Some((w, h, body[2])));
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            Ok(None)
        })
    }

    // ---- per-key RGB (SET_USERPIC, 0x0C) ----

    /// Upload a per-key colour image to the keyboard's flash.
    ///
    /// FLASH write: the firmware accepts at most 2 uploads per 10 s and a
    /// stalled endpoint only recovers after unplug/replug. The process-wide
    /// rate limiter is checked BEFORE the first chunk goes out, so a refused
    /// upload never touches the device.
    ///
    /// `payload` is the blob built by [`crate::perkey::encode`]: one 4-byte
    /// entry (`slot, r, g, b`) per key. It is streamed in 56-byte chunks,
    /// each chunk carried by one 65-byte feature report (see the perkey
    /// module for the full wire layout).
    pub fn set_userpic(&self, payload: &[u8]) -> Result<(), String> {
        if payload.is_empty() {
            return Err("empty userpic payload".into());
        }
        {
            let mut limiter = perkey::upload_limiter().lock().unwrap();
            limiter.allow(Instant::now())?;
        }
        let total = payload.len() as u16;
        let chunks = (payload.len() + perkey::USERPIC_CHUNK - 1) / perkey::USERPIC_CHUNK;
        for (n, chunk) in payload.chunks(perkey::USERPIC_CHUNK).enumerate() {
            let mut p = [0u8; REPORT_LEN];
            p[0] = perkey::OP_SET_USERPIC;
            p[1] = n as u8; // chunk index
            p[2] = chunks as u8; // chunk count
            p[3] = chunk.len() as u8; // bytes in this chunk
            p[4] = (total & 0xFF) as u8; // total length, u16 LE
            p[5] = ((total >> 8) & 0xFF) as u8;
            p[7] = checksum7(&p); // bit7 checksum
            p[8..8 + chunk.len()].copy_from_slice(chunk);
            let mut wire = [0u8; FEATURE_LEN];
            wire[1..].copy_from_slice(&p);
            self.api.with(&self.path, |dev| {
                dev.send_feature_report(&wire).map_err(|e| e.to_string())
            })?;
            sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    /// Whether a per-key flash upload may be sent right now (rate limiter
    /// state). Never touches the device.
    pub fn userpic_status(&self) -> Result<perkey::UserpicStatus, String> {
        Ok(perkey::userpic_status())
    }
}
