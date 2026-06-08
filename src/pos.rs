//! Scanner-driven point-of-sale loop.
//!
//! Reads barcodes from the Honeywell 4600g (in USB-Serial mode) on a COM/tty
//! port — no window focus needed. Each scanned item is logged and added to the
//! cart; scanning the **PRINT** barcode prints the receipt (the actual cart) and
//! starts a new sale; **VOID** clears the cart. Known item barcodes map to names
//! via `assets/items.csv`; unknown barcodes get a random joke-snack name but are
//! still logged by barcode so you get real numbers.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use crate::receipt;
use crate::transport::serial::SerialPrinter;
use crate::transport::Transport;

const ITEMS_FILE: &str = "assets/items.csv";
const LOG_FILE: &str = "pos-log.csv";
const PRINT_CODE: &str = "PRINT";
const SAVE_CODE: &str = "SAVE";
const VOID_CODE: &str = "VOID";
const STATS_CODE: &str = "STATS";

/// Run the POS loop: read scans from `scanner_port`, print to `printer_port`.
pub fn run_pos(scanner_port: &str, printer_port: &str, printer_baud: u32, printer_flow: &str) -> Result<()> {
    let mut names = load_items(ITEMS_FILE);
    println!("loaded {} known item(s) from {ITEMS_FILE}", names.len());

    let log_is_new = !Path::new(LOG_FILE).exists();
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)
        .with_context(|| format!("opening {LOG_FILE}"))?;
    if log_is_new {
        writeln!(log, "timestamp,transaction,barcode,name")?;
    }

    let mut scanner = serialport::new(scanner_port, 9600)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .timeout(Duration::from_millis(500))
        .open()
        .with_context(|| format!("opening scanner on {scanner_port}"))?;

    let mut cart: Vec<(String, String)> = Vec::new(); // (barcode, name)
    let mut txn: u32 = 1;
    let mut buf: Vec<u8> = Vec::new();

    println!("\n=== 6-SEVEN POS READY ===");
    println!("scanner: {scanner_port}   printer: {printer_port}");
    println!("scan items, then PRINT (receipt+log), SAVE (log only), or VOID (discard). Ctrl-C to quit.\n");

    loop {
        let code = read_barcode(&mut *scanner, &mut buf)?;
        match code.to_uppercase().as_str() {
            PRINT_CODE => {
                if cart.is_empty() {
                    // Empty cart → print a fully random joke receipt, log nothing.
                    let job = receipt::build_receipt_job();
                    match print_job(printer_port, printer_baud, printer_flow, &job) {
                        Ok(()) => println!("🎲 random receipt (empty cart — not logged)\n"),
                        Err(e) => eprintln!("print failed: {e:#}"),
                    }
                    continue;
                }
                let order = 1000 + txn;
                let job = receipt::build_receipt_job_items(&aggregate(&cart), order);
                match print_job(printer_port, printer_baud, printer_flow, &job) {
                    Ok(()) => println!("🧾 printed order #{order}  ({} items)\n", cart.len()),
                    Err(e) => eprintln!("print failed: {e:#}"),
                }
                save_cart(&mut log, txn, &cart);
                txn += 1;
                cart.clear();
            }
            SAVE_CODE => {
                if cart.is_empty() {
                    println!("(nothing to save)\n");
                    continue;
                }
                let n = cart.len();
                save_cart(&mut log, txn, &cart);
                println!("💾 saved {n} item(s) to log (txn {txn}, no receipt)\n");
                txn += 1;
                cart.clear();
            }
            VOID_CODE => {
                println!("✗ voided {} item(s) (not saved)\n", cart.len());
                cart.clear();
            }
            STATS_CODE => {
                // Cumulative sales report from the whole pos-log.csv.
                let job = build_stats_report();
                match print_job(printer_port, printer_baud, printer_flow, &job) {
                    Ok(()) => println!("📊 sales report printed\n"),
                    Err(e) => eprintln!("print failed: {e:#}"),
                }
            }
            // Rescanning a printed receipt's own barcode ("6SEVEN<order>") prints
            // a giant lengthwise NO REFUNDS. Not an item — logs nothing.
            c if c.starts_with("6SEVEN") => {
                let job = receipt::build_no_refunds_banner();
                match print_job(printer_port, printer_baud, printer_flow, &job) {
                    Ok(()) => println!("🚫 NO REFUNDS (lengthwise)\n"),
                    Err(e) => eprintln!("print failed: {e:#}"),
                }
            }
            _ => {
                // Items are held in the cart only; nothing is logged until SAVE or
                // PRINT (so VOID can truly discard them).
                let known = names.contains_key(&code);
                let name = names
                    .entry(code.clone())
                    .or_insert_with(receipt::random_snack)
                    .clone();
                cart.push((code.clone(), name.clone()));
                let tag = if known { "" } else { "  (unmapped)" };
                println!("+ {name}{tag}   [{code}]   cart: {}", cart.len());
            }
        }
    }
}

