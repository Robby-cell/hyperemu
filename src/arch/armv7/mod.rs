pub mod decode;
pub mod execute;
pub mod instr;
pub mod registers;

#[cfg(test)]
mod tests;

use crate::arch::armv7::instr::Instr;
use crate::bus::MemoryBus;
use crate::config::CpuMode as GlobalCpuMode;
use crate::error::EmuError;
use crate::hook::HookRegistry;
use crate::interface::Cpu;
use registers::*;

const DECODE_CACHE_SIZE: usize = 1024;

pub struct Armv7Cpu {
    pub regs: [u32; 16],
    pub cpsr: u32,
    pub banked_sp: [u32; 6],
    pub banked_lr: [u32; 6],
    pub banked_spsr: [u32; 6],

    // Instead of calling the Bus, we fetch directly from this slice
    fetch_slice: *const u8,
    fetch_base: u32,
    fetch_len: u32,

    // We store a simple, flat representation of the instruction
    cache_tags: Box<[u32]>,
    cache_instrs: Box<[Instr]>,
}

impl Armv7Cpu {
    // CPSR Getters

    pub fn get_n(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::N)
    }

    pub fn get_z(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::Z)
    }

    pub fn get_c(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::C)
    }

    pub fn get_v(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::V)
    }

    pub fn get_q(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::Q)
    }

    pub fn get_j(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::J)
    }

    pub fn get_e(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::E)
    }

    pub fn get_a(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::A)
    }

    pub fn get_i(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::I)
    }

    pub fn get_f(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::F)
    }

    pub fn get_t(&self) -> bool {
        Cpsr::from_bits_retain(self.cpsr).contains(Cpsr::T)
    }

    // CPSR Setters

    pub fn set_n(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::N.bits();
        } else {
            self.cpsr &= !Cpsr::N.bits();
        }
    }

    pub fn set_z(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::Z.bits();
        } else {
            self.cpsr &= !Cpsr::Z.bits();
        }
    }

    pub fn set_c(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::C.bits();
        } else {
            self.cpsr &= !Cpsr::C.bits();
        }
    }

    pub fn set_v(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::V.bits();
        } else {
            self.cpsr &= !Cpsr::V.bits();
        }
    }

    pub fn set_q(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::Q.bits();
        } else {
            self.cpsr &= !Cpsr::Q.bits();
        }
    }

    pub fn set_j(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::J.bits();
        } else {
            self.cpsr &= !Cpsr::J.bits();
        }
    }

    pub fn set_e(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::E.bits();
        } else {
            self.cpsr &= !Cpsr::E.bits();
        }
    }

    pub fn set_a(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::A.bits();
        } else {
            self.cpsr &= !Cpsr::A.bits();
        }
    }

    pub fn set_i(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::I.bits();
        } else {
            self.cpsr &= !Cpsr::I.bits();
        }
    }

    pub fn set_f(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::F.bits();
        } else {
            self.cpsr &= !Cpsr::F.bits();
        }
    }

    pub fn set_t(&mut self, val: bool) {
        if val {
            self.cpsr |= Cpsr::T.bits();
        } else {
            self.cpsr &= !Cpsr::T.bits();
        }
    }

    // Execution Mode

    pub fn current_mode(&self) -> CpuModeBits {
        CpuModeBits::from_u32(self.cpsr)
    }

    pub fn trigger_exception(&mut self, new_mode: CpuModeBits, target_pc: u32) {
        let current_cpsr = self.cpsr;

        let mode_idx = match new_mode {
            CpuModeBits::Supervisor => 0,
            CpuModeBits::Irq => 1,
            CpuModeBits::Fiq => 2,
            CpuModeBits::Abort => 3,
            CpuModeBits::Undefined => 4,
            _ => return, // Cannot trigger exception into User/System mode directly via hardware exceptions
        };

        // Bank the current CPSR into the target mode's SPSR
        self.banked_spsr[mode_idx] = current_cpsr;

        // Bank the return address into the target mode's LR
        self.banked_lr[mode_idx] = self.regs[REG_PC];

        // Switch modes and disable IRQs automatically on exception
        self.cpsr = (self.cpsr & !Cpsr::MODE_MASK.bits()) | (new_mode as u32);
        self.cpsr |= Cpsr::I.bits();

        // Jump to exception vector
        self.regs[REG_PC] = target_pc;
    }
}

impl Armv7Cpu {
    /// Forces the CPU to refresh its direct pointer to the code RAM.
    /// This is called only when the PC leaves the current 4KB page.
    fn refresh_fetch_ptr(&mut self, bus: &mut MemoryBus) -> Result<(), EmuError> {
        let pc = self.regs[REG_PC];
        // We ask the bus for a direct reference to the underlying RAM
        // If it's a BusDevice::Ram, we get the slice. If it's Custom, we fail fast.
        let (device, _offset) = bus.resolve_mut(pc as u64)?;

        if let crate::bus::BusDevice::Ram(ram) = device {
            self.fetch_slice = ram.data.as_ptr();
            self.fetch_base = pc;
            self.fetch_len = ram.data.len() as u32;
            Ok(())
        } else {
            Err(EmuError::DeviceError(
                "Cannot execute from non-RAM device".into(),
            ))
        }
    }
}

