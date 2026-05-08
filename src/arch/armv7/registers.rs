pub const REG_SP: usize = 13;
pub const REG_LR: usize = 14;
pub const REG_PC: usize = 15;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Cpsr: u32 {
        const N = 1 << 31; // Negative
        const Z = 1 << 30; // Zero
        const C = 1 << 29; // Carry
        const V = 1 << 28; // Overflow
        const Q = 1 << 27; // Saturation
        const J = 1 << 24; // Jazelle
        const E = 1 << 9;  // Endianness
        const A = 1 << 8;  // Asynchronous abort mask
        const I = 1 << 7;  // IRQ mask
        const F = 1 << 6;  // FIQ mask
        const T = 1 << 5;  // Thumb state

        // Mode bits (0-4)
        const MODE_MASK = 0b11111;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuModeBits {
    User = 0b10000,
    Fiq = 0b10001,
    Irq = 0b10010,
    Supervisor = 0b10011,
    Abort = 0b10111,
    Undefined = 0b11011,
    System = 0b11111,
}

impl CpuModeBits {
    pub fn from_u32(val: u32) -> Self {
        match val & 0b11111 {
            0b10000 => Self::User,
            0b10001 => Self::Fiq,
            0b10010 => Self::Irq,
            0b10011 => Self::Supervisor,
            0b10111 => Self::Abort,
            0b11011 => Self::Undefined,
            0b11111 => Self::System,
            _ => Self::Supervisor, // Fallback for safety, usually bad state
        }
    }
}
