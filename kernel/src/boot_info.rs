#[repr(C)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub size: u32,

    pub boot_drive: u8,
    pub _reserved: [u8; 7],

    pub kernel_phys_start: u64,
    pub kernel_phys_end: u64,
}

pub const BOOT_INFO_MAGIC: u64 = 0x554E424F554E4442;
pub const BOOT_INFO_VERSION: u32 = 1;
