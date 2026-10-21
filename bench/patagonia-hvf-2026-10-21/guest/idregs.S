// Minimal aarch64 guest for QEMU virt: print CurrentEL, the ID registers and SMMUv3 IDR0..5 on PL011, then PSCI SYSTEM_OFF.
.equ UART, 0x09000000
.equ SMMU, 0x09050000
.global _start
.section .text
_start:
    ldr x0, =stack_top
    mov sp, x0
    adr x0, s_el;    mrs x1, CurrentEL;         bl pr
    adr x0, s_midr;  mrs x1, MIDR_EL1;          bl pr
    adr x0, s_isar0; mrs x1, ID_AA64ISAR0_EL1;  bl pr
    adr x0, s_isar1; mrs x1, ID_AA64ISAR1_EL1;  bl pr
    adr x0, s_isar2; mrs x1, S3_0_C0_C6_2;      bl pr
    adr x0, s_pfr0;  mrs x1, ID_AA64PFR0_EL1;   bl pr
    adr x0, s_pfr1;  mrs x1, ID_AA64PFR1_EL1;   bl pr
    adr x0, s_zfr0;  mrs x1, S3_0_C0_C4_4;      bl pr
    adr x0, s_smfr0; mrs x1, S3_0_C0_C4_5;      bl pr
    adr x0, s_mmfr0; mrs x1, ID_AA64MMFR0_EL1;  bl pr
    adr x0, s_mmfr1; mrs x1, ID_AA64MMFR1_EL1;  bl pr
    adr x0, s_mmfr2; mrs x1, ID_AA64MMFR2_EL1;  bl pr
    adr x0, s_mmfr3; mrs x1, S3_0_C0_C7_3;      bl pr
    adr x0, s_dfr0;  mrs x1, ID_AA64DFR0_EL1;   bl pr
    adr x0, s_cntfrq; mrs x1, CNTFRQ_EL0;       bl pr
    ldr x2, =SMMU
    adr x0, s_sidr0; ldr w1, [x2, #0x0];  bl pr
    ldr x2, =SMMU
    adr x0, s_sidr1; ldr w1, [x2, #0x4];  bl pr
    ldr x2, =SMMU
    adr x0, s_sidr5; ldr w1, [x2, #0x14]; bl pr
    ldr x2, =SMMU
    adr x0, s_saidr; ldr w1, [x2, #0x1c]; bl pr
    adr x0, s_done;  bl puts
    ldr x0, =0x84000008
    hvc #0
1:  wfi
    b 1b

// puts(x0 = NUL-terminated string)
puts:
    ldr x9, =UART
2:  ldrb w10, [x0], #1
    cbz w10, 3f
    str w10, [x9]
    b 2b
3:  ret

// pr(x0 = label, x1 = value): "label 0x%016x\n"
pr:
    mov x19, x30
    mov x20, x1
    bl puts
    ldr x9, =UART
    mov x11, #60
4:  lsr x10, x20, x11
    and x10, x10, #0xf
    cmp x10, #10
    add x12, x10, #'0'
    add x13, x10, #('a' - 10)
    csel x10, x12, x13, lo
    str w10, [x9]
    subs x11, x11, #4
    b.ge 4b
    mov w10, #'\n'
    str w10, [x9]
    mov x30, x19
    ret

s_el:    .asciz "CurrentEL          0x"
s_midr:  .asciz "MIDR_EL1           0x"
s_isar0: .asciz "ID_AA64ISAR0_EL1   0x"
s_isar1: .asciz "ID_AA64ISAR1_EL1   0x"
s_isar2: .asciz "ID_AA64ISAR2_EL1   0x"
s_pfr0:  .asciz "ID_AA64PFR0_EL1    0x"
s_pfr1:  .asciz "ID_AA64PFR1_EL1    0x"
s_zfr0:  .asciz "ID_AA64ZFR0_EL1    0x"
s_smfr0: .asciz "ID_AA64SMFR0_EL1   0x"
s_mmfr0: .asciz "ID_AA64MMFR0_EL1   0x"
s_mmfr1: .asciz "ID_AA64MMFR1_EL1   0x"
s_mmfr2: .asciz "ID_AA64MMFR2_EL1   0x"
s_mmfr3: .asciz "ID_AA64MMFR3_EL1   0x"
s_dfr0:  .asciz "ID_AA64DFR0_EL1    0x"
s_cntfrq: .asciz "CNTFRQ_EL0         0x"
s_sidr0: .asciz "SMMU_IDR0          0x"
s_sidr1: .asciz "SMMU_IDR1          0x"
s_sidr5: .asciz "SMMU_IDR5          0x"
s_saidr: .asciz "SMMU_AIDR          0x"
s_done:  .asciz "done\n"
.balign 16
.section .bss
.space 4096
stack_top:
