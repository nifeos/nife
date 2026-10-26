// The kernel's real entry, reached by the `b _boot` in image_header.s.
//
// # We are linked HIGH but loaded LOW
//
// link-aarch64.ld places the kernel at 0xffff_0000_4008_0000 (virtual) but tells the loader to put
// the bytes at 0x4008_0000 (physical). So every absolute address the compiler baked into
// this binary is a virtual address that **does not work yet**, and the code that turns the
// MMU on is inside that binary.
//
// Two facts get us out.
//
// ## 1. `adrp` is PC-relative, so it yields PHYSICAL addresses right now
//
//     adrp x0, __stack_top          // x0 = (PC & ~0xfff) + linker_offset
//
// The linker computes `linker_offset` from *virtual* addresses. But PC is currently a
// *physical* address, and VA - PA is a constant (0xffff_0000_0000_0000), so the two
// differences cancel and we get the physical address of the symbol. Free of charge.
//
// This is why nothing below uses `ldr x0, =symbol` until the MMU is on: a literal pool holds
// the absolute VA, which is exactly the thing that doesn't work yet. (Literal pools holding
// *constants* are fine; the load itself is PC-relative.)
//
// ## 2. Bits 63:48 are not translated, so ONE table serves as both maps
//
// A virtual address `PA + 0xffff_0000_0000_0000` has the same L0/L1/L2/L3 indices as `PA`
// itself, because the index only ever reads bits 47:12 (see notes/page-tables.md). So the
// identity map and the high-half map are **the same table contents**, and we simply point
// TTBR0 and TTBR1 at the same root.
//
// That is the whole trick, and it's why the boot tables below are 2 pages rather than a
// careful dance.
//
// # We may be entered at EL2, and we may be entered at EL1
//
// QEMU's `virt` starts a kernel at **EL1** unless the machine is built with
// `virtualization=on`, and every real aarch64 bootloader this project is headed for starts a
// payload at **EL2**: U-Boot on the Jetson TX1 does, with TF-A's tegra210 BL31 providing PSCI
// below it, and TF-A's Raspberry Pi 4 port does too. A kernel that assumes EL1 writes `TTBR0_EL1`
// and `SCTLR_EL1` from EL2, where those writes configure a translation regime it is not
// executing in, and then branches into a high half nothing maps.
//
// So the entry **reads `CurrentEL` and drops itself** rather than being built two ways. A
// compile-time switch would mean one binary for QEMU and a different one for a board, which is
// the arrangement that produces a defect nobody can reproduce on the machine they have.
// `enter_el1` below is the whole of it, and both entries (core 0 and the PSCI secondaries) go
// through it, because PSCI starts a secondary at the highest implemented non-secure exception
// level regardless of which level called `CPU_ON`.
//
// # The sequence
//
//   0. drop to EL1 if we were entered at EL2 (`enter_el1`)
//   1. park cores 1..n
//   2. set up a PHYSICAL stack (adrp)
//   3. zero .bss (adrp)
//   4. build a crude 1 GiB-block map: device @ 0, RAM @ 0x4000_0000
//   5. TTBR0 = TTBR1 = that map
//   6. MMU on. We are still executing at the physical address, via TTBR0's identity map.
//   7. sp = the HIGH virtual address of the stack
//   8. jump to kernel_main's HIGH virtual address
//
// After step 8, everything is virtual and Rust never has to think about this again.
//
// See notes/higher-half.md.

// CFI (call-frame information) below: see notes/cfi-unwind.md for what these directives mean and
// why this file's shape is `.cfi_undefined lr` almost everywhere. Short version: nothing in this
// file is reached by `bl`, so `lr` (x30) never holds a real return address here, and inventing a
// CFA formula anyway would tell a debugger to unwind into whatever garbage x30 last held. Marking
// it undefined tells the unwinder, correctly, that this is as far back as it can go.

.section ".text.boot", "ax"
.global _boot

// The boot map is deliberately COARSE and PERMISSIVE: two 1 GiB blocks, and the RAM one is
// executable everywhere. It exists to survive the next twenty instructions, nothing more.
// mmu.rs immediately replaces it with a fine-grained map that enforces W^X and punches out
// the guard page. Linux does exactly this, for exactly this reason.
//
//   block @ 0x0000_0000, DEVICE:  AF | PXN | UXN | block
//   block @ 0x4000_0000, NORMAL:  AF | SH_inner | AttrIdx=1 | UXN | block   (PXN clear: we
//                                 must be able to execute our own .text)
.equ BOOT_DEVICE_BLOCK, 0x0060000000000401
.equ BOOT_NORMAL_BLOCK, 0x0040000040000705

