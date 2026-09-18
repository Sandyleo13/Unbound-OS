// ================================================================
// UNBOUND TASK
// ================================================================
//
// Phase 3.4:
//   Kernel task CPU context + interrupt context foundation.
//
// ================================================================

use core::sync::atomic::{AtomicU64, Ordering};

use super::context::unbound_start_interrupt_context;

// ================================================================
// TASK ID
// ================================================================

pub type TaskId = u64;

// ================================================================
// TASK STATE
// ================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskState {
    Ready = 0,
    Running = 1,
    Blocked = 2,
    Terminated = 3,
}

impl TaskState {
    pub fn name(self) -> &'static [u8] {
        match self {
            Self::Ready => b"READY",
            Self::Running => b"RUNNING",
            Self::Blocked => b"BLOCKED",
            Self::Terminated => b"TERMINATED",
        }
    }
}

// ================================================================
// KERNEL STACK
// ================================================================

pub const KERNEL_STACK_SIZE: usize = 16 * 1024;

// Keep the stack aligned for ABI correctness.
#[repr(align(16))]
pub struct KernelStack {
    pub data: [u8; KERNEL_STACK_SIZE],
}

impl KernelStack {
    pub const fn new() -> Self {
        Self {
            data: [0; KERNEL_STACK_SIZE],
        }
    }

    #[inline]
    pub fn top(&self) -> u64 {
        let start = core::ptr::addr_of!(self.data) as u64;

        start + KERNEL_STACK_SIZE as u64
    }
}

// ================================================================
// CPU CONTEXT
// ================================================================
//
// Used by the low-level cooperative context switch.
//
// Layout:
//
//   +0   RSP
//   +8   RBP
//   +16  RBX
//   +24  R12
//   +32  R13
//   +40  R14
//   +48  R15
//   +56  RIP
//
// ================================================================

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TaskContext {
    pub rsp: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
}

impl TaskContext {
    pub const fn empty() -> Self {
        Self {
            rsp: 0,
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: 0,
        }
    }
}

// ================================================================
// INTERRUPT CONTEXT
// ================================================================
//
// The hardware IRQ0 path builds:
//
//   push 32
//   push rax
//   push rcx
//   ...
//   push r15
//
// Therefore, after the register pushes:
//
//   RSP + 0    R15
//   RSP + 8    R14
//   RSP + 16   R13
//   RSP + 24   R12
//   RSP + 32   R11
//   RSP + 40   R10
//   RSP + 48   R9
//   RSP + 56   R8
//   RSP + 64   RDI
//   RSP + 72   RSI
//   RSP + 80   RBP
//   RSP + 88   RBX
//   RSP + 96   RDX
//   RSP + 104  RCX
//   RSP + 112  RAX
//   RSP + 120  VECTOR
//   RSP + 128  RIP
//   RSP + 136  CS
//   RSP + 144  RFLAGS
//
// Total:
//
//   19 * 8 = 152 bytes
//
// ================================================================

const INTERRUPT_CONTEXT_SIZE: u64 = 19 * 8;

// Reserve space ABOVE the synthetic frame.
//
// The task begins at:
//
//   context_rsp + 152
//
// which is:
//
//   stack_top - INITIAL_STACK_RESERVE
//
// This prevents the first normal function calls from immediately
// overwriting the synthetic interrupt frame.
//
// ================================================================

const INITIAL_STACK_RESERVE: u64 = 2048;

// ================================================================
// TASK
// ================================================================

#[repr(C)]
pub struct Task {
    pub id: TaskId,

    pub state: TaskState,

    pub priority: u8,

    pub _reserved: [u8; 6],

    pub time_slice_ticks: u64,

    pub total_ticks: u64,

    // ------------------------------------------------------------
    // Phase 3.3 cooperative CPU context.
    // ------------------------------------------------------------
    pub context: TaskContext,

    // ------------------------------------------------------------
    // Phase 3.4 interrupt CPU context.
    //
    // 0 = task has not yet received an interrupt context.
    // ------------------------------------------------------------
    pub interrupt_rsp: u64,

    // ------------------------------------------------------------
    // Private kernel stack.
    // ------------------------------------------------------------
    pub kernel_stack: KernelStack,
}

impl Task {
    // ============================================================
    // CREATE TASK
    // ============================================================

    pub const fn new(id: TaskId, priority: u8) -> Self {
        Self {
            id,

            state: TaskState::Ready,

            priority,

            _reserved: [0; 6],

            time_slice_ticks: 0,

            total_ticks: 0,

            // Cooperative context initially empty.
            context: TaskContext::empty(),

            // Interrupt context initially empty.
            interrupt_rsp: 0,

            kernel_stack: KernelStack::new(),
        }
    }

    // ============================================================
    // SET STATE
    // ============================================================

    pub fn set_state(&mut self, state: TaskState) {
        self.state = state;
    }

    // ============================================================
    // RUNNABLE
    // ============================================================

    pub fn is_runnable(&self) -> bool {
        matches!(self.state, TaskState::Ready | TaskState::Running)
    }

    // ============================================================
    // COOPERATIVE CONTEXT
    // ============================================================
    //
    // Used by Phase 3.3.
    //
    // The cooperative context switch performs:
    //
    //   restore registers
    //   jmp context.rip
    //
    // For a normal task:
    //
    //   context.rsp -> return address
    //   context.rip -> task entry
    //
    // ============================================================

