/* nife kernel image layout, x86_64 (milestone 161, notes/x86-port.md).
 *
 * THE KERNEL IS LINKED HIGH AND LOADED LOW, the same shape as link-aarch64.ld and
 * link-riscv64.ld, for the same reason: the kernel lives in the high half of the address space, so
 * the portable "kernel = high half, user = low half" model (paging::Half, the user-VA gate) holds
 * identically on all three architectures (DECISIONS §17).
 *
 * WHAT IS DIFFERENT HERE, AND IT IS THE ONE STRUCTURAL DIVERGENCE OF THE THREE.
 *
 * The other two ISAs enter the kernel in the mode it runs in, with paging off but the register
 * width already correct, so their whole image can be linked high and the boot code reaches its own
 * symbols PC-relatively until it turns the MMU on. x86_64 cannot do that: a multiboot kernel is
 * entered in 32-BIT protected mode, and a 32-bit instruction stream cannot even name a 64-bit
 * address, let alone load one. So the image has TWO parts:
 *
 *   1. `.boot`, linked at its PHYSICAL address (VA == PA, low), holding the 32-bit trampoline, the
 *      multiboot header, the boot GDT and the boot page tables. Absolute symbol references inside
 *      it are 32-bit and correct because they are physical.
 *   2. Everything else, linked HIGH at KERNEL_VA_BASE + its physical address, with AT() telling the
 *      loader where the bytes go. The trampoline builds a page table that maps the kernel both
 *      identity (so `mov cr0` does not vanish mid-instruction-stream) and high, then far-jumps into
 *      64-bit mode and, from there, to the high alias of itself.
 *
 * KERNEL_VA_BASE IS NOT A FREE CHOICE. `x86_64-unknown-none` has `code-model: kernel`, which
 * promises LLVM that every symbol is in the top 2 GiB of the address space so it may use the
 * sign-extended 32-bit relocations that make kernel code compact. 0xffffffff80000000 is that model's
 * base, exactly as it is for Linux. Moving it breaks every relocation in the image, silently, in a
 * way that looks like random corruption. See notes/x86-port/boot.md.
 *
 * PHYS_START WAS 1 MiB AND IS NOW 32 MiB, and the reason is firmware rather than specification
 * (milestone 195). 1 MiB is the LOWEST address a multiboot image may be loaded at, because the
 * first megabyte is real-mode legacy (the IVT, the BIOS data area, the VGA window, option ROMs);
 * it was never the only one. Under QEMU's PVH loader nothing else is in low memory, so the lowest
 * legal address looked free. Under real firmware it is not: OVMF keeps ACPI NVS at 8 MiB and its
 * own boot-services allocations from 9 MiB to 23.5 MiB, so an image at 1 MiB has under 7 MiB of
 * headroom before `AllocatePages(AllocateAddress)` refuses the whole span. The kernel's TEST build
 * reaches 10 MiB and hit exactly that: it would not load under OVMF at all, while the tour build
 * fitted with room the next test module would have eaten.
 *
 * 32 MiB clears every low reservation OVMF makes, is 2 MiB-aligned (which is the page size boot.s
 * builds its identity map out of), and is still nameable by the 32-bit instruction stream `_start`
 * is entered in. Linux solves the same problem the same way and its own default is 16 MiB
 * (CONFIG_PHYSICAL_START); 16 would not clear this firmware.
 *
 * THIS IS NOT A FIX, IT IS A LARGER GAP, and the difference matters on hardware nobody can attach a
 * debugger to. The image is still placed at one address chosen at link time, so a firmware that
 * wants 32 MiB refuses the boot exactly as OVMF refused 1 MiB. What makes that survivable is that
 * the loader now NAMES what is in the way (uefi_loader::say_conflict) instead of reporting a bare
 * Load Error. The real answer is a physically relocatable image, which is a milestone rather than a
 * constant: `.boot` holds 32-bit absolute references to its own labels, so moving it means making
 * that code position-independent.
 */

ENTRY(_start)

KERNEL_VA_BASE = 0xffffffff80000000;

/* The lowest address a multiboot image may be loaded at. QEMU's `-kernel` ELF loader honours the
 * program headers' p_paddr, so this is where the bytes actually land. */
