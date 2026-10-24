# What each specification actually says

*Name: provisional. After the parent's "What each specification actually says". calef names files.*

An appendix of [can a userspace process hold a timer?](../timer-capability.md). The parent keeps the
verdict per architecture. This file keeps the working: the register fields, the specification text
quoted, and the tree's own code each verdict rests on. It is the "full working, with the
specification citations" that [§147 (a timer a userspace service cannot
hold)](../../design/decisions/0147-a-timer-a-userspace-service-cannot-hold.md) points at. It
describes the tree of 2026-09-05.

Three specifications were read rather than recalled. Milestone 263 (can a userspace process hold a
timer)'s own `BUGS` section says the table it was scoped from is this tree's doc comments, and *"the
maintainer's reading is the thing most likely to be wrong here."* It was wrong about one of three.

## aarch64: the block's negative is refuted

The block says `CNTKCTL_EL1` "can open EL0 access, but that is ambient across EL0 rather than held
by one process." Both halves need correcting.

The register has four independent enables, not one, and two of them are about the comparators
rather than the counters. The source is Arm's own machine-readable system-register description of
`CNTKCTL_EL1`, rendered at `arm.jonpalmisc.com/latest_sysreg/AArch64-cntkctl_el1` and read
2026-09-05. The normative text is the Arm Architecture Reference Manual for A-profile,
`CNTKCTL_EL1`.

| bit | field | what it opens to EL0 |
|---|---|---|
| 0 | `EL0PCTEN` | `CNTPCT_EL0`, `CNTPCTSS_EL0` (and conditionally `CNTFRQ_EL0`) |
| 1 | `EL0VCTEN` | `CNTVCT_EL0`, `CNTVCTSS_EL0` (and conditionally `CNTFRQ_EL0`) |
| 8 | `EL0VTEN` | `CNTV_CTL_EL0`, `CNTV_CVAL_EL0`, `CNTV_TVAL_EL0` |
| 9 | `EL0PTEN` | `CNTP_CTL_EL0`, `CNTP_CVAL_EL0`, `CNTP_TVAL_EL0` |

Of `EL0PTEN`: *"Traps EL0 accesses to the physical timer registers to EL1, or to EL2 when it is
implemented and enabled"*, and when the bit is 1, *"this control does not cause any instructions to
be trapped."* `EL0VTEN` says the same of the virtual timer registers.

This kernel writes only bit 1. `kernel/src/arch/aarch64/timer.rs`'s `init` sets `EL0VCTEN` and
nothing else. That is why the tree's own comments describe the timer registers as trapped: they
are, because the bit that would open them has never been written. The registers exist and are
grantable; this tree has never granted them.

And the grant can be per thread, because the tree now has the machinery.
`design/decisions/0139-cycle-counter-authority.md` says, in its "what this tree already does"
section, that `CNTKCTL_EL1.EL0VCTEN` and `scounteren.TM` are *"per-machine bits, set once at init.
There is no precedent in this tree for a per-thread system-register bit maintained across a context
switch. That is the one piece of machinery option 4 below needs and the tree does not have."*

That sentence is now stale, and its own decision is what made it stale. Milestones 229 and 237 built
exactly that. A `cycle_counter_grant` bool on `Thread` is read in `sched::schedule` at a
`#[cfg(any(test, feature = "cycle_counter_grant"))]`-gated switch site. (Milestone 300 (decompose
the icount baseline drift) removed the const-`false` helper `cycle_counter_grant_of` that used to
carry it through the shipping switch tuple.) It is installed on the core about to run the thread by
`install_cycle_counter_grant` -> `arch::timer::set_cycle_counter_grant`. That is a cached
compare-and-skip around a `PMUSERENR_EL0` write on aarch64 and an `scounteren` write on riscv64. A
per-thread `CNTKCTL_EL1.EL0PTEN` grant is the same shape at the same call site, on a register two
lines away in the same file. The precedent §139 (who may read the cycle counter) wanted exists.

What is genuinely scarce is comparators, not authority. At EL1/EL0 aarch64 offers two: the physical
timer (`CNTP_*`, GIC INTID 30) and the virtual timer (`CNTV_*`, INTID 27). This kernel's tick owns
the virtual one, and owns it *because* of a portability finding this tree paid for. The physical
timer *"traps under a hypervisor: the physical timer belongs to EL2, and a guest at EL1 that writes
`CNTP_CVAL_EL0` takes an 'Unknown reason' trap"*. That was found on Apple's Hypervisor.framework
(`kernel/src/arch/aarch64/timer.rs`, milestone 9 (a virtio-blk driver at EL0)). So:

