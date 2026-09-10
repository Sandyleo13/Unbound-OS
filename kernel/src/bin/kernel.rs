#![no_std]
#![no_main]

#[path = "../boot_info.rs"]
mod boot_info;

use boot_info::{
    BootInfo,
    MemoryMapEntry,
    BOOT_INFO_MAGIC,
    BOOT_INFO_VERSION,
    MEMORY_MAP_ENTRY_SIZE,
};
use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    serial_write_string(b"KERNEL ENTRY\r\n");

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

    // Print the actual E820 entries.
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

    serial_write_string(b"\r\nUNBOUND KERNEL RUNNING\r\n");

    halt();
}

fn halt() -> ! {
    unsafe {
        asm!(
            "cli",
            "2:",
            "hlt",
            "jmp 2b",
            options(noreturn)
        );
    }
}

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

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_write_string(b"KERNEL PANIC\r\n");
    halt();
}