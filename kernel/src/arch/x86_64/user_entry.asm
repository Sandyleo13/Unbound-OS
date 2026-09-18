.globl unbound_user_test_entry
.type unbound_user_test_entry, @function

unbound_user_test_entry:

    /*
     * SYS_GET_VERSION = 0
     *
     * Arguments:
     *   RAX = syscall number
     *   RDI = arg0
     *   RSI = arg1
     *   RDX = arg2
     */

    mov rax, 0
    xor rdi, rdi
    xor rsi, rsi
    xor rdx, rdx

    syscall

    /*
     * If SYSRETQ successfully returns to Ring 3,
     * execute HLT.
     *
     * HLT is privileged in CPL3 and therefore generates
     * #GP from Ring 3.
     *
     * The kernel exception handler can then verify that
     * the exception came from CS=0x33.
     */
    hlt

    ud2

.size unbound_user_test_entry, .-unbound_user_test_entry