// MAIR: slot 0 = Device-nGnRnE (0x00), slot 1 = Normal write-back (0xff).
.equ BOOT_MAIR,         0xff00

// TCR: T0SZ=T1SZ=16 (48-bit VAs), 4 KiB granule both halves, inner-shareable write-back
// table walks, both TTBRs enabled.
//
// **TG0 and TG1 use DIFFERENT ENCODINGS for 4 KiB**: TG0=0b00, TG1=0b10. That is not a typo
// below, it is the architecture.
.equ BOOT_TCR,          0xb5103510

.type _boot, @function
_boot:
    .cfi_startproc
    // No caller: this is where the firmware/QEMU jumps with the MMU off and no stack. See the
    // file header's CFI note.
    .cfi_undefined lr
    // The firmware handed us the device tree pointer in x0, and it is a PHYSICAL address. Keep
    // it; kernel_main converts it. This is the FIRST instruction executed for the same reason
    // image_header.s does not touch x0: everything below is allowed to clobber it, and x19 is
    // preserved across the `eret` in `enter_el1` exactly as it is across everything else.
    //
    // **What is verified and what is a firmware contract.** That QEMU puts a device tree here is
    // verified continuously: `device_tree_pointer_was_provided` in main.rs fails if x0 arrives as
    // zero, and it exists because milestone 1 shipped an ELF, printed x0, and got zero. That
    // U-Boot's `booti` does the same is Linux's arm64 boot protocol
    // (Documentation/arch/arm64/booting.rst: x0 is the physical address of the device tree blob,
    // x1-x3 zero), which is a contract this project has not yet held a board to. It is the first
    // thing milestone 127's bench list checks.
    mov     x19, x0

    // Drop to EL1 if we were entered at EL2. Returns (or `eret`s) to the label in x21, leaves
    // the entry level in x22, and clobbers nothing else this function needs.
    adr     x21, _boot_el1
    b       enter_el1

_boot_el1:
    // Park every core but core 0 (DECISIONS §6).
    mrs     x0, mpidr_el1
    and     x0, x0, #0xff
    cbnz    x0, park

    // A physical stack. adrp+add yields the PA; see the header comment.
    adrp    x0, __stack_top
    add     x0, x0, :lo12:__stack_top
    mov     sp, x0

    // Zero .bss by hand. Nobody loaded it (it occupies no bytes in the file) and there is no
    // C runtime here. The boot page tables live in .bss, so this also zeroes them, which is
    // load-bearing: a page table full of whatever was in RAM is a set of pointers to nowhere,
    // followed at speed.
    adrp    x0, __bss_start
    add     x0, x0, :lo12:__bss_start
    adrp    x1, __bss_end
    add     x1, x1, :lo12:__bss_end
1:  cmp     x0, x1
    b.hs    2f
    str     xzr, [x0], #8
    b       1b
