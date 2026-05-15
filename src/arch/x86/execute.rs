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
            let res = do_add(cpu, v1, v2, false);
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Adc { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_add(cpu, v1, v2, true);
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Sub { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_sub(cpu, v1, v2, false);
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Sbb { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_sub(cpu, v1, v2, true);
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Xor { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = v1 ^ v2;
            store_op(cpu, bus, dest, res)?;
            set_logic_flags(cpu, res);
        }
        Instr::Cmp { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            do_sub(cpu, v1, v2, false);
        }
        Instr::Push(op) => {
            let val = load_op(cpu, bus, op)?;
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_sub(4);
            bus.write_32(cpu.regs[REG_ESP] as u64, val)?;
        }
        Instr::Pop(op) => {
            let val = bus.read_32(cpu.regs[REG_ESP] as u64)?;
            // Increment ESP first
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_add(4);
            // Then store the value. If `op` is ESP, it correctly overwrites the increment
            store_op(cpu, bus, op, val)?;
        }

        Instr::Test { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = v1 & v2;
            // TEST performs a bitwise AND, discards the result, and sets flags
            set_logic_flags(cpu, res);
        }
        Instr::Inc(op) => {
            let v = load_op(cpu, bus, op)?;
            // INC does NOT affect the Carry Flag on x86. We must preserve it.
            let old_cf = cpu.regs[REG_EFLAGS] & EFlags::CF.bits();
            let res = do_add(cpu, v, 1, false);

            // Restore Carry Flag
            cpu.regs[REG_EFLAGS] = (cpu.regs[REG_EFLAGS] & !EFlags::CF.bits()) | old_cf;
            store_op(cpu, bus, op, res)?;
        }
        Instr::Dec(op) => {
            let v = load_op(cpu, bus, op)?;
            // DEC does NOT affect the Carry Flag
            let old_cf = cpu.regs[REG_EFLAGS] & EFlags::CF.bits();
            let res = do_sub(cpu, v, 1, false);

            // Restore Carry Flag
            cpu.regs[REG_EFLAGS] = (cpu.regs[REG_EFLAGS] & !EFlags::CF.bits()) | old_cf;
            store_op(cpu, bus, op, res)?;
        }

        Instr::Not(op) => {
            let val = load_op(cpu, bus, op)?;
            store_op(cpu, bus, op, !val)?;
            // NOT does not touch EFLAGS
        }
        Instr::Neg(op) => {
            let val = load_op(cpu, bus, op)?;
            let res = do_sub(cpu, 0, val, false);
            store_op(cpu, bus, op, res)?;
            // NEG inherently updates EFLAGS perfectly via do_sub
        }

        Instr::And { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = v1 & v2;
            store_op(cpu, bus, dest, res)?;
            set_logic_flags(cpu, res);
        }
        Instr::Or { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = v1 | v2;
            store_op(cpu, bus, dest, res)?;
            set_logic_flags(cpu, res);
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
                set_logic_flags(cpu, res);
                let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
                f.set(EFlags::CF, ((v >> (32 - c)) & 1) != 0);
                cpu.regs[REG_EFLAGS] = f.bits();
            }
        }
        Instr::Shr { dest, count } => {
            let v = load_op(cpu, bus, dest)?;
            let c = load_op(cpu, bus, count)? & 0x1F;
            if c > 0 {
                let res = v >> c;
                store_op(cpu, bus, dest, res)?;
                set_logic_flags(cpu, res);
                let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
                f.set(EFlags::CF, ((v >> (c - 1)) & 1) != 0);
                cpu.regs[REG_EFLAGS] = f.bits();
            }
        }
        Instr::Sar { dest, count } => {
            let v = load_op(cpu, bus, dest)? as i32;
            let c = load_op(cpu, bus, count)? & 0x1F;
            if c > 0 {
                let res = (v >> c) as u32;
                store_op(cpu, bus, dest, res)?;
                set_logic_flags(cpu, res);
                let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
                f.set(EFlags::CF, ((v >> (c - 1)) & 1) != 0);
                cpu.regs[REG_EFLAGS] = f.bits();
            }
        }
        Instr::Movzx8 { dest, src } => {
            // Read ONLY 8 bits, extend to 32 bits (zero-padded)
            let val = match src {
                Operand::Reg(r) => cpu.regs[r as usize] & 0xFF,
                Operand::Mem(m) => bus.read_8(calc_addr(cpu, m))? as u32,
                _ => unreachable!(),
            };
            store_op(cpu, bus, dest, val)?;
        }
        Instr::Movsx8 { dest, src } => {
            // Read ONLY 8 bits, sign-extend to 32 bits
            let val = match src {
                Operand::Reg(r) => (cpu.regs[r as usize] as i8) as i32 as u32,
                Operand::Mem(m) => (bus.read_8(calc_addr(cpu, m))? as i8) as i32 as u32,
                _ => unreachable!(),
            };
            store_op(cpu, bus, dest, val)?;
        }

        Instr::Leave => {
            // 1. ESP = EBP
            cpu.regs[REG_ESP] = cpu.regs[REG_EBP];

            // 2. POP EBP
            let val = bus.read_32(cpu.regs[REG_ESP] as u64)?;
            cpu.regs[REG_EBP] = val;
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_add(4);
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
        Instr::Jmp(rel) => {
            cpu.regs[REG_EIP] = (cpu.regs[REG_EIP] as i32).wrapping_add(rel) as u32;
        }
        Instr::Jcc(cond, rel) => {
            if check_condition(cpu, cond) {
                cpu.regs[REG_EIP] = (cpu.regs[REG_EIP] as i32).wrapping_add(rel) as u32;
            }
        }
        Instr::Int(vec) => {
            let handled = hooks.trigger_interrupt(cpu, bus, vec as u32)?;
            if !handled {
                if vec == 3 {
                    // x86 Debug Breakpoint
                    return Err(EmuError::Breakpoint(3));
                }
                return Err(EmuError::NotImplemented("Unhandled x86 Software Interrupt"));
            }
        }
        Instr::Nop => {}
        Instr::Hlt => return Err(EmuError::Breakpoint(0)),
        Instr::Unknown(op) => return Err(EmuError::InvalidInstruction(op as u64)),
    }
    Ok(())
}

// FLAG CALCULATION LOGIC

#[inline(always)]
fn set_logic_flags(cpu: &mut X86Cpu, res: u32) {
    let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    f.set(EFlags::ZF, res == 0);
    f.set(EFlags::SF, (res >> 31) != 0);
    f.set(EFlags::PF, (res as u8).count_ones() % 2 == 0);
    f.remove(EFlags::CF | EFlags::OF);
    cpu.regs[REG_EFLAGS] = f.bits();
}

#[inline(always)]
fn do_add(cpu: &mut X86Cpu, a: u32, b: u32, carry: bool) -> u32 {
    let c_in = if carry && (cpu.regs[REG_EFLAGS] & EFlags::CF.bits() != 0) {
        1
    } else {
        0
    };
    let (t1, c1) = a.overflowing_add(b);
    let (res, c2) = t1.overflowing_add(c_in);

    let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    f.set(EFlags::CF, c1 | c2);
    f.set(EFlags::ZF, res == 0);
    f.set(EFlags::SF, (res >> 31) != 0);
    f.set(EFlags::OF, ((a ^ res) & (b ^ res)) >> 31 != 0);
    f.set(EFlags::PF, (res as u8).count_ones() % 2 == 0);
    cpu.regs[REG_EFLAGS] = f.bits();
    res
}

#[inline(always)]
fn do_sub(cpu: &mut X86Cpu, a: u32, b: u32, borrow: bool) -> u32 {
    let b_in = if borrow && (cpu.regs[REG_EFLAGS] & EFlags::CF.bits() != 0) {
        1
    } else {
        0
    };
    let (t1, c1) = a.overflowing_sub(b);
    let (res, c2) = t1.overflowing_sub(b_in);

    let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    f.set(EFlags::CF, c1 | c2);
    f.set(EFlags::ZF, res == 0);
    f.set(EFlags::SF, (res >> 31) != 0);
    f.set(EFlags::OF, ((a ^ b) & (a ^ res)) >> 31 != 0);
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

// HELPERS

#[inline(always)]
fn load_op(cpu: &X86Cpu, bus: &mut MemoryBus, op: Operand) -> Result<u32, EmuError> {
    match op {
        Operand::Reg(r) => Ok(cpu.regs[r as usize]),
        Operand::Imm32(i) => Ok(i),
        Operand::Imm16(i) => Ok(i as u32),
        Operand::Imm8(i) => Ok(i as u32),
        Operand::Mem(m) => bus.read_32(calc_addr(cpu, m)),
    }
}

#[inline(always)]
fn store_op(cpu: &mut X86Cpu, bus: &mut MemoryBus, op: Operand, val: u32) -> Result<(), EmuError> {
    match op {
        Operand::Reg(r) => {
            cpu.regs[r as usize] = val;
            Ok(())
        }
        Operand::Mem(m) => bus.write_32(calc_addr(cpu, m), val),
        _ => Err(EmuError::DeviceError("Store to Immediate".into())),
    }
}

#[inline(always)]
fn calc_addr(cpu: &X86Cpu, m: MemoryAddr) -> u64 {
    let mut addr = m.disp;
    if let Some(base) = m.base {
        addr = addr.wrapping_add(cpu.regs[base as usize] as i32);
    }
    if let Some(idx) = m.index {
        addr = addr.wrapping_add((cpu.regs[idx as usize] as i32).wrapping_mul(m.scale as i32));
    }
    addr as u32 as u64
}
