#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    Armv7 = 1,
    X86 = 2,
    Rv32i = 3,
}

bitflags::bitflags! {
    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CpuMode: u32 {
        const LITTLE_ENDIAN = 0;
        const BIG_ENDIAN    = 1 << 30;
        const MODE_32       = 1 << 2;
        const THUMB         = 1 << 4; // ARM specific
    }
}
