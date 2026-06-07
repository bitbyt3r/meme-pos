//! Grayscale → 1-bit conversion for thermal printing.
//!
//! Thermal heads print a monochrome dot grid. We take an 8-bit luma buffer
//! (0 = black, 255 = white) and produce [`Mono`]: rows packed MSB-first, one
//! bit per dot, `1` = black (printed). [`escpos::raster`](crate::escpos::raster)
//! turns that into `GS v 0` commands.
//!
//! Dithering only perturbs *gray* midtones — pure black/white regions (e.g.
//! vector text from resvg) stay crisp, so we can dither the whole receipt in
//! one pass and only the meme photo gets the halftone treatment.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Threshold/FloydSteinberg are selectable alternatives to the Atkinson default
pub enum Dither {
    /// Hard threshold at 50% — crispest for line art / text, blocky for photos.
    Threshold,
    /// Floyd–Steinberg error diffusion — classic, slightly noisy.
    FloydSteinberg,
    /// Atkinson error diffusion — higher contrast, cleaner on thermal. Default.
    Atkinson,
}

/// A packed 1-bit-per-pixel image, ready for `GS v 0`.
pub struct Mono {
    pub width: usize,
    pub height: usize,
    /// Bytes per row = ceil(width / 8).
    pub row_bytes: usize,
    /// `row_bytes * height` bytes, MSB-first, `1` = black dot.
    pub bits: Vec<u8>,
}

impl Mono {
    /// Is the dot at (x, y) black (printed)? Out-of-bounds reads as white.
    #[inline]
    pub fn black(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.bits[y * self.row_bytes + (x >> 3)] & (0x80 >> (x & 7)) != 0
    }
}

/// Add `v` to the error buffer at (x, y), ignoring out-of-bounds neighbours.
#[inline]
fn spill(buf: &mut [f32], w: usize, h: usize, x: isize, y: isize, v: f32) {
    if x >= 0 && (x as usize) < w && y >= 0 && (y as usize) < h {
        buf[(y as usize) * w + x as usize] += v;
    }
}

/// Dither an 8-bit luma buffer to packed 1-bpp. `luma.len()` must be `w * h`.
pub fn dither_to_mono(luma: &[u8], width: usize, height: usize, algo: Dither) -> Mono {
    assert_eq!(luma.len(), width * height, "luma buffer size mismatch");

    let mut buf: Vec<f32> = luma.iter().map(|&v| v as f32).collect();

    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let old = buf[i];
            let new = if old < 128.0 { 0.0 } else { 255.0 };
            buf[i] = new;
            let err = old - new;
            let (xi, yi) = (x as isize, y as isize);
            match algo {
                Dither::Threshold => {}
                Dither::FloydSteinberg => {
                    spill(&mut buf, width, height, xi + 1, yi, err * 7.0 / 16.0);
                    spill(&mut buf, width, height, xi - 1, yi + 1, err * 3.0 / 16.0);
                    spill(&mut buf, width, height, xi, yi + 1, err * 5.0 / 16.0);
                    spill(&mut buf, width, height, xi + 1, yi + 1, err * 1.0 / 16.0);
                }
                Dither::Atkinson => {
                    let f = err / 8.0;
                    spill(&mut buf, width, height, xi + 1, yi, f);
                    spill(&mut buf, width, height, xi + 2, yi, f);
                    spill(&mut buf, width, height, xi - 1, yi + 1, f);
                    spill(&mut buf, width, height, xi, yi + 1, f);
                    spill(&mut buf, width, height, xi + 1, yi + 1, f);
                    spill(&mut buf, width, height, xi, yi + 2, f);
                }
            }
        }
    }

    let row_bytes = width.div_ceil(8);
    let mut bits = vec![0u8; row_bytes * height];
    for y in 0..height {
        for x in 0..width {
            if buf[y * width + x] < 128.0 {
                bits[y * row_bytes + (x >> 3)] |= 0x80 >> (x & 7);
            }
        }
    }

    Mono { width, height, row_bytes, bits }
}
