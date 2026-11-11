---
status: AMENDED
raised: 2026-09-05
decided: 2026-09-05
ratified_by: calef
---

# 147. A timer a userspace service cannot hold

(one amendment block below, from milestone 106 (a wait that ends on either the interrupt or the
deadline)'s own PR.)

calef, 2026-09-05: option 1, the new object. Option 2 was ruled out first
(*"§19 exists to make this expensive"*, and a scope note recording something that cannot be done
is not the thing a scope note is for). Between 1 and 3 his reason was one sentence: *"A new object
seems like the winner. A deadline argument seems like a work around to reduce effort."*

That is [§92 (a caretaker is supervised by the client it serves)](0092-caretaker-lifetime.md)'s test
applied, and it is worth recording as such because §92 exists for exactly this shape: *"the failure
mode is not laziness, it is a recommendation that sounds like design."* Milestone 51 (wall-clock
time, the `date` command, and an NTP service) called the deadline argument strongest on "three
problems, one addition", which counts effort. Asked §92's question, *would I still choose this if
both options were the same amount of work*, it loses on the merits:

- It changes a primitive every program uses, where a new object is opt-in. §10 (process model:
  capability-based, microkernel) calls the surface *"a boundary, not a habit"*, and an addition can
  be ignored while a changed `RECV` cannot.
- "One addition" undercounts. `net_stack` blocks on an interrupt, not an endpoint, so milestone
  106 has to add `Irq::WAIT` with a deadline as well. A notification takes IPC and IRQ signals
  alike, so option 1 needs one mechanism where this needs two.
- Sleeping becomes something you do to an endpoint, which a thread that only wants to sleep must
  then acquire.

Two of option 3's three shapes never reached the comparison, and this section says so rather than
leaving them as live alternatives. `SYS_SLEEP` is ambient, which §10 refuses by design, and answers
one of four consumers. A timer object with a blocking `WAIT` has one wake source, so it does not
meet milestone 106's own title, which is a wait ending on *either* the interrupt or the deadline.
Option 1 is that shape with the blocking removed, which is what makes it work.

The cost accepted. This is two builds rather than one. Milestone 151 (notification objects)
must land first, and is `NOT-STARTED`: without the TCB binding this buys a thread a timer and no
way to wait on a timer and a message at once. Option 1 is 151 plus one object and was priced
that way.

*(Was `PROPOSED` from 2026-09-05, written by milestone 263 (can a userspace process hold a timer,
on all three architectures)'s lane, which named no winner deliberately. It was raised because that
spike's answer invalidated the premise of a decision calef had made the same day. AGENTS.md's rule
is that an irreversible fork arrives with options rather than a recommendation. He ruled it the
same day it was written.)*

## What is being decided

Milestone 106 asks for a wait that ends on either a message or a deadline. calef decided on
2026-09-05 to serve it from a userspace timer service signaling a notification, rather than from
a new kernel blocking primitive. He minted milestone 263 to price the prerequisite: whether such a
service can hold a timer at all.

It cannot, on riscv64, on any machine. So the decision needs re-making, and this is the fork.

## Why the premise failed

The full working, with the specification citations, is
[`notes/timer-capability/specifications.md`](../../notes/timer-capability/specifications.md), and the corrected
per-architecture table is in `design/roadmap/0263-can-a-timer-be-a-capability.md`. The short form:

- riscv64: closed three independent ways. Sstc's every enable (`mcounteren.TM`, `menvcfg.STCE`,
  `henvcfg.STCE`) gates S-mode and VS-mode, and the privileged architecture contains no U-mode
  timer-compare CSR for any of them to enable. `stimecmp` is CSR `0x14D`, and the privileged spec's
  CSR address convention makes bits `[9:8]` the lowest privilege that may access it: `0b01`,
  supervisor. So a U-mode `csrw stimecmp` is an illegal instruction by its address, independently of
  Sstc's own text. And a U-mode `ecall` raises cause 8, a different cause from the S-mode ecall the
  SBI dispatch decodes, so the SBI TIME route this kernel's tick uses is unreachable from U-mode
  too. The one `mtimecmp` per hart is already spent on the tick.
- aarch64: open, and milestone 263's block was wrong to say otherwise. `CNTKCTL_EL1.EL0PTEN`
  opens `CNTP_CTL_EL0`, `CNTP_CVAL_EL0` and `CNTP_TVAL_EL0` to EL0, and the bit is an EL1 register
  the scheduler may rewrite per thread, which milestones 229 and 237 already do for `PMUSERENR_EL0`.
  The scarcity is comparators: one spare (`CNTP_*`), per PE, and it traps under a hypervisor.
