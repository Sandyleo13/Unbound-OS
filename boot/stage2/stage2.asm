bits 16
org 0x8000

; ============================================================
; UNBOUND STAGE 2
;
; Disk layout:
;   LBA 0       Stage 1
;   LBA 1-32    Stage 2
;   LBA 33+     Kernel ELF
;
; Kernel temporary buffer:
;   physical 0x20000
;
; Kernel final load addresses:
;   determined from ELF PT_LOAD entries
; ============================================================

start:
    cli

mov [boot_drive], dl

    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x8000

    call serial_init

    mov si, stage2_message
    call print_string_16

    ; --------------------------------------------------------
    ; Load kernel ELF from disk while BIOS services are usable.
    ;
    ; Kernel starts at LBA 33.
    ; 11000 bytes / 512 = 21.48, therefore 22 sectors.
    ;
    ; Destination = physical 0x20000
    ;              segment 0x2000:offset 0
    ; --------------------------------------------------------

    mov si, kernel_dap
    mov dl, [boot_drive]
    mov ah, 0x42
    int 0x13
    jc kernel_disk_error

    mov si, kernel_loaded_message
    call print_string_16

    ; --------------------------------------------------------
    ; Enter protected mode
    ; --------------------------------------------------------

    cli

    lgdt [gdt_descriptor]

    mov eax, cr0
    or eax, 0x00000001
    mov cr0, eax

    jmp 0x08:protected_mode_entry


; ============================================================
; 32-BIT PROTECTED MODE
; ============================================================

bits 32

protected_mode_entry:

    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    mov esp, 0x90000

    ; --------------------------------------------------------
    ; Enable A20
    ; --------------------------------------------------------

    in al, 0x92
    or al, 00000010b
    out 0x92, al

    ; --------------------------------------------------------
    ; Load PML4
    ; --------------------------------------------------------

    mov eax, pml4
    mov cr3, eax

    ; --------------------------------------------------------
    ; Enable PAE
    ; --------------------------------------------------------

    mov eax, cr4
    or eax, 0x20
    mov cr4, eax

    ; --------------------------------------------------------
    ; Enable Long Mode
    ; --------------------------------------------------------

    mov ecx, 0xC0000080
    rdmsr

    or eax, 0x100

    wrmsr

    ; --------------------------------------------------------
    ; Enable paging
    ; --------------------------------------------------------

    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    ; --------------------------------------------------------
    ; Enter 64-bit mode
    ; --------------------------------------------------------

    jmp 0x18:long_mode_entry


; ============================================================
; 64-BIT LONG MODE
; ============================================================

bits 64

long_mode_entry:

    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax

    xor eax, eax
    mov fs, ax
    mov gs, ax

    mov rsp, 0x90000

    mov rsi, long_mode_message
    call serial_write_string_64

    mov rsi, kernel_loading_message
    call serial_write_string_64

    ; --------------------------------------------------------
    ; Parse ELF64 and load PT_LOAD segments
    ; --------------------------------------------------------

    mov rsi, kernel_buffer

    ; Check ELF magic:
    ; 7F 'E' 'L' 'F'

    cmp dword [rsi], 0x464C457F
    jne elf_error

    ; Check ELF class = ELF64
    cmp byte [rsi + 4], 2
    jne elf_error

    ; Check little endian
    cmp byte [rsi + 5], 1
    jne elf_error

    ; --------------------------------------------------------
    ; Read ELF header fields
    ;
    ; e_entry    = +0x18
    ; e_phoff    = +0x20
    ; e_phentsize= +0x36
    ; e_phnum    = +0x38
    ; --------------------------------------------------------

    mov r12, [rsi + 0x18]       ; kernel entry

    mov r13, [rsi + 0x20]       ; program header offset

    movzx r14, word [rsi + 0x36] ; program header size

    movzx r15, word [rsi + 0x38] ; program header count

    ; rbx = address of first program header

    lea rbx, [rsi + r13]

    xor r8d, r8d                ; program header index


; ============================================================
; ELF PROGRAM HEADER LOOP
; ============================================================

elf_program_loop:

    cmp r8, r15
    jae elf_segments_done

    ; --------------------------------------------------------
    ; ELF64 Program Header:
    ;
    ; +0x00 p_type
    ; +0x08 p_offset
    ; +0x10 p_vaddr
    ; +0x18 p_paddr
    ; +0x20 p_filesz
    ; +0x28 p_memsz
    ; +0x30 p_flags
    ; +0x38 p_align
    ; --------------------------------------------------------

    mov eax, dword [rbx]
    cmp eax, 1                  ; PT_LOAD
    jne next_program_header

    ; --------------------------------------------------------
    ; Source = kernel_buffer + p_offset
    ; --------------------------------------------------------

    mov r9, [rbx + 0x08]
    lea rsi, [kernel_buffer + r9]

    ; --------------------------------------------------------
    ; Destination = p_paddr
    ; --------------------------------------------------------

    mov rdi, [rbx + 0x18]

    ; --------------------------------------------------------
    ; Copy p_filesz bytes
    ; --------------------------------------------------------

    mov rcx, [rbx + 0x20]

    call memory_copy

    ; --------------------------------------------------------
    ; Zero p_memsz - p_filesz
    ; --------------------------------------------------------

    mov rax, [rbx + 0x28]
    sub rax, [rbx + 0x20]

    mov rcx, rax
    xor eax, eax

    call memory_zero


next_program_header:

    add rbx, r14
    inc r8
    jmp elf_program_loop


; ============================================================
; ALL LOADABLE SEGMENTS COPIED
; ============================================================

