//! Linux evdev reader for the K86's **native** knob.
//!
//! The keyboard's firmware exposes the knob as a relative-axis mouse-like
//! HID device: the kernel creates a "ROYUAN Gaming Keyboard Mouse" node with
//! REL_X/REL_Y/REL_Z/REL_RY/REL_DIAL/REL_WHEEL axes plus buttons. Reading that
//! node gives us the knob's real, native behaviour (which the vendor app
//! maps to volume) and lets the desktop app react to it.
//!
//! Reading /dev/input/event* requires membership of the `input` group:
//! `sudo gpasswd -a <user> input` (then log out/in or replug).

use std::fs;
use std::io::Read;

/// One kernel input device node.
#[derive(Clone, Debug, serde::Serialize)]
pub struct InputDevice {
    /// /dev/input/eventN
    pub path: String,
    /// Human name from sysfs.
    pub name: String,
    /// True when the node reports relative axes (the knob).
    pub has_rel: bool,
    /// True when the node reports key/button events.
    pub has_keys: bool,
}

/// One decoded input event.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct RawEvent {
    /// 1 = EV_KEY, 2 = EV_REL, 3 = EV_ABS, …
    pub kind: u16,
    /// Axis / button code.
    pub code: u16,
    /// Value (delta for axes, 0/1 for buttons).
    pub value: i32,
}

/// True when the device name looks like the K86's knob node.
pub fn is_knob_device(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.contains("royuan") || n.contains("attack shark")) && n.contains("mouse")
}

/// List the kernel input nodes, richest metadata first.
pub fn list_devices() -> Vec<InputDevice> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/dev/input") else {
        return out;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("event") {
            continue;
        }
        let path = format!("/dev/input/{name}");
        let (dev_name, has_rel, has_keys) = read_sysfs(&name);
        out.push(InputDevice {
            path,
            name: dev_name,
            has_rel,
            has_keys,
        });
    }
    out.sort_by(|a, b| {
        (is_knob_device(&b.name), &b.name)
            .cmp(&(is_knob_device(&a.name), &a.name))
    });
    out
}

/// Read the device name + capabilities of /dev/input/eventN from sysfs.
fn read_sysfs(event: &str) -> (String, bool, bool) {
    let base = format!("/sys/class/input/{event}/device");
    let name = fs::read_to_string(format!("{base}/name"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let cap = |kind: &str| -> bool {
        fs::read_to_string(format!("{base}/capabilities/{kind}"))
            .map(|s| s.trim() != "0" && !s.trim().is_empty())
            .unwrap_or(false)
    };
    (name, cap("rel"), cap("key"))
}

/// Read raw events from a node for `seconds`.
///
/// The `input_event` struct on 64-bit Linux is 24 bytes:
/// `timeval time` (16 B) + `u16 type` + `u16 code` + `i32 value`.
pub fn read_events(path: &str, seconds: u64) -> Result<Vec<RawEvent>, String> {
    let mut f = fs::File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!("permission denied opening {path} — add your user to the 'input' group: sudo gpasswd -a $USER input (then log out/in)")
        } else {
            format!("open {path}: {e}")
        }
    })?;
    let start = std::time::Instant::now();
    let mut buf = [0u8; 24 * 64];
    let mut out = Vec::new();
    while start.elapsed().as_secs() < seconds {
        let n = match f.read(&mut buf) {
            Ok(n) => n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(format!("read {path}: {e}")),
        };
        if n == 0 {
            break; // device removed
        }
        for chunk in buf[..n].chunks_exact(24) {
            let kind = u16::from_le_bytes([chunk[16], chunk[17]]);
            let code = u16::from_le_bytes([chunk[18], chunk[19]]);
            let value = i32::from_le_bytes([chunk[20], chunk[21], chunk[22], chunk[23]]);
            // The kernel appends a 4-byte padding word in some layouts; the
            // 24-byte stride above matches the 64-bit ABI used by Linux.
            out.push(RawEvent { kind, code, value });
        }
    }
    Ok(out)
}

/// Human label for a relative-axis code.
pub fn rel_label(code: u16) -> &'static str {
    match code {
        0 => "REL_X",
        1 => "REL_Y",
        2 => "REL_Z",
        3 => "REL_RX",
        4 => "REL_RY",
        5 => "REL_RZ",
        6 => "REL_HWHEEL",
        7 => "REL_DIAL",
        8 => "REL_WHEEL",
        _ => "REL_?",
    }
}

/// Human label for a button code.
pub fn key_label(code: u16) -> &'static str {
    match code {
        0x110 => "BTN_LEFT",
        0x111 => "BTN_RIGHT",
        0x112 => "BTN_MIDDLE",
        0x113 => "BTN_SIDE",
        0x114 => "BTN_EXTRA",
        0x115 => "BTN_FORWARD",
        0x116 => "BTN_BACK",
        0x117 => "BTN_TASK",
        _ => "BTN_?",
    }
}

/// Summarise knob activity: turns per axis direction and button presses.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct KnobSummary {
    pub left: i32,
    pub right: i32,
    pub up: i32,
    pub down: i32,
    pub presses: i32,
    pub wheel: i32,
    pub axes: Vec<String>,
}

/// Fold raw events into a knob summary.
pub fn summarise(events: &[RawEvent]) -> KnobSummary {
    let mut s = KnobSummary::default();
    for e in events {
        match (e.kind, e.code) {
            (1, code) => {
                if e.value == 1 {
                    s.presses += 1;
                }
                let l = key_label(code);
                if !s.axes.iter().any(|a| a == l) {
                    s.axes.push(l.to_string());
                }
            }
            (2, 8) => s.wheel += e.value,
            (2, 6) => s.wheel -= e.value,
            (2, 0) => {
                if e.value < 0 {
                    s.left += -e.value;
                } else {
                    s.right += e.value;
                }
            }
            (2, 1) => {
                if e.value < 0 {
                    s.up += -e.value;
                } else {
                    s.down += e.value;
                }
            }
            (2, 2) => {
                if e.value < 0 {
                    s.left += -e.value;
                } else {
                    s.right += e.value;
                }
            }
            _ => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_device_detection() {
        assert!(is_knob_device("ROYUAN Gaming Keyboard Mouse"));
        assert!(!is_knob_device("ROYUAN Gaming Keyboard"));
    }

    #[test]
    fn summarise_counts() {
        let ev = vec![
            RawEvent { kind: 2, code: 0, value: -3 },
            RawEvent { kind: 2, code: 0, value: 5 },
            RawEvent { kind: 2, code: 8, value: 1 },
            RawEvent { kind: 1, code: 0x110, value: 1 },
        ];
        let s = summarise(&ev);
        assert_eq!(s.left, 3);
        assert_eq!(s.right, 5);
        assert_eq!(s.wheel, 1);
        assert_eq!(s.presses, 1);
    }
}
