// ================================================================
// UNBOUND CPU CONTEXT SWITCH
// ================================================================

use core::arch::global_asm;

global_asm!(
    r#"
.global unbound_context_switch
.type unbound_context_switch, @function

// ------------------------------------------------------------
// Cooperative context switch
//
// rdi = old TaskContext*
// rsi = new TaskContext*
//
// TaskContext:
//
//   +0   RSP
//   +8   RBP
//   +16  RBX
//   +24  R12
//   +32  R13
//   +40  R14
//   +48  R15
//   +56  RIP
// ------------------------------------------------------------

unbound_context_switch:

    // --------------------------------------------------------
    // Save current context.
    //
    // The CALL instruction placed the return address at [rsp].
    // --------------------------------------------------------

    mov rax, [rsp]

    // Save RIP.
    mov [rdi + 56], rax

    // Save RSP.
    lea rax, [rsp + 8]
    mov [rdi + 0], rax

    // Save callee-saved registers.
    mov [rdi + 8],  rbp
    mov [rdi + 16], rbx
    mov [rdi + 24], r12
    mov [rdi + 32], r13
    mov [rdi + 40], r14
    mov [rdi + 48], r15

    // --------------------------------------------------------
    // Restore new context.
    // --------------------------------------------------------

    mov rsp, [rsi + 0]

    mov rbp, [rsi + 8]
    mov rbx, [rsi + 16]
    mov r12, [rsi + 24]
    mov r13, [rsi + 32]
    mov r14, [rsi + 40]
    mov r15, [rsi + 48]

    mov rax, [rsi + 56]

    // Jump to saved RIP.
    jmp rax

.size unbound_context_switch, .-unbound_context_switch


// ============================================================
// PREEMPTIVE TASK START
// ============================================================
//
// RSP initially points at:
//
//   +0    R15
//   +8    R14
//   +16   R13
//   +24   R12
//   +32   R11
//   +40   R10
//   +48   R9
//   +56   R8
//   +64   RDI
//   +72   RSI
//   +80   RBP
//   +88   RBX
//   +96   RDX
//   +104  RCX
//   +112  RAX
//   +120  VECTOR
//   +128  RIP
//   +136  CS
//   +144  RFLAGS
//
// ============================================================

.global unbound_start_interrupt_context
.type unbound_start_interrupt_context, @function

unbound_start_interrupt_context:

    // --------------------------------------------------------
    // Restore registers in exact reverse order.
    // --------------------------------------------------------

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8

    pop rdi
    pop rsi
    pop rbp
    pop rbx
    pop rdx
    pop rcx
    pop rax

    // --------------------------------------------------------
    // Remove synthetic IRQ vector.
    // --------------------------------------------------------

    add rsp, 8

    // --------------------------------------------------------
    // RSP now points to:
    //
    //   +0   RIP
    //   +8   CS
    //   +16  RFLAGS
    //
    // iretq restores the task.
    // --------------------------------------------------------

    iretq

.size unbound_start_interrupt_context, .-unbound_start_interrupt_context
"#
);

unsafe extern "C" {
    pub fn unbound_context_switch(
        old: *mut super::task::TaskContext,
        new: *const super::task::TaskContext,
    );

    pub fn unbound_start_interrupt_context();
}

pub unsafe fn switch(old: *mut super::task::TaskContext, new: *const super::task::TaskContext) {
    unsafe {
        unbound_context_switch(old, new);
    }
}

pub const TASK_CONTEXT_SIZE: usize = core::mem::size_of::<super::task::TaskContext>();
