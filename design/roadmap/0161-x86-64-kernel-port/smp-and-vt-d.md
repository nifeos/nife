# Milestone 161 appendix: SMP bring-up and VT-d, items 5 and 6

An appendix to [milestone 161 (the x86_64 kernel port)](../0161-x86-64-kernel-port.md), moved out
of that page on 2026-10-12 (UTC) under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md).
The main page is complete without it; this is items 5 and 6 of the open list as they stood on
2026-08-25. Failures (1) and (3) below were closed later; [closing.md](closing.md) has how. Long
sentences were split to meet [§213 (writing standards)](../../decisions/0213-writing-standards.md);
no fact changed.

## Item 5: SMP, via INIT-SIPI-SIPI

PARTIALLY BUILT 2026-08-25. The mechanism that starts a second logical CPU was built and did what it
said. Nothing downstream of "a second core exists" had been shown safe. The standard test suite did
not exercise it by default because of that, not because the mechanism itself was unfinished. Both of
the item's own two named bugs were fixed, not inherited.

### What landed

The Interrupt Command Register sequence: `arch::x86_64::irq::send_init` and `send_startup`.

A real-mode trampoline: `boot.s`'s `secondary_boot`, now real code rather than a `hlt` stub. It is
linked at a fixed low virtual address (`AP_TRAMPOLINE_PHYS = 0x8000`, `link-x86_64.ld`'s
`.ap_trampoline`). So its own address already is the physical page a `STARTUP` IPI has to name. It
is copied there at runtime from an ordinary spot in the loaded image
(`arch::x86_64::ap_boot::prepare`). Its two natural-looking homes, `.boot_scratch` and the secondary
stacks, are both runtime-mutated and would hand back corrupted bytes.

The trampoline replays `_start`'s own 16→32→64-bit transition against the same boot page tables. It
reads its own local APIC id via `CPUID`, the same "Initial APIC ID" `arch::boot_cpu_id` now reads on
the boot core. That was fixed from a hardcoded 0, so the roster's seating and the boot core's own
logical id agree. Then it jumps to the portable `secondary_main` the other two architectures already
use.

A per-CPU TSS and GDT (`segments.rs`, indexed by `cpu::id()`) replace the single boot-CPU pair item 4
left behind. `trap.s`'s `isr_restore` and `x86_system_call_entry` reach three words per CPU:

- the system call path's kernel stack;
- its scratch for the interrupted user `rsp`;
- a pointer to this core's own `TSS.rsp0`.

They moved into `cpu::PerCpu::x86_trap`, reached through a `gs`-relative offset. Every other per-CPU
write on this architecture already works that way.

The ACPI MADT's local-APIC-id roster is read into the same `HWID`/`STARTABLE` arrays that
`smp::read_cpu_list` fills from a device tree (`smp::seat_cpus_from_acpi`). Each core, boot core
included, is seated at the slot its own local APIC id names. A single secondary, started this way,
reached `secondary_main` and its own idle loop reliably, measured across dozens of boots.

### Both named bugs are fixed

`smp::bring_up_secondaries` no longer calls `arch::mmu::virt_to_phys` on `secondary_boot`'s address
on this architecture. That address is already physical, by construction of the linker script above.
`arch::secondary_boot_entry` is the identity function here and says why.

`irq::send_reschedule` no longer uses the logical cpu id as a local APIC id directly. It looks the
real one up in the roster (`smp::hwid`) that the same MADT read built.

### What did not land: anything past "a second core idles"

Two separate failures were unresolved at the time, and a third was found later.

#### (1) A third or later secondary fails intermittently

Brought up while an earlier one was already online and running, exactly one secondary typically
failed to reach `secondary_main`'s online mark. Which one varied run to run. Instrumented with raw
port-I/O checkpoints, the failing core reached 64-bit long mode. It did not reach the last
checkpoint before jumping to `secondary_main`. Between them were five ordinary instructions,
identical to what the succeeding cores had just run.

Two direct hypotheses were tested, and neither held up.

- Routing `cpu_start`'s wait through `hlt` instead of a busy spin. The theory was that CPU 0's own
  loop starved the target vCPU thread of host scheduling time under QEMU TCG. It turned an
  occasional full hang into a reliable clean give-up, but did not fix the underlying failure.
- Copying the trampoline's code bytes only once instead of once per `STARTUP` IPI. The theory was
  that QEMU's self-modifying-code detection mishandled a rewrite-and-re-execute of a page another
  vCPU might be running from. It made no measurable difference.

Both changes were kept, on their own independent merits, but neither was the fix. The real cause,
found on 2026-09-19, was a counting bug in `cpu_start` (see [closing.md](closing.md)).

#### (2) Two cores crashed under the suite's real scheduler workload

FIXED 2026-08-25, by the lane that took this finding. The symptom was a `script/test` run at
`NIFE_SMP=2` faulting at `rip 0x0` or at `rip 0x5afe57ac5afe57ac`. The latter is `stack::PAINT`, the
pattern this kernel writes into a fresh kernel stack. It reproduced 10 times in 10.

