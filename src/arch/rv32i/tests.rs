use super::RiscvCpu;
use super::execute::execute_instr;
use super::instr::Instr;
use crate::bus::{MemoryBus, Perms};
use crate::config::CpuMode;
use crate::device::ram::Ram;
use crate::hook::HookRegistry;
use crate::interface::Cpu;

fn setup_test_env() -> (RiscvCpu, MemoryBus, HookRegistry) {
    let cpu = RiscvCpu::init(CpuMode::MODE_32).unwrap();
    let mut bus = MemoryBus::new();
    let hooks = HookRegistry::new();
    bus.map(0x1000, 0x1000, Perms::RWX, Ram::new(0x1000).into());
    (cpu, bus, hooks)
}

#[test]
fn test_riscv_hardwired_zero() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // ADDI x0, x0, 5
    let instr = Instr::Addi {
        rd: 0,
        rs1: 0,
        imm: 5,
    };
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 0, "Register x0 must always remain 0!");
}

#[test]
fn test_riscv_arithmetic() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    cpu.regs[1] = 10;
    cpu.regs[2] = 20;

    // SUB x3, x1, x2 (10 - 20)
    let instr = Instr::Sub {
        rd: 3,
        rs1: 1,
        rs2: 2,
    };
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[3], 0xFFFFFFF6, "SUB failed or failed to wrap");
}

#[test]
fn test_riscv_branches() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();
    cpu.pc = 0x1000;
    cpu.regs[1] = 5;
    cpu.regs[2] = 10;

    // BLT x1, x2, 0x20
    let instr = Instr::Blt {
        rs1: 1,
        rs2: 2,
        imm: 0x20,
    };
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.pc, 0x1020, "BLT should branch since 5 < 10");
}
