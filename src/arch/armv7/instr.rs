#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Condition {
    Eq = 0,
    Ne = 1,
    Cs = 2,
    Cc = 3,
    Mi = 4,
    Pl = 5,
    Vs = 6,
    Vc = 7,
    Hi = 8,
    Ls = 9,
    Ge = 10,
    Lt = 11,
    Gt = 12,
    Le = 13,
    Al = 14,
    Nv = 15,
}

impl Condition {
    pub fn from_u32(val: u32) -> Self {
        match val & 0xF {
            0 => Condition::Eq,
            1 => Condition::Ne,
            2 => Condition::Cs,
            3 => Condition::Cc,
            4 => Condition::Mi,
            5 => Condition::Pl,
            6 => Condition::Vs,
            7 => Condition::Vc,
            8 => Condition::Hi,
            9 => Condition::Ls,
            10 => Condition::Ge,
            11 => Condition::Lt,
            12 => Condition::Gt,
            13 => Condition::Le,
            14 => Condition::Al,
            15 => Condition::Nv,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ShiftType {
    Lsl = 0,
    Lsr = 1,
    Asr = 2,
    Ror = 3,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Shift {
    Immediate { shift_type: ShiftType, amount: u32 },
    Register { shift_type: ShiftType, rs: usize },
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Operand2 {
    Immediate { val: u32, carry_out: Option<bool> },
    Register { rm: usize, shift: Shift },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Instr {
    // Data Processing
    And {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Eor {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Sub {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Rsb {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Add {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Adc {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Sbc {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Rsc {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Tst {
        cond: Condition,
        rn: usize,
        op2: Operand2,
    },
    Teq {
        cond: Condition,
        rn: usize,
        op2: Operand2,
    },
    Cmp {
        cond: Condition,
        rn: usize,
        op2: Operand2,
    },
    Cmn {
        cond: Condition,
        rn: usize,
        op2: Operand2,
    },
    Orr {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Mov {
        cond: Condition,
        s: bool,
        rd: usize,
        op2: Operand2,
    },
    Bic {
        cond: Condition,
        s: bool,
        rd: usize,
        rn: usize,
        op2: Operand2,
    },
    Mvn {
        cond: Condition,
        s: bool,
        rd: usize,
        op2: Operand2,
    },

    // Status Register Access
    Mrs {
        cond: Condition,
        rd: usize,
        use_spsr: bool,
    },
    Msr {
        cond: Condition,
        use_spsr: bool,
        mask: u8,
        op2: Operand2,
    },

    // Multiplies (32-bit & 64-bit)
    Mul {
        cond: Condition,
        s: bool,
        rd: usize,
        rm: usize,
        rs: usize,
    },
    Mla {
        cond: Condition,
        s: bool,
        rd: usize,
        rm: usize,
        rs: usize,
        rn: usize,
    },
    Umull {
        cond: Condition,
        s: bool,
        rd_lo: usize,
        rd_hi: usize,
        rm: usize,
        rs: usize,
    },
    Umlal {
        cond: Condition,
        s: bool,
        rd_lo: usize,
        rd_hi: usize,
        rm: usize,
        rs: usize,
    },
    Smull {
        cond: Condition,
        s: bool,
        rd_lo: usize,
        rd_hi: usize,
        rm: usize,
        rs: usize,
    },
    Smlal {
        cond: Condition,
        s: bool,
        rd_lo: usize,
        rd_hi: usize,
        rm: usize,
        rs: usize,
    },

    // Bit Manipulation & Extension (Media Instructions)
    Bfc {
        cond: Condition,
        rd: usize,
        lsb: u32,
        width: u32,
    },
    Bfi {
        cond: Condition,
        rd: usize,
        rn: usize,
        lsb: u32,
        width: u32,
    },
    Ubfx {
        cond: Condition,
        rd: usize,
        rn: usize,
        lsb: u32,
        width: u32,
    },
    Sbfx {
        cond: Condition,
        rd: usize,
        rn: usize,
        lsb: u32,
        width: u32,
    },
    Rev {
        cond: Condition,
        rd: usize,
        rm: usize,
    },
    Rev16 {
        cond: Condition,
        rd: usize,
        rm: usize,
    },
    Revsh {
        cond: Condition,
        rd: usize,
        rm: usize,
    },
    Sxtb {
        cond: Condition,
        rd: usize,
        rm: usize,
        rot: u8,
        rn: Option<usize>,
    },
    Sxth {
        cond: Condition,
        rd: usize,
        rm: usize,
        rot: u8,
        rn: Option<usize>,
    },
    Uxtb {
        cond: Condition,
        rd: usize,
        rm: usize,
        rot: u8,
        rn: Option<usize>,
    },
    Uxth {
        cond: Condition,
        rd: usize,
        rm: usize,
        rot: u8,
        rn: Option<usize>,
    },
    Clz {
        cond: Condition,
        rd: usize,
        rm: usize,
    },

    // Load/Store Single
    Ldr {
        cond: Condition,
        rd: usize,
        rn: usize,
        offset: Operand2,
        pre: bool,
        writeback: bool,
        up: bool,
    },
    Str {
        cond: Condition,
        rd: usize,
        rn: usize,
        offset: Operand2,
        pre: bool,
        writeback: bool,
        up: bool,
    },
    Ldrb {
        cond: Condition,
        rd: usize,
        rn: usize,
        offset: Operand2,
        pre: bool,
        writeback: bool,
        up: bool,
    },
    Strb {
        cond: Condition,
        rd: usize,
        rn: usize,
        offset: Operand2,
        pre: bool,
        writeback: bool,
        up: bool,
    },
    Ldrh {
        cond: Condition,
        rd: usize,
        rn: usize,
        offset: Operand2,
        pre: bool,
        writeback: bool,
        up: bool,
    },
    Strh {
        cond: Condition,
        rd: usize,
        rn: usize,
        offset: Operand2,
        pre: bool,
        writeback: bool,
        up: bool,
    },
    Ldrsb {
        cond: Condition,
        rd: usize,
        rn: usize,
        offset: Operand2,
        pre: bool,
        writeback: bool,
        up: bool,
    },
    Ldrsh {
        cond: Condition,
        rd: usize,
        rn: usize,
        offset: Operand2,
        pre: bool,
        writeback: bool,
        up: bool,
    },

    // Load/Store Multiple
    Ldm {
        cond: Condition,
        rn: usize,
        reg_list: u16,
        p: bool,
        u: bool,
        w: bool,
    },
    Stm {
        cond: Condition,
        rn: usize,
        reg_list: u16,
        p: bool,
        u: bool,
        w: bool,
    },

    // Branches
    B {
        cond: Condition,
        target: i32,
    },
    Bl {
        cond: Condition,
        target: i32,
    },
    Bx {
        cond: Condition,
        rm: usize,
    },
    Blx {
        cond: Condition,
        rm: usize,
    },

    // System & Control
    Svc {
        cond: Condition,
        imm: u32,
    },
    Bkpt {
        imm16: u16,
    },
    Nop {
        cond: Condition,
    },

    Unknown(u32),
}