/// Build a printable sheet of the PRINT and VOID control barcodes (Code 39).
pub fn control_barcodes_job() -> Vec<u8> {
    use crate::escpos as e;
    let mut j = e::init();
    j.extend(e::align(1));
    j.extend(e::bold(true));
    j.extend(e::text_size(2, 2));
    j.extend(e::text("6-SEVEN POS\n"));
    j.extend(e::text_size(1, 1));
    j.extend(e::bold(false));
    j.extend(e::text("scan items, then PRINT / SAVE / VOID\n\n"));

    j.extend(e::bold(true));
    j.extend(e::text(">> PRINT / CHECKOUT <<\n"));
    j.extend(e::bold(false));
    j.extend(e::barcode_code39(PRINT_CODE, 110));
    j.extend(e::text("\n\n"));

    j.extend(e::bold(true));
    j.extend(e::text(">> SAVE / LOG (NO RECEIPT) <<\n"));
    j.extend(e::bold(false));
    j.extend(e::barcode_code39(SAVE_CODE, 110));
    j.extend(e::text("\n\n"));

    j.extend(e::bold(true));
    j.extend(e::text(">> VOID / DISCARD <<\n"));
    j.extend(e::bold(false));
    j.extend(e::barcode_code39(VOID_CODE, 110));
    j.extend(e::text("\n\n"));

    j.extend(e::bold(true));
    j.extend(e::text(">> STATS / SALES REPORT <<\n"));
    j.extend(e::bold(false));
    j.extend(e::barcode_code39(STATS_CODE, 110));
    j.extend(e::text("\n"));
    j.extend(e::cut());
    j
}

