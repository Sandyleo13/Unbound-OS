use core::arch::asm;

#[repr(C, packed)]
#[derive(Clone, Copy)]
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

    fn address(&'static self) -> u64 {
        self as *const Self as u64
    }

    fn limit(&'static self) -> u16 {
        (core::mem::size_of::<Self>() - 1) as u16
    }
}

static GDT: GlobalDescriptorTable =
    GlobalDescriptorTable::new();

pub fn init() {
    GDT.load();
}

pub fn verify() -> bool {
    let expected_base = GDT.address();
    let expected_limit = GDT.limit();

    let actual = unsafe {
        read_gdtr()
    };

    actual.base == expected_base
        && actual.limit == expected_limit
}

pub fn current_base() -> u64 {
    unsafe {
        read_gdtr().base
    }
}

pub fn current_limit() -> u16 {
    unsafe {
        read_gdtr().limit
    }
}

unsafe fn read_gdtr() -> DescriptorTablePointer {
    let mut pointer = DescriptorTablePointer {
        limit: 0,
        base: 0,
    };

    unsafe {
        asm!(
            "sgdt [{}]",
            in(reg) &mut pointer,
            options(nostack, preserves_flags),
        );
    }

    pointer
}
