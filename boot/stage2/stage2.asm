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
    ; Load kernel ELF
    ; --------------------------------------------------------

    mov si, kernel_dap
    mov dl, [boot_drive]
    mov ah, 0x42
    int 0x13
    jc kernel_disk_error

    mov si, kernel_loaded_message
    call print_string_16

    ; --------------------------------------------------------
    ; Get BIOS physical memory map using INT 15h E820
    ;
    ; Buffer:
    ;   physical 0x60000
    ;
    ; Maximum:
    ;   128 entries
    ;   128 * 24 = 3072 bytes
    ; --------------------------------------------------------

    call detect_memory_map
    jc memory_map_error

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
; BIOS E820 MEMORY MAP
;
; Output:
;   memory_map_count = number of entries
;
; Buffer:
;   physical 0x60000
;
; Each entry:
;   +0x00 base address     u64
;   +0x08 length           u64
;   +0x10 type             u32
;   +0x14 attributes       u32
;
; BIOS interface:
;   EAX = 0xE820
;   EDX = "SMAP"
;   ECX = 24
;   EBX = continuation value
;   ES:DI = destination
; ============================================================

bits 16

detect_memory_map:

    push es
    push di
    push si
    push dx

    ; E820 buffer = physical 0x60000
    mov ax, 0x6000
    mov es, ax
    xor di, di

    ; EBX = E820 continuation value
    xor ebx, ebx

    ; SI = entry count
    xor si, si

.e820_next:

    ; Maximum 128 entries
    cmp si, 128
    jae .success

    ; --------------------------------------------------------
    ; INT 15h, E820
    ; --------------------------------------------------------

    mov eax, 0xE820
    mov edx, 0x534D4150        ; "SMAP"
    mov ecx, 24

    ; Extended attributes = valid
    mov dword [es:di + 20], 1

    int 0x15

    ; --------------------------------------------------------
    ; Check BIOS result
    ; --------------------------------------------------------

    ; Carry flag = BIOS error
    jc .cf_error

    ; BIOS must return SMAP signature
    cmp eax, 0x534D4150
    jne .signature_error

    ; BIOS must return at least 20 bytes
    cmp ecx, 20
    jb .size_error

    ; Valid entry
    inc si
    add di, 24

    ; EBX = 0 means final entry
    test ebx, ebx
    jz .success

    jmp .e820_next


.success:

    mov [memory_map_count], si

    clc

    pop dx
    pop si
    pop di
    pop es
    ret


.cf_error:

    mov byte [memory_map_error_code], 1
    mov word [memory_map_count], 0

    stc

    pop dx
    pop si
    pop di
    pop es
    ret


.signature_error:

    mov byte [memory_map_error_code], 2
    mov word [memory_map_count], 0

    stc

    pop dx
    pop si
    pop di
    pop es
    ret


.size_error:

    mov byte [memory_map_error_code], 3
    mov word [memory_map_count], 0

    stc

    pop dx
    pop si
    pop di
    pop es
    ret


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

    ; Check ELF magic
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
    ; e_entry     = +0x18
    ; e_phoff     = +0x20
    ; e_phentsize = +0x36
    ; e_phnum     = +0x38
    ; --------------------------------------------------------

    mov r12, [rsi + 0x18]

    ; --------------------------------------------------------
    ; Track physical address range of loaded kernel segments
    ;
    ; r10 = lowest p_paddr
    ; r11 = highest p_paddr + p_memsz
    ; --------------------------------------------------------

    mov r10, 0xFFFFFFFFFFFFFFFF
    xor r11d, r11d

    mov r13, [rsi + 0x20]
    movzx r14, word [rsi + 0x36]
    movzx r15, word [rsi + 0x38]

    ; rbx = first program header
    lea rbx, [rsi + r13]

    xor r8d, r8d


; ============================================================
; ELF PROGRAM HEADER LOOP
; ============================================================

