use core::arch::asm;

use crate::serial_write_string;

// ================================================================
// CONSTANTS
// ================================================================

const IDT_ENTRIES: usize = 256;
const KERNEL_CODE_SELECTOR: u16 = 0x08;
const INTERRUPT_GATE: u8 = 0x8E;

// ================================================================
// DESCRIPTOR TABLE POINTER
// ================================================================

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

// ================================================================
// IDT ENTRY
// ================================================================

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            attributes: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    fn set_handler(&mut self, handler: u64) {
        self.set_handler_with_ist(handler, 0);
    }

    fn set_handler_with_ist(&mut self, handler: u64, ist: u8) {
        self.offset_low = handler as u16;
        self.selector = KERNEL_CODE_SELECTOR;
        self.ist = ist & 0x07;
        self.attributes = INTERRUPT_GATE;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.reserved = 0;
    }
}

// ================================================================
// IDT
// ================================================================

#[repr(C, align(16))]
struct InterruptDescriptorTable {
    entries: [IdtEntry; IDT_ENTRIES],
}

impl InterruptDescriptorTable {
    const fn new() -> Self {
        Self {
            entries: [IdtEntry::missing(); IDT_ENTRIES],
        }
    }

    fn set_handler(&mut self, vector: usize, handler: u64) {
        self.entries[vector].set_handler(handler);
    }

    fn set_handler_with_ist(&mut self, vector: usize, handler: u64, ist: u8) {
        self.entries[vector].set_handler_with_ist(handler, ist);
    }
}

// ================================================================
// STATIC IDT
// ================================================================

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

// ================================================================
// CPU EXCEPTION FRAME
// ================================================================

#[repr(C)]
pub struct InterruptFrame {
    pub vector: u64,
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
}

// ================================================================
// HARDWARE IRQ FRAME
// ================================================================

#[repr(C)]
pub struct IrqFrame {
    pub vector: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
}

// ================================================================
// ISR DECLARATIONS
// ================================================================

unsafe extern "C" {
    fn unbound_isr0();
    fn unbound_isr1();
    fn unbound_isr2();
    fn unbound_isr3();
    fn unbound_isr4();
    fn unbound_isr5();
    fn unbound_isr6();
    fn unbound_isr7();
    fn unbound_isr8();
    fn unbound_isr9();
    fn unbound_isr10();
    fn unbound_isr11();
    fn unbound_isr12();
    fn unbound_isr13();
    fn unbound_isr14();
    fn unbound_isr15();
    fn unbound_isr16();
    fn unbound_isr17();
    fn unbound_isr18();
    fn unbound_isr19();
    fn unbound_isr20();
    fn unbound_isr21();
    fn unbound_isr22();
    fn unbound_isr23();
    fn unbound_isr24();
    fn unbound_isr25();
    fn unbound_isr26();
    fn unbound_isr27();
    fn unbound_isr28();
    fn unbound_isr29();
    fn unbound_isr30();
    fn unbound_isr31();

    fn unbound_irq0();
}

// ================================================================
// DEBUG COUNTER
// ================================================================

static IRET_DEBUG_COUNT: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

// ================================================================
// SERIAL HEX64 HELPER
// ================================================================

fn serial_write_hex64(value: u64) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    crate::serial_write_string(b"0x");

    let mut shift = 60;

    loop {
        let digit = ((value >> shift) & 0xF) as usize;

        unsafe {
            asm!(
                "out dx, al",
                in("dx") 0x3F8u16,
                in("al") HEX[digit],
                options(nostack, preserves_flags),
            );
        }

        if shift == 0 {
            break;
        }

        shift -= 4;
    }
}

// ================================================================
// IRET FRAME DIAGNOSTIC
// ================================================================

