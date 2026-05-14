pub mod decode;
pub mod execute;
pub mod instr;
pub mod registers;

#[cfg(test)]
mod tests;

use crate::arch::x86::registers::*;
use crate::bus::MemoryBus;
use crate::config::CpuMode;
use crate::error::EmuError;
use crate::hook::HookRegistry;
use crate::interface::Cpu;

#[derive(Debug, Clone, Copy, Default)]
pub struct ExecOptions {
    pub run_code_hooks: bool,
}

pub struct X86Cpu {
    pub regs: [u32; 16],
    fetch_slice: *const u8,
    fetch_base: u32,
    fetch_len: u32,
}

impl X86Cpu {
    fn refresh_fetch_ptr(&mut self, bus: &mut MemoryBus) -> Result<(), EmuError> {
        let pc = self.regs[REG_EIP];
        let (device, offset) = bus.resolve_mut(pc as u64)?;
        if let crate::bus::BusDevice::Ram(ram) = device {
            self.fetch_slice = ram.data.as_ptr();
            self.fetch_base = (pc as u64 - offset) as _;
            self.fetch_len = ram.data.len() as u32;
            Ok(())
        } else {
            Err(EmuError::DeviceError("x86 requires RAM fetch".into()))
        }
    }

    #[inline(always)]
    fn execute_one_extended(
        &mut self,
        bus: &mut MemoryBus,
        hooks: &mut HookRegistry,
        options: &ExecOptions,
    ) -> Result<bool, EmuError> {
        let pc = self.regs[REG_EIP];

        if pc < self.fetch_base || pc >= self.fetch_base + self.fetch_len {
            self.refresh_fetch_ptr(bus)?;
        }

        if options.run_code_hooks {
            hooks.trigger_code(self, bus, pc as u64)?;
        }

        // Fetch using the DMA slice
        let offset = (pc - self.fetch_base) as usize;
        let available = (self.fetch_len - (pc - self.fetch_base)) as usize;
        let buffer = unsafe { std::slice::from_raw_parts(self.fetch_slice.add(offset), available) };

        let mut decoder = decode::X86Decoder::new(buffer);
        let instr = decoder.decode_instr();
        let bytes = decoder.consumed();

        // Standard x86: EIP advances by bytes consumed *before* execution logic
        let next_pc = pc.wrapping_add(bytes as u32);
        self.regs[REG_EIP] = next_pc;

        execute::execute_instr(self, instr, bus, hooks)?;

        // Return true if the instruction caused a branch (EIP was manually overwritten)
        Ok(self.regs[REG_EIP] != next_pc)
    }
}

impl Cpu for X86Cpu {
    fn init(_mode: CpuMode) -> Result<Self, EmuError> {
        let mut cpu = Self {
            regs: [0; 16],
            fetch_slice: std::ptr::null(),
            fetch_base: 0,
            fetch_len: 0,
        };
        cpu.regs[REG_EFLAGS] = EFlags::RSVD1.bits();
        Ok(cpu)
    }

    #[inline(always)]
    fn step(&mut self, bus: &mut MemoryBus, hooks: &mut HookRegistry) -> Result<(), EmuError> {
        let options = ExecOptions {
            run_code_hooks: !hooks.code_hooks.is_empty(),
        };
        self.execute_one_extended(bus, hooks, &options)?;
        Ok(())
    }

    #[inline(always)]
    fn step_batch(
        &mut self,
        bus: &mut MemoryBus,
        hooks: &mut HookRegistry,
        max: u64,
    ) -> Result<u64, EmuError> {
        let mut executed = 0;
        let options = ExecOptions {
            run_code_hooks: !hooks.code_hooks.is_empty(),
        };

        while executed < max {
            let mut branched = false;

            // Hard unroll block for LLVM
            for _ in 0..16 {
                if executed >= max {
                    break;
                }
                branched = self.execute_one_extended(bus, hooks, &options)?;
                executed += 1;
                if branched {
                    break;
                }
            }

            if branched {
                break;
            }
        }
        Ok(executed)
    }

    fn read_reg(&self, id: usize) -> Result<u64, EmuError> {
        Ok(self.regs[id] as u64)
    }
    fn write_reg(&mut self, id: usize, val: u64) -> Result<(), EmuError> {
        self.regs[id] = val as u32;
        Ok(())
    }
    fn pc(&self) -> u64 {
        self.regs[REG_EIP] as u64
    }
}
