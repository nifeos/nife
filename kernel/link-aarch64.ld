/* nife kernel image layout.
 *
 * QEMU's `virt` machine puts RAM at 0x4000_0000. Given a flat arm64 Image, QEMU follows
 * the Linux boot protocol: it loads us at RAM_base + text_offset, hands us a device tree
 * pointer in x0, and lets go. See notes/boot-protocol.md and notes/linker-scripts.md.
 *
 * Every section boundary here is exported, because milestone 4 maps each one with
 * DIFFERENT permissions and the linker is the only thing that knows where they landed.
 * They are all page-aligned for the same reason: MMU permissions are per-page, so a
 * section sharing a page with another section cannot have its own permissions.
 */

ENTRY(_start)

/* THE KERNEL IS LINKED HIGH AND LOADED LOW.
 *
 * Every symbol below is a VIRTUAL address in the top half. But `AT()` tells the loader to
 * put the bytes at the PHYSICAL address, which is where QEMU actually loads a flat Image.
 *
 * So the ELF carries both: p_vaddr = 0xffff_0000_4008_0000, p_paddr = 0x4008_0000. objcopy
 * lays the flat binary out by LMA, which is what QEMU reads.
 *
 * Which means the code in boot.s runs at a physical address while every absolute address
 * baked into it is virtual. See boot.s and notes/higher-half.md for how it survives that.
 *
 * The base is 0xffff_0000_0000_0000, and the choice is not arbitrary: it touches only bits
 * 63:48, which are NOT TRANSLATED. So a kernel virtual address and its physical address have
 * identical page-table indices, VA = PA + KERNEL_VA_BASE is exact, and the boot map can be a
 * single table serving as both the identity map and the high-half map. */
KERNEL_VA_BASE = 0xffff000000000000;

/* 512 KiB into RAM. NOT free to change: it must match `text_offset` in the arm64 Image
 * header (image_header.s), which is what tells QEMU where to load us. */
PHYS_START = 0x40080000;

