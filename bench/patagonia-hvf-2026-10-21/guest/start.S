.global _start
_start: ldr x0, =stack_top; mov sp, x0; bl main; 1: wfi; b 1b
.section .bss
.balign 16
.space 16384
stack_top:
