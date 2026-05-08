use crate::bus::MemoryBus;
use crate::error::EmuError;

/// Loads a raw binary blob directly into the memory bus at the specified address.
/// Note: The memory region must already be mapped by the user prior to calling this!
pub fn load_raw(bus: &mut MemoryBus, data: &[u8], load_addr: u64) -> Result<(), EmuError> {
    log::info!(
        "Raw Loader: Copying {} bytes into memory at 0x{:016X}",
        data.len(),
        load_addr
    );

    bus.write_bytes(load_addr, data)?;

    Ok(())
}
