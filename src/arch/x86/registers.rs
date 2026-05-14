pub const REG_EAX: usize = 0;
pub const REG_ECX: usize = 1;
pub const REG_EDX: usize = 2;
pub const REG_EBX: usize = 3;
pub const REG_ESP: usize = 4;
pub const REG_EBP: usize = 5;
pub const REG_ESI: usize = 6;
pub const REG_EDI: usize = 7;
pub const REG_EIP: usize = 8;
pub const REG_EFLAGS: usize = 9;

// Segment Registers (Stored as 16-bit selectors in a 32-bit slot)
pub const REG_CS: usize = 10;
pub const REG_DS: usize = 11;
pub const REG_ES: usize = 12;
pub const REG_SS: usize = 13;
pub const REG_FS: usize = 14;
pub const REG_GS: usize = 15;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EFlags: u32 {
        const CF = 1 << 0;   // Carry Flag
        const RSVD1 = 1 << 1; // Reserved, always 1 in hardware
        const PF = 1 << 2;   // Parity Flag
        const AF = 1 << 4;   // Auxiliary Carry Flag
        const ZF = 1 << 6;   // Zero Flag
        const SF = 1 << 7;   // Sign Flag
        const TF = 1 << 8;   // Trap Flag
        const IF = 1 << 9;   // Interrupt Enable Flag
        const DF = 1 << 10;  // Direction Flag
        const OF = 1 << 11;  // Overflow Flag
        const IOPL = 3 << 12; // I/O Privilege Level
        const NT = 1 << 14;  // Nested Task Flag
        const RF = 1 << 16;  // Resume Flag
        const VM = 1 << 17;  // Virtual 8086 Mode
        const AC = 1 << 18;  // Alignment Check
        const VIF = 1 << 19; // Virtual Interrupt Flag
        const VIP = 1 << 20; // Virtual Interrupt Pending
        const ID = 1 << 21;  // ID Flag
    }
}