#[unsafe(no_mangle)]
pub extern "C" fn unbound_debug_iret_frames(current_rsp: u64, next_rsp: u64) {
    use core::sync::atomic::Ordering;

    let count = IRET_DEBUG_COUNT.fetch_add(1, Ordering::Relaxed);

    if count >= 16 {
        return;
    }

    unsafe {
        crate::serial_write_string(b"\r\n================================\r\n");

        crate::serial_write_string(b"        IRET FRAME DEBUG\r\n");

        crate::serial_write_string(b"================================\r\n");

        // --------------------------------------------------------
        // CURRENT TASK
        // --------------------------------------------------------

        crate::serial_write_string(b"CURRENT FRAME RSP: ");

        serial_write_hex64(current_rsp);

        crate::serial_write_string(b"\r\n");

        if current_rsp != 0 {
            let current = current_rsp as *const u64;

            let vector = core::ptr::read_volatile(current.add(15));

            let rip = core::ptr::read_volatile(current.add(16));

            let cs = core::ptr::read_volatile(current.add(17));

            let rflags = core::ptr::read_volatile(current.add(18));

            crate::serial_write_string(b"CURRENT VECTOR: ");

            serial_write_hex64(vector);

            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"CURRENT IRET RIP: ");

            serial_write_hex64(rip);

            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"CURRENT IRET CS: ");

            serial_write_hex64(cs);

            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"CURRENT IRET RFLAGS: ");

            serial_write_hex64(rflags);

            crate::serial_write_string(b"\r\n");
        }

        // --------------------------------------------------------
        // NEXT TASK
        // --------------------------------------------------------

        crate::serial_write_string(b"NEXT FRAME RSP: ");

        serial_write_hex64(next_rsp);

        crate::serial_write_string(b"\r\n");

        if next_rsp != 0 {
            let next = next_rsp as *const u64;

            let vector = core::ptr::read_volatile(next.add(15));

            let rip = core::ptr::read_volatile(next.add(16));

            let cs = core::ptr::read_volatile(next.add(17));

            let rflags = core::ptr::read_volatile(next.add(18));

            crate::serial_write_string(b"NEXT VECTOR: ");

            serial_write_hex64(vector);

            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"NEXT IRET RIP: ");

            serial_write_hex64(rip);

            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"NEXT IRET CS: ");

            serial_write_hex64(cs);

            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"NEXT IRET RFLAGS: ");

            serial_write_hex64(rflags);

            crate::serial_write_string(b"\r\n");
        }

        crate::serial_write_string(b"================================\r\n");
    }
}

// ================================================================
// SELECTED IRET FRAME DIAGNOSTIC
// ================================================================

#[unsafe(no_mangle)]
pub extern "C" fn unbound_debug_iret_frame(rsp: u64) {
    unsafe {
        let frame = rsp as *const u64;

        // RSP points at VECTOR immediately before iretq.
        let vector = core::ptr::read_volatile(frame.add(0));
        let rip = core::ptr::read_volatile(frame.add(1));
        let cs = core::ptr::read_volatile(frame.add(2));
        let rflags = core::ptr::read_volatile(frame.add(3));

        serial_write_string(b"VECTOR: ");
        serial_write_hex64(vector);
        serial_write_string(b"\r\n");

        serial_write_string(b"RIP: ");
        serial_write_hex64(rip);
        serial_write_string(b"\r\n");

        serial_write_string(b"CS: ");
        serial_write_hex64(cs);
        serial_write_string(b"\r\n");

        serial_write_string(b"RFLAGS: ");
        serial_write_hex64(rflags);
        serial_write_string(b"\r\n");

        serial_write_string(b"RSP: ");
        serial_write_hex64(rsp);
        serial_write_string(b"\r\n");

        serial_write_string(b"RIP: ");
        serial_write_hex64(rip);
        serial_write_string(b"\r\n");

        serial_write_string(b"CS: ");
        serial_write_hex64(cs);
        serial_write_string(b"\r\n");

        serial_write_string(b"RFLAGS: ");
        serial_write_hex64(rflags);
        serial_write_string(b"\r\n");
    }
}

// ================================================================
// PRE-IRET DIAGNOSTIC
// ================================================================

