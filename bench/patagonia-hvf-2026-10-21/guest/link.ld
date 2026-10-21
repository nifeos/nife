ENTRY(_start)
SECTIONS { . = 0x40080000; .text : { *(.text) *(.rodata*) } .data : { *(.data) } .bss : { *(.bss) } }
