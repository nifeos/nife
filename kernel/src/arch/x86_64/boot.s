# The kernel's real entry on x86_64, and the 32-bit -> 64-bit -> higher-half transition.
#
# This file is written in INTEL syntax, because that is what Rust's `global_asm!` selects by default
# on x86 and the alternative is an `options(att_syntax)` this kernel would then have to remember
# everywhere. The other two architectures' boot.s have no such choice to make.
#
# HOW WE GET HERE, AND WHY IT IS NOT MULTIBOOT. The obvious answer for `-kernel` on x86 is
# Multiboot 1, and it does not work for a 64-bit kernel: QEMU's loader recognises the header, then
# refuses the image with "Cannot load x86-64 image, give a 32bit one", because the Multiboot 1
# specification says the OS image is ELF32 and QEMU enforces it. Multiboot 2 lifts that restriction
# and QEMU 11 does not implement Multiboot 2 (its x86 loader is one file, hw/i386/multiboot.c, and
# it is version 1 only; checked by inspecting the binary for a version-2 code path, 2026-08-23).
# The header is fatal rather than ignored, so an image cannot carry both and fall back.
#
# So this kernel boots by **PVH**, the direct-boot protocol Xen defined and QEMU, Firecracker and
# cloud-hypervisor all implement. Its entire interface is one ELF note: name "Xen", type
# XEN_ELFNOTE_PHYS32_ENTRY (18), whose descriptor is the physical address to enter at. QEMU finds it
# by walking the PT_NOTE program headers, loads the ELF by p_paddr as usual, and jumps to that
# address **in 32-bit protected mode with paging off**: eax = 0x336EC578, ebx = the physical address
# of a `struct hvm_start_info`, CS/DS flat 4 GiB, interrupts disabled, no valid stack.
#
# PVH is a better fit than Multiboot for this kernel quite apart from the ELF64 question, and the
# reason is what rides in ebx. `hvm_start_info` carries the memory map **and the RSDP address**,
# which is the root of the ACPI tables: on the other two architectures the equivalent single pointer
# is the device tree, and ACPI is what x86 has instead. One pointer in, everything discoverable from
# it, exactly the shape `kernel_main(dtb)` already has.
#
# WHAT THIS DOES NOT BOOT. PVH is a hypervisor protocol. A real machine's firmware does not speak it,
# so milestone 87's OptiPlex will need either a UEFI stub or GRUB's Multiboot (which enters in the
# same 32-bit protected mode this trampoline already expects, so the trampoline itself carries over
# unchanged; only the header and the ebx contract differ). See notes/x86-port.md's BUGS.
#
# That entry mode is the whole reason this file is three times the size of its RISC-V twin. RISC-V
# and aarch64 are entered in the width they run in, so their boot code can be linked high and use
# PC-relative addressing until the MMU comes on. A 32-bit instruction stream cannot name a 64-bit
# address at all, so the trampoline below is linked LOW (VA == PA, the `.boot` output section in
# link-x86_64.ld) and only the second half of this file lives at the high addresses the rest of the
# kernel is linked for.
#
# THE ORDER, AND WHY EACH STEP CANNOT MOVE:
#
#   1. Zero the boot page tables. They are NOLOAD, so nothing has zeroed them, and PML4/PDPT entries
#      we do not write must read as not-present or the CPU walks into garbage.
#   2. Fill them: 4 GiB identity-mapped with 2 MiB pages, the first GiB aliased at KERNEL_VA_BASE
#      (where the image is linked), and the same 4 GiB aliased again at DIRECT_MAP_BASE (where
#      `mmu::phys_to_virt` points). The identity half is not a convenience, it is what keeps
#      `mov cr0, eax` from being the last instruction ever fetched: paging comes on between one
#      instruction and the next, and the next one's address is still low. The direct map is not a
#      convenience either; see step 2e for why it has to exist before Rust runs.
#   3. CR4.PAE, then CR3, then EFER.LME, then CR0.PG. The CPU only enters long mode when PG is set
#      *while* LME is set and PAE is on; any other order either faults or silently enters 32-bit
#      paging, which then walks our 4-level table as a 3-level one and triple-faults.
#   4. A far jump through a 64-bit code descriptor. Setting LME+PG puts the CPU in COMPATIBILITY
#      mode, not 64-bit mode; the L bit in the code segment is what actually widens it, and the only
#      way to load CS is a far transfer.
#   5. Only then jump to the high alias, and only then touch an absolute symbol from the rest of the
#      image.
#
# This is the single sketchiest moment in the x86_64 port, and it fails silently: a wrong page table
# means the instruction after `mov cr0, eax` is fetched through a broken mapping, the CPU takes a
# page fault with no IDT, escalates to a double fault with no IDT, and triple-faults, which on QEMU
# is a machine reset with no output. `-d int,cpu_reset -no-reboot` is how you see it. See
# notes/x86-port/boot.md.

