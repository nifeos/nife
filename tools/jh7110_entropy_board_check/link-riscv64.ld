/* The payload runs where the kernel runs: booti relocates an image to RAM base plus the header's
 * text_offset, 0x4000_0000 + 0x4020_0000 on the VisionFive 2, and OpenSBI's default payload address
 * on QEMU virt is the same 0x8020_0000 (see kernel/src/arch/riscv64/boot.s). */
ENTRY(_start)
SECTIONS
{
    . = 0x80200000;
    __image_start = .;
    .text : { KEEP(*(.text.boot)) *(.text .text.*) }
    .rodata : ALIGN(16) { *(.rodata .rodata.*) *(.srodata .srodata.*) }
    .data : ALIGN(16) { *(.data .data.*) *(.sdata .sdata.*) }
    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        *(.bss .bss.*) *(.sbss .sbss.*) *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    }
    . = ALIGN(16);
    . += 0x20000;
    __stack_top = .;
    __image_size = . - __image_start;
    /DISCARD/ : { *(.eh_frame*) *(.comment) }
}
