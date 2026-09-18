use core::arch::asm;

use super::address::phys_to_virt;
use super::frame_allocator::FrameAllocator;
use super::page_table::{PAGE_SIZE, PageTable, map};

pub struct AddressSpace {
    pub pml4_phys: u64,
}

impl AddressSpace {
    /// Create a fresh address space with an empty PML4.
    pub fn new(allocator: &mut FrameAllocator) -> Result<Self, &'static str> {
        let pml4_phys = allocator.allocate_frame().ok_or("out of physical memory")?;

        let pml4 = unsafe { &mut *(phys_to_virt(pml4_phys) as *mut PageTable) };

        pml4.zero();

        Ok(Self { pml4_phys })
    }

    /// Identity-map a physical range.
    ///
    /// Virtual address == physical address.
    ///
    /// Identity mappings created here are supervisor-only.
    /// User mappings must explicitly pass `user = true`
    /// through the lower-level `page_table::map()` API.
    pub fn identity_map(
        &mut self,
        allocator: &mut FrameAllocator,
        start: u64,
        end: u64,
        writable: bool,
    ) -> Result<(), &'static str> {
        if start % PAGE_SIZE != 0 {
            return Err("identity-map start is not page aligned");
        }

        if end % PAGE_SIZE != 0 {
            return Err("identity-map end is not page aligned");
        }

        if start >= end {
            return Err("identity-map range is empty");
        }

        let pml4 = unsafe { &mut *(phys_to_virt(self.pml4_phys) as *mut PageTable) };

        let mut address = start;

        while address < end {
            map(pml4, allocator, address, address, writable, false)?;

            address += PAGE_SIZE;
        }

        Ok(())
    }

    /// Activate this address space by loading its PML4 into CR3.
    pub fn activate(&self) {
        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) self.pml4_phys,
                options(nostack, preserves_flags),
            );
        }
    }

    /// Return the physical address of the PML4.
    pub fn pml4_phys(&self) -> u64 {
        self.pml4_phys
    }
}
