.global unbound_syscall_entry
.type unbound_syscall_entry, @function

unbound_syscall_entry:
    /*
     * Initial syscall ABI:
     *
     * RAX = syscall number
     * RDI = arg0
     * RSI = arg1
     * RDX = arg2
     *
     * SYSCALL places:
     * RCX = user RIP
     * R11 = user RFLAGS
     *
     * For this first milestone we do not enter user mode yet.
     * The entry point is prepared so the dispatcher can be tested
     * from kernel context first.
     */

    push rcx
    push r11

    /*
     * Preserve caller-visible registers that we modify.
     */
    push rdi
    push rsi
    push rdx

    /*
     * Keep stack aligned before the C/Rust call.
     */
    mov r12, rsp
    and rsp, -16

    /*
     * dispatch(number, arg0, arg1, arg2)
     *
     * RAX = number
     * RDI = arg0
     * RSI = arg1
     * RDX = arg2
     *
     * Rearrange for System V ABI:
     *
     * RDI = number
     * RSI = arg0
     * RDX = arg1
     * RCX = arg2
     */

    mov rcx, rdx
    mov rdx, rsi
    mov rsi, rdi
    mov rdi, rax

    call unbound_syscall_dispatch

    mov rsp, r12

    pop rdx
    pop rsi
    pop rdi

    pop r11
    pop rcx

    ret