elf_program_loop:

    cmp r8, r15
    jae elf_segments_done

    ; --------------------------------------------------------
    ; ELF64 Program Header
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
    cmp eax, 1
    jne next_program_header

    ; --------------------------------------------------------
    ; Source = kernel_buffer + p_offset
    ; --------------------------------------------------------

    mov r9, [rbx + 0x08]
    lea rsi, [kernel_buffer + r9]

    ; --------------------------------------------------------
    ; Update kernel physical start
    ; --------------------------------------------------------

    mov rax, [rbx + 0x18]

    cmp rax, r10
    jae .range_start_done

    mov r10, rax

.range_start_done:

    ; --------------------------------------------------------
    ; Update kernel physical end
    ; --------------------------------------------------------

    mov rax, [rbx + 0x18]
    add rax, [rbx + 0x28]

    cmp rax, r11
    jbe .range_end_done

    mov r11, rax

.range_end_done:

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

    ; ========================================================
    ; BUILD UNBOUND BOOTINFO
    ;
    ; BootInfo @ physical address 0x70000
    ;
    ; +0x00 : u64 magic
    ; +0x08 : u32 version
    ; +0x0C : u32 size
    ; +0x10 : u8  boot_drive
    ; +0x11 : u8[7] reserved
    ; +0x18 : u64 kernel_phys_start
    ; +0x20 : u64 kernel_phys_end
    ; +0x28 : u64 memory_map_addr
    ; +0x30 : u32 memory_map_count
    ; +0x34 : u32 memory_map_entry_size
    ; +0x38 : u64 reserved
    ;
    ; Total size = 64 bytes
    ; ========================================================

    mov rdi, 0x70000

    ; magic
    mov rax, 0x554E424F554E4442
    mov [rdi + 0x00], rax

    ; version = 2
    mov dword [rdi + 0x08], 2

    ; size = 64
    mov dword [rdi + 0x0C], 64

    ; boot drive + reserved bytes
    mov qword [rdi + 0x10], 0

    mov al, [abs boot_drive]
    mov [rdi + 0x10], al

    ; kernel physical start
    mov [rdi + 0x18], r10

    ; kernel physical end
    mov [rdi + 0x20], r11

    ; memory_map_addr
    mov qword [rdi + 0x28], 0x60000

    ; memory_map_count
    movzx rax, word [abs memory_map_count]
    mov dword [rdi + 0x30], eax

    ; memory_map_entry_size
    mov dword [rdi + 0x34], 24

    ; reserved
    mov qword [rdi + 0x38], 0

    ; --------------------------------------------------------
    ; Pass BootInfo pointer to kernel
    ; --------------------------------------------------------

    mov rdi, 0x70000

    ; --------------------------------------------------------
    ; Tell us the kernel is ready
    ; --------------------------------------------------------

    mov rsi, kernel_ready_message
    call serial_write_string_64

    ; --------------------------------------------------------
    ; Jump to ELF entry
    ;
    ; R12 = ELF e_entry
    ; RDI = BootInfo
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


memory_map_error:

    mov si, memory_map_error_message
    call print_string_16

    ; Print diagnostic code: 1, 2, or 3
    mov al, [memory_map_error_code]
    add al, '0'
    call serial_write_16

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

memory_map_error_message:
    db "MEMORY MAP ERROR CODE ", 0

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

; Number of valid E820 memory-map entries
memory_map_count:
    dw 0

; E820 diagnostic error code
; 1 = Carry Flag / BIOS error
; 2 = Invalid SMAP signature
; 3 = Returned structure smaller than 20 bytes
memory_map_error_code:
    db 0


; ============================================================
; KERNEL BIOS DISK ADDRESS PACKET
;
; Read 29 sectors starting at LBA 33.
;
; Destination:
;   0x2000:0000
;   physical 0x20000
; ============================================================

align 4, db 0

kernel_dap:
    db 0x10
    db 0x00

    dw 29

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