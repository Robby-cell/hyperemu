use crate::bus::MemoryBus;
use crate::config::CpuMode;
use crate::error::EmuError;
use crate::hook::HookRegistry;

pub trait Cpu {
    // Requires Self: Sized so it is excluded from `dyn Cpu` object trait generation
    fn init(mode: CpuMode) -> Result<Self, EmuError>
    where
        Self: Sized;

    fn step(&mut self, bus: &mut MemoryBus, hooks: &mut HookRegistry) -> Result<u32, EmuError>;
    fn read_reg(&self, reg_id: usize) -> Result<u64, EmuError>;
    fn write_reg(&mut self, reg_id: usize, val: u64) -> Result<(), EmuError>;
    fn pc(&self) -> u64;
}