#[unsafe(no_mangle)]
pub extern "C" fn unbound_debug_pre_iret(iret_rsp: u64) {
    unsafe {
        crate::serial_write_string(b"\r\n+++ PRE-IRET CHECK +++\r\n");

        crate::serial_write_string(b"IRET RSP: ");

        serial_write_hex64(iret_rsp);

        crate::serial_write_string(b"\r\n");

        if iret_rsp == 0 {
            crate::serial_write_string(b"PRE-IRET: NULL RSP\r\n");

            return;
        }

        let frame = iret_rsp as *const u64;

        let rip = core::ptr::read_volatile(frame.add(0));

        let cs = core::ptr::read_volatile(frame.add(1));

        let rflags = core::ptr::read_volatile(frame.add(2));

        crate::serial_write_string(b"IRETQ RIP: ");

        serial_write_hex64(rip);

        crate::serial_write_string(b"\r\n");

        crate::serial_write_string(b"IRETQ CS: ");

        serial_write_hex64(cs);

        crate::serial_write_string(b"\r\n");

        crate::serial_write_string(b"IRETQ RFLAGS: ");

        serial_write_hex64(rflags);

        crate::serial_write_string(b"\r\n");

        crate::serial_write_string(b"PRE-IRET VALIDATION: ");

        if rip == 0 {
            crate::serial_write_string(b"BAD RIP\r\n");
        } else if cs != KERNEL_CODE_SELECTOR as u64 {
            crate::serial_write_string(b"BAD CS\r\n");
        } else if (rflags & 0x2) == 0 {
            crate::serial_write_string(b"BAD RFLAGS\r\n");
        } else {
            crate::serial_write_string(b"PASS\r\n");
        }

        crate::serial_write_string(b"+++++++++++++++++++++++\r\n");
    }
}

// ================================================================
// IDT INITIALIZATION
// ================================================================

pub fn init() {
    unsafe {
        let idt = core::ptr::addr_of_mut!(IDT);

        (*idt).set_handler(0, unbound_isr0 as *const () as u64);

        (*idt).set_handler(1, unbound_isr1 as *const () as u64);

        (*idt).set_handler(2, unbound_isr2 as *const () as u64);

        (*idt).set_handler(3, unbound_isr3 as *const () as u64);

        (*idt).set_handler(4, unbound_isr4 as *const () as u64);

        (*idt).set_handler(5, unbound_isr5 as *const () as u64);

        (*idt).set_handler(6, unbound_isr6 as *const () as u64);

        (*idt).set_handler(7, unbound_isr7 as *const () as u64);

        // --------------------------------------------------------
        // DOUBLE FAULT -> IST1
        // --------------------------------------------------------

        (*idt).set_handler_with_ist(8, unbound_isr8 as *const () as u64, 1);

        (*idt).set_handler(9, unbound_isr9 as *const () as u64);

        (*idt).set_handler(10, unbound_isr10 as *const () as u64);

        (*idt).set_handler(11, unbound_isr11 as *const () as u64);

        (*idt).set_handler(12, unbound_isr12 as *const () as u64);

        (*idt).set_handler(13, unbound_isr13 as *const () as u64);

        (*idt).set_handler(14, unbound_isr14 as *const () as u64);

        (*idt).set_handler(15, unbound_isr15 as *const () as u64);

        (*idt).set_handler(16, unbound_isr16 as *const () as u64);

        (*idt).set_handler(17, unbound_isr17 as *const () as u64);

        (*idt).set_handler(18, unbound_isr18 as *const () as u64);

        (*idt).set_handler(19, unbound_isr19 as *const () as u64);

        (*idt).set_handler(20, unbound_isr20 as *const () as u64);

        (*idt).set_handler(21, unbound_isr21 as *const () as u64);

        (*idt).set_handler(22, unbound_isr22 as *const () as u64);

        (*idt).set_handler(23, unbound_isr23 as *const () as u64);

        (*idt).set_handler(24, unbound_isr24 as *const () as u64);

        (*idt).set_handler(25, unbound_isr25 as *const () as u64);

        (*idt).set_handler(26, unbound_isr26 as *const () as u64);

        (*idt).set_handler(27, unbound_isr27 as *const () as u64);

        (*idt).set_handler(28, unbound_isr28 as *const () as u64);

        (*idt).set_handler(29, unbound_isr29 as *const () as u64);

        (*idt).set_handler(30, unbound_isr30 as *const () as u64);

        (*idt).set_handler(31, unbound_isr31 as *const () as u64);

        // --------------------------------------------------------
        // PIC IRQ0 -> IDT VECTOR 32
        // --------------------------------------------------------

        (*idt).set_handler(32, unbound_irq0 as *const () as u64);

        // --------------------------------------------------------
        // LOAD IDTR
        // --------------------------------------------------------

        let pointer = DescriptorTablePointer {
            limit: (core::mem::size_of::<InterruptDescriptorTable>() - 1) as u16,

            base: core::ptr::addr_of!(IDT) as u64,
        };

        asm!(
            "lidt [{}]",
            in(reg) &pointer,
            options(
                readonly,
                nostack,
                preserves_flags
            ),
        );
    }
}

