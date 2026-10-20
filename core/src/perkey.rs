//! Per-key RGB support for the Attack Shark K86 (`SET_USERPIC`, opcode 0x0C).
//!
//! WHAT IS KNOWN
//! -------------
//! * The K86 firmware exposes `SET_USERPIC` = 0x0C. It stores a per-key
//!   colour map ("user picture") in the keyboard's flash next to the regular
//!   lighting configuration.
//! * It is a FLASH write. The endpoint stalls when an upload is interrupted
//!   or repeated too quickly: the firmware tolerates **at most 2 uploads per
//!   10 seconds**, and a stalled endpoint only recovers when the keyboard is
//!   unplugged and replugged.
//! * Its checksum is the family's standard bit7 checksum
//!   (`0xFF - sum(payload[0..6])` stored in `payload[7]`), like the other
//!   SET commands (0x05 profile, 0x06 options, 0x07/0x08 lighting).
//!
//! WHAT IS ASSUMED (best effort — the blob layout is not publicly documented)
//! ----------------------------------------------------------------------------
//! The exact wire format of the image blob is unknown, so this module
//! implements ONE layout and documents it in full. It is a **per-key 4-byte
//! entry list** (NOT an RGB565 raster):
//!
//! ```text
//! image blob := entry[0 .. key_count]
//! entry      := slot:u8, r:u8, g:u8, b:u8        (4 bytes per key)
//! ```
//!
//! * `slot` is the keymap slot index of the key (0..127) as published in
//!   `k86_layout.json`, so the firmware can address the key by identity
//!   instead of by position.
//! * `r`, `g`, `b` are the raw 8-bit colour of that key.
//! * Entries are emitted in grid order (row by row, left to right) so the
//!   stream is deterministic and reproducible.
//!
//! The blob is streamed in 56-byte chunks inside the usual 64-byte
//! feature-report payload (65 bytes on the wire, report id 0 first):
//!
//! ```text
//! p[0]      = 0x0C (SET_USERPIC)
//! p[1]      = chunk index (0-based)
//! p[2]      = chunk count
//! p[3]      = bytes carried by this chunk (<= 56)
//! p[4..5]   = total blob length, u16 LE
//! p[6]      = 0 (reserved)
//! p[7]      = bit7 checksum = 0xFF - sum(p[0..6])
//! p[8..64]  = chunk data
//! ```
//!
//! Chunks are sent back to back with a 10 ms gap so the flash controller can
//! catch up, mirroring the screen-stream (0x25) pacing.
//!
//! SAFETY
//! ------
//! Nothing here writes to the keyboard unless the caller explicitly asks for
//! it. `Keyboard::set_userpic` is the only function that touches the device
//! and it is guarded by the process-wide `UploadLimiter` (2 uploads / 10 s).
//! Previewing an image (`encode`) is pure computation and never opens HID.

use crate::layout::{k86_layout, Layout};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `SET_USERPIC` — per-key colour image upload (flash write).
pub const OP_SET_USERPIC: u8 = 0x0C;

/// Bytes of image data carried by one feature report.
pub const USERPIC_CHUNK: usize = 56;

/// Flash uploads allowed per `UPLOAD_WINDOW`.
pub const UPLOAD_MAX: usize = 2;

/// Rate-limit window for flash uploads.
pub const UPLOAD_WINDOW: Duration = Duration::from_secs(10);

/// Colour of a single key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeyColour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl KeyColour {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        KeyColour { r, g, b }
    }

    /// Key switched off.
    pub fn black() -> Self {
        KeyColour::new(0, 0, 0)
    }

    /// RGB565 packing (same bit layout as the display panel).
    pub fn to_rgb565(self) -> u16 {
        (((self.r as u16) >> 3) << 11) | (((self.g as u16) >> 2) << 5) | ((self.b as u16) >> 3)
    }

    /// `#RRGGBB` (upper case).
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// A per-key colour image: one `KeyColour` per grid cell, row-major.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PerKeyImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<KeyColour>,
}

impl PerKeyImage {
    /// Black image of `width` x `height` cells.
    pub fn new(width: u32, height: u32) -> Self {
        let n = (width as usize) * (height as usize);
        PerKeyImage {
            width,
            height,
            pixels: vec![KeyColour::black(); n],
        }
    }

