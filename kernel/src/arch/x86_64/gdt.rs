use core::arch::{asm, global_asm};

use crate::{serial_write_hex64, serial_write_string};

use super::tss;

// -----------------------------------------------------------------------------
// GDT descriptor pointer
// -----------------------------------------------------------------------------

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

// -----------------------------------------------------------------------------
// GDT layout
//
// 0x00 = null
// 0x08 = kernel code
// 0x10 = kernel data
// 0x18 = TSS low
// 0x20 = TSS high
// 0x28 = user data
// 0x30 = user code
// -----------------------------------------------------------------------------

pub const KERNEL_CODE_SELECTOR: u16 = 0x08;
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;

pub const TSS_SELECTOR: u16 = 0x18;

pub const USER_DATA_SELECTOR: u16 = 0x28;
pub const USER_CODE_SELECTOR: u16 = 0x30;

// RPL 3 selectors used by Ring 3.
pub const USER_DATA_SELECTOR_R3: u16 = USER_DATA_SELECTOR | 0x03;

pub const USER_CODE_SELECTOR_R3: u16 = USER_CODE_SELECTOR | 0x03;

// -----------------------------------------------------------------------------
// GDT entries
// -----------------------------------------------------------------------------

const GDT_NULL: u64 = 0x0000_0000_0000_0000;

const GDT_KERNEL_CODE: u64 = 0x00AF_9A00_0000_FFFF;

const GDT_KERNEL_DATA: u64 = 0x00CF_9200_0000_FFFF;

// User data:
//   Present = 1
//   DPL = 3
//   Data segment
//   Writable
//   64-bit long mode does not use the data-segment D/B bit.
const GDT_USER_DATA: u64 = 0x00CF_F200_0000_FFFF;

// User code:
//   Present = 1
//   DPL = 3
//   Code segment
//   Readable
//   Long mode
const GDT_USER_CODE: u64 = 0x00AF_FA00_0000_FFFF;

// -----------------------------------------------------------------------------
// GDT
//
// Six normal entries plus two TSS entries:
//
//   [0] null
//   [1] kernel code
//   [2] kernel data
//   [3] TSS low
//   [4] TSS high
//   [5] user data
//   [6] user code
// -----------------------------------------------------------------------------

#[repr(C, align(8))]
struct GlobalDescriptorTable {
    entries: [u64; 7],
}

impl GlobalDescriptorTable {
    const fn new() -> Self {
        Self {
            entries: [
                GDT_NULL,
                GDT_KERNEL_CODE,
                GDT_KERNEL_DATA,
                0,
                0,
                GDT_USER_DATA,
                GDT_USER_CODE,
            ],
        }
    }

    fn install_tss(&mut self) {
        let base = tss::address();
        let limit = (tss::size() - 1) as u32;

        let mut low = 0u64;

        low |= (limit as u64) & 0xFFFF;
        low |= (base & 0xFFFF) << 16;
        low |= ((base >> 16) & 0xFF) << 32;

        // Present = 1
        // DPL = 0
        // Type = 1001b
        // 64-bit available TSS
        low |= 0x89u64 << 40;

        low |= (((limit as u64) >> 16) & 0x0F) << 48;

        low |= ((base >> 24) & 0xFF) << 56;

        let high = (base >> 32) & 0xFFFF_FFFF;

        self.entries[3] = low;
        self.entries[4] = high;
    }

    fn pointer(&'static self) -> DescriptorTablePointer {
        DescriptorTablePointer {
            limit: (core::mem::size_of::<Self>() - 1) as u16,
            base: self as *const Self as u64,
        }
    }
}

// -----------------------------------------------------------------------------
// Static GDT
// -----------------------------------------------------------------------------

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable { entries: [0; 7] };

// -----------------------------------------------------------------------------
// Reload CS / DS / ES / SS
// -----------------------------------------------------------------------------

global_asm!(
    r#"
.global unbound_reload_segments
.type unbound_reload_segments, @function

unbound_reload_segments:

    /*
     * Build a 64-bit far-return frame.
     *
     * RETFQ pops:
     *
     *   [RSP + 0] = RIP
     *   [RSP + 8] = CS
     *
     * Therefore CS must be pushed FIRST and RIP SECOND.
     *
     * Stack before RETFQ:
     *
     *   RSP -> reload_done
     *          0x08
     */

    mov rax, 0x08
    push rax

    lea rax, [rip + reload_done]
    push rax

    /*
     * RETFQ
     *
     * Explicit encoding:
     *   48 CB
     */

    .byte 0x48
    .byte 0xcb

reload_done:

    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax

    ret

.size unbound_reload_segments, .-unbound_reload_segments
"#
);

