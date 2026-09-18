#![no_std]
#![no_main]

#[path = "../arch/mod.rs"]
mod arch;

#[path = "../boot_info.rs"]
mod boot_info;

#[path = "../memory/mod.rs"]
mod memory;

#[path = "../scheduler/mod.rs"]
mod scheduler;

#[path = "../sync/mod.rs"]
mod sync;

#[path = "../objects/mod.rs"]
mod objects;

#[path = "../ipc/mod.rs"]
mod ipc;

#[path = "../syscall/mod.rs"]
mod syscall;

use boot_info::{
    BOOT_INFO_MAGIC, BOOT_INFO_VERSION, BootInfo, MEMORY_MAP_ENTRY_SIZE, MemoryMapEntry,
};

use core::arch::asm;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU64, Ordering};

use memory::address_space::AddressSpace;
use memory::frame_allocator::FrameAllocator;
use memory::heap::KernelHeap;
use memory::page_table::PageTable;

// ================================================================
// DEBUG STATE
// ================================================================

static TIMER_IRQ_DEBUG_COUNT: AtomicU64 = AtomicU64::new(0);

static EXCEPTION_DEBUG_COUNT: AtomicU64 = AtomicU64::new(0);

// ================================================================
// KERNEL OBJECT TABLE
// ================================================================

static KERNEL_OBJECTS: objects::ObjectTable = objects::ObjectTable::new();

// ================================================================
// IPC MANAGER
// ================================================================

static IPC_MANAGER: ipc::IpcManager = ipc::IpcManager::new();

