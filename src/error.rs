use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmuError {
    #[error("Memory access violation at address 0x{0:016X}")]
    MemoryFault(u64),

    #[error("Invalid or unimplemented instruction at 0x{0:016X}")]
    InvalidInstruction(u64),

    #[error("Invalid register ID: {0}")]
    InvalidRegister(usize),

    #[error("Hardware/Device error: {0}")]
    DeviceError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(&'static str),

    #[error("Breakpoint hit: {0}")]
    Breakpoint(u16),
}