    /// Colour of one cell, `None` when out of bounds.
    pub fn get(&self, x: u32, y: u32) -> Option<KeyColour> {
        self.index(x, y).map(|i| self.pixels[i])
    }

    /// Paint one cell. `Err` when the cell is outside the image.
    pub fn set(&mut self, x: u32, y: u32, colour: KeyColour) -> Result<(), String> {
        let i = self.index(x, y).ok_or_else(|| {
            format!(
                "cell ({x},{y}) outside {}x{} image",
                self.width, self.height
            )
        })?;
        self.pixels[i] = colour;
        Ok(())
    }

    /// Flat index of a cell, `None` when out of bounds.
    pub fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    /// Number of cells (including the ones no key occupies).
    pub fn cell_count(&self) -> usize {
        self.pixels.len()
    }

    /// Grid image of a physical layout: keys with the same `y` share a row,
    /// `x` gives the column. Cells without a key stay black.
    pub fn from_layout(layout: &Layout) -> PerKeyImage {
        let map = grid_map(layout);
        PerKeyImage::new(map.width, map.height)
    }
}

/// Grid projection of a physical layout: which key sits in which cell.
#[derive(Clone, Debug)]
pub struct GridMap {
    pub width: u32,
    pub height: u32,
    /// Cell index -> index into `Layout::keys` (`None` = empty cell).
    pub cells: Vec<Option<usize>>,
}

impl GridMap {
    /// Grid cell of a key, `None` when the key is not on the layout.
    pub fn cell_of_key(&self, key_index: usize) -> Option<(u32, u32)> {
        let cell = self.cells.iter().position(|c| *c == Some(key_index))?;
        Some((cell as u32 % self.width, cell as u32 / self.width))
    }

    /// Key in a cell, `None` for empty cells.
    pub fn key_of_cell(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells[y as usize * self.width as usize + x as usize]
    }

    /// Flat index of a cell.
    pub fn cell_index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    /// Number of cells that actually carry a key.
    pub fn occupied(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }
}

/// Project a physical layout onto a grid: rows are the distinct `y` values,
/// columns the distinct `x` values (both sorted ascending).
pub fn grid_map(layout: &Layout) -> GridMap {
    let mut ys: Vec<i64> = layout.keys.iter().map(|k| k.y as i64).collect();
    ys.sort_unstable();
    ys.dedup();
    let mut xs: Vec<i64> = layout.keys.iter().map(|k| k.x as i64).collect();
    xs.sort_unstable();
    xs.dedup();

    let width = xs.len().max(1) as u32;
    let height = ys.len().max(1) as u32;
    let mut cells: Vec<Option<usize>> = vec![None; (width * height) as usize];

    for (i, key) in layout.keys.iter().enumerate() {
        let Some(col) = xs.iter().position(|x| *x == key.x as i64) else {
            continue;
        };
        let Some(row) = ys.iter().position(|y| *y == key.y as i64) else {
            continue;
        };
        let idx = row * width as usize + col;
        // First key wins: the layout has no duplicate cells, but be defensive.
        if cells[idx].is_none() {
            cells[idx] = Some(i);
        }
    }

    GridMap {
        width,
        height,
        cells,
    }
}

/// Encode a per-key image for `SET_USERPIC` using the embedded K86 layout.
///
/// See the module header for the assumed wire format. The blob holds one
/// 4-byte entry (`slot, r, g, b`) per key, in grid order.
pub fn encode(image: &PerKeyImage) -> Vec<u8> {
    encode_with(image, k86_layout())
}

/// Encode a per-key image for a specific layout (same blob format as
/// [`encode`]). Entries are emitted in grid order: row by row, left to right.
pub fn encode_with(image: &PerKeyImage, layout: &Layout) -> Vec<u8> {
    let map = grid_map(layout);
    let mut out = Vec::with_capacity(map.occupied() * 4);
    for row in 0..map.height {
        for col in 0..map.width {
            let Some(key_index) = map.key_of_cell(col, row) else {
                continue;
            };
            let colour = image.get(col, row).unwrap_or_default();
            out.push(layout.keys[key_index].slot);
            out.push(colour.r);
            out.push(colour.g);
            out.push(colour.b);
        }
    }
    out
}

