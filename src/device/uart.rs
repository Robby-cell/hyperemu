use crate::device::Device;
use crate::error::EmuError;
use std::io::Write;

/// A simple UART (Universal Asynchronous Receiver-Transmitter) for I/O.
#[repr(transparent)]
pub struct Uart(Box<dyn Write + Send>);

impl Uart {
    /// Creates a UART using a custom writer (Files, Sockets, Channels, etc.)
    pub fn new<W: Write + Send + 'static>(writer: W) -> Self {
        Self(Box::new(writer))
    }

    /// Convenience constructor for writing directly to the host's terminal
    pub fn new_stdout() -> Self {
        Self::new(std::io::stdout())
    }
}

impl Device for Uart {
    fn read_8(&mut self, offset: u64) -> Result<u8, EmuError> {
        match offset {
            0x18 => Ok(0x90), // PL011 Flag Register (TXFE | RXFE ready flags)
            _ => Ok(0),
        }
    }

    fn write_8(&mut self, offset: u64, val: u8) -> Result<(), EmuError> {
        if offset == 0 {
            let _ = self.0.write_all(&[val]);
        }
        Ok(())
    }
}
