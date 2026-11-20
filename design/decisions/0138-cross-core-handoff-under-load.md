---
status: DECIDED
raised: 2026-09-02
decided: 2026-09-02
ratified_by: calef
---

# 138. How a saturated workload is made to hand threads across cores

Answered by calef on 2026-09-02, on the one question that was blocking.
Written by a research lane
on 2026-09-01, after milestone 219 (the boot tour ends and the kernel halts, so there is nothing to
soak) measured that it never happens today.

calef approved option D, the `--features soak` hook that signals a routed rendezvous from
`sched::on_tick`, so soak workers block on it through the `Irq::WAIT` that already exists. It adds
no system call and nothing of it exists in a production build. That unblocks the second half of fatal
risk 5's decisive experiment, which had never been runnable.

Two things stay open on purpose and neither is blocking.

**Option A, thread affinity, is not refused; it is not yet asked.** It is system call surface, so this
section gives options rather than a recommendation, and it should be taken on its own merits rather
than carried in on the back of an experiment D can run without it. What the prior art narrowed:
the core belongs in a capability rather than in a message, and DECISIONS §28 already chose the shape
at ratification. What is still calef's: singleton per core or narrowable set, and whether it is a
new object or a field of one that exists.

Option B, a periodic rebalancer, is declined rather than dismissed, and the Linux sourcing
weakened the case against it. nife already has counterparts for three of Linux v6.12's four
balancing moments, all event-driven, and a saturated rendezvous workload starves all three of their
triggers, which is why the crossing count freezes. B is the fourth moment, the only one that fires
on a clock and the only one that workload cannot starve, so calling it "importing Linux's answer"
overstated what it is. The refusal now rests on DECISIONS §28's own reopening trigger, a real
workload where fairness visibly fails, which has not fired. A reader who believes it has should
reopen B, and that sentence is here so nobody has to relitigate the whole section to do it.

**The number is provisional.** This lane cannot see the other lanes running beside it, so the
integrator mints the real section number at merge, the same as every other name global to the tree.

## What is being decided

`design/fatal-risks.md`'s fifth entry, *it cannot be made reliable on multicore, and the bugs appear
only on silicon*, names its decisive experiment as sustained multi-core stress on the boards. Milestone
219 built that stress and found the experiment is two experiments, of which it delivers one:

- Contention on shared kernel state at rate. Four harts entering `IPC_TABLES` tens of thousands
  of times a second. The soak sustains this.
- Cross-core handoff under load. Threads actually moving between cores while the machine is
  busy. This is where the risk's only observed defect lived: on radon, `sched::wake_load_aware` made
  a receiver `Ready` without a delivery, on three harts, found at a bench rather than by any test.
  **The soak does not sustain this, and cannot.**

So: what mechanism gets threads crossing cores under load, and what does the tree pay for it
permanently?

## The premise, verified by this lane rather than taken from the milestone

The premise run and the code behind it are in
[`premise.md`](0138-cross-core-handoff-under-load/premise.md).

## What this tree already does in the analogous case

The other precedents are in
[`premise.md`](0138-cross-core-handoff-under-load/premise.md#what-this-tree-already-does-in-the-analogous-case).
The one that stays here:

A feature-gated kernel affordance built only to make an experiment possible is established practice.
`kernel/Cargo.toml` carries `fastpath_pad`, which adds a large reachable-but-never-taken function to
the IPC fast path so milestone 134 (the register of measures)'s footprint experiment has something
to compare. It also carries `watchdog_probe`, which adds a test that deliberately livelocks and is
expected to fail the run. Neither ships. `soak` itself is the third. So an affordance that exists
only under `--features soak` crosses no line this tree has not already crossed on purpose. Milestone
219 established the hygiene that goes with it: `script/lint` clippies the feature, and
`script/fastpath-footprint` proves the production binary is byte-identical.

## The prior art, read rather than recalled

[`prior-art.md`](0138-cross-core-handoff-under-load/prior-art.md) holds seL4, L4Re, Barrelfish and
QNX. [`linux.md`](0138-cross-core-handoff-under-load/linux.md) holds Linux v6.12's load balancing.

The maintainer's three-moment summary was substantively right, and this is a re-cut rather than a
correction. It folded the tickless case into the idle one, where the source puts it beside
the periodic one, since a NOHZ kick makes a *busy* CPU run the balancer for idle neighbors.
`sched_setaffinity` is not on this list because it is a request from userspace rather than a moment
the kernel chooses.

**What this is evidence of, and it is not that Linux is careless.** Linux has to guess continuously
because almost nothing ever tells it anything. It schedules unknown binaries for users who never
declare a placement. The interface that would let them exists (`sched_setaffinity`), but the kernel
cannot assume it was called, so its working inputs are load, topology and history. Four sampling
points plus a push is what it takes to keep an arbitrary workload spread without being told. That is
a harder scheduling problem than nife has, and the machinery is proportionate to it rather than
excessive. The contrast with seL4 and Barrelfish is about the premise, not about the quality of
either answer. They assume the application knows where its threads belong and make that a right it
holds, so there is nothing left for the kernel to infer. nife's §28 (SMP placement) sits on the
second premise, and milestone 219's frozen counter is what the second premise looks like from a
bench.

### What the prior art decides, and what it leaves open

- **Nothing here treats "a saturated workload does not migrate" as a bug.** seL4 and Barrelfish both
  refuse implicit migration on purpose, and seL4's stated reasons (user control of resources,
  real-time analysability) are reasons this project already holds. That is a strong argument
  against option B and it did not come from this tree.
