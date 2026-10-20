//! RGB565 screen content: decode, scale, pattern generators, tiny-font clock.

use crate::protocol::{encode565, PixelOrder};
use image::imageops::FilterType;
use image::{RgbaImage};

pub const DEFAULT_W: u32 = 128;
pub const DEFAULT_H: u32 = 128;

/// Candidate non-square panel sizes worth testing in the calibration lab
/// (registry lists the K86 as 240x135; community hardware-verified 128x128).
pub const CANDIDATE_SIZES: &[(u32, u32, &str)] = &[
    (128, 128, "square · verified default"),
    (240, 135, "registry value"),
    (240, 128, ""),
    (235, 128, "user-anchored width"),
    (232, 128, ""),
    (240, 140, ""),
    (220, 128, ""),
];

/// A framebuffer in row-major RGBA (4 bytes/px). Frontend can emit these
/// (canvas ImageData) or the backend produces them from images/patterns.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ContentMode {
    /// Crop the long edge; no distortion.
    Fill,
    /// Fit everything inside, pad the short edge.
    Fit,
    /// Stretch, distorting non-square sources.
    Stretch,
}

fn img(rgba: &[u8], w: u32, h: u32) -> Result<RgbaImage, String> {
    RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or("bad rgba buffer".into())
}

/// Scale `src` to the panel with the given content mode (black pad for Fit).
pub fn scale(src: &RgbaFrame, mode: ContentMode, dw: u32, dh: u32) -> RgbaFrame {
    let sw = src.width;
    let sh = src.height;
    if sw == dw && sh == dh {
        return src.clone();
    }
    let source = match img(&src.rgba, sw, sh) {
        Ok(i) => i,
        Err(_) => return src.clone(),
    };
    let out = match mode {
        ContentMode::Stretch => image::imageops::resize(&source, dw, dh, FilterType::Triangle),
        ContentMode::Fit => {
            let scale = (dw as f32 / sw as f32).min(dh as f32 / sh as f32);
            let w = (sw as f32 * scale).round().max(1.0) as u32;
            let h = (sh as f32 * scale).round().max(1.0) as u32;
            let scaled = image::imageops::resize(&source, w, h, FilterType::Triangle);
            let mut canvas = RgbaImage::new(dw, dh);
            image::imageops::overlay(
                &mut canvas,
                &scaled,
                (dw.saturating_sub(w) / 2) as i64,
                (dh.saturating_sub(h) / 2) as i64,
            );
            canvas
        }
        ContentMode::Fill => {
            // Center-crop the source to the target aspect ratio, then resize.
            let target_ratio = dw as f32 / dh as f32;
            let src_ratio = sw as f32 / sh as f32;
            let (cw, ch, cx, cy) = if src_ratio > target_ratio {
                let cw = (sh as f32 * target_ratio).round() as u32;
                (cw.max(1), sh, (sw - cw) / 2, 0)
            } else {
                let ch = (sw as f32 / target_ratio).round() as u32;
                (sw, ch.max(1), 0, (sh - ch) / 2)
            };
            let cropped = image::imageops::crop_imm(&source, cx, cy, cw.min(sw), ch.min(sh)).to_image();
            image::imageops::resize(&cropped, dw, dh, FilterType::Triangle)
        }
    };
    RgbaFrame {
        width: dw,
        height: dh,
        rgba: out.into_raw(),
    }
}

/// Decode an image file into a panel-sized RGBA frame (static content).
pub fn decode_static(path: &str, mode: ContentMode, dw: u32, dh: u32) -> Result<RgbaFrame, String> {
    let img = image::ImageReader::open(path)
        .map_err(|e| format!("open {path}: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("format {path}: {e}"))?
        .decode()
        .map_err(|e| format!("decode {path}: {e}"))?
        .to_rgba8();
    let frame = RgbaFrame {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    };
    Ok(scale(&frame, mode, dw, dh))
}

/// Decode an animated GIF into panel-sized frames with delay bytes
/// (10ms units, clamped 1..255). Non-GIF reach the static path.
pub fn decode_gif(
    path: &str,
    mode: ContentMode,
    dw: u32,
    dh: u32,
    max_frames: usize,
) -> Result<(Vec<(RgbaFrame, u8)>, bool), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut dec = opts.read_info(file).map_err(|e| format!("gif: {e}"))?;
    let (gw, gh) = (dec.width() as u32, dec.height() as u32);
    let mut frames = Vec::new();
    while let Some(frame) = dec.read_next_frame().map_err(|e| format!("gif frame: {e}"))? {
        if frames.len() >= max_frames {
            break;
        }
        let buf = frame.buffer.clone();
        let rgba = RgbaFrame {
            width: gw,
            height: gh,
            rgba: buf.into_owned(),
        };
        let delay_units = (frame.delay as f32).round().clamp(1.0, 255.0) as u8;
        frames.push((scale(&rgba, mode, dw, dh), delay_units));
    }
    let animated = frames.len() > 1;
    if frames.is_empty() {
        return Err("gif has no frames".into());
    }
    Ok((frames, animated))
}

/// RGBA frame + 10ms-units delay, ready for screen_write_animation.
pub fn frames_to_565(frames: &[(RgbaFrame, u8)], order: PixelOrder) -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut bytes = Vec::with_capacity(frames.len());
    let mut delays = Vec::with_capacity(frames.len());
    for (f, d) in frames {
        bytes.push(encode565(&f.rgba, f.width as usize, f.height as usize, 4, order));
        delays.push(*d);
    }
    (bytes, delays)
}

