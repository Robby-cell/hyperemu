pub const REG_ZERO: usize = 0;
pub const REG_RA: usize = 1; // Return Address
pub const REG_SP: usize = 2; // Stack Pointer
pub const REG_GP: usize = 3; // Global Pointer
pub const REG_TP: usize = 4; // Thread Pointer

// Temporary / Argument / Saved Registers
pub const REG_T0: usize = 5;
pub const REG_T1: usize = 6;
pub const REG_T2: usize = 7;
pub const REG_S0: usize = 8; // Frame Pointer (FP)
pub const REG_S1: usize = 9;
pub const REG_A0: usize = 10;
pub const REG_A1: usize = 11;
pub const REG_A2: usize = 12;
pub const REG_A3: usize = 13;
pub const REG_A4: usize = 14;
pub const REG_A5: usize = 15;
pub const REG_A6: usize = 16;
pub const REG_A7: usize = 17;
pub const REG_S2: usize = 18;
pub const REG_S3: usize = 19;
pub const REG_S4: usize = 20;
pub const REG_S5: usize = 21;
pub const REG_S6: usize = 22;
pub const REG_S7: usize = 23;
pub const REG_S8: usize = 24;
pub const REG_S9: usize = 25;
pub const REG_S10: usize = 26;
pub const REG_S11: usize = 27;
pub const REG_T3: usize = 28;
pub const REG_T4: usize = 29;
pub const REG_T5: usize = 30;
pub const REG_T6: usize = 31;

pub const REG_PC: usize = 32; // Not in the standard 32, but handled by the CPU state
