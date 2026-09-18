use core::arch::asm;

// ================================================================
// IA32 MSRs
// ================================================================

pub const IA32_EFER: u32 = 0xC000_0080;
pub const IA32_STAR: u32 = 0xC000_0081;
pub const IA32_LSTAR: u32 = 0xC000_0082;
pub const IA32_CSTAR: u32 = 0xC000_0083;
pub const IA32_FMASK: u32 = 0xC000_0084;

// ================================================================
// IA32_EFER FLAGS
// ================================================================

/// System Call Extensions Enable.
///
/// EFER.SCE = bit 0
pub const EFER_SCE: u64 = 1 << 0;

// ================================================================
// SYSCALL SELECTORS
// ================================================================
//
// Current GDT:
//
// 0x08 = Kernel Code
// 0x10 = Kernel Data
// 0x18 = TSS Low
// 0x20 = TSS High
// 0x28 = User Data
// 0x30 = User Code
//
// IA32_STAR:
//
// [63:48] = SYSRET selector base
// [47:32] = SYSCALL kernel CS
//
// For SYSCALL:
//
//     Kernel CS = STAR[47:32]
//     Kernel SS = Kernel CS + 8
//
// Therefore:
//
//     Kernel CS = 0x08
//     Kernel SS = 0x10
//
// For SYSRETQ:
//
//     User CS = STAR[63:48] + 16
//     User SS = STAR[63:48] + 8
//
// We want:
//
//     User CS = 0x30
//     User SS = 0x28
//
// Therefore:
//
//     STAR[63:48] = 0x20
//
//     0x20 + 16 = 0x30  -> User Code
//     0x20 +  8 = 0x28  -> User Data
//
// RPL=3 is supplied by the SYSRET mechanism.
//
// ================================================================

pub const STAR_KERNEL_CS: u64 = 0x08;
pub const STAR_USER_CS_BASE: u64 = 0x20;

// Expected Ring 3 selectors from the current GDT.

pub const USER_DATA_SELECTOR_R3: u16 = 0x2B;
pub const USER_CODE_SELECTOR_R3: u16 = 0x33;

// ================================================================
// FMASK
// ================================================================
//
// RFLAGS:
//
//     IF = bit 9 = 0x200
//
// SYSCALL clears every RFLAGS bit selected by FMASK.
//
// We initially clear IF so hardware interrupts cannot arrive
// while the syscall entry path establishes kernel state.
//
// ================================================================

pub const SYSCALL_FMASK: u64 = 0x0000_0000_0000_0200;

// ================================================================
// Raw MSR access
// ================================================================

#[inline(always)]
pub unsafe fn read(msr: u32) -> u64 {
    let low: u32;
    let high: u32;

    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(
                nostack,
                preserves_flags
            ),
        );
    }

    ((high as u64) << 32) | (low as u64)
}

#[inline(always)]
pub unsafe fn write(msr: u32, value: u64) {
    let low = value as u32;

    let high = (value >> 32) as u32;

    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(
                nostack,
                preserves_flags
            ),
        );
    }
}

// ================================================================
// EFER
// ================================================================

#[inline(always)]
pub unsafe fn read_efer() -> u64 {
    unsafe { read(IA32_EFER) }
}

#[inline(always)]
pub unsafe fn write_efer(value: u64) {
    unsafe {
        write(IA32_EFER, value);
    }
}

/// Enable the CPU SYSCALL/SYSRET extensions.
///
/// This only sets EFER.SCE.
///
/// It does NOT execute SYSCALL.
#[inline(always)]
pub unsafe fn enable_syscall_extensions() {
    let efer = unsafe { read_efer() };

    if (efer & EFER_SCE) == 0 {
        unsafe {
            write_efer(efer | EFER_SCE);
        }
    }
}

/// Return true when EFER.SCE is enabled.
#[inline(always)]
pub unsafe fn syscall_extensions_enabled() -> bool {
    unsafe { (read_efer() & EFER_SCE) != 0 }
}