// ================================================================
// KERNEL ENTRY
// ================================================================

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    serial_write_string(b"KERNEL ENTRY\r\n");

    // ============================================================
    // BOOTINFO VALIDATION
    // ============================================================

    if boot_info.is_null() {
        serial_write_string(b"BOOTINFO NULL\r\n");
        halt();
    }

    serial_write_string(b"BOOTINFO PTR OK\r\n");

    let info = unsafe { &*boot_info };

    serial_write_string(b"BOOTINFO READ OK\r\n");

    if info.magic != BOOT_INFO_MAGIC {
        serial_write_string(b"BAD MAGIC\r\n");
        halt();
    }

    if info.version != BOOT_INFO_VERSION {
        serial_write_string(b"BAD VERSION\r\n");
        halt();
    }

    if info.memory_map_addr == 0 {
        serial_write_string(b"BAD MEMORY MAP ADDRESS\r\n");
        halt();
    }

    if info.memory_map_count == 0 {
        serial_write_string(b"BAD MEMORY MAP COUNT\r\n");
        halt();
    }

    if info.memory_map_entry_size != MEMORY_MAP_ENTRY_SIZE {
        serial_write_string(b"BAD MEMORY MAP ENTRY SIZE\r\n");
        halt();
    }

    if info.size < core::mem::size_of::<BootInfo>() as u32 {
        serial_write_string(b"BAD SIZE\r\n");
        halt();
    }

    if info.kernel_phys_end <= info.kernel_phys_start {
        serial_write_string(b"BAD KERNEL RANGE\r\n");
        halt();
    }

    serial_write_string(b"BOOTINFO OK\r\n");

    // ============================================================
    // GDT
    // ============================================================

    serial_write_string(b"\r\nGDT INITIALIZATION\r\n");

    arch::x86_64::gdt::init();

    serial_write_string(b"GDT: PASS\r\n");

    serial_write_string(b"GDT VERIFICATION\r\n");

    serial_write_string(b"GDTR BASE: 0x");

    serial_write_hex64(arch::x86_64::gdt::current_base());

    serial_write_string(b"\r\n");

    serial_write_string(b"GDTR LIMIT: 0x");

    serial_write_hex32(arch::x86_64::gdt::current_limit() as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"GDT USER SEGMENTS VERIFY: ");

    if arch::x86_64::gdt::verify_user_segments() {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    // ============================================================
    // TSS / IST
    // ============================================================

    serial_write_string(b"\r\nTSS INITIALIZATION\r\n");

    serial_write_string(b"TR: 0x");

    serial_write_hex32(arch::x86_64::gdt::task_register() as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"TSS VERIFY: ");

    if arch::x86_64::gdt::verify_task_register() {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    let ist1 = arch::x86_64::tss::ist1();

    serial_write_string(b"IST1 STACK TOP: 0x");

    serial_write_hex64(ist1);

    serial_write_string(b"\r\n");

    serial_write_string(b"IST1 ALIGNMENT: ");

    if ist1 != 0 && (ist1 & 0x0F) == 0 {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"IST1 NON-NULL: ");

    if ist1 != 0 {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"TSS/IST INITIALIZATION: PASS\r\n");

    // ============================================================
    // IDT
    // ============================================================

    serial_write_string(b"\r\nIDT INITIALIZATION\r\n");

    arch::x86_64::idt::init();

    serial_write_string(b"IDT: PASS\r\n");

    serial_write_string(b"IDT VERIFICATION\r\n");

    serial_write_string(b"IDTR BASE: 0x");

    serial_write_hex64(arch::x86_64::idt::current_base());

    serial_write_string(b"\r\n");

    serial_write_string(b"IDTR LIMIT: 0x");

    serial_write_hex32(arch::x86_64::idt::current_limit() as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"IDT VERIFY: ");

    if arch::x86_64::idt::verify() {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    // ============================================================
    // IDT VECTOR 3 DEBUG
    // ============================================================

    serial_write_string(b"\r\nIDT[3] DEBUG\r\n");

    unsafe {
        let idt_base = arch::x86_64::idt::current_base();

        serial_write_string(b"  IDT BASE: 0x");

        serial_write_hex64(idt_base);

        serial_write_string(b"\r\n");

        let entry_addr = idt_base + (3 * 16);

        serial_write_string(b"  IDT[3] ADDRESS: 0x");

        serial_write_hex64(entry_addr);

        serial_write_string(b"\r\n");

        let entry = core::slice::from_raw_parts(entry_addr as *const u8, 16);

        serial_write_string(b"  IDT[3] BYTES: ");

        for i in 0..16 {
            serial_write_hex8(entry[i]);
            serial_write_string(b" ");
        }

        serial_write_string(b"\r\n");
    }

    // ============================================================
    // EARLY CPU EXCEPTION TEST
    // ============================================================

    serial_write_string(b"\r\nEARLY EXCEPTION TEST: INT3\r\n");

    unsafe {
        asm!("int3");
    }

    serial_write_string(b"EARLY EXCEPTION TEST: INT3 RETURNED\r\n");

    // ============================================================
    // BOOT INFORMATION
    // ============================================================

    serial_write_string(b"MEMORY MAP ENTRIES: ");

    serial_write_hex32(info.memory_map_count);

    serial_write_string(b"\r\n");

    serial_write_string(b"BOOT DRIVE: 0x");

    serial_write_hex8(info.boot_drive);

    serial_write_string(b"\r\n");

    serial_write_string(b"KERNEL START: 0x");

    serial_write_hex64(info.kernel_phys_start);

    serial_write_string(b"\r\n");

    serial_write_string(b"KERNEL END: 0x");

    serial_write_hex64(info.kernel_phys_end);

    serial_write_string(b"\r\n");

    // ============================================================
    // E820 MEMORY MAP
    // ============================================================

    serial_write_string(b"\r\nE820 MEMORY MAP\r\n");

    let entries = unsafe {
        core::slice::from_raw_parts(
            info.memory_map_addr as *const MemoryMapEntry,
            info.memory_map_count as usize,
        )
    };

    for (index, entry) in entries.iter().enumerate() {
        serial_write_string(b"ENTRY ");

        serial_write_hex32(index as u32);

        serial_write_string(b"\r\n");

        serial_write_string(b"  BASE: 0x");

        serial_write_hex64(entry.base_addr);

        serial_write_string(b"\r\n");

        serial_write_string(b"  LENGTH: 0x");

        serial_write_hex64(entry.length);

        serial_write_string(b"\r\n");

        serial_write_string(b"  TYPE: 0x");

        serial_write_hex32(entry.entry_type);

        serial_write_string(b"\r\n");

        serial_write_string(b"  ATTR: 0x");

        serial_write_hex32(entry.attributes);

        serial_write_string(b"\r\n");
    }

    // ============================================================
    // PHYSICAL MEMORY MANAGER
    // ============================================================

    let mut frame_allocator = FrameAllocator::empty();

    frame_allocator.initialize(entries, info.kernel_phys_start, info.kernel_phys_end);

    serial_write_string(b"\r\nPHYSICAL MEMORY MANAGER\r\n");

    serial_write_string(b"FREE FRAMES: ");

    serial_write_hex64(frame_allocator.free_frames() as u64);

    serial_write_string(b"\r\n");

    // ============================================================
    // PHYSICAL FRAME ALLOCATION TEST
    // ============================================================

    serial_write_string(b"ALLOCATING TEST FRAMES\r\n");

    let frame1 = frame_allocator.allocate_frame();

    let frame2 = frame_allocator.allocate_frame();

    let frame3 = frame_allocator.allocate_frame();

    serial_write_string(b"FRAME 1: 0x");

    serial_write_hex64(frame1.unwrap_or(0));

    serial_write_string(b"\r\n");

    serial_write_string(b"FRAME 2: 0x");

    serial_write_hex64(frame2.unwrap_or(0));

    serial_write_string(b"\r\n");

    serial_write_string(b"FRAME 3: 0x");

    serial_write_hex64(frame3.unwrap_or(0));

    serial_write_string(b"\r\n");

    serial_write_string(b"FREE FRAMES AFTER ALLOC: ");

    serial_write_hex64(frame_allocator.free_frames() as u64);

    serial_write_string(b"\r\n");

    if let Some(frame) = frame1 {
        frame_allocator.free_frame(frame);
    }

    serial_write_string(b"FREE FRAMES AFTER FREE: ");

    serial_write_hex64(frame_allocator.free_frames() as u64);

    serial_write_string(b"\r\n");

    // ============================================================
    // RESERVED FRAME FREE TEST
    // ============================================================

    let free_before_reserved_test = frame_allocator.free_frames();

    frame_allocator.free_frame(0x00000000);

    let free_after_reserved_test = frame_allocator.free_frames();

    serial_write_string(b"RESERVED FRAME FREE TEST: ");

    if free_before_reserved_test == free_after_reserved_test {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
    }

    // ============================================================
    // VIRTUAL MEMORY TEST
    // ============================================================

    serial_write_string(b"\r\nVIRTUAL MEMORY TEST\r\n");

    let pml4_frame = frame_allocator.allocate_frame();

    if let Some(pml4_frame) = pml4_frame {
        let pml4 = unsafe { &mut *(pml4_frame as *mut PageTable) };

        pml4.zero();

        let virtual_address = 0x0040_0000;

        let physical_address = 0x0010_0000;

        match memory::page_table::map(
            pml4,
            &mut frame_allocator,
            virtual_address,
            physical_address,
            true,
            false,
        ) {
            Ok(()) => {
                let [pml4_index, pdpt_index, pd_index, pt_index] =
                    memory::page_table::indexes(virtual_address);

                let pdpt_address = memory::page_table::entry_address(pml4.entries[pml4_index]);

                let pdpt = unsafe { &*(pdpt_address as *const PageTable) };

                let pd_address = memory::page_table::entry_address(pdpt.entries[pdpt_index]);

                let pd = unsafe { &*(pd_address as *const PageTable) };

                let pt_address = memory::page_table::entry_address(pd.entries[pd_index]);

                let pt = unsafe { &*(pt_address as *const PageTable) };

                let mapped_address = memory::page_table::entry_address(pt.entries[pt_index]);

                serial_write_string(b"VIRTUAL ADDRESS: 0x");

                serial_write_hex64(virtual_address);

                serial_write_string(b"\r\n");

                serial_write_string(b"EXPECTED PHYSICAL: 0x");

                serial_write_hex64(physical_address);

                serial_write_string(b"\r\n");

                serial_write_string(b"ACTUAL PHYSICAL: 0x");

                serial_write_hex64(mapped_address);

                serial_write_string(b"\r\n");

                serial_write_string(b"PAGE TABLE MAP TEST: ");

                if mapped_address == physical_address {
                    serial_write_string(b"PASS\r\n");
                } else {
                    serial_write_string(b"FAIL\r\n");
                }
            }

            Err(error) => {
                serial_write_string(b"PAGE TABLE MAP ERROR: ");

                serial_write_string(error.as_bytes());

                serial_write_string(b"\r\n");
            }
        }
    } else {
        serial_write_string(b"PML4 ALLOCATION: FAIL\r\n");
    }

    serial_write_string(b"VIRTUAL MEMORY TEST COMPLETE\r\n");

    // ============================================================
    // CANONICAL ADDRESS TEST
    // ============================================================

    serial_write_string(b"\r\nCANONICAL ADDRESS TEST\r\n");

    let canonical_low = 0x0000_7FFF_FFFF_F000u64;

    let canonical_high = 0xFFFF_8000_0000_0000u64;

    let non_canonical_low = 0x0000_8000_0000_0000u64;

    let non_canonical_high = 0xFFFF_7FFF_FFFF_FFFFu64;

    serial_write_string(b"LOW CANONICAL: ");

    if memory::page_table::is_canonical(canonical_low) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
    }

    serial_write_string(b"HIGH CANONICAL: ");

    if memory::page_table::is_canonical(canonical_high) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
    }

    serial_write_string(b"LOW NON-CANONICAL: ");

    if !memory::page_table::is_canonical(non_canonical_low) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
    }

    serial_write_string(b"HIGH NON-CANONICAL: ");

    if !memory::page_table::is_canonical(non_canonical_high) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
    }

    serial_write_string(b"CANONICAL ADDRESS TEST COMPLETE\r\n");

    // ============================================================
    // ADDRESS SPACE TEST
    // ============================================================

    serial_write_string(b"\r\nADDRESS SPACE TEST\r\n");

    serial_write_string(b"AS: BEFORE NEW\r\n");

    match AddressSpace::new(&mut frame_allocator) {
        Ok(mut address_space) => {
            serial_write_string(b"AS: AFTER NEW\r\n");

            serial_write_string(b"PML4 CREATED: 0x");

            serial_write_hex64(address_space.pml4_phys());

            serial_write_string(b"\r\n");

            // ====================================================
            // IDENTITY MAP
            // ====================================================

            serial_write_string(b"AS: BEFORE IDENTITY MAP\r\n");

            match address_space.identity_map(&mut frame_allocator, 0x0000_0000, 0x0020_0000, true) {
                Ok(()) => {
                    serial_write_string(b"AS: AFTER IDENTITY MAP\r\n");

                    serial_write_string(b"IDENTITY MAP: PASS\r\n");

                    let pml4 = unsafe { &mut *(address_space.pml4_phys() as *mut PageTable) };

                    // =============================================
                    // TEST MAP
                    // =============================================

                    serial_write_string(b"AS: BEFORE TEST MAP\r\n");

                    match memory::page_table::map(
                        pml4,
                        &mut frame_allocator,
                        0x0040_0000,
                        0x0010_0000,
                        true,
                        false,
                    ) {
                        Ok(()) => {
                            serial_write_string(b"AS: AFTER TEST MAP\r\n");

                            serial_write_string(b"TEST MAP: PASS\r\n");
                        }

                        Err(error) => {
                            serial_write_string(b"TEST MAP: FAIL: ");

                            serial_write_string(error.as_bytes());

                            serial_write_string(b"\r\n");

                            halt();
                        }
                    }

                    // =============================================
                    // TRANSLATION TEST
                    // =============================================

                    serial_write_string(b"AS: BEFORE TRANSLATION\r\n");

                    serial_write_string(b"TRANSLATION TEST: ");

                    match memory::page_table::translate(pml4, 0x0040_0000) {
                        Some(physical_address) if physical_address == 0x0010_0000 => {
                            serial_write_string(b"PASS\r\n");
                        }

                        Some(physical_address) => {
                            serial_write_string(b"FAIL: WRONG PHYSICAL\r\n");

                            serial_write_string(b"ACTUAL: 0x");

                            serial_write_hex64(physical_address);

                            serial_write_string(b"\r\n");

                            halt();
                        }

                        None => {
                            serial_write_string(b"FAIL: UNMAPPED\r\n");

                            halt();
                        }
                    }

                    // =============================================
                    // UNMAP TEST
                    // =============================================

                    serial_write_string(b"AS: BEFORE UNMAP\r\n");

                    serial_write_string(b"UNMAP TEST: ");

                    match memory::page_table::unmap(pml4, 0x0040_0000) {
                        Ok(physical_address) if physical_address == 0x0010_0000 => {
                            serial_write_string(b"PASS\r\n");
                        }

                        Ok(physical_address) => {
                            serial_write_string(b"FAIL: WRONG PHYSICAL\r\n");

                            serial_write_string(b"ACTUAL: 0x");

                            serial_write_hex64(physical_address);

                            serial_write_string(b"\r\n");

                            halt();
                        }

                        Err(error) => {
                            serial_write_string(b"FAIL: ");

                            serial_write_string(error.as_bytes());

                            serial_write_string(b"\r\n");

                            halt();
                        }
                    }

                    // =============================================
                    // POST-UNMAP TRANSLATION TEST
                    // =============================================

                    serial_write_string(b"AS: BEFORE POST-UNMAP TRANSLATION\r\n");

                    serial_write_string(b"POST-UNMAP TRANSLATION TEST: ");

                    match memory::page_table::translate(pml4, 0x0040_0000) {
                        None => {
                            serial_write_string(b"PASS: UNMAPPED\r\n");
                        }

                        Some(physical_address) => {
                            serial_write_string(b"FAIL: STILL MAPPED\r\n");

                            serial_write_string(b"PHYSICAL: 0x");

                            serial_write_hex64(physical_address);

                            serial_write_string(b"\r\n");

                            halt();
                        }
                    }

                    // =============================================
                    // USER ADDRESS SPACE TEST
                    // =============================================

                    serial_write_string(b"\r\nUSER ADDRESS SPACE TEST\r\n");

                    // ============================================================
                    // TEMPORARY USER CODE MAPPING
                    // ============================================================

                    serial_write_string(b"USER: BEFORE CODE MAP\r\n");

                    let user_code_physical =
                        arch::x86_64::user::test_entry_physical_address();

                    let user_code_page =
                        user_code_physical & !(memory::page_table::PAGE_SIZE - 1);

                    let user_pml4 =
                        unsafe { &mut *(address_space.pml4_phys() as *mut PageTable) };

                    serial_write_string(b"USER CODE PHYSICAL: 0x");
                    serial_write_hex64(user_code_physical);
                    serial_write_string(b"\r\n");

                    serial_write_string(b"USER CODE PAGE: 0x");
                    serial_write_hex64(user_code_page);
                    serial_write_string(b"\r\n");

                    match memory::page_table::map(
                        user_pml4,
                        &mut frame_allocator,
                        arch::x86_64::user::USER_CODE_VADDR,
                        user_code_page,
                        true,
                        true,
                    ) {
                        Ok(()) => serial_write_string(b"USER CODE MAP: PASS\r\n"),
                        Err(error) => {
                            serial_write_string(b"USER CODE MAP: FAIL: ");
                            serial_write_string(error.as_bytes());
                            serial_write_string(b"\r\n");
                            halt();
                        }
                    }

                    // ============================================================
                    // VERIFY USER CODE TRANSLATION
                    // ============================================================

                    serial_write_string(b"USER: VERIFY CODE TRANSLATION\r\n");

                    match memory::page_table::translate(
                        user_pml4,
                        arch::x86_64::user::USER_CODE_VADDR,
                    ) {
                        Some(physical_address) => {
                            serial_write_string(b"USER CODE TRANSLATION: 0x");
                            serial_write_hex64(physical_address);
                            serial_write_string(b"\r\n");

                            if physical_address == user_code_page {
                                serial_write_string(b"USER CODE TRANSLATION: PASS\r\n");
                            } else {
                                serial_write_string(b"USER CODE TRANSLATION: FAIL\r\n");
                                halt();
                            }
                        }
                        None => {
                            serial_write_string(b"USER CODE TRANSLATION: FAIL: UNMAPPED\r\n");
                            halt();
                        }
                    }

                    // ============================================================
                    // TEMPORARY USER STACK MAPPING
                    // ============================================================

                    serial_write_string(b"USER: BEFORE STACK MAP\r\n");

                    let user_stack_physical =
                        arch::x86_64::user::stack_physical_start();

                    let user_stack_pages =
                        arch::x86_64::user::USER_STACK_SIZE as u64
                            / memory::page_table::PAGE_SIZE;

                    serial_write_string(b"USER STACK PHYSICAL: 0x");
                    serial_write_hex64(user_stack_physical);
                    serial_write_string(b"\r\n");

                    serial_write_string(b"USER STACK PAGES: ");
                    serial_write_hex64(user_stack_pages);
                    serial_write_string(b"\r\n");

                    for page in 0..user_stack_pages {
                        let physical_address =
                            user_stack_physical + page * memory::page_table::PAGE_SIZE;
                        let virtual_address =
                            arch::x86_64::user::USER_STACK_VADDR
                                + page * memory::page_table::PAGE_SIZE;

                        match memory::page_table::map(
                            user_pml4,
                            &mut frame_allocator,
                            virtual_address,
                            physical_address,
                            true,
                            true,
                        ) {
                            Ok(()) => {}
                            Err(error) => {
                                serial_write_string(b"USER STACK MAP: FAIL: ");
                                serial_write_string(error.as_bytes());
                                serial_write_string(b"\r\n");
                                halt();
                            }
                        }
                    }

                    serial_write_string(b"USER STACK MAP: PASS\r\n");

                    // ============================================================
                    // VERIFY USER STACK TRANSLATION
                    // ============================================================

                    serial_write_string(b"USER: VERIFY STACK TRANSLATION\r\n");

                    match memory::page_table::translate(
                        user_pml4,
                        arch::x86_64::user::USER_STACK_VADDR,
                    ) {
                        Some(physical_address) => {
                            serial_write_string(b"USER STACK TRANSLATION: 0x");
                            serial_write_hex64(physical_address);
                            serial_write_string(b"\r\n");

                            if physical_address == user_stack_physical {
                                serial_write_string(b"USER STACK TRANSLATION: PASS\r\n");
                            } else {
                                serial_write_string(b"USER STACK TRANSLATION: FAIL\r\n");
                                halt();
                            }
                        }
                        None => {
                            serial_write_string(b"USER STACK TRANSLATION: FAIL: UNMAPPED\r\n");
                            halt();
                        }
                    }

                    // ============================================================
                    // RING 3 KERNEL STACK / TSS.RSP0
                    // ============================================================

                    serial_write_string(b"USER: INITIALIZING RING3 KERNEL STACK\r\n");

                    // Initialize the temporary kernel stack that is also
                    // used by the SYSCALL entry path.
                    arch::x86_64::syscall::initialize_stack();

                    let syscall_stack_top =
                        unsafe { arch::x86_64::syscall::unbound_syscall_stack_top };

                    serial_write_string(b"SYSCALL STACK TOP: 0x");

                    serial_write_hex64(syscall_stack_top);

                    serial_write_string(b"\r\n");

                    if syscall_stack_top == 0 {
                        serial_write_string(b"SYSCALL STACK: FAIL\r\n");

                        halt();
                    }

                    serial_write_string(b"SYSCALL STACK: PASS\r\n");

                    // Use the initialized syscall stack as the temporary
                    // Ring 0 stack for CPL3 -> CPL0 exceptions.
                    arch::x86_64::tss::set_rsp0(syscall_stack_top);

                    let rsp0 = arch::x86_64::tss::rsp0();

                    serial_write_string(b"TSS RSP0: 0x");

                    serial_write_hex64(rsp0);

                    serial_write_string(b"\r\n");

                    if rsp0 != 0 {
                        serial_write_string(b"TSS RSP0: PASS\r\n");
                    } else {
                        serial_write_string(b"TSS RSP0: FAIL\r\n");

                        halt();
                    }

                    // =============================================
                    // CR3 SWITCH
                    // =============================================

                    serial_write_string(b"\r\nCR3 SWITCH: START\r\n");

                    serial_write_string(b"PHASE 5: BEFORE CR3 ACTIVATE\r\n");

                    address_space.activate();

                    serial_write_string(b"PHASE 5: AFTER CR3 ACTIVATE\r\n");

                    serial_write_string(b"CR3 SWITCH: PASS\r\n");

                    // =============================================
                    // KERNEL HEAP
                    // =============================================

                    serial_write_string(b"\r\nKERNEL HEAP INITIALIZATION\r\n");

                    serial_write_string(b"PHASE 5: BEFORE HEAP OBJECT\r\n");

                    let mut kernel_heap = KernelHeap::new();

                    serial_write_string(b"PHASE 5: AFTER HEAP OBJECT\r\n");

                    serial_write_string(b"HEAP START: 0x");

                    serial_write_hex64(kernel_heap.start());

                    serial_write_string(b"\r\n");

                    serial_write_string(b"HEAP END: 0x");

                    serial_write_hex64(kernel_heap.end());

                    serial_write_string(b"\r\n");

                    serial_write_string(b"HEAP CAPACITY: ");

                    serial_write_hex64(kernel_heap.page_capacity());

                    serial_write_string(b" PAGES\r\n");

                    // =============================================
                    // SAME PML4 AS CR3
                    // =============================================

                    serial_write_string(b"PHASE 5: BEFORE HEAP PML4\r\n");

                    let heap_pml4 = unsafe { &mut *(address_space.pml4_phys() as *mut PageTable) };

                    serial_write_string(b"PHASE 5: AFTER HEAP PML4\r\n");

                    // =============================================
                    // HEAP MAPPING
                    // =============================================

                    serial_write_string(b"HEAP MAPPING: 4 PAGES\r\n");

                    serial_write_string(b"PHASE 5: BEFORE HEAP INITIALIZE\r\n");

                    match kernel_heap.initialize(heap_pml4, &mut frame_allocator, 4) {
                        Ok(()) => {
                            serial_write_string(b"PHASE 5: AFTER HEAP INITIALIZE\r\n");

                            serial_write_string(b"HEAP MAP: PASS\r\n");

                            serial_write_string(b"HEAP MAPPED PAGES: ");

                            serial_write_hex64(kernel_heap.mapped_pages());

                            serial_write_string(b"\r\n");

                            serial_write_string(b"HEAP INITIALIZED: ");

                            if kernel_heap.is_initialized() {
                                serial_write_string(b"YES\r\n");
                            } else {
                                serial_write_string(b"NO\r\n");
                            }
                        }

                        Err(error) => {
                            serial_write_string(b"HEAP MAP: FAIL: ");

                            serial_write_string(error.as_bytes());

                            serial_write_string(b"\r\n");

                            halt();
                        }
                    }

                    // =============================================
                    // SPINLOCK TEST
                    // =============================================

                    serial_write_string(b"\r\nSPINLOCK TEST\r\n");

                    let spinlock = sync::SpinLock::new(0u64);

                    serial_write_string(b"SPINLOCK INITIAL STATE: ");

                    if !spinlock.is_locked() {
                        serial_write_string(b"PASS\r\n");
                    } else {
                        serial_write_string(b"FAIL\r\n");

                        halt();
                    }

                    serial_write_string(b"SPINLOCK ACQUIRE: ");

                    {
                        let mut guard = spinlock.lock();

                        if spinlock.is_locked() {
                            serial_write_string(b"PASS\r\n");
                        } else {
                            serial_write_string(b"FAIL\r\n");

                            halt();
                        }

                        *guard = 0x1234_5678;
                    }

                    serial_write_string(b"SPINLOCK RELEASE: ");

                    if !spinlock.is_locked() {
                        serial_write_string(b"PASS\r\n");
                    } else {
                        serial_write_string(b"FAIL\r\n");

                        halt();
                    }

                    serial_write_string(b"SPINLOCK DATA: ");

                    {
                        let guard = spinlock.lock();

                        if *guard == 0x1234_5678 {
                            serial_write_string(b"PASS\r\n");
                        } else {
                            serial_write_string(b"FAIL\r\n");

                            halt();
                        }
                    }

                    serial_write_string(b"SPINLOCK: PASS\r\n");

                    // =============================================
                    // KERNEL OBJECT TEST
                    // =============================================

                    run_kernel_object_test();

                    // =============================================
                    // IPC TEST
                    // =============================================

                    run_ipc_test();

                    // =============================================
                    // SYSCALL DISPATCHER TEST
                    // =============================================

                    run_syscall_test();

                    // =============================================
                    // MSR / EFER.SCE TEST
                    // =============================================

                    run_msr_test();

                    // =============================================
                    // STAR / LSTAR / FMASK TEST
                    // =============================================

                    run_syscall_msr_test();

                    // =============================================
                    // PHASE 5 CHECKPOINT
                    // =============================================

                    serial_write_string(b"\r\n================================\r\n");

                    serial_write_string(b"PHASE 5 KERNEL INFRASTRUCTURE\r\n");

                    serial_write_string(b"================================\r\n");

                    serial_write_string(b"PREEMPTIVE SCHEDULER: PARKED\r\n");

                    serial_write_string(b"KERNEL HEAP: PASS\r\n");

                    serial_write_string(b"SPINLOCK: PASS\r\n");

                    serial_write_string(b"KERNEL OBJECTS: PASS\r\n");

                    serial_write_string(b"IPC: PASS\r\n");

                    serial_write_string(b"SYSCALL: PASS\r\n");

                    serial_write_string(b"MSR: PASS\r\n");

                    serial_write_string(b"STAR: PASS\r\n");

                    serial_write_string(b"LSTAR: PASS\r\n");

                    serial_write_string(b"FMASK: PASS\r\n");

                    serial_write_string(b"UNBOUND KERNEL: SYSCALL ABI FOUNDATION READY\r\n");
                    // =============================================
                    // PHASE 6 — RING 3 SYSCALL TEST
                    // =============================================

                    serial_write_string(b"\r\n================================\r\n");
                    serial_write_string(b"PHASE 6 RING 3 SYSCALL TEST\r\n");
                    serial_write_string(b"================================\r\n");

                    let user_entry = arch::x86_64::user::USER_CODE_VADDR;

                    let user_stack_top = arch::x86_64::user::USER_STACK_VADDR
                        + arch::x86_64::user::USER_STACK_SIZE as u64;

                    serial_write_string(b"USER ENTRY: 0x");
                    serial_write_hex64(user_entry);
                    serial_write_string(b"\r\n");

                    serial_write_string(b"USER STACK TOP: 0x");
                    serial_write_hex64(user_stack_top);
                    serial_write_string(b"\r\n");

                    serial_write_string(b"\r\n--- RING 3 IRET CHECK ---\r\n");

serial_write_string(b"USER RIP: 0x");
serial_write_hex64(user_entry);
serial_write_string(b"\r\n");

serial_write_string(b"USER CS: 0x");
serial_write_hex64(
    arch::x86_64::gdt::USER_CODE_SELECTOR_R3 as u64
);
serial_write_string(b"\r\n");

serial_write_string(b"USER RSP: 0x");
serial_write_hex64(user_stack_top);
serial_write_string(b"\r\n");

serial_write_string(b"USER SS: 0x");
serial_write_hex64(
    arch::x86_64::gdt::USER_DATA_SELECTOR_R3 as u64
);
serial_write_string(b"\r\n");

serial_write_string(b"USER RFLAGS: 0x202\r\n");

serial_write_string(b"--- END IRET CHECK ---\r\n");

                    serial_write_string(b"ENTERING RING 3...\r\n");

                    unsafe {
                        arch::x86_64::user::enter_ring3(user_entry, user_stack_top);
                    }
                }

                Err(error) => {
                    serial_write_string(b"IDENTITY MAP: FAIL: ");

                    serial_write_string(error.as_bytes());

                    serial_write_string(b"\r\n");

                    halt();
                }
            }
        }

        Err(error) => {
            serial_write_string(b"PML4 CREATION: FAIL: ");

            serial_write_string(error.as_bytes());

            serial_write_string(b"\r\n");

            halt();
        }
    }
}

