global _start
extern long_mode_start

section .text_init
bits 32
_start:
    mov esp, stack_top
    mov edi, ebx
    call check_multiboot
    call check_cpuid
    call check_long_mode

    call set_up_page_tables
    call enable_paging

    lgdt [gdt64.pointer]

    jmp gdt64.code:long_mode_start ; JUMP

    mov al, "_"
    call error

    hlt

enable_paging:
    ; load P4 to cr3 register (cpu uses this to access the P4 table)
    mov eax, p4_table
    mov cr3, eax

    ; enable PAE-flag in cr4 (Physical Address Extension)
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; set the long mode bit in the EFER MSR (model specific register)
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ; enable paging in the cr0 register
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    ret

set_up_page_tables:
    ; Recursive P4 entry 511
    mov eax, p4_table
    or eax, 0b11;
    mov [p4_table + 8 * 511], eax

    ; map first P4 entry to P3 table
    mov eax, p3_table
    or eax, 0b11 ; present + writable
    mov [p4_table], eax

    ; map first P3 entry to P2 table
    mov eax, p2_table
    or eax, 0b11 ; present + writable
    mov [p3_table], eax

   ; map each P2 entry to a huge 2MiB page
   mov ecx, 0         ; counter variable

.map_p2_table:
   ; map ecx-th P2 entry to a huge page that starts at address 2MiB*ecx
   mov eax, 0x200000  ; 2MiB
   mul ecx            ; start address of ecx-th page
   or eax, 0b10000011 ; present + writable + huge
   mov [p2_table + ecx * 8], eax ; map ecx-th entry

   inc ecx            ; increase counter
   cmp ecx, 512       ; if counter == 512, the whole P2 table is mapped
   jne .map_p2_table  ; else map the next entry

   ret

check_long_mode:
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    mov al, "l"
    jb no_long_mode ;; CPU Version below this 0x8000001 is TOO OLD

    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    mov al, "L"
    jz no_long_mode
    ret

no_long_mode:
    jmp error

;; Write ERR: with an ascii char stored in AL
error:
    mov dword [0xb8000], 0x4f524f45
    mov dword [0xb8004], 0x4f3a4f52
    mov dword [0xb8008], 0x4f204f20
    mov byte  [0xb800a], al
    hlt


check_cpuid:
    ; Check if CPUID is supported by attempting to flip the ID bit (bit 21)
    ; in the FLAGS register. If we can flip it, CPUID is available.

    ; Copy FLAGS in to EAX via stack
    pushfd
    pop eax

    ; Copy to ECX as well for comparing later on
    mov ecx, eax

    ; Flip the ID bit
    xor eax, 1 << 21

    ; Copy EAX to FLAGS via the stack
    push eax
    popfd

    ; Copy FLAGS back to EAX (with the flipped bit if CPUID is supported)
    pushfd
    pop eax

    ; Restore FLAGS from the old version stored in ECX (i.e. flipping the
    ; ID bit back if it was ever flipped).
    push ecx
    popfd

    ; Compare EAX and ECX. If they are equal then that means the bit
    ; wasn't flipped, and CPUID isn't supported.
    cmp eax, ecx
    je .no_cpuid
    ret
.no_cpuid:
    mov al, "I" ;; ERROR: CPUID Not Supported
    jmp error

check_multiboot:
    cmp eax, 0x36d76289
    jne .no_multiboot
    ret
.no_multiboot:
    mov al, "M" ;; ERROR: Not Loaded by Multiboot
    jmp error

section .bss
align 4096
p4_table:
    resb 4096
p3_table:
    resb 4096
p2_table:
    resb 4096
align 16 ; stack needs to be 16 aligned
stack_bottom:
    resb 4096 * 16
stack_top:


section .rodata
gdt64:
    dq 0 ; zero entry
.code: equ $ - gdt64 ; new
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
.pointer:
    dw $ - gdt64 - 1
    dq gdt64