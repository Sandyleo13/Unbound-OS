use core::arch::{asm, global_asm};

const IDT_ENTRIES: usize = 256;

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const INTERRUPT_GATE: u16 = 0x8E;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    options: u16,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            options: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    fn set_handler(&mut self, handler: u64) {
        self.offset_low = handler as u16;
        self.selector = KERNEL_CODE_SELECTOR;
        self.options = INTERRUPT_GATE;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.reserved = 0;
    }
}

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

    fn address(&self) -> u64 {
        self as *const Self as u64
    }

    fn limit(&self) -> u16 {
        (core::mem::size_of::<Self>() - 1) as u16
    }
}

static mut IDT: InterruptDescriptorTable =
    InterruptDescriptorTable::new();

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
}

pub fn init() {
    unsafe {
        let idt = core::ptr::addr_of_mut!(IDT);

        (*idt).set_handler(
            0,
            unbound_isr0 as *const () as usize as u64,
        );

        (*idt).set_handler(
            1,
            unbound_isr1 as *const () as usize as u64,
        );

        (*idt).set_handler(
            2,
            unbound_isr2 as *const () as usize as u64,
        );

        (*idt).set_handler(
            3,
            unbound_isr3 as *const () as usize as u64,
        );

        (*idt).set_handler(
            4,
            unbound_isr4 as *const () as usize as u64,
        );

        (*idt).set_handler(
            5,
            unbound_isr5 as *const () as usize as u64,
        );

        (*idt).set_handler(
            6,
            unbound_isr6 as *const () as usize as u64,
        );

        (*idt).set_handler(
            7,
            unbound_isr7 as *const () as usize as u64,
        );

        (*idt).set_handler(
            8,
            unbound_isr8 as *const () as usize as u64,
        );

        (*idt).set_handler(
            9,
            unbound_isr9 as *const () as usize as u64,
        );

        (*idt).set_handler(
            10,
            unbound_isr10 as *const () as usize as u64,
        );

        (*idt).set_handler(
            11,
            unbound_isr11 as *const () as usize as u64,
        );

        (*idt).set_handler(
            12,
            unbound_isr12 as *const () as usize as u64,
        );

        (*idt).set_handler(
            13,
            unbound_isr13 as *const () as usize as u64,
        );

        (*idt).set_handler(
            14,
            unbound_isr14 as *const () as usize as u64,
        );

        (*idt).set_handler(
            15,
            unbound_isr15 as *const () as usize as u64,
        );

        (*idt).set_handler(
            16,
            unbound_isr16 as *const () as usize as u64,
        );

        (*idt).set_handler(
            17,
            unbound_isr17 as *const () as usize as u64,
        );

        (*idt).set_handler(
            18,
            unbound_isr18 as *const () as usize as u64,
        );

        (*idt).set_handler(
            19,
            unbound_isr19 as *const () as usize as u64,
        );

        (*idt).set_handler(
            20,
            unbound_isr20 as *const () as usize as u64,
        );

        (*idt).set_handler(
            21,
            unbound_isr21 as *const () as usize as u64,
        );

        (*idt).set_handler(
            22,
            unbound_isr22 as *const () as usize as u64,
        );

        (*idt).set_handler(
            23,
            unbound_isr23 as *const () as usize as u64,
        );

        (*idt).set_handler(
            24,
            unbound_isr24 as *const () as usize as u64,
        );

        (*idt).set_handler(
            25,
            unbound_isr25 as *const () as usize as u64,
        );

        (*idt).set_handler(
            26,
            unbound_isr26 as *const () as usize as u64,
        );

        (*idt).set_handler(
            27,
            unbound_isr27 as *const () as usize as u64,
        );

        (*idt).set_handler(
            28,
            unbound_isr28 as *const () as usize as u64,
        );

        (*idt).set_handler(
            29,
            unbound_isr29 as *const () as usize as u64,
        );

        (*idt).set_handler(
            30,
            unbound_isr30 as *const () as usize as u64,
        );

        (*idt).set_handler(
            31,
            unbound_isr31 as *const () as usize as u64,
        );

        let base = (*idt).address();
        let limit = (*idt).limit();

        load_idt(base, limit);
    }
}

pub fn current_base() -> u64 {
    unsafe {
        read_idtr().base
    }
}