// ================================================================
// KERNEL OBJECT TEST
// ================================================================

fn run_kernel_object_test() {
    serial_write_string(b"\r\nKERNEL OBJECT TEST\r\n");

    serial_write_string(b"OBJECT TABLE INITIAL: ");

    if KERNEL_OBJECTS.count() == 0 {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    let process = match KERNEL_OBJECTS.create(objects::ObjectType::Process) {
        Some(handle) => handle,

        None => {
            serial_write_string(b"OBJECT CREATE: FAIL\r\n");

            halt();
        }
    };

    serial_write_string(b"OBJECT CREATE: PASS\r\n");

    serial_write_string(b"OBJECT HANDLE: ");

    if KERNEL_OBJECTS.is_valid(process) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"OBJECT TYPE: ");

    if KERNEL_OBJECTS.get_type(process) == Some(objects::ObjectType::Process) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"OBJECT RETAIN: ");

    if KERNEL_OBJECTS.retain(process) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"OBJECT RELEASE: ");

    if KERNEL_OBJECTS.release(process) && KERNEL_OBJECTS.is_valid(process) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"OBJECT FINAL RELEASE: ");

    if KERNEL_OBJECTS.release(process) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"OBJECT DESTROY: ");

    if !KERNEL_OBJECTS.is_valid(process) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    let thread = match KERNEL_OBJECTS.create(objects::ObjectType::Thread) {
        Some(handle) => handle,

        None => {
            serial_write_string(b"OBJECT REUSE: FAIL\r\n");

            halt();
        }
    };

    serial_write_string(b"OBJECT REUSE: ");

    if thread.index() == process.index() && thread.generation() != process.generation() {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"STALE HANDLE: ");

    if !KERNEL_OBJECTS.is_valid(process) && KERNEL_OBJECTS.is_valid(thread) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"OBJECT TYPE ISOLATION: ");

    if KERNEL_OBJECTS.get_type(thread) == Some(objects::ObjectType::Thread)
        && KERNEL_OBJECTS.get_type(process) != Some(objects::ObjectType::Thread)
    {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"OBJECT CLEANUP: ");

    if KERNEL_OBJECTS.release(thread) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"OBJECT TABLE FINAL: ");

    if KERNEL_OBJECTS.count() == 0 {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");
        halt();
    }

    serial_write_string(b"KERNEL OBJECTS: PASS\r\n");
}