// ---------------------------------------------------------------------------
// Pattern generators (ported from keyboard-studio's Screen.swift)
// ---------------------------------------------------------------------------

type Bgr = (u8, u8, u8);

fn mut_px(rgb: &mut [u8], w: u32, x: u32, y: u32, c: Bgr) {
    let i = ((y * w + x) * 3) as usize;
    rgb[i] = c.0;
    rgb[i + 1] = c.1;
    rgb[i + 2] = c.2;
}

fn blank(w: u32, h: u32, c: Bgr) -> (Vec<u8>, u32) {
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for _ in 0..w * h {
        rgb.push(c.0);
        rgb.push(c.1);
        rgb.push(c.2);
    }
    (rgb, w)
}

/// Vertical stripes + colored corner squares + asymmetric border: one upload
/// answers width (shearing stripes = wrong), rotation (corner colors) and
/// cropping (missing bars/blocks).
pub fn orientation_pattern(w: u32, h: u32) -> RgbaFrame {
    let (mut rgb, w) = blank(w, h, (0, 10, 10));
    for x in (0..w / 16 * 16).step_by(16) {
        for y in 0..h {
            mut_px(&mut rgb, w, x, y, (70, 60, 60));
        }
    }
    let box_ = (w.min(h) / 4).max(8);
    let corners: [(u32, u32, Bgr); 4] = [
        (0, 0, (40, 40, 255)),           // top-left red
        (w - box_, 0, (60, 220, 40)),    // top-right green
        (0, h - box_, (255, 110, 60)),   // bottom-left blue
        (w - box_, h - box_, (40, 210, 255)), // bottom-right yellow
    ];
    for (cx, cy, c) in corners {
        for y in cy..(cy + box_).min(h) {
            for x in cx..(cx + box_).min(w) {
                mut_px(&mut rgb, w, x, y, c);
            }
        }
    }
    // 6px top bar vs 2px left bar tells top from bottom.
    for x in 0..w {
        for y in 0..6 {
            mut_px(&mut rgb, w, x, y, (255, 255, 255));
        }
        mut_px(&mut rgb, w, x, h - 1, (255, 255, 255));
    }
    for y in 0..h {
        for x in 0..2 {
            mut_px(&mut rgb, w, x, y, (255, 255, 255));
        }
        mut_px(&mut rgb, w, w - 1, y, (255, 255, 255));
    }
    to_rgba(rgb, w, h)
}

/// 2px border + corner blocks + crosshair: shows dead-space when the assumed
/// size is larger than the panel.
pub fn ruler_pattern(w: u32, h: u32) -> RgbaFrame {
    let (mut rgb, w) = blank(w, h, (20, 18, 16));
    for y in 0..h {
        for x in 0..w {
            let border = x < 2 || y < 2 || x >= w - 2 || y >= h - 2;
            if border {
                mut_px(&mut rgb, w, x, y, (255, 255, 255));
            }
            if x == w / 2 || y == h / 2 {
                mut_px(&mut rgb, w, x, y, (80, 70, 70));
            }
        }
    }
    for y in 0..10 {
        for x in 0..10 {
            mut_px(&mut rgb, w, 2 + x, 2 + y, (60, 60, 255));
            mut_px(&mut rgb, w, w - 12 + x, 2 + y, (60, 255, 60));
            mut_px(&mut rgb, w, 2 + x, h - 12 + y, (255, 120, 60));
            mut_px(&mut rgb, w, w - 12 + x, h - 12 + y, (60, 220, 255));
        }
    }
    to_rgba(rgb, w, h)
}

