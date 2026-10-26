---
status: NOT-STARTED
raised: 2026-08-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 187. Read the x86_64 arch tree through the lens the first arch audit used

This is finding 8 of the architecture-list parity sweep, which found eleven siblings of
`script/stack-frame-check`'s hardcoded `arches="aarch64 riscv64"`. Ten are strings in shell scripts
that a one-line edit fixes, and they are the worklist of milestone 186 (derive the architecture
list). This one is not a string: `notes/arch-audit.md` read no x86_64 code, and its BUGS names the
gap, and that directory is the largest of the three.

The lens exists, the template exists, and the disposition vocabulary exists
(DECISIONS §35, how a scanner's findings get dispositioned). Nothing here is a design fork. Findings
may raise forks, which is what an audit is for; raising them is not the same as this milestone
needing one to start.

## What the existing audit actually did

`notes/arch-audit.md` is not a review. It took one bug class, stated before any code was read,
and asked four fixed questions of every candidate, the last being whether the window is reachable.
It recorded cleared candidates as well as findings, and bounded the search with structural facts.
It read for nothing else, and says so. [`method.md`](0187-x86-64-arch-audit/method.md) holds the
method in full and how the line counts below were taken.

## The scale, measured

Counted on 2026-08-27 as physical lines of every file, the same way the original note counted; the
command and its check against that note are in [`method.md`](0187-x86-64-arch-audit/method.md).

| | Files | Lines | State |
|---|---|---|---|
| `kernel/src/arch/x86_64` | 18 | **6,797** | never read under any lens |
| `kernel/src/arch/aarch64` + `riscv64`, at the audit (2026-07-29) | 25 | 6,202 | read in full |
| `kernel/src/arch/aarch64` + `riscv64`, today | 29 | 9,835 | read in full as of 2026-07-29 |

Two things follow, and the second is a correction to how the sweep put it.

The unread directory is larger than the entire two-ISA tree that the one affordable pass read in
full. That is finding 8's claim and it holds exactly.

But x86_64 is **41% of today's arch tree, not a majority of what is unread**. The two ISAs the audit
did read have grown by 3,633 lines since it read them, 59%, and riscv64 has more than doubled
(2,374 to 4,932). "Read in full, both ISAs" describes a state of the tree that no longer exists.
This milestone does not take that on, and says so under *What this does not decide*, but a reader
who takes 6,797 as "the unaudited fraction" is reading it too favorably.

## Why this is the highest-value unaudited surface

It is in the trusted computing base, and no prover reaches it. `script/verify`'s Kani harnesses
cover the pure-logic crates: `paging`, `frames`, `slab`, `elf`, `capability`, the allocators, the
ring validators. No tool here can prove `trap.s`, which is why
`notes/arch-audit.md` calls the hand-written architecture assembly the least-verified code in the
TCB and why milestone 20 (a portable HAL, proven on a second architecture) says the same. An audit
by reading is the compensating control, and on the largest of the three trees it has not been paid
at all.

It is the newest port and has had the least review time. aarch64 has been read since 2026-07-12,
riscv64 since the parity work, and both were read deliberately on 2026-07-29. The x86_64 arch layer
landed 2026-08-23 (commit `797a20ac`, milestone 161 (the x86_64 kernel port), BUILT 2026-09-19).
Four days of elapsed time when this block was written, no dedicated read.

Less of it is exercised than on the other two ISAs. Every x86_64 boot in this tree was QEMU q35
when this was written; xenon first booted nife on bare metal on 2026-09-04, under milestone 87
(the x86_64 bare-metal machine). That runner's own header says what is not attached: "no virtio disks, no NIC, no GPU, no RNG", with
NVMe wired as the exception. x86_64 also had no interactive boot at all until milestone 182
(x86_64's own interactive-boot entry point) was built on 2026-09-19. So the suite does reach the
trap path, the MMU, the timer and the interrupt controller on every run. A reader should not
imagine this code is unexecuted. What it does not reach is the device and userspace surface the
other two runners cover. The narrower claim is the true one: on aarch64 a mistake has more ways to
become a failing test than it does here.

**The caveat, stated so this is not manufactured urgency.** By exposure, this is the *least* urgent
of the three architecture trees, for the same reason it is the least reviewed: nobody is running it.
The argument above is not "x86_64 is on fire". It is that the compensating control this tree relies
on for its least-verified code has been applied to two of the three architectures and to neither of
them recently. The gap is invisible to every gate we run (see below). The cost of closing it is
known, because the same read has been done once already at a comparable size.

## What makes it tractable

The original pass read 6,202 lines under one lens and produced three findings, a longer cleared
list, and two bounding facts. It was one lane. This is 6,797 lines, so the honest price is **three
lanes** if the passes are cut by risk rather than by file, and each pass is then *smaller* than the
single pass that already proved affordable.

The cut below is a partition: the three sums are 1,741, 1,722 and 3,334, and they total 6,797.

Pass 1: the entry and exit path (1,741 lines). `trap.s` (303), `exceptions.rs` (934),
`context.s` (89), `context.rs` (94), `segments.rs` (321). This is where the bug class lives, it is
smaller than either ISA the original read, and it should go first because everything else is
measured against what it establishes.

The bounding facts to re-establish here, which is the first hour's work and not the audit proper:

- One way out, confirmed. `grep -E '^\s*(iretq|sysretq)'` over the tree finds exactly one
  instruction, `trap.s:182`. The original note's "no second way out of the kernel to audit" transfers.
- One frame-fabrication site, `TrapFrame::for_user_entry` at `exceptions.rs:134`, matching both
  other ISAs. Transfers.
- **Two ways *in*, and this one does not transfer.** `exceptions.rs` configures `IA32_LSTAR`,
  `IA32_STAR`, `IA32_FMASK` and `IA32_EFER.SCE`, so `syscall` is a second entry whose masking
  discipline is set by an MSR rather than by a gate descriptor. The original audit's archetype is
  "a path that enters the save/restore sequence with interrupts live", and what `IA32_FMASK` clears
  is exactly the question that archetype asks. Establish it before reading anything else.
- `swapgs` is the x86_64 archetype of the class, and neither other ISA has one. Three
  instructions, `trap.s:105`, `:178`, `:206`. The window is single-copy by construction: between the
  exit `swapgs` and the `iretq` the CPU is in ring 0 holding the *user's* GS base, and the comment
  above `serve_shootdown_nmi` in `mmu/shootdown.rs` already documents that window. aarch64's per-CPU pointer is `TPIDR_EL1`, a
  system register no frame carries; riscv64's is `tp`, whose migration hazard the original audit
  already cleared. So the class transfers to x86_64 with an instance that has no precedent in the
  note, which is the best possible sign that the lens is still worth aiming.

Pass 2: masking and the interrupt-controller adapter (1,722 lines). `interrupt_controller.rs` (789),
`mod.rs` (411), `timer.rs` (351), `port.rs` (109), `interrupts.rs` (62). Second because finding 3 is
the shape most likely to repeat: it was not a missing barrier, it was one ISA's driver written as if
its hardware gave a guarantee it does not. The asymmetry that points here is a line count.
`interrupt_controller.rs` is 789 lines on x86_64 against 90 on aarch64 and 208 on riscv64, because it holds the LAPIC
and IOAPIC where the others hold a thin adapter over a driver in `drivers/`. Eight times aarch64's
adapter is eight times the surface for finding 3's shape, and none of it has a `drivers/gic.rs` to
have taken a lock already.

Pass 3: address space, bring-up, and the rest (3,334 lines). `mmu.rs` (1,295), `machine.rs`
(618), `boot.s` (457), `iommu.rs` (368), `rtc.rs` (197), `ap_boot.rs` (183), `isa.rs` (156),
`semihosting.rs` (60). Last because the original pass cleared the analogous code on both ISAs by a
structural argument that is cheap to re-check. `boot.s` and the MMU-enable sequences run on a core
with no trap vector installed and interrupts masked. They are uninterruptible by construction
rather than by care. If that argument holds on x86_64 the pass is short; `ap_boot.rs` and
`machine.rs` have no analogue in the note and are where it is most likely not to.

One finding's disposition must be re-argued rather than inherited. Finding 2 (riscv64's
`trap_entry` parking a user-controlled value in `sscratch` across the faultable frame stores) was
left documented, on a cost argument. Hardening it costs a store and a load on the hottest path in
the kernel, to guard a case behind a kernel-stack overflow that is already fatal. Option 2 in that
note is "a separate per-hart trap stack, entered when `SPP = 0`", and x86_64 has that in
hardware: `segments.rs` builds a TSS with seven IST slots, and an IST vector switches stacks
unconditionally. So the same question reaches x86_64 with the expensive half already paid, and the
answer may well differ. Inheriting the riscv64 disposition would be the exact mistake finding 1
recorded as its general lesson.

## The cadence question: a second milestone, not half of this one

Finding 8's sweep also claimed, in pull request #568's body and in no file, that the audit cadence
"has no notion of an architecture". Checked against `script/audits`, it holds: an entire ISA moved
none of the counted triggers, and the cadence tracks an audit's kind rather than its lens.

The decision: split. This milestone is the reading half, and teaching the cadence what an
architecture is goes to a second milestone, proposed provisionally for calef to mint. The strongest
reason is that the gate must not be written by the lane that satisfies it. Neither blocks the other.

[`the-cadence-question.md`](0187-x86-64-arch-audit/the-cadence-question.md) holds the check, the
reasons for the split, and the questions the second milestone decides.

## What this audit cannot find

The `BUGS` posture, stated before the work rather than after it, because every item here is a limit
a reader should know when they read the resulting note.

The headline limit is the original note's own closing paragraph, and nothing this milestone does
changes it:

> That is a reassuring result, and it should be read with its limit attached: an audit by reading
> finds what the reader thinks to look for. The original bug was found by a failure, not by
> inspection, and it had survived every previous reading of that file.
>
> -- notes/arch-audit.md

The rest, specific to this pass:

- **One lens is one lens, not coverage.** Reading 6,797 lines for state staged across single-copy
  registers will not find x86_64's memory-ordering bugs, its integer overflows, its IOMMU
  misconfigurations, or its ACPI parsing. Those are other audits, and milestone 43 (a second
  security audit, with a different lens) is the precedent for taking them separately rather than
  pretending one pass covers a surface.
- **Most findings will not be confirmable by running anything**, and that is a property of the bug
  class rather than of x86_64. Every window this lens looks for is a few instructions wide and
  closed by an invariant somewhere else, so the usual confirmation (a test that fails without the
  fix) is mostly unavailable. The original audit said so about finding 3 in its own words: a loop of
  two harts hammering the PLIC's enable bits passes with the lock and passes without it. x86_64
  makes it somewhat worse, because the device surface the suite does not attach is a set of callers
  that cannot be made to race at all today.
- It proves nothing. After this milestone the honest position is what it was after the last one:
  `vectors.s`, `trap.s` and their x86_64 twin are trusted rather than verified, and notes like these
  are how the project pays for that.
- It does not re-audit the two ISAs that grew underneath their own audit (*What this does not
  decide*).
- A cleared candidate is cleared as of one commit. The cleared list is the more durable half of
  the output and it is also the half that rots silently, because nothing recomputes it.

## What a lane taking this should do

1. Re-establish the bounding facts in pass 1 before reading for findings. They are cheap and they
   decide how much of the rest is finite.
2. Read pass by pass, in the order above, one pull request per pass. A pass that finds nothing still
   ships its cleared list; that section is the deliverable, not the leftovers.
3. Fix what is a live defect and record what needs a decision. The original pass fixed two of
   three findings on review, as execution inside decided architecture, and left the third documented
   with its options priced. A finding that needs a design fork gets written up and raised, not
   settled mid-read.
4. Land the result as a note under `notes/`, named by calef, and add the rows to **both** tables of
   `design/audit-reports/README.md` with counts taken from `script/audits --baseline` at the landing
   commit. Which `kind` the row carries depends on the cadence milestone: if it has landed, use the
   kind it creates; if not, record it as `security` the way 2026-07-29 was, and let that milestone
   re-date it.

## What this does not decide

Whether the two ISAs that were read stay read. Re-reading their growth since 2026-07-29 (*The
scale, measured*) is a second body of work with its own size. It is left to the cadence milestone to
make visible rather than folded in. A mechanism that can see an unaudited architecture
should be able to see an audited one that moved, and building it once is cheaper than reading twice.

How the cadence learns about architectures. The three questions in
[`the-cadence-question.md`](0187-x86-64-arch-audit/the-cadence-question.md) are recorded, not
answered. Answering them means touching `script/audits`,
the audit index, and what `script/lint` runs on every pull request, and this milestone deliberately
touches none of the three.

Anything a finding turns out to require. A finding that needs a memory-ordering decision
(DECISIONS rule 4), a change to the trap frame two programs agree on, or a hot-path cost is a fork
for calef, raised with its options priced the way finding 2 was. An audit that quietly redesigns the
thing it is auditing has stopped being an audit.

## Prior art

The in-tree precedent is the method itself, and it has run four times: `notes/security.md`
(2026-07-15), `notes/arch-audit.md` (2026-07-29), `notes/shared-page-audit.md` (2026-08-04), and
`notes/untrusted-input-audit.md` (2026-08-15). The second is the template and the others are
evidence that a single-lens pass is the shape that works here.

The three build-versus-reuse questions the roadmap asks of every milestone do not apply: nothing is
being built, so there is no code to use and no dependency to weigh. The ecosystem question that
would apply, how other microkernels compensate for unprovable assembly, is not researched here and
is not claimed. seL4's answer is known in outline (its proof stops at the same boundary and its
assembly is axiomatised) and is deliberately not cited, because `AGENTS.md` asks for prior art read
rather than recalled and this lane did not read it.