// ================================================================
// IPC TEST
// ================================================================

fn run_ipc_test() {
    serial_write_string(b"\r\nIPC TEST\r\n");

    serial_write_string(b"IPC INITIAL: ");

    if IPC_MANAGER.count() == 0 {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        halt();
    }

    let endpoint = match IPC_MANAGER.create() {
        Some(handle) => {
            serial_write_string(b"IPC ENDPOINT CREATE: PASS\r\n");

            handle
        }

        None => {
            serial_write_string(b"IPC ENDPOINT CREATE: FAIL\r\n");

            halt();
        }
    };

    serial_write_string(b"IPC HANDLE: ");

    if IPC_MANAGER.is_valid(endpoint) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        halt();
    }

    let sender = match KERNEL_OBJECTS.create(objects::ObjectType::Process) {
        Some(handle) => handle,

        None => {
            serial_write_string(b"IPC SENDER CREATE: FAIL\r\n");

            halt();
        }
    };

    let payload = b"Hello from Unbound IPC";

    serial_write_string(b"IPC SEND: ");

    if IPC_MANAGER.send(endpoint, sender, payload) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        KERNEL_OBJECTS.release(sender);

        halt();
    }

    serial_write_string(b"IPC QUEUE: ");

    if IPC_MANAGER.queue_len(endpoint) == Some(1) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        KERNEL_OBJECTS.release(sender);

        halt();
    }

    let message = match IPC_MANAGER.receive(endpoint) {
        Some(message) => {
            serial_write_string(b"IPC RECEIVE: PASS\r\n");

            message
        }

        None => {
            serial_write_string(b"IPC RECEIVE: FAIL\r\n");

            KERNEL_OBJECTS.release(sender);

            halt();
        }
    };

    serial_write_string(b"IPC SENDER: ");

    if message.sender == sender {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        KERNEL_OBJECTS.release(sender);

        halt();
    }

    serial_write_string(b"IPC MESSAGE: ");

    if message.payload() == payload {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        KERNEL_OBJECTS.release(sender);

        halt();
    }

    serial_write_string(b"IPC EMPTY: ");

    if IPC_MANAGER.queue_len(endpoint) == Some(0) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        KERNEL_OBJECTS.release(sender);

        halt();
    }

    serial_write_string(b"IPC DESTROY: ");

    if IPC_MANAGER.destroy(endpoint) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        KERNEL_OBJECTS.release(sender);

        halt();
    }

    serial_write_string(b"IPC STALE HANDLE: ");

    if !IPC_MANAGER.is_valid(endpoint) {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        KERNEL_OBJECTS.release(sender);

        halt();
    }

    KERNEL_OBJECTS.release(sender);

    serial_write_string(b"IPC CLEANUP: ");

    if IPC_MANAGER.count() == 0 && KERNEL_OBJECTS.count() == 0 {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        halt();
    }

    serial_write_string(b"IPC: PASS\r\n");
}

