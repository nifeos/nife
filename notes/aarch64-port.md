# The aarch64 port: what `kernel/src/arch/aarch64/` decides

aarch64 is the first architecture, so it has no port story the way [riscv-port.md](riscv-port.md)
and [x86-port.md](x86-port.md) do. This note holds the design behind the files in
`kernel/src/arch/aarch64/` that no subsystem note already carries. Their doc comments link the
section that applies.

Most of the design lives elsewhere, and this note does not repeat it:

- The instruction set, exception levels and the target triple: [aarch64.md](aarch64.md).
- The vector table, the trap frame, `brk` against `svc`: [exceptions.md](exceptions.md).
- The GIC, the tick and its re-arm grid, interrupts as messages: [interrupts.md](interrupts.md).
- Page tables, the two TTBRs, turning the MMU on: [mmu.md](mmu.md), [page-tables.md](page-tables.md),
  [higher-half.md](higher-half.md), [address-space-identifiers.md](address-space-identifiers.md).
- The cycle counter and who may read it: [pmu.md](pmu.md).
- The SMMUv3: [iommu.md](iommu.md).
- Why the virtual timer and not the physical one: [virtualization.md](virtualization.md).
- `TPIDR_EL0`, the thread pointer: [thread-pointer.md](thread-pointer.md).
- Semihosting: [semihosting.md](semihosting.md). `wfi` against `wfe`: [qemu.md](qemu.md).

## The Generic Timer

The timer is part of the CPU, reached through system registers, so it lives in `arch/` and not
`drivers/`. Every core has one, and nothing in the device tree is needed except which interrupt it
raises. That is why an aarch64 kernel has a clock before it can enumerate a peripheral.

| register | what it is |
|---|---|
| `CNTFRQ_EL0` | the counter's rate. Set by firmware, read by the kernel. QEMU states 62.5 MHz. |
| `CNTVCT_EL0` | the virtual counter: 64 bits, only ever increasing, never stopped by a mask |
| `CNTV_CVAL_EL0` | an absolute deadline: fire when `CNTVCT_EL0` reaches it |
| `CNTV_TVAL_EL0` | a relative countdown, `CVAL = now + N`. Not used, for the reason below. |
| `CNTV_CTL_EL0` | enable, mask, and the read-only `ISTATUS` |

The counter does not wrap on any timescale that matters (64 bits at 62.5 MHz is about 9,000
years), and it counts while interrupts are masked. That makes it the clock for uptime and for
timing a critical section, where a tick count would undercount every missed tick.

The timer is one-shot. Once it fires it holds the line high until the handler writes a new
deadline, and that write is what lowers it. The handler re-arms from `CVAL` as `previous +
interval`, so the deadlines sit on a grid anchored at boot and a slow handler makes one tick late
rather than every tick after it. When the next deadline is already past, the handler counts a
missed tick and re-anchors to now instead of firing in a loop. The measurement that chose `CVAL`
over `TVAL` is in interrupts.md, "The bug we shipped and then measured".

The tick is per core, a banked PPI, and so is its count. A test that reads the count either side
of a wait names the core (`ticks_on`), because a kernel thread can be stolen by an idle core at any
preemption point (§28.3) and two reads on two cores compare unrelated counters.

### The counter's rate is checked against the device tree

`CNTFRQ_EL0` is architected and EL0 reads the same register the kernel does, so this architecture
needs no timebase page. Its weakness is that firmware writes it. The `arm,armv8-timer` binding
carries an optional `clock-frequency` override for boards where firmware gets it wrong, and Linux
prefers that override to the register.

This kernel refuses the boot when the two disagree, rather than preferring either (calef,
2026-09-21 UTC): a wrong rate scales every measurement silently, and two sources that disagree mean
nobody knows which is right. A board with a bad register and a good tree is the argument for
preferring the tree, and it can be made with that board's numbers.

QEMU's `virt` states no `clock-frequency` (measured 2026-09-21 with `-machine virt,dumpdtb=`), so
every machine this kernel is tested on takes the absent-property path. argon, the Jetson TX1, is
the first that can exercise the comparison.

### What the timer tests can and cannot claim

The emulator raises the timer from its own main loop, as much as 86 ms after the deadline
(measured 2026-09-24). Tests that need a tick wait for it, bounded by a second
(`RAISE_BOUND_PERIODS`), rather than spinning a fixed number of periods. The bound is a leak trap,
not a timing claim.