// ================================================================
// IDTR VERIFICATION
// ================================================================

pub fn current_base() -> u64 {
    unsafe { read_idtr().base }
}

pub fn current_limit() -> u16 {
    unsafe { read_idtr().limit }
}

pub fn verify() -> bool {
    let expected_base = core::ptr::addr_of!(IDT) as u64;

    let expected_limit = (core::mem::size_of::<InterruptDescriptorTable>() - 1) as u16;

    let actual = unsafe { read_idtr() };

    actual.base == expected_base && actual.limit == expected_limit
}

// ================================================================
// READ IDTR
// ================================================================

unsafe fn read_idtr() -> DescriptorTablePointer {
    let mut pointer = DescriptorTablePointer { limit: 0, base: 0 };

    unsafe {
        asm!(
            "sidt [{}]",
            in(reg) &mut pointer,
            options(
                nostack,
                preserves_flags
            ),
        );
    }

    pointer
}

// ================================================================
// CPU EXCEPTION ASSEMBLY
// ================================================================

core::arch::global_asm!(
    r#"
.section .text

.global unbound_isr0
.global unbound_isr1
.global unbound_isr2
.global unbound_isr3
.global unbound_isr4
.global unbound_isr5
.global unbound_isr6
.global unbound_isr7
.global unbound_isr8
.global unbound_isr9
.global unbound_isr10
.global unbound_isr11
.global unbound_isr12
.global unbound_isr13
.global unbound_isr14
.global unbound_isr15
.global unbound_isr16
.global unbound_isr17
.global unbound_isr18
.global unbound_isr19
.global unbound_isr20
.global unbound_isr21
.global unbound_isr22
.global unbound_isr23
.global unbound_isr24
.global unbound_isr25
.global unbound_isr26
.global unbound_isr27
.global unbound_isr28
.global unbound_isr29
.global unbound_isr30
.global unbound_isr31

// ================================================================
// EXCEPTIONS WITHOUT CPU ERROR CODE
// ================================================================

unbound_isr0:
    push 0
    push 0
    jmp unbound_exception_common

unbound_isr1:
    push 0
    push 1
    jmp unbound_exception_common

unbound_isr2:
    push 0
    push 2
    jmp unbound_exception_common

unbound_isr3:
    push 0
    push 3
    jmp unbound_exception_common

unbound_isr4:
    push 0
    push 4
    jmp unbound_exception_common

unbound_isr5:
    push 0
    push 5
    jmp unbound_exception_common

unbound_isr6:
    push 0
    push 6
    jmp unbound_exception_common

unbound_isr7:
    push 0
    push 7
    jmp unbound_exception_common

// ================================================================
// EXCEPTIONS WITH CPU ERROR CODE
// ================================================================

unbound_isr8:
    push 8
    jmp unbound_exception_common

unbound_isr9:
    push 0
    push 9
    jmp unbound_exception_common

unbound_isr10:
    push 10
    jmp unbound_exception_common

unbound_isr11:
    push 11
    jmp unbound_exception_common

unbound_isr12:
    push 12
    jmp unbound_exception_common

unbound_isr13:
    push 13
    jmp unbound_exception_common

unbound_isr14:
    push 14
    jmp unbound_exception_common

unbound_isr15:
    push 0
    push 15
    jmp unbound_exception_common

unbound_isr16:
    push 0
    push 16
    jmp unbound_exception_common

unbound_isr17:
    push 17
    jmp unbound_exception_common

unbound_isr18:
    push 0
    push 18
    jmp unbound_exception_common

unbound_isr19:
    push 0
    push 19
    jmp unbound_exception_common

unbound_isr20:
    push 0
    push 20
    jmp unbound_exception_common

unbound_isr21:
    push 0
    push 21
    jmp unbound_exception_common

unbound_isr22:
    push 0
    push 22
    jmp unbound_exception_common

unbound_isr23:
    push 0
    push 23
    jmp unbound_exception_common

unbound_isr24:
    push 0
    push 24
    jmp unbound_exception_common

unbound_isr25:
    push 0
    push 25
    jmp unbound_exception_common

unbound_isr26:
    push 0
    push 26
    jmp unbound_exception_common

unbound_isr27:
    push 0
    push 27
    jmp unbound_exception_common

unbound_isr28:
    push 0
    push 28
    jmp unbound_exception_common

unbound_isr29:
    push 0
    push 29
    jmp unbound_exception_common

unbound_isr30:
    push 0
    push 30
    jmp unbound_exception_common

unbound_isr31:
    push 0
    push 31
    jmp unbound_exception_common

// ================================================================
// COMMON CPU EXCEPTION ENTRY
// ================================================================

unbound_exception_common:

    push rax
    push rcx
    push rdx
    push rbx
    push rbp
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    // ------------------------------------------------------------
    // InterruptFrame:
    //
    // +120 = vector
    // +128 = error code
    // +136 = RIP
    // +144 = CS
    // +152 = RFLAGS
    // ------------------------------------------------------------

    lea rdi, [rsp + 120]

    // ------------------------------------------------------------
    // Align stack for SysV x86-64 CALL.
    // ------------------------------------------------------------

    sub rsp, 8

    call unbound_exception_handler

    add rsp, 8

    // ------------------------------------------------------------
    // Restore registers.
    // ------------------------------------------------------------

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rbp
    pop rbx
    pop rdx
    pop rcx
    pop rax

    // ------------------------------------------------------------
    // Remove:
    //
    //     vector
    //     error code
    //
    // Both are 8 bytes.
    // ------------------------------------------------------------

    add rsp, 8
iretq
"#
);

