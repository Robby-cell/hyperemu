use crate::device::Device;
use crate::error::EmuError;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Perms: u8 {
        const R = 1 << 0;
        const W = 1 << 1;
        const X = 1 << 2;
        const RW = Self::R.bits() | Self::W.bits();
        const RWX = Self::R.bits() | Self::W.bits() | Self::X.bits();
    }
}

pub struct MemoryRegion {
    pub start: u64,
    pub size: u64,
    pub perms: Perms,
    pub device: Box<dyn Device>,
}

pub struct MemoryBus {
    regions: Vec<MemoryRegion>,
}

impl MemoryBus {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn map(&mut self, start: u64, size: u64, perms: Perms, device: Box<dyn Device>) {
        self.regions.push(MemoryRegion {
            start,
            size,
            perms,
            device,
        });
        // Keeping it sorted ensures we can optimize lookup later (e.g., binary search)
        self.regions.sort_by_key(|r| r.start);
    }

    /// Helper to find the correct device and translate absolute address to offset
    fn resolve_mut(&mut self, addr: u64) -> Result<(&mut Box<dyn Device>, u64), EmuError> {
        for region in &mut self.regions {
            if addr >= region.start && addr < region.start + region.size {
                return Ok((&mut region.device, addr - region.start));
            }
        }
        Err(EmuError::MemoryFault(addr))
    }

    pub fn read_8(&mut self, addr: u64) -> Result<u8, EmuError> {
        let (device, offset) = self.resolve_mut(addr)?;
        device.read_8(offset)
    }

    pub fn write_8(&mut self, addr: u64, val: u8) -> Result<(), EmuError> {
        let (device, offset) = self.resolve_mut(addr)?;
        device.write_8(offset, val)
    }

    pub fn read_16(&mut self, addr: u64) -> Result<u16, EmuError> {
        let (device, offset) = self.resolve_mut(addr)?;
        device.read_16(offset)
    }

    pub fn write_16(&mut self, addr: u64, val: u16) -> Result<(), EmuError> {
        let (device, offset) = self.resolve_mut(addr)?;
        device.write_16(offset, val)
    }

    pub fn read_32(&mut self, addr: u64) -> Result<u32, EmuError> {
        let (device, offset) = self.resolve_mut(addr)?;
        device.read_32(offset)
    }

    pub fn write_32(&mut self, addr: u64, val: u32) -> Result<(), EmuError> {
        let (device, offset) = self.resolve_mut(addr)?;
        device.write_32(offset, val)
    }

    /// Reads a block of bytes across the bus.
    /// Iterates byte-by-byte to ensure safe handling if the buffer crosses memory region boundaries.
    pub fn read_bytes(&mut self, mut addr: u64, buf: &mut [u8]) -> Result<(), EmuError> {
        for byte in buf.iter_mut() {
            *byte = self.read_8(addr)?;
            addr += 1;
        }
        Ok(())
    }

    /// Writes a block of bytes across the bus.
    /// Iterates byte-by-byte to handle cross-region boundaries safely.
    pub fn write_bytes(&mut self, mut addr: u64, buf: &[u8]) -> Result<(), EmuError> {
        for &byte in buf.iter() {
            self.write_8(addr, byte)?;
            addr += 1;
        }
        Ok(())
    }
}