# ---------------------------------------------------------------------------------------------
# The PVH entry note. This is the whole boot header: four words that tell the loader where to jump.
#
# It has to end up in a **PT_NOTE program header**, because that is what QEMU walks; a note sitting
# in a PT_LOAD segment is invisible to it. The link script therefore gives `.note.Xen` its own
# output section (SHT_NOTE, allocated), which is what makes lld emit the PT_NOTE, and does NOT sweep
# it into the `/DISCARD/` rule that drops every other `.note*`.
# ---------------------------------------------------------------------------------------------
.section .note.Xen, "a", @note
.align 4
    .long 2f - 1f                       # namesz
    .long 4f - 3f                       # descsz
    .long 18                            # XEN_ELFNOTE_PHYS32_ENTRY
1:  .asciz "Xen"
2:  .align 4
3:  .long _start                        # the 32-bit entry, a physical address (`.boot` is linked low)
4:  .align 4

# CFI (call-frame information): see notes/cfi-unwind.md. No directives anywhere in the 16-bit and
# 32-bit stretches of this file (`_start` through `long_mode_entry`, and the whole of
# `secondary_boot` through `ap_long_mode_entry`): these modes have no valid stack for most of their
# length (no caller, no `sp` worth describing) and DWARF call-frame information is specified and
# consumed as this ELF's own 64-bit format throughout, so annotating a 16- or 32-bit instruction
# stream inside it would be describing a frame in a format nothing downstream reads the same way
# real 64-bit code is read here. `long_mode_entry`/`_start_high` and `ap_long_mode_entry` (both
# `.code64`) get real CFI once execution is running in the width the rest of this file is.

# ---------------------------------------------------------------------------------------------
# The 32-bit trampoline. Linked low; every `offset` below is therefore a physical address.
# ---------------------------------------------------------------------------------------------
.section .text.boot, "ax"
.code32
.global _start
_start:
    cli                                 # the loader already masked, but say so
    cld                                 # `rep stos` below depends on DF=0, and nothing else set it

    # ebx holds the `hvm_start_info` pointer and must survive to `kernel_main`. Park it in ebp:
    # a 32-bit write zeroes the upper half of rbp, so the value is already a correct 64-bit physical
    # address by the time long mode reads it. (eax holds PVH's 0x336EC578 magic and is checked in
    # Rust rather than here, where a mismatch could not be reported.)
    mov ebp, ebx

    # --- 1. zero the boot page tables (.boot_scratch is NOLOAD; nothing else has touched it) ---
    mov edi, offset __boot_scratch_start
    mov ecx, offset __boot_scratch_end
    sub ecx, edi
    shr ecx, 2                          # bytes -> dwords
    xor eax, eax
    rep stosd

    # --- 2a. the page directories: 2048 entries of 2 MiB = the low 4 GiB, identity ---
    # 4 GiB rather than the 1 GiB the kernel image needs, because everything x86 talks to early is
    # above 1 GiB and below 4: the local APIC at 0xfee00000, the IO APIC at 0xfec00000, and q35's
    # PCIe ECAM window at 0xb0000000. A boot map that stopped at 1 GiB would mean every one of those
    # had to wait for the real page tables, which is a worse ordering constraint than 16 KiB of
    # page directories.
    mov edi, offset boot_pd
    mov eax, 0x00000083                 # present | writable | PS (this PDE maps a 2 MiB page)
    mov ecx, 2048
