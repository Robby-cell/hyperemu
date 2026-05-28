use super::X86Cpu;
use super::instr::*;
use super::registers::*;
use crate::bus::MemoryBus;
use crate::error::EmuError;
use crate::hook::HookRegistry;

#[inline(always)]
pub fn execute_instr(
    cpu: &mut X86Cpu,
    instr: Instr,
    bus: &mut MemoryBus,
    hooks: &mut HookRegistry,
) -> Result<(), EmuError> {
    match instr {
        Instr::Mov { dest, src } => {
            let val = load_op(cpu, bus, src)?;
            store_op(cpu, bus, dest, val)?;
        }
        Instr::Lea { dest, src } => {
            let addr = calc_addr(cpu, src);
            cpu.regs[dest as usize] = addr as u32;
        }
        Instr::Add { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_add(cpu, v1, v2, false, dest.size());
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Adc { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_add(cpu, v1, v2, true, dest.size());
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Sub { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_sub(cpu, v1, v2, false, dest.size());
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Sbb { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_sub(cpu, v1, v2, true, dest.size());
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Xor { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = v1 ^ v2;
            store_op(cpu, bus, dest, res)?;
            set_logic_flags(cpu, res, dest.size());
        }
        Instr::And { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = v1 & v2;
            store_op(cpu, bus, dest, res)?;
            set_logic_flags(cpu, res, dest.size());
        }
        Instr::Or { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = v1 | v2;
            store_op(cpu, bus, dest, res)?;
            set_logic_flags(cpu, res, dest.size());
        }
        Instr::Cmp { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            do_sub(cpu, v1, v2, false, dest.size()); // Updates flags, discards result
        }
        Instr::Test { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = v1 & v2;
            set_logic_flags(cpu, res, dest.size()); // Updates flags, discards result
        }
        Instr::Inc(op) => {
            let v = load_op(cpu, bus, op)?;
            let old_cf = cpu.regs[REG_EFLAGS] & EFlags::CF.bits();
            let res = do_add(cpu, v, 1, false, op.size());
            cpu.regs[REG_EFLAGS] = (cpu.regs[REG_EFLAGS] & !EFlags::CF.bits()) | old_cf;
            store_op(cpu, bus, op, res)?;
        }
        Instr::Dec(op) => {
            let v = load_op(cpu, bus, op)?;
            let old_cf = cpu.regs[REG_EFLAGS] & EFlags::CF.bits();
            let res = do_sub(cpu, v, 1, false, op.size());
            cpu.regs[REG_EFLAGS] = (cpu.regs[REG_EFLAGS] & !EFlags::CF.bits()) | old_cf;
            store_op(cpu, bus, op, res)?;
        }
        Instr::Neg(op) => {
            let val = load_op(cpu, bus, op)?;
            let res = do_sub(cpu, 0, val, false, op.size());
            store_op(cpu, bus, op, res)?;
        }
        Instr::Not(op) => {
            let val = load_op(cpu, bus, op)?;
            let res = match op.size() {
                OpSize::Byte => (!val) & 0xFF,
                OpSize::Word => (!val) & 0xFFFF,
                OpSize::Dword => !val,
            };
            store_op(cpu, bus, op, res)?;
        }

        Instr::Mul(op) => {
            let v = load_op(cpu, bus, op)?;
            let res = (cpu.regs[REG_EAX] as u64).wrapping_mul(v as u64);
            cpu.regs[REG_EAX] = res as u32;
            cpu.regs[REG_EDX] = (res >> 32) as u32;
            let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
            if cpu.regs[REG_EDX] != 0 {
                f.insert(EFlags::CF | EFlags::OF);
            } else {
                f.remove(EFlags::CF | EFlags::OF);
            }
            cpu.regs[REG_EFLAGS] = f.bits();
        }
        Instr::Imul(dest, src) => {
            let v1 = load_op(cpu, bus, dest)? as i32 as i64;
            let v2 = load_op(cpu, bus, src)? as i32 as i64;
            let res = v1.wrapping_mul(v2);
            store_op(cpu, bus, dest, res as u32)?;
            let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
            if res != (res as i32 as i64) {
                f.insert(EFlags::CF | EFlags::OF);
            } else {
                f.remove(EFlags::CF | EFlags::OF);
            }
            cpu.regs[REG_EFLAGS] = f.bits();
        }
        Instr::Div(op) => {
            let divisor = load_op(cpu, bus, op)?;
            if divisor == 0 {
                return Err(EmuError::DeviceError("Divide by zero (#DE)".into()));
            }
            let dividend = ((cpu.regs[REG_EDX] as u64) << 32) | (cpu.regs[REG_EAX] as u64);
            let quot = dividend / (divisor as u64);
            let rem = dividend % (divisor as u64);
            if quot > 0xFFFFFFFF {
                return Err(EmuError::DeviceError("Divide overflow (#DE)".into()));
            }
            cpu.regs[REG_EAX] = quot as u32;
            cpu.regs[REG_EDX] = rem as u32;
        }

        Instr::Shl { dest, count } => {
            let v = load_op(cpu, bus, dest)?;
            let c = load_op(cpu, bus, count)? & 0x1F;
            if c > 0 {
                let res = v << c;
                store_op(cpu, bus, dest, res)?;
                set_logic_flags(cpu, res, dest.size());
                let bits = match dest.size() {
                    OpSize::Byte => 8,
                    OpSize::Word => 16,
                    OpSize::Dword => 32,
                };
                let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
                f.set(EFlags::CF, ((v >> (bits - c)) & 1) != 0);
                cpu.regs[REG_EFLAGS] = f.bits();
            }
        }
        Instr::Shr { dest, count } => {
            let v = load_op(cpu, bus, dest)?;
            let c = load_op(cpu, bus, count)? & 0x1F;
            if c > 0 {
                let res = v >> c;
                store_op(cpu, bus, dest, res)?;
                set_logic_flags(cpu, res, dest.size());
                let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
                f.set(EFlags::CF, ((v >> (c - 1)) & 1) != 0);
                cpu.regs[REG_EFLAGS] = f.bits();
            }
        }
        Instr::Sar { dest, count } => {
            let v = load_op(cpu, bus, dest)?;
            let c = load_op(cpu, bus, count)? & 0x1F;
            if c > 0 {
                let v_signed = match dest.size() {
                    OpSize::Byte => (v as i8) as i32,
                    OpSize::Word => (v as i16) as i32,
                    OpSize::Dword => v as i32,
                };
                let res = (v_signed >> c) as u32;
                store_op(cpu, bus, dest, res)?;
                set_logic_flags(cpu, res, dest.size());
                let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
                f.set(EFlags::CF, ((v >> (c - 1)) & 1) != 0);
                cpu.regs[REG_EFLAGS] = f.bits();
            }
        }
        Instr::Movzx8 { dest, src } => {
            let val = match src {
                Operand::Reg8(r) => {
                    let idx = (r as usize) % 4;
                    let val = cpu.regs[idx];
                    if (r as usize) >= 4 {
                        (val >> 8) & 0xFF
                    } else {
                        val & 0xFF
                    }
                }
                Operand::Mem8(m) => bus.read_8(calc_addr(cpu, m))? as u32,
                _ => unreachable!(),
            };
            store_op(cpu, bus, dest, val)?;
        }
        Instr::Movsx8 { dest, src } => {
            let val = match src {
                Operand::Reg8(r) => {
                    let idx = (r as usize) % 4;
                    let raw = cpu.regs[idx];
                    let v = if (r as usize) >= 4 {
                        (raw >> 8) & 0xFF
                    } else {
                        raw & 0xFF
                    };
                    (v as i8) as i32 as u32
                }
                Operand::Mem8(m) => (bus.read_8(calc_addr(cpu, m))? as i8) as i32 as u32,
                _ => unreachable!(),
            };
            store_op(cpu, bus, dest, val)?;
        }

        Instr::Push(op) => {
            let val = load_op(cpu, bus, op)?;
            let sz = if op.size() == OpSize::Word { 2 } else { 4 };
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_sub(sz);
            if sz == 2 {
                bus.write_16(cpu.regs[REG_ESP] as u64, val as u16)?;
            } else {
                bus.write_32(cpu.regs[REG_ESP] as u64, val)?;
            }
        }
        Instr::Pop(op) => {
            let sz = if op.size() == OpSize::Word { 2 } else { 4 };
            let val = if sz == 2 {
                bus.read_16(cpu.regs[REG_ESP] as u64)? as u32
            } else {
                bus.read_32(cpu.regs[REG_ESP] as u64)?
            };
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_add(sz);
            store_op(cpu, bus, op, val)?;
        }
        Instr::Call(rel) => {
            let ret_addr = cpu.regs[REG_EIP];
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_sub(4);
            bus.write_32(cpu.regs[REG_ESP] as u64, ret_addr)?;
            cpu.regs[REG_EIP] = (ret_addr as i32).wrapping_add(rel) as u32;
        }
        Instr::Ret => {
            cpu.regs[REG_EIP] = bus.read_32(cpu.regs[REG_ESP] as u64)?;
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_add(4);
        }
        Instr::Leave => {
            cpu.regs[REG_ESP] = cpu.regs[REG_EBP];
            cpu.regs[REG_EBP] = bus.read_32(cpu.regs[REG_ESP] as u64)?;
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_add(4);
        }
        Instr::Jmp(rel) => {
            cpu.regs[REG_EIP] = (cpu.regs[REG_EIP] as i32).wrapping_add(rel) as u32;
        }
        Instr::Jcc(cond, rel) => {
            if check_condition(cpu, cond) {
                cpu.regs[REG_EIP] = (cpu.regs[REG_EIP] as i32).wrapping_add(rel) as u32;
            }
        }

        // String Operations & Transformations
        Instr::Lods(size, rep) => {
            let step = get_string_step(cpu, size);
            let do_lods = |cpu: &mut X86Cpu, bus: &mut MemoryBus| -> Result<(), EmuError> {
                let esi = cpu.regs[REG_ESI];
                let val = match size {
                    OpSize::Byte => bus.read_8(esi as u64)? as u32,
                    OpSize::Word => bus.read_16(esi as u64)? as u32,
                    OpSize::Dword => bus.read_32(esi as u64)?,
                };
                cpu.regs[REG_ESI] = esi.wrapping_add(step);
                match size {
                    OpSize::Byte => cpu.regs[REG_EAX] = (cpu.regs[REG_EAX] & 0xFFFFFF00) | val,
                    OpSize::Word => cpu.regs[REG_EAX] = (cpu.regs[REG_EAX] & 0xFFFF0000) | val,
                    OpSize::Dword => cpu.regs[REG_EAX] = val,
                }
                Ok(())
            };
            if rep.is_some() {
                while cpu.regs[REG_ECX] > 0 {
                    do_lods(cpu, bus)?;
                    cpu.regs[REG_ECX] = cpu.regs[REG_ECX].wrapping_sub(1);
                }
            } else {
                do_lods(cpu, bus)?;
            }
        }
        Instr::Stos(size, rep) => {
            let step = get_string_step(cpu, size);
            let val = match size {
                OpSize::Byte => cpu.regs[REG_EAX] & 0xFF,
                OpSize::Word => cpu.regs[REG_EAX] & 0xFFFF,
                OpSize::Dword => cpu.regs[REG_EAX],
            };
            let do_stos = |cpu: &mut X86Cpu, bus: &mut MemoryBus| -> Result<(), EmuError> {
                let edi = cpu.regs[REG_EDI];
                match size {
                    OpSize::Byte => bus.write_8(edi as u64, val as u8)?,
                    OpSize::Word => bus.write_16(edi as u64, val as u16)?,
                    OpSize::Dword => bus.write_32(edi as u64, val)?,
                }
                cpu.regs[REG_EDI] = edi.wrapping_add(step);
                Ok(())
            };
            if rep.is_some() {
                while cpu.regs[REG_ECX] > 0 {
                    do_stos(cpu, bus)?;
                    cpu.regs[REG_ECX] = cpu.regs[REG_ECX].wrapping_sub(1);
                }
            } else {
                do_stos(cpu, bus)?;
            }
        }
        Instr::Movs(size, rep) => {
            let step = get_string_step(cpu, size);
            let do_movs = |cpu: &mut X86Cpu, bus: &mut MemoryBus| -> Result<(), EmuError> {
                let esi = cpu.regs[REG_ESI];
                let edi = cpu.regs[REG_EDI];
                let val = match size {
                    OpSize::Byte => bus.read_8(esi as u64)? as u32,
                    OpSize::Word => bus.read_16(esi as u64)? as u32,
                    OpSize::Dword => bus.read_32(esi as u64)?,
                };
                match size {
                    OpSize::Byte => bus.write_8(edi as u64, val as u8)?,
                    OpSize::Word => bus.write_16(edi as u64, val as u16)?,
                    OpSize::Dword => bus.write_32(edi as u64, val)?,
                }
                cpu.regs[REG_ESI] = esi.wrapping_add(step);
                cpu.regs[REG_EDI] = edi.wrapping_add(step);
                Ok(())
            };
            if rep.is_some() {
                while cpu.regs[REG_ECX] > 0 {
                    do_movs(cpu, bus)?;
                    cpu.regs[REG_ECX] = cpu.regs[REG_ECX].wrapping_sub(1);
                }
            } else {
                do_movs(cpu, bus)?;
            }
        }
        Instr::Scas(size, rep) => {
            let step = get_string_step(cpu, size);
            let do_scas = |cpu: &mut X86Cpu, bus: &mut MemoryBus| -> Result<(), EmuError> {
                let edi = cpu.regs[REG_EDI];
                let val = match size {
                    OpSize::Byte => bus.read_8(edi as u64)? as u32,
                    OpSize::Word => bus.read_16(edi as u64)? as u32,
                    OpSize::Dword => bus.read_32(edi as u64)?,
                };
                let eax_val = match size {
                    OpSize::Byte => cpu.regs[REG_EAX] & 0xFF,
                    OpSize::Word => cpu.regs[REG_EAX] & 0xFFFF,
                    OpSize::Dword => cpu.regs[REG_EAX],
                };
                do_sub(cpu, eax_val, val, false, size); // Updates Flags
                cpu.regs[REG_EDI] = edi.wrapping_add(step);
                Ok(())
            };
            if let Some(r) = rep {
                while cpu.regs[REG_ECX] > 0 {
                    do_scas(cpu, bus)?;
                    cpu.regs[REG_ECX] = cpu.regs[REG_ECX].wrapping_sub(1);
                    let zf = (cpu.regs[REG_EFLAGS] & EFlags::ZF.bits()) != 0;
                    if r == RepPrefix::Rep && !zf {
                        break;
                    } // REPE/REPZ ends if ZF=0
                    if r == RepPrefix::Repne && zf {
                        break;
                    } // REPNE/REPNZ ends if ZF=1
                }
            } else {
                do_scas(cpu, bus)?;
            }
        }
        Instr::Cmps(size, rep) => {
            let step = get_string_step(cpu, size);
            let do_cmps = |cpu: &mut X86Cpu, bus: &mut MemoryBus| -> Result<(), EmuError> {
                let esi = cpu.regs[REG_ESI];
                let edi = cpu.regs[REG_EDI];
                let src_val = match size {
                    OpSize::Byte => bus.read_8(esi as u64)? as u32,
                    OpSize::Word => bus.read_16(esi as u64)? as u32,
                    OpSize::Dword => bus.read_32(esi as u64)?,
                };
                let dst_val = match size {
                    OpSize::Byte => bus.read_8(edi as u64)? as u32,
                    OpSize::Word => bus.read_16(edi as u64)? as u32,
                    OpSize::Dword => bus.read_32(edi as u64)?,
                };
                do_sub(cpu, src_val, dst_val, false, size); // Updates Flags
                cpu.regs[REG_ESI] = esi.wrapping_add(step);
                cpu.regs[REG_EDI] = edi.wrapping_add(step);
                Ok(())
            };
            if let Some(r) = rep {
                while cpu.regs[REG_ECX] > 0 {
                    do_cmps(cpu, bus)?;
                    cpu.regs[REG_ECX] = cpu.regs[REG_ECX].wrapping_sub(1);
                    let zf = (cpu.regs[REG_EFLAGS] & EFlags::ZF.bits()) != 0;
                    if r == RepPrefix::Rep && !zf {
                        break;
                    }
                    if r == RepPrefix::Repne && zf {
                        break;
                    }
                }
            } else {
                do_cmps(cpu, bus)?;
            }
        }
        Instr::Cld => cpu.regs[REG_EFLAGS] &= !EFlags::DF.bits(),
        Instr::Std => cpu.regs[REG_EFLAGS] |= EFlags::DF.bits(),
        Instr::Cbw(size) => match size {
            OpSize::Word => {
                let al = (cpu.regs[REG_EAX] & 0xFF) as i8;
                cpu.regs[REG_EAX] = (cpu.regs[REG_EAX] & 0xFFFF0000) | ((al as i16 as u16) as u32);
            }
            OpSize::Dword => {
                let ax = (cpu.regs[REG_EAX] & 0xFFFF) as i16;
                cpu.regs[REG_EAX] = ax as i32 as u32;
            }
            OpSize::Byte => unreachable!(),
        },
        Instr::Cwd(size) => match size {
            OpSize::Word => {
                let ax = (cpu.regs[REG_EAX] & 0xFFFF) as i16;
                let dx = if ax < 0 { 0xFFFF } else { 0 };
                cpu.regs[REG_EDX] = (cpu.regs[REG_EDX] & 0xFFFF0000) | dx;
            }
            OpSize::Dword => {
                let eax = cpu.regs[REG_EAX] as i32;
                cpu.regs[REG_EDX] = if eax < 0 { 0xFFFFFFFF } else { 0 };
            }
            OpSize::Byte => unreachable!(),
        },
        Instr::Syscall | Instr::Sysret | Instr::Sysenter | Instr::Sysexit => {
            return Err(EmuError::NotImplemented("Syscall/Sysenter instructions"));
        }
        Instr::In(_, _) => return Err(EmuError::NotImplemented("Port I/O (IN)")),
        Instr::Out(_, _) => return Err(EmuError::NotImplemented("Port I/O (OUT)")),
        Instr::Ins(_, _) => return Err(EmuError::NotImplemented("Port I/O (INS)")),
        Instr::Outs(_, _) => return Err(EmuError::NotImplemented("Port I/O (OUTS)")),

        Instr::Int(vec) => {
            let handled = hooks.trigger_interrupt(cpu, bus, vec as u32)?;
            if !handled {
                if vec == 3 {
                    return Err(EmuError::Breakpoint(3));
                }
                return Err(EmuError::NotImplemented("Unhandled x86 Software Interrupt"));
            }
        }

        Instr::Nop => {}
        Instr::Hlt => return Err(EmuError::Breakpoint(0xDEAD)),
        Instr::Unknown(op) => return Err(EmuError::InvalidInstruction(op as u64)),
    }
    Ok(())
}

// SIZE AWARE FLAG EVALUATION

#[inline(always)]
fn set_logic_flags(cpu: &mut X86Cpu, res: u32, size: OpSize) {
    let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    match size {
        OpSize::Byte => {
            f.set(EFlags::ZF, (res & 0xFF) == 0);
            f.set(EFlags::SF, (res & 0x80) != 0);
        }
        OpSize::Word => {
            f.set(EFlags::ZF, (res & 0xFFFF) == 0);
            f.set(EFlags::SF, (res & 0x8000) != 0);
        }
        OpSize::Dword => {
            f.set(EFlags::ZF, res == 0);
            f.set(EFlags::SF, (res >> 31) != 0);
        }
    }
    f.set(EFlags::PF, (res as u8).count_ones() % 2 == 0);
    f.remove(EFlags::CF | EFlags::OF);
    cpu.regs[REG_EFLAGS] = f.bits();
}

#[inline(always)]
fn do_add(cpu: &mut X86Cpu, a: u32, b: u32, carry: bool, size: OpSize) -> u32 {
    let c_in = if carry && (cpu.regs[REG_EFLAGS] & EFlags::CF.bits() != 0) {
        1
    } else {
        0
    };
    let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    let res;

    match size {
        OpSize::Byte => {
            let (t1, c1) = (a as u8).overflowing_add(b as u8);
            let (r, c2) = t1.overflowing_add(c_in as u8);
            f.set(EFlags::CF, c1 | c2);
            f.set(EFlags::ZF, r == 0);
            f.set(EFlags::SF, (r & 0x80) != 0);
            f.set(EFlags::OF, (((a as u8 ^ r) & (b as u8 ^ r)) & 0x80) != 0);
            res = r as u32;
        }
        OpSize::Word => {
            let (t1, c1) = (a as u16).overflowing_add(b as u16);
            let (r, c2) = t1.overflowing_add(c_in as u16);
            f.set(EFlags::CF, c1 | c2);
            f.set(EFlags::ZF, r == 0);
            f.set(EFlags::SF, (r & 0x8000) != 0);
            f.set(
                EFlags::OF,
                (((a as u16 ^ r) & (b as u16 ^ r)) & 0x8000) != 0,
            );
            res = r as u32;
        }
        OpSize::Dword => {
            let (t1, c1) = a.overflowing_add(b);
            let (r, c2) = t1.overflowing_add(c_in);
            f.set(EFlags::CF, c1 | c2);
            f.set(EFlags::ZF, r == 0);
            f.set(EFlags::SF, (r >> 31) != 0);
            f.set(EFlags::OF, (((a ^ r) & (b ^ r)) >> 31) != 0);
            res = r;
        }
    }
    f.set(EFlags::PF, (res as u8).count_ones() % 2 == 0);
    cpu.regs[REG_EFLAGS] = f.bits();
    res
}

#[inline(always)]
fn do_sub(cpu: &mut X86Cpu, a: u32, b: u32, borrow: bool, size: OpSize) -> u32 {
    let b_in = if borrow && (cpu.regs[REG_EFLAGS] & EFlags::CF.bits() != 0) {
        1
    } else {
        0
    };
    let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    let res;

    match size {
        OpSize::Byte => {
            let (t1, c1) = (a as u8).overflowing_sub(b as u8);
            let (r, c2) = t1.overflowing_sub(b_in as u8);
            f.set(EFlags::CF, c1 | c2);
            f.set(EFlags::ZF, r == 0);
            f.set(EFlags::SF, (r & 0x80) != 0);
            f.set(
                EFlags::OF,
                (((a as u8 ^ b as u8) & (a as u8 ^ r)) & 0x80) != 0,
            );
            res = r as u32;
        }
        OpSize::Word => {
            let (t1, c1) = (a as u16).overflowing_sub(b as u16);
            let (r, c2) = t1.overflowing_sub(b_in as u16);
            f.set(EFlags::CF, c1 | c2);
            f.set(EFlags::ZF, r == 0);
            f.set(EFlags::SF, (r & 0x8000) != 0);
            f.set(
                EFlags::OF,
                (((a as u16 ^ b as u16) & (a as u16 ^ r)) & 0x8000) != 0,
            );
            res = r as u32;
        }
        OpSize::Dword => {
            let (t1, c1) = a.overflowing_sub(b);
            let (r, c2) = t1.overflowing_sub(b_in);
            f.set(EFlags::CF, c1 | c2);
            f.set(EFlags::ZF, r == 0);
            f.set(EFlags::SF, (r >> 31) != 0);
            f.set(EFlags::OF, (((a ^ b) & (a ^ r)) >> 31) != 0);
            res = r;
        }
    }
    f.set(EFlags::PF, (res as u8).count_ones() % 2 == 0);
    cpu.regs[REG_EFLAGS] = f.bits();
    res
}

#[inline(always)]
fn check_condition(cpu: &X86Cpu, cond: Condition) -> bool {
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    let z = f.contains(EFlags::ZF);
    let s = f.contains(EFlags::SF);
    let v = f.contains(EFlags::OF);
    let c = f.contains(EFlags::CF);

    match cond {
        Condition::O => v,
        Condition::NO => !v,
        Condition::B => c,
        Condition::AE => !c,
        Condition::E => z,
        Condition::NE => !z,
        Condition::BE => c || z,
        Condition::A => !c && !z,
        Condition::S => s,
        Condition::NS => !s,
        Condition::P => f.contains(EFlags::PF),
        Condition::NP => !f.contains(EFlags::PF),
        Condition::L => s != v,
        Condition::GE => s == v,
        Condition::LE => z || (s != v),
        Condition::G => !z && (s == v),
    }
}

// SIZE AWARE OPERAND FETCHING

#[inline(always)]
fn load_op(cpu: &X86Cpu, bus: &mut MemoryBus, op: Operand) -> Result<u32, EmuError> {
    match op {
        Operand::Reg32(r) => Ok(cpu.regs[r as usize]),
        Operand::Reg16(r) => Ok(cpu.regs[r as usize] & 0xFFFF),
        Operand::Reg8(r) => {
            let idx = (r as usize) % 4;
            let val = cpu.regs[idx];
            Ok(if (r as usize) >= 4 {
                (val >> 8) & 0xFF
            } else {
                val & 0xFF
            })
        }
        Operand::Mem32(m) => bus.read_32(calc_addr(cpu, m)),
        Operand::Mem16(m) => Ok(bus.read_16(calc_addr(cpu, m))? as u32),
        Operand::Mem8(m) => Ok(bus.read_8(calc_addr(cpu, m))? as u32),
        Operand::Imm32(i) => Ok(i),
        Operand::Imm16(i) => Ok(i as u32),
        Operand::Imm8(i) => Ok(i as u32),
    }
}

#[inline(always)]
fn store_op(cpu: &mut X86Cpu, bus: &mut MemoryBus, op: Operand, val: u32) -> Result<(), EmuError> {
    match op {
        Operand::Reg32(r) => {
            cpu.regs[r as usize] = val;
            Ok(())
        }
        Operand::Reg16(r) => {
            cpu.regs[r as usize] = (cpu.regs[r as usize] & 0xFFFF0000) | (val & 0xFFFF);
            Ok(())
        }
        Operand::Reg8(r) => {
            let idx = (r as usize) % 4;
            let cur = cpu.regs[idx];
            if (r as usize) >= 4 {
                cpu.regs[idx] = (cur & 0xFFFF00FF) | ((val & 0xFF) << 8);
            } else {
                cpu.regs[idx] = (cur & 0xFFFFFF00) | (val & 0xFF);
            }
            Ok(())
        }
        Operand::Mem32(m) => bus.write_32(calc_addr(cpu, m), val),
        Operand::Mem16(m) => bus.write_16(calc_addr(cpu, m), val as u16),
        Operand::Mem8(m) => bus.write_8(calc_addr(cpu, m), val as u8),
        _ => Err(EmuError::DeviceError("Store to Immediate".into())),
    }
}

#[inline(always)]
fn calc_addr(cpu: &X86Cpu, m: MemoryAddr) -> u64 {
    let mut addr = m.disp;
    if let Some(b) = m.base {
        addr = addr.wrapping_add(cpu.regs[b as usize] as i32);
    }
    if let Some(idx) = m.index {
        addr = addr.wrapping_add((cpu.regs[idx as usize] as i32).wrapping_mul(m.scale as i32));
    }
    addr as u32 as u64
}

#[inline(always)]
fn get_string_step(cpu: &X86Cpu, size: OpSize) -> u32 {
    let df = cpu.regs[REG_EFLAGS] & EFlags::DF.bits() != 0;
    let step = match size {
        OpSize::Byte => 1,
        OpSize::Word => 2,
        OpSize::Dword => 4,
    };
    if df { (0u32).wrapping_sub(step) } else { step }
}