// ================================================================
// HARDWARE IRQ ASSEMBLY
// ================================================================

core::arch::global_asm!(
    r#"
.section .text

.global unbound_irq0

// ================================================================
// IRQ0 ENTRY
// ================================================================

unbound_irq0:

    // CPU already pushed:
    //
    //     RIP
    //     CS
    //     RFLAGS
    //
    // Add software vector 32.

    push 32

    jmp unbound_irq_common


// ================================================================
// COMMON HARDWARE IRQ ENTRY
// ================================================================

unbound_irq_common:

    // ------------------------------------------------------------
    // Save registers.
    //
    // RSP now points directly at:
    //
    //     +0    R15
    //     +8    R14
    //     +16   R13
    //     +24   R12
    //     +32   R11
    //     +40   R10
    //     +48   R9
    //     +56   R8
    //     +64   RDI
    //     +72   RSI
    //     +80   RBP
    //     +88   RBX
    //     +96   RDX
    //     +104  RCX
    //     +112  RAX
    //     +120  VECTOR
    //     +128  RIP
    //     +136  CS
    //     +144  RFLAGS
    //
    // ------------------------------------------------------------

    push rax
    push rcx
    push rdx
    push rbx
    push rbp
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    // ------------------------------------------------------------
    // RDI = current interrupt frame.
    // ------------------------------------------------------------

    mov rdi, rsp

    // ------------------------------------------------------------
    // Preserve current frame pointer.
    // ------------------------------------------------------------

    mov r12, rsp

    // ------------------------------------------------------------
    // Align stack for CALL.
    // ------------------------------------------------------------

    and rsp, -16

    // ------------------------------------------------------------
    // Call timer handler.
    //
    // RAX == 0:
    //     Resume current task.
    //
    // RAX != 0:
    //     RAX contains next task interrupt_rsp.
    // ------------------------------------------------------------

    call unbound_timer_handler

    // ------------------------------------------------------------
    // Restore current frame pointer.
    // ------------------------------------------------------------

    mov rsp, r12

    // ------------------------------------------------------------
    // Select context.
    // ------------------------------------------------------------

    test rax, rax
    jz .debug_selected

    mov rsp, rax

.debug_selected:

    // ------------------------------------------------------------
    // DEBUG:
    //
    // RSP points directly to the selected task's saved register
    // frame.
    // ------------------------------------------------------------

    mov rdi, rsp

call unbound_debug_iret_frame

    // ------------------------------------------------------------
    // Restore selected task registers.
    // ------------------------------------------------------------

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rbp
    pop rbx
    pop rdx
    pop rcx
    pop rax

    // ------------------------------------------------------------
    // Remove IRQ vector.
    // ------------------------------------------------------------

    add rsp, 8

    // ------------------------------------------------------------
    // Final IRET.
    //
    // RSP now points to:
    //
    //     RIP
    //     CS
    //     RFLAGS
    //
    // ------------------------------------------------------------

    iretq
"#
);