The cause was a missing cross-core TLB shootdown. It was neither in the INIT-SIPI-SIPI mechanism nor
in the portable scheduler. `mmu::flush_tlb` on this port was local to the calling CPU, because
`invlpg` is. Its own doc comment said exactly that and predicted exactly this: "this is the line that
will need company". aarch64 needs no remote half at all, since `tlbi ..., is` is broadcast in
hardware. RISC-V has had one since
[milestone 58 (RISC-V TLB shootdown)](../0058-riscv-tlb-shootdown.md). That is why the same
`sched`/`thread` code runs clean at `-smp 4` on both.

So a dead thread's kernel-stack address range, handed back by `KernelStack::drop`, was remapped onto
different frames for the next thread. The other core still translated it to the old frame. A
`Context` read back through that stale entry is whatever the recycled frame now holds, and a freshly
recycled stack page is painted. This was confirmed before any fix was written, by making a stale
entry harmless (never reusing stack address space). The fault vanished, 8 runs of 8.

The fix is `mmu::shoot_down_others`. It is an NMI rather than an ordinary IPI, which is the one
design point worth carrying up. `unmap_page` runs inside `KERNEL_MMU`, an `IrqSafeMutex`. So the
sender and every other core running the same code have interrupts masked. A maskable IPI would
deadlock the first time two cores spawned and reaped at once. notes/riscv-tlb-shootdown.md already
names that property as load-bearing there: "a hart with S-mode interrupts masked still services it".
RISC-V gets it from M-mode, and aarch64 does not need it. On x86 the NMI is the only delivery `cli`
cannot suppress.

Verified rather than gated. `user::tests::an_asid_flush_reaches_the_other_cores` is the portable test
milestone 58 wrote for this exact property. It fails on this port without the shootdown and passes
with it. The suite reached `test result: ok. 177 passed` at two cores once (3) below was stepped
over. 20 further runs produced no paint fault.

#### (3) The suite cannot agree on which core booted

Found by the same lane while verifying (2). Not fixed by it, and a different subsystem's bug.
`arch::x86_64::boot_cpu_id` read CPUID leaf 1's initial local APIC id. That answers "which core am I"
rather than "which core booted". aarch64 answers a constant `0`, and RISC-V returns the hart id
recorded at boot.

So a test body that the placement of [§28 (SMP placement)](../../decisions/0028-smp-placement.md)
had migrated onto a secondary read that secondary's id as "the boot core".
`smp::tests::every_secondary_runs_scheduled_work` waited for a `RAN_ON` mark the real boot core never
set. `stack.rs`'s high-water report picked the slot to skip the same way. It scanned a never-painted
one and reported a secondary stack at 65536/65536.

One cause, both symptoms, about half of two-core runs. It reproduced with no shootdown code present
at all, checked against the pre-fix tree, so it predates (2)'s fix. The likely answer was a
boot-time record, which is what RISC-V's `boot_hartid` already is. Small, wanting no hardware, and
wanting a lane. [Milestone 316 (which core booted)](../0316-x86-smp-two-cores.md) took it and fixed
it on 2026-09-17.

#### Where the account lives

`arch::x86_64::ap_boot`'s own `BUGS` has the full account of all three, and stays the authoritative
record. `helpers/qemu-runner-x86_64.sh` kept `NIFE_SMP` at 1 because (1) and (3) were open and either
could fail a run. So the `smp` tests the roster work would otherwise unblock kept skipping, honestly,
until those two were answered.

Whether (1) was a QEMU TCG emulation quirk or a real bug in this port's own code was exactly the kind
of question [milestone 87 (the x86_64 bare-metal machine)](../0087-x86-machine.md)'s real hardware
would settle. It was the one still worth a lane of its own. It turned out to be this port's code.

## Item 6: VT-d, x86_64's IOMMU

BUILT 2026-08-25. It fills the same milestone-16b role the SMMUv3 and RISC-V IOMMU drivers already
fill. This time the interface is register-driven rather than memory-queue-driven. VT-d has no
command or fault queue in memory. Invalidation is a register write-and-poll (`CCMD_REG`), and faults
are read out of a small bank of Fault Recording Registers instead.

`arch::iommu::init` (`kernel/src/arch/x86_64/iommu.rs`) builds the root and context tables. It turns
translation on over an all-absent root, so every bus faults until its context table exists. That is
the same default-deny posture the other two drivers give their own table shapes. It is wired into
`main.rs`'s boot tour right after the fine page tables, and before anything could attach a device.
It covers:

- one DRHD (`machine_discovery::acpi::first_drhd`);
- translation enable and disable through `GCMD`/`GSTS`;
- register-based context-cache and IOTLB invalidation;
- fault detection through `FSTS.PPF` and the first Fault Recording Register.

### What is deliberately scoped out, per the driver's own `BUGS`

- Exactly one DRHD is brought up. A machine reporting more than one VT-d unit has devices this driver
  never sees.
- No interrupt remapping: `GCMD.IRE` is never set.
- Invalidation is always global-granularity, never domain- or device-selective.
- `RWBF` (`CAP_REG` bit 4) is honored in code but has never been exercised, since QEMU's model does
  not set it.
- The fault path decodes only the first Fault Recording Register, since QEMU reports `CAP.NFR = 0`.

At the time no PCI device was confined through it, because no virtio-pci or NVMe driver existed on
`x86_64` (item 4's own hand-off). So `main.rs` proved the driver against the hardware's own status
registers rather than against a downstream escape.
[Milestone 215 (a PCI function's interrupt reaches nothing on x86_64)](../0215-x86-64-pci-interrupt-routing.md)
later closed that gap.