`ticks_arrive_at_the_configured_rate` asserts the re-arm law: across a window with no missed tick,
`CNTV_CVAL_EL0` moves by exactly one interval per delivered tick. Re-arming with `TVAL` fails it on
the first tick, and a descheduled emulator cannot, because a deschedule long enough to slip the
grid is counted as a miss and the window is retried. When eight windows in a row hold a miss the
test reports `UNMEASURED` and skips rather than failing: from inside the guest a contended host and
a slow handler are the same observation. `script/icount` asserts the handler's length in
instructions, which the host cannot move. The disposition and its measurements are in
[load-sensitive-assertions/timer-assertion-disposition.md](load-sensitive-assertions/timer-assertion-disposition.md).

## Masking interrupts: `PSTATE.DAIF`

| bit | name | masks |
|---|---|---|
| 9 | `D` | debug exceptions |
| 8 | `A` | SError |
| 7 | `I` | IRQ |
| 6 | `F` | FIQ |

A set bit masks. `msr daifset` and `msr daifclr` take a 4-bit immediate in which bit 1 is `I`, so
masking IRQs is `msr daifset, #2`, one instruction that touches only the bits it names and so has
no read-modify-write window. The kernel touches only `I`; FIQ and SError stay as the boot protocol
left them.

## The GICv3 CPU interface

On a GICv2 the CPU interface is memory (`drivers/gic.rs`). On a GICv3 it is system registers, so its
instructions live in `arch/aarch64/gic_cpu_interface.rs` by §4 rule 1. The distributor and
redistributors are memory, in `drivers/gicv3.rs`. `arch/aarch64/irq.rs` is the one place that holds
both versions, so every caller names `arch::irq` and never a driver.

| register | what it does | the failure if skipped |
|---|---|---|
| `ICC_SRE_EL1.SRE` | selects the system-register interface over the legacy MMIO one | every other `ICC_*` access is UNDEFINED or goes nowhere |
| `ICC_PMR_EL1` | priority mask; `0xff` lets everything through | a mask of 0, a common reset value, delivers nothing |
| `ICC_BPR1_EL1` | binary point; 0 restores the reset grouping | firmware can leave it wide enough to stop preemption between priorities |
| `ICC_CTLR_EL1.EOImode` | 0: one EOI write drops priority and deactivates | mode 1 without `ICC_DIR_EL1` leaves every interrupt active |
| `ICC_AP1R0_EL1` | active-priority bits, for firmware that left some set | a stale bit masks everything at or below its priority |
| `ICC_IGRPEN1_EL1` | Group 1 on, at this core | the distributor signals and the core never listens |

A system-register write is not a memory access, so neither program order nor a `dmb` orders it
against memory; an `isb` makes its effect visible to the instructions after it. The sequence is
Linux's (v6.16), and each site cites the function it comes from:

- `ICC_SRE_EL1` write, `isb`, read back (`gic_enable_sre`). If the bit did not stick, EL2 has the
  interface disabled; Linux prints "panic ahead", and this kernel panics.
- `isb` before `ICC_IGRPEN1_EL1` and after it (`gic_cpu_sys_reg_init`, `gic_write_grpen1`), so the
  mask and the control word are in effect before the group is switched on.
- `dsb sy` after reading `ICC_IAR1_EL1` (`gic_read_iar_common`), so the acknowledge completes
  before the handler's own device accesses.
- `isb` after `ICC_EOIR1_EL1` (`gic_eoi_irq`).
- `dsb ishst` before `ICC_SGI1R_EL1` and `isb` after (`gic_ipi_send_mask`). The reschedule SGI
  tells another core to read an inbox this core just wrote, and without the `dsb` the SGI can
  overtake the stores. The GICv2 path has the same question with a Device store in place of the
  `msr`, and does not answer it; `drivers/gic.rs`'s BUGS records that.

The register names in the `asm!` are the assembler's own (`icc_pmr_el1`); LLVM knows them without a
target feature, so no `S3_0_C12_...` encodings appear. Why the split between files falls where it
does is in design/roadmap/0227-gicv3-driver.md.

### Which GIC, and where its lines go

