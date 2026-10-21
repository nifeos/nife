// At guest EL2: HCR_EL2 = RW|TGE (and E2H=0), EL2 and EL1 vectors that print which caught it, eret to EL0, svc.
.equ UART, 0x09000000
.global _start
_start:
    ldr x0, =stack_top; mov sp, x0
    mrs x0, CurrentEL; cmp x0, #8; b.ne not_el2
    adr x0, vec2; msr vbar_el2, x0
    adr x0, vec1; msr vbar_el1, x0
    mov x0, #(1 << 31); orr x0, x0, #(1 << 27); msr hcr_el2, x0   // RW | TGE
    isb
    adr x0, el0; msr elr_el2, x0
    mov x0, #0x3c0; msr spsr_el2, x0                               // EL0t, DAIF masked
    ldr x0, =stack_top; msr sp_el0, x0
    adr x0, msg_go; bl puts
    eret
el0:
    svc #0
    b .
not_el2: adr x0, msg_not; bl puts; b .
puts: ldr x9, =UART
1:  ldrb w10, [x0], #1; cbz w10, 2f; str w10, [x9]; b 1b
2:  ret
caught2: adr x0, msg2; bl puts; mrs x1, esr_el2; b .
caught1: adr x0, msg1; bl puts; b .
.balign 2048
vec2: .rept 16
      b caught2
      .balign 128
      .endr
.balign 2048
vec1: .rept 16
      b caught1
      .balign 128
      .endr
msg_go: .asciz "eret to EL0 with HCR_EL2.TGE=1, then svc\n"
msg2: .asciz "svc from EL0 taken to EL2 (TGE honored)\n"
msg1: .asciz "svc from EL0 taken to EL1 (TGE NOT honored)\n"
msg_not: .asciz "not at EL2\n"
.section .bss
.balign 16
.space 4096
stack_top:
