use crate::device::Device;
use crate::error::EmuError;

pub struct Ram {
    pub data: Vec<u8>,
}

impl Ram {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
    }
}

impl Device for Ram {
    #[inline(always)]
    fn read_8(&mut self, offset: u64) -> Result<u8, EmuError> {
        self.data
            .get(offset as usize)
            .copied()
            .ok_or(EmuError::MemoryFault(offset))
    }

    #[inline(always)]
    fn write_8(&mut self, offset: u64, val: u8) -> Result<(), EmuError> {
        if let Some(byte) = self.data.get_mut(offset as usize) {
            *byte = val;
            Ok(())
        } else {
            Err(EmuError::MemoryFault(offset))
        }
    }

    #[inline(always)]
    fn read_32(&mut self, offset: u64) -> Result<u32, EmuError> {
        let offset = offset as usize;
        // Grab exactly 4 bytes and cast them in a single hardware cycle
        if let Some(bytes) = self.data.get(offset..offset + 4) {
            Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
        } else {
            Err(EmuError::MemoryFault(offset as u64))
        }
    }

    #[inline(always)]
    fn write_32(&mut self, offset: u64, val: u32) -> Result<(), EmuError> {
        let offset = offset as usize;
        if let Some(bytes) = self.data.get_mut(offset..offset + 4) {
            bytes.copy_from_slice(&val.to_le_bytes());
            Ok(())
        } else {
            Err(EmuError::MemoryFault(offset as u64))
        }
    }

    #[inline(always)]
    fn read_16(&mut self, offset: u64) -> Result<u16, EmuError> {
        let offset = offset as usize;
        if let Some(bytes) = self.data.get(offset..offset + 2) {
            Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
        } else {
            Err(EmuError::MemoryFault(offset as u64))
        }
    }

    #[inline(always)]
    fn write_16(&mut self, offset: u64, val: u16) -> Result<(), EmuError> {
        let offset = offset as usize;
        if let Some(bytes) = self.data.get_mut(offset..offset + 2) {
            bytes.copy_from_slice(&val.to_le_bytes());
            Ok(())
        } else {
            Err(EmuError::MemoryFault(offset as u64))
        }
    }
}
