---
status: BUILT
raised: 2026-09-05
built: 2026-09-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 263. Can a userspace process hold a timer, on all three architectures?

Answered 2026-09-05, the day it was minted. Minted by calef as a spike under
his decision that a timed wait should be served by a userspace timer service rather than by a new
kernel blocking primitive. The deliverable is `notes/timer-capability.md` and this block, and both
are built; no code was, and nothing is decided. *(Number provisional until the merge queue lands it.)*

Reading three specifications and pricing one kernel addition. No hardware, no
decision waiting.

## The answer

No, and therefore the userspace-timer-service answer to milestone 106 does not survive
[§19 (architectural)](../decisions/0019-architectural-parity.md) parity. Two clauses, and both are needed:

1. On riscv64 no *architected* timer can be granted to U-mode, on any machine, by the privileged
   architecture rather than by a gap in this kernel. Three independent closures, below.
2. The one mechanism that works on all three is per-board MMIO, and it is not uniformly
   available. Both real boards have generous spare timer blocks (argon fourteen channels, radon four,
   each channel with its own interrupt line), but QEMU's aarch64 `virt` has no MMIO timer device at
   all. And QEMU is what every gate in this tree runs on. A userspace timer service would be three
   different drivers and a hole, against one syscall method.

And the block's aarch64 negative was wrong, which is the correction worth having: aarch64 *can*
grant a comparator to one thread rather than to all of EL0. The machinery to do it was built
in this tree three days ago for a different reason.

## Why a spike, and the decision it sits under

Both moved on 2026-10-12 (UTC) into
[`background.md`](0263-can-a-timer-be-a-capability/background.md) under §212 (a prose budget): why
this was priced as a spike, with milestone 106's own precedent, and calef's decision on 106's fork
that it sits under.

## The problem this spike exists to settle

A userspace timer service needs a timer of its own, because it cannot use a timed wait to
implement a timed wait. seL4's driver holds the hardware timer's registers and interrupt, programs
it for the next deadline, and blocks on that interrupt.

On two of three architectures there appears to be nothing to hold, from this tree's own headers
rather than from the specifications, which is exactly what the spike must correct or confirm:

| ISA | what the kernel's tick uses | why it may not be grantable |
|---|---|---|
| aarch64 | the ARM Generic Timer | `kernel/src/arch/aarch64/timer.rs`: *"It is not an MMIO device. It is part of the CPU, reached through system registers... There is no base [address]."* `CNTKCTL_EL1` can open EL0 access, but that is ambient across EL0 rather than held by one process |
| riscv64 | SBI TIME (`sbi_set_timer`, an `ecall` to OpenSBI in M-mode) | one timer per hart, write-only, and U-mode cannot issue SBI calls at all: only S-mode can |
| x86_64 | the local APIC timer | the exception. HPET exists with multiple comparators in MMIO, so a spare grantable timer plausibly exists here |

Corrected against the specifications, 2026-09-05. The table above is left as written (`BUGS` says
why). This is what the specifications say instead, and the working is in
[`notes/timer-capability/specifications.md`](../../notes/timer-capability/specifications.md):