- x86_64: open in principle, unverified on the machine this project owns. The HPET has several
  comparators in MMIO and would need no new kernel mechanism at all, since a `DeviceFrame` already
  delegates an MMIO page. Nothing in this tree records xenon's HPET or its comparator count. The
  IA-PC HPET Specification 1.0a's recommended minimum is three comparators, and §2.3.5's legacy
  replacement route spends two of them when it is on, so the floor is one free general-purpose
  timer rather than several.
- And there is a spare-MMIO route on both real boards, which does not rescue the option. argon's
  Tegra X1 has fourteen one-shot-capable channels on fourteen GIC SPIs; radon's JH7110 has four on
  four PLIC lines. But QEMU's aarch64 `virt` has no MMIO timer at all, and QEMU is what every gate
  in this tree runs on, so §19's *"proven by the same suite"* is not met: the service would be three
  different drivers and a hole. It is the right answer for one workload on one board and the wrong
  one for `thread::sleep`.

[§19 (architectural parity is a tenet)](0019-architectural-parity.md) is what makes this a decision rather than a port task. A
capability that ships on two of three architectures is a scope note at best, and this gap is
permanent: no amount of work closes it, because the RISC-V privileged architecture has nothing to
open.

## The options

### Option 1: the fourth shape, `Timer::ARM(deadline, notification)`

The kernel owns the comparator on every architecture, which is where two of three put it anyway, and
signals a notification at the deadline on the holder's behalf. A thread blocks in `RECV` with the
notification bound to its TCB ([§101](0101-notification-objects.md)) and wakes on either.