1:
    mov [edi], eax
    mov dword ptr [edi + 4], 0          # bits 63:32 of the entry: physical address < 4 GiB, no NX
    add eax, 0x00200000                 # next 2 MiB frame
    add edi, 8
    loop 1b

    # --- 2b. the low PDPT: four entries, one per page directory, covering 0 .. 4 GiB ---
    mov edi, offset boot_pdpt_low
    mov eax, offset boot_pd
    or eax, 0x03                        # present | writable
    mov ecx, 4
2:
    mov [edi], eax
    mov dword ptr [edi + 4], 0
    add eax, 4096                       # the next page directory
    add edi, 8
    loop 2b

    # --- 2c. the high PDPT: one entry, so KERNEL_VA_BASE aliases physical 0 .. 1 GiB ---
    # KERNEL_VA_BASE = 0xffffffff80000000 decomposes as PML4[511], PDPT[510], PD[0], which is why
    # the high alias reuses the FIRST page directory rather than needing one of its own.
    mov edi, offset boot_pdpt_high
    mov eax, offset boot_pd
    or eax, 0x03
    mov [edi + 510 * 8], eax
    mov dword ptr [edi + 510 * 8 + 4], 0

    # --- 2d. the PML4: entry 0 (identity) and entry 511 (the high half) ---
    mov edi, offset boot_pml4
    mov eax, offset boot_pdpt_low
    or eax, 0x03
    mov [edi], eax
    mov dword ptr [edi + 4], 0
    mov eax, offset boot_pdpt_high
    or eax, 0x03
    mov [edi + 511 * 8], eax
    mov dword ptr [edi + 511 * 8 + 4], 0

    # --- 2e. the direct map, at PML4[273], reusing the SAME low PDPT ---
    #
    # THIS IS THE ENTRY THAT MAKES `mmu::phys_to_virt` MEAN ONE THING FOR THE MACHINE'S WHOLE LIFE,
    # and it costs eight bytes because the low 4 GiB are already described by boot_pdpt_low.
    #
    # x86_64 cannot do what the other two architectures do, which is put the kernel image and the
    # direct map at one base: `code-model: kernel` pins every kernel symbol to the top 2 GiB, so
    # KERNEL_VA_BASE is 0xffffffff80000000 and there is no room above it for a map of physical
    # memory. The kernel image therefore keeps that base and the direct map gets its own,
    # DIRECT_MAP_BASE = 0xffff888000000000, which is Linux's and which decomposes as PML4[273].
    #
    # Establishing it HERE rather than when mmu::initialize builds the fine tables is what removes the
    # sequencing hazard the roadmap warned about: the frame allocator's bitmap, the PVH structure
    # and the ACPI tables are all read through phys_to_virt before the fine map exists, and if that
    # arithmetic changed meaning at the `mov cr3` those pointers would silently start naming
    # somewhere else. It does not change meaning, because this entry and the fine map's direct map
    # are the same base. See arch/x86_64/mmu.rs, which asserts the index below at compile time.
    mov eax, offset boot_pdpt_low
    or eax, 0x03
    mov [edi + 273 * 8], eax
    mov dword ptr [edi + 273 * 8 + 4], 0

    # --- 2f. NX, checked HERE because the next step is what would fault ---
    #
    # The first of three boot gates (design/roadmap/0524-the-three-x86-64-boot-gates.md). The other
    # two are checked in Rust by arch::x86_64::isa::initialize, where the console exists and a refusal
    # can be read. This one cannot wait for that: step 3 below sets EFER.NXE, and on a part without
    # NX that bit is reserved, so `wrmsr` raises #GP with no IDT installed, which escalates to a
    # double fault with no IDT and then triple-faults. On QEMU that is a machine reset with no
    # output whatsoever, which is the least informative failure this file can produce.
    #
    # So the check is here and the report is four bytes over a UART nobody has configured. That is
    # a deliberate exception to "a refusal a reader cannot act on is half a gate" and it is worth
    # naming as one: the alternative at this point in the boot is silence. See `no_nx` below.
    #
    # Only the boot CPU checks. `secondary_boot` replays this same EFER write without the check,
    # because a secondary is another core of the part this one just interrogated; a hybrid part
    # where that stops being true is in arch/x86_64/isa.rs's BUGS.
    mov eax, 0x80000000                 # the maximum EXTENDED leaf, a separate space from leaf 0's
    cpuid
    cmp eax, 0x80000001
    jb no_nx                            # a part that does not answer the leaf at all: CPUID would
                                        # reply with some other leaf's bits, so this is absence
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 20                   # EDX[20]: NX. AMD published it in 2003; Intel calls it XD
    jz no_nx

    # --- 3. PAE, CR3, LME, PG, in that order ---
    mov eax, cr4
    or eax, 1 << 5                      # CR4.PAE: 4-level paging needs it, and long mode needs it
    mov cr4, eax

    mov eax, offset boot_pml4
    mov cr3, eax

    mov ecx, 0xC0000080                 # IA32_EFER
    rdmsr
    or eax, 1 << 8                      # LME: long mode enable
    or eax, 1 << 11                     # NXE: bit 63 of a page entry means no-execute rather than
                                        # reserved. Set here, before any table exists that uses it,
                                        # because a page table built while NXE is clear and walked
                                        # while it is set changes meaning under the walker.
    wrmsr

    mov eax, cr0
    or eax, 1 << 31                     # PG: paging on. Long mode begins at this instruction's
                                        # retirement, and the next fetch goes through boot_pml4.
    or eax, 1 << 16                     # WP: supervisor writes honour the read-only bit. Without it
                                        # the kernel can write user read-only pages, which is a
                                        # confinement hole the other two architectures do not have
                                        # an equivalent switch for.
    mov cr0, eax

    # --- 4. widen to 64 bits by loading a code segment whose L bit is set ---
    lgdt [boot_gdt_pointer]
    # A far jump, hand-encoded. LLVM's Intel-syntax parser spells this several mutually incompatible
    # ways depending on version, and getting it wrong here produces a triple fault with no output
    # rather than an assembler error, so the three bytes are written out: EA = jmp far ptr16:32,
    # then the 32-bit offset, then the 16-bit selector.
    .byte 0xEA
    .long long_mode_entry
    .word 0x08                          # boot_gdt entry 1: 64-bit code, DPL 0