// ================================================================
// SYSCALL DISPATCHER TEST
// ================================================================

fn run_syscall_test() {
    serial_write_string(b"\r\nSYSCALL TEST\r\n");

    let version = syscall::dispatch(syscall::SYS_GET_VERSION, 0, 0, 0);

    serial_write_string(b"SYSCALL GET_VERSION: ");

    if version == syscall::UNBOUND_SYSCALL_ABI_VERSION {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        serial_write_string(b"EXPECTED: ");

        serial_write_hex64(syscall::UNBOUND_SYSCALL_ABI_VERSION);

        serial_write_string(b"\r\nACTUAL: ");

        serial_write_hex64(version);

        serial_write_string(b"\r\n");

        halt();
    }

    let unknown = syscall::dispatch(0xFFFF, 0, 0, 0);

    serial_write_string(b"SYSCALL UNKNOWN: ");

    if unknown == u64::MAX {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        serial_write_string(b"RETURNED: ");

        serial_write_hex64(unknown);

        serial_write_string(b"\r\n");

        halt();
    }

    serial_write_string(b"SYSCALL: PASS\r\n");
}

// ================================================================
// MSR / SYSCALL EXTENSION TEST
// ================================================================

fn run_msr_test() {
    serial_write_string(b"\r\nMSR TEST\r\n");

    serial_write_string(b"MSR EFER READ: ");

    let before = unsafe { arch::x86_64::msr::read_efer() };

    serial_write_string(b"PASS\r\n");

    serial_write_string(b"EFER BEFORE: 0x");

    serial_write_hex64(before);

    serial_write_string(b"\r\n");

    serial_write_string(b"MSR EFER.SCE ENABLE: ");

    unsafe {
        arch::x86_64::msr::enable_syscall_extensions();
    }

    serial_write_string(b"PASS\r\n");

    serial_write_string(b"MSR EFER.SCE VERIFY: ");

    let enabled = unsafe { arch::x86_64::msr::syscall_extensions_enabled() };

    if enabled {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        halt();
    }

    let after = unsafe { arch::x86_64::msr::read_efer() };

    serial_write_string(b"EFER AFTER: 0x");

    serial_write_hex64(after);

    serial_write_string(b"\r\n");

    serial_write_string(b"MSR SCE: PASS\r\n");
}

