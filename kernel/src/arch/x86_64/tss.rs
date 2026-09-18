#![allow(dead_code)]

//! x86_64 Task State Segment (TSS) support.
//!
//! The TSS is currently used for:
//! - RSP0: kernel stack used when entering Ring 0 from Ring 3
//! - IST1: dedicated stack for critical exceptions such as Double Fault

use core::mem::size_of;

pub const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 4;

/// Temporary Ring 0 stack used when the CPU transitions from
/// Ring 3 → Ring 0 for an interrupt or exception.
///
/// This is separate from the user stack.
pub const RING3_KERNEL_STACK_SIZE: usize = 16 * 1024;

#[repr(C, packed)]
pub struct TaskStateSegment {
    reserved1: u32,

    /// Ring 0 stack used when an interrupt/exception enters
    /// the kernel from CPL3.
    pub rsp0: u64,

    pub rsp1: u64,
    pub rsp2: u64,

    reserved2: u64,

    pub ist1: u64,
    pub ist2: u64,
    pub ist3: u64,
    pub ist4: u64,
    pub ist5: u64,
    pub ist6: u64,
    pub ist7: u64,

    reserved3: u64,
    reserved4: u16,

    pub iomap_base: u16,
}

pub const TSS_SIZE: usize = size_of::<TaskStateSegment>();

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            reserved1: 0,

            rsp0: 0,
            rsp1: 0,
            rsp2: 0,

            reserved2: 0,

            ist1: 0,
            ist2: 0,
            ist3: 0,
            ist4: 0,
            ist5: 0,
            ist6: 0,
            ist7: 0,

            reserved3: 0,
            reserved4: 0,

            iomap_base: 0,
        }
    }

    /// Initialize the TSS.
    ///
    /// IST1 is used for critical exceptions such as Double Fault.
    /// RSP0 is configured separately once the Ring 3 test kernel
    /// stack is available.
    pub fn init(&mut self) {
        let stack_start = core::ptr::addr_of!(DOUBLE_FAULT_STACK) as u64;

        let stack_end = stack_start + size_of::<DoubleFaultStack>() as u64;

        self.ist1 = stack_end;

        // Disable the TSS I/O bitmap by placing its base immediately
        // after the TSS structure.
        self.iomap_base = TSS_SIZE as u16;
    }

    /// Set the Ring 0 stack used for CPL3 → CPL0 transitions.
    pub fn set_rsp0(&mut self, stack_top: u64) {
        self.rsp0 = stack_top;
    }

    /// Read the current Ring 0 stack pointer.
    pub fn rsp0(&self) -> u64 {
        self.rsp0
    }
}

/// Dedicated stack for Double Fault / IST1.
#[repr(align(16))]
struct DoubleFaultStack([u8; DOUBLE_FAULT_STACK_SIZE]);

static mut DOUBLE_FAULT_STACK: DoubleFaultStack = DoubleFaultStack([0; DOUBLE_FAULT_STACK_SIZE]);

/// Dedicated Ring 0 stack used when entering the kernel from Ring 3.
///
/// This must NOT be the user stack.
#[repr(align(16))]
struct Ring3KernelStack {
    data: [u8; RING3_KERNEL_STACK_SIZE],
}

static mut RING3_KERNEL_STACK: Ring3KernelStack = Ring3KernelStack {
    data: [0; RING3_KERNEL_STACK_SIZE],
};

static mut TSS: TaskStateSegment = TaskStateSegment::new();

/// Initialize the TSS.
pub fn init() {
    unsafe {
        core::ptr::addr_of_mut!(TSS).as_mut().unwrap().init();
    }
}

/// Physical/linear address of the TSS.
pub fn address() -> u64 {
    core::ptr::addr_of!(TSS) as u64
}

/// Size of the TSS.
pub fn size() -> usize {
    TSS_SIZE
}

/// Current IST1 stack top.
pub fn ist1() -> u64 {
    unsafe { core::ptr::addr_of!(TSS).as_ref().unwrap().ist1 }
}

pub fn debug_ist1() -> u64 {
    ist1()
}

/// Return the top of the dedicated Ring 0 stack.
///
/// This address will be loaded into TSS.RSP0 before entering
/// the temporary Ring 3 test environment.
pub fn ring3_kernel_stack_top() -> u64 {
    unsafe { core::ptr::addr_of!(RING3_KERNEL_STACK.data) as u64 + RING3_KERNEL_STACK_SIZE as u64 }
}

/// Set the kernel stack used for CPL3 → CPL0 transitions.
pub fn set_rsp0(stack_top: u64) {
    unsafe {
        core::ptr::addr_of_mut!(TSS)
            .as_mut()
            .unwrap()
            .set_rsp0(stack_top);
    }
}

/// Read the current RSP0 value.
pub fn rsp0() -> u64 {
    unsafe { core::ptr::addr_of!(TSS).as_ref().unwrap().rsp0() }
}