/// 32px color bands every axis + faint checkerboard (skew reveals wrapping).
pub fn band_pattern(w: u32, h: u32, step: u32) -> RgbaFrame {
    let palette: [Bgr; 8] = [
        (60, 60, 255),
        (40, 170, 255),
        (60, 240, 255),
        (90, 220, 70),
        (235, 190, 60),
        (255, 110, 90),
        (230, 90, 200),
        (255, 255, 255),
    ];
    let (mut rgb, w) = blank(w, h, (18, 18, 22));
    for y in 0..h {
        for x in 0..w {
            let c = if y < step {
                palette[(x / step) as usize % palette.len()]
            } else if x < step {
                palette[(y / step) as usize % palette.len()]
            } else {
                let dark = ((x / step) + (y / step)) % 2 == 0;
                if dark { (22, 18, 18) } else { (42, 34, 34) }
            };
            mut_px(&mut rgb, w, x, y, c);
        }
    }
    to_rgba(rgb, w, h)
}

/// RGBW quadrants — quick smoke test.
pub fn test_pattern(w: u32, h: u32) -> RgbaFrame {
    let (mut rgb, w) = blank(w, h, (0, 0, 0));
    for y in 0..h {
        for x in 0..w {
            let c: Bgr = match (x < w / 2, y < h / 2) {
                (true, true) => (0, 0, 255),
                (false, true) => (0, 255, 0),
                (true, false) => (255, 0, 0),
                (false, false) => (255, 255, 255),
            };
            mut_px(&mut rgb, w, x, y, c);
        }
    }
    to_rgba(rgb, w, h)
}

pub fn solid(c: Bgr, w: u32, h: u32) -> RgbaFrame {
    let (rgb, w) = blank(w, h, c);
    to_rgba(rgb, w, h)
}

fn to_rgba(rgb: Vec<u8>, w: u32, h: u32) -> RgbaFrame {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for px in rgb.chunks_exact(3) {
        rgba.push(px[0]);
        rgba.push(px[1]);
        rgba.push(px[2]);
        rgba.push(255);
    }
    RgbaFrame {
        width: w,
        height: h,
        rgba,
    }
}

// ---------------------------------------------------------------------------
// Tiny 3x5 font for the on-device clock
// ---------------------------------------------------------------------------

/// 3x5 font (rows packed LSB-first left-to-right): digits + colon.
const GLYPHS: [[u8; 5]; 11] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
    [0b000, 0b010, 0b000, 0b010, 0b000], // colon
];

fn glyph(ch: char) -> Option<&'static [u8; 5]> {
    match ch {
        '0'..='9' => Some(&GLYPHS[ch as usize - '0' as usize]),
        ':' => Some(&GLYPHS[10]),
        _ => None,
    }
}

fn draw_chars(
    rgb: &mut [u8],
    w: u32,
    h: u32,
    text: &str,
    scale: u32,
    color: Bgr,
    centered: bool,
) -> u32 {
    let cell_w = 3 * scale + scale; // glyph + 1px gap
    let total = (text.len() as u32) * cell_w;
    let mut cx = if centered { w.saturating_sub(total) / 2 } else { 8 };
    let cy = h.saturating_sub(5 * scale / 2) / 2;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (ry, row) in rows.iter().enumerate() {
                for rx in 0..3 {
                    if row & (0b100 >> rx) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let x = cx + rx * scale + sx;
                                let y = cy + (ry as u32) * scale + sy;
                                if x < w && y < h {
                                    mut_px(rgb, w, x, y, color);
                                }
                            }
                        }
                    }
                }
            }
        }
        cx += cell_w;
    }
    total
}

/// Render `text` (digits + colons) scaled to fill the panel height.
pub fn clock_frame(text: &str, w: u32, h: u32, fg: Bgr, bg: Bgr, now_colon: bool) -> RgbaFrame {
    let (mut rgb, w) = blank(w, h, bg);
    let shown: String = if now_colon {
        text.to_string()
    } else {
        text.replace(':', " ")
    };
    let scale = if shown.len() <= 5 { 6 } else { 4 };
    draw_chars(&mut rgb, w, h, &shown, scale, fg, true);
    to_rgba(rgb, w, h)
}

/// `n` clock frames of 1s each (colon blinks per frame).
pub fn clock_frames(
    n: usize,
    with_seconds: bool,
    fg: Bgr,
    bg: Bgr,
    w: u32,
    h: u32,
) -> Vec<(RgbaFrame, u8)> {
    use chrono::Local;
    let start = Local::now();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = start + chrono::Duration::seconds(i as i64);
        let text = if with_seconds {
            t.format("%H:%M:%S").to_string()
        } else {
            t.format("%H:%M").to_string()
        };
        let colon = i % 2 == 0;
        out.push((clock_frame(&text, w, h, fg, bg, colon), 10)); // 10 units = 1s
    }
    out
}