- There is exactly one spare comparator on aarch64, the physical timer.
- It is spare on bare metal and under QEMU/TCG, and not under a hypervisor. That is the same
  finding that pushed the tick onto the virtual timer in the first place. A grant that works on
  argon and fails under HVF is a parity problem of its own kind.
- It is per PE, so a service holding it is pinned to a core, or its deadline follows it.

## riscv64: the block's negative is confirmed, and it is total

Three routes, all closed.

Sstc's `stimecmp` is S-mode, and U-mode is not mentioned because it was never a candidate. From the
ratified *"Sstc" Extension for Supervisor-mode Timer Interrupts, Version 1.0* (RISC-V International;
the ratified PDF at `docs.riscv.org/reference/isa/extensions/sstc/`, read 2026-09-05):

> When the TM bit in the mcounteren register is clear, attempts to access the stimecmp register while
> executing in S-mode will cause an illegal instruction exception. When this bit is set, access to
> the stimecmp register (if implemented) is permitted in S-mode.

and

> Bit 63 of menvcfg [...] named STCE (STimecmp Enable) enables stimecmp for S-mode when set to one,
> and the same bit of henvcfg enables vstimecmp for VS-mode.

Every enable in the extension gates S-mode and VS-mode. There is no U-mode enable, and no U-mode
timer-compare CSR exists anywhere in the privileged architecture to be enabled.

The CSR numbering forecloses it independently of Sstc's text. The RISC-V Privileged
Architecture's CSR address convention (*Control and Status Registers (CSRs)*, the CSR listing
chapter) says the two bits `csr[9:8]` *"encode the lowest privilege level that can access the
CSR"*. It adds that *"attempts to access a CSR without appropriate privilege level or to write a
read-only register raise illegal-instruction exceptions."* `stimecmp` is `0x14D`: bits `[9:8]` are
`0b01`, supervisor. A U-mode `csrw stimecmp` is an illegal instruction by the address it is written
at, and no configuration bit changes that.

SBI is not reachable from U-mode either. This kernel's tick arms through `sbi_set_timer`, an
`ecall` from S-mode to OpenSBI in M-mode. An `ecall` executed in U-mode raises *Environment call
from U-mode* (`scause`/`mcause` 8), a different cause from *Environment call from S-mode* (9). It is
delivered to S-mode when `medeleg` delegates it and to M-mode otherwise. In neither case does it
enter the SBI dispatch, which decodes only the S-mode and M-mode ecall causes. A U-mode program
cannot make an SBI call at all, which is exactly what the milestone block said.

The one comparator the machine has is already spent. RISC-V gives one `mtimecmp` per hart, in
M-mode's CLINT, and SBI TIME is the multiplexer over it. There is no second one, so even a
hypothetical MMIO grant of the CLINT would be handing away the kernel's own tick rather than a
spare.

## x86_64: the exception is real, but it is not confirmed on the machine this project owns

The HPET is memory-mapped and has several independent comparators. It is therefore the one timer
on the three architectures that could be handed to a process with the tree's existing
`DeviceFrame` capability and no new mechanism at all.

What is confirmed in this tree: `crates/machine_discovery/src/acpi.rs` sees an `HPET` table in the
XSDT walk and does nothing with it. `notes/x86-port/acpi-and-pci.md` records the QEMU q35 table
list read on 2026-08-23 with `0x000ffe22a8 HPET (56 bytes)` in it.

What is not confirmed: xenon's own HPET. `notes/x86-uefi-boot.md`'s first-light section
(2026-09-05) says the boot tour printed *"the full table list"*. But the transcript is a photograph
(`art/bench/xenon-2026-09-05-first-light.jpg`), and the list is not written down in the note.
Nothing in this tree names an HPET on xenon, and nothing has read its `NUM_TIM_CAP`. So the x86_64
row is "the architecture has a spare, and this project has not looked at its own machine's".

`design/decisions/0139-cycle-counter-authority/x86-clock-sources.md` already read the HPET
specification for a different purpose, and its findings apply. The architectural floor is 100 ns
per tick, the common part runs at 14.31818 MHz, and a *read* costs several microseconds, which is
why Linux deleted the vDSO mapping. None of that prices arming one, which is a write and a one-shot
interrupt rather than a polled read. So §139's conclusion (the HPET loses to a syscall as a
*clock*) does not carry over to using it as a *timer*. That distinction is worth keeping straight,
because the two uses share a device and nothing else.
