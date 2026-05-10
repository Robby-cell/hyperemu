use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::device::Device;
use crate::device::ram::Ram;
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

pub enum BusDevice {
    Ram(Ram),

    /// Exclusively owned dynamic device (Fastest dynamic dispatch)
    Dynamic(Box<dyn Device>),

    /// Shared device for Single-Threaded GUIs and WebAssembly
    SharedRc(Rc<RefCell<dyn Device>>),

    /// Shared device for Multi-Threaded GUIs
    SharedArc(Arc<Mutex<dyn Device + Send>>),
}

impl From<Ram> for BusDevice {
    fn from(val: Ram) -> Self {
        BusDevice::Ram(val)
    }
}
impl<D: Device + 'static> From<Box<D>> for BusDevice {
    fn from(val: Box<D>) -> Self {
        BusDevice::Dynamic(val)
    }
}
impl From<Box<dyn Device>> for BusDevice {
    fn from(val: Box<dyn Device>) -> Self {
        BusDevice::Dynamic(val)
    }
}
impl<D: Device + 'static> From<Rc<RefCell<D>>> for BusDevice {
    fn from(val: Rc<RefCell<D>>) -> Self {
        BusDevice::SharedRc(val)
    }
}
impl From<Rc<RefCell<dyn Device>>> for BusDevice {
    fn from(val: Rc<RefCell<dyn Device>>) -> Self {
        BusDevice::SharedRc(val)
    }
}
impl<D: Device + Send + 'static> From<Arc<Mutex<D>>> for BusDevice {
    fn from(val: Arc<Mutex<D>>) -> Self {
        BusDevice::SharedArc(val)
    }
}
impl From<Arc<Mutex<dyn Device + Send>>> for BusDevice {
    fn from(val: Arc<Mutex<dyn Device + Send>>) -> Self {
        BusDevice::SharedArc(val)
    }
}

impl Device for BusDevice {
    #[inline(always)]
    fn read_8(&mut self, offset: u64) -> Result<u8, EmuError> {
        match self {
            BusDevice::Ram(ram) => ram.read_8(offset),
            BusDevice::Dynamic(dev) => dev.read_8(offset),
            BusDevice::SharedRc(dev) => dev.borrow_mut().read_8(offset),
            BusDevice::SharedArc(dev) => dev.lock().unwrap().read_8(offset),
        }
    }

    #[inline(always)]
    fn write_8(&mut self, offset: u64, val: u8) -> Result<(), EmuError> {
        match self {
            BusDevice::Ram(ram) => ram.write_8(offset, val),
            BusDevice::Dynamic(dev) => dev.write_8(offset, val),
            BusDevice::SharedRc(dev) => dev.borrow_mut().write_8(offset, val),
            BusDevice::SharedArc(dev) => dev.lock().unwrap().write_8(offset, val),
        }
    }

    // Repeat this pattern exactly for read_16, write_16, read_32, write_32...
    #[inline(always)]
    fn read_32(&mut self, offset: u64) -> Result<u32, EmuError> {
        match self {
            BusDevice::Ram(ram) => ram.read_32(offset),
            BusDevice::Dynamic(dev) => dev.read_32(offset),
            BusDevice::SharedRc(dev) => dev.borrow_mut().read_32(offset),
            BusDevice::SharedArc(dev) => dev.lock().unwrap().read_32(offset),
        }
    }

    #[inline(always)]
    fn write_32(&mut self, offset: u64, val: u32) -> Result<(), EmuError> {
        match self {
            BusDevice::Ram(ram) => ram.write_32(offset, val),
            BusDevice::Dynamic(dev) => dev.write_32(offset, val),
            BusDevice::SharedRc(dev) => dev.borrow_mut().write_32(offset, val),
            BusDevice::SharedArc(dev) => dev.lock().unwrap().write_32(offset, val),
        }
    }
}

pub struct MemoryRegion {
    pub start: u64,
    pub size: u64,
    pub perms: Perms,
    pub device: BusDevice,
}

pub struct MemoryBus {
    regions: Vec<MemoryRegion>,
    // The TLB Cache (Stores the index of the last used region)
    last_region_idx: usize,
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryBus {
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
            last_region_idx: 0,
        }
    }

    pub fn map(&mut self, start: u64, size: u64, perms: Perms, device: BusDevice) {
        self.regions.push(MemoryRegion {
            start,
            size,
            perms,
            device,
        });
        self.regions.sort_by_key(|r| r.start);
        self.last_region_idx = 0; // Reset cache on map
    }

    pub fn unmap(&mut self, start: u64) {
        self.regions.retain(|r| r.start != start);
        self.last_region_idx = 0; // Invalidate the TLB cache
    }

    /// Helper to find the correct device and translate absolute address to offset
    #[inline(always)]
    pub fn resolve_mut(&mut self, addr: u64) -> Result<(&mut BusDevice, u64), EmuError> {
        // TLB Fast-Path (Hits 99% of the time for sequential execution)
        if let Some(&MemoryRegion { start, size, .. }) = self.regions.get(self.last_region_idx)
            && addr >= start
            && addr < start + size
        {
            return Ok((&mut self.regions[self.last_region_idx].device, addr - start));
        }

        // Fallback to Binary Search
        let idx = self
            .regions
            .binary_search_by(|r| {
                if addr < r.start {
                    std::cmp::Ordering::Greater
                } else if addr >= r.start + r.size {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .map_err(|_| EmuError::MemoryFault(addr))?;

        // Update the TLB
        self.last_region_idx = idx;
        let start = self.regions[idx].start;
        Ok((&mut self.regions[idx].device, addr - start))
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
