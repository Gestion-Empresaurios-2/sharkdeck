//! Physical K86 layout slot map (from dniminenn/sharkfin's evidence, which
//! anchors the vendor app's layout table to the factory firmware image).

use std::sync::OnceLock;

#[derive(serde::Deserialize, Clone, Debug, serde::Serialize)]
pub struct KeyDef {
    /// keymap slot index (0..127)
    pub slot: u8,
    /// browser-style key name (Escape, Digit1, KeyQ, Backquote, …)
    pub code: String,
    /// display label from the vendor layout ("Esc", "1!", "Q"), may be empty
    pub text: Option<String>,
    /// pixel geometry on the vendor canvas (origin normalized to 0,0)
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// factory default binding in the factory flash image
    pub entry: [u8; 4],
}

#[derive(serde::Deserialize, Clone, Debug, serde::Serialize)]
pub struct SlotOnly {
    #[serde(rename = "matrixIndex")]
    pub matrix_index: u8,
    pub entry: [u8; 4],
}

#[derive(serde::Deserialize, Clone, Debug, serde::Serialize)]
pub struct Canvas {
    pub width: f32,
    pub height: f32,
}

#[derive(serde::Deserialize, Clone, Debug, serde::Serialize)]
pub struct Layout {
    pub canvas: Canvas,
    pub keys: Vec<KeyDef>,
    pub unused: Vec<SlotOnly>,
}

const RAW: &str = include_str!("../data/k86_layout.json");

pub fn k86_layout() -> &'static Layout {
    static LAYOUT: OnceLock<Layout> = OnceLock::new();
    LAYOUT.get_or_init(|| serde_json::from_str(RAW).expect("embedded k86_layout.json is valid"))
}

/// Slots the firmware reserves for the rotary knob on the K86 (page 6 head).
/// Note: mapping of press/turn to these slots varies between firmware builds;
/// the app verifies by read-back and never assumes a specific position.
pub const KNOB_CANDIDATE_SLOTS: &[u8] = &[96, 97, 98, 99, 102, 103];

/// Human labels for HID usages 0x04..0x65 (auto-generated compact table).
pub fn usage_label(usage: u8) -> String {
    match usage {
        0x04..=0x1D => format!("{}", (b'A' + usage - 0x04) as char),
        0x1E..=0x26 => format!("{}", (b'1' + usage - 0x1E) as char),
        0x27 => "0".into(),
        0x28 => "Enter".into(),
        0x29 => "Esc".into(),
        0x2A => "Bksp".into(),
        0x2B => "Tab".into(),
        0x2C => "Space".into(),
        0x39 => "Caps".into(),
        0x2D..=0x38 => (r"-=[]\#;',`/.".as_bytes()[(usage - 0x2D) as usize] as char).to_string(),
        0x3A..=0x45 => format!("F{}", usage - 0x3A + 1),
        0x4F => "→".into(),
        0x50 => "←".into(),
        0x51 => "↓".into(),
        0x52 => "↑".into(),
        0x58 => "Enter".into(),
        0x64 => r"\+".into(),
        // extended set (225+): modifiers / reserved / pad
        0xE0 => "LCtrl".into(),
        0xE1 => "LShift".into(),
        0xE2 => "LAlt".into(),
        0xE3 => "LGui".into(),
        0xE4 => "RCtrl".into(),
        0xE5 => "RShift".into(),
        0xE6 => "RAlt".into(),
        0xE7 => "RGui".into(),
        _ => format!("Usage {:#04x}", usage),
    }
}

/// Notable consumer usages → display names.
pub fn consumer_label(code: u16) -> String {
    use crate::protocol::consumer::*;
    match code {
        VOL_UP => "Volume Up".into(),
        VOL_DOWN => "Volume Down".into(),
        MUTE => "Mute".into(),
        PLAY_PAUSE => "Play/Pause".into(),
        NEXT => "Next Track".into(),
        PREV => "Previous Track".into(),
        STOP => "Stop".into(),
        BRIGHTNESS_UP => "Brightness Up".into(),
        BRIGHTNESS_DOWN => "Brightness Down".into(),
        SEARCH => "Search".into(),
        CALCULATOR => "Calculator".into(),
        BROWSER => "Browser".into(),
        0x0157 => "Compass?".into(),
        _ => format!("Consumer {:#06x}", code),
    }
}

pub fn firmware_label(action: u8) -> String {
    use crate::protocol::firmware_action::*;
    match action {
        OFFICE_GAMING => "Office/Gaming".into(),
        KEYBOARD_LOCK => "Keyboard Lock".into(),
        CHECK_BATTERY => "Check Battery".into(),
        LED_ON_OFF => "LED On/Off".into(),
        WASD_CHANGE => "WASD Swap".into(),
        FN_MATRIX_CHANGE => "Fn Matrix".into(),
        POWER_SAVES => "Power Save Toggle".into(),
        FN_LOCK => "Fn Lock".into(),
        WHEEL_SWAP => "Wheel Swap (Vol↔Bright)".into(),
        CAPS_SWAP => "Caps Swap".into(),
        SLEEP_TOGGLE => "Sleep Toggle".into(),
        CAPS_LED_SWAP => "Caps/LED Swap".into(),
        POWER_DOWN => "Power Down".into(),
        FN_KEY_SWAP => "Fn Key Swap".into(),
        ALT_TAB => "Alt-Tab".into(),
        LANGUAGE_SWITCH => "Language Switch".into(),
        _ => format!("Firmware Action {}", action),
    }
}
