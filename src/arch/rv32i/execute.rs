use super::RiscvCpu;
use super::instr::Instr;
use crate::bus::MemoryBus;
use crate::error::EmuError;
use crate::hook::HookRegistry;

#[inline(always)]
fn write_reg(cpu: &mut RiscvCpu, reg: u8, val: u32) {
    if reg != 0 {
        cpu.regs[reg as usize] = val;
    }
}

#[inline(always)]
pub fn execute_instr(
    cpu: &mut RiscvCpu,
    instr: Instr,
    bus: &mut MemoryBus,
    hooks: &mut HookRegistry,
) -> Result<(), EmuError> {
    let pc = cpu.pc;

    match instr {
        Instr::Lui { rd, imm } => write_reg(cpu, rd, imm),
        Instr::Auipc { rd, imm } => write_reg(cpu, rd, pc.wrapping_add(imm)),

        Instr::Jal { rd, imm } => {
            write_reg(cpu, rd, pc.wrapping_add(4));
            cpu.pc = (pc as i32).wrapping_add(imm) as u32;
        }
        Instr::Jalr { rd, rs1, imm } => {
            let target = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            write_reg(cpu, rd, pc.wrapping_add(4));
            cpu.pc = target & !1;
        }

        Instr::Beq { rs1, rs2, imm } => {
            if cpu.regs[rs1 as usize] == cpu.regs[rs2 as usize] {
                cpu.pc = (pc as i32).wrapping_add(imm) as u32;
            }
        }
        Instr::Bne { rs1, rs2, imm } => {
            if cpu.regs[rs1 as usize] != cpu.regs[rs2 as usize] {
                cpu.pc = (pc as i32).wrapping_add(imm) as u32;
            }
        }
        Instr::Blt { rs1, rs2, imm } => {
            if (cpu.regs[rs1 as usize] as i32) < (cpu.regs[rs2 as usize] as i32) {
                cpu.pc = (pc as i32).wrapping_add(imm) as u32;
            }
        }
        Instr::Bge { rs1, rs2, imm } => {
            if (cpu.regs[rs1 as usize] as i32) >= (cpu.regs[rs2 as usize] as i32) {
                cpu.pc = (pc as i32).wrapping_add(imm) as u32;
            }
        }
        Instr::Bltu { rs1, rs2, imm } => {
            if cpu.regs[rs1 as usize] < cpu.regs[rs2 as usize] {
                cpu.pc = (pc as i32).wrapping_add(imm) as u32;
            }
        }
        Instr::Bgeu { rs1, rs2, imm } => {
            if cpu.regs[rs1 as usize] >= cpu.regs[rs2 as usize] {
                cpu.pc = (pc as i32).wrapping_add(imm) as u32;
            }
        }

        Instr::Lb { rd, rs1, imm } => {
            let addr = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            let val = (bus.read_8(addr as u64)? as i8) as i32 as u32;
            write_reg(cpu, rd, val);
        }
        Instr::Lh { rd, rs1, imm } => {
            let addr = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            let val = (bus.read_16(addr as u64)? as i16) as i32 as u32;
            write_reg(cpu, rd, val);
        }
        Instr::Lw { rd, rs1, imm } => {
            let addr = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            let val = bus.read_32(addr as u64)?;
            write_reg(cpu, rd, val);
        }
        Instr::Lbu { rd, rs1, imm } => {
            let addr = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            let val = bus.read_8(addr as u64)? as u32;
            write_reg(cpu, rd, val);
        }
        Instr::Lhu { rd, rs1, imm } => {
            let addr = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            let val = bus.read_16(addr as u64)? as u32;
            write_reg(cpu, rd, val);
        }

        Instr::Sb { rs1, rs2, imm } => {
            let addr = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            bus.write_8(addr as u64, cpu.regs[rs2 as usize] as u8)?;
        }
        Instr::Sh { rs1, rs2, imm } => {
            let addr = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            bus.write_16(addr as u64, cpu.regs[rs2 as usize] as u16)?;
        }
        Instr::Sw { rs1, rs2, imm } => {
            let addr = (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32;
            bus.write_32(addr as u64, cpu.regs[rs2 as usize])?;
        }

        Instr::Addi { rd, rs1, imm } => write_reg(
            cpu,
            rd,
            (cpu.regs[rs1 as usize] as i32).wrapping_add(imm) as u32,
        ),
        Instr::Slti { rd, rs1, imm } => write_reg(
            cpu,
            rd,
            if (cpu.regs[rs1 as usize] as i32) < imm {
                1
            } else {
                0
            },
        ),
        Instr::Sltiu { rd, rs1, imm } => write_reg(
            cpu,
            rd,
            if cpu.regs[rs1 as usize] < (imm as u32) {
                1
            } else {
                0
            },
        ),
        Instr::Xori { rd, rs1, imm } => write_reg(cpu, rd, cpu.regs[rs1 as usize] ^ (imm as u32)),
        Instr::Ori { rd, rs1, imm } => write_reg(cpu, rd, cpu.regs[rs1 as usize] | (imm as u32)),
        Instr::Andi { rd, rs1, imm } => write_reg(cpu, rd, cpu.regs[rs1 as usize] & (imm as u32)),
        Instr::Slli { rd, rs1, shamt } => write_reg(cpu, rd, cpu.regs[rs1 as usize] << shamt),
        Instr::Srli { rd, rs1, shamt } => write_reg(cpu, rd, cpu.regs[rs1 as usize] >> shamt),
        Instr::Srai { rd, rs1, shamt } => {
            write_reg(cpu, rd, ((cpu.regs[rs1 as usize] as i32) >> shamt) as u32)
        }

        Instr::Add { rd, rs1, rs2 } => write_reg(
            cpu,
            rd,
            cpu.regs[rs1 as usize].wrapping_add(cpu.regs[rs2 as usize]),
        ),
        Instr::Sub { rd, rs1, rs2 } => write_reg(
            cpu,
            rd,
            cpu.regs[rs1 as usize].wrapping_sub(cpu.regs[rs2 as usize]),
        ),
        Instr::Sll { rd, rs1, rs2 } => write_reg(
            cpu,
            rd,
            cpu.regs[rs1 as usize] << (cpu.regs[rs2 as usize] & 0x1F),
        ),
        Instr::Slt { rd, rs1, rs2 } => write_reg(
            cpu,
            rd,
            if (cpu.regs[rs1 as usize] as i32) < (cpu.regs[rs2 as usize] as i32) {
                1
            } else {
                0
            },
        ),
        Instr::Sltu { rd, rs1, rs2 } => write_reg(
            cpu,
            rd,
            if cpu.regs[rs1 as usize] < cpu.regs[rs2 as usize] {
                1
            } else {
                0
            },
        ),
        Instr::Xor { rd, rs1, rs2 } => {
            write_reg(cpu, rd, cpu.regs[rs1 as usize] ^ cpu.regs[rs2 as usize])
        }
        Instr::Srl { rd, rs1, rs2 } => write_reg(
            cpu,
            rd,
            cpu.regs[rs1 as usize] >> (cpu.regs[rs2 as usize] & 0x1F),
        ),
        Instr::Sra { rd, rs1, rs2 } => write_reg(
            cpu,
            rd,
            ((cpu.regs[rs1 as usize] as i32) >> (cpu.regs[rs2 as usize] & 0x1F)) as u32,
        ),
        Instr::Or { rd, rs1, rs2 } => {
            write_reg(cpu, rd, cpu.regs[rs1 as usize] | cpu.regs[rs2 as usize])
        }
        Instr::And { rd, rs1, rs2 } => {
            write_reg(cpu, rd, cpu.regs[rs1 as usize] & cpu.regs[rs2 as usize])
        }

        Instr::Ecall => {
            let handled = hooks.trigger_interrupt(cpu, bus, 0)?;
            if !handled {
                return Err(EmuError::NotImplemented("Unhandled RISC-V ECALL"));
            }
        }
        Instr::Ebreak => return Err(EmuError::Breakpoint(3)),
        Instr::Unknown(op) => return Err(EmuError::InvalidInstruction(op as u64)),
    }
    Ok(())
}