PHYS_START = 0x02000000;

/* Where the AP real-mode trampoline executes from (milestone 161's SMP item): a STARTUP IPI names
 * a physical PAGE below 1 MiB (`vector << 12`), so this has to be low, page-aligned, and fixed at
 * link time so `secondary_boot`'s own address IS that physical page with no conversion needed. See
 * `.ap_trampoline` below and arch/x86_64/boot.s's header comment on `secondary_boot`.
 *
 * 0x8000 (32 KiB): conventional real-mode-trampoline territory (the same page Linux's own uses),
 * clear of the IVT (0..0x400), the BIOS Data Area (0x400..0x500), and every EBDA this kernel has
 * measured on QEMU (typically ~0x9fc00). The frame allocator never hands out anything below 1 MiB
 * regardless (`machine::LOW_MEGABYTE`), so this page is never anyone else's to collide with. */
AP_TRAMPOLINE_PHYS = 0x8000;

SECTIONS
{
    . = PHYS_START;
    __boot_start = .;

    /* The PVH entry note, and the reason it gets an output section to itself: QEMU finds the entry
     * point by walking PT_NOTE program headers, and lld only emits a PT_NOTE for an output section
     * whose type is SHT_NOTE. Folding this into `.boot` below would make the output section
     * PROGBITS, produce no PT_NOTE, and leave QEMU reporting nothing more informative than a guest
     * that never runs. KEEP() because nothing references it. See arch/x86_64/boot.s. */
    .note.Xen : ALIGN(4) {
        KEEP(*(.note.Xen))
    }

    /* The 32-bit world, linked LOW, and SPLIT IN TWO ON PURPOSE (milestone 208).
     *
     * These were one output section, `.boot`, holding both the trampoline's code and its data. A
     * program header's permissions are the union of what its output section holds, so one section
     * carrying both produced ONE `PT_LOAD` marked `RWX` at 0x101000: writable and executable
     * memory, in the boot path of a kernel whose whole claim is about what a confined component can
     * reach. `crates/elf::Elf::parse` refuses such an image outright
     * (`Error::WritableAndExecutable`) and `paging::Flags` has no constructor that can build such a
     * page, so the kernel's own image was the one thing in the tree breaking the rule the tree
     * enforces everywhere else. Nobody noticed because nothing parses the kernel's own image; that
     * is what `script/image-permissions` is for.
     *
     * Each half is page-aligned because a permission boundary IS a page boundary: two output
     * sections sharing a page would land in one program header again and re-merge the permissions
     * the split exists to separate. The cost is one 4 KiB page of image, which is why they were
     * folded together in the first place.
     *
     * `.boot_data` ships `RW`, and where that `W` comes from is worth knowing before anyone tries
     * to make it `R` only. boot.s declares the input `.section .data.boot, "a"`, with no `w`, and
     * the contents really are read-only (the boot GDT and its `lgdt` pointer, never written). The
     * assembler overrides it: LLVM gives a section whose name begins with `.data` its default ELF
     * flags, `SHF_ALLOC | SHF_WRITE`, and the object file measurably carries `WA`. That `W` is the
     * one that was being unioned with `.text.boot`'s `X`. Renaming the input section would make
     * this half read-only too, which is a boot.s change rather than a linker-script one and is not
     * what this milestone is; W^X is satisfied either way.
     *
     * `.boot_data` and `.boot_scratch` below share one `RW` program header, because they are
     * adjacent and their permissions match. That is the mechanism working rather than a leak: the
     * merge a program header performs is over permissions, so sections that agree may share one and
     * sections that disagree may not. */
    .boot_text : ALIGN(4096) {
        KEEP(*(.text.boot))
        . = ALIGN(4096);
    }

    .boot_data : ALIGN(4096) {
        KEEP(*(.data.boot))
        . = ALIGN(4096);
    }

    /* The boot page tables and the 32-bit stack: reserved address space rather than image bytes,
     * because 28 KiB of zeros in the ELF would be 28 KiB QEMU copies for nothing. NOLOAD means the
     * loader is told nothing about them, so boot.s zeroes them itself before filling them in
     * (`rep stosd`); do not add anything here that expects to arrive pre-zeroed from the loader. */
    .boot_scratch (NOLOAD) : ALIGN(4096) {
        __boot_scratch_start = .;
        *(.bss.boot)
        . = ALIGN(4096);
        __boot_scratch_end = .;
    }

    __boot_end = .;

    /* From here on, virtual addresses are high-half aliases of the physical addresses the loader
     * writes to (AT()). VA = PA + KERNEL_VA_BASE is exact and reversible, which is what
     * `mmu::phys_to_virt`/`virt_to_phys` are. */
    . = __boot_end + KERNEL_VA_BASE;

    /* The image bounds `memory::initialize` reads, as HIGH virtual addresses (it passes them straight to
     * virt_to_phys). The image really begins at PHYS_START, in `.boot`, so this names that page's
     * high alias even though nothing executes there. */
    __image_start = KERNEL_VA_BASE + PHYS_START;

    .text : AT(ADDR(.text) - KERNEL_VA_BASE) ALIGN(4096) {
        __text_start = .;
        /* **The hot trap path, at one address in every build**: milestone 796 (pin the hot trap
         * path's placement). link-riscv64.ld has the reasoning. Nothing in `.text` precedes it,
         * and 16 KiB is a multiple of a common x86 L1i way (32 KiB, eight-way, so 4 KiB), so the
         * block's cache sets do not depend on what the boot sections weigh.
         * `script/fastpath-footprint` fails the build when a hot symbol is outside these bounds. */
        . = ALIGN(16384);
        __text_hot_start = .;
        *(SORT_BY_NAME(.text.hot.*))
        __text_hot_end = .;
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

    /* The AP real-mode trampoline (milestone 161's SMP item): linked at the fixed LOW physical
     * address it is copied to and executed from (AP_TRAMPOLINE_PHYS, above), rather than at the
     * high address everything else in this file uses. Because its virtual address (VMA) already
     * equals its final execution address, every absolute address the assembler bakes into it (its
     * own tiny GDT pointer, both far-jump targets) is already correct the moment
     * `arch::x86_64::ap_boot::prepare` finishes copying it there; nothing needs runtime patching
     * beyond the one genuinely dynamic value, the target core's stack top
     * (`ap_trampoline_stack_top`, written by that same call).
     *
     * ITS LOAD ADDRESS (LMA) IS DELIBERATELY NOT THE NEXT FREE LOW-PHYSICAL BYTE. `.boot_scratch`
     * and `.secondary_stacks` both look like free space below the high image but are runtime-
     * mutated (the first zeroed and filled with page-table entries by boot.s before any Rust runs,
     * the second written into as threads use their stacks), so a trampoline whose *source* bytes
     * landed there would read back corrupted the moment `prepare` tries to copy them, on every boot
     * after the first `CPU_ON`-equivalent call. Placed instead right after `.rodata`: ordinary,
     * file-backed, and never written by anything at runtime. */
    __ap_trampoline_lma = ADDR(.rodata) - KERNEL_VA_BASE + SIZEOF(.rodata);
    __resume_after_ap_trampoline = .;
    .ap_trampoline AP_TRAMPOLINE_PHYS : AT(__ap_trampoline_lma) ALIGN(16) {
        __ap_trampoline_start = .;
        KEEP(*(.ap_trampoline))
        . = ALIGN(16);
        __ap_trampoline_end = .;
    }
    /* Resume the high image's own layout, ADVANCED by the trampoline's own size: `.ap_trampoline`'s
     * explicit low VMA above left `.` at AP_TRAMPOLINE_PHYS-ish, which is not where `.data` belongs,
     * so it is not simply restored to where `.rodata` left it either. `.data`'s LMA is DERIVED from
     * its VMA (`AT(ADDR(.data) - KERNEL_VA_BASE)`, a fixed subtraction), so the only way to make its
     * LMA start after the trampoline's own LMA range, rather than overlapping it, is to advance the
     * VMA `.` by the same amount: both march forward together. */
    . = __resume_after_ap_trampoline + SIZEOF(.ap_trampoline);

    .data : AT(ADDR(.data) - KERNEL_VA_BASE) ALIGN(4096) {
        __data_start = .;
        *(.data .data.*)
        /* Named explicitly rather than left as orphans, and this is a trap specific to this script
         * rather than a tidiness preference. lld places an output section it was not told about
         * immediately after the last section with compatible flags, and here that is `.boot`, at a
         * LOW address 4 GiB below everything the kernel is linked for. A `.got` entry landing there
         * still resolves (the identity map covers it) and would hide the fact that a relocation the
         * kernel code model promised would not exist does; naming them means they land high with
         * everything else, where a stray one shows up as a build-visible size instead. */
        *(.got .got.plt)
        *(.data.rel.ro .data.rel.ro.*)
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
     * reserved address space beneath the stack. Same mechanism as the other two; see notes/stack.md. */
    . = ALIGN(4096);
    __stack_guard = .;
    . += 4096;

    /* The boot stack. Grows DOWN, so __stack_top is the HIGH address, after the reserved space.
     * The System V AMD64 ABI wants rsp 16-byte aligned at a call boundary, which a page-aligned
     * top satisfies. */
    __stack_bottom = .;
    . += 0x10000;              /* 64 KiB */
    __stack_top = .;

    /* The per-CPU secondary stacks (milestone 90), each with an unmapped page below it. Same shape
     * as the other two scripts; the slot layout lives in kernel/src/smp.rs. */
    .secondary_stacks (NOLOAD) : AT(ADDR(.secondary_stacks) - KERNEL_VA_BASE) ALIGN(4096) {
        __secondary_stacks_start = .;
        *(.secondary_stacks)
        . = ALIGN(4096);
        __secondary_stacks_end = .;
    }

    /* The per-CPU interrupt stacks: milestone 124 (a thread is born where it lives), laid out by
     * kernel/src/interrupt_stack.rs. The handler body of a trap from ring 0 moves here in
     * software; the CPU never switches to them, and neither `TSS.rsp0` (the running thread's own
     * kernel stack, written by trap.s on every return to ring 3) nor any IST entry points here.
     * See notes/stack.md. */
    .interrupt_stacks (NOLOAD) : AT(ADDR(.interrupt_stacks) - KERNEL_VA_BASE) ALIGN(4096) {
        __interrupt_stacks_start = .;
        *(.interrupt_stacks)
        . = ALIGN(4096);
        __interrupt_stacks_end = .;
    }

    /* The stacks the TSS's IST entries point at: milestone 915 (NMI, double fault and machine
     * check run on their own stacks), provisional. One each per core for the NMI, the double
     * fault and the machine check, each over an unmapped guard page, laid out by
     * kernel/src/arch/x86_64/ist.rs. The other two architectures have no equivalent. */
    .ist_stacks (NOLOAD) : AT(ADDR(.ist_stacks) - KERNEL_VA_BASE) ALIGN(4096) {
        __ist_stacks_start = .;
        *(.ist_stacks)
        . = ALIGN(4096);
        __ist_stacks_end = .;
    }

    __image_end = .;
    __image_size = __image_end - __image_start;

    /* `.eh_frame*` (call-frame information; milestone: CFI in hand-written asm) is deliberately
     * not discarded any more, and placed HERE, after `__image_end`/`__image_size` are already
     * computed, rather than left to default orphan placement: see the long comment in
     * link-aarch64.ld for the full reasoning (default placement inflated that ISA's image-size
     * symbol by the CFI's own size; NOLOAD and INFO output sections each broke something else).
     * `__image_size` has no reader on this ISA yet (there is no board port), but keeping the same
     * shape across all three link scripts is one fewer thing to re-derive later.
     *
     * UNLIKE aarch64, nothing strips this section back out before QEMU boots it: this port has no
     * flat-Image / objcopy step at all (PVH loads the ELF's PT_LOAD segments directly; see
     * helpers/qemu-runner-x86_64.sh's own header comment), so the CFI rides along into guest RAM.
     * That is harmless extra RAM in a VM with QEMU's usual hundreds of megabytes and is not why
     * this section moved; see notes/cfi-unwind.md's BUGS section for the honest scope of what
     * this milestone did and did not strip. */
    .eh_frame : { KEEP(*(.eh_frame)) KEEP(*(.eh_frame_hdr)) }

    /DISCARD/ : {
        *(.comment)
        *(.note*)
    }
}
