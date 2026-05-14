use super::X86Cpu;
use super::instr::*;
use super::registers::*;
use crate::bus::MemoryBus;
use crate::error::EmuError;

#[inline(always)]
pub fn execute_instr(cpu: &mut X86Cpu, instr: Instr, bus: &mut MemoryBus) -> Result<(), EmuError> {
    match instr {
        Instr::Mov { dest, src } => {
            let val = load_op(cpu, bus, src)?;
            store_op(cpu, bus, dest, val)?;
        }
        Instr::Add { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_add(cpu, v1, v2, false);
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Sub { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_sub(cpu, v1, v2, false);
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
            store_op(cpu, bus, op, val)?;
            cpu.regs[REG_ESP] = cpu.regs[REG_ESP].wrapping_add(4);
        }
        Instr::Lea { dest, src } => {
            // LEA calculates the memory address and stores the *address* in the register.
            // It does NOT read from memory, and it does NOT update EFLAGS.
            let addr = calc_addr(cpu, src);
            cpu.regs[dest as usize] = addr as u32;
        }
        Instr::Adc { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_add(cpu, v1, v2, true); // true = Use Carry Flag
            store_op(cpu, bus, dest, res)?;
        }
        Instr::Sbb { dest, src } => {
            let v1 = load_op(cpu, bus, dest)?;
            let v2 = load_op(cpu, bus, src)?;
            let res = do_sub(cpu, v1, v2, true); // true = Use Carry Flag
            store_op(cpu, bus, dest, res)?;
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
        Instr::Jcc(cond, rel) => {
            if check_condition(cpu, cond) {
                cpu.regs[REG_EIP] = (cpu.regs[REG_EIP] as i32).wrapping_add(rel) as u32;
            }
        }
        Instr::Int(vec) => {
            let _ = vec;
            return Err(EmuError::NotImplemented("x86 INT hooks"));
        }
        Instr::Nop => {}
        Instr::Unknown(op) => return Err(EmuError::InvalidInstruction(op as u64)),
        _ => return Err(EmuError::NotImplemented("x86 Op logic")),
    }
    Ok(())
}

// FLAG CALCULATION LOGIC

#[inline(always)]
fn set_logic_flags(cpu: &mut X86Cpu, res: u32) {
    let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    f.set(EFlags::ZF, res == 0);
    f.set(EFlags::SF, (res >> 31) != 0);
    f.set(EFlags::PF, (res as u8).count_ones().is_multiple_of(2));
    f.remove(EFlags::CF | EFlags::OF);
    cpu.regs[REG_EFLAGS] = f.bits();
}

#[inline(always)]
fn do_add(cpu: &mut X86Cpu, a: u32, b: u32, carry: bool) -> u32 {
    let c_in = if carry && (cpu.regs[REG_EFLAGS] & 1 != 0) {
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
    f.set(EFlags::PF, (res as u8).count_ones().is_multiple_of(2));
    cpu.regs[REG_EFLAGS] = f.bits();
    res
}

#[inline(always)]
fn do_sub(cpu: &mut X86Cpu, a: u32, b: u32, borrow: bool) -> u32 {
    let b_in = if borrow && (cpu.regs[REG_EFLAGS] & 1 != 0) {
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
    f.set(EFlags::PF, (res as u8).count_ones().is_multiple_of(2));
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
        Operand::Imm8(i) => Ok(i as u32),
        Operand::Mem(m) => bus.read_32(calc_addr(cpu, m)),
        _ => Err(EmuError::NotImplemented("X86 Operand Size")),
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
