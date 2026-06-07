//! Transport: how raw ESC/POS bytes reach the printer. In production this is the
//! serial port (the NCR 7198's Edgeport USB-serial COM port).

pub mod serial;

/// Anything that can swallow a complete ESC/POS job and ship it to paper.
pub trait Transport {
    /// Send one complete job (already-built ESC/POS byte stream).
    fn write_job(&mut self, data: &[u8]) -> anyhow::Result<()>;
}
