/* nife kernel image layout, RISC-V (milestone 20, notes/riscv-port.md).
 *
 * THE KERNEL IS LINKED HIGH AND LOADED LOW, exactly like the aarch64 script, and for the same
 * reason: the kernel lives in the high (kernel) half of the address space, so the portable
 * "kernel = high half, user = low half" model (page_tables::Half, the user-VA gate) holds identically on
 * both architectures (DECISIONS §17). Every symbol below is a VIRTUAL address in the Sv39 high half;
 * AT() tells the loader to place the bytes at the PHYSICAL address, which is where OpenSBI loads us.
 *
 * QEMU's `virt` enters the payload at its PHYSICAL start (the Linux RISC-V boot protocol: hart id in
 * a0, DTB in a1), so boot.s runs at low physical addresses with every absolute symbol linked high.
 * It uses PC-relative addressing until it turns Sv39 on, then jumps to the high alias of itself. See
 * boot.s and notes/riscv-port.md for how it survives that.
 *
 * KERNEL_VA_BASE is the Sv39 high-half base: bits 63:38 all one (the sign extension of bit 38 = 1),
 * so it touches only the untranslated top bits. VA = PA | KERNEL_VA_BASE is exact and reversible,
 * and a kernel virtual address has the same page-table indices as its physical address, which is
 * what lets one boot table serve as both the identity map and the high-half map.
 */

ENTRY(_start)

KERNEL_VA_BASE = 0xffffffc000000000;

/* OpenSBI drops the S-mode payload here (its fw_payload address for the virt machine). */
PHYS_START = 0x80200000;

