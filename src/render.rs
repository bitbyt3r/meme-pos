//! Image rendering for the two rastered parts of the receipt: the SVG "6-SEVEN"
//! logo and the meme photo. Both end up as a dithered 1-bit [`Mono`].

use anyhow::{Context, Result};
use resvg::{tiny_skia, usvg};

use crate::raster::{dither_to_mono, Dither, Mono};

/// Render an SVG document (authored at the printer's pixel width) to [`Mono`].
pub fn svg_to_mono(svg: &str, dither: Dither) -> Result<Mono> {
    let mut opt = usvg::Options::default();
    // The logo SVG asks for Arial Black/Arial, which don't exist on Linux/RPi.
    // Load system + bundled fonts, then fall back to whatever font IS installed
    // so the logo never prints blank.
    let fallback = {
        let db = opt.fontdb_mut();
        db.load_system_fonts();
        db.load_fonts_dir("assets/fonts"); // optional bundled fonts (portable across OS)
        db.faces()
            .next()
            .and_then(|f| f.families.first().map(|(name, _)| name.clone()))
    };
    if let Some(family) = fallback {
        opt.font_family = family;
    }

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

/// Render `text` as huge letters running LENGTHWISE down the receipt (rotated 90°,
/// letters as tall as the paper is wide). Used for the "NO REFUNDS" stunt print.
pub fn banner_lengthwise(text: &str) -> Result<Mono> {
    const W: usize = 576; // receipt width = the rotated text's height
    // Render the text big and horizontal on a generously wide canvas.
    let esc = text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='6000' height='{W}'>\
         <rect width='6000' height='{W}' fill='#fff'/>\
         <text x='20' y='548' font-size='720' \
         font-family='Arial Black, Arial, Liberation Sans, DejaVu Sans, FreeSans, sans-serif' \
         font-weight='900' fill='#000'>{esc}</text></svg>"
    );
    let flat = svg_to_mono(&svg, Dither::Threshold)?;

    // Horizontal extent of the inked text (so we don't print blank paper).
    let (mut minx, mut maxx) = (flat.width, 0usize);
    for x in 0..flat.width {
        if (0..flat.height).any(|y| flat.black(x, y)) {
            minx = minx.min(x);
            maxx = maxx.max(x);
        }
    }
    if maxx < minx {
        anyhow::bail!("no glyphs rendered for banner (no font available?)");
    }
    let pad = 16usize;
    let minx = minx.saturating_sub(pad);
    let maxx = (maxx + pad).min(flat.width - 1);
    let textw = maxx - minx + 1;

    // Rotate 90° clockwise: new image is W wide (=old height) × textw tall (=text length).
    let mut luma = vec![255u8; W * textw];
    for ny in 0..textw {
        for nx in 0..W {
            // CW: dest(nx,ny) = src(minx + ny, (W-1) - nx)
            if flat.black(minx + ny, (W - 1) - nx) {
                luma[ny * W + nx] = 0;
            }
        }
    }
    Ok(dither_to_mono(&luma, W, textw, Dither::Threshold))
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