// ================================================================
// STAR / LSTAR / FMASK TEST
// ================================================================

fn run_syscall_msr_test() {
    serial_write_string(b"\r\nSYSCALL MSR TEST\r\n");

    // ============================================================
    // SYSCALL ENTRY ADDRESS
    // ============================================================

    let entry = arch::x86_64::syscall::entry_address();

    serial_write_string(b"SYSCALL ENTRY ADDRESS: 0x");

    serial_write_hex64(entry);

    serial_write_string(b"\r\n");

    if entry == 0 {
        serial_write_string(b"SYSCALL ENTRY ADDRESS: FAIL\r\n");

        halt();
    }

    serial_write_string(b"SYSCALL ENTRY ADDRESS: PASS\r\n");

    // ============================================================
    // INITIALIZE STAR / LSTAR / CSTAR / FMASK
    // ============================================================

    serial_write_string(b"SYSCALL MSR INITIALIZATION: ");

    // The syscall stack was already initialized before TSS.RSP0
    // was configured. Do not initialize it again here.

    unsafe {
        arch::x86_64::msr::init_syscall_msrs(entry);
    }

    serial_write_string(b"PASS\r\n");

    // ============================================================
    // STAR
    // ============================================================

    let star = unsafe { arch::x86_64::msr::read_star() };

    serial_write_string(b"STAR: 0x");

    serial_write_hex64(star);

    serial_write_string(b"\r\n");

    let expected_star = arch::x86_64::msr::make_star();

    serial_write_string(b"STAR VERIFY: ");

    if star == expected_star {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        serial_write_string(b"EXPECTED STAR: 0x");

        serial_write_hex64(expected_star);

        serial_write_string(b"\r\n");

        halt();
    }

    // ============================================================
    // LSTAR
    // ============================================================

    let lstar = unsafe { arch::x86_64::msr::read_lstar() };

    serial_write_string(b"LSTAR: 0x");

    serial_write_hex64(lstar);

    serial_write_string(b"\r\n");

    serial_write_string(b"LSTAR VERIFY: ");

    if lstar == entry {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        serial_write_string(b"EXPECTED LSTAR: 0x");

        serial_write_hex64(entry);

        serial_write_string(b"\r\n");

        halt();
    }

    // ============================================================
    // CSTAR
    // ============================================================

    let cstar = unsafe { arch::x86_64::msr::read_cstar() };

    serial_write_string(b"CSTAR: 0x");

    serial_write_hex64(cstar);

    serial_write_string(b"\r\n");

    serial_write_string(b"CSTAR VERIFY: ");

    if cstar == 0 {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        serial_write_string(b"EXPECTED CSTAR: 0x0000000000000000\r\n");

        halt();
    }

    // ============================================================
    // FMASK
    // ============================================================

    let fmask = unsafe { arch::x86_64::msr::read_fmask() };

    serial_write_string(b"FMASK: 0x");

    serial_write_hex64(fmask);

    serial_write_string(b"\r\n");

    serial_write_string(b"FMASK VERIFY: ");

    if fmask == arch::x86_64::msr::SYSCALL_FMASK {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        serial_write_string(b"EXPECTED FMASK: 0x");

        serial_write_hex64(arch::x86_64::msr::SYSCALL_FMASK);

        serial_write_string(b"\r\n");

        halt();
    }

    // ============================================================
    // FINAL MSR VERIFICATION
    // ============================================================

    serial_write_string(b"SYSCALL MSR FINAL VERIFY: ");

    let verified = unsafe { arch::x86_64::msr::verify_syscall_msrs(entry) };

    if verified {
        serial_write_string(b"PASS\r\n");
    } else {
        serial_write_string(b"FAIL\r\n");

        halt();
    }

    // ============================================================
    // SYSCALL SELECTORS
    // ============================================================

    serial_write_string(b"SYSCALL SELECTORS\r\n");

    serial_write_string(b"KERNEL CS: 0x");

    serial_write_hex32(arch::x86_64::gdt::KERNEL_CODE_SELECTOR as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"KERNEL SS: 0x");

    serial_write_hex32(arch::x86_64::gdt::KERNEL_DATA_SELECTOR as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"USER CODE BASE: 0x");

    serial_write_hex32(arch::x86_64::gdt::USER_CODE_SELECTOR as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"USER DATA BASE: 0x");

    serial_write_hex32(arch::x86_64::gdt::USER_DATA_SELECTOR as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"USER CODE R3: 0x");

    serial_write_hex32(arch::x86_64::gdt::USER_CODE_SELECTOR_R3 as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"USER DATA R3: 0x");

    serial_write_hex32(arch::x86_64::gdt::USER_DATA_SELECTOR_R3 as u32);

    serial_write_string(b"\r\n");

    serial_write_string(b"SYSCALL MSRS: PASS\r\n");
}