/// Build a cumulative sales report from the entire `pos-log.csv`. Known barcodes
/// (present in `items.csv`) are grouped by canonical name; unknown barcodes are
/// listed by raw code + count so they can be added to `items.csv` afterward.
fn build_stats_report() -> Vec<u8> {
    use crate::escpos as e;
    let known = load_items(ITEMS_FILE);

    let mut named: HashMap<String, u32> = HashMap::new(); // canonical name -> qty
    let mut unknown: HashMap<String, u32> = HashMap::new(); // raw barcode -> qty
    let mut total = 0u32;
    let mut first_ts: Option<String> = None;
    let mut last_ts: Option<String> = None;

    if let Ok(content) = std::fs::read_to_string(LOG_FILE) {
        for line in content.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // ts,txn,barcode,"name"  — name may contain commas, so split into 4.
            let parts: Vec<&str> = line.splitn(4, ',').collect();
            if parts.len() < 4 {
                continue;
            }
            let ts = parts[0].trim();
            let barcode = parts[2].trim();
            if barcode.is_empty() {
                continue;
            }
            total += 1;
            if first_ts.is_none() {
                first_ts = Some(ts.to_string());
            }
            last_ts = Some(ts.to_string());
            match known.get(barcode) {
                Some(name) => *named.entry(name.clone()).or_insert(0) += 1,
                None => *unknown.entry(barcode.to_string()).or_insert(0) += 1,
            }
        }
    }

    // Sort each list by count (desc), then name/barcode for stable ties.
    let mut named_v: Vec<(String, u32)> = named.into_iter().collect();
    named_v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut unknown_v: Vec<(String, u32)> = unknown.into_iter().collect();
    unknown_v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M");

    let mut j = e::init();
    j.extend(e::align(1));
    j.extend(e::bold(true));
    j.extend(e::text_size(2, 2));
    j.extend(e::text("6-SEVEN\n"));
    j.extend(e::text_size(1, 1));
    j.extend(e::text("SALES REPORT\n"));
    j.extend(e::bold(false));
    j.extend(e::text(&format!("generated {now}\n")));
    j.extend(e::rule('='));

    if total == 0 {
        j.extend(e::text("\nNO SALES LOGGED YET\n\n"));
        j.extend(e::cut());
        return j;
    }

    j.extend(e::align(0));
    if let (Some(f), Some(l)) = (&first_ts, &last_ts) {
        j.extend(e::text(&format!("FIRST SCAN: {f}\n")));
        j.extend(e::text(&format!("LAST SCAN:  {l}\n")));
    }
    let distinct = named_v.len() + unknown_v.len();
    j.extend(e::line_lr("TOTAL ITEMS SCANNED", &total.to_string()));
    j.extend(e::line_lr("DISTINCT ITEMS", &distinct.to_string()));
    j.extend(e::rule('-'));

    j.extend(e::bold(true));
    j.extend(e::text("KNOWN ITEMS\n"));
    j.extend(e::bold(false));
    if named_v.is_empty() {
        j.extend(e::text("(none)\n"));
    } else {
        for (name, qty) in &named_v {
            j.extend(e::line_lr(name, &format!("x{qty}")));
        }
    }
    j.extend(e::rule('-'));

    j.extend(e::bold(true));
    j.extend(e::text("UNKNOWN ITEMS\n"));
    j.extend(e::bold(false));
    j.extend(e::text("add these barcodes to items.csv:\n"));
    if unknown_v.is_empty() {
        j.extend(e::text("(none - all mapped!)\n"));
    } else {
        for (barcode, qty) in &unknown_v {
            j.extend(e::line_lr(barcode, &format!("x{qty}")));
        }
    }
    j.extend(e::rule('='));

    // Top seller across everything.
    if let Some((name, qty)) = named_v.iter().chain(unknown_v.iter()).max_by_key(|(_, q)| *q) {
        j.extend(e::align(1));
        j.extend(e::bold(true));
        j.extend(e::text(&format!("TOP SELLER: {name} (x{qty})\n")));
        j.extend(e::bold(false));
    }
    j.extend(e::align(1));
    j.extend(e::text(&format!("{total} items moved * $0.00 earned\n")));

    j.extend(e::cut());
    j
}

fn load_items(path: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((code, name)) = line.split_once(',') {
                m.insert(code.trim().to_string(), name.trim().to_string());
            }
        }
    }
    m
}

/// Block until a full barcode (terminated by CR/LF) is read. TimedOut is ignored
/// so we keep waiting; other IO errors propagate (e.g. scanner unplugged).
fn read_barcode(port: &mut dyn serialport::SerialPort, buf: &mut Vec<u8>) -> Result<String> {
    let mut b = [0u8; 1];
    loop {
        match port.read(&mut b) {
            Ok(0) => continue,
            Ok(_) => {
                if b[0] == b'\r' || b[0] == b'\n' {
                    if buf.is_empty() {
                        continue;
                    }
                    let s = String::from_utf8_lossy(buf).trim().to_string();
                    buf.clear();
                    if !s.is_empty() {
                        return Ok(s);
                    }
                } else if b[0] >= 0x20 {
                    buf.push(b[0]);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(e).context("reading from scanner"),
        }
    }
}

/// Aggregate the cart into `(name, qty)` preserving first-seen order.
fn aggregate(cart: &[(String, String)]) -> Vec<(String, u32)> {
    let mut order: Vec<(String, u32)> = Vec::new();
    let mut idx: HashMap<&str, usize> = HashMap::new();
    for (_, name) in cart {
        if let Some(&i) = idx.get(name.as_str()) {
            order[i].1 += 1;
        } else {
            idx.insert(name, order.len());
            order.push((name.clone(), 1));
        }
    }
    order
}

/// Append every cart line to the log under one transaction number.
fn save_cart(log: &mut std::fs::File, txn: u32, cart: &[(String, String)]) {
    for (barcode, name) in cart {
        log_scan(log, txn, barcode, name);
    }
}

fn log_scan(log: &mut std::fs::File, txn: u32, barcode: &str, name: &str) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let _ = writeln!(log, "{ts},{txn},{barcode},\"{}\"", name.replace('"', "'"));
    let _ = log.flush();
}

fn print_job(port: &str, baud: u32, flow: &str, job: &[u8]) -> Result<()> {
    let mut p = SerialPrinter::open(port, baud, flow)?;
    p.write_job(job)
}