2:

    // Record the exception level we were ENTERED at, now that .bss holds zeros rather than
    // whatever was in RAM. `arch::aarch64::entry_el` reads this, the boot banner prints it, and
    // the PSCI-conduit test keys on it: a machine that enters at EL2 also states `smc` rather
    // than `hvc`, because an `hvc` from EL1 would arrive at an EL2 with no vectors installed.
    //
    // It is core 0's answer only. Every core runs `enter_el1` and each reads its own
    // `CurrentEL`; nothing has ever seen them disagree, and a per-core record would be four
    // words to say one thing.
    adrp    x0, boot_entry_el
    add     x0, x0, :lo12:boot_entry_el
    str     x22, [x0]

    // --- build the boot page tables ---

    adrp    x0, boot_l0
    add     x0, x0, :lo12:boot_l0       // x0 = PA of the L0 table
    adrp    x1, boot_l1
    add     x1, x1, :lo12:boot_l1       // x1 = PA of the L1 table

    // L0[0] -> L1.  A table descriptor is just the address with bits[1:0] = 0b11.
    orr     x2, x1, #3
    str     x2, [x0]

    // L1[0]: 1 GiB block at 0x0000_0000, device memory. Covers the PL011 at 0x0900_0000.
    // Without this the machine goes silent the instant the MMU comes on.
    ldr     x2, =BOOT_DEVICE_BLOCK
    str     x2, [x1]

    // L1[1]: 1 GiB block at 0x4000_0000, normal memory, executable. This is where we are.
    //
    // NOTE: hardcoded for QEMU `virt`, whose RAM starts at 0x4000_0000. The Raspberry Pi
    // puts RAM at 0, and this is one of the handful of places that port will have to touch.
    ldr     x2, =BOOT_NORMAL_BLOCK
    str     x2, [x1, #8]

    // --- turn the MMU on ---

    ldr     x2, =BOOT_MAIR
    msr     mair_el1, x2

    ldr     x2, =BOOT_TCR
    mrs     x3, id_aa64mmfr0_el1
    and     x3, x3, #0xf                // PARange: how many physical address bits this CPU
    orr     x2, x2, x3, lsl #32         // actually has. Claiming more is UNPREDICTABLE.
    msr     tcr_el1, x2

    // BOTH registers, SAME table. See fact 2 in the header comment: the identity map and the
    // high-half map have identical contents, because bits 63:48 never reach an index.
    msr     ttbr0_el1, x0
    msr     ttbr1_el1, x0

    // Every write above must be visible to the page-table walker, which is a separate
    // observer, before it can possibly walk them.
    dsb     sy
    isb
    tlbi    vmalle1                     // throw away any stale translations
    dsb     ish
    isb

    // The point of no return. The instruction fetched AFTER this one goes through the MMU.
    // We survive it because TTBR0 identity-maps the page we are executing from.
    mrs     x2, sctlr_el1
    orr     x2, x2, #(1 << 0)           // M: MMU enable
    orr     x2, x2, #(1 << 2)           // C: data cache
    orr     x2, x2, #(1 << 12)          // I: instruction cache
    msr     sctlr_el1, x2
    isb

    // --- we are now running with paging on, still at the physical address ---
    //
    // From here, `ldr x, =symbol` finally means what it says: the literal pool holds the
    // virtual address, and TTBR1 maps it.

    ldr     x0, =__stack_top            // the HIGH stack
    mov     sp, x0

    mov     x0, x19                     // the device tree (still a physical address)
    ldr     x1, =kernel_main            // the HIGH entry point
    br      x1                          // and we are in the high half forever
    .cfi_endproc
.size _boot, . - _boot

// --- the EL2 to EL1 drop (milestone 127's first prerequisite) ---
//
// Both entries above come here first. In:
//
//   x21  where to continue, a PHYSICAL address (`adr`, not `ldr =`: the MMU is off)
//
// Out:
//
//   x22  the exception level we were entered at, as a number
//   x0-x2 clobbered. x19 and x21 are untouched, and so is every other register: an `eret`
//         changes the exception level and nothing else about the general-purpose file, which is
//         what lets the device tree pointer ride through in x19.
//
// **Every write below is here because the register it names is UNKNOWN out of reset**, or
// because a bootloader is allowed to leave it however it liked. That is the whole design
// criterion: at EL1 these are either invisible (the kernel cannot write `HCR_EL2`) or read as
// something the kernel would then believe. Each one cites where it comes from; the register
// descriptions are Arm DDI 0487 (the Arm Architecture Reference Manual for A-profile), and the
// choice of *which* ones is checked against Linux's `arch/arm64/kernel/head.S` `init_el2`, which
// is the same list arrived at independently by people who have booted on everything.
//
// **Not here, and deliberately:** `SCTLR_EL2` is left alone, because the arm64 boot protocol
// requires the MMU and caches to be off on entry and this kernel takes that contract rather than
// re-proving it; and `CNTFRQ_EL0` is writable only at the highest
// implemented level and is firmware's to set, so writing our own guess would replace a real
// number with an invented one.
.global enter_el1
.type enter_el1, @function
enter_el1:
    .cfi_startproc
    // Reached by a plain `b`, not `bl`: the caller passes its own resume point in x21, not lr.
    // lr therefore carries whatever the caller's caller left in it and names no return here at
    // all; describing it as this function's return address would be describing the wrong thing.
    .cfi_undefined lr
    // `CurrentEL` holds the level in bits [3:2] and reads as RES0 elsewhere, so this is the
    // level as an ordinary number.
    mrs     x0, CurrentEL
    ubfx    x22, x0, #2, #2
    cmp     x22, #2
    b.ne    9f                          // already EL1: the QEMU `virt` path, unchanged

    // 1. SCTLR_EL1. Its reset value is UNKNOWN and this is the register the MMU-enable below
    //    read-modify-writes, so entering at EL2 without this would `orr` three bits into
    //    garbage. 0x30d0_0800 is the RES1 bits (11, 20, 22, 23, 28, 29) and nothing else: MMU
    //    off, caches off, alignment checking off, little-endian. Linux spells the same constant
    //    `INIT_SCTLR_EL1_MMU_OFF`.
    mov     x0, #0x0800
    movk    x0, #0x30d0, lsl #16
    msr     sctlr_el1, x0

    // 2. HCR_EL2.RW (bit 31): the lower exception level is AArch64. Zero means AArch32, which is
    //    what a cleared HCR_EL2 would give us, and this kernel is aarch64 only. Every other bit
    //    stays zero, which is what says stage-2 translation is off (VM) and that none of EL1's
    //    ordinary operations trap up to EL2.
    mov     x0, #0x80000000
    msr     hcr_el2, x0

    // 3. CPTR_EL2 = its RES1 pattern with TFP and TTA clear, so EL1 and EL0 may use FP and SIMD.
    //    Reset is UNKNOWN; a set TFP would fault the first floating-point instruction anywhere in
    //    the system, which on this ISA includes the compiler's own use of `q` registers in
    //    `memcpy`.
    mov     x0, #0x33ff
    msr     cptr_el2, x0

    // 4. HSTR_EL2 = 0: no trapping of AArch32 system-register accesses. Nothing here is AArch32,
    //    and a stale nonzero value would trap instructions that do not exist in this kernel; it
    //    is one instruction to make the answer definite rather than inherited.
    msr     hstr_el2, xzr

    // 5. MDCR_EL2: no debug or PMU traps to EL2, and every event counter handed to EL1. This is
    //    the one on milestone 74's path (the PMU counters half): MDCR_EL2.TPM traps every EL1
    //    access to `PMCCNTR_EL0`, its reset value is UNKNOWN, and a trap into an EL2 with no
    //    vector table is a hang with no console output at all.
    //
    //    **Every field is zero except HPMN (bits 4:0), and HPMN is not zero on purpose.** It says
    //    how many event counters EL1 may use, and zero is a RESERVED value unless the part has
    //    FEAT_HPMN0, which the Cortex-A57 on argon (Armv8.0) does not: the Arm ARM makes the
    //    split CONSTRAINED UNPREDICTABLE, and EL1's `PMCR_EL0.N` reads HPMN back. This wrote 0 from
    //    2026-09-02, when the drop was built, until milestone 74's boot line found it on
    //    2026-09-19 by printing `0 event counters visible` under `NIFE_EL2=1` and 6 without it. Linux's `init_el2`
    //    (`arch/arm64/include/asm/el2_setup.h`) does exactly this, HPMN = `PMCR_EL0.N` read at
    //    EL2, where N is the real count. Gated on `ID_AA64DFR0_EL1.PMUVer` like every other PMU
    //    access in this kernel, because `PMCR_EL0` is UNDEFINED without PMUv3; there, HPMN is
    //    RES0 and zero is right. The cycle counter is not an event counter and never depended on
    //    this; nothing in this kernel uses an event counter yet either.
    mov     x0, xzr
    mrs     x1, id_aa64dfr0_el1
    ubfx    x1, x1, #8, #4              // PMUVer
    cbz     x1, 5f                      // 0: no PMU
    cmp     x1, #0xf
    b.eq    5f                          // 0xf: IMPLEMENTATION DEFINED, not PMUv3
    mrs     x0, pmcr_el0
    ubfx    x0, x0, #11, #5             // PMCR_EL0.N, the implemented count, as HPMN
5:  msr     mdcr_el2, x0

    // 6. MDSCR_EL1 = 0: no EL1 debug exceptions armed (MDE, SS). Reset UNKNOWN, and a bootloader
    //    that was itself debugged can leave single-stepping on. Linux zeroes this on the same
    //    path and for the same reason.
    msr     mdscr_el1, xzr

    // 7. The counter and timer. Two writes, and the second is the one that matters here.
    //
    //    CNTHCTL_EL2.EL1PCTEN and .EL1PCEN (bits 0 and 1, the E2H=0 layout) stop EL1 accesses to
    //    the physical counter and timer trapping to EL2.
    //
    //    CNTVOFF_EL2 is subtracted from the physical counter to produce the VIRTUAL one, and
    //    `arch/aarch64/timer.rs` deliberately uses the virtual timer (CNTVCT_EL0, CNTV_CVAL_EL0)
    //    because it is the one available at EL1 under a hypervisor too. Its reset value is
    //    UNKNOWN, so leaving it is leaving the system clock offset by an arbitrary 64-bit number.
    mrs     x0, cnthctl_el2
    orr     x0, x0, #3
    msr     cnthctl_el2, x0
    msr     cntvoff_el2, xzr

    // 8. VPIDR_EL2 and VMPIDR_EL2. **At EL1 with EL2 implemented, `MIDR_EL1` and `MPIDR_EL1`
    //    return these registers rather than the hardware's own**, and both reset UNKNOWN. Read
    //    at EL2 they still give the real values, so this copies the part id and the affinity
    //    across. Getting it wrong is not subtle in its consequences and is very subtle in its
    //    symptoms: `_boot_el1` parks on MPIDR affinity 0, `arch::isa` decodes MIDR to decide
    //    whether the machine can run this kernel at all, and `smp.rs` starts cores by affinity.
    mrs     x0, midr_el1
    msr     vpidr_el2, x0
    mrs     x0, mpidr_el1
    msr     vmpidr_el2, x0

    // 9. VTTBR_EL2 = 0. Stage-2 translation is off (HCR_EL2.VM is clear above), but the VMID
    //    field of this register still tags the EL1 TLB entries we are about to create, so a
    //    stale value would tag them with a number nothing invalidates by name.
    msr     vttbr_el2, xzr

    // 9a. The GICv3 system-register interface (milestone 227). **Without this an EL1 kernel on a
    //     GICv3 cannot reach its own CPU interface**: `ICC_SRE_EL2.Enable` (bit 3) clear makes
    //     every EL1 write of `ICC_SRE_EL1.SRE` read back as zero, and `ICC_SRE_EL2.SRE` (bit 0) is
    //     this level's own switch. Both set, then `isb` and read back; only if SRE stuck is
    //     `ICH_HCR_EL2` zeroed, which turns the virtual CPU interface off so nothing is diverted to
    //     it. That is Linux's `__init_el2_gicv3` (arch/arm64/include/asm/el2_setup.h, v6.16),
    //     including the guard: `ID_AA64PFR0_EL1.GIC` (bits 27:24) zero means the core has no
    //     system-register interface, the registers do not exist, and touching them is UNDEFINED.
    //     So a GICv2 machine, argon's, skips the whole step. `gic_cpu_interface::initialize_this_cpu`
    //     asserts the EL1 half actually stuck.
    mrs     x0, id_aa64pfr0_el1
    ubfx    x0, x0, #24, #4
    cbz     x0, 8f
    mrs     x0, icc_sre_el2
    orr     x0, x0, #(1 << 0)           // SRE
    orr     x0, x0, #(1 << 3)           // Enable: EL1 may use the system registers too
    msr     icc_sre_el2, x0
    isb
    mrs     x0, icc_sre_el2
    tbz     x0, #0, 8f
    msr     ich_hcr_el2, xzr
8:

    // 10. And go. SPSR_EL2 = 0x3c5 is D, A, I and F masked (bits 9:6) with M[3:0] = 0b0101,
    //     which is EL1h: EL1 using SP_EL1, the same stack pointer arrangement `_boot_el1` and
    //     `_secondary_el1` then write with `mov sp, x`. Masked interrupts are what EL1 entry
    //     looks like on the QEMU path too, and nothing is ready to take one until
    //     `exceptions::initialize` writes VBAR_EL1.
    //
    //     No `isb` before the `eret`: an `eret` is itself a context synchronization event, so
    //     every write above is in effect for the first instruction at EL1.
    mov     x0, #0x3c5
    msr     spsr_el2, x0
    msr     elr_el2, x21
    eret

9:  br      x21
    .cfi_endproc
.size enter_el1, . - enter_el1

.type park, @function
park:
    .cfi_startproc
    .cfi_undefined lr
    // wfi, not wfe: QEMU idles the host thread on wfi and merely spins on wfe. A parked core
    // that burns 100% of a host CPU is not parked. See notes/qemu.md.
    wfi
    b       park
    .cfi_endproc
.size park, . - park

// --- secondary core entry (SMP step 2, DECISIONS §11) ---
//
// PSCI CPU_ON starts a secondary HERE, at this PHYSICAL address, with the MMU off, at EL1,
// exactly the way QEMU started core 0 at `_boot`. x0 holds the context word core 0 passed to
// CPU_ON: this core's HIGH-VA stack top (unusable until the MMU is on, which is fine, nothing
// below touches the stack before then).
//
// The crucial difference from `_boot`: the page tables already exist. Core 0 built `boot_l0`
// and it is still sitting in .bss, so we do NOT rebuild it. We only replay the MMU-enable
// (fact 1 in the header comment gets us the table's PA with `adrp`) and jump to the high half.
.global secondary_boot
.type secondary_boot, @function
secondary_boot:
    .cfi_startproc
    // No caller: PSCI CPU_ON starts a secondary core here directly, MMU off, no stack. Same
    // reasoning as `_boot`.
    .cfi_undefined lr
    mov     x19, x0                     // stash the stack-top VA for after the MMU is on

    // **A secondary can arrive at EL2 even though EL1 made the call**, so this drop is not
    // defensive symmetry. PSCI's own wording is that a core brought up by `CPU_ON` enters at the
    // highest implemented non-secure exception level (Arm DEN 0022, `CPU_ON`), and both
    // implementations this kernel will meet do exactly that: QEMU's TCG PSCI picks EL2 whenever
    // the machine has EL2 at all, and TF-A enters the non-secure world at the EL its BL31 context
    // was built for, which on the tegra210 port is EL2. So a kernel that dropped only core 0
    // would come up single-core-correct and then fault on its second core, which is the worst
    // shape a bug can have.
    adr     x21, _secondary_el1
    b       enter_el1

_secondary_el1:
    // Point at the boot tables core 0 already built. adrp yields the PA (PC-relative, MMU off).
    adrp    x0, boot_l0
    add     x0, x0, :lo12:boot_l0

    ldr     x2, =BOOT_MAIR
    msr     mair_el1, x2

    ldr     x2, =BOOT_TCR
    mrs     x3, id_aa64mmfr0_el1
    and     x3, x3, #0xf                // PARange, per-core; claiming more than we have is UB
    orr     x2, x2, x3, lsl #32
    msr     tcr_el1, x2

    // Both registers, same table, same reason as core 0 (header fact 2).
    msr     ttbr0_el1, x0
    msr     ttbr1_el1, x0

    dsb     sy
    isb
    tlbi    vmalle1
    dsb     ish
    isb

    mrs     x2, sctlr_el1
    orr     x2, x2, #(1 << 0)           // M: MMU
    orr     x2, x2, #(1 << 2)           // C: data cache
    orr     x2, x2, #(1 << 12)          // I: instruction cache
    msr     sctlr_el1, x2
    isb

    // Paging on. The high-VA stack top in x19 resolves now (TTBR1 maps the kernel image, and
    // the coarse boot map covers all of low RAM where the image lives).
    mov     sp, x19

    // This core's id is MPIDR affinity 0: QEMU `virt` numbers cores 0..N there.
    mrs     x0, mpidr_el1
    and     x0, x0, #0xff

    ldr     x1, =secondary_main         // the HIGH entry, x0 = cpu id
    br      x1
    .cfi_endproc
.size secondary_boot, . - secondary_boot

// The boot page tables. In .bss, so the zeroing loop above clears them for free.
.section ".bss", "aw", @nobits
.balign 4096
boot_l0:
    .skip 4096
boot_l1:
    .skip 4096

// The exception level core 0 was entered at, as a plain number (1 or 2), written by `_boot_el1`
// once .bss is zeroed. Read from Rust by `arch::aarch64::entry_el`.
.balign 8
.global boot_entry_el
boot_entry_el:
    .skip 8
