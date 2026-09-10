use super::address::phys_to_virt;
use super::frame_allocator::FrameAllocator;

pub const PAGE_SIZE: u64 = 4096;

const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const WRITE_THROUGH: u64 = 1 << 3;
const CACHE_DISABLE: u64 = 1 << 4;
const ACCESSED: u64 = 1 << 5;
const DIRTY: u64 = 1 << 6;
const HUGE_PAGE: u64 = 1 << 7;
const GLOBAL: u64 = 1 << 8;
const NO_EXECUTE: u64 = 1 << 63;

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

/// Check whether a virtual address is canonical in x86_64 long mode.
///
/// With the current 48-bit virtual-address configuration, bits 63:48
/// must be a sign extension of bit 47.
#[inline]
pub fn is_canonical(virtual_address: u64) -> bool {
    let upper = virtual_address >> 48;
    let sign = (virtual_address >> 47) & 1;

    if sign == 0 {
        upper == 0
    } else {
        upper == 0xFFFF
    }
}

/// Return the physical address stored in a page-table entry.
pub fn entry_address(entry: u64) -> u64 {
    entry & ADDRESS_MASK
}

/// Build a normal 4 KiB page-table entry.
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
    if !is_canonical(virtual_address) {
        return Err("virtual address is not canonical");
    }

    if virtual_address % PAGE_SIZE != 0 {
        return Err("virtual address is not page aligned");
    }

    if physical_address % PAGE_SIZE != 0 {
        return Err("physical address is not page aligned");
    }

    if physical_address > ADDRESS_MASK {
        return Err("physical address exceeds supported page-table range");
    }

    let [pml4_index, pdpt_index, pd_index, pt_index] =
        indexes(virtual_address);

    let pdpt_address = get_or_create_table(
        pml4,
        pml4_index,
        allocator,
        writable,
    )?;

    let pdpt =
        unsafe { &mut *(phys_to_virt(pdpt_address) as *mut PageTable) };

    let pd_address = get_or_create_table(
        pdpt,
        pdpt_index,
        allocator,
        writable,
    )?;

    let pd =
        unsafe { &mut *(phys_to_virt(pd_address) as *mut PageTable) };

    let pt_address = get_or_create_table(
        pd,
        pd_index,
        allocator,
        writable,
    )?;

    let pt =
        unsafe { &mut *(phys_to_virt(pt_address) as *mut PageTable) };

    if pt.entries[pt_index] & PRESENT != 0 {
        return Err("virtual page is already mapped");
    }

    pt.entries[pt_index] = make_entry(physical_address, writable);

    Ok(())
}

/// Translate a virtual address using the supplied PML4.
///
/// Returns the physical address corresponding to the virtual address.
pub fn translate(
    pml4: &PageTable,
    virtual_address: u64,
) -> Option<u64> {
    if !is_canonical(virtual_address) {
        return None;
    }

    let [pml4_index, pdpt_index, pd_index, pt_index] =
        indexes(virtual_address);

    let pml4_entry = pml4.entries[pml4_index];

    if pml4_entry & PRESENT == 0 {
        return None;
    }

    let pdpt_address = entry_address(pml4_entry);

    let pdpt =
        unsafe { &*(phys_to_virt(pdpt_address) as *const PageTable) };

    let pdpt_entry = pdpt.entries[pdpt_index];

    if pdpt_entry & PRESENT == 0 {
        return None;
    }

    if pdpt_entry & HUGE_PAGE != 0 {
        let base = pdpt_entry & 0x000F_FFC0_0000_0000;
        return Some(base + (virtual_address & 0x3FFF_FFFF));
    }

    let pd_address = entry_address(pdpt_entry);

    let pd =
        unsafe { &*(phys_to_virt(pd_address) as *const PageTable) };

    let pd_entry = pd.entries[pd_index];

    if pd_entry & PRESENT == 0 {
        return None;
    }

    if pd_entry & HUGE_PAGE != 0 {
        let base = pd_entry & 0x000F_FFFF_FFE0_0000;
        return Some(base + (virtual_address & 0x1F_FFFF));
    }

    let pt_address = entry_address(pd_entry);

    let pt =
        unsafe { &*(phys_to_virt(pt_address) as *const PageTable) };

    let pt_entry = pt.entries[pt_index];

    if pt_entry & PRESENT == 0 {
        return None;
    }

    Some(entry_address(pt_entry) + (virtual_address & 0xFFF))
}

/// Unmap one 4 KiB virtual page.
///
/// The physical frame is not freed here. This function only removes
/// the virtual-to-physical mapping and returns the physical address
/// that was previously mapped.
pub fn unmap(
    pml4: &mut PageTable,
    virtual_address: u64,
) -> Result<u64, &'static str> {
    if !is_canonical(virtual_address) {
        return Err("virtual address is not canonical");
    }

    if virtual_address % PAGE_SIZE != 0 {
        return Err("virtual address is not page aligned");
    }

    let [pml4_index, pdpt_index, pd_index, pt_index] =
        indexes(virtual_address);

    let pml4_entry = pml4.entries[pml4_index];

    if pml4_entry & PRESENT == 0 {
        return Err("PML4 entry is not present");
    }

    let pdpt_address = entry_address(pml4_entry);

    let pdpt =
        unsafe { &mut *(phys_to_virt(pdpt_address) as *mut PageTable) };

    let pdpt_entry = pdpt.entries[pdpt_index];

    if pdpt_entry & PRESENT == 0 {
        return Err("PDPT entry is not present");
    }

    if pdpt_entry & HUGE_PAGE != 0 {
        return Err("cannot unmap a huge page with 4 KiB unmap");
    }

    let pd_address = entry_address(pdpt_entry);

    let pd =
        unsafe { &mut *(phys_to_virt(pd_address) as *mut PageTable) };

    let pd_entry = pd.entries[pd_index];

    if pd_entry & PRESENT == 0 {
        return Err("PD entry is not present");
    }

    if pd_entry & HUGE_PAGE != 0 {
        return Err("cannot unmap a huge page with 4 KiB unmap");
    }

    let pt_address = entry_address(pd_entry);

    let pt =
        unsafe { &mut *(phys_to_virt(pt_address) as *mut PageTable) };

    let pte = pt.entries[pt_index];

    if pte & PRESENT == 0 {
        return Err("virtual page is not mapped");
    }

    let physical_address = entry_address(pte);

    pt.entries[pt_index] = 0;

    Ok(physical_address)
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

    let table =
        unsafe { &mut *(phys_to_virt(frame) as *mut PageTable) };

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
