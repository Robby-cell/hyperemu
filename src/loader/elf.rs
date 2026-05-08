use super::LoadInfo;
use crate::bus::{MemoryBus, Perms};
use crate::device::ram::Ram;
use crate::error::EmuError;
use goblin::elf::{Elf, program_header::PT_LOAD};

pub fn load_elf(bus: &mut MemoryBus, data: &[u8]) -> Result<LoadInfo, EmuError> {
    let elf = Elf::parse(data)
        .map_err(|e| EmuError::DeviceError(format!("Failed to parse ELF: {}", e)))?;

    for ph in &elf.program_headers {
        if ph.p_type == PT_LOAD {
            let vaddr = ph.p_vaddr;
            let memsz = ph.p_memsz;
            let filesz = ph.p_filesz;

            if memsz == 0 {
                continue;
            }

            // Determine Memory Permissions
            let mut perms = Perms::empty();
            if ph.is_read() {
                perms |= Perms::R;
            }
            if ph.is_write() {
                perms |= Perms::W;
            }
            if ph.is_executable() {
                perms |= Perms::X;
            }
            if perms.is_empty() {
                perms = Perms::RWX;
            } // Fallback

            // Map RAM device at the specified Virtual Address
            log::info!(
                "ELF Loader: Mapping 0x{:X} bytes at 0x{:08X} (Perms: {:?})",
                memsz,
                vaddr,
                perms
            );
            bus.map(vaddr, memsz, perms, Box::new(Ram::new(memsz as usize)));

            // Copy the actual file data into the newly mapped RAM
            // If memsz > filesz, the remainder is left as 0s (This correctly handles the .bss section).
            if filesz > 0 {
                let file_start = ph.p_offset as usize;
                let file_end = file_start + (filesz as usize);
                let slice = &data[file_start..file_end];

                bus.write_bytes(vaddr, slice)?;
            }
        }
    }

    Ok(LoadInfo {
        entry_point: elf.header.e_entry,
    })
}