elf_segments_done:

    mov rsi, kernel_ready_message
    call serial_write_string_64

    ; --------------------------------------------------------
    ; Jump to ELF entry point.
    ;
    ; Current kernel entry:
    ;   0x1001d0
    ;
    ; We deliberately use the ELF header's e_entry instead of
    ; hard-coding the address.
    ; --------------------------------------------------------

    jmp r12


; ============================================================
; MEMORY COPY
;
; rsi = source
; rdi = destination
; rcx = byte count
; ============================================================

memory_copy:

    test rcx, rcx
    jz .done

.copy_loop:
    mov al, [rsi]
    mov [rdi], al

    inc rsi
    inc rdi
    dec rcx

    jnz .copy_loop

.done:
    ret


; ============================================================
; MEMORY ZERO
;
; rdi = destination
; rcx = byte count
; ============================================================

memory_zero:

    test rcx, rcx
    jz .done

.zero_loop:
    mov byte [rdi], 0

    inc rdi
    dec rcx

    jnz .zero_loop

.done:
    ret


; ============================================================
; 64-BIT SERIAL STRING
;
; RSI = zero-terminated string
; ============================================================

serial_write_string_64:

.next:
    lodsb

    test al, al
    jz .done

    call serial_write_64
    jmp .next

.done:
    ret


; ============================================================
; 64-BIT SERIAL CHARACTER
; ============================================================

serial_write_64:

    push rax
    push rdx

.wait:
    mov dx, 0x3FD
    in al, dx

    test al, 0x20
    jz .wait

    pop rdx
    pop rax

    mov dx, 0x3F8
    out dx, al

    ret


; ============================================================
; 16-BIT SERIAL STRING
;
; DS:SI = zero-terminated string
; ============================================================

bits 16

print_string_16:

.next:
    lodsb

    test al, al
    jz .done

    call serial_write_16
    jmp .next

.done:
    ret


; ============================================================
; 16-BIT SERIAL INITIALIZATION
; ============================================================

serial_init:

    mov dx, 0x3F9
    xor al, al
    out dx, al

    mov dx, 0x3FB
    mov al, 0x80
    out dx, al

    mov dx, 0x3F8
    mov al, 0x03
    out dx, al

    mov dx, 0x3F9
    xor al, al
    out dx, al

    mov dx, 0x3FB
    mov al, 0x03
    out dx, al

    mov dx, 0x3FA
    mov al, 0xC7
    out dx, al

    mov dx, 0x3FC
    mov al, 0x0B
    out dx, al

    ret


; ============================================================
; 16-BIT SERIAL CHARACTER
; ============================================================

serial_write_16:

    push ax
    push dx

.wait:
    mov dx, 0x3FD
    in al, dx

    test al, 0x20
    jz .wait

    pop dx
    pop ax

    mov dx, 0x3F8
    out dx, al

    ret


; ============================================================
; ERROR HANDLERS
; ============================================================

kernel_disk_error:

    mov si, kernel_disk_error_message
    call print_string_16

.halt:
    cli
    hlt
    jmp .halt


bits 64

elf_error:

    mov rsi, elf_error_message
    call serial_write_string_64

.halt:
    cli
    hlt
    jmp .halt


; ============================================================
; MESSAGES
; ============================================================

bits 16

stage2_message:
    db "STAGE 2 LOADED", 13, 10, 0

kernel_loaded_message:
    db "KERNEL ELF LOADED", 13, 10, 0

kernel_disk_error_message:
    db "KERNEL DISK READ ERROR", 13, 10, 0


bits 64

long_mode_message:
    db "LONG MODE OK", 13, 10, 0

kernel_loading_message:
    db "KERNEL LOADING", 13, 10, 0

kernel_ready_message:
    db "KERNEL READY", 13, 10, 0

elf_error_message:
    db "ELF ERROR", 13, 10, 0


; ============================================================
; BOOT DRIVE
; ============================================================

bits 16

boot_drive:
    db 0


; ============================================================
; KERNEL BIOS DISK ADDRESS PACKET
;
; Read 22 sectors starting at LBA 33.
;
; Destination:
;   0x2000:0000
;   physical 0x20000
; ============================================================

align 4, db 0

kernel_dap:
    db 0x10
    db 0x00

    dw 22

    dw 0x0000
    dw 0x2000

    dq 33


; ============================================================
; GDT
; ============================================================

align 8, db 0

gdt_start:

gdt_null:
    dq 0

gdt_code32:
    dw 0xFFFF
    dw 0
    db 0
    db 0x9A
    db 0xCF
    db 0

gdt_data:
    dw 0xFFFF
    dw 0
    db 0
    db 0x92
    db 0xCF
    db 0

gdt_code64:
    dw 0xFFFF
    dw 0
    db 0
    db 0x9A
    db 0xAF
    db 0

gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start


; ============================================================
; PAGE TABLES
;
; Identity-map first 2 MiB.
;
; PML4
;   ↓
; PDPT
;   ↓
; Page Directory
;   ↓
; 2 MiB page
;
; Kernel is loaded at 0x100000, so it is inside this mapping.
; ============================================================

align 4096, db 0

pml4:
    dq pdpt + 0x003
    times 511 dq 0

align 4096, db 0

pdpt:
    dq page_directory + 0x003
    times 511 dq 0

align 4096, db 0

page_directory:
    dq 0x0000000000000083
    times 511 dq 0


; ============================================================
; CONSTANTS / LABELS
; ============================================================

kernel_buffer equ 0x20000

stage2_end:
