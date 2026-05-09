use crate::emu::HyperEmu;
use crate::error::EmuError;

use gdbstub::common::Signal;
use gdbstub::target::ext::base::BaseOps;
use gdbstub::target::ext::base::singlethread::{
    SingleThreadBase, SingleThreadResume, SingleThreadResumeOps, SingleThreadSingleStep,
    SingleThreadSingleStepOps,
};
use gdbstub::target::{Target, TargetError, TargetResult};
use gdbstub_arch::arm::Armv4t;
use gdbstub_arch::arm::reg::ArmCoreRegs;

pub struct HyperEmuGdb<'a> {
    pub emu: &'a mut HyperEmu,
}

impl<'a> HyperEmuGdb<'a> {
    pub fn new(emu: &'a mut HyperEmu) -> Self {
        Self { emu }
    }
}

impl<'a> Target for HyperEmuGdb<'a> {
    type Arch = Armv4t;
    type Error = EmuError;

    #[inline(always)]
    fn base_ops(&mut self) -> BaseOps<'_, Self::Arch, Self::Error> {
        BaseOps::SingleThread(self)
    }
}

impl<'a> SingleThreadBase for HyperEmuGdb<'a> {
    fn read_registers(&mut self, regs: &mut ArmCoreRegs) -> TargetResult<(), Self> {
        for i in 0..16 {
            regs.r[i] = self.emu.reg_read(i).unwrap() as u32;
        }
        regs.cpsr = self.emu.reg_read(16).unwrap() as u32;
        Ok(())
    }

    fn write_registers(&mut self, regs: &ArmCoreRegs) -> TargetResult<(), Self> {
        for i in 0..16 {
            self.emu.reg_write(i, regs.r[i] as u64).unwrap();
        }
        self.emu.reg_write(16, regs.cpsr as u64).unwrap();
        Ok(())
    }

    fn read_addrs(&mut self, start_addr: u32, data: &mut [u8]) -> TargetResult<usize, Self> {
        match self.emu.bus.read_bytes(start_addr as u64, data) {
            Ok(_) => Ok(data.len()),
            Err(_) => Err(TargetError::NonFatal),
        }
    }

    fn write_addrs(&mut self, start_addr: u32, data: &[u8]) -> TargetResult<(), Self> {
        match self.emu.bus.write_bytes(start_addr as u64, data) {
            Ok(_) => Ok(()),
            Err(_) => Err(TargetError::NonFatal),
        }
    }

    #[inline(always)]
    fn support_resume(&mut self) -> Option<SingleThreadResumeOps<'_, Self>> {
        Some(self)
    }
}

impl<'a> SingleThreadResume for HyperEmuGdb<'a> {
    fn resume(&mut self, _signal: Option<Signal>) -> Result<(), Self::Error> {
        // In 0.7, the EventLoop drives execution outside of this trait.
        Ok(())
    }

    #[inline(always)]
    fn support_single_step(&mut self) -> Option<SingleThreadSingleStepOps<'_, Self>> {
        Some(self)
    }
}

impl<'a> SingleThreadSingleStep for HyperEmuGdb<'a> {
    fn step(&mut self, _signal: Option<Signal>) -> Result<(), Self::Error> {
        Ok(())
    }
}
