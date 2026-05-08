use crate::device::Device;
use crate::error::EmuError;

pub struct Ram {
    data: Vec<u8>,
}

impl Ram {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
    }
}

impl Device for Ram {
    fn read_8(&mut self, offset: u64) -> Result<u8, EmuError> {
        self.data
            .get(offset as usize)
            .copied()
            .ok_or(EmuError::MemoryFault(offset))
    }

    fn write_8(&mut self, offset: u64, val: u8) -> Result<(), EmuError> {
        if let Some(byte) = self.data.get_mut(offset as usize) {
            *byte = val;
            Ok(())
        } else {
            Err(EmuError::MemoryFault(offset))
        }
    }
}
