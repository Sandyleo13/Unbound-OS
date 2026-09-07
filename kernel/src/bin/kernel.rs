#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    unsafe {
        asm!(
            "lea rsi, [rip + 8f]",

            "7:",
            "lodsb",
            "test al, al",
            "jz 6f",

            "push rax",
            "push rdx",

            "5:",
            "mov dx, 0x3FD",
            "in al, dx",
            "test al, 0x20",
            "jz 5b",

            "pop rdx",
            "pop rax",

            "mov dx, 0x3F8",
            "out dx, al",

            "jmp 7b",

            "6:",
            "cli",

            "4:",
            "hlt",
            "jmp 4b",

            "8:",
            ".ascii \"UNBOUND KERNEL\\r\\n\\0\"",

            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