# ---------------------------------------------------------------------------------------------
# The NX refusal. Reached only from the check above, and it never returns.
#
# This is the earliest code in the kernel that has anything to say, and it says it the only way
# available at this point: bytes out of a 16550's transmit register at the fixed legacy COM1 base,
# which is the same UART `kernel/src/arch/x86_64/mod.rs`'s console driver later uses. Nothing has
# configured it, so this does the five writes that make it send (8N1 at 115200, interrupts off) and
# then polls the line-status register for each byte. `console::initialize` is never reached from here, so
# there is no state to hand back and nothing to conflict with.
#
# BUGS, and it is the same one the comment at the check states: on a machine with no COM1 at all
# (a modern laptop, a server whose serial is behind a BMC) these `out` instructions go nowhere and
# the halt below is the entire report. That is still strictly better than the triple-fault reset
# the check exists to prevent, because a halted machine can be attached to with a debugger and a
# reset machine cannot, but a reader at a blank screen learns nothing. A refusal this early has no
# better medium available.
# ---------------------------------------------------------------------------------------------
.type no_nx, @function
no_nx:
    mov dx, 0x3f9                       # IER: no interrupts, we poll
    xor eax, eax
    out dx, al
    mov dx, 0x3fb                       # LCR: DLAB, so the next two writes are the divisor
    mov al, 0x80
    out dx, al
    mov dx, 0x3f8
    mov al, 1                           # divisor 1 = 115200 baud, which QEMU ignores and a real
    out dx, al                          # 16550 needs
    mov dx, 0x3f9
    xor eax, eax
    out dx, al
    mov dx, 0x3fb                       # LCR: 8 bits, no parity, 1 stop, DLAB back off
    mov al, 0x03
    out dx, al

    mov esi, offset no_nx_message
