use super::Armv7Cpu;
use super::instr::{Condition, Instr, Operand2, Shift, ShiftType};
use super::registers::*;
use crate::bus::MemoryBus;
use crate::error::EmuError;
use crate::hook::HookRegistry;

pub fn execute_instr(
    cpu: &mut Armv7Cpu,
    instr: Instr,
    bus: &mut MemoryBus,
    hooks: &mut HookRegistry,
) -> Result<(), EmuError> {
    let cond = instr_condition(&instr);

    if !check_condition(cpu, cond) {
        return Ok(());
    }

    match instr {
        // Data Processing
        Instr::And { s, rd, rn, op2, .. } => {
            let (val_op2, carry_out) = evaluate_operand2(cpu, op2);
            let result = cpu.reg(rn) & val_op2;
            cpu.regs[rd as usize] = result;
            if s {
                set_logic_flags(cpu, result, carry_out);
            }
        }
        Instr::Eor { s, rd, rn, op2, .. } => {
            let (val_op2, carry_out) = evaluate_operand2(cpu, op2);
            let result = cpu.reg(rn) ^ val_op2;
            cpu.regs[rd as usize] = result;
            if s {
                set_logic_flags(cpu, result, carry_out);
            }
        }
        Instr::Sub { s, rd, rn, op2, .. } => {
            let (val_op2, _) = evaluate_operand2(cpu, op2);
            let result = do_sub(cpu, cpu.reg(rn), val_op2, s);
            cpu.regs[rd as usize] = result;
        }
        Instr::Rsb { s, rd, rn, op2, .. } => {
            let (val_op2, _) = evaluate_operand2(cpu, op2);
            let result = do_sub(cpu, val_op2, cpu.reg(rn), s);
            cpu.regs[rd as usize] = result;
        }
        Instr::Add { s, rd, rn, op2, .. } => {
            let (val_op2, _) = evaluate_operand2(cpu, op2);
            let result = do_add(cpu, cpu.reg(rn), val_op2, s);
            cpu.regs[rd as usize] = result;
        }
        Instr::Adc { s, rd, rn, op2, .. } => {
            let (val_op2, _) = evaluate_operand2(cpu, op2);
            let carry_val = if cpu.get_c() { 1 } else { 0 };
            let result = do_add_carry(cpu, cpu.reg(rn), val_op2, carry_val, s);
            cpu.regs[rd as usize] = result;
        }
        Instr::Sbc { s, rd, rn, op2, .. } => {
            let (val_op2, _) = evaluate_operand2(cpu, op2);
            let not_carry = if cpu.get_c() { 0 } else { 1 };
            let result = do_sub_carry(cpu, cpu.reg(rn), val_op2, not_carry, s);
            cpu.regs[rd as usize] = result;
        }
        Instr::Rsc { s, rd, rn, op2, .. } => {
            let (val_op2, _) = evaluate_operand2(cpu, op2);
            let not_carry = if cpu.get_c() { 0 } else { 1 };
            let result = do_sub_carry(cpu, val_op2, cpu.reg(rn), not_carry, s);
            cpu.regs[rd as usize] = result;
        }
        Instr::Tst { rn, op2, .. } => {
            let (val_op2, carry_out) = evaluate_operand2(cpu, op2);
            let result = cpu.reg(rn) & val_op2;
            set_logic_flags(cpu, result, carry_out);
        }
        Instr::Teq { rn, op2, .. } => {
            let (val_op2, carry_out) = evaluate_operand2(cpu, op2);
            let result = cpu.reg(rn) ^ val_op2;
            set_logic_flags(cpu, result, carry_out);
        }
        Instr::Cmp { rn, op2, .. } => {
            let (val_op2, _) = evaluate_operand2(cpu, op2);
            do_sub(cpu, cpu.reg(rn), val_op2, true);
        }
        Instr::Cmn { rn, op2, .. } => {
            let (val_op2, _) = evaluate_operand2(cpu, op2);
            do_add(cpu, cpu.reg(rn), val_op2, true);
        }
        Instr::Orr { s, rd, rn, op2, .. } => {
            let (val_op2, carry_out) = evaluate_operand2(cpu, op2);
            let result = cpu.reg(rn) | val_op2;
            cpu.regs[rd as usize] = result;
            if s {
                set_logic_flags(cpu, result, carry_out);
            }
        }
        Instr::Mov { s, rd, op2, .. } => {
            let (val_op2, carry_out) = evaluate_operand2(cpu, op2);
            cpu.regs[rd as usize] = val_op2;
            if s {
                set_logic_flags(cpu, val_op2, carry_out);
            }
        }
        Instr::Bic { s, rd, rn, op2, .. } => {
            let (val_op2, carry_out) = evaluate_operand2(cpu, op2);
            let result = cpu.reg(rn) & !val_op2;
            cpu.regs[rd as usize] = result;
            if s {
                set_logic_flags(cpu, result, carry_out);
            }
        }
        Instr::Mvn { s, rd, op2, .. } => {
            let (val_op2, carry_out) = evaluate_operand2(cpu, op2);
            let result = !val_op2;
            cpu.regs[rd as usize] = result;
            if s {
                set_logic_flags(cpu, result, carry_out);
            }
        }

        Instr::Movw { rd, imm16, .. } => {
            cpu.regs[rd as usize] = imm16 as u32;
        }
        Instr::Movt { rd, imm16, .. } => {
            let current = cpu.reg(rd);
            cpu.regs[rd as usize] = (current & 0x0000FFFF) | ((imm16 as u32) << 16);
        }

        // Status Register
        Instr::Mrs { rd, use_spsr, .. } => {
            let mode = cpu.current_mode();
            let mode_idx = get_bank_index(mode);
            cpu.regs[rd as usize] = if use_spsr && mode_idx.is_some() {
                cpu.banked_spsr[mode_idx.unwrap()]
            } else {
                cpu.cpsr
            };
        }
        Instr::Msr {
            use_spsr,
            mask,
            op2,
            ..
        } => {
            let (val, _) = evaluate_operand2(cpu, op2);
            let mut byte_mask: u32 = 0;
            if (mask & 1) != 0 {
                byte_mask |= 0x000000FF;
            }
            if (mask & 2) != 0 {
                byte_mask |= 0x0000FF00;
            }
            if (mask & 4) != 0 {
                byte_mask |= 0x00FF0000;
            }
            if (mask & 8) != 0 {
                byte_mask |= 0xFF000000;
            }

            let mode = cpu.current_mode();
            let mode_idx = get_bank_index(mode);

            if use_spsr && mode_idx.is_some() {
                let current = cpu.banked_spsr[mode_idx.unwrap()];
                cpu.banked_spsr[mode_idx.unwrap()] = (current & !byte_mask) | (val & byte_mask);
            } else {
                let current = cpu.cpsr;
                cpu.cpsr = (current & !byte_mask) | (val & byte_mask);
            }
        }

        // Multiplies
        Instr::Mul { s, rd, rm, rs, .. } => {
            let result = cpu.reg(rm).wrapping_mul(cpu.reg(rs));
            cpu.regs[rd as usize] = result;
            if s {
                cpu.set_n((result >> 31) == 1);
                cpu.set_z(result == 0);
            }
        }
        Instr::Mla {
            s, rd, rm, rs, rn, ..
        } => {
            let mul = cpu.reg(rm).wrapping_mul(cpu.reg(rs));
            let result = mul.wrapping_add(cpu.reg(rn));
            cpu.regs[rd as usize] = result;
            if s {
                cpu.set_n((result >> 31) == 1);
                cpu.set_z(result == 0);
            }
        }
        Instr::Umull {
            s,
            rd_lo,
            rd_hi,
            rm,
            rs,
            ..
        } => {
            let result = (cpu.reg(rm) as u64).wrapping_mul(cpu.reg(rs) as u64);
            cpu.regs[rd_lo as usize] = result as u32;
            cpu.regs[rd_hi as usize] = (result >> 32) as u32;
            if s {
                cpu.set_n((result >> 63) == 1);
                cpu.set_z(result == 0);
            }
        }
        Instr::Umlal {
            s,
            rd_lo,
            rd_hi,
            rm,
            rs,
            ..
        } => {
            let mul = (cpu.reg(rm) as u64).wrapping_mul(cpu.reg(rs) as u64);
            let accum = (cpu.reg(rd_lo) as u64) | ((cpu.reg(rd_hi) as u64) << 32);
            let result = mul.wrapping_add(accum);
            cpu.regs[rd_lo as usize] = result as u32;
            cpu.regs[rd_hi as usize] = (result >> 32) as u32;
            if s {
                cpu.set_n((result >> 63) == 1);
                cpu.set_z(result == 0);
            }
        }
        Instr::Smull {
            s,
            rd_lo,
            rd_hi,
            rm,
            rs,
            ..
        } => {
            let result = (cpu.reg(rm) as i32 as i64).wrapping_mul(cpu.reg(rs) as i32 as i64) as u64;
            cpu.regs[rd_lo as usize] = result as u32;
            cpu.regs[rd_hi as usize] = (result >> 32) as u32;
            if s {
                cpu.set_n((result >> 63) == 1);
                cpu.set_z(result == 0);
            }
        }
        Instr::Smlal {
            s,
            rd_lo,
            rd_hi,
            rm,
            rs,
            ..
        } => {
            let mul = (cpu.reg(rm) as i32 as i64).wrapping_mul(cpu.reg(rs) as i32 as i64) as u64;
            let accum = (cpu.reg(rd_lo) as u64) | ((cpu.reg(rd_hi) as u64) << 32);
            let result = mul.wrapping_add(accum);
            cpu.regs[rd_lo as usize] = result as u32;
            cpu.regs[rd_hi as usize] = (result >> 32) as u32;
            if s {
                cpu.set_n((result >> 63) == 1);
                cpu.set_z(result == 0);
            }
        }

        // Bit Manipulation & Extension
        Instr::Bfc { rd, lsb, width, .. } => {
            let mask = ((1u64.wrapping_shl(width)).wrapping_sub(1) as u32).wrapping_shl(lsb);
            cpu.regs[rd as usize] &= !mask;
        }
        Instr::Bfi {
            rd, rn, lsb, width, ..
        } => {
            let mask = ((1u64.wrapping_shl(width)).wrapping_sub(1) as u32).wrapping_shl(lsb);
            let val = (cpu.reg(rn) & ((1u64.wrapping_shl(width)).wrapping_sub(1) as u32))
                .wrapping_shl(lsb);
            cpu.regs[rd as usize] = (cpu.reg(rd) & !mask) | val;
        }
        Instr::Ubfx {
            rd, rn, lsb, width, ..
        } => {
            let val = (cpu.reg(rn).wrapping_shr(lsb))
                & ((1u64.wrapping_shl(width)).wrapping_sub(1) as u32);
            cpu.regs[rd as usize] = val;
        }
        Instr::Sbfx {
            rd, rn, lsb, width, ..
        } => {
            let shift_up = 32 - lsb - width;
            let val = ((cpu.reg(rn) as i32).wrapping_shl(shift_up)).wrapping_shr(32 - width);
            cpu.regs[rd as usize] = val.cast_unsigned();
        }
        Instr::Rev { rd, rm, .. } => {
            cpu.regs[rd as usize] = cpu.reg(rm).swap_bytes();
        }
        Instr::Rev16 { rd, rm, .. } => {
            let val = cpu.reg(rm);
            cpu.regs[rd as usize] = ((val & 0xFF) << 8)
                | ((val & 0xFF00) >> 8)
                | ((val & 0xFF0000) << 8)
                | ((val & 0xFF000000) >> 8);
        }
        Instr::Revsh { rd, rm, .. } => {
            let val = cpu.reg(rm);
            let half = ((val & 0xFF) << 8) | ((val & 0xFF00) >> 8);
            cpu.regs[rd as usize] = (half as i16 as i32).cast_unsigned();
        }
        Instr::Sxtb {
            rd, rm, rot, rn, ..
        } => {
            let rotated = cpu.reg(rm).rotate_right((rot * 8) as u32);
            let val = (rotated as i8 as i32).cast_unsigned();
            cpu.regs[rd as usize] = if let Some(n) = rn {
                cpu.reg(n).wrapping_add(val)
            } else {
                val
            };
        }
        Instr::Sxth {
            rd, rm, rot, rn, ..
        } => {
            let rotated = cpu.reg(rm).rotate_right((rot * 8) as u32);
            let val = (rotated as i16 as i32).cast_unsigned();
            cpu.regs[rd as usize] = if let Some(n) = rn {
                cpu.reg(n).wrapping_add(val)
            } else {
                val
            };
        }
        Instr::Uxtb {
            rd, rm, rot, rn, ..
        } => {
            let rotated = cpu.reg(rm).rotate_right((rot * 8) as u32);
            let val = rotated & 0xFF;
            cpu.regs[rd as usize] = if let Some(n) = rn {
                cpu.reg(n).wrapping_add(val)
            } else {
                val
            };
        }
        Instr::Uxth {
            rd, rm, rot, rn, ..
        } => {
            let rotated = cpu.reg(rm).rotate_right((rot * 8) as u32);
            let val = rotated & 0xFFFF;
            cpu.regs[rd as usize] = if let Some(n) = rn {
                cpu.reg(n).wrapping_add(val)
            } else {
                val
            };
        }
        Instr::Clz { rd, rm, .. } => {
            cpu.regs[rd as usize] = cpu.reg(rm).leading_zeros();
        }

        // Memory (Single)
        Instr::Ldr {
            rd,
            rn,
            offset,
            pre,
            writeback,
            up,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let (off_val, _) = evaluate_operand2(cpu, offset);

            if pre {
                addr = if up {
                    addr.wrapping_add(off_val)
                } else {
                    addr.wrapping_sub(off_val)
                };
            }
            cpu.regs[rd as usize] = cpu.read_data_32(bus, addr)?;
            if writeback || !pre {
                let final_addr = if !pre {
                    if up {
                        addr.wrapping_add(off_val)
                    } else {
                        addr.wrapping_sub(off_val)
                    }
                } else {
                    addr
                };
                if rn as usize != REG_PC {
                    cpu.regs[rn as usize] = final_addr;
                }
            }
        }
        Instr::Str {
            rd,
            rn,
            offset,
            pre,
            writeback,
            up,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let (off_val, _) = evaluate_operand2(cpu, offset);
            let val = cpu.reg(rd);

            if pre {
                addr = if up {
                    addr.wrapping_add(off_val)
                } else {
                    addr.wrapping_sub(off_val)
                };
            }
            cpu.write_data_32(bus, addr, val)?;

            if writeback || !pre {
                let final_addr = if !pre {
                    if up {
                        addr.wrapping_add(off_val)
                    } else {
                        addr.wrapping_sub(off_val)
                    }
                } else {
                    addr
                };
                if rn as usize != REG_PC {
                    cpu.regs[rn as usize] = final_addr;
                }
            }
        }
        Instr::Ldrb {
            rd,
            rn,
            offset,
            pre,
            writeback,
            up,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let (off_val, _) = evaluate_operand2(cpu, offset);

            if pre {
                addr = if up {
                    addr.wrapping_add(off_val)
                } else {
                    addr.wrapping_sub(off_val)
                };
            }
            cpu.regs[rd as usize] = cpu.read_data_8(bus, addr)? as u32;
            if writeback || !pre {
                let final_addr = if !pre {
                    if up {
                        addr.wrapping_add(off_val)
                    } else {
                        addr.wrapping_sub(off_val)
                    }
                } else {
                    addr
                };
                if rn as usize != REG_PC {
                    cpu.regs[rn as usize] = final_addr;
                }
            }
        }
        Instr::Strb {
            rd,
            rn,
            offset,
            pre,
            writeback,
            up,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let (off_val, _) = evaluate_operand2(cpu, offset);
            let val = cpu.reg(rd);

            if pre {
                addr = if up {
                    addr.wrapping_add(off_val)
                } else {
                    addr.wrapping_sub(off_val)
                };
            }
            cpu.write_data_8(bus, addr, (val & 0xFF) as u8)?;

            if writeback || !pre {
                let final_addr = if !pre {
                    if up {
                        addr.wrapping_add(off_val)
                    } else {
                        addr.wrapping_sub(off_val)
                    }
                } else {
                    addr
                };
                if rn as usize != REG_PC {
                    cpu.regs[rn as usize] = final_addr;
                }
            }
        }
        Instr::Ldrh {
            rd,
            rn,
            offset,
            pre,
            writeback,
            up,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let (off_val, _) = evaluate_operand2(cpu, offset);

            if pre {
                addr = if up {
                    addr.wrapping_add(off_val)
                } else {
                    addr.wrapping_sub(off_val)
                };
            }
            cpu.regs[rd as usize] = cpu.read_data_16(bus, addr)? as u32;
            if writeback || !pre {
                let final_addr = if !pre {
                    if up {
                        addr.wrapping_add(off_val)
                    } else {
                        addr.wrapping_sub(off_val)
                    }
                } else {
                    addr
                };
                if rn as usize != REG_PC {
                    cpu.regs[rn as usize] = final_addr;
                }
            }
        }
        Instr::Strh {
            rd,
            rn,
            offset,
            pre,
            writeback,
            up,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let (off_val, _) = evaluate_operand2(cpu, offset);
            let val = cpu.reg(rd);

            if pre {
                addr = if up {
                    addr.wrapping_add(off_val)
                } else {
                    addr.wrapping_sub(off_val)
                };
            }
            cpu.write_data_16(bus, addr, (val & 0xFFFF) as u16)?;
            if writeback || !pre {
                let final_addr = if !pre {
                    if up {
                        addr.wrapping_add(off_val)
                    } else {
                        addr.wrapping_sub(off_val)
                    }
                } else {
                    addr
                };
                if rn as usize != REG_PC {
                    cpu.regs[rn as usize] = final_addr;
                }
            }
        }
        Instr::Ldrsb {
            rd,
            rn,
            offset,
            pre,
            writeback,
            up,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let (off_val, _) = evaluate_operand2(cpu, offset);

            if pre {
                addr = if up {
                    addr.wrapping_add(off_val)
                } else {
                    addr.wrapping_sub(off_val)
                };
            }
            let val = (cpu.read_data_8(bus, addr)? as i8 as i32).cast_unsigned();
            cpu.regs[rd as usize] = val;
            if writeback || !pre {
                let final_addr = if !pre {
                    if up {
                        addr.wrapping_add(off_val)
                    } else {
                        addr.wrapping_sub(off_val)
                    }
                } else {
                    addr
                };
                if rn as usize != REG_PC {
                    cpu.regs[rn as usize] = final_addr;
                }
            }
        }
        Instr::Ldrsh {
            rd,
            rn,
            offset,
            pre,
            writeback,
            up,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let (off_val, _) = evaluate_operand2(cpu, offset);

            if pre {
                addr = if up {
                    addr.wrapping_add(off_val)
                } else {
                    addr.wrapping_sub(off_val)
                };
            }
            let val = (cpu.read_data_16(bus, addr)? as i16 as i32).cast_unsigned();
            cpu.regs[rd as usize] = val;
            if writeback || !pre {
                let final_addr = if !pre {
                    if up {
                        addr.wrapping_add(off_val)
                    } else {
                        addr.wrapping_sub(off_val)
                    }
                } else {
                    addr
                };
                if rn as usize != REG_PC {
                    cpu.regs[rn as usize] = final_addr;
                }
            }
        }

        // Memory (Multiple)
        Instr::Ldm {
            rn,
            reg_list,
            p,
            u,
            w,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let ones = reg_list.count_ones();
            let total_bytes = ones * 4;

            if !u {
                addr = addr.wrapping_sub(total_bytes);
            }
            let mut current_addr = if p == u { addr.wrapping_add(4) } else { addr };

            for i in 0..16 {
                if (reg_list & (1 << i)) != 0 {
                    cpu.regs[i] = cpu.read_data_32(bus, current_addr)?;
                    current_addr += 4;
                }
            }

            if w {
                if u {
                    cpu.regs[rn as usize] = cpu.regs[rn as usize].wrapping_add(total_bytes);
                } else {
                    cpu.regs[rn as usize] = cpu.regs[rn as usize].wrapping_sub(total_bytes);
                }
            }
        }
        Instr::Stm {
            rn,
            reg_list,
            p,
            u,
            w,
            ..
        } => {
            let mut addr = cpu.reg(rn);
            let ones = reg_list.count_ones();
            let total_bytes = ones * 4;

            if !u {
                addr = addr.wrapping_sub(total_bytes);
            }
            let mut current_addr = if p == u { addr.wrapping_add(4) } else { addr };

            for i in 0..16 {
                if (reg_list & (1 << i)) != 0 {
                    let val = cpu.reg(i as u8);
                    cpu.write_data_32(bus, current_addr, val)?;
                    current_addr += 4;
                }
            }

            if w {
                if u {
                    cpu.regs[rn as usize] = cpu.regs[rn as usize].wrapping_add(total_bytes);
                } else {
                    cpu.regs[rn as usize] = cpu.regs[rn as usize].wrapping_sub(total_bytes);
                }
            }
        }

        // Branching
        Instr::B { target, .. } => {
            let pc_base = cpu.regs[REG_PC].wrapping_add(4);
            cpu.regs[REG_PC] = (pc_base as i32).wrapping_add(target).cast_unsigned();
        }
        Instr::Bl { target, .. } => {
            cpu.regs[REG_LR] = cpu.regs[REG_PC];
            let pc_base = cpu.regs[REG_PC].wrapping_add(4);
            cpu.regs[REG_PC] = (pc_base as i32).wrapping_add(target).cast_unsigned();
        }
        Instr::Bx { rm, .. } => {
            let val = cpu.reg(rm);
            cpu.set_t((val & 1) == 1);
            cpu.regs[REG_PC] = val & !1;
        }
        Instr::Blx { rm, .. } => {
            cpu.regs[REG_LR] = cpu.regs[REG_PC];
            let val = cpu.reg(rm);
            cpu.set_t((val & 1) == 1);
            cpu.regs[REG_PC] = val & !1;
        }

        // System
        Instr::Svc { imm, .. } => {
            let handled = hooks.trigger_interrupt(cpu, bus, imm)?;
            if !handled {
                // If we were running a real Linux Kernel inside the emulator,
                // we would trigger the hardware exception vector here.
                // cpu.trigger_exception(CpuModeBits::Supervisor, 0x08);

                // For now, if no hook handles a syscall in our HLE emulator, it's a fatal error.
                return Err(EmuError::NotImplemented("Unhandled SVC Syscall"));
            }
        }
        Instr::Bkpt { imm16, .. } => {
            // Accurately triggers Debug State. This halts the execution loop and bubbles
            // the breakpoint status back to the host process / debugger.
            return Err(EmuError::Breakpoint(imm16));
        }
        Instr::Nop { .. } => {
            // Literally Do Nothing
        }
        Instr::Unknown(raw) => {
            return Err(EmuError::InvalidInstruction(raw as u64));
        }
    }
    Ok(())
}

