#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpReg32 {
    Eax = 0,
    Ecx = 1,
    Edx = 2,
    Ebx = 3,
    Esp = 4,
    Ebp = 5,
    Esi = 6,
    Edi = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpReg16 {
    Ax = 0,
    Cx = 1,
    Dx = 2,
    Bx = 3,
    Sp = 4,
    Bp = 5,
    Si = 6,
    Di = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpReg8 {
    Al = 0,
    Cl = 1,
    Dl = 2,
    Bl = 3,
    Ah = 4,
    Ch = 5,
    Dh = 6,
    Bh = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryAddr {
    pub base: Option<GpReg32>,
    pub index: Option<GpReg32>,
    pub scale: u8,
    pub disp: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpSize {
    Byte,
    Word,
    Dword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    Reg32(GpReg32),
    Reg16(GpReg16),
    Reg8(GpReg8),
    Mem32(MemoryAddr),
    Mem16(MemoryAddr),
    Mem8(MemoryAddr),
    Imm32(u32),
    Imm16(u16),
    Imm8(u8),
}

impl Operand {
    pub fn size(&self) -> OpSize {
        match self {
            Operand::Reg8(_) | Operand::Mem8(_) | Operand::Imm8(_) => OpSize::Byte,
            Operand::Reg16(_) | Operand::Mem16(_) | Operand::Imm16(_) => OpSize::Word,
            Operand::Reg32(_) | Operand::Mem32(_) | Operand::Imm32(_) => OpSize::Dword,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepPrefix {
    Rep,
    Repne,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instr {
    Mov { dest: Operand, src: Operand },
    Push(Operand),
    Pop(Operand),
    Lea { dest: GpReg32, src: MemoryAddr },

    Add { dest: Operand, src: Operand },
    Adc { dest: Operand, src: Operand },
    Sub { dest: Operand, src: Operand },
    Sbb { dest: Operand, src: Operand },
    And { dest: Operand, src: Operand },
    Or { dest: Operand, src: Operand },
    Xor { dest: Operand, src: Operand },
    Cmp { dest: Operand, src: Operand },
    Test { dest: Operand, src: Operand },
    Inc(Operand),
    Dec(Operand),
    Neg(Operand),
    Not(Operand),

    Mul(Operand),           // Unsigned EDX:EAX = EAX * r/m32
    Imul(Operand, Operand), // Signed dest *= src
    Div(Operand),           // Unsigned EAX = EDX:EAX / r/m32, EDX = Remainder
    Shl { dest: Operand, count: Operand },
    Shr { dest: Operand, count: Operand },
    Sar { dest: Operand, count: Operand },
    Movzx8 { dest: Operand, src: Operand }, // Zero-extend 8-bit to 32-bit
    Movsx8 { dest: Operand, src: Operand }, // Sign-extend 8-bit to 32-bit

    // String Operations & Flags
    Lods(OpSize, Option<RepPrefix>),
    Stos(OpSize, Option<RepPrefix>),
    Movs(OpSize, Option<RepPrefix>),
    Scas(OpSize, Option<RepPrefix>),
    Cmps(OpSize, Option<RepPrefix>),
    Ins(OpSize, Option<RepPrefix>),
    Outs(OpSize, Option<RepPrefix>),
    Cld,
    Std,
    Cbw(OpSize),
    Cwd(OpSize),
    Syscall,
    Sysret,
    Sysenter,
    Sysexit,
    In(OpSize, Operand),
    Out(OpSize, Operand),

    // Control Flow
    Jmp(i32), // Relative
    Jcc(Condition, i32),
    Call(i32),
    Ret,
    Leave,

    // System
    Int(u8), // Software Interrupt (HLE point)
    Nop,
    Hlt,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    O = 0,
    NO = 1,
    B = 2,
    AE = 3,
    E = 4,
    NE = 5,
    BE = 6,
    A = 7,
    S = 8,
    NS = 9,
    P = 10,
    NP = 11,
    L = 12,
    GE = 13,
    LE = 14,
    G = 15,
}
