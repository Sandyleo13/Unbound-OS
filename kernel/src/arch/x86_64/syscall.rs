use core::arch::global_asm;

pub const SYSCALL_STACK_SIZE: usize = 16 * 1024;

#[repr(align(16))]
struct SyscallStack {
    data: [u8; SYSCALL_STACK_SIZE],
}

static mut SYSCALL_STACK: SyscallStack = SyscallStack {
    data: [0; SYSCALL_STACK_SIZE],
};

#[unsafe(no_mangle)]
pub static mut unbound_syscall_stack_top: u64 = 0;

global_asm!(include_str!("syscall.asm"));

unsafe extern "C" {
    pub fn unbound_syscall_entry();
}

#[inline(always)]
pub fn entry_address() -> u64 {
    unbound_syscall_entry as *const () as usize as u64
}

#[inline(always)]
pub fn initialize_stack() {
    unsafe {
        unbound_syscall_stack_top =
            core::ptr::addr_of!(SYSCALL_STACK.data) as u64 + SYSCALL_STACK_SIZE as u64;
    }
}
