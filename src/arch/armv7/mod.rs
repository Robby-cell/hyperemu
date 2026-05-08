pub mod decode;
pub mod execute;
pub mod instr;
pub mod registers;

#[cfg(test)]
mod tests;

use crate::bus::MemoryBus;
use crate::config::CpuMode as GlobalCpuMode;
use crate::error::EmuError;
use crate::hook::HookRegistry;
use crate::interface::Cpu;
use registers::*;

pub struct Armv7Cpu {
    pub regs: [u32; 16],
    pub cpsr: u32,
    pub banked_sp: [u32; 6],
    pub banked_lr: [u32; 6],
    pub banked_spsr: [u32; 6],
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

impl Cpu for Armv7Cpu {
    fn init(_mode: GlobalCpuMode) -> Result<Self, EmuError> {
        Ok(Self {
            regs: [0; 16],
            cpsr: CpuModeBits::Supervisor as u32,
            banked_sp: [0; 6],
            banked_lr: [0; 6],
            banked_spsr: [0; 6],
        })
    }

    fn step(&mut self, bus: &mut MemoryBus, hooks: &mut HookRegistry) -> Result<(), EmuError> {
        let pc_val = self.regs[REG_PC];

        // Pass `self` (which gets coerced to &mut dyn Cpu) and the bus
        hooks.trigger_code(self, bus, pc_val as u64)?;

        let raw_instr = bus.read_32(pc_val as u64)?;
        self.regs[REG_PC] = self.regs[REG_PC].wrapping_add(4);

        let instr = decode::decode_arm(raw_instr);
        execute::execute_instr(self, instr, bus, hooks)
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
