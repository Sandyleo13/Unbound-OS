use core::arch::asm;

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
struct SegmentDescriptor(u64);

impl SegmentDescriptor {
    const fn null() -> Self {
        Self(0)
    }

    const fn kernel_code() -> Self {
        // Present | Ring 0 | Code | Readable | Long Mode
        Self(0x00AF_9A00_0000_FFFF)
    }

    const fn kernel_data() -> Self {
        // Present | Ring 0 | Data | Writable
        Self(0x00CF_9200_0000_FFFF)
    }
}

#[repr(C, align(8))]
struct GlobalDescriptorTable {
    entries: [SegmentDescriptor; 3],
}

impl GlobalDescriptorTable {
    const fn new() -> Self {
        Self {
            entries: [
                SegmentDescriptor::null(),
                SegmentDescriptor::kernel_code(),
                SegmentDescriptor::kernel_data(),
            ],
        }
    }

    fn load(&'static self) {
        let pointer = DescriptorTablePointer {
            limit: (core::mem::size_of::<Self>() - 1) as u16,
            base: self as *const Self as u64,
        };

        unsafe {
            asm!(
                "lgdt [{}]",
                in(reg) &pointer,
                options(readonly, nostack, preserves_flags),
            );
        }
    }
}

static GDT: GlobalDescriptorTable =
    GlobalDescriptorTable::new();

pub fn init() {
    GDT.load();
}