// ================================================================
// STAR
// ================================================================

/// Construct IA32_STAR from the current GDT layout.
///
/// Result:
///
///     0x0020_0008_0000_0000
///
/// Upper half:
///
///     0x20 -> SYSRET selector base
///
/// Lower selector field:
///
///     0x08 -> SYSCALL kernel CS
#[inline(always)]
pub const fn make_star() -> u64 {
    (STAR_USER_CS_BASE << 48) | (STAR_KERNEL_CS << 32)
}

// ================================================================
// SYSCALL MSR READERS
// ================================================================

#[inline(always)]
pub unsafe fn read_star() -> u64 {
    unsafe { read(IA32_STAR) }
}

#[inline(always)]
pub unsafe fn read_lstar() -> u64 {
    unsafe { read(IA32_LSTAR) }
}

#[inline(always)]
pub unsafe fn read_cstar() -> u64 {
    unsafe { read(IA32_CSTAR) }
}

#[inline(always)]
pub unsafe fn read_fmask() -> u64 {
    unsafe { read(IA32_FMASK) }
}

// ================================================================
// INITIALIZE SYSCALL MSRs
// ================================================================
//
// This programs the CPU registers used by SYSCALL/SYSRET.
//
// It does NOT execute SYSCALL.
//
// ================================================================

pub unsafe fn init_syscall_msrs(lstar: u64) {
    let star = make_star();

    unsafe {
        // STAR:
        //
        // [63:48] = 0x20
        // [47:32] = 0x08
        //
        // Result:
        //
        // 0x0020_0008_0000_0000
        write(IA32_STAR, star);

        // 64-bit SYSCALL entry point.
        write(IA32_LSTAR, lstar);

        // Compatibility-mode entry point.
        //
        // We are not implementing compatibility-mode
        // syscall entry yet.
        write(IA32_CSTAR, 0);

        // Clear IF when SYSCALL executes.
        write(IA32_FMASK, SYSCALL_FMASK);
    }
}

// ================================================================
// VERIFY SYSCALL MSRs
// ================================================================

pub unsafe fn verify_syscall_msrs(lstar: u64) -> bool {
    let star = unsafe { read_star() };

    let actual_lstar = unsafe { read_lstar() };

    let cstar = unsafe { read_cstar() };

    let fmask = unsafe { read_fmask() };

    // Verify the complete STAR value.
    if star != make_star() {
        return false;
    }

    // Verify LSTAR points to the expected entry.
    if actual_lstar != lstar {
        return false;
    }

    // Compatibility entry is intentionally disabled.
    if cstar != 0 {
        return false;
    }

    // Verify interrupt masking behavior.
    if fmask != SYSCALL_FMASK {
        return false;
    }

    true
}

// ================================================================
// DEBUG / SELECTOR VERIFICATION
// ================================================================

/// Verify that the STAR/GDT relationship is exactly what our
/// current GDT expects for SYSRETQ.
#[inline(always)]
pub const fn verify_syscall_selectors() -> bool {
    let user_code = STAR_USER_CS_BASE + 16;

    let user_data = STAR_USER_CS_BASE + 8;

    user_code as u16 == 0x30
        && user_data as u16 == 0x28
        && USER_CODE_SELECTOR_R3 == 0x33
        && USER_DATA_SELECTOR_R3 == 0x2B
        && STAR_KERNEL_CS as u16 == 0x08
}

// ================================================================
// Expected selector helpers
// ================================================================

#[inline(always)]
pub const fn syscall_kernel_cs() -> u16 {
    STAR_KERNEL_CS as u16
}

#[inline(always)]
pub const fn syscall_kernel_ss() -> u16 {
    (STAR_KERNEL_CS + 8) as u16
}

#[inline(always)]
pub const fn sysret_user_cs() -> u16 {
    (STAR_USER_CS_BASE + 16) as u16 | 0x03
}

#[inline(always)]
pub const fn sysret_user_ss() -> u16 {
    (STAR_USER_CS_BASE + 8) as u16 | 0x03
}