Measured (milestone 263's scaffold, built, gated green with `script/test`, and deleted):

| what | figure |
|---|---|
| the `Object` variant | **free**; the enum is already 24 bytes wide because of `PageFrame` |
| `ipc_fastpath` | **unchanged on all three ISAs** |
| `syscall_entry` (flat) | aarch64 +12 B, riscv64 +158 B, x86_64 +96 B |
| kernel symbol bytes | +738 B / +2,266 B / +1,386 B, all under 0.25% |
| per-tick bookkeeping | already priced in `notes/timed-wait.md`: one comparison per idle tick |

What it costs that is not bytes. A syscall-surface addition, permanent, that every future program is
written against. And a dependency on milestone 151, which is unbuilt. Without §101 (notification
objects)'s TCB binding this buys a thread a timer it can block on and no way to block on a timer
*and* a message at once, which is milestone 106's actual complaint. Option 1 is 151 plus one object,
and pricing it any other way understates it.

### Option 2: a userspace service on aarch64 and x86_64, with a riscv64 scope note

Keeps the 2026-09-05 decision on two architectures and records the third as a gap.

What it costs. §19 exists to make this expensive, and the usual mitigating argument does not apply:
a scope note normally records a port that has not been done, and this one would record something that
cannot be done. It is also worse than two-of-three suggests, because the aarch64 half rests on the
`CNTP_*` comparator, which traps under a hypervisor and is one per PE. riscv64 programs would need a different timed wait from the other two, which is the
"a feature works on one ISA and silently not another" shape rule 5 names as the bug. It also needs
aarch64's per-thread `CNTKCTL_EL1` grant built (the machinery exists; the policy does not) and the
hypervisor caveat accepted, and it needs xenon's HPET confirmed before anything on x86_64 is real.

### Option 3: reopen milestone 51's three shapes

`SYS_SLEEP`, a timer object, or a deadline argument on `RECV`/`CALL`. `notes/timed-wait.md` prices all
three and names no winner, deliberately. This spike does not price them and does not need to: they
were never blocked on the question 263 answered.

What it costs. The fork calef declined to decide on 2026-09-05 comes back, which is the outcome
his decision was avoiding. It is worth saying that the reason he could avoid it was the userspace
service, and that reason is gone.

## What is blocked until this is answered

- Milestone 106, which is what all of this is for.
- Milestone 151 is not blocked, and is now on the critical path of every surviving option rather
  than beside it. A notification object is a prerequisite of 1 and 2 and useful to 3.
- The four consumers 106 names keep spinning: `thread::sleep`, `Endpoint::RECV`'s callers, the
  shell's `^C` poll, and `net_stack`, whose retransmit backoff burns a core, at a measured ratio of
  about `10^5` to 1 against a timed wait.

## What this file does not do

It did not recommend, and that was on purpose. AGENTS.md: *"Recommend on reversible forks; give
options only on irreversible ones"*, and a syscall-surface addition is the named example on the
irreversible side. Milestone 263's lane priced the options and stopped there; calef ruled from the
prices. Kept in the past tense rather than deleted, because how a decision was reached is part of
the record. A reader who finds a file that names a winner should be able to see that the winner
was not this file's to name.

It also does not amend
[§139 (who may read the cycle counter, and by what authority)](0139-cycle-counter-authority.md),
which says *"There is no precedent
in this tree for a per-thread system-register bit maintained across a context switch."* Milestones 229
and 237 built one, so that sentence is stale inside its own decision. The correction is recorded in
[`notes/timer-capability/specifications.md`](../../notes/timer-capability/specifications.md#aarch64-the-blocks-negative-is-refuted) and in 263's block; amending a decision section is not a lane's to do.

## Amendment (milestone 106): the syscall numbers, `bits`, and no slack argument

This section's option 1 named the shape, `Timer::ARM(deadline, notification)`, and left the numbers
and one detail to the building lane. Milestone 106's PR (#1378) asked for both, and calef ruled on
the surface it built rather than the shape alone.

`object_type::TIMER = 5`, the module's name spelled out by the ruling of 2026-11-11 (UTC). A `Timer`
is retyped from untyped by `MemoryRegion::RETYPE_OBJECT` (method 2, its name spelled out by the same
ruling), the same route as every other kernel object §147 already assumed. calef, 2026-09-27T01:59:29Z (UTC), on PR #1378: *"Confirmed"*, against the
lane's proposal of `objtype::TIMER = 5` and `Timer::ARM = 0` / `Timer::CANCEL = 1`.

`Timer::ARM(deadline, notification_slot, bits) = 0`, a third argument beyond this section's
text. `invoke(cap, ARM, deadline, notification_slot, bits)` arms the timer: when the counter
reaches `deadline`, the kernel ORs `bits` into the notification named by `notification_slot`,
exactly as `notification::SIGNAL(bits)` would. Arming replaces any pending deadline, which then
never fires; a deadline already reached signals at once and leaves nothing armed. `bits = 0` is
accepted and delivers nothing when the deadline passes, the same reading `SIGNAL(0)` already has
(it wakes nobody, because it has nothing to deliver). A timer armed with `bits = 0` still consumes
and replaces the pending deadline; it just signals nothing when it fires. Needs `WRITE` on the timer
and on the notification (the same right `SIGNAL` itself needs, because arming is signaling later);
`WrongObject` if the slot named is not a notification. A notification destroyed while the timer is
armed makes the expiry a no-op, not an error: there is nobody left to tell.

Deadlines are absolute counter ticks, the same counter a program already reads without a
syscall (`CNTVCT_EL0`, `rdtime`, `rdtsc`); nothing converts between clocks. Resolution is the
scheduler tick, 100 Hz / 10 ms: a deadline fires at the first tick at or after it, never before.

`Timer::CANCEL = 1`. `invoke(cap, CANCEL, _, _, _)` disarms the timer: `1` if a deadline was
pending and now never fires, `0` if nothing was armed (never armed, already fired, or already
cancelled). The return distinguishes the two so a caller can tell whether its own signal already
went out. A cancel does not reach into the notification's word: bits a timer already signaled stay
there, because a notification never loses a bit.

No slack argument. calef, 2026-09-27T02:00:47Z (UTC), on PR #1378, after reading the prior art
this section's option 1 did not need to (Zircon's `zx_timer_set`, which carries one; Mach's
`mk_timer`; seL4's badge bits; Linux `timerfd`; kqueue): *"Keep it confirmed. We will build a slack
if we need it."* `ARM` takes exactly three arguments beyond the object and method. A slack would be
a later, additive syscall change, only if a tickless kernel ever needs one; this kernel's timer is
tick-resolution already, so there is nothing for a slack to buy today.

Both rulings are calef's own comments on #1378, relayed there under his account by the maintainer
lane that opened the PR; the exact wording and both timestamps are quoted as posted. The full
surface, with its rationale in prose, is `crates/abi/src/lib.rs`'s `objtype::TIMER` and `timer`
module doc comments on that branch.