- Linux is the instructive contrast rather than the cautionary tale, and reading it sharpened what
  option B actually is. nife is not innocent of load balancing. `pick_spawn_target` is the fork-time
  placement Linux does in `select_task_rq_fair` under `SD_BALANCE_FORK`, `try_initiate_steal` in
  `run_idle` is the going-idle pull Linux does in `sched_balance_newidle`, and `wake_load_aware` off
  `irq_notify` is a narrow version of its wake-time placement. Three of Linux's four moments already
  have a counterpart here. The one nife does not have is the periodic one, and it is the only one of
  the four that fires whether or not anything happened. That is exactly why the counter freezes: a
  saturated rendezvous workload starves the other three of their triggers (nothing forks, no core
  goes idle, no device interrupt lands), and an event-driven balancer with no events does nothing.
  This is a point in option B's favor and it is recorded as one, below.
- If affinity is built, the shape has a clear winner in the family this project belongs to: the
  core is a property of a capability, not an argument in a message. seL4 MCS and L4Re agree, from
  opposite directions (a singleton per core, a narrowable set), and seL4's own history is a
  worked example of the parameter form being the thing that got replaced.
- Still open, and calef's: whether nife wants the singleton or the set, and whether a placement
  authority is a new object or a field of something that exists. Nothing read here decides that.

## The options

### A. Thread affinity: a program says where a thread runs

What it is. A placement request a program can make. §28's own deferred wording puts it in the
spawn manifest, where it is a property of a thread being created rather than a method on one that
exists.

