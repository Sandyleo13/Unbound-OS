pub const SYS_GET_VERSION: u64 = 0;

pub const UNBOUND_SYSCALL_ABI_VERSION: u64 = 1;

pub fn dispatch(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    match number {
        SYS_GET_VERSION => {
            let _ = arg0;
            let _ = arg1;
            let _ = arg2;

            UNBOUND_SYSCALL_ABI_VERSION
        }

        _ => u64::MAX,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn unbound_syscall_dispatch(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    dispatch(number, arg0, arg1, arg2)
}
