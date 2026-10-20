//! Linux evdev reader for the K86's **native** knob.
//!
//! Measured on a K86 (fw 1.19): the knob is **not** a mouse. The firmware
//! reports it as consumer-control *key* events on the keyboard's "Consumer
//! Control" node: `KEY_VOLUMEUP` (115) for one direction, `KEY_VOLUMEDOWN`
//! (114) for the other and `KEY_MUTE` (113) for the press — exactly the
//! factory behaviour (turn-left = volume down, turn-right = volume up,
//! press = mute, matching the on-device keymap slots 96/97/78).
//! Other ROYUAN nodes (keyboard bitmap, system control, mouse) are also
//! captured, but only the consumer codes above are classified as knob input,
//! so ordinary typing is never mistaken for a knob turn.
//!
//! Reading /dev/input/event* requires membership of the `input` group:
//! `sudo gpasswd -a <user> input` (then log out/in or replug).

use std::fs;
use std::io::Read;
use std::time::{Duration, Instant};

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

/// True when the device name belongs to the K86 (any of its HID nodes).
///
/// The knob shows up on the *Consumer Control* node; the keyboard, system
/// control and mouse nodes of the same device are captured too, but only
/// consumer codes are classified as knob input.
pub fn is_knob_device(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("royuan") || n.contains("attack shark")
}

