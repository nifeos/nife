---
status: DECIDED
raised: 2026-09-02
decided: 2026-09-02
ratified_by: calef
---

# 139. Who may read the cycle counter, and by what authority

Answered by calef on 2026-09-02, in three parts, each after the evidence for it
was gathered rather than before.

1. The model: option 4, a per-thread grant enforced at the context switch. The read stays one
instruction with no system call inside the measured operation, it is what Linux arm64 converged on for
the same reason, and it costs the compare that `switch_user_root` already pays for `TTBR0_EL1`.
Option 2 (a first-class capability object) was declined for now on the ground this tree already
applies elsewhere. It buys a new object type before there is a second consumer, and seL4's own
RFC-16 for that shape has been unmerged since 2024-02-02. New methods are allowed within the
established model (AGENTS.md), so option 2 remains reachable if a consumer appears.

2. The grant is a field in the spawn manifest, not a method on a live thread. calef asked
whether milestone 147 (a profiler that holds exactly the counters it was granted) argued for the
live form, since a consumer already exists; checking found it does not. 147 wants *this profiler may
read that subtree's counters*, which is cross-thread authority with a named target, and neither
shape here provides it. So the immutable grant costs nothing 147 needs, and it is the stronger
statement: a program cannot acquire a timing side channel it was not given at creation.
DECISIONS §28 chose the same shape for thread placement at ratification.

3. `x86_64` keeps its ambient counter, recorded as a position rather than left as an accident.
Three options were live and two were closed by measurement, both in this document. There is no
second user-readable clock on that architecture: Linux's own vDSO fast path cannot work without a
userspace TSC read, and it deletes the HPET mapping rather than offer it. Trap-and-emulate was
priced from Xen's "15 to 20 times slower" and measured at 1,667 ns, which is 4.1x the system call it
was meant to beat, with an in-kernel minor page fault at 1,219 ns as the floor a nife handler could
aspire to.

The deciding cost is not the one this document first named. smoltcp would not notice a coarse clock:
`net_stack.rs`'s `instant()` divides to `Instant::from_millis` and discards the resolution. The wall
clock would notice. `components/src/network_time_client.rs` advances `local.now()` between syncs by
`monotonic_nanos().saturating_sub(self.mono0)`, and `monotonic_nanos` reads the cycle counter. So on
`x86_64` a tick-resolution page would make every timestamp between NTP syncs step in tick-sized
jumps, with NTP computing corrections against a clock coarser than the corrections. aarch64 and
riscv64 never meet this, because gating the cycle counter leaves them a generic timer.

What the grant buys, stated so nobody reads it as more. Two threads and a shared word
reconstruct a fine clock with no privileged instruction, measured at 6.8 ns of usable resolution on
cordoba and matching Schwarz et al. (FC 2017). That holds on all three architectures. So option 4
buys accountable authority, meaning the cheap accurate path is granted rather than ambient and
the kernel knows which threads hold it. It does not buy timing confinement, and this tree should not
claim it does.

**The number is provisional.** This lane could not see the other lanes running beside it, so 139 is
a claim on the next free slot rather than a mint. The integrator assigns the real one at merge, and
two lanes collided on a number the day before this was written. Cite it as
`design/decisions/0139-cycle-counter-authority.md` until it lands.

Written 2026-09-02 by a research lane briefed to answer milestone 75
(who may read the cycle counter), which is `Gate: DECISION` and has been NOT-STARTED since
2026-08-03. It implements nothing. Milestone 75's own block is the question; this is the evidence and
the options, and the parts that are calef's are marked as his.

## The appendices

This page is the ruling and what a reader needs to act on it. calef ruled on 2026-10-11 (UTC)
that it is split under [§212 (a prose budget)](0212-a-prose-budget-for-every-document.md) rather
than given an exception, so the evidence moved, uncut, to appendices:

- [`premise-checks.md`](0139-cycle-counter-authority/premise-checks.md): the four findings below,
  with their Arm, Linux and RISC-V sources.
- [`x86-clock-sources.md`](0139-cycle-counter-authority/x86-clock-sources.md): why `x86_64` has
  no second user-readable clock, and the honest scope note for its row.