SECTIONS
{
    . = KERNEL_VA_BASE + PHYS_START;
    __image_start = .;

    .text : AT(ADDR(.text) - KERNEL_VA_BASE) ALIGN(4096) {
        __text_start = .;
        /* _start must be first: QEMU enters the payload at its physical base. KEEP() so
         * dead-code elimination cannot delete an entry point nothing calls. */
        KEEP(*(.text.boot))
        /* **The hot trap path, at one address in every build**: milestone 796 (pin the hot trap
         * path's placement). Every function a
         * system call or an IPC round trip runs (`script/fastpath-footprint`'s two closures plus its
         * entry set) carries a `.text.hot.*` section, by `#[link_section]` on the Rust and a
         * `.section` directive in the assembly. They land here, contiguous, sorted by section name
         * so the order does not follow which codegen unit rustc put each one in, and starting on a
         * 16 KiB boundary: the U74's L1i way (32 KiB, two-way), so this block's cache sets are the
         * same whatever the boot stub weighs. Nothing but the boot stub precedes it, so code added
         * anywhere else moves none of it, and while the block is under 16 KiB no two of its lines
         * share a set. On 2026-10-05 a text shift that changed no instruction moved radon's
         * one-task `null_system_call` between 110 and 118 ticks; whether this flattens that is the
         * radon sweep in design/roadmap/0796-pin-the-hot-trap-paths-placement.md.
         * `script/fastpath-footprint` fails the build when a hot symbol is outside these bounds.
         * See notes/benchmarks/kernel-footprint-and-caches.md, "The hot section". */
        . = ALIGN(16384);
        __text_hot_start = .;
        *(SORT_BY_NAME(.text.hot.*))
        __text_hot_end = .;
        /* **E3's padding sled, and only when it exists** (milestone 370, the layout control).
         * Nothing else pins where the linker drops `.text.fastpath_pad_body` among the kernel's
         * other `.text.*` inputs, and it has moved: on 2026-09-04 it landed ahead of the whole
         * trap path, and by 2026-09-19 it landed past all but 5% of the fastpath's bytes, so how
         * much of the hot path a pad perturbed was redrawn every commit. Placed first after the
         * hot section, a pad of N bytes moves the rest of the kernel text by N, which is the same
         * perturbation `NIFE_FASTPATH_SHIFT` applies, so E3's two ladders differ only in whether
         * the bytes are inside the footprint the gate counts. Since milestone 796 neither moves
         * the hot section itself: that is what a shift sweep on radon now tests. **An ordinary
         * build has no such section**, so this line matches nothing and the default link order
         * is unchanged (checked when the layout control landed: the default boot image was
         * byte-identical across it). See kernel/src/fastpath_pad.rs. */
        *(.text.fastpath_pad_body)
        *(.text .text.*)
        . = ALIGN(4096);
        __text_end = .;
    }

    .rodata : AT(ADDR(.rodata) - KERNEL_VA_BASE) ALIGN(4096) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        . = ALIGN(4096);
        __rodata_end = .;
    }

    .data : AT(ADDR(.data) - KERNEL_VA_BASE) ALIGN(4096) {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(4096);
        __data_end = .;
    }

    /* .bss occupies zero bytes in the file; boot.s zeroes it by hand once running high, 8 bytes at
     * a time, so both bounds are 8-byte aligned (the 4096 alignment more than covers it). */
    .bss : AT(ADDR(.bss) - KERNEL_VA_BASE) ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4096);
        __bss_end = .;
    }

    /* The guard page: one page, left unmapped once the fine tables are built. Until then it is
     * reserved address space beneath the stack. Same mechanism as aarch64; see notes/stack.md. */
    . = ALIGN(4096);
    __stack_guard = .;
    . += 4096;

    /* The boot stack. Grows DOWN, so __stack_top is the HIGH address, after the reserved space.
     * sp must be 16-byte aligned (the RISC-V calling convention requires it too). */
    __stack_bottom = .;
    . += 0x10000;              /* 64 KiB */
    __stack_top = .;

    /* The per-CPU secondary stacks (milestone 90), each with an unmapped page below it, exactly as
     * the boot stack has above. The slot layout (and the core count) lives in kernel/src/smp.rs;
     * this script only anchors a page-aligned region so the holes can exist, because the stacks
     * used to be in `.bss`, which is mapped wholesale. (NOLOAD): a stack needs no file bytes, and
     * nothing zeroes this region. Same shape as link-aarch64.ld; see notes/stack-high-water.md. */
    .secondary_stacks (NOLOAD) : AT(ADDR(.secondary_stacks) - KERNEL_VA_BASE) ALIGN(4096) {
        __secondary_stacks_start = .;
        *(.secondary_stacks)
        . = ALIGN(4096);
        __secondary_stacks_end = .;
    }

    /* The per-CPU interrupt stacks (milestone 124), each over its own unmapped page, laid out by
     * kernel/src/interrupt_stack.rs. A trap taken on a kernel thread's stack used to build its
     * handler frames there; these are where they go instead, so a preemption is not charged to the
     * thread it interrupted. Same shape as link-aarch64.ld; see notes/stack.md. */
    .interrupt_stacks (NOLOAD) : AT(ADDR(.interrupt_stacks) - KERNEL_VA_BASE) ALIGN(4096) {
        __interrupt_stacks_start = .;
        *(.interrupt_stacks)
        . = ALIGN(4096);
        __interrupt_stacks_end = .;
    }

    __image_end = .;
    __image_size = __image_end - __image_start;

    /* `.eh_frame*` (call-frame information; milestone: CFI in hand-written asm) is deliberately
     * not discarded any more, and placed HERE, after `__image_end`/`__image_size` are already
     * computed, rather than left to default orphan placement: see the long comment in
     * link-aarch64.ld for the full reasoning (default placement inflated that ISA's image-size
     * symbol by the CFI's own size; NOLOAD and INFO output sections each broke something else).
     * This keeps `__image_size` -- which this file's own boot.s writes into the RISC-V Image
     * header for a real board's U-Boot -- exactly what it was before this milestone.
     *
     * UNLIKE aarch64, nothing strips this section back out before QEMU boots it: this port has
     * no flat-Image / objcopy step at all (see helpers/qemu-runner-riscv64.sh's own header
     * comment), so QEMU's `-kernel` loads the ELF's PT_LOAD segments directly, CFI included. That
     * is harmless extra RAM in a VM with QEMU's usual hundreds of megabytes and is not why this
     * section moved; see notes/cfi-unwind.md's BUGS section for the honest scope of what this
     * milestone did and did not strip. */
    .eh_frame : { KEEP(*(.eh_frame)) KEEP(*(.eh_frame_hdr)) }

    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.riscv.attributes)  /* ISA-attribute records; not needed in the image */
    }
}