    pub fn initialize_context(&mut self, entry: u64) {
        let stack_top = self.kernel_stack.top();

        // Leave 8 bytes for the return address.
        let initial_rsp = stack_top - 8;

        unsafe {
            let stack_return = initial_rsp as *mut u64;

            stack_return.write(task_exit as *const () as usize as u64);
        }

        self.context = TaskContext {
            rsp: initial_rsp,

            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,

            rip: entry,
        };
    }

    // ============================================================
    // PREEMPTIVE INTERRUPT CONTEXT
    // ============================================================
    //
    // Creates a synthetic IRQ0-compatible stack frame.
    //
    // IMPORTANT:
    //
    // The frame is NOT placed directly at the top of the stack.
    //
    // Layout:
    //
    //   stack_top
    //       |
    //       |  2048-byte startup stack area
    //       |
    //       +----------------------------
    //       | R15
    //       | R14
    //       | R13
    //       | R12
    //       | R11
    //       | R10
    //       | R9
    //       | R8
    //       | RDI
    //       | RSI
    //       | RBP
    //       | RBX
    //       | RDX
    //       | RCX
    //       | RAX
    //       | VECTOR
    //       | RIP
    //       | CS
    //       | RFLAGS
    //       +----------------------------
    //       |
    //       context_rsp
    //
    // ============================================================

    pub fn initialize_interrupt_context(&mut self, entry: u64) {
        let stack_top = self.kernel_stack.top();

        let context_rsp = stack_top - INITIAL_STACK_RESERVE - INTERRUPT_CONTEXT_SIZE;

        unsafe {
            let frame = context_rsp as *mut u64;

            // ----------------------------------------------------
            // Saved general-purpose registers.
            // ----------------------------------------------------

            frame.add(0).write(0); // R15
            frame.add(1).write(0); // R14
            frame.add(2).write(0); // R13
            frame.add(3).write(0); // R12
            frame.add(4).write(0); // R11
            frame.add(5).write(0); // R10
            frame.add(6).write(0); // R9
            frame.add(7).write(0); // R8

            frame.add(8).write(0); // RDI
            frame.add(9).write(0); // RSI
            frame.add(10).write(0); // RBP
            frame.add(11).write(0); // RBX
            frame.add(12).write(0); // RDX
            frame.add(13).write(0); // RCX
            frame.add(14).write(0); // RAX

            // ----------------------------------------------------
            // Synthetic IRQ vector.
            //
            // IRQ0 = PIC vector 0x20.
            // ----------------------------------------------------

            frame.add(15).write(0x20);

            // ----------------------------------------------------
            // Synthetic IRET frame.
            // ----------------------------------------------------

            // RIP
            frame.add(16).write(entry);

            // Kernel code selector.
            frame.add(17).write(0x08);

            // RFLAGS:
            //
            // Bit 1 = reserved and must be 1.
            // Bit 9 = IF.
            //
            frame.add(18).write(0x202);
        }

        self.interrupt_rsp = context_rsp;
    }

    // ============================================================
    // PREEMPTIVE START
    // ============================================================
    //
    // A preemptive task cannot be started using the normal
    // cooperative entry path.
    //
    // Instead:
    //
    //   TaskContext
    //        |
    //        v
    //   unbound_start_interrupt_context
    //        |
    //        v
    //   restore synthetic registers
    //        |
    //        v
    //   skip vector
    //        |
    //        v
    //   iretq
    //        |
    //        v
    //   task entry
    //
    // After the trampoline removes the 15 registers + vector,
    // iretq consumes:
    //
    //   RIP
    //   CS
    //   RFLAGS
    //
    // and leaves RSP at:
    //
    //   context_rsp + 152
    //
    // which equals:
    //
    //   stack_top - INITIAL_STACK_RESERVE
    //
    // ============================================================

    pub fn prepare_preemptive_start(&mut self) {
        debug_assert!(
            self.interrupt_rsp != 0,
            "interrupt context must be initialized first"
        );

        self.context = TaskContext {
            rsp: self.interrupt_rsp,

            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,

            rip: unbound_start_interrupt_context as *const () as usize as u64,
        };
    }

    // ============================================================
    // DEBUG
    // ============================================================

    pub fn debug_interrupt_context(&self) {
        let rsp = self.interrupt_rsp;

        if rsp == 0 {
            return;
        }

        unsafe {
            let frame = rsp as *const u64;

            crate::serial_write_string(b"TASK INTERRUPT FRAME\r\n");

            crate::serial_write_string(b"  RSP: ");
            crate::serial_write_hex64(rsp);
            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"  VECTOR: ");
            crate::serial_write_hex64(frame.add(15).read());
            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"  RIP: ");
            crate::serial_write_hex64(frame.add(16).read());
            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"  CS: ");
            crate::serial_write_hex64(frame.add(17).read());
            crate::serial_write_string(b"\r\n");

            crate::serial_write_string(b"  RFLAGS: ");
            crate::serial_write_hex64(frame.add(18).read());
            crate::serial_write_string(b"\r\n");
        }
    }
}

// ================================================================
// TASK EXIT
// ================================================================
//
// Temporary safety endpoint.
//
// A task must never return into arbitrary memory.
//
// ================================================================

extern "C" fn task_exit() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// ================================================================
// TASK ID GENERATOR
// ================================================================

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

pub fn allocate_task_id() -> TaskId {
    NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
}
