pub mod decode;
pub mod execute;
pub mod instr;
pub mod registers;

#[cfg(test)]
mod tests;

use crate::bus::MemoryBus;
use crate::config::CpuMode;
use crate::error::EmuError;
use crate::hook::HookRegistry;
use crate::interface::Cpu;
use instr::Instr;

const DECODE_CACHE_SIZE: usize = 1024;

#[derive(Debug, Clone, Copy, Default)]
pub struct ExecOptions {
    pub run_code_hooks: bool,
}

pub struct RiscvCpu {
    pub regs: [u32; 32],
    pub pc: u32,

    fetch_slice: *const u8,
    fetch_base: u32,
    fetch_len: u32,

    cache_tags: Box<[u32]>,
    cache_raw: Box<[u32]>,
    cache_instrs: Box<[Instr]>,
}

impl RiscvCpu {
    fn refresh_fetch_ptr(&mut self, bus: &mut MemoryBus) -> Result<(), EmuError> {
        let (device, offset) = bus.resolve_mut(self.pc as u64)?;
        if let crate::bus::BusDevice::Ram(ram) = device {
            self.fetch_slice = ram.data.as_ptr();
            self.fetch_base = (self.pc as u64 - offset) as _;
            self.fetch_len = ram.data.len() as u32;
            Ok(())
        } else {
            Err(EmuError::DeviceError("Execution requires RAM".into()))
        }
    }

    #[inline(always)]
    fn execute_one_extended(
        &mut self,
        bus: &mut MemoryBus,
        hooks: &mut HookRegistry,
        opts: &ExecOptions,
    ) -> Result<bool, EmuError> {
        let current_pc = self.pc;

        if current_pc < self.fetch_base
            || current_pc.wrapping_add(4) > self.fetch_base + self.fetch_len
        {
            self.refresh_fetch_ptr(bus)?;
        }

        if opts.run_code_hooks {
            hooks.trigger_code(self, bus, current_pc as u64)?;
        }

        let raw_instr = unsafe {
            let offset = (current_pc - self.fetch_base) as usize;
            let ptr = self.fetch_slice.add(offset) as *const u32;
            u32::from_le(ptr.read_unaligned())
        };

        let next_pc = current_pc.wrapping_add(4);
        self.pc = next_pc;

        let cache_idx = ((current_pc >> 2) as usize) & (DECODE_CACHE_SIZE - 1);

        let instr =
            if self.cache_tags[cache_idx] == current_pc && self.cache_raw[cache_idx] == raw_instr {
                self.cache_instrs[cache_idx].clone()
            } else {
                let decoded = decode::decode_riscv(raw_instr);
                self.cache_tags[cache_idx] = current_pc;
                self.cache_raw[cache_idx] = raw_instr;
                self.cache_instrs[cache_idx] = decoded.clone();
                decoded
            };

        match execute::execute_instr(self, instr, bus, hooks) {
            Ok(()) => Ok(self.pc != next_pc),
            Err(e) => {
                self.pc = current_pc; // Rewind on fault
                Err(e)
            }
        }
    }
}

impl Cpu for RiscvCpu {
    fn init(_mode: CpuMode) -> Result<Self, EmuError> {
        Ok(Self {
            regs: [0; 32],
            pc: 0,
            fetch_slice: std::ptr::null(),
            fetch_base: 0,
            fetch_len: 0,
            cache_instrs: vec![Instr::Unknown(0); DECODE_CACHE_SIZE].into_boxed_slice(),
            cache_tags: vec![0xFFFFFFFF; DECODE_CACHE_SIZE].into_boxed_slice(),
            cache_raw: vec![0; DECODE_CACHE_SIZE].into_boxed_slice(),
        })
    }

    #[inline(always)]
    fn step(&mut self, bus: &mut MemoryBus, hooks: &mut HookRegistry) -> Result<(), EmuError> {
        self.execute_one_extended(
            bus,
            hooks,
            &ExecOptions {
                run_code_hooks: !hooks.code_hooks.is_empty(),
            },
        )?;
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
        let opts = ExecOptions {
            run_code_hooks: !hooks.code_hooks.is_empty(),
        };

        while executed < max {
            let mut branched = false;
            for _ in 0..16 {
                if executed >= max {
                    break;
                }
                branched = self.execute_one_extended(bus, hooks, &opts)?;
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

    fn read_reg(&self, reg_id: usize) -> Result<u64, EmuError> {
        if reg_id < 32 {
            Ok(self.regs[reg_id] as u64)
        } else if reg_id == 32 {
            Ok(self.pc as u64)
        } else {
            Err(EmuError::InvalidRegister(reg_id))
        }
    }

    fn write_reg(&mut self, reg_id: usize, val: u64) -> Result<(), EmuError> {
        if reg_id == 0 {
            Ok(())
        }
        // Hardwired 0
        else if reg_id < 32 {
            self.regs[reg_id] = val as u32;
            Ok(())
        } else if reg_id == 32 {
            self.pc = val as u32;
            Ok(())
        } else {
            Err(EmuError::InvalidRegister(reg_id))
        }
    }

    fn pc(&self) -> u64 {
        self.pc as u64
    }
}
