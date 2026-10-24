ENTRY(_start)
SECTIONS
{
    . = 0x80000000;
    .text : { KEEP(*(.text.boot)) *(.text .text.*) }
    .rodata : ALIGN(16) { *(.rodata .rodata.*) *(.srodata .srodata.*) }
    .data : ALIGN(4096) { *(.data .data.*) *(.sdata .sdata.*) }
    .bss (NOLOAD) : ALIGN(4096) { *(.bss .bss.*) *(.sbss .sbss.*) *(COMMON) }
    . = ALIGN(16);
    . += 0x40000;
    __stack_top = .;
    /DISCARD/ : { *(.eh_frame*) *(.comment) }
}
