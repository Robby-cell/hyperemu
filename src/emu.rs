use crate::bus::{BusDevice, MemoryBus, Perms};
use crate::config::{Arch, CpuMode};
use crate::error::EmuError;
use crate::hook::HookRegistry;
use crate::interface::Cpu;
use crate::loader;

pub struct HyperEmu {
    pub cpu: Box<dyn Cpu>,
    pub bus: MemoryBus,
    pub hooks: HookRegistry,
}

impl HyperEmu {
    pub fn new(arch: Arch, mode: CpuMode) -> Result<Self, EmuError> {
        let cpu: Box<dyn Cpu> = match arch {
            Arch::Armv7 => {
                #[cfg(feature = "armv7")]
                {
                    Box::new(crate::arch::armv7::Armv7Cpu::init(mode)?)
                }
                #[cfg(not(feature = "armv7"))]
                {
                    return Err(EmuError::NotImplemented("ARMv7 CPU not enabled"));
                }
            }
            Arch::X86 => {
                return Err(EmuError::NotImplemented("x86 CPU not supported"));
            }
        };

        Ok(Self {
            cpu,
            bus: MemoryBus::new(),
            hooks: HookRegistry::new(),
        })
    }

    pub fn mem_map(&mut self, start: u64, size: u64, perms: Perms, device: BusDevice) {
        self.bus.map(start, size, perms, device);
    }

    pub fn reg_read(&self, reg_id: usize) -> Result<u64, EmuError> {
        self.cpu.read_reg(reg_id)
    }

    pub fn reg_write(&mut self, reg_id: usize, val: u64) -> Result<(), EmuError> {
        self.cpu.write_reg(reg_id, val)
    }

    /// Executes one CPU instruction.
    pub fn step(&mut self) -> Result<(), EmuError> {
        // By passing `&mut self.bus` and `&mut self.hooks`, we temporarily lend
        // access without tangling lifetimes or using RefCell.
        self.cpu.step(&mut self.bus, &mut self.hooks)
    }

    /// Starts execution from a given point until an error or intentional stop.
    pub fn start(&mut self, entry_point: u64, sp: u64) -> Result<(), EmuError> {
        log::info!("Starting execution at 0x{:016X}", entry_point);

        // Setup initial state (15 = PC, 13 = SP for standard ARM)
        self.reg_write(15, entry_point)?;
        self.reg_write(13, sp)?;

        loop {
            self.step()?;
        }
    }
}

impl HyperEmu {
    #[cfg(feature = "elf")]
    pub fn load_elf(&mut self, data: &[u8]) -> Result<u64, EmuError> {
        let info = loader::elf::load_elf(&mut self.bus, data)?;
        Ok(info.entry_point)
    }

    /// Loads a raw binary into memory. Memory must be mapped beforehand.
    pub fn load_raw(&mut self, data: &[u8], load_addr: u64) -> Result<(), EmuError> {
        loader::raw::load_raw(&mut self.bus, data, load_addr)
    }
}
