bits 16
org 0x7C00

start:
    cli

    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00

    mov [boot_drive], dl

    call serial_init

    mov si, loading_message

.print:
    lodsb
    test al, al
    jz load_stage2

    call serial_write
    jmp .print


; ============================================================
; Load Stage 2
;
; Stage 2 = 32 sectors
; LBA     = 1
; Memory  = 0000:8000
; ============================================================

load_stage2:

    mov si, dap
    mov dl, [boot_drive]

    mov ah, 0x42
    int 0x13

    jc disk_error

    jmp 0x0000:0x8000


; ============================================================
; Disk error
; ============================================================

disk_error:

    mov si, error_message

.error_print:
    lodsb
    test al, al
    jz halt

    call serial_write
    jmp .error_print


halt:
    cli

.halt_loop:
    hlt
    jmp .halt_loop


; ============================================================
; COM1 initialization
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


serial_write:

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
; Messages
; ============================================================

loading_message:
    db "UNBOUND BOOTLOADER", 13, 10, 0

error_message:
    db "STAGE 2 LOAD ERROR", 13, 10, 0


; ============================================================
; Boot drive
; ============================================================

boot_drive:
    db 0


; ============================================================
; BIOS INT 13h Extensions Disk Address Packet
;
; Read 32 sectors starting at LBA 1.
;
; Destination:
;   0000:8000
;
; Total:
;   32 * 512 = 16384 bytes
; ============================================================

dap:
    db 0x10
    db 0x00

    dw 32

    dw 0x8000
    dw 0x0000

    dq 1


; ============================================================
; Boot signature
; ============================================================

times 510 - ($ - $$) db 0
dw 0xAA55