// ================================================================
// TIMER INTERRUPT HANDLER
// ================================================================

#[unsafe(no_mangle)]
pub extern "C" fn unbound_timer_handler(interrupt_rsp: u64) -> u64 {
    let count = TIMER_IRQ_DEBUG_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

    if count == 1 {
        serial_write_string(b"\r\n*** IRQ0 FIRED ***\r\n");
    }

    let next_rsp = interrupt_rsp;

    if count <= 4 {
        serial_write_string(b"\r\n*** TIMER DEBUG ***\r\n");

        serial_write_string(b"CURRENT RSP: 0x");

        serial_write_hex64(interrupt_rsp);

        serial_write_string(b"\r\n");

        serial_write_string(b"NEXT RSP: 0x");

        serial_write_hex64(next_rsp);

        serial_write_string(b"\r\n");

        unsafe {
            let p = next_rsp as *const u64;

            serial_write_string(b"NEXT +00: 0x");

            serial_write_hex64(p.read());

            serial_write_string(b"\r\n");

            serial_write_string(b"NEXT +08: 0x");

            serial_write_hex64(p.add(1).read());

            serial_write_string(b"\r\n");

            serial_write_string(b"NEXT +16: 0x");

            serial_write_hex64(p.add(2).read());

            serial_write_string(b"\r\n");

            serial_write_string(b"NEXT +24: 0x");

            serial_write_hex64(p.add(3).read());

            serial_write_string(b"\r\n");

            serial_write_string(b"NEXT +32: 0x");

            serial_write_hex64(p.add(4).read());

            serial_write_string(b"\r\n");

            serial_write_string(b"NEXT +40: 0x");

            serial_write_hex64(p.add(5).read());

            serial_write_string(b"\r\n");
        }

        serial_write_string(b"******************************\r\n");
    }

    arch::x86_64::pic::end_of_interrupt(0);

    next_rsp
}