- [`trap-and-emulate.md`](0139-cycle-counter-authority/trap-and-emulate.md): the 1,667 ns
  measurement, the `now()` census, and what the number does not settle.
- [`prior-art.md`](0139-cycle-counter-authority/prior-art.md): seL4, Linux and L4Re, and the
  two-thread clock that defeats a closed counter on all three architectures.

## What is being decided

May a program running at EL0 read a cycle counter, and if so by what authority? Three registers,
one question, three different answers today:

| | the fine counter | opened to EL0 by | state in this tree |
|---|---|---|---|
| aarch64 | `PMCCNTR_EL0` | `PMUSERENR_EL0.CR` or `.EN` | never written by this kernel |
| riscv64 | the `cycle` CSR | `scounteren.CY` (and `mcounteren.CY` in firmware) | never cleared by this kernel |
| `x86_64` | the TSC, via `rdtsc` | `CR4.TSD` clear | **open, and load-bearing** |

Milestone 74 (cycle counters) needs the answer before its
aarch64 half can land, and milestone 147 (a profiler that holds exactly the counters it was granted)
cannot be scoped at all until the grant unit exists.

## The premise was half false, and that is the most useful thing in this document

Milestone 75's block frames this as a decision about whether to open something that is closed.
Three checks say the framing is wrong in three different directions, and each one changes what the
decision has to cover.

The four findings, each with its evidence in
[`premise-checks.md`](0139-cycle-counter-authority/premise-checks.md):

1. On `x86_64` the counter is already open to every program, and nothing decided that.
2. And closing it on `x86_64` would take the clock away, because there is only one register.
   [`x86-clock-sources.md`](0139-cycle-counter-authority/x86-clock-sources.md) checks that claim.
3. On aarch64 and riscv64 the tree does not *establish* that the counter is closed, it assumes it.
   That part is a defect fix, and the recommendation below lists it outright.
4. And 74's aarch64 half is blocked on more than this: PR #650, the EL2 to EL1 entry drop, was
   open on 2026-09-02.

## What this tree already does in the analogous case

Four analogues, and they do not all point the same way, which is why this needed reading rather than
recalling.

- The generic timer is ambient, deliberately, and the record says why. notes/abi.md calls it "the
  one ambient thing" and defends it: "A monotonic counter grants no authority to *affect* anything,
  only to observe the passage of time". `crates/uptime` inherits it and its module docs make the
  point that the program "needed no manifest field, no new capability, and no wiring".
- The wall clock is a capability, expressed in objects the kernel already had. §43 gives read as
  a read-only page, set as a writable page, and propose as an endpoint, with "No new system call, no
  new method number, no new object type". That is the shape a cheap answer here would want to
  copy: an authority expressed in existing objects rather than a new type.
- Entropy and the clock are both services, reached by capability, so "a program that needs a
  privileged read asks a service" is the tree's normal case, not an exotic one.
- `CNTKCTL_EL1.EL0VCTEN` and `scounteren.TM` are per-machine bits, set once at init. There is no
  precedent in this tree for a per-thread system-register bit maintained across a context switch.
  That is the one piece of machinery option 4 below needs and the tree does not have.

## The options, with what each costs

Milestone 75's block names three. There is a fourth, it is the one the prior art converged on, and it
did not exist in the block.

### Option 1: ambient, like the generic timer

Set `PMUSERENR_EL0.CR` once at init, set `scounteren.CY`, leave `CR4.TSD` clear.

- Cost to build: aarch64 one `msr` in `timer::init` or `cpu` init; riscv64 one more bit in the
  existing `csrs`; `x86_64` nothing at all, since it is the state today. Call it three instructions.
- Cost to the claim: it spends §10's exception a second time on an instrument roughly 160x finer
  (0.25 ns against 41 ns), and it is the configuration seL4 declines to verify and declines to ship
  on by default. It is also the one that cannot be walked back: an ambient opening becomes something
  programs depend on, which milestone 75's own scope note names as the worst outcome.
- What it is honest about: it is what we already do on `x86_64`, so choosing it makes the tree
  consistent rather than making it worse.