fn instr_condition(instr: &Instr) -> Condition {
    match instr {
        Instr::And { cond, .. }
        | Instr::Eor { cond, .. }
        | Instr::Sub { cond, .. }
        | Instr::Rsb { cond, .. }
        | Instr::Add { cond, .. }
        | Instr::Adc { cond, .. }
        | Instr::Sbc { cond, .. }
        | Instr::Rsc { cond, .. }
        | Instr::Tst { cond, .. }
        | Instr::Teq { cond, .. }
        | Instr::Cmp { cond, .. }
        | Instr::Cmn { cond, .. }
        | Instr::Orr { cond, .. }
        | Instr::Mov { cond, .. }
        | Instr::Bic { cond, .. }
        | Instr::Mvn { cond, .. }
        | Instr::Movw { cond, .. }
        | Instr::Movt { cond, .. }
        | Instr::Mrs { cond, .. }
        | Instr::Msr { cond, .. }
        | Instr::Mul { cond, .. }
        | Instr::Mla { cond, .. }
        | Instr::Umull { cond, .. }
        | Instr::Umlal { cond, .. }
        | Instr::Smull { cond, .. }
        | Instr::Smlal { cond, .. }
        | Instr::Bfc { cond, .. }
        | Instr::Bfi { cond, .. }
        | Instr::Ubfx { cond, .. }
        | Instr::Sbfx { cond, .. }
        | Instr::Rev { cond, .. }
        | Instr::Rev16 { cond, .. }
        | Instr::Revsh { cond, .. }
        | Instr::Clz { cond, .. }
        | Instr::Sxtb { cond, .. }
        | Instr::Sxth { cond, .. }
        | Instr::Uxtb { cond, .. }
        | Instr::Uxth { cond, .. }
        | Instr::Ldr { cond, .. }
        | Instr::Str { cond, .. }
        | Instr::Ldrb { cond, .. }
        | Instr::Strb { cond, .. }
        | Instr::Ldrh { cond, .. }
        | Instr::Strh { cond, .. }
        | Instr::Ldrsb { cond, .. }
        | Instr::Ldrsh { cond, .. }
        | Instr::Ldm { cond, .. }
        | Instr::Stm { cond, .. }
        | Instr::B { cond, .. }
        | Instr::Bl { cond, .. }
        | Instr::Bx { cond, .. }
        | Instr::Blx { cond, .. }
        | Instr::Svc { cond, .. }
        | Instr::Nop { cond, .. } => *cond,

        // We treat these as always executing. They are not real or for debugging
        Instr::Bkpt { .. } => Condition::Al,
        Instr::Unknown(_) => Condition::Al,
    }
}

