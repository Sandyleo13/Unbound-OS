use core::arch::{asm, global_asm};

use super::gdt::{USER_CODE_SELECTOR_R3, USER_DATA_SELECTOR_R3};

pub const USER_CODE_VADDR: u64 = 0x0000_0000_0040_0000;

pub const USER_STACK_VADDR: u64 = 0x0000_0000_0080_0000;

pub const USER_STACK_SIZE: usize = 16 * 1024;

#[repr(align(4096))]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

static mut USER_STACK: UserStack = UserStack {
    data: [0; USER_STACK_SIZE],
};

global_asm!(include_str!("user_entry.asm"));

unsafe extern "C" {
    pub fn unbound_user_test_entry();
}

/// Physical address of the temporary test stack.
#[inline(always)]
pub fn stack_physical_start() -> u64 {
    unsafe { core::ptr::addr_of!(USER_STACK.data) as u64 }
}

/// End of the temporary test stack.
#[inline(always)]
pub fn stack_physical_end() -> u64 {
    stack_physical_start() + USER_STACK_SIZE as u64
}

/// Address of the temporary user test entry.
#[inline(always)]
pub fn test_entry_physical_address() -> u64 {
    unbound_user_test_entry as *const () as usize as u64
}

/// Enter the temporary Ring 3 test environment.
///
/// `entry` must be a user-accessible virtual address.
///
/// The target executes SYSCALL immediately.
pub unsafe fn enter_ring3(entry: u64, user_stack: u64) -> ! {
    unsafe {
        asm!(
            "push {ss}",
            "push {rsp}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",
            "iretq",

            ss = in(reg) USER_DATA_SELECTOR_R3 as u64,
            rsp = in(reg) user_stack,
            rflags = in(reg) 0x202u64,
            cs = in(reg) USER_CODE_SELECTOR_R3 as u64,
            rip = in(reg) entry,

            options(noreturn)
        );
    }
}