### Option 2: a first-class capability object

A PMU object, a grant in the spawn path, a checked invocation. seL4's RFC-16 shape.

- Cost to build: a new object type, a new method number, spawn-path wiring, `caps` output, and
  Kani reach. Nothing in this tree prices at a morning. Milestone 147 says the counter-set and
  target-naming parts have "no precedent in this tree to price from".
- Cost at the measurement: this is the one that decides it. If the read is an invocation, the
  measured operation now contains a system call, which is option 3's defect arriving through a different
  door. It is only free if the capability's *effect* is to open the register, at which point the
  capability is a grant of option 4 and the object is bookkeeping around it.
- Cost to reverse: highest on the list. A new object type and method number is the system call
  surface, which §10 and §16 put in the expensive category, and milestone 147 would build on it.

### Option 3: kernel-mediated

EL0 asks the kernel to time an operation; the register never opens.

- Refused, and 75 already refused it, correctly: the measurement then contains the system call it is
  trying to measure. Recorded so it stays visibly rejected.
- **One thing it is right for, which 75 does not say.** On riscv64 the SBI PMU route (`EID 0x504D55`)
  is inherently this shape: SBI calls are made from S-mode, so a U-mode program cannot make one and
  the kernel is in the path by construction. So RISC-V's cheap-read story is the `cycle` CSR and
  `scounteren.CY`, not SBI, and milestone 74's RISC-V half will want both for different jobs.

### Option 4: a per-thread grant, enforced at the context switch

The thread that was granted it runs with the counter open; every other thread runs with it closed.
The kernel writes the enable on the switch, the same way it writes the address-space root.

- Cost at the measurement: zero. The read stays one `mrs`, no system call, no trap. It is the same
  instrument seL4's published numbers were taken with, which is what comparability requires.
- Cost on the context-switch path: one comparison, and one `msr` only when the value changes.
  That is exactly the shape `kernel/src/arch/aarch64/mmu.rs`'s `switch_user_root` already has (it
  early-returns when `TTBR0_EL1` already holds the wanted value), called from `schedule`
  (`sched/switch.rs`). If no
  thread is granted, the value never changes and the whole cost is a compare.
- Cost to build: a bit on the TCB, a write at the switch site on three architectures, and a way
  to set the bit. The last part is the expensive one: setting it is a system-call-surface change, either
  a field on TCB configure or a new spawn-path input, and that is calef's rather than a lane's.
- The `x86_64` asymmetry survives this option and has to be decided separately. `CR4.TSD` is
  writable per switch too, but closing it for ungranted threads removes `user_rt::now()` from every
  x86 program, so option 4 on `x86_64` is blocked behind giving that architecture a second time
  source. Until then `x86_64` is option 1 whatever the other two do, and a scope note should say so
  rather than letting §19 (architectural parity) report a gap it cannot close.
- **What it does not do:** it does not name a target the way milestone 147 wants. It says "this
  thread may read the counter", not "this profiler may read that subtree's counters". 147's work is
  still 147's.

## Recommendation

Split three ways, because the parts have different costs and different owners.

Recommended outright, and reversible: close what we claim is closed. Independent of the
authority question, and before any of milestone 74 lands:

1. Write `PMUSERENR_EL0 = 0` explicitly in aarch64 CPU init, per-core, rather than inheriting an
   architecturally UNKNOWN value. This is Linux's fix, for Linux's reason.
2. Clear `scounteren.CY` and `.IR` explicitly in the riscv64 per-hart timer init, so the comment
   that says they "stay closed" is made true by the code that says it.
3. Record in `notes/x86-port.md` and in `crates/user_rt`'s `now()` that on `x86_64` the cycle counter
   is ambient today, that this was inherited rather than chosen, and what closing it would cost.

Rung one is not available here (a register cannot be made unrepresentable), so this is rung two done
at init, plus a `BUGS` line where the reader meets it. It is three small writes, it is not the
decision, and it is the difference between a claim and a fact on argon.