fn check_condition(cpu: &Armv7Cpu, cond: Condition) -> bool {
    let cpsr = Cpsr::from_bits_retain(cpu.cpsr);
    match cond {
        Condition::Eq => cpsr.contains(Cpsr::Z),
        Condition::Ne => !cpsr.contains(Cpsr::Z),
        Condition::Cs => cpsr.contains(Cpsr::C),
        Condition::Cc => !cpsr.contains(Cpsr::C),
        Condition::Mi => cpsr.contains(Cpsr::N),
        Condition::Pl => !cpsr.contains(Cpsr::N),
        Condition::Vs => cpsr.contains(Cpsr::V),
        Condition::Vc => !cpsr.contains(Cpsr::V),
        Condition::Hi => cpsr.contains(Cpsr::C) && !cpsr.contains(Cpsr::Z),
        Condition::Ls => !cpsr.contains(Cpsr::C) || cpsr.contains(Cpsr::Z),
        Condition::Ge => cpsr.contains(Cpsr::N) == cpsr.contains(Cpsr::V),
        Condition::Lt => cpsr.contains(Cpsr::N) != cpsr.contains(Cpsr::V),
        Condition::Gt => {
            !cpsr.contains(Cpsr::Z) && (cpsr.contains(Cpsr::N) == cpsr.contains(Cpsr::V))
        }
        Condition::Le => {
            cpsr.contains(Cpsr::Z) || (cpsr.contains(Cpsr::N) != cpsr.contains(Cpsr::V))
        }
        Condition::Al => true,
        Condition::Nv => false,
    }
}