/// Options for highly specific execution control (e.g., skipping hooks for max speed)
#[derive(Debug, Clone, Copy)]
pub struct ExecOptions {
    pub run_code_hooks: bool,
}

impl Default for ExecOptions {
    fn default() -> Self {
        Self {
            run_code_hooks: true,
        }
    }
}

impl Armv7Cpu {
    /// Internal helper: Standard execution
    #[inline(always)]
    fn execute_one(
        &mut self,
        bus: &mut MemoryBus,
        hooks: &mut HookRegistry,
    ) -> Result<bool, EmuError> {
        self.execute_one_extended(
            bus,
            hooks,
            &ExecOptions {
                run_code_hooks: !hooks.code_hooks.is_empty(),
            },
        )
    }

    #[inline(always)]
    pub fn execute_one_extended(
        &mut self,
        bus: &mut MemoryBus,
        hooks: &mut HookRegistry,
        options: &ExecOptions,
    ) -> Result<bool, EmuError> {
        let pc_val = self.regs[REG_PC];

        // Refresh fetch pointer if we crossed a region boundary
        if pc_val < self.fetch_base || pc_val >= self.fetch_base + self.fetch_len {
            self.refresh_fetch_ptr(bus)?;
        }

        // Option: Conditionally skip code hooks for pure JIT speed!
        if options.run_code_hooks && !hooks.code_hooks.is_empty() {
            hooks.trigger_code(self, bus, pc_val as u64)?;
        }

        let raw_instr = unsafe {
            let offset = (pc_val - self.fetch_base) as usize;
            let ptr = self.fetch_slice.add(offset) as *const u32;
            ptr.read_unaligned()
        };

        if options.run_code_hooks {
            hooks.trigger_code(self, bus, pc_val as u64)?;
        }

        let next_pc = pc_val.wrapping_add(4);
        self.regs[REG_PC] = next_pc;

        let cache_idx = ((pc_val >> 2) as usize) & (DECODE_CACHE_SIZE - 1);
        let instr = if self.cache_tags[cache_idx] == pc_val {
            self.cache_instrs[cache_idx].clone()
        } else {
            let decoded = decode::decode_arm(raw_instr);
            self.cache_tags[cache_idx] = pc_val;
            self.cache_instrs[cache_idx] = decoded.clone();
            decoded
        };

        execute::execute_instr(self, instr, bus, hooks)?;

        Ok(self.regs[REG_PC] != next_pc)
    }
}

impl Cpu for Armv7Cpu {
    fn init(_mode: GlobalCpuMode) -> Result<Self, EmuError> {
        Ok(Self {
            regs: [0; 16],
            cpsr: CpuModeBits::Supervisor as u32,
            banked_sp: [0; 6],
            banked_lr: [0; 6],
            banked_spsr: [0; 6],

            fetch_slice: std::ptr::null(),
            fetch_base: 0,
            fetch_len: 0,

            cache_instrs: vec![Instr::Unknown(0); DECODE_CACHE_SIZE].into_boxed_slice(),
            cache_tags: vec![0xFFFFFFFF; DECODE_CACHE_SIZE].into_boxed_slice(),
        })
    }

    #[inline(always)]
    fn step(&mut self, bus: &mut MemoryBus, hooks: &mut HookRegistry) -> Result<(), EmuError> {
        self.execute_one(bus, hooks)?;
        Ok(())
    }

    #[inline(always)]
    fn step_batch(
        &mut self,
        bus: &mut MemoryBus,
        hooks: &mut HookRegistry,
        max_instrs: u32,
    ) -> Result<u32, EmuError> {
        let mut total_executed = 0;
        let run_code_hooks = !hooks.code_hooks.is_empty();

        let options = &ExecOptions { run_code_hooks };

        // 16-instruction hot loop
        while total_executed < max_instrs {
            let mut branched = false;

            // INNER HOT LOOP: Hardcoded to 16.
            // LLVM will completely unroll this into 16 sequential executions!
            for _ in 0..16 {
                if total_executed >= max_instrs {
                    break;
                }

                branched = self.execute_one_extended(bus, hooks, &options)?;
                total_executed += 1;

                if branched {
                    break;
                }
            }

            // If a branch broke the unrolled block, we yield back to the host.
            // This guarantees the host CPU's instruction pipeline stays perfectly clean.
            if branched {
                break;
            }
        }

        Ok(total_executed)
    }

    fn read_reg(&self, reg_id: usize) -> Result<u64, EmuError> {
        if reg_id < 16 {
            Ok(self.regs[reg_id] as u64)
        } else if reg_id == 16 {
            Ok(self.cpsr as u64)
        } else {
            Err(EmuError::InvalidRegister(reg_id))
        }
    }

    fn write_reg(&mut self, reg_id: usize, val: u64) -> Result<(), EmuError> {
        if reg_id < 16 {
            self.regs[reg_id] = val as u32;
            Ok(())
        } else if reg_id == 16 {
            self.cpsr = val as u32;
            Ok(())
        } else {
            Err(EmuError::InvalidRegister(reg_id))
        }
    }

    fn pc(&self) -> u64 {
        self.regs[REG_PC] as u64
    }
}