Recommended, and calef's to confirm because it touches the system call surface: option 4. It is the
only option that keeps the measured path free of a system call while making the authority checkable. It
is what Linux arm64 converged on for the same reason, and it costs a compare on a path that already
does exactly this compare for `TTBR0_EL1`. Option 2 is the more seL4-shaped answer and is what
milestone 147 would eventually want. It is also unbuilt in seL4 after two and a half years. Choosing
it now buys a new object type before there is a second consumer, which is the speculative
abstraction both 74's and 147's scope notes already refuse.

Options rather than a recommendation, because it is irreversible: how the grant is expressed.
A field on TCB configure, a new spawn input, or a badge on an existing capability are three shapes
with three different system-call-surface costs, and a lane should not pick one. Nor should a lane decide
the `x86_64` row, since keeping `rdtsc` ambient there is a published confinement position and not
only an implementation state.

## Does this touch fatal risk 7?

**Yes, and the honest form of the answer is that it touches a claim the tree does not currently
make.** `notes/confinement-claims.md` enumerates 26 claims and names three more that are "stated
nowhere". The strings `timing`, `side channel` and `covert` appear in that note zero times, and zero
times in `DECISIONS.md` and in `design/fatal-risks.md`. So nothing in the confinement enumeration is
falsified by any answer here, because timing isolation is not among the things nife claims.

That absence is the finding, and it is the same category milestone 202 (every confinement test is a
ritual until somebody breaks the confinement) already found three members of. seL4 states the
position explicitly and in one clause ("this option opens the possibility of timing channels"), and
this tree, which will publish cycle-denominated numbers against seL4's, states nothing.

**What this decision should therefore also produce, whichever option wins: one row in
`notes/confinement-claims.md` stating what nife does not claim.** A confined component's *timing* is
not confined. That belongs beside the row saying a confined device's values are not confined, for
exactly the same reason: so nobody reads the capability rows as covering it.

## What it costs the benchmark story to say no

Real, and smaller than it looks, and it is worth being exact about who pays.

- Milestone 25's `sel4bench` comparability is the part that genuinely needs it. seL4's published
  413 and 426 are single-shot PMU measurements taken from user level. Reproducing that instrument on
  argon needs a user-level cycle read; a kernel-mediated timing of the same operation is not the same
  measurement and would not referee anything.
- **Our own numbers do not need it.** notes/pmu.md's whole point is that the long-loop generic-timer
  method is valid and is what survives virtualization: "Both are valid; they fail under different
  conditions." Milestone 168 (a multi-tasking workload benchmark) is a workload benchmark rather than
  a single-operation one, so it is a long-loop measurement and risk 4's decisive experiment is **not**
  blocked by a "no" here. That is worth saying plainly, because the brief that produced this document
  assumed otherwise, and the chain from this decision to risk 4 is weaker than it looks.
- Milestone 74's most-cited payoff survives a no. Turning "roughly 1,120 cycles at an assumed
  3.2 GHz" into a read number needs the counter read *somewhere*, and the kernel may read
  `PMCCNTR_EL0` at EL1 with no EL0 opening at all. What a no costs is the seL4-identical instrument,
  not cycles as a unit.

So a no is affordable for everything except the one comparison milestone 127 bought a board for.

## What is blocked until this is answered

- Milestone 74's aarch64 half, by its own gate. Its riscv64 SBI half is not, and neither is a
  kernel-side EL1 read.
- Milestone 147, entirely, by its own gate, since it cannot know what a grant unit is.
- Nothing else. Milestone 168 and risk 4 are not blocked, per the section above.

## BUGS

- **The context-switch cost is priced by shape, not measured.** "One compare, one `msr` on change" is
  read off `switch_user_root`'s structure. Nobody has measured what an added `msr` costs on the
  switch path on any of the three architectures, and on `x86_64` a `CR4` write is serializing and
  would not be free.
- The evidence carries its own `BUGS` in three of the appendices: what was read rather than
  measured, what did not reproduce, and what no emulator can answer. The unrun aarch64 spike and
  the untested riscv64 `mcounteren` half are in [`premise-checks.md`](0139-cycle-counter-authority/premise-checks.md#bugs).
- L4Re is missing from the prior art and it is the one gap in that section. See
  [`prior-art.md`](0139-cycle-counter-authority/prior-art.md).
