use super::frame_allocator::{FRAME_SIZE, FrameAllocator};
use super::page_table::{self, PAGE_SIZE, PageTable};

/// Start of the kernel heap virtual address range.
///
/// 0x4000_0000 = 1 GiB virtual address.
pub const HEAP_START: u64 = 0x0000_0000_4000_0000;

/// Maximum kernel heap size: 16 MiB.
pub const HEAP_SIZE: u64 = 16 * 1024 * 1024;

/// End of the kernel heap virtual address range.
pub const HEAP_END: u64 = HEAP_START + HEAP_SIZE;

/// Initial kernel heap.
///
/// The heap is currently a page-backed virtual-memory region.
/// A real allocator (`GlobalAlloc`) will be added later.
pub struct KernelHeap {
    start: u64,
    size: u64,
    mapped_pages: u64,
    initialized: bool,
}

impl KernelHeap {
    /// Create an uninitialized kernel heap.
    pub const fn new() -> Self {
        Self {
            start: HEAP_START,
            size: HEAP_SIZE,
            mapped_pages: 0,
            initialized: false,
        }
    }

    /// Map physical frames into the kernel heap virtual address range.
    ///
    /// `pages` specifies how many 4 KiB pages should initially be mapped.
    pub fn initialize(
        &mut self,
        pml4: &mut PageTable,
        allocator: &mut FrameAllocator,
        pages: u64,
    ) -> Result<(), &'static str> {
        if self.initialized {
            return Err("kernel heap already initialized");
        }

        if pages == 0 {
            return Err("kernel heap page count is zero");
        }

        let required_size = pages
            .checked_mul(PAGE_SIZE)
            .ok_or("kernel heap size overflow")?;

        if required_size > self.size {
            return Err("kernel heap exceeds configured size");
        }

        let mut virtual_address = self.start;
        let mut mapped_pages = 0u64;

        for _ in 0..pages {
            let physical_address = match allocator.allocate_frame() {
                Some(frame) => frame,

                None => {
                    self.rollback(pml4, allocator, mapped_pages);

                    return Err("out of physical memory for kernel heap");
                }
            };

            match page_table::map(
                pml4,
                allocator,
                virtual_address,
                physical_address,
                true,
                false,
            ) {
                Ok(()) => {
                    mapped_pages += 1;
                    virtual_address += PAGE_SIZE;
                }

                Err(error) => {
                    allocator.free_frame(physical_address);

                    self.rollback(pml4, allocator, mapped_pages);

                    return Err(error);
                }
            }
        }

        self.mapped_pages = mapped_pages;
        self.initialized = true;

        Ok(())
    }

    /// Roll back mappings created during initialization.
    fn rollback(&mut self, pml4: &mut PageTable, allocator: &mut FrameAllocator, pages: u64) {
        let mut virtual_address = self.start;

        for _ in 0..pages {
            if let Ok(physical_address) = page_table::unmap(pml4, virtual_address) {
                allocator.free_frame(physical_address);
            }

            virtual_address += PAGE_SIZE;
        }

        self.mapped_pages = 0;
        self.initialized = false;
    }

    /// Start virtual address of the heap.
    pub const fn start(&self) -> u64 {
        self.start
    }

    /// End virtual address of the heap.
    pub const fn end(&self) -> u64 {
        self.start + self.size
    }

    /// Configured heap size in bytes.
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Number of currently mapped heap pages.
    pub const fn mapped_pages(&self) -> u64 {
        self.mapped_pages
    }

    /// Whether the heap has been initialized successfully.
    pub const fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Maximum number of 4 KiB pages available in the heap.
    pub const fn page_capacity(&self) -> u64 {
        self.size / FRAME_SIZE
    }
}