fn get_bank_index(mode: CpuModeBits) -> Option<usize> {
    match mode {
        CpuModeBits::Supervisor => Some(0),
        CpuModeBits::Irq => Some(1),
        CpuModeBits::Fiq => Some(2),
        CpuModeBits::Abort => Some(3),
        CpuModeBits::Undefined => Some(4),
        _ => None,
    }
}

fn set_logic_flags(cpu: &mut Armv7Cpu, result: u32, carry_out: bool) {
    cpu.set_n((result >> 31) == 1);
    cpu.set_z(result == 0);
    cpu.set_c(carry_out);
}

fn do_add(cpu: &mut Armv7Cpu, a: u32, b: u32, update_flags: bool) -> u32 {
    let (res, carry) = a.overflowing_add(b);
    let overflow = ((a ^ b) >> 31 == 0) && ((a ^ res) >> 31 != 0);
    if update_flags {
        cpu.set_n((res >> 31) == 1);
        cpu.set_z(res == 0);
        cpu.set_c(carry);
        cpu.set_v(overflow);
    }
    res
}

fn do_sub(cpu: &mut Armv7Cpu, a: u32, b: u32, update_flags: bool) -> u32 {
    let (res, borrow) = a.overflowing_sub(b);
    let overflow = ((a ^ b) >> 31 != 0) && ((a ^ res) >> 31 != 0);
    if update_flags {
        cpu.set_n((res >> 31) == 1);
        cpu.set_z(res == 0);
        cpu.set_c(!borrow);
        cpu.set_v(overflow);
    }
    res
}