## What this unblocks

Nothing depends on it, which is the honest answer and part of why it went unnoticed. What it changes
is the confidence attached to every x86_64 claim the project makes. Milestones 182 (x86_64's own
interactive-boot entry point), 184 (extend the `std` port to x86_64) and the end of milestone
161, all BUILT since, were built on this layer. Each of them inherits an unread TCB. DECISIONS
§19 (architectural parity is a tenet) makes x86_64 a first-class target; this is the compensating
control that parity implies and that the other two targets have already had.

## BUGS

Not started; nothing built yet to carry its own BUGS section. The limits of the work itself are
under *What this audit cannot find*, above, and are stated in advance on purpose: they are the
reasons a reader should not treat the resulting note as a clean bill of health.

## Index row

`notes/arch-audit.md` never read x86_64; its BUGS says so. Measured: `kernel/src/arch/x86_64` is 18
files and 6,797 lines, larger than the 6,202-line two-ISA tree that audit read in full, in the
code that note calls the least-verified in the TCB and that no prover reaches. Priced as three
passes cut by risk, each smaller than the one pass already proven affordable. The cadence's
blindness to this (an entire ISA moved none of `script/audits`' four counters) is argued out as a
separate milestone rather than folded in, so the gate is not written by the lane that satisfies
it.
