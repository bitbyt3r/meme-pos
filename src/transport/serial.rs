//! Serial (COM / tty) transport via the cross-platform `serialport` crate.
//!
//! The NCR 7198's "EPiC/ION" USB mode is an Edgeport USB-to-serial bridge. With
//! the Edgeport driver (Windows) or the `io_edgeport` kernel module (Linux), the
//! printer appears as a serial port — `COM5`, `/dev/ttyUSB0`, etc. — and the OS
//! serial layer honours the printer's hardware flow control (DTR/DSR), which is
//! exactly what raw WinUSB to the EPiC interface could not do.
//!
//! At the printer's default 9600 baud the data rate (~960 B/s) is already well
//! below the print/drain rate, so the 4 KB buffer never overflows even without
//! flow control — slow but reliable. Raise the baud (printer config + here) and
//! enable hardware flow control for speed.

use anyhow::{Context, Result};
use std::io::Write;
use std::time::Duration;

use super::Transport;

pub struct SerialPrinter {
    port: Box<dyn serialport::SerialPort>,
}

impl SerialPrinter {
    /// Open a serial port (`COM5`, `/dev/ttyUSB0`, …) at `baud`, 8-N-1.
    /// `flow` selects flow control: `"none"`, `"hardware"` (RTS/CTS), or
    /// `"software"` (XON/XOFF). Hardware is safest for binary image data but
    /// requires the printer set to RTS/CTS; `none` is fine at low baud.
    pub fn open(path: &str, baud: u32, flow: &str) -> Result<Self> {
        let flow_control = match flow {
            "hardware" | "hw" | "rtscts" => serialport::FlowControl::Hardware,
            "software" | "sw" | "xonxoff" => serialport::FlowControl::Software,
            _ => serialport::FlowControl::None,
        };
        let port = serialport::new(path, baud)
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .flow_control(flow_control)
            .timeout(Duration::from_secs(60))
            .open()
            .with_context(|| format!("opening serial port '{path}' @ {baud} 8N1"))?;
        Ok(Self { port })
    }
}

impl Transport for SerialPrinter {
    fn write_job(&mut self, data: &[u8]) -> Result<()> {
        // write_all blocks (respecting flow control) until every byte is sent.
        self.port.write_all(data).context("serial write")?;
        self.port.flush().context("serial flush (drain)")?;
        Ok(())
    }
}
