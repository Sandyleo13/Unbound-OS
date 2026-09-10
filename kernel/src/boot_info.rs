#[repr(C)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub size: u32,

    pub boot_drive: u8,
    pub _reserved: [u8; 7],

    pub kernel_phys_start: u64,
    pub kernel_phys_end: u64,

    pub memory_map_addr: u64,
    pub memory_map_count: u32,
    pub memory_map_entry_size: u32,

    pub _reserved2: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryMapEntry {
    pub base_addr: u64,
    pub length: u64,
    pub entry_type: u32,
    pub attributes: u32,
}

pub const BOOT_INFO_MAGIC: u64 = 0x554E424F554E4442;
pub const BOOT_INFO_VERSION: u32 = 2;

pub const MEMORY_MAP_ENTRY_SIZE: u32 =
    core::mem::size_of::<MemoryMapEntry>() as u32;