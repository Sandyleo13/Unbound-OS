use super::frame_allocator::FrameAllocator;
use super::page_table::{map, PageTable, PAGE_SIZE};

pub struct AddressSpace {
    pub pml4_phys: u64,
}

impl AddressSpace {
    /// Create a fresh address space with an empty PML4.
    pub fn new(allocator: &mut FrameAllocator) -> Result<Self, &'static str> {
        let pml4_phys = allocator
            .allocate_frame()
            .ok_or("out of physical memory")?;

        let pml4 = unsafe { &mut *(pml4_phys as *mut PageTable) };
        pml4.zero();

        Ok(Self { pml4_phys })
    }

    /// Identity-map a physical range.
    ///
    /// Virtual address == physical address.
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

        let pml4 = unsafe { &mut *(self.pml4_phys as *mut PageTable) };

        let mut address = start;

        while address < end {
            map(
                pml4,
                allocator,
                address,
                address,
                writable,
            )?;

            address += PAGE_SIZE;
        }

        Ok(())
    }

    /// Return the physical address of the PML4.
    pub fn pml4_phys(&self) -> u64 {
        self.pml4_phys
    }
}
