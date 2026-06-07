//! Image rendering for the two rastered parts of the receipt: the SVG "6-SEVEN"
//! logo and the meme photo. Both end up as a dithered 1-bit [`Mono`].

use anyhow::{Context, Result};
use resvg::{tiny_skia, usvg};

use crate::raster::{dither_to_mono, Dither, Mono};

/// Render an SVG document (authored at the printer's pixel width) to [`Mono`].
pub fn svg_to_mono(svg: &str, dither: Dither) -> Result<Mono> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();

    let tree = usvg::Tree::from_str(svg, &opt).context("parsing logo SVG")?;
    let size = tree.size();
    let w = size.width().ceil() as u32;
    let h = size.height().ceil() as u32;

    let mut pixmap =
        tiny_skia::Pixmap::new(w, h).with_context(|| format!("allocating {w}x{h} pixmap"))?;
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());

    let data = pixmap.data();
    let (w, h) = (w as usize, h as usize);
    let mut luma = vec![0u8; w * h];
    for (i, px) in luma.iter_mut().enumerate() {
        let r = data[i * 4] as u32;
        let g = data[i * 4 + 1] as u32;
        let b = data[i * 4 + 2] as u32;
        *px = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
    }
    Ok(dither_to_mono(&luma, w, h, dither))
}

/// Load a meme image, scale to `target_w` dots, centre on a 576-dot white canvas,
/// and dither to [`Mono`] — the only rastered photo on the receipt.
pub fn meme_to_mono(path: &std::path::Path, target_w: u32) -> Result<Mono> {
    const CANVAS_W: usize = 576;
    let img = image::open(path)
        .with_context(|| format!("opening meme {}", path.display()))?
        .to_luma8();
    let (iw, ih) = img.dimensions();
    let w = target_w.min(CANVAS_W as u32).min(iw.max(1));
    let h = (ih * w / iw.max(1)).max(1);
    let scaled = image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle);

    let x0 = (CANVAS_W - w as usize) / 2;
    let mut luma = vec![255u8; CANVAS_W * h as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            luma[y * CANVAS_W + x0 + x] = scaled.get_pixel(x as u32, y as u32)[0];
        }
    }
    Ok(dither_to_mono(&luma, CANVAS_W, h as usize, Dither::Atkinson))
}