fn do_add_carry(cpu: &mut Armv7Cpu, a: u32, b: u32, c: u32, update_flags: bool) -> u32 {
    let (res1, carry1) = a.overflowing_add(b);
    let (res2, carry2) = res1.overflowing_add(c);
    let carry = carry1 || carry2;
    let overflow = ((a ^ b) >> 31 == 0) && ((a ^ res2) >> 31 != 0);
    if update_flags {
        cpu.set_n((res2 >> 31) == 1);
        cpu.set_z(res2 == 0);
        cpu.set_c(carry);
        cpu.set_v(overflow);
    }
    res2
}

fn do_sub_carry(cpu: &mut Armv7Cpu, a: u32, b: u32, not_c: u32, update_flags: bool) -> u32 {
    let (res1, borrow1) = a.overflowing_sub(b);
    let (res2, borrow2) = res1.overflowing_sub(not_c);
    let borrow = borrow1 || borrow2;
    let overflow = ((a ^ b) >> 31 != 0) && ((a ^ res2) >> 31 != 0);
    if update_flags {
        cpu.set_n((res2 >> 31) == 1);
        cpu.set_z(res2 == 0);
        cpu.set_c(!borrow);
        cpu.set_v(overflow);
    }
    res2
}

fn evaluate_operand2(cpu: &Armv7Cpu, op2: Operand2) -> (u32, bool) {
    match op2 {
        Operand2::Immediate { val, carry_out } => (val, carry_out.unwrap_or_else(|| cpu.get_c())),
        Operand2::Register { rm, shift } => {
            let val = cpu.reg(rm);
            let (shift_type, amount) = match shift {
                Shift::Immediate { shift_type, amount } => (shift_type, amount),
                Shift::Register { shift_type, rs } => (shift_type, cpu.reg(rs) & 0xFF),
            };

            if amount == 0 {
                return match shift_type {
                    ShiftType::Lsl => (val, cpu.get_c()),
                    ShiftType::Lsr => (0, (val >> 31) == 1),
                    ShiftType::Asr => {
                        let bit31 = (val >> 31) == 1;
                        (if bit31 { 0xFFFFFFFF } else { 0 }, bit31)
                    }
                    ShiftType::Ror => {
                        let c = if cpu.get_c() { 1 } else { 0 };
                        let carry_out = (val & 1) == 1;
                        ((c << 31) | (val >> 1), carry_out)
                    }
                };
            }

            match shift_type {
                ShiftType::Lsl => {
                    if amount >= 32 {
                        (0, if amount == 32 { (val & 1) == 1 } else { false })
                    } else {
                        (val << amount, ((val >> (32 - amount)) & 1) == 1)
                    }
                }
                ShiftType::Lsr => {
                    if amount >= 32 {
                        (
                            0,
                            if amount == 32 {
                                (val >> 31) == 1
                            } else {
                                false
                            },
                        )
                    } else {
                        (val >> amount, ((val >> (amount - 1)) & 1) == 1)
                    }
                }
                ShiftType::Asr => {
                    if amount >= 32 {
                        let bit31 = (val >> 31) == 1;
                        (if bit31 { 0xFFFFFFFF } else { 0 }, bit31)
                    } else {
                        (
                            ((val as i32) >> amount).cast_unsigned(),
                            ((val >> (amount - 1)) & 1) == 1,
                        )
                    }
                }
                ShiftType::Ror => {
                    let amt = amount % 32;
                    if amt == 0 {
                        (val, (val >> 31) == 1)
                    } else {
                        (val.rotate_right(amt), ((val >> (amt - 1)) & 1) == 1)
                    }
                }
            }
        }
    }
}
