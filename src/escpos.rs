//! ESC/POS command builders for the NCR 7198 (in 7194 emulation, driven over the
//! Edgeport serial port). Only what the receipt needs: native text formatting, a
//! real Code 39 barcode, and `GS *` download for the rastered bits (logo + meme).

use crate::raster::Mono;

pub const ESC: u8 = 0x1B;
pub const GS: u8 = 0x1D;

/// Characters per line for the built-in font at 80 mm (measured on this 7198).
pub const COLS: usize = 44;

/// `ESC @` — initialize printer.
pub fn init() -> Vec<u8> {
    vec![ESC, b'@']
}

/// Raw text (ASCII).
pub fn text(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

/// `ESC d n` — print buffer and feed `n` lines.
pub fn feed_lines(n: u8) -> Vec<u8> {
    vec![ESC, b'd', n]
}

/// Feed clear of the cutter, then partial cut (`GS V 1`). The cutter sits
/// ~15-20 mm above the head, so feed first or it slices the last line. (The 7198
/// does NOT support `GS V 66`.)
pub fn cut() -> Vec<u8> {
    let mut out = feed_lines(8);
    out.extend_from_slice(&[GS, b'V', 1]);
    out
}

// --- native text formatting -------------------------------------------------

/// `ESC a n` — justification: 0 = left, 1 = center, 2 = right.
pub fn align(n: u8) -> Vec<u8> {
    vec![ESC, b'a', n]
}

/// `GS ! n` — character magnification, `w`/`h` in 1..=8.
pub fn text_size(w: u8, h: u8) -> Vec<u8> {
    let w = w.clamp(1, 8) - 1;
    let h = h.clamp(1, 8) - 1;
    vec![GS, b'!', (w << 4) | h]
}

/// `ESC E n` — emphasized (bold) on/off.
pub fn bold(on: bool) -> Vec<u8> {
    vec![ESC, b'E', on as u8]
}

/// A line with `left` justified and `right` right-justified, padded to [`COLS`].
pub fn line_lr(left: &str, right: &str) -> Vec<u8> {
    let budget = COLS.saturating_sub(right.len());
    let left = if left.len() > budget { &left[..budget] } else { left };
    let pad = COLS.saturating_sub(left.len() + right.len());
    format!("{left}{}{right}\n", " ".repeat(pad)).into_bytes()
}

/// A full-width rule of `ch` (e.g. `'='` or `'-'`).
pub fn rule(ch: char) -> Vec<u8> {
    let mut s: String = std::iter::repeat(ch).take(COLS).collect();
    s.push('\n');
    s.into_bytes()
}

// --- native barcode ---------------------------------------------------------

/// A scannable **Code 39** barcode via `GS k` (length-prefixed), digits below.
pub fn barcode_code39(data: &str, height_dots: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[GS, b'h', height_dots]); // bar height
    out.extend_from_slice(&[GS, b'w', 3]); // module width
    out.extend_from_slice(&[GS, b'H', 2]); // HRI text below
    out.extend_from_slice(&[GS, b'f', 0]); // HRI font A
    let bytes = data.as_bytes();
    out.extend_from_slice(&[GS, b'k', 69, bytes.len() as u8]); // m=69 = CODE39
    out.extend_from_slice(bytes);
    out
}

// --- raster image (the ONLY clean image path in 7194 mode) ------------------

/// Print a 1-bit image via `GS *` download + `GS /`, the only image command that
/// renders correctly here (`GS v 0` and `ESC *` are dead ends). Tiled at 168 dots
/// (`x*y ≤ 1536`), column-major, MSB = top dot. NO control-byte detox — `GS *`
/// reads an exact byte count over serial, so raw bytes are safe (detox added a
/// spurious dot per 0x01..0x1F byte → "echo" lines above text).
pub fn download_image(m: &Mono) -> Vec<u8> {
    let x = m.width.div_ceil(8); // 72 for 576 dots
    let y_max = (1536 / x.max(1)).clamp(1, 21); // 21 byte-rows = 168 dots/tile
    let total_byte_rows = m.height.div_ceil(8);

    let mut out = Vec::with_capacity(m.bits.len() + 8 * (total_byte_rows / y_max + 1));
    let mut row0 = 0;
    while row0 < total_byte_rows {
        let y = (total_byte_rows - row0).min(y_max);
        out.extend_from_slice(&[GS, b'*', x as u8, y as u8]);
        for c in 0..m.width {
            for r in 0..y {
                let mut byte = 0u8;
                for bit in 0..8 {
                    if m.black(c, (row0 + r) * 8 + bit) {
                        byte |= 0x80 >> bit;
                    }
                }
                out.push(byte);
            }
        }
        out.extend_from_slice(&[GS, b'/', 0]); // print + auto-feed by tile height
        row0 += y;
    }
    out
}
