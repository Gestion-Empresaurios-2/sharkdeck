//! Byte-exact builders for the Attack Shark K86 (yc500 "royuan" family) control
//! protocol, reversed by the community and hardware-verified by:
//! - RaphaelCaputo2/shark-k86-mac (RGB + screen)
//! - mehmetnadir/keyboard-studio  (lighting, screen, knob reads on real K86)
//! - dniminenn/sharkfin (protocol bible, family registry)

pub const VID: u16 = 0x3151;
pub const PID: u16 = 0x4015;

/// Reply from identify (0x8F): u32 LE at payload bytes 1..4.
pub const K86_HANDSHAKE_ID: u32 = 1168;

/// 64-byte payload; wire transport is a 65-byte feature report (report id 0 first).
pub const REPORT_LEN: usize = 64;
pub const FEATURE_LEN: usize = 65;

// ---- opcodes (yc500 family-A shape; empirically verified on K86 hardware) ----
pub const OP_GET_ID: u8 = 0x8F;
pub const OP_GET_FIRMWARE: u8 = 0x80;
pub const OP_SET_LED: u8 = 0x07; // main lighting (bit8 checksum)
pub const OP_SET_SLED: u8 = 0x08; // side lighting (bit8 checksum)
pub const OP_GET_LED: u8 = 0x87;
pub const OP_GET_SLED: u8 = 0x88;
pub const OP_SET_KBOPTION: u8 = 0x06;
pub const OP_GET_KBOPTION: u8 = 0x86;
pub const OP_SET_PROFILE: u8 = 0x05;
pub const OP_GET_PROFILE: u8 = 0x85;
pub const OP_KEYMAP_PAGE: u8 = 0x89; // GET [profile,page] -> raw page
pub const OP_KEYMAP_SLOT: u8 = 0x13; // SET single slot (base layer)
pub const OP_FN_PAGE: u8 = 0x90; // GET Fn layer page
pub const OP_FN_SLOT: u8 = 0x15; // SET single Fn slot
pub const OP_MACRO_SET: u8 = 0x16; // 56-byte macro pages
pub const OP_MACRO_GET: u8 = 0x8B; // GET macro page
pub const OP_SCREEN_HANDSHAKE: u8 = 0xA5; // 165
pub const OP_SCREEN_DATA: u8 = 0x25; // 37, 56-byte chunks
pub const OP_GET_DISPLAY: u8 = 0x0E; // display params (width/height/brightness)

// Screen metadata
pub const SCREEN_CHUNK: usize = 56;
pub const IMAGE_SLOTS: u8 = 5;

/// Checksum: 0xFF - (sum & 0xFF). Bit7 over payload[0..6] lands in payload[7]
/// (every command except LED parameter sets); Bit8 over payload[0..7] lands in
/// payload[8] (SET_LEDPARAM / SET_SLEDPARAM only).
pub fn checksum7(payload: &[u8; REPORT_LEN]) -> u8 {
    let sum = payload[0..7].iter().fold(0u8, |a, &b| a.wrapping_add(b));
    0xFFu8.wrapping_sub(sum)
}

pub fn checksum8(payload: &[u8; REPORT_LEN]) -> u8 {
    let sum = payload[0..8].iter().fold(0u8, |a, &b| a.wrapping_add(b));
    0xFFu8.wrapping_sub(sum)
}

/// Build a SET packet payload: opcode + args, bit7 checksum at byte 7.
pub fn cmd_bit7(op: u8, args: &[u8]) -> [u8; REPORT_LEN] {
    let mut p = [0u8; REPORT_LEN];
    p[0] = op;
    for (i, a) in args.iter().enumerate() {
        if i + 1 >= p.len() {
            break;
        }
        p[i + 1] = *a;
    }
    p[7] = checksum7(&p);
    p
}

/// Build a SET packet payload for the lighting opcodes (bit8 checksum byte 8).
pub fn cmd_bit8(op: u8, args: &[u8]) -> [u8; REPORT_LEN] {
    let mut p = [0u8; REPORT_LEN];
    p[0] = op;
    for (i, a) in args.iter().enumerate() {
        if i + 1 >= p.len() {
            break;
        }
        p[i + 1] = *a;
    }
    p[7] = 0;
    p[8] = checksum8(&p);
    p
}

/// GET request for `op`: [op, 0, ...] with bit7 checksum (unless bit8 requested).
pub fn get_bit7(op: u8) -> [u8; REPORT_LEN] {
    cmd_bit7(op, &[0])
}

/// RGB565 encoding order: column-major + big endian was empirically verified on
/// the K86's square panel; row-major + little endian is what the vendor app
/// emits and matters only for non-square panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelOrder {
    Column,
    Row,
}