pub fn current_limit() -> u16 {
    unsafe {
        read_idtr().limit
    }
}

pub fn verify() -> bool {
    let expected_base =
        core::ptr::addr_of!(IDT) as *const _ as u64;

    let expected_limit =
        (core::mem::size_of::<InterruptDescriptorTable>() - 1)
            as u16;

    let actual = unsafe {
        read_idtr()
    };

    actual.base == expected_base
        && actual.limit == expected_limit
}

unsafe fn load_idt(base: u64, limit: u16) {
    let pointer = DescriptorTablePointer {
        limit,
        base,
    };

    unsafe {
        asm!(
            "lidt [{}]",
            in(reg) &pointer,
            options(readonly, nostack, preserves_flags),
        );
    }
}

unsafe fn read_idtr() -> DescriptorTablePointer {
    let mut pointer = DescriptorTablePointer {
        limit: 0,
        base: 0,
    };

    unsafe {
        asm!(
            "sidt [{}]",
            in(reg) &mut pointer,
            options(nostack, preserves_flags),
        );
    }

    pointer
}

// ================================================================
// INTERRUPT FRAME
// ================================================================

#[repr(C)]
pub struct InterruptFrame {
    pub vector: u64,
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

// ================================================================
// CPU EXCEPTION HANDLER
// ================================================================

#[unsafe(no_mangle)]
extern "C" fn unbound_exception_handler(
    frame: &InterruptFrame,
) -> ! {
    unsafe {
        asm!(
            "cli",
            options(nostack, preserves_flags)
        );
    }

    crate::serial_write_string(
        b"\r\n\r\n==============================\r\n",
    );

    crate::serial_write_string(
        b"UNBOUND CPU EXCEPTION\r\n",
    );

    crate::serial_write_string(
        b"VECTOR: 0x",
    );

    crate::serial_write_hex64(
        frame.vector,
    );

    crate::serial_write_string(
        b"\r\nERROR: 0x",
    );

    crate::serial_write_hex64(
        frame.error_code,
    );

    crate::serial_write_string(
        b"\r\nRIP: 0x",
    );

    crate::serial_write_hex64(
        frame.rip,
    );

    crate::serial_write_string(
        b"\r\nCS: 0x",
    );

    crate::serial_write_hex64(
        frame.cs,
    );

    crate::serial_write_string(
        b"\r\nRFLAGS: 0x",
    );

    crate::serial_write_hex64(
        frame.rflags,
    );

    crate::serial_write_string(
        b"\r\n==============================\r\n",
    );

    loop {
        unsafe {
            asm!(
                "hlt",
                options(nostack, preserves_flags)
            );
        }
    }
}

// ================================================================
// INTERRUPT STUBS
// ================================================================

global_asm!(
    r#"
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

.extern unbound_exception_handler

.macro ISR_NO_ERROR number
unbound_isr\number:
    push 0
    push \number
    jmp unbound_exception_common
.endm

.macro ISR_ERROR number
unbound_isr\number:
    push \number
    jmp unbound_exception_common
.endm

ISR_NO_ERROR 0
ISR_NO_ERROR 1
ISR_NO_ERROR 2
ISR_NO_ERROR 3
ISR_NO_ERROR 4
ISR_NO_ERROR 5
ISR_NO_ERROR 6
ISR_NO_ERROR 7

ISR_ERROR 8

ISR_NO_ERROR 9

ISR_ERROR 10
ISR_ERROR 11
ISR_ERROR 12
ISR_ERROR 13
ISR_ERROR 14

ISR_NO_ERROR 15
ISR_NO_ERROR 16

ISR_ERROR 17

ISR_NO_ERROR 18
ISR_NO_ERROR 19
ISR_NO_ERROR 20
ISR_NO_ERROR 21
ISR_NO_ERROR 22
ISR_NO_ERROR 23
ISR_NO_ERROR 24
ISR_NO_ERROR 25
ISR_NO_ERROR 26
ISR_NO_ERROR 27
ISR_NO_ERROR 28

ISR_ERROR 29
ISR_ERROR 30

ISR_NO_ERROR 31

unbound_exception_common:
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax

    mov rdi, rsp

    call unbound_exception_handler

unbound_exception_halt:
    cli
    hlt
    jmp unbound_exception_halt
"#
);
