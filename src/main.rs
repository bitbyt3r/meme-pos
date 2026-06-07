//! 6-SEVEN scanner POS: read barcodes from a Honeywell scanner (USB-serial),
//! log every item, and print a joke receipt of the cart on the PRINT barcode.

mod escpos;
mod pos;
mod raster;
mod receipt;
mod render;
mod transport;

use anyhow::{anyhow, Result};
use transport::serial::SerialPrinter;
use transport::Transport;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--pos") => {
            let scanner = args.get(1).map(String::as_str).ok_or_else(usage_err)?;
            let printer = args.get(2).map(String::as_str).ok_or_else(usage_err)?;
            let (baud, flow) = printer_opts(&args, 3);
            pos::run_pos(scanner, printer, baud, flow)
        }
        Some("--pos-barcodes") => {
            let printer = args.get(1).map(String::as_str).ok_or_else(usage_err)?;
            let (baud, flow) = printer_opts(&args, 2);
            println!("printing POS control barcodes (PRINT / VOID) to {printer}...");
            let mut p = SerialPrinter::open(printer, baud, flow)?;
            p.write_job(&pos::control_barcodes_job())?;
            println!("done — tape them to the counter.");
            Ok(())
        }
        Some("--list-ports") => {
            for p in serialport::available_ports().unwrap_or_default() {
                println!("  {}  ({:?})", p.port_name, p.port_type);
            }
            Ok(())
        }
        _ => {
            print_usage();
            Ok(())
        }
    }
}

/// Parse optional `[baud] [flow]` printer args starting at index `i`.
fn printer_opts(args: &[String], i: usize) -> (u32, &str) {
    let baud = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(115200);
    let flow = args.get(i + 1).map(String::as_str).unwrap_or("hardware");
    (baud, flow)
}

fn usage_err() -> anyhow::Error {
    anyhow!("missing port argument")
}

fn print_usage() {
    eprintln!(
        "6-SEVEN POS\n\
         \n\
         Run the register:\n\
         \x20 receipt-printer --pos <scanner_port> <printer_port> [baud] [flow]\n\
         \x20   scan items, then the PRINT barcode; VOID clears; empty PRINT = random receipt\n\
         \x20   every scan is logged to pos-log.csv; item names map via assets/items.csv\n\
         \n\
         One-time setup:\n\
         \x20 receipt-printer --list-ports                  list serial ports (find scanner + printer)\n\
         \x20 receipt-printer --pos-barcodes <printer_port> print the PRINT/VOID barcodes to tape down\n\
         \n\
         printer baud defaults to 115200, flow to hardware (none|hardware|software)."
    );
}