/// Snap every cell of the image to the closest palette entry (the firmware
/// stores a small fixed palette; this keeps the picture representable).
/// An empty palette leaves the image untouched.
pub fn apply_palette(image: &mut PerKeyImage, palette: &[KeyColour]) {
    if palette.is_empty() {
        return;
    }
    for px in image.pixels.iter_mut() {
        *px = palette[palette_slot(palette, *px)];
    }
}

/// Index of the palette entry closest to `colour` (weighted RGB distance).
pub fn palette_slot(palette: &[KeyColour], colour: KeyColour) -> usize {
    let mut best = 0usize;
    let mut best_d = u32::MAX;
    for (i, p) in palette.iter().enumerate() {
        let dr = p.r as i32 - colour.r as i32;
        let dg = p.g as i32 - colour.g as i32;
        let db = p.b as i32 - colour.b as i32;
        let d = (2 * dr * dr + 4 * dg * dg + 3 * db * db) as u32;
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

/// Sliding-window rate limiter for flash uploads: at most `max` uploads per
/// `window`. The oldest stamp decides when the next slot frees up.
#[derive(Debug)]
pub struct UploadLimiter {
    max: usize,
    window: Duration,
    stamps: Vec<Instant>,
}

impl UploadLimiter {
    pub fn new(max: usize, window: Duration) -> Self {
        UploadLimiter {
            max,
            window,
            stamps: Vec::new(),
        }
    }

    /// Reserve one upload slot. `Ok(())` when the write may proceed, `Err`
    /// with a human-readable reason when the limiter refuses.
    pub fn allow(&mut self, now: Instant) -> Result<(), String> {
        self.prune(now);
        if self.stamps.len() >= self.max {
            let wait = self.retry_after(now);
            return Err(format!(
                "flash upload rate limit: {} uploads per {} s exhausted — retry in {:.1} s (unplug/replug the keyboard if it stopped answering)",
                self.max,
                self.window.as_secs(),
                wait.as_secs_f64()
            ));
        }
        self.stamps.push(now);
        Ok(())
    }

    /// Uploads still available in the current window.
    pub fn remaining(&self, now: Instant) -> usize {
        self.max.saturating_sub(self.live(now))
    }

    /// Time until the next upload is allowed (`Duration::ZERO` when one is
    /// available right now).
    pub fn retry_after(&self, now: Instant) -> Duration {
        let live = self.live(now);
        if live < self.max {
            return Duration::ZERO;
        }
        let oldest = self
            .stamps
            .iter()
            .copied()
            .filter(|t| now.checked_duration_since(*t).unwrap_or_default() < self.window)
            .min()
            .unwrap_or(now);
        oldest + self.window - now
    }

    /// Uploads allowed per window.
    pub fn max(&self) -> usize {
        self.max
    }

    /// Length of the rate-limit window.
    pub fn window(&self) -> Duration {
        self.window
    }

    fn live(&self, now: Instant) -> usize {
        self.stamps
            .iter()
            .filter(|t| now.checked_duration_since(**t).unwrap_or_default() < self.window)
            .count()
    }

    fn prune(&mut self, now: Instant) {
        self.stamps
            .retain(|t| now.checked_duration_since(*t).unwrap_or_default() < self.window);
    }
}

/// Snapshot of the flash-upload limiter for the UI.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct UserpicStatus {
    /// An upload may be sent right now.
    pub allowed: bool,
    /// Uploads left in the current window.
    pub remaining: usize,
    /// Uploads allowed per window.
    pub max: usize,
    /// Window length in seconds.
    pub window_s: u32,
    /// Seconds until the next upload is allowed (0 when it is allowed now).
    pub next_allowed_in_s: f64,
}

/// Process-wide flash upload limiter (shared by every keyboard handle).
pub fn upload_limiter() -> &'static Mutex<UploadLimiter> {
    static LIMITER: OnceLock<Mutex<UploadLimiter>> = OnceLock::new();
    LIMITER.get_or_init(|| Mutex::new(UploadLimiter::new(UPLOAD_MAX, UPLOAD_WINDOW)))
}

/// Current state of the flash upload limiter.
pub fn userpic_status() -> UserpicStatus {
    let now = Instant::now();
    let limiter = upload_limiter().lock().unwrap();
    let remaining = limiter.remaining(now);
    UserpicStatus {
        allowed: remaining > 0,
        remaining,
        max: limiter.max(),
        window_s: limiter.window().as_secs() as u32,
        next_allowed_in_s: limiter.retry_after(now).as_secs_f64(),
    }
}