SECTIONS
{
    . = KERNEL_VA_BASE + PHYS_START;
    __image_start = .;

    .text : AT(ADDR(.text) - KERNEL_VA_BASE) ALIGN(4096) {
        __text_start = .;
        /* The Image header MUST be at byte 0 of the image, and _start with it. KEEP()
         * because nothing *calls* it, so dead-code elimination would delete the entry
         * point. */
        KEEP(*(.text.header))
        KEEP(*(.text.boot))
        /* **The hot trap path, at one address in every build**: milestone 796 (pin the hot trap
         * path's placement). link-riscv64.ld has the reasoning. 16 KiB is a multiple of every L1i
         * way this ISA's targets have, so the block's cache sets do not depend on what the image
         * header and boot stub weigh. `script/fastpath-footprint` fails the build when a hot
         * symbol is outside these bounds. */
        . = ALIGN(16384);
        __text_hot_start = .;
        /* The vector table first, because it must start on 2 KiB and this boundary is one. */
        *(.text.hot.exception_vectors)
        *(SORT_BY_NAME(.text.hot.*))
        /* compiler_builtins' memcpy: the IPC closure calls it on this ISA (the other two inline
         * their copies), and it carries no attribute we can add. */
        *(.text.memcpy)
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

    /* .bss occupies zero bytes in the file, so nothing loads it and nothing zeroes it.
     * boot.s does that by hand, 8 bytes at a time, which is why both bounds must be
     * 8-byte aligned. */
    .bss : AT(ADDR(.bss) - KERNEL_VA_BASE) ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4096);
        __bss_end = .;
    }

    /* THE GUARD PAGE.
     *
     * One page, deliberately LEFT UNMAPPED when we build the page tables. The stack grows
     * down into it, and the MMU faults on the very first byte written past the end.
     *
     * This is the real fix for the milestone 3 incident, where a 16 KiB array on the stack
     * walked straight through .bss, .data, and .text, and the kernel executed its own
     * overwritten code. It hung for 150 seconds with no output and two of the three
     * diagnoses along the way were wrong. See notes/stack.md.
     *
     * The canary in kernel/src/stack.rs stays, but it is a mitigation: it detects damage
     * after the fact, and only if the overflow happened to write over the canary words. A
     * guard page PREVENTS the damage, precisely, at the first byte, for free at runtime.
     * That is the difference between an afternoon and an instant, legible fault. */
    . = ALIGN(4096);
    __stack_guard = .;
    . += 4096;

    /* The boot stack. Grows DOWN, so __stack_top is the HIGH address and comes AFTER the
     * reserved space. sp must be 16-byte aligned (notes/stack.md). */
    __stack_bottom = .;
    . += 0x10000;              /* 64 KiB */
    __stack_top = .;

    /* THE PER-CPU SECONDARY STACKS (milestone 90).
     *
     * Same mechanism as the boot stack's guard page above, one per core. The stacks used to be a
     * plain array in `.bss`, which `map_everything` maps wholesale, so there was nowhere to put a
     * hole: a secondary that ran deep did not fault, it scribbled on whatever `.bss` sat below.
     *
     * The slots and their guards are laid out by `SECONDARY_STACKS` in kernel/src/smp.rs, which is
     * where MAX_CPUS lives; this script only anchors the region, page-aligned, so each slot's
     * bottom page can be left unmapped. Deliberately NOT a second copy of the core count.
     *
     * (NOLOAD) because these are stacks: nothing needs to be in the file for them, and without it
     * a zero-initialized static in an explicitly named section becomes PROGBITS and pads the flat
     * binary with a quarter megabyte of zeroes. Nothing zeroes this region either (boot.s zeroes
     * only `__bss_start`..`__bss_end`), which is fine for a stack and is why it can be NOLOAD.
     *
     * It is inside `__image_size` below, so the bootloader knows the memory is ours. */
    .secondary_stacks (NOLOAD) : AT(ADDR(.secondary_stacks) - KERNEL_VA_BASE) ALIGN(4096) {
        __secondary_stacks_start = .;
        *(.secondary_stacks)
        . = ALIGN(4096);
        __secondary_stacks_end = .;
    }

    /* THE PER-CPU INTERRUPT STACKS (milestone 124).
     *
     * One slot per core, guard page first, exactly like the secondary stacks above and for exactly
     * the same reason: the layout lives in kernel/src/interrupt_stack.rs, and this script only
     * anchors a page-aligned region so each slot's bottom page can be left out of the map.
     *
     * What they are for is different, though. A trap taken while a kernel thread is running builds
     * its handler frames on whichever stack it interrupted, so a preemption arriving at a thread's
     * deepest instant was billed to that thread. These stacks are where those frames go instead.
     * See notes/stack.md. */
    .interrupt_stacks (NOLOAD) : AT(ADDR(.interrupt_stacks) - KERNEL_VA_BASE) ALIGN(4096) {
        __interrupt_stacks_start = .;
        *(.interrupt_stacks)
        . = ALIGN(4096);
        __interrupt_stacks_end = .;
    }

    /* `image_size` for the arm64 Image header.
     *
     * Runs all the way to __stack_top, not just to the end of .data. The flat binary stops
     * after .data (since .bss occupies no file bytes), but this number tells the bootloader
     * how much MEMORY we occupy. Understate it and QEMU will place the device tree on top
     * of our .bss or our stack, and we corrupt it on the first push. */
    __image_end = .;
    __image_size = __image_end - __image_start;

    /* `.eh_frame*` (call-frame information; milestone: CFI in hand-written asm) is deliberately
     * not discarded any more, so it survives in the ELF for a debugger reading it from disk
     * (`gdb <elf>`, `script/gdb`). "We never unwind" stops being true with this milestone: a
     * debugger can now walk a frame in hand-written assembly, and whatever the compiler already
     * emits for ordinary Rust functions besides.
     *
     * Three things were tried for WHERE it lives, and this is the one that worked; the other two
     * are worth recording so nobody reaches for them again.
     *
     *   - Left unmentioned (default orphan placement): LLD keeps it ALLOC (the compiler marks it
     *     so, unlike `.debug_info`/`.debug_line`, which are never ALLOC and so need no script
     *     attention) and slots it in with the other read-only data, BETWEEN `.rodata` and `.data`
     *     -- i.e. before `__image_end` above, which is exactly the number the arm64 Image header
     *     and every bootloader trust. Measured: `__image_size` grew by the CFI's own size (about
     *     100 KiB), for a debugging convenience that has nothing to do with how much RAM the
     *     image needs. Wrong place, not wrong content.
     *   - An explicit `(NOLOAD)` output section moved it out of the way but also turned it into
     *     SHT_NOBITS, the same representation `.bss` uses: LLD dropped the actual bytes, and the
     *     CFI content was gone.
     *   - An explicit `(INFO)` output section kept the bytes and dropped ALLOC, but also let LLD
     *     place it far from `.text` (multiple megabytes away), which overflowed a 32-bit
     *     PC-relative relocation elsewhere in the image outright.
     *
     * What's below is the same explicit-output-section idea as those last two, corrected: an
     * ORDINARY output section (no NOLOAD, no INFO), placed in the script text AFTER `__image_end`
     * is computed rather than left to the orphan-placement heuristic. It stays ALLOC (so its
     * bytes survive) and keeps the linker's normal sequential layout (so it lands right after
     * `.bss`/the per-core stacks, close enough to `.text` that no relocation overflows), while
     * costing `__image_size` nothing, because that symbol was already assigned before this
     * section exists. `script/image` and every QEMU runner still strip it from the flat binary
     * with `llvm-objcopy --remove-section` (Image size on disk, not debuggability); ordinary
     * bytes trailing at the end of an ALLOC layout is the shape that flag was built for. The ELF
     * `gdb <elf>` reads directly keeps the real content either way. See notes/cfi-unwind.md. */
    .eh_frame : { KEEP(*(.eh_frame)) KEEP(*(.eh_frame_hdr)) }

    /DISCARD/ : {
        *(.comment)
        *(.note*)
    }
}
