.globl unbound_syscall_entry
.type unbound_syscall_entry, @function

/*
 * SYSCALL entry
 *
 * On entry from Ring 3:
 *
 *   RAX = syscall number
 *   RDI = arg0
 *   RSI = arg1
 *   RDX = arg2
 *   RCX = user RIP
 *   R11 = user RFLAGS
 *   RSP = user RSP
 *
 * SYSCALL does not push anything and does not change RSP.
 *
 * We therefore:
 *
 *   1. Save the user RSP.
 *   2. Switch to the dedicated kernel syscall stack.
 *   3. Save the user return state.
 *   4. Save the syscall arguments/registers.
 *   5. Call the Rust dispatcher.
 *   6. Restore RCX/R11/RSP.
 *   7. SYSRETQ back to Ring 3.
 */

unbound_syscall_entry:

    /*
     * Save user RSP.
     *
     * R12 is saved into the syscall frame immediately after
     * switching stacks, so its temporary use here is safe.
     */
    mov r12, rsp

    /*
     * Switch to the dedicated kernel stack.
     */
    mov rsp, qword ptr [rip + unbound_syscall_stack_top]

    /*
     * Build the syscall frame.
     *
     * Keep the frame simple and explicitly defined.
     *
     * At the end:
     *
     *   [rsp +  0] = R15
     *   [rsp +  8] = R14
     *   [rsp + 16] = R13
     *   [rsp + 24] = R12
     *   [rsp + 32] = R10
     *   [rsp + 40] = R9
     *   [rsp + 48] = R8
     *   [rsp + 56] = RDI
     *   [rsp + 64] = RSI
     *   [rsp + 72] = RBP
     *   [rsp + 80] = RBX
     *   [rsp + 88] = RDX
     *   [rsp + 96] = RAX
     *   [rsp +104] = user R11
     *   [rsp +112] = user RCX
     *   [rsp +120] = user RSP
     */

    push r12
    push rcx
    push r11

    push rax
    push rdx
    push rbx
    push rbp
    push rsi
    push rdi
    push r8
    push r9
    push r10

    /*
     * R11, RCX and R12 have already been saved above.
     * We don't need duplicate copies of them.
     */

    push r13
    push r14
    push r15

    /*
     * Disable interrupts while inside the syscall path.
     */
    cli

    /*
     * At this point RSP points to R15.
     *
     * Rust dispatcher signature:
     *
     *   unbound_syscall_dispatch(
     *       number,
     *       arg0,
     *       arg1,
     *       arg2
     *   )
     *
     * The syscall number/arguments are still available in the
     * saved frame, so load them for the call.
     */

    mov rax, [rsp + 96]
    mov rdi, [rsp + 56]
    mov rsi, [rsp + 64]
    mov rdx, [rsp + 88]

    /*
     * Preserve the frame pointer across the Rust call.
     */
    mov r12, rsp

    /*
     * System V ABI requires RSP to be 16-byte aligned at a call.
     */
    and rsp, -16

    call unbound_syscall_dispatch

    /*
     * RAX now contains the syscall return value.
     */
    mov rsp, r12

    /*
     * Restore registers except RAX.
     *
     * RAX must contain the syscall return value for SYSRET.
     */

    pop r15
    pop r14
    pop r13

    pop r10
    pop r9
    pop r8

    pop rdi
    pop rsi
    pop rbp
    pop rbx
    pop rdx

    /*
     * Skip the saved original RAX.
     */
    add rsp, 8

    /*
     * Retrieve user return state.
     *
     * Current stack:
     *
     *   +0  user R11
     *   +8  user RCX
     *   +16 user RSP
     */

    pop r11
    pop rcx
    pop r12

    /*
     * Restore user stack.
     */
    mov rsp, r12

    /*
     * SYSRETQ:
     *
     *   RCX = user RIP
     *   R11 = user RFLAGS
     *   RSP = user RSP
     *
     * CPU derives:
     *
     *   CS = STAR[63:48] + 16 = 0x33
     *   SS = STAR[63:48] + 8  = 0x2B
     */
    sysretq

.size unbound_syscall_entry, .-unbound_syscall_entry