Option A's working is in
[`options.md`](0138-cross-core-handoff-under-load/options.md#a-thread-affinity-a-program-says-where-a-thread-runs).

Reversibility. Low. A manifest field is a wire format two programs agree on, which is the *move
fast on what can be undone* tenet's first irreversible category. Nobody has acted on it yet, which is
the one thing in its favor.

### B. A periodic rebalancer in the scheduler

What it is. Kernel policy: on some tick boundary, move a thread from a loaded queue to a lighter
one.

Reversibility. High, and it is the only option here that is. §28's own changeability note says
so: *"this is scheduler-internal policy. No ABI, no capability semantics, no baseline movement"*.

What the prior art says about it, and it is the strongest single argument in this document. No
capability microkernel read for this proposal rebalances threads at all. seL4 does not, on the
record and in the source, and its authors' stated reason is *"seL4's general philosophy of having
all resource management under user control (and also helps reasoning about real-time properties)"*.
Barrelfish does not, structurally. Linux does, and offers turning it off as a first-class control
(`cpuset.cpus.partition` set to `"isolated"`, *"Partition root without load balancing"*). So the
behavior milestone 219 measured and read as a gap is the design position of the kernel this project
measures itself against. B would be the one option that moves nife away from it in order to run a
test.

Option B's working is in
[`options.md`](0138-cross-core-handoff-under-load/options.md#b-a-periodic-rebalancer-in-the-scheduler).

### C. A userspace-drivable interrupt source

Option C is in
[`options.md`](0138-cross-core-handoff-under-load/options.md#c-a-userspace-drivable-interrupt-source).

### D. A soak-only, kernel-side interrupt raiser (this lane's finding, and it was measured)

What it is. Under `--features soak` and nowhere else, the kernel signals a routed rendezvous from
inside a real interrupt handler, and soak workers block on it through the existing `Irq::WAIT`. No
system call is added. Nothing exists in a production build.

How option D was read, run and costed is in
[`options.md`](0138-cross-core-handoff-under-load/options.md#d-a-soak-only-kernel-side-interrupt-raiser-this-lanes-finding-and-it-was-measured).

**What it does not reach**, said precisely because a claim of "the real path" is worth nothing
without it: the interrupt controller's own claim, mask and complete sequence (the PLIC on riscv64,
the GIC on aarch64) is not exercised. That is because the timer is not a controller-routed source.
The experiment is about the wake protocol, and that is what this runs.

`refused`, `mismatch` and `stalled` stayed at zero throughout both runs. **The spike is not
committed to this branch** and this document is the only thing that survives it. The patch is a
scratch file, deliberately, because shipping it is the decision being asked for rather than a thing
a research lane may do.

Reversibility. Highest of the four. A feature-gated affordance is deleted by deleting it, nobody
outside the tree can act on it, and no program is written against it.

## Recommendation

On the reversible part, this lane recommends D, and recommends building it before deciding A.

The reason is not that D is cheaper, which would be an argument from implementation convenience and
this tree refuses those. It is that D tests the path the defect was on and A tests the path beside
it. Milestone 219's whole lesson was that an instrument *near* the question is not an instrument
that answers it, and choosing A here would repeat that mistake at the level of the experiment rather
than the counter. D also leaves the machine unchanged when nobody is testing, which was the property
the maintainer's provisional pick of A was reaching for. D has it more completely: A is inert only
if no program uses it, and a mechanism that is only inert by convention is rung four.

**On the irreversible part, this lane gives options and does not recommend.** A is system call surface
and its capability question is genuinely open. This document's contribution to it is to narrow what
must be decided rather than to decide it:

- §28 already chose the shape (a grant in the spawn manifest, not a method on a live thread), so
  that part is not open unless someone argues to reopen it.
- The authority question is open and has no default. The three readings are above.
- **A is worth having on its own merits and does not need this experiment to justify it.** A
  latency-sensitive server pinned away from a noisy neighbor, and a driver thread near its device's
  interrupt, are things real systems want, and §28 deferred it with a trigger rather than refusing
  it. That case should be made on its own, with its own workload, rather than being carried in on the
  back of a test that D can run without it.

B is declined, and the refusal is the valuable half. Not because it would not work, but for two
reasons that compound. It changes what the scheduler promises in every boot forever to make one
experiment possible, and §28 named the trigger for reopening scheduling policy as *a real workload
where fairness visibly fails*; there is no such workload. And the prior art says the thing it would
add is the thing seL4 and Barrelfish deliberately do not have, for reasons this project shares. If one appears, B comes back on its own merits with something to
tune against, which it does not have today.

The Linux reading weakened that refusal without overturning it, and the weakening is stated here
rather than buried. B is the fourth of four balancing moments, three of which nife already has, so
"importing Linux's answer" overstates it; and it is the only moment a saturated workload cannot
starve, which is a real argument that nothing in this tree had made. What survives is the second
reason and not the first. The cost is not the mechanism, it is the tuning surface, and Linux is the
evidence for how large that surface gets and for the fact that it is only tunable against workloads.
The refusal now rests on §28's trigger alone. A reader who thinks that trigger has been met should
reopen B, and this paragraph is the invitation.

C is declined in favor of D, having established that its userspace half already exists and its
kernel half does not need a system call.

## What is blocked until this is answered

Answered 2026-09-02: D approved. The list below is what was blocked, kept because it records
what the decision bought rather than what remains.

- The second half of risk 5's decisive experiment. Until one of these exists, sustained
  cross-core handoff cannot be run on radon, argon or xenon at all, and the soak on those boards
  (milestone 219's other proposed follow-on) covers one of the two things the risk needs.
- Nothing else. The soak runs, the tooling is built, and the boards can be soaked for contention
  today. This is a decision about how much more the experiment can be made to cover, not a blocker
  on anything already planned.

## What was considered and refused

- Driving a real device at rate instead (issue file reads through the block server, so virtio
  completion interrupts hit `wake_load_aware` on the fully real path). Refused for the boards, which
  is where the experiment happens: radon is real hardware with no virtio, and the device it does have
  is a UART whose interrupt rate a soak cannot drive. It would also measure the block path rather
  than the wake path, at a fraction of the rate.
- Raising a software interrupt per architecture instead of using the timer. aarch64 can already do
  it (`drivers::gic::send_sgi`, which `system_tests/src/user/elf_child_builder_tests.rs` uses to
  raise `DELEGATED_TEST_INTERRUPT` at a chosen core), and x86_64 has a self-IPI vector. But riscv64
  has neither: its software-interrupt arm is the reschedule IPI and never reaches `irq_route`, which
  `kernel/src/user/tests.rs` states as the reason that test is aarch64-only. Since radon is the
  riscv64 board and the one machine that has already produced a defect of this class, a mechanism
  that cannot run there is the wrong mechanism. The timer arm is the one thing all three
  architectures share, through a function (`sched::on_tick`) that is already architecture-neutral.
- Making the soak workers themselves cross by pinning them (option A applied only to the soak).
  Refused on `cpu.rs`'s recorded reasoning: a workload the scheduler is not free to place is one
  interleaving explored thoroughly, and this experiment's value is in the interleavings it has not
  seen.
- Counting `PlaceRemote` more carefully and calling that the second experiment. Refused because
  milestone 219 already tried exactly this and recorded why it fails: a placement counter is
  structurally blind to migration, since a rendezvous wake places locally even when the thread has
  moved. A better counter cannot substitute for a workload that does the thing.
