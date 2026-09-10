pub const IDENTITY_MAP_BASE: u64 = 0;

#[inline]
pub fn phys_to_virt(physical_address: u64) -> u64 {
    physical_address + IDENTITY_MAP_BASE
}
