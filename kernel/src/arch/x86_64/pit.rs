use core::arch::asm;

// ================================================================
// 8253 / 8254 PIT
// ================================================================

const PIT_CHANNEL0_DATA: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

const PIT_BASE_FREQUENCY: u32 = 1_193_182;

pub const TIMER_FREQUENCY: u32 = 100;

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

// ================================================================
// INITIALIZATION
// ================================================================

pub fn init() {
    let divisor = PIT_BASE_FREQUENCY / TIMER_FREQUENCY;

    unsafe {
        // Channel 0
        //
        // 0x36:
        //   00 = channel 0
        //   11 = access low byte then high byte
        //   011 = mode 3 square wave
        //   0 = binary mode
        outb(PIT_COMMAND, 0x36);

        // Low byte.
        outb(PIT_CHANNEL0_DATA, (divisor & 0xFF) as u8);

        // High byte.
        outb(PIT_CHANNEL0_DATA, ((divisor >> 8) & 0xFF) as u8);
    }
}

pub const fn frequency() -> u32 {
    TIMER_FREQUENCY
}