impl Default for PixelOrder {
    fn default() -> Self {
        PixelOrder::Column
    }
}

/// Pack a row-major RGB[A] buffer into the panel's RGB565 wire order.
/// `stride` = bytes per pixel (3 or 4).
pub fn encode565(
    rgb: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    order: PixelOrder,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * 2);
    let pack = |i: usize| -> u16 {
        let r = (rgb[i] >> 3) as u16;
        let g = (rgb[i + 1] >> 2) as u16;
        let b = (rgb[i + 2] >> 3) as u16;
        (r << 11) | (g << 5) | b
    };
    match order {
        PixelOrder::Column => {
            for x in 0..width {
                for y in 0..height {
                    let v = pack((y * width + x) * stride);
                    out.push((v >> 8) as u8);
                    out.push((v & 0xFF) as u8);
                }
            }
        }
        PixelOrder::Row => {
            for y in 0..height {
                for x in 0..width {
                    let v = pack((y * width + x) * stride);
                    out.push((v & 0xFF) as u8);
                    out.push((v >> 8) as u8);
                }
            }
        }
    }
    out
}

/// Keymap slot: 4 bytes.
/// [0x00, modifiers, hid usage, 0] | [0x03, 0, consumer code, 0]
/// | [0x09, mode, macro index, 0] | [0x0A, firmware action, 0, 0] | zeros.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotEntry(pub [u8; 4]);

impl SlotEntry {
    pub fn zero() -> SlotEntry {
        SlotEntry([0, 0, 0, 0])
    }
    pub fn is_empty(&self) -> bool {
        self.0 == [0, 0, 0, 0]
    }
    pub fn hid(modifiers: u8, usage: u8) -> SlotEntry {
        SlotEntry([0x00, modifiers, usage, 0])
    }
    pub fn consumer(code: u16) -> SlotEntry {
        SlotEntry([0x03, 0, (code & 0xFF) as u8, ((code >> 8) & 0xFF) as u8])
    }
    pub fn macro_bind(mode: u8, index: u8) -> SlotEntry {
        SlotEntry([0x09, mode, index, 0])
    }
    pub fn firmware(action: u8) -> SlotEntry {
        SlotEntry([0x0A, action, 0, 0])
    }
}

/// Consumer (media) usage codes present in the K86 firmware.
pub mod consumer {
    pub const VOL_UP: u16 = 0xE9;
    pub const VOL_DOWN: u16 = 0xEA;
    pub const MUTE: u16 = 0xE2;
    pub const PLAY_PAUSE: u16 = 0xCD;
    pub const NEXT: u16 = 0xB5;
    pub const PREV: u16 = 0xB6;
    pub const STOP: u16 = 0xB7;
    pub const BRIGHTNESS_UP: u16 = 0x6F;
    pub const BRIGHTNESS_DOWN: u16 = 0x70;
    pub const SEARCH: u16 = 0x221;
    pub const CALCULATOR: u16 = 0x192;
    pub const BROWSER: u16 = 0x196;
}

/// Firmware self-actions bound via [0x0A, action, 0, 0]. 14 = wheel/volume
/// swap is the only known knob-mode control baked into the firmware.
pub mod firmware_action {
    pub const OFFICE_GAMING: u8 = 6;
    pub const KEYBOARD_LOCK: u8 = 7;
    pub const CHECK_BATTERY: u8 = 8;
    pub const LED_ON_OFF: u8 = 9;
    pub const WASD_CHANGE: u8 = 10;
    pub const FN_MATRIX_CHANGE: u8 = 11;
    pub const POWER_SAVES: u8 = 12;
    pub const FN_LOCK: u8 = 13;
    pub const WHEEL_SWAP: u8 = 14;
    pub const CAPS_SWAP: u8 = 15;
    pub const SLEEP_TOGGLE: u8 = 16;
    pub const CAPS_LED_SWAP: u8 = 17;
    pub const POWER_DOWN: u8 = 18;
    pub const FN_KEY_SWAP: u8 = 19;
    pub const ALT_TAB: u8 = 20;
    pub const LANGUAGE_SWITCH: u8 = 21;
}

/// HID modifier bit positions (byte 1 of HID slot entries).
pub mod mods {
    pub const LCTRL: u8 = 0x01;
    pub const LSHIFT: u8 = 0x02;
    pub const LALT: u8 = 0x04;
    pub const LGUI: u8 = 0x08;
    pub const RCTRL: u8 = 0x10;
    pub const RSHIFT: u8 = 0x20;
    pub const RALT: u8 = 0x40;
    pub const RGUI: u8 = 0x80;
}