1:  movzx ebx, byte ptr [esi]
    test bl, bl
    jz 3f
2:  mov dx, 0x3fd                       # LSR bit 5: the transmit holding register is empty
    in al, dx
    test al, 0x20
    jz 2b
    mov eax, ebx
    mov dx, 0x3f8
    out dx, al
    inc esi
    jmp 1b
3:  hlt                                 # and stay stopped: there is nothing this kernel can do
    jmp 3b                              # on a part whose page tables cannot say "not executable"
.size no_nx, . - no_nx

# In `.text.boot` with the code that reads it, because `offset no_nx_message` above is a 32-bit
# absolute address and only this section is linked at VA == PA.
no_nx_message:
    .asciz "\r\nnife cannot run on this machine:\r\n  cpu feature : nx is absent (the hardware bit W^X is made of; without it the page tables' no-execute is a comment)\r\n                CPUID.80000001H:EDX[20] reads 0\r\nhalted before enabling paging, because EFER.NXE would #GP here.\r\n"

.code64
.type long_mode_entry, @function
long_mode_entry:
    .cfi_startproc
    # No caller: falls out of the 32-bit trampoline above with no stack yet. A SEPARATE region from
    # `_start_high` below despite being one continuous control-flow: they land in different output
    # sections (`.text.boot` here, `.text` there), and `.size`'s `. - symbol` cannot cross a section
    # boundary, so each needs its own `.cfi_startproc`/`.cfi_endproc`/`.size`.
    .cfi_undefined rip
    # Load the flat data descriptor everywhere. In 64-bit mode the base and limit of these are
    # ignored, but the segment registers still have to hold something loadable, and a stale 32-bit
    # descriptor from the loader's own GDT is not it once we have replaced the GDT.
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    # Leave the identity map behind. `movabs` is the only way to get a 64-bit immediate into a
    # register, and this is the first instruction in the kernel that names a high address.
    movabs rax, offset _start_high
    jmp rax
    .cfi_endproc
.size long_mode_entry, . - long_mode_entry

