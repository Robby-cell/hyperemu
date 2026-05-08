pub mod gpio;
pub mod ram;
pub mod uart;

use crate::error::EmuError;

pub trait Device {
    fn read_8(&mut self, offset: u64) -> Result<u8, EmuError>;
    fn write_8(&mut self, offset: u64, val: u8) -> Result<(), EmuError>;

    // Default implementations using little-endian access
    fn read_16(&mut self, offset: u64) -> Result<u16, EmuError> {
        let lo = self.read_8(offset)? as u16;
        let hi = self.read_8(offset + 1)? as u16;
        Ok(lo | (hi << 8))
    }

    fn write_16(&mut self, offset: u64, val: u16) -> Result<(), EmuError> {
        self.write_8(offset, (val & 0xFF) as u8)?;
        self.write_8(offset + 1, ((val >> 8) & 0xFF) as u8)?;
        Ok(())
    }

    fn read_32(&mut self, offset: u64) -> Result<u32, EmuError> {
        let lo = self.read_16(offset)? as u32;
        let hi = self.read_16(offset + 2)? as u32;
        Ok(lo | (hi << 16))
    }

    fn write_32(&mut self, offset: u64, val: u32) -> Result<(), EmuError> {
        self.write_16(offset, (val & 0xFFFF) as u16)?;
        self.write_16(offset + 2, ((val >> 16) & 0xFFFF) as u16)?;
        Ok(())
    }
}
