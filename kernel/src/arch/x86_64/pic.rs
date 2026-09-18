use core::arch::asm;

// ================================================================
// 8259 PIC
// ================================================================
//
// Master PIC:
//   Command: 0x20
//   Data:    0x21
//
// Slave PIC:
//   Command: 0xA0
//   Data:    0xA1
//
// We remap:
//   Master IRQ0..7  -> vectors 32..39
//   Slave  IRQ8..15 -> vectors 40..47
//
// ================================================================

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;

const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;

const ICW4_8086: u8 = 0x01;

const MASTER_VECTOR_OFFSET: u8 = 32;
const SLAVE_VECTOR_OFFSET: u8 = 40;

// ================================================================
// PORT I/O
// ================================================================

#[inline]
unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags),
        );
    }
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;

    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags),
        );
    }

    value
}

#[inline]
fn io_wait() {
    unsafe {
        outb(0x80, 0);
    }
}

// ================================================================
// PIC INITIALIZATION
// ================================================================

pub fn init() {
    unsafe {
        let master_mask = inb(PIC1_DATA);
        let slave_mask = inb(PIC2_DATA);

        // Start initialization sequence.
        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        // Vector offsets.
        outb(PIC1_DATA, MASTER_VECTOR_OFFSET);
        io_wait();

        outb(PIC2_DATA, SLAVE_VECTOR_OFFSET);
        io_wait();

        // Wiring:
        // Master IRQ2 has the slave PIC connected.
        outb(PIC1_DATA, 0x04);
        io_wait();

        // Slave is connected to master's IRQ2.
        outb(PIC2_DATA, 0x02);
        io_wait();

        // 8086/88 mode.
        outb(PIC1_DATA, ICW4_8086);
        io_wait();

        outb(PIC2_DATA, ICW4_8086);
        io_wait();

        // Restore masks.
        outb(PIC1_DATA, master_mask);
        outb(PIC2_DATA, slave_mask);
    }

    // During early timer testing we only want IRQ0.
    //
    // Master:
    //   bit 0 = IRQ0 -> enabled
    //   bits 1..7 -> masked
    //
    // Slave:
    //   all IRQs masked
    mask_all_except_timer();
}

// ================================================================
// MASKING
// ================================================================

pub fn mask_all_except_timer() {
    unsafe {
        // IRQ0 enabled, every other master IRQ masked.
        outb(PIC1_DATA, 0b1111_1110);

        // All slave IRQs masked.
        outb(PIC2_DATA, 0xFF);
    }
}

pub fn enable_irq(irq: u8) {
    if irq >= 16 {
        return;
    }

    unsafe {
        if irq < 8 {
            let mask = inb(PIC1_DATA);
            outb(PIC1_DATA, mask & !(1 << irq));
        } else {
            let slave_irq = irq - 8;
            let mask = inb(PIC2_DATA);
            outb(PIC2_DATA, mask & !(1 << slave_irq));
        }
    }
}

pub fn disable_irq(irq: u8) {
    if irq >= 16 {
        return;
    }

    unsafe {
        if irq < 8 {
            let mask = inb(PIC1_DATA);
            outb(PIC1_DATA, mask | (1 << irq));
        } else {
            let slave_irq = irq - 8;
            let mask = inb(PIC2_DATA);
            outb(PIC2_DATA, mask | (1 << slave_irq));
        }
    }
}

// ================================================================
// END OF INTERRUPT
// ================================================================

pub fn end_of_interrupt(irq: u8) {
    unsafe {
        if irq >= 8 {
            outb(PIC2_COMMAND, 0x20);
        }

        outb(PIC1_COMMAND, 0x20);
    }
}