# ---------------------------------------------------------------------------------------------
# The 64-bit, high-half world. From here every absolute symbol resolves correctly.
# ---------------------------------------------------------------------------------------------
.section .text, "ax"
.code64
.type _start_high, @function
_start_high:
    .cfi_startproc
    # No caller either, same reasoning as `long_mode_entry` just above (which jumps here).
    .cfi_undefined rip
    movabs rsp, offset __stack_top      # the high boot stack

    # Zero .bss by hand (it occupies no bytes in the ELF). Both bounds are page-aligned, so the
    # 8-byte store loop divides evenly. es and DF are already set correctly above.
    movabs rdi, offset __bss_start
    movabs rcx, offset __bss_end
    sub rcx, rdi
    shr rcx, 3
    xor eax, eax
    rep stosq

    # Stash WHICH core booted, before any secondary exists to be confused with it. The value is
    # CPUID leaf 1's "Initial APIC ID" (ebx[31:24]), the same one `arch::boot_cpu_id` used to
    # recompute per caller; the point of recording it here is that this code runs exactly once, on
    # the boot processor, so the answer means "which core booted" rather than "which core am I"
    # (ap_boot.rs's BUGS #3, and riscv64/boot.s's BOOT_HARTID stash is the same move). .bss was just
    # zeroed, so this survives. `cpuid` clobbers eax/ebx/ecx/edx; rbp (hvm_start_info) is untouched.
    mov eax, 1
    cpuid
    shr ebx, 24
    movzx ebx, bl
    movabs rax, offset BOOT_CPU_ID
    mov [rax], rbx

    # kernel_main(hvm_start_info_physical_address). The other two architectures pass a device-tree
    # pointer in this argument; x86 has no device tree, so what travels is PVH's `hvm_start_info`,
    # which is where the memory map and the ACPI RSDP address live. See arch/x86_64/machine.rs.
    mov rdi, rbp
    # The System V AMD64 ABI wants rsp 16-byte aligned *before* the call instruction pushes the
    # return address. __stack_top is page-aligned, so it already is; the alignment is stated here
    # because a misaligned stack only shows up much later, in SSE code that does not exist yet.
    call kernel_main

    # kernel_main is `-> !`. If it ever returns, stop rather than run off into whatever follows.
3:  hlt
    jmp 3b
    .cfi_endproc
.size _start_high, . - _start_high

# ---------------------------------------------------------------------------------------------
# The boot GDT: the minimum that makes a far jump into 64-bit mode legal.
#
# Three descriptors, because x86 requires a null one, a code one with L set, and a data one for
# ss/ds. It is deliberately NOT the GDT the kernel runs on: that one carries a TSS (the only way
# x86 knows where to put the stack on a ring-3 -> ring-0 transition) and is installed from Rust
# once there is somewhere to allocate it. See arch/x86_64/segments.rs.
# ---------------------------------------------------------------------------------------------
.section .data.boot, "a"
.align 16
boot_gdt:
    .quad 0x0000000000000000            # 0x00: the mandatory null descriptor
    .quad 0x00AF9A000000FFFF            # 0x08: code, DPL 0, L=1 (64-bit), present
    .quad 0x00CF92000000FFFF            # 0x10: data, DPL 0, present
boot_gdt_end:

boot_gdt_pointer:
    .word boot_gdt_end - boot_gdt - 1   # the limit is size MINUS ONE, and always has been
    .long boot_gdt                      # a 32-bit base, because lgdt runs in 32-bit mode

# ---------------------------------------------------------------------------------------------
# The boot page tables. NOLOAD (see link-x86_64.ld), so these are 28 KiB of reserved physical
# address space rather than 28 KiB of zeros in the image; the trampoline zeroes them itself.
#
# boot_pd is FOUR contiguous pages and the code above depends on that adjacency: it walks the low
# PDPT writing boot_pd, boot_pd+4096, boot_pd+8192, boot_pd+12288.
# ---------------------------------------------------------------------------------------------
.section .bss.boot, "aw", @nobits
.align 4096
boot_pml4:
    .space 4096
boot_pdpt_low:
    .space 4096
boot_pdpt_high:
    .space 4096
boot_pd:
    .space 4 * 4096

