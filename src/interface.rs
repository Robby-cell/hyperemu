use crate::bus::MemoryBus;
use crate::config::CpuMode;
use crate::error::EmuError;
use crate::hook::HookRegistry;

pub trait Cpu {
    // Requires Self: Sized so it is excluded from `dyn Cpu` object trait generation
    fn init(mode: CpuMode) -> Result<Self, EmuError>
    where
        Self: Sized;

    /// Executes exactly one instruction.
    fn step(&mut self, bus: &mut MemoryBus, hooks: &mut HookRegistry) -> Result<(), EmuError>;

    /// Executes a batch of instructions for high performance.
    /// Returns the exact number of instructions executed before yielding or branching.
    fn step_batch(
        &mut self,
        bus: &mut MemoryBus,
        hooks: &mut HookRegistry,
        max_instrs: u32,
    ) -> Result<u32, EmuError>;

    fn read_reg(&self, reg_id: usize) -> Result<u64, EmuError>;

    fn write_reg(&mut self, reg_id: usize, val: u64) -> Result<(), EmuError>;

    fn pc(&self) -> u64;
}