/// Linux key code the firmware sends for a knob turn towards "volume up".
pub const KNOB_TURN_UP: u16 = 115;
/// Linux key code for a knob turn towards "volume down".
pub const KNOB_TURN_DOWN: u16 = 114;
/// Linux key code for a knob press (mute).
pub const KNOB_PRESS: u16 = 113;

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
/// Open an input node in non-blocking mode (so a silent device cannot wedge
/// the reader).
fn open_nonblocking(path: &str) -> Result<fs::File, String> {
    let f = fs::File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "permission denied opening {path} — add your user to the 'input' group: sudo gpasswd -a $USER input (then log out/in)"
            )
        } else {
            format!("open {path}: {e}")
        }
    })?;
    #[cfg(unix)]
    unsafe {
        use std::os::unix::io::AsRawFd;
        let fd = f.as_raw_fd();
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags >= 0 {
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
    Ok(f)
}

/// Read raw events from a node for `seconds` (non-blocking, returns early).
///
/// The `input_event` struct on 64-bit Linux is 24 bytes:
/// `timeval time` (16 B) + `u16 type` + `u16 code` + `i32 value`.
pub fn read_events(path: &str, seconds: u64) -> Result<Vec<RawEvent>, String> {
    let mut f = open_nonblocking(path)?;
    let start = Instant::now();
    let mut buf = [0u8; 24 * 64];
    let mut out = Vec::new();
    while start.elapsed().as_secs() < seconds {
        let n = match f.read(&mut buf) {
            Ok(n) => n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => return Err(format!("read {path}: {e}")),
        };
        if n == 0 {
            break; // device removed
        }
        for chunk in buf[..n].chunks_exact(24) {
            let kind = u16::from_le_bytes([chunk[16], chunk[17]]);
            let code = u16::from_le_bytes([chunk[18], chunk[19]]);
            let value = i32::from_le_bytes([chunk[20], chunk[21], chunk[22], chunk[23]]);
            out.push(RawEvent { kind, code, value });
        }
    }
    Ok(out)
}

/// Read from several nodes at the same time, tagging every event with the
/// node it came from. Nodes that cannot be opened are skipped silently.
pub fn read_events_multi(paths: &[String], seconds: u64) -> Vec<(String, RawEvent)> {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<(String, RawEvent)>();
    let out = std::thread::scope(|scope| {
        for p in paths {
            let tx = tx.clone();
            let p = p.clone();
            scope.spawn(move || {
                if let Ok(evs) = read_events(&p, seconds) {
                    for e in evs {
                        let _ = tx.send((p.clone(), e));
                    }
                }
            });
        }
        drop(tx);
        let mut out = Vec::new();
        for (p, e) in rx {
            out.push((p, e));
        }
        out
    });
    out
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
        113 => "MUTE (knob press)",
        114 => "VOLUME DOWN (knob turn)",
        115 => "VOLUME UP (knob turn)",
        _ => "KEY_?",
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
            (1, KNOB_TURN_UP) => {
                if e.value == 1 {
                    s.right += 1; // turn towards volume up
                }
            }
            (1, KNOB_TURN_DOWN) => {
                if e.value == 1 {
                    s.left += 1; // turn towards volume down
                }
            }
            (1, KNOB_PRESS) => {
                if e.value == 1 {
                    s.presses += 1;
                }
            }
            (1, code) => {
                // Mouse buttons (0x110..0x117) are the only generic presses;
                // ordinary typing is ignored so it is never counted as a knob.
                if e.value == 1 && code >= 0x110 && code <= 0x117 {
                    s.presses += 1;
                }
            }
            (2, code) => {
                let l = rel_label(code);
                if !s.axes.iter().any(|a| a == l) {
                    s.axes.push(l.to_string());
                }
                match code {
                    8 => s.wheel += e.value,
                    6 => s.wheel -= e.value,
                    0 => {
                        if e.value < 0 {
                            s.left += -e.value;
                        } else {
                            s.right += e.value;
                        }
                    }
                    1 => {
                        if e.value < 0 {
                            s.up += -e.value;
                        } else {
                            s.down += e.value;
                        }
                    }
                    2 => {
                        if e.value < 0 {
                            s.left += -e.value;
                        } else {
                            s.right += e.value;
                        }
                    }
                    _ => {}
                }
            }
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
        assert!(is_knob_device("ROYUAN Gaming Keyboard Consumer Control"));
        assert!(is_knob_device("ROYUAN Gaming Keyboard"));
        assert!(!is_knob_device("SEMICCHIP USB Mouse"));
    }

    #[test]
    fn consumer_codes_map_to_knob() {
        let ev = vec![
            RawEvent { kind: 1, code: KNOB_TURN_UP, value: 1 },
            RawEvent { kind: 1, code: KNOB_TURN_UP, value: 0 },
            RawEvent { kind: 1, code: KNOB_TURN_DOWN, value: 1 },
            RawEvent { kind: 1, code: KNOB_PRESS, value: 1 },
            RawEvent { kind: 1, code: 30, value: 1 }, // a normal keypress
        ];
        let s = summarise(&ev);
        assert_eq!(s.right, 1);
        assert_eq!(s.left, 1);
        assert_eq!(s.presses, 1);
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

// ---------------------------------------------------------------------------
// Raw vendor-HID report decoding
// ---------------------------------------------------------------------------

/// Consumer-page usage names seen on this keyboard family.
pub fn consumer_name(code: u16) -> &'static str {
    match code {
        0xE9 => "Volume Up",
        0xEA => "Volume Down",
        0xE2 => "Mute",
        0xCD => "Play/Pause",
        0xB5 => "Next Track",
        0xB6 => "Previous Track",
        0xB7 => "Stop",
        0x6F => "Brightness Up",
        0x70 => "Brightness Down",
        0x221 => "Search",
        0x192 => "Calculator",
        0x196 => "Browser",
        0xE0 => "Volume Mute",
        _ => "consumer",
    }
}

/// Describe one raw HID report from the vendor interface.
///
/// The K86's interface 1 exposes several INPUT report ids (1 = keyboard
/// bitmap, 2 = system control, 3 = 16-bit consumer, 5 = vendor 0xFFFF/1 with
/// three 8-bit bytes, 6 = generic/consumer). The knob's axis data is expected
/// in the vendor report, so the bytes are also shown as signed deltas.
pub fn describe_hid_report(b: &[u8]) -> String {
    if b.is_empty() {
        return "(empty report)".into();
    }
    let id = b[0];
    let rest = &b[1..];
    let mut parts: Vec<String> = Vec::new();
    match id {
        1 => parts.push("keyboard bitmap".into()),
        2 => parts.push("system control".into()),
        3 => {
            let code = if rest.len() >= 2 {
                u16::from_le_bytes([rest[0], rest[1]])
            } else {
                0
            };
            parts.push(format!("consumer {code:#06x} ({})", consumer_name(code)));
        }
        5 => {
            parts.push("vendor 0xFFFF/1".into());
            let signed: Vec<i8> = rest.iter().map(|v| *v as i8).collect();
            parts.push(format!("bytes {signed:?}"));
        }
        6 => {
            parts.push("generic/consumer".into());
            if rest.len() >= 3 {
                let code = u16::from_le_bytes([rest[1], rest[2]]);
                if code != 0 {
                    parts.push(format!("consumer {code:#06x} ({})", consumer_name(code)));
                }
            }
        }
        other => parts.push(format!("report id {other}")),
    }
    parts.join(" · ")
}