# ---------------------------------------------------------------------------------------------
# The secondary-CPU entry (milestone 161's SMP item): a real-mode trampoline, linked at the fixed
# low physical address AP_TRAMPOLINE_PHYS (link-x86_64.ld) rather than at this file's usual high
# addresses, because a STARTUP IPI's vector can only name a physical PAGE below 1 MiB and the target
# CPU begins executing there in 16-bit real mode (cs = vector << 8, ip = 0, so the physical address
# is exactly vector << 12).
#
# `secondary_boot`'s VIRTUAL address (VMA) IS its final execution address: link-x86_64.ld gives
# `.ap_trampoline` an explicit low VMA but leaves its LOAD address (LMA) wherever the image
# naturally places its bytes, so at boot they sit somewhere ordinary (beside `.rodata`) and
# `arch::x86_64::ap_boot::prepare` copies them here, to AP_TRAMPOLINE_PHYS, before the first
# `STARTUP` IPI ever fires. Because the VMA already matches, every absolute address the assembler
# baked into this code (this file's OWN tiny GDT pointer below, and both far-jump targets) is
# already correct the instant the copy lands; only the one genuinely dynamic value
# (`ap_trampoline_stack_top`) needs writing at runtime, and `prepare` writes it.
#
# THE SHAPE, THREE MODES IN ONE FUNCTION, no stack used anywhere in it (matching `_start`'s own
# steps 3-4, which this deliberately mirrors rather than jumps into: `_start`'s continuation is
# `_start_high`, which zeroes `.bss` and calls `kernel_main`, and neither is what a second core
# should do):
#
#   1. 16-bit real mode: `cli`, zero the segment registers (their post-SIPI value is UNDEFINED, and
#      every memory operand below is `[disp16]`, i.e. DS-relative, so DS must be exactly 0 for a
#      disp16 equal to this trampoline's own low VMA to name the right byte), load THIS file's own
#      tiny 32-bit-flat GDT (a 16-bit real-mode `lgdt` cannot reach `_start`'s `boot_gdt`, which is
#      linked near 1 MiB, well past what a zero-based 16-bit displacement can address), set
#      `CR0.PE`, and far-jump into 32-bit protected mode. The far jump is hand-encoded, as
#      `_start`'s is, and for the same reason (assembler Intel-syntax quirks); it is the 16-bit
#      *ptr16:16* form here (`EA iw iw`), not `_start`'s 32-bit *ptr16:32* form, because the default
#      operand size is still 16 here.
#   2. 32-bit protected mode: **from here, ordinary flat 32-bit addressing reaches every low-physical
#      symbol in this image**, including `_start`'s own `boot_pml4` and `boot_gdt_pointer` (already
#      built once by the boot core and never freed: `.boot_scratch` is on the frame allocator's
#      forbidden list for the whole life of the machine). Replays `_start`'s own PAE/CR3/LME/NXE/
#      PG/WP sequence verbatim, against the SAME page tables, then `lgdt`s `_start`'s real
#      `boot_gdt` (which has the 64-bit code descriptor this trampoline's own tiny GDT does not) and
#      far-jumps into long mode, exactly as `_start` does.
#   3. 64-bit long mode: reload the flat data selectors, set `rsp` from `ap_trampoline_stack_top`
#      (read RIP-relative: still executing at the low VMA here, referencing another symbol in the
#      same low blob), read this CPU's own local APIC id via `CPUID` leaf 1 `EBX[31:24]` (the same
#      "Initial APIC ID" `arch::boot_cpu_id` reads on the boot core, so the logical id this hands to
#      `secondary_main` agrees with the roster's own seating), and jump to `secondary_main`'s HIGH
#      virtual address. That address is reachable through `boot_pml4` because `_start`'s own high
#      alias (`boot_pdpt_high[510]`) covers the whole kernel image, which is well under the 1 GiB it
#      spans.
# ---------------------------------------------------------------------------------------------
.section .ap_trampoline, "ax"
# NOT `.text.ap_trampoline`: link-x86_64.ld's `.text` output section greedily matches every input
# section named `.text.*` (`*(.text .text.*)`), which runs *before* this file's own `.ap_trampoline`
# output section is reached, so a `.text.ap_trampoline` input section would be silently swallowed
# into the ordinary high `.text` output section instead of landing at AP_TRAMPOLINE_PHYS. Found by
# building: every symbol in this trampoline resolved to a `KERNEL_VA_BASE`-relative address instead
# of a low one, and every absolute reference to it then overflowed its relocation width.
.code16
.global secondary_boot
secondary_boot:
    cli
    cld
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax

    lgdt [ap_gdt_pointer]

    mov eax, cr0
    or eax, 1                          # CR0.PE: protected mode enable
    mov cr0, eax

    # Far jump, hand-encoded (ptr16:16, since the default operand size here is still 16 bits): the
    # opcode, a 16-bit offset (fits: this whole trampoline is far under 64 KiB from its own base, so
    # a plain disp16 names it), then the selector.
    .byte 0xEA
    .word ap_pmode32
    .word 0x08                          # ap_gdt entry 1: 32-bit flat code, DPL 0

