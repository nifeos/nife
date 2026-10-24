ENTRY(_start)
SECTIONS
{
    . = 0x40080000;
    .text : { KEEP(*(.text.boot)) *(.text .text.*) }
    .rodata : ALIGN(16) { *(.rodata .rodata.*) }
    .data : ALIGN(4096) { *(.data.pagetable) *(.data .data.*) }
    .bss (NOLOAD) : ALIGN(4096) { *(.bss .bss.*) *(COMMON) }
    . = ALIGN(16);
    . += 0x40000;
    __stack_top = .;
    /DISCARD/ : { *(.eh_frame*) *(.comment) }
}