| ISA | the block said | the specification says |
|---|---|---|
| aarch64 | ambient across EL0, not holdable by one process | wrong on both halves. `CNTKCTL_EL1` has four independent enables, and this kernel writes exactly one of them (`EL0VCTEN`, bit 1). `EL0PTEN` (bit 9) opens `CNTP_CTL_EL0`, `CNTP_CVAL_EL0` and `CNTP_TVAL_EL0` to EL0; `EL0VTEN` (bit 8) opens the virtual set. And the bit is an EL1 register the scheduler may rewrite per thread, which is exactly what milestones 229 and 237 built for `PMUSERENR_EL0`. The real scarcity is comparators, not authority: there is one spare (`CNTP_*`), and it traps under a hypervisor, which is why the tick took the virtual one at milestone 9 (virtio) |
| riscv64 | no U-mode path | confirmed, and the negative is stronger than the block claimed. Three independent closures: Sstc's every enable (`mcounteren.TM`, `menvcfg.STCE`, `henvcfg`) gates S-mode and VS-mode and there is no U-mode timer-compare CSR to enable; `stimecmp` is CSR `0x14D`, whose bits `[9:8]` are the privileged spec's own encoding of "supervisor", so a U-mode write is an illegal instruction by the address alone; and a U-mode `ecall` raises cause 8, a different cause from the S-mode ecall the SBI dispatch decodes. One `mtimecmp` per hart, spent on the tick |
| all three | (not considered) | the spare-MMIO-timer route the block did not name, and it needs no new kernel mechanism, since a `DeviceFrame` plus an `Object::Irq` is how every userspace driver here owns a device. argon's Tegra X1 has `timer@60005000`, fourteen 29-bit one-shot-capable channels on fourteen GIC SPIs; radon's JH7110 has `si5_timer` at `0x13050000`, four channels on four PLIC lines (no mainline driver; the binding was posted ten times and never merged). QEMU aarch64 `virt` has none (`hw/arm/virt.c`'s memmap has a PL031 whose alarm is one-second resolution, and a watchdog that is not created by default), and QEMU riscv64 `virt` has one, the goldfish RTC's nanosecond alarm, which this tree drives for the wall clock. Good answer for one workload on one board; not a portable capability |
| x86_64 | plausibly a spare exists | plausible and still unconfirmed on the machine this project owns. The `HPET` table is in the tree's own q35 transcript (`notes/x86-port/acpi-and-pci.md`) and `crates/machine_discovery` sees it and does nothing with it. xenon's is not recorded anywhere: `notes/x86-uefi-boot.md` says first light printed "the full table list" and the transcript is a photograph. Nobody has read this project's own `NUM_TIM_CAP` |

[§19](../decisions/0019-architectural-parity.md) makes parity a gate, so an answer that works on
x86_64 alone is not an answer.

## What the spike must produce

1. Confirm or refute the two negatives from the specifications, not from this tree's doc
   comments. Can aarch64 grant timer access per thread rather than to all of EL0? Is there
   genuinely no U-mode path to arm a timer on riscv64, including Sstc's `stimecmp`?
2. Price the fourth shape, below, in the kernel. The pricing lane measured that any
   deadline structure costs one comparison per tick (1.000 comparisons and 0.000 writes over
   100,000 idle ticks, for a scan and for a sorted list). So what is unpriced is the object, its
   methods and the signaling, not the bookkeeping.
3. Say plainly whether the userspace-service answer survives parity. If it does not, an
   architect's decision changes, and it is better to know that in a day than three days into a lane.

## The fourth shape, which neither milestone 51 (wall) nor 106 lists

If the kernel must own the timer on two of three architectures, the service cannot hold one, but the
kernel can signal on its behalf:

```
Timer::ARM(deadline, notification) -> the kernel signals that notification at the deadline
```

A thread then blocks in `RECV` on its endpoint with the notification bound to its TCB
([§101](../decisions/0101-notification-objects.md), decided 2026-08-20, unbuilt, milestone 151 (notification)), and
wakes on either a message or the deadline. That is milestone 106's own title met.

It is smaller than any of milestone 51's three shapes. Not "block until a deadline", only "signal
this at T". `RECV` keeps its signature, nothing becomes ambient, and the authority to wait on time is
a capability like everything else. It is close to what seL4 does, with the timer's ownership
moved to the only place two of our three architectures allow.

This block does not recommend it. It is named so the spike prices it beside the userspace
service, and because a fork should not be settled by discovering an option late.

## BUGS

- Three headers are not three specifications. The table above is read from this tree's own
  comments, which is how the spike was scoped and is not evidence about the hardware. Item 1 exists
  because the maintainer's reading is the thing most likely to be wrong here.
- It prices nothing about a timer service itself, only whether one can hold a timer. If the
  answer is yes on all three, the service is still unbuilt, unpriced, and unnamed.
- The fourth shape is a syscall-surface addition whatever its size, so it is an architect's
  under §10 (process) and this spike may only measure it.
- Milestone 151 is unbuilt, so every option here that composes with a notification composes with
  something that does not exist yet, and the spike should say what it assumes about it.

## What the fourth shape costs, measured

Built as a throwaway scaffold on 2026-09-05, gated green with `script/test` (205 passed, 69 skipped,
all three ISAs plus the OVMF leg), and deleted. That is milestone 106's own pricing method and
its reason: shipping it would settle a syscall-surface fork by accident. Method and error bars:
[`notes/timer-capability/first-scaffold.md`](../../notes/timer-capability/first-scaffold.md#method) and
[`notes/timer-capability/second-scaffold.md`](../../notes/timer-capability/second-scaffold.md#method-and-error-bars-both-scaffolds).

| what | figure |
|---|---|
| the `Object` variant | **free.** `size_of::<Object>() == 24` and `size_of::<Cap>() == 32` still hold with `Timer(u64)` added, because `PageFrame(u64, NonZeroU64)` is the widest arm. Zero bytes per slot, per capability table, per TCB |
| `ipc_fastpath` | **unchanged on all three ISAs** (7,028 / 5,936 / 8,122). An IPC round trip fetches what it fetched before |
| `syscall_entry`, flat | aarch64 **+12 B** (+0.8%), riscv64 **+158 B** (+8.6%), x86_64 **+96 B** (+5.9%). One more `Object` arm in `invoke`, measured with `#[inline(never)]` already applied to both method bodies |
| kernel symbol bytes | aarch64 **+738 B** (+0.073%), riscv64 **+2,266 B** (+0.224%), x86_64 **+1,386 B** (+0.144%) |
| per-tick bookkeeping | **not re-measured.** `notes/timed-wait.md` already has it: one comparison per idle tick, +30/+31 instructions, and a per-thread `deadline: u64` at zero bytes |

**What it assumes about milestone 151, which is item 4 of the brief and is a finding rather than a
footnote.** The scaffold signaled a `Rendezvous`, because that is what exists; §101's whole argument
is that a notification is *its own object*. If 151 builds that, the byte figures barely move and the
syscall's meaning does. More importantly: **the fourth shape is not independently useful.** Milestone
106's title ("wake on either a message or a deadline") is met only by §101's TCB binding. Without
151, this buys a thread a timer it can block on and no way to block on a timer *and* a message at once.
That is 106's actual complaint. The fourth shape is 151 plus one object, and it should be priced that
way.

## The two prices the spike did not pay, measured 2026-09-13

The table above left two things unpriced, and its own `BUGS` named the first. A second scaffold paid
both: four kernels differing by one change each, built release on all three ISAs, measured with
`llvm-nm --print-size`, and deleted. The error bar is zero bytes (a build was reverted and
rebuilt from its patch and came out byte-identical). Full method in
[`notes/timer-capability/second-scaffold.md`](../../notes/timer-capability/second-scaffold.md#method-and-error-bars-both-scaffolds).

| what | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| the holder dies with a timer armed | **+268 B** code, 0 data | +212 B, 0 | +276 B, 0 |
| serving a kernel thread | **+312 B** code, 0 data | +282 B, 0 | +400 B, 0 |

**The holder-death case is not bookkeeping, it is a use-after-free on the timer interrupt**, and
that is the correction worth carrying. The expiry walk reads and writes a registry entry's page on
every tick, so a timer page freed by `MemoryRegion::DESTROY` is read and written by `on_tick` on
every core for the life of the machine. The fix is the sweep `reap_region_objects` already runs for
`rendezvous_table`, one phase earlier, which is why it is 268 bytes. Three things it does **not**
need, because the tree already has them: a stale signal target is dropped by `irq_notify`'s existing
generational check. The cached earliest deadline may be early and never late, so it need not be
recomputed. And the registry slot returns with the page, closing a spawn-and-die slot-exhaustion
attack as a side effect.

**And the kernel thread cannot be served by the fourth shape either**, which is the finding this
block did not have on 2026-09-05 because milestone 106's consumer census had not been run. The
reason is not architectural: a kernel thread runs at EL1 and cannot issue a syscall at all, so
`Timer::ARM` is as unreachable to it as a userspace timer service is. Not for want of a cspace,
which is the thing a reader expects to be missing and is not (`Thread::spawn_into` gives every
kernel thread an empty `CapabilityTable`). What it needs is *smaller* than the fourth shape, because
inside the kernel authority is not the question: a `sleep_until` that writes a deadline word on the
TCB and parks, plus a second loop in the same expiry walk. The word is free (`size_of::<Thread>()`
is 1,152 bytes in a 4,096-byte page, 2,944 spare), the walk shares the cached earliest, and an idle
tick still costs one comparison. Applied to the real consumer, `soak.rs`'s six-line yield loop
becomes one call and the kernel builds clean with `--features soak`.

**Whether that consumer should be served is an architect's**, under §101 (notification)'s carve-out
and milestone 106 (deadline)'s own "this stays owed against a kernel-side consumer appearing". This
block only says what it costs.

## Follow-on

- **Milestone 106.** `design/decisions/0147-a-timer-a-userspace-service-cannot-hold.md` is
  decided (calef, 2026-09-05: *"option 1, the new object. ... A deadline argument seems
  like a work around to reduce effort."*), so the fork this spike was minted to force is closed and
  the lines below are the history of how it was put rather than a live question. What remains owed
  is the build, which is 106's, and the two prices above are part of what it costs. The options as
  they were presented:
  1. The fourth shape (`Timer::ARM(deadline, notification)`), priced above and cheap. The kernel
     owns the comparator on every architecture, which is where two of three put it anyway, and the
     authority to wait on time stays a capability. Costs a syscall-surface addition and depends on
     milestone 151.
  2. A userspace service on aarch64 and x86_64 only, with riscv64 carrying a scope note under §19.
     This is the option §19 exists to make expensive, and it is worse than it looks: the riscv64 gap
     is permanent rather than a port that has not been done.
  3. Reopen milestone 51's three shapes, which this spike does not price and `notes/timed-wait.md`
     does.
- **Milestone 151.** Unblocked in the sense that it is now on the critical path rather than beside it:
  every surviving option above composes with a notification object, so 151 is a prerequisite of the
  answer rather than an enhancement to it.
- **Done.** **§147's text was damaged by its own `PROPOSED` -> `DECIDED` edit, and the maintainer
  repaired it the same day this lane reported it (#830).** Kept here because what was found is worth more
  than that it is fixed. Three things, all in the first thirty lines: a mangled clause, *"Milestone
  151 (notification objects)built) must land first"*. An orphaned sentence fragment, *"invalidates
  the premise of a decision calef made the same day."*, left behind when the `PROPOSED` preamble
  above it was replaced. And, the substantive one, a `**No recommendation.**` paragraph and a whole
  `## What this file does not do` section arguing that the file deliberately names no winner, still
  standing underneath a header that names one. So a reader was told both that option 1 was chosen
  and that choosing was not this file's job. That is the cheap-edit-destroys-the-expensive-record
  shape AGENTS.md's *move fast on what can be undone* names, one file over from where it names it.
  A lane may not repair a decision section, and this one correctly did not.
- **Recorded.** **Adding a field to `IpcTables` costs riscv64 roughly the struct again**, and
  nothing in this tree says so. Measured while pricing the registry above. Growing `IpcTables` from
  15,432 to 25,696 bytes made that ISA materialize a 25,680-byte anonymous `.rodata` template of
  `EMPTY_TABLES` that it did not carry before and that aarch64 and x86_64 do not carry at all. So
  the same source costs riscv64 +37,061 data bytes against aarch64's +10,980. It is a cliff rather
  than a slope, it prices every future addition to the scheduler's tables rather than anything about
  timers. And it is left unactioned because the right response may be a smaller registry rather than
  a fight with the backend.
  [`notes/timer-capability/second-scaffold.md`](../../notes/timer-capability/second-scaffold.md#one-finding-that-is-not-about-timers-at-all)
  has the symbol-level working.
- **Recorded.** `design/decisions/0139-cycle-counter-authority.md` says *"There is no precedent in this
  tree for a per-thread system-register bit maintained across a context switch."* Milestones 229 and
  237 built one, so that sentence is stale inside its own decision, in the same way §102's per-slot
  arithmetic went stale inside §102. Amending a `DECISIONS` section is not a lane's (AGENTS.md), so it
  is recorded here and in
  [`notes/timer-capability/specifications.md`](../../notes/timer-capability/specifications.md#aarch64-the-blocks-negative-is-refuted) for
  that section's next owner.
- **Recorded.** xenon's HPET. Nothing in this tree records whether the machine this project owns
  has one, how many comparators, or whether the legacy replacement route is on. It needs the
  first-light photograph transcribed or the boot tour re-run with the table list captured, and it
  decides nothing until option 2 above is live.
- **Recorded.** The aarch64 refutation is a specification reading, not an experiment. Setting
  `CNTKCTL_EL1.EL0PTEN`, having an EL0 program write `CNTP_CVAL_EL0`, and seeing INTID 30 arrive is
  perhaps an hour and would turn it into a measurement. It was not run.

## Index row

answered 2026-09-05, the day it was minted. **No on riscv64, architecturally**, so the
userspace-timer-service answer to 106 does not survive §19 and the fork goes back to calef as a
PROPOSED decision. aarch64's negative was **refuted**: `CNTKCTL_EL1.EL0PTEN` opens the physical
comparator to EL0, and milestones 229/237 built the per-thread grant §139 said the tree lacked.
The fourth shape priced at zero bytes per capability, no `ipc_fastpath` movement, and under 0.25%
of the kernel. notes/timer-capability.md
