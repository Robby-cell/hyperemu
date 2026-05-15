#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpReg {
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
pub struct MemoryAddr {
    pub base: Option<GpReg>,
    pub index: Option<GpReg>,
    pub scale: u8, // 1, 2, 4, 8
    pub disp: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    Reg(GpReg),
    Mem(MemoryAddr),
    Imm8(u8),
    Imm16(u16),
    Imm32(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instr {
    // Data Movement
    Mov { dest: Operand, src: Operand },
    Push(Operand),
    Pop(Operand),
    Lea { dest: GpReg, src: MemoryAddr },

    // Arithmetic / Logical
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