// ================================================================
// RUST EXCEPTION HANDLER
// ================================================================

#[unsafe(no_mangle)]
pub extern "C" fn unbound_exception_handler(frame: &arch::x86_64::idt::InterruptFrame) {
    let count = EXCEPTION_DEBUG_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

    serial_write_string(b"\r\n");

    serial_write_string(b"================================\r\n");

    serial_write_string(b"UNBOUND EXCEPTION HANDLER\r\n");

    serial_write_string(b"================================\r\n");

    serial_write_string(b"EXCEPTION COUNT: ");

    serial_write_hex64(count);

    serial_write_string(b"\r\n");

    serial_write_string(b"FRAME ADDRESS: 0x");

    serial_write_hex64(frame as *const _ as u64);

    serial_write_string(b"\r\n");

    serial_write_string(b"IST1 STACK TOP: 0x");

    serial_write_hex64(arch::x86_64::tss::ist1());

    serial_write_string(b"\r\n");

    serial_write_string(b"VECTOR: 0x");

    serial_write_hex64(frame.vector);

    serial_write_string(b"\r\n");

    serial_write_string(b"ERROR CODE: 0x");

    serial_write_hex64(frame.error_code);

    serial_write_string(b"\r\n");

    serial_write_string(b"RIP: 0x");

    serial_write_hex64(frame.rip);

    serial_write_string(b"\r\n");

    serial_write_string(b"CS: 0x");

    serial_write_hex64(frame.cs);

    serial_write_string(b"\r\n");

    serial_write_string(b"RFLAGS: 0x");

    serial_write_hex64(frame.rflags);

    serial_write_string(b"\r\n");

    serial_write_string(b"EXCEPTION HANDLER COMPLETE\r\n");

    serial_write_string(b"EXCEPTION HANDLER RETURNING\r\n");
}

// ================================================================
// CPU HALT
// ================================================================

fn halt() -> ! {
    unsafe {
        asm!("cli", "2:", "hlt", "jmp 2b", options(noreturn));
    }
}

// ================================================================
// SERIAL OUTPUT
// ================================================================

fn serial_write_char(c: u8) {
    unsafe {
        loop {
            let status: u8;

            asm!(
                "in al, dx",
                in("dx") 0x3FDu16,
                out("al") status,
            );

            if status & 0x20 != 0 {
                break;
            }
        }

        asm!(
            "out dx, al",
            in("dx") 0x3F8u16,
            in("al") c,
        );
    }
}

fn serial_write_string(s: &[u8]) {
    for &c in s {
        serial_write_char(c);
    }
}

// ================================================================
// HEX OUTPUT
// ================================================================

fn serial_write_hex8(value: u8) {
    serial_write_hex_digit((value >> 4) & 0x0F);

    serial_write_hex_digit(value & 0x0F);
}

fn serial_write_hex32(value: u32) {
    let mut shift = 28;

    loop {
        let digit = ((value >> shift) & 0x0F) as u8;

        serial_write_hex_digit(digit);

        if shift == 0 {
            break;
        }

        shift -= 4;
    }
}

fn serial_write_hex64(value: u64) {
    let mut shift = 60;

    loop {
        let digit = ((value >> shift) & 0x0F) as u8;

        serial_write_hex_digit(digit);

        if shift == 0 {
            break;
        }

        shift -= 4;
    }
}

fn serial_write_hex_digit(value: u8) {
    let c = if value < 10 {
        b'0' + value
    } else {
        b'A' + (value - 10)
    };

    serial_write_char(c);
}

// ================================================================
// PANIC HANDLER
// ================================================================

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_write_string(b"KERNEL PANIC\r\n");

    halt();
}
