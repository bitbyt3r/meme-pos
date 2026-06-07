//! Composes the "6-SEVEN" joke receipt as a NATIVE ESC/POS command stream:
//! printer-font text + a real scannable Code 39 barcode, with `GS *` raster used
//! only for the logo and the random meme. `$0.00` everything, of course.

use rand::seq::SliceRandom;
use rand::Rng;

use crate::escpos as e;

const WIDTH: u32 = 576;
const MEME_DIR: &str = "assets/memes";

/// Fallback joke names for snacks / unmapped scanned barcodes.
const SNACKS: &[&str] = &[
    "BIG GULP",
    "TAQUITO (ROLLER)",
    "FLAMIN' HOT CHEETOS",
    "BLUE SLURPEE",
    "PINK FROSTED DONUT",
    "BEEF JERKY STICK",
    "MONSTER ENERGY",
    "PIZZA HOT POCKET",
    "NACHOS SUPREME",
    "CHILI CHEESE CORN DOG",
    "GAS STATION SUSHI",
    "FUNYUNS",
];

const FOOTERS: &[&str] = &[
    "NO REFUNDS. NO EXCEPTIONS.",
    "YOUR MONEY IS NO GOOD HERE",
    "SNACKS ARE FREE. VIBES ARE PRICELESS.",
    "TELL NO ONE",
];

/// A receipt with a fresh random cart (used for the empty-cart "mystery" print).
pub fn build_receipt_job() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let order = rng.gen_range(1000..9999);
    let n = rng.gen_range(3..=6);
    let items: Vec<(String, u32)> = SNACKS
        .choose_multiple(&mut rng, n)
        .map(|s| (s.to_string(), rng.gen_range(1..=3)))
        .collect();
    build_receipt_job_items(&items, order)
}

/// A random joke snack name (used for unmapped scanned barcodes in the POS).
pub fn random_snack() -> String {
    SNACKS.choose(&mut rand::thread_rng()).unwrap().to_string()
}

/// Build the native receipt for a specific cart of `(name, qty)` items + order #.
pub fn build_receipt_job_items(items: &[(String, u32)], order: u32) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let reg = rng.gen_range(1..7);
    let footer = *FOOTERS.choose(&mut rng).unwrap();

    let mut j = e::init();

    // header: rastered "6-SEVEN" logo (Impact font renders cleaner than native).
    j.extend(e::align(1));
    if let Ok(logo) = crate::render::svg_to_mono(&logo_svg(), crate::raster::Dither::Atkinson) {
        j.extend(e::download_image(&logo));
    }
    j.extend(e::text("YOUR HOMETOWN-ISH CONVENIENCE STORE\n"));
    j.extend(e::text("MAGStock * Lot 67 * Open 24/7-ish\n"));
    j.extend(e::rule('='));

    // order meta + items
    j.extend(e::align(0));
    j.extend(e::line_lr(&format!("REGISTER #{reg}"), &format!("ORDER {order}")));
    j.extend(e::text("CASHIER: TRUSTWORTHY TEEN\n"));
    j.extend(e::rule('-'));
    let items: Vec<(String, u32)> = if items.is_empty() {
        vec![("(NOTHING - JUST VIBES)".to_string(), 1)]
    } else {
        items.to_vec()
    };
    for (name, qty) in &items {
        let label = if *qty > 1 { format!("{name} x{qty}") } else { name.clone() };
        j.extend(e::line_lr(&label, "$0.00"));
    }
    j.extend(e::rule('-'));
    j.extend(e::line_lr("SUBTOTAL", "$0.00"));
    j.extend(e::line_lr("TAX (0%)", "$0.00"));
    j.extend(e::bold(true));
    j.extend(e::text_size(2, 2));
    j.extend(e::line_lr("TOTAL", "$0.00"));
    j.extend(e::text_size(1, 1));
    j.extend(e::bold(false));
    j.extend(e::line_lr("TENDERED", "YOUR DIGNITY"));
    j.extend(e::line_lr("CHANGE", "A NEW PERSPECTIVE"));
    j.extend(e::rule('='));

    // footer
    j.extend(e::align(1));
    j.extend(e::bold(true));
    j.extend(e::text_size(2, 2));
    j.extend(e::text("*** NO REFUNDS ***\n"));
    j.extend(e::text_size(1, 1));
    j.extend(e::bold(false));
    j.extend(e::text(footer));
    j.extend(e::text("\n"));

    // the one rastered photo: a random meme
    if let Some(path) = pick_meme_path() {
        if let Ok(mono) = crate::render::meme_to_mono(&path, 480) {
            j.extend(e::text("~ TODAY'S FREE MEME ~\n"));
            j.extend(e::download_image(&mono));
            j.extend(e::text("\n"));
        }
    }

    // real scannable barcode of the order
    j.extend(e::barcode_code39(&format!("6SEVEN{order}"), 90));
    j.extend(e::text("\nTHANK YOU * NO REFUNDS\n"));

    j.extend(e::cut());
    j
}

/// Stylized "6•SEVEN" logo as a 576-wide SVG (rastered for the receipt header).
fn logo_svg() -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{WIDTH}' height='104'>\
         <rect width='{WIDTH}' height='104' fill='#fff'/>\
         <text x='{}' y='80' font-size='84' \
         font-family='Arial Black, Arial, sans-serif' font-weight='900' \
         text-anchor='middle' fill='#000'>6\u{2022}SEVEN</text></svg>",
        WIDTH / 2
    )
}

/// A random meme image file path from [`MEME_DIR`] (or `None` if empty/missing).
pub fn pick_meme_path() -> Option<std::path::PathBuf> {
    let entries: Vec<std::path::PathBuf> = std::fs::read_dir(MEME_DIR)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            matches!(
                p.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()).as_deref(),
                Some("png" | "jpg" | "jpeg")
            )
        })
        .collect();
    entries.choose(&mut rand::thread_rng()).cloned()
}
