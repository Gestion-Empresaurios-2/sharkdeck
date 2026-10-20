//! HID transport discovery + open logic for the K86's vendor interface.
//! Control collection: interface 2, usage page 0xFFFF usage 0x0002.

use hidapi::{HidApi, HidDevice};
use std::sync::{Arc, Mutex};

pub const VID: u16 = 0x3151;
pub const PID: u16 = 0x4015;

/// One candidate HID path with whatever metadata hidapi could read.
#[derive(Clone, Debug, serde::Serialize)]
pub struct HidCandidate {
    pub path: String,
    pub interface: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub serial: String,
    pub product: String,
}

fn scan(api: &HidApi) -> Vec<HidCandidate> {
    let mut out = Vec::new();
    for info in api.device_list() {
        if info.vendor_id() != VID || info.product_id() != PID {
            continue;
        }
        // hidapi's Linux hidraw backend reports usage from the report descriptor
        // in modern versions; older builds leave zeros — don't filter on it.
        out.push(HidCandidate {
            path: info.path().to_string_lossy().into_owned(),
            interface: info.interface_number(),
            usage_page: info.usage_page(),
            usage: info.usage(),
            serial: info
                .serial_number()
                .map(str::to_string)
                .unwrap_or_default(),
            product: info
                .product_string()
                .map(str::to_string)
                .unwrap_or_default(),
        });
    }
    // Control interface first, then numeric order.
    out.sort_by_key(|c| {
        let ctrl = (c.usage_page == 0xFF00 || c.usage_page == 0xFFFF) && c.usage == 0x0002;
        (if ctrl { 0 } else { 1 }, c.interface, c.path.clone())
    });
    out
}

/// Read raw INPUT reports from a hidraw path for `seconds`, returning
/// `(elapsed_ms, bytes)` pairs. Used to explore the device's native event
/// channels (the K86's interface 1 exposes a 3-byte vendor INPUT report).
/// Trailing zero bytes are trimmed so the output stays readable.
pub fn read_events(path: &str, seconds: u64) -> Result<Vec<(u64, Vec<u8>)>, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let dev = api
        .open_path(std::ffi::CString::new(path).unwrap().as_c_str())
        .map_err(|e| format!("open {path}: {e}"))?;
    let mut out = Vec::new();
    let start = std::time::Instant::now();
    let mut buf = [0u8; 64];
    while start.elapsed().as_secs() < seconds {
        match dev.read_timeout(&mut buf, 200) {
            Ok(n) => {
                if n == 0 {
                    continue; // timeout tick
                }
                let mut v = buf[..n].to_vec();
                while v.len() > 1 && v[v.len() - 1] == 0 {
                    v.pop();
                }
                out.push((start.elapsed().as_millis() as u64, v));
            }
            Err(_) => continue, // timeout: keep listening
        }
    }
    Ok(out)
}

/// Refresh-and-scan: create a fresh HidApi so the OS re-enumerates after
/// plug/unplug.
pub fn find_candidates() -> Result<Vec<HidCandidate>, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    Ok(scan(&api))
}

/// Shared HidApi guarded by a mutex. `HidDevice` is lifetime-bound to its
/// `HidApi`, so devices are opened per-operation under the same lock; the
/// device is dropped at the end of every operation, which also dodges the
/// firmware's cached-read bug after writes (close/reopen == fresh reads).
pub struct ApiBox {
    pub inner: Mutex<HidApi>,
}

unsafe impl Send for ApiBox {}
unsafe impl Sync for ApiBox {}

impl ApiBox {
    pub fn new() -> Result<ApiBox, String> {
        let api = HidApi::new().map_err(|e| e.to_string())?;
        Ok(ApiBox {
            inner: Mutex::new(api),
        })
    }

    pub fn with<T>(&self, path: &str, f: impl FnOnce(&HidDevice) -> Result<T, String>) -> Result<T, String> {
        let api = self.inner.lock().map_err(|e| e.to_string())?;
        let dev = api
            .open_path(std::ffi::CString::new(path).unwrap().as_c_str())
            .map_err(|e| format!("open {path}: {e}"))?;
        f(&dev)
    }

    pub fn list(&self) -> Vec<HidCandidate> {
        let api = self.inner.lock().unwrap();
        scan(&api)
    }
}

pub type SharedApi = Arc<ApiBox>;