unsafe extern "C" {
    fn unbound_reload_segments();
}

// -----------------------------------------------------------------------------
// GDT initialization
// -----------------------------------------------------------------------------

pub fn init() {
    serial_write_string(b"GDT: TSS INIT\r\n");

    // -------------------------------------------------------------------------
    // GDT address / size diagnostics
    // -------------------------------------------------------------------------

    let gdt_addr = core::ptr::addr_of!(GDT) as u64;

    let gdt_size = core::mem::size_of::<GlobalDescriptorTable>() as u64;

    serial_write_string(b"GDT STATIC ADDRESS: ");

    serial_write_hex64(gdt_addr);

    serial_write_string(b"\r\n");

    serial_write_string(b"GDT SIZE: ");

    serial_write_hex64(gdt_size);

    serial_write_string(b"\r\n");

    serial_write_string(b"GDT END: ");

    serial_write_hex64(gdt_addr + gdt_size);

    serial_write_string(b"\r\n");

    // -------------------------------------------------------------------------
    // Initialize the normal GDT entries
    // -------------------------------------------------------------------------

    unsafe {
        let gdt = core::ptr::addr_of_mut!(GDT).as_mut().unwrap();

        gdt.entries[0] = GDT_NULL;
        gdt.entries[1] = GDT_KERNEL_CODE;
        gdt.entries[2] = GDT_KERNEL_DATA;

        // TSS entries will be installed below.
        gdt.entries[3] = 0;
        gdt.entries[4] = 0;

        // Ring 3 descriptors.
        gdt.entries[5] = GDT_USER_DATA;
        gdt.entries[6] = GDT_USER_CODE;
    }

    // -------------------------------------------------------------------------
    // Verify initial GDT contents
    // -------------------------------------------------------------------------

    unsafe {
        let gdt = core::ptr::addr_of!(GDT);

        serial_write_string(b"GDT BEFORE TSS INIT:\r\n");

        serial_write_string(b"GDT0: ");
        serial_write_hex64((*gdt).entries[0]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT1: ");
        serial_write_hex64((*gdt).entries[1]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT2: ");
        serial_write_hex64((*gdt).entries[2]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT3: ");
        serial_write_hex64((*gdt).entries[3]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT4: ");
        serial_write_hex64((*gdt).entries[4]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT5: ");
        serial_write_hex64((*gdt).entries[5]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT6: ");
        serial_write_hex64((*gdt).entries[6]);
        serial_write_string(b"\r\n");
    }

    // -------------------------------------------------------------------------
    // TSS initialization
    // -------------------------------------------------------------------------

    serial_write_string(b"GDT: TSS INIT EXECUTE\r\n");

    tss::init();

    serial_write_string(b"GDT: TSS INIT COMPLETE\r\n");

    // -------------------------------------------------------------------------
    // TSS diagnostics
    // -------------------------------------------------------------------------

    serial_write_string(b"TSS ADDRESS: ");

    serial_write_hex64(tss::address());

    serial_write_string(b"\r\n");

    serial_write_string(b"TSS SIZE: ");

    serial_write_hex64(tss::size() as u64);

    serial_write_string(b"\r\n");

    serial_write_string(b"TSS END: ");

    serial_write_hex64(tss::address() + tss::size() as u64);

    serial_write_string(b"\r\n");

    serial_write_string(b"IST1 STACK: ");

    serial_write_hex64(tss::ist1());

    serial_write_string(b"\r\n");

    // -------------------------------------------------------------------------
    // Install TSS descriptor
    // -------------------------------------------------------------------------

    serial_write_string(b"GDT: INSTALL TSS\r\n");

    unsafe {
        core::ptr::addr_of_mut!(GDT).as_mut().unwrap().install_tss();
    }

    // -------------------------------------------------------------------------
    // Verify GDT after TSS installation
    // -------------------------------------------------------------------------

    unsafe {
        let gdt = core::ptr::addr_of!(GDT);

        serial_write_string(b"GDT AFTER INSTALL TSS:\r\n");

        serial_write_string(b"GDT0: ");
        serial_write_hex64((*gdt).entries[0]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT1: ");
        serial_write_hex64((*gdt).entries[1]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT2: ");
        serial_write_hex64((*gdt).entries[2]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT3: ");
        serial_write_hex64((*gdt).entries[3]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT4: ");
        serial_write_hex64((*gdt).entries[4]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT5: ");
        serial_write_hex64((*gdt).entries[5]);
        serial_write_string(b"\r\n");

        serial_write_string(b"GDT6: ");
        serial_write_hex64((*gdt).entries[6]);
        serial_write_string(b"\r\n");
    }

    // -------------------------------------------------------------------------
    // Prepare GDTR
    // -------------------------------------------------------------------------

    serial_write_string(b"GDT: PREPARE GDTR\r\n");

    let gdt_ptr = unsafe { core::ptr::addr_of!(GDT).as_ref().unwrap().pointer() };

    serial_write_string(b"GDT BASE: ");

    serial_write_hex64(gdt_ptr.base);

    serial_write_string(b"\r\n");

    serial_write_string(b"GDT LIMIT: ");

    serial_write_hex64(gdt_ptr.limit as u64);

    serial_write_string(b"\r\n");

    // -------------------------------------------------------------------------
    // LGDT
    // -------------------------------------------------------------------------

    serial_write_string(b"GDT: EXECUTE LGDT\r\n");

    unsafe {
        asm!(
            "lgdt [{}]",
            in(reg) &gdt_ptr,
            options(preserves_flags),
        );
    }

    serial_write_string(b"GDT: LGDT COMPLETE\r\n");

    // -------------------------------------------------------------------------
    // Reload kernel segment registers
    // -------------------------------------------------------------------------

    serial_write_string(b"GDT: RELOAD SEGMENTS\r\n");

    unsafe {
        unbound_reload_segments();
    }

    serial_write_string(b"GDT: RELOAD SEGMENTS COMPLETE\r\n");

    // -------------------------------------------------------------------------
    // Load TSS
    // -------------------------------------------------------------------------

    serial_write_string(b"GDT: LOAD TR\r\n");

    unsafe {
        load_task_register(TSS_SELECTOR);
    }

    serial_write_string(b"GDT: LOAD TR COMPLETE\r\n");

    serial_write_string(b"GDT: INIT COMPLETE\r\n");
}

// -----------------------------------------------------------------------------
// Load task register
// -----------------------------------------------------------------------------

unsafe fn load_task_register(selector: u16) {
    unsafe {
        asm!(
            "ltr {selector:x}",
            selector = in(reg) selector,
            options(preserves_flags),
        );
    }
}

// -----------------------------------------------------------------------------
// Read task register
// -----------------------------------------------------------------------------

pub fn task_register() -> u16 {
    let mut selector: u16 = 0;

    unsafe {
        asm!(
            "str {selector:x}",
            selector = out(reg) selector,
            options(preserves_flags),
        );
    }

    selector
}

// -----------------------------------------------------------------------------
// Verify GDTR
// -----------------------------------------------------------------------------

pub fn verify() -> bool {
    let expected_base = core::ptr::addr_of!(GDT) as u64;

    let expected_limit = (core::mem::size_of::<GlobalDescriptorTable>() - 1) as u16;

    let actual = unsafe { read_gdtr() };

    actual.base == expected_base && actual.limit == expected_limit
}

// -----------------------------------------------------------------------------
// Verify TR
// -----------------------------------------------------------------------------

pub fn verify_task_register() -> bool {
    task_register() == TSS_SELECTOR
}

// -----------------------------------------------------------------------------
// Verify user descriptors
// -----------------------------------------------------------------------------

pub fn verify_user_segments() -> bool {
    unsafe {
        let gdt = core::ptr::addr_of!(GDT);

        let user_data = (*gdt).entries[5];

        let user_code = (*gdt).entries[6];

        // User data:
        //   Present
        //   DPL 3
        //   Data
        //   Writable
        let user_data_ok = user_data == GDT_USER_DATA;

        // User code:
        //   Present
        //   DPL 3
        //   Code
        //   Readable
        //   Long mode
        let user_code_ok = user_code == GDT_USER_CODE;

        user_data_ok && user_code_ok
    }
}

// -----------------------------------------------------------------------------
// Current GDTR base
// -----------------------------------------------------------------------------

pub fn current_base() -> u64 {
    unsafe { read_gdtr().base }
}

// -----------------------------------------------------------------------------
// Current GDTR limit
// -----------------------------------------------------------------------------

pub fn current_limit() -> u16 {
    unsafe { read_gdtr().limit }
}

// -----------------------------------------------------------------------------
// SGDT
// -----------------------------------------------------------------------------

unsafe fn read_gdtr() -> DescriptorTablePointer {
    let mut pointer = DescriptorTablePointer { limit: 0, base: 0 };

    unsafe {
        asm!(
            "sgdt [{}]",
            in(reg) &mut pointer,
            options(preserves_flags),
        );
    }

    pointer
}
