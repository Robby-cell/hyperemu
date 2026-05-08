pub mod raw;

#[cfg(feature = "elf")]
pub mod elf;

pub struct LoadInfo {
    pub entry_point: u64,
}
