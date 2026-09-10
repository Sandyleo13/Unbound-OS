use crate::boot_info::MemoryMapEntry;

pub const FRAME_SIZE: u64 = 4096;

/// Initial PMM limit: 4 GiB of physical address space.
pub const MAX_PHYSICAL_MEMORY: u64 = 4 * 1024 * 1024 * 1024;

pub const MAX_FRAMES: usize = (MAX_PHYSICAL_MEMORY / FRAME_SIZE) as usize;
pub const BITMAP_SIZE: usize = MAX_FRAMES / 8;

/// A frame is free/available when its bit is 0.
/// A frame is reserved or allocated when its bit is 1.
static mut FRAME_BITMAP: [u8; BITMAP_SIZE] = [0; BITMAP_SIZE];

/// Tracks frames specifically handed out by allocate_frame().
///
/// This lets free_frame() reject attempts to free bootloader/kernel
/// reserved frames that were never allocated by this allocator.
static mut ALLOCATED_BITMAP: [u8; BITMAP_SIZE] = [0; BITMAP_SIZE];

pub struct FrameAllocator {
    free_frames: usize,
}

impl FrameAllocator {
    pub const fn empty() -> Self {
        Self {
            free_frames: 0,
        }
    }

    pub fn initialize(
        &mut self,
        memory_map: &[MemoryMapEntry],
        kernel_start: u64,
        kernel_end: u64,
    ) {
        /*
         * Start with every frame reserved.
         *
         * Only E820 type-1 memory will subsequently be released.
         */
        unsafe {
            let ptr = core::ptr::addr_of_mut!(FRAME_BITMAP);
            core::ptr::write_bytes(ptr.cast::<u8>(), 0xFF, BITMAP_SIZE);

            let ptr = core::ptr::addr_of_mut!(ALLOCATED_BITMAP);
            core::ptr::write_bytes(ptr.cast::<u8>(), 0x00, BITMAP_SIZE);
        }

        self.free_frames = 0;

        /*
         * Release frames belonging to usable E820 regions.
         */
        for entry in memory_map {
            if entry.entry_type != 1 {
                continue;
            }

            let region_start = align_up(entry.base_addr, FRAME_SIZE);

            let region_end = match entry.base_addr.checked_add(entry.length) {
                Some(end) => align_down(end, FRAME_SIZE),
                None => continue,
            };

            if region_start >= region_end {
                continue;
            }

            let region_end = region_end.min(MAX_PHYSICAL_MEMORY);

            let mut address = region_start;

            while address < region_end {
                let frame = (address / FRAME_SIZE) as usize;

                if frame < MAX_FRAMES && self.is_used(frame) {
                    self.mark_free(frame);
                    self.free_frames += 1;
                }

                address += FRAME_SIZE;
            }
        }

        /*
         * Reserve the first 1 MiB.
         *
         * This contains BIOS/legacy memory and bootloader-owned data.
         */
        self.reserve_range(0x00000000, 0x00100000);

        /*
         * Reserve the Stage 2 image.
         *
         * Stage 1 loads Stage 2 at 0x8000.
         * Stage 2 occupies exactly 32 sectors = 16 KiB.
         */
        self.reserve_range(0x00008000, 0x0000C000);

        /*
         * Reserve the temporary kernel ELF buffer.
         *
         * Stage 2 loads the kernel beginning at 0x20000.
         *
         * Current kernel read size is 42 sectors:
         * 42 * 512 = 21504 bytes.
         */
        self.reserve_range(0x00020000, 0x00024000);

        /*
         * Reserve the E820 memory-map buffer.
         */
        self.reserve_range(0x00060000, 0x00061000);

        /*
         * Reserve BootInfo.
         */
        self.reserve_range(0x00070000, 0x00071000);

        /*
         * Reserve the initial kernel stack.
         */
        self.reserve_range(0x00090000, 0x00091000);

        /*
         * Reserve the loaded kernel.
         *
         * The range is rounded to complete pages so no page containing
         * kernel code/data/BSS can be allocated.
         */
        self.reserve_range(kernel_start, kernel_end);
    }

    pub fn allocate_frame(&mut self) -> Option<u64> {
        for frame in 0..MAX_FRAMES {
            if !self.is_used(frame) {
                self.mark_used(frame);
                self.mark_allocated(frame);
                self.free_frames -= 1;

                return Some(frame as u64 * FRAME_SIZE);
            }
        }

        None
    }

    pub fn free_frame(&mut self, address: u64) {
        if address % FRAME_SIZE != 0 {
            return;
        }

        let frame = (address / FRAME_SIZE) as usize;

        if frame >= MAX_FRAMES {
            return;
        }

        /*
         * Only frames actually handed out by allocate_frame() may be freed.
         */
        if !self.is_allocated(frame) {
            return;
        }

        self.mark_free(frame);
        self.mark_unallocated(frame);
        self.free_frames += 1;
    }

    pub fn free_frames(&self) -> usize {
        self.free_frames
    }

    fn reserve_range(&mut self, start: u64, end: u64) {
        let start = align_down(start, FRAME_SIZE);
        let end = align_up(end, FRAME_SIZE);

        let mut address = start;

        while address < end {
            let frame = (address / FRAME_SIZE) as usize;

            if frame < MAX_FRAMES && !self.is_used(frame) {
                self.mark_used(frame);
                self.free_frames -= 1;
            }

            address += FRAME_SIZE;
        }
    }

    fn is_used(&self, frame: usize) -> bool {
        unsafe {
            let byte = FRAME_BITMAP[frame / 8];
            let mask = 1u8 << (frame % 8);

            byte & mask != 0
        }
    }

    fn mark_used(&mut self, frame: usize) {
        unsafe {
            FRAME_BITMAP[frame / 8] |= 1u8 << (frame % 8);
        }
    }

    fn mark_free(&mut self, frame: usize) {
        unsafe {
            FRAME_BITMAP[frame / 8] &= !(1u8 << (frame % 8));
        }
    }

    fn is_allocated(&self, frame: usize) -> bool {
        unsafe {
            let byte = ALLOCATED_BITMAP[frame / 8];
            let mask = 1u8 << (frame % 8);

            byte & mask != 0
        }
    }

    fn mark_allocated(&mut self, frame: usize) {
        unsafe {
            ALLOCATED_BITMAP[frame / 8] |= 1u8 << (frame % 8);
        }
    }

    fn mark_unallocated(&mut self, frame: usize) {
        unsafe {
            ALLOCATED_BITMAP[frame / 8] &= !(1u8 << (frame % 8));
        }
    }
}

fn align_up(value: u64, alignment: u64) -> u64 {
    if value % alignment == 0 {
        value
    } else {
        value + (alignment - value % alignment)
    }
}

fn align_down(value: u64, alignment: u64) -> u64 {
    value - (value % alignment)
}