.code32
ap_pmode32:
    mov ax, 0x10                        # ap_gdt entry 2: 32-bit flat data, DPL 0
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    # From here, ordinary 32-bit absolute addressing reaches any low-physical symbol, `_start`'s
    # own included. Replays `_start`'s step 3 verbatim, against the SAME boot page tables.
    mov eax, cr4
    or eax, 1 << 5                      # CR4.PAE
    mov cr4, eax

    mov eax, offset boot_pml4
    mov cr3, eax

    mov ecx, 0xC0000080                 # IA32_EFER
    rdmsr
    or eax, 1 << 8                      # LME
    or eax, 1 << 11                     # NXE
    wrmsr

    mov eax, cr0
    or eax, 1 << 31                     # PG
    or eax, 1 << 16                     # WP
    mov cr0, eax

    # `_start`'s REAL boot GDT, which (unlike this file's own tiny one above) carries the 64-bit
    # code descriptor long mode needs. Reachable now: flat 32-bit addressing, no 16-bit range limit.
    lgdt [boot_gdt_pointer]
    .byte 0xEA
    .long ap_long_mode_entry
    .word 0x08                          # boot_gdt entry 1: 64-bit code, DPL 0, L=1

.code64
.type ap_long_mode_entry, @function
ap_long_mode_entry:
    .cfi_startproc
    # No caller: falls out of the AP's own 16/32-bit trampoline above. Same reasoning as
    # `long_mode_entry`.
    .cfi_undefined rip
    mov ax, 0x10                        # boot_gdt entry 2: flat data
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    # Still executing at the low VMA: RIP-relative addressing names another symbol in this same
    # trampoline blob correctly, the one value `ap_boot::prepare` had to write at runtime because
    # nothing else could have known it ahead of time.
    mov rsp, [rip + ap_trampoline_stack_top]

    # This core's own local APIC id, the same "Initial APIC ID" `arch::boot_cpu_id` reads on the
    # boot core, so the id handed to `secondary_main` agrees with the roster's seating
    # (`smp::seat_cpus_from_acpi`, which seats every core at the slot its own local APIC id names).
    mov eax, 1
    cpuid
    shr ebx, 24
    movzx edi, bl                       # secondary_main(cpu_id: usize) -> rdi, the SysV first arg

    push rdi
    pop rdi

    movabs rax, offset secondary_main
    jmp rax
    .cfi_endproc
.size ap_long_mode_entry, . - ap_long_mode_entry

# ---------------------------------------------------------------------------------------------
# This trampoline's own data: a tiny GDT good only for the 16->32 step (this file's real one,
# `boot_gdt`, carries the 64-bit descriptor the second step needs and is reused directly), and the
# one word `ap_boot::prepare` writes before every `STARTUP` IPI.
# ---------------------------------------------------------------------------------------------
.align 16
ap_gdt:
    .quad 0x0000000000000000            # 0x00: the mandatory null descriptor
    .quad 0x00CF9A000000FFFF            # 0x08: 32-bit flat code, DPL 0, D/B=1, present
    .quad 0x00CF92000000FFFF            # 0x10: 32-bit flat data, DPL 0, present
ap_gdt_end:

ap_gdt_pointer:
    .word ap_gdt_end - ap_gdt - 1
    .long ap_gdt

.align 8
.global ap_trampoline_stack_top
ap_trampoline_stack_top:
    .quad 0