`irq::init` decides the version once, on the boot core, from the device tree's `compatible`. It then
asks the hardware whether the tree is right (`Gic::confirm`, on the distributor's revision for a
GICv3 and the CPU interface's for a GICv2) before any configuration write, and a mismatch panics
naming both sides. A GICv3 driven as a GICv2 boots, prints `interrupts ON` and takes nothing; the
check turns that into a boot that stops and says why. The version is one byte, written before any
interrupt is unmasked, so the IRQ path reads it relaxed.

Device lines (SPIs) are spread over the online cores round robin, and each line's target is
assigned once and reused. The IRQ handler masks a routed line when it fires and the driver's ACK
re-enables it, so a target chosen on every enable would move the line on every completion. The
cursor counts online cores, not indices, so a line never targets a parked core.

## Floating point and SIMD

`CPACR_EL1.FPEN` decides whether FP and Advanced SIMD instructions execute or trap. `0b00` traps at
EL0 and EL1, `0b11` at neither, and those are the only two values the kernel writes. The register is
banked per core and resets to an UNKNOWN value, so every core writes it at init. QEMU resets it to
zero, which hides a part that reset it open: every thread would run with FP enabled, no first-use
trap would fire, and two threads would share one register file with the emulator green.

aarch64 has no dirty bit. RISC-V's `sstatus.FS` says in hardware whether the registers changed
since they were loaded; here the question is coarser, whether this thread has ever executed an FP
instruction. `FpState::live` records it, the first-use trap sets it, and it never clears.

The first-use trap serves EL1 as well as EL0. The kernel is built `softfloat`, so nothing in a
shipping build reaches it from EL1. A kernel thread that asks for the unit on purpose does, and that
is how the save path is tested while every userspace target is soft-float.

`init` also closes `CPACR_EL1.ZEN` and `CPACR_EL1.SMEN`, the SVE and SME enables. The kernel saves
neither register set, and their reset value is UNKNOWN, so a part that reset them open would let a
thread keep `Z`, `P` or `ZA` state that nobody saves or scrubs. On a part without the extension the
bits are RES0 and the write changes nothing.

## PSCI and the entry exception level

PSCI is the firmware call standard for turning ARM cores on and off, and this kernel makes two
calls: `CPU_ON` to start a secondary and `SYSTEM_RESET` to reboot. Both go on the conduit `/psci`
states, `hvc` when firmware listens at EL2 and `smc` at EL3. Choosing the wrong one is an
undefined-instruction trap rather than an error code, so it is read, never defaulted. `/psci` also
publishes the `CPU_ON` id, which a PSCI 0.1 machine had to because the standard id space did not
exist yet.

The conduit follows the exception level firmware entered at. `boot.s` drops to EL1 when entered at
EL2, which is where U-Boot enters a payload, and records which it was. A machine that runs the
kernel below an EL2 states `smc`, because an `hvc` from EL1 would land in an EL2 with no vector
table; a machine that enters at EL1 states `hvc`. `isa`'s test asserts that pairing rather than a
constant. The boot banner prints the entry level first.

The PSCI facts are three atomics rather than a field behind the ISA lock: `cpu_start` runs on the
bring-up path with interrupts enabled, and the facts are written once before any secondary exists.

## The per-CPU block and the current thread

`TPIDR_EL1` is a register the architecture reserves for software, and the kernel keeps each core's
`PerCpu` pointer in it, as Linux does. The trap frame never saves it, so a thread that migrates
reads whichever core it is on.

Reading the current thread is two steps, the pointer and then the field, and a kernel thread
preempted between them resumes holding another core's block. riscv64 and `x86_64` close that with
one load relative to the per-CPU register. aarch64 has no addressing mode relative to a system
register, so `read_current_thread` masks IRQs across the two instructions and restores `DAIF` whole.
Linux keeps `current` in `SP_EL0` instead, which would mean swapping that register on every trap.

It is one `asm!` block of five instructions rather than `interrupts::disable` and a load, because
the icount gate measures an opt-level 0 kernel where those are five calls that never inline. Built
that way it cost `ipc_rtt` 13,875 ticks per 1,000 round trips
([benchmarks/drift-since-the-2026-10-10-floor.md](benchmarks/drift-since-the-2026-10-10-floor.md)).

## Asking the part what it is

ARM never removed the CPU's self-description. `MIDR_EL1` names the part and the `ID_AA64*` space
says what it can do, architected, mandatory and readable at EL1, so ISA discovery reads three
registers and parses no firmware. `machine_discovery::aarch64` decodes them, host-tested against
words real parts report. The boot refuses a part with no 4 KiB granule or a reserved `ASIDBits`
encoding: both are unreachable on a conforming part and silent if unchecked. The 4 KiB granule the
page tables use, the ASID width `crates/address_space_identifier` relies on, and the `PARange`
`mmu::init` writes to `TCR_EL1.IPS` all come from that one record.

One aarch64 fact is in no ID register and could not be: the PSCI conduit, which depends on what
runs above the kernel. That is the one thing `isa::init` reads from the device tree.

---

*Name: provisional. Named for its siblings `riscv-port.md` and `x86-port.md`.*
