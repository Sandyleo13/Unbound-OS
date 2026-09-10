use super::frame_allocator::FrameAllocator;

pub const PAGE_SIZE: u64 = 4096;

const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const ADDRESS_MASK: u64 = 0x000F_FFFF_FFFF_F000;

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [u64; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [0; 512],
        }
    }

    pub fn zero(&mut self) {
        self.entries.fill(0);
    }
}

/// Convert a virtual address into:
/// PML4, PDPT, PD, PT indexes.
pub fn indexes(virtual_address: u64) -> [usize; 4] {
    [
        ((virtual_address >> 39) & 0x1FF) as usize,
        ((virtual_address >> 30) & 0x1FF) as usize,
        ((virtual_address >> 21) & 0x1FF) as usize,
        ((virtual_address >> 12) & 0x1FF) as usize,
    ]
}

pub fn entry_address(entry: u64) -> u64 {
    entry & ADDRESS_MASK
}

pub fn make_entry(address: u64, writable: bool) -> u64 {
    let mut entry = address & ADDRESS_MASK;
    entry |= PRESENT;

    if writable {
        entry |= WRITABLE;
    }

    entry
}

/// Map one 4 KiB page.
///
/// Missing intermediate page tables are allocated from the PMM.
pub fn map(
    pml4: &mut PageTable,
    allocator: &mut FrameAllocator,
    virtual_address: u64,
    physical_address: u64,
    writable: bool,
) -> Result<(), &'static str> {
    if virtual_address % PAGE_SIZE != 0 {
        return Err("virtual address is not page aligned");
    }

    if physical_address % PAGE_SIZE != 0 {
        return Err("physical address is not page aligned");
    }

    let [pml4_index, pdpt_index, pd_index, pt_index] = indexes(virtual_address);

    let pdpt_address = get_or_create_table(
        pml4,
        pml4_index,
        allocator,
        writable,
    )?;

    let pdpt = unsafe { &mut *(pdpt_address as *mut PageTable) };

    let pd_address = get_or_create_table(
        pdpt,
        pdpt_index,
        allocator,
        writable,
    )?;

    let pd = unsafe { &mut *(pd_address as *mut PageTable) };

    let pt_address = get_or_create_table(
        pd,
        pd_index,
        allocator,
        writable,
    )?;

    let pt = unsafe { &mut *(pt_address as *mut PageTable) };

    if pt.entries[pt_index] & PRESENT != 0 {
        return Err("virtual page is already mapped");
    }

    pt.entries[pt_index] = make_entry(physical_address, writable);

    Ok(())
}

/// Get an existing child page table or allocate a new one.
fn get_or_create_table(
    parent: &mut PageTable,
    index: usize,
    allocator: &mut FrameAllocator,
    writable: bool,
) -> Result<u64, &'static str> {
    let entry = parent.entries[index];

    if entry & PRESENT != 0 {
        return Ok(entry_address(entry));
    }

    let frame = allocator
        .allocate_frame()
        .ok_or("out of physical memory")?;

    let table = unsafe { &mut *(frame as *mut PageTable) };
    table.zero();

    parent.entries[index] = make_entry(frame, writable);

    Ok(frame)
}

/// Allocate a physical frame for a page table.
pub fn allocate_page_table_frame(
    allocator: &mut FrameAllocator,
) -> Option<u64> {
    allocator.allocate_frame()
}
