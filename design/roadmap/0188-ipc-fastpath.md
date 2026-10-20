---
status: PARTIAL
raised: 2026-08-28
milestone_dependencies: 708
decision_dependencies: 95
machine_requirements: none
specific_machine: radon (E5 on radon, milestone 708, is the measurement that decides phase 4)
needs_person: yes
---
# 188. The IPC fastpath: the gate measures a shape userspace does not use, and three cheaper cuts come before a hand-written path

Partial. Phases 1 to 3 are built and measured; phase 4 is an architect's and is untouched. The
results change the recommendation's arithmetic without changing its conclusion.

calef's framing: *"a hand-maintained IPC fastpath, so the common case stops paying for the general
one."* The title says the premise needs checking before the fastpath does. The gate that says we are
over target measures an IPC shape that no userspace program in this tree performs. And the largest
single item it reports on aarch64 is a symbol of which 94% is never fetched.

Phase 4 only needs the measurement and the go-ahead; phases 1 to 3 were a lane's own call, needed
nobody, and are done.
The decision is [§95 (hand-written IPC)](../decisions/0095-a-proven-ipc-fastpath.md), a hand-written
IPC fastpath and whether it can stay proven. It is `DECIDED`, and what calef decided is *"don't
decide yet"*, in two tiers. The eligibility predicate and its proof in `crates/ipc` are ratified as
buildable now. The fastpath itself stays gated on the one measurement that would justify it:
milestone 74 (cycle counters) on milestone 127 (the seL4 machine). So the decision field names §95
and nothing else, and the dependency graph resolves it: the ruling answered what it was asked, and
the wait that remains is E5, milestone 708 (an executed footprint ladder for radon). The milestone
field names 708, and the machine and person fields carry its evening at radon. The spend is still
calef's. Phase 4 is a standing verification obligation and a permanent maintenance cost, and this
block recommends against starting it until the measurement exists.

## Appendices

This page is complete on its own. The working behind it is in
[`0188-ipc-fastpath/`](0188-ipc-fastpath/README.md):

- [scoping.md](0188-ipc-fastpath/scoping.md): the starting numbers and their sources, and the
  tables behind the three findings.
- [design-space.md](0188-ipc-fastpath/design-space.md): what a fastpath may skip and must keep,
  the proofs and invariants, how two paths stay in agreement, and alternatives A to F priced.
- [phases-1-to-3.md](0188-ipc-fastpath/phases-1-to-3.md): what pull request #732 measured, with
  the before and after on every architecture.
- [phase-4-evidence.md](0188-ipc-fastpath/phase-4-evidence.md): the case for and against phase 4,
  and the E3 run on radon.

## What scoping found

The starting figures came from `bench/fastpath-*.txt` at fc7b04e7, written by
`script/fastpath-footprint`, which milestone 132 (the fast path's footprint) owns. On aarch64
`ipc_fastpath` was 5,788 bytes and `syscall_entry` 3,304. The target, under 4 KiB of fastpath
instructions and under 1 KiB of data per IPC, is stated in
`notes/benchmarks/kernel-footprint-and-caches.md` ("The target"). Every architecture was over it on
`ipc_fastpath` alone.

The brief's premise had already moved. Milestone 132 named `syscall::dispatch` at 2,024 bytes as the
largest single item. Milestone 156 (extract the rest and ratchet both ways) has since cut it to 1,160
by moving rare arms into `#[inline(never)]` functions. That cheap option was built and measured
before this block existed.

1. The gate measured the wrong shape. Its roots are `ipc_send` and `ipc_receive`, the shape of the
   `ipc_rtt_el0` benchmark. Services use `CALL`, `RECEIVE_CAP` and `REPLY`: 25 files imported
   `call` against 12 programs using both `send` and `receive`. Re-rooted on the same binary, the
   CALL/reply closure was 7,516 bytes, 88% over target, against the gate's 5,788 (45% over). Icount
   saw only 2 to 3% of that 30% footprint difference.
2. aarch64's entry figure counted a table a syscall never fetches. `exception_vectors` is 2,020 of
   the 3,304 bytes, and a syscall from EL0 fetches one 128-byte entry of its sixteen. The honest
   figure was about 1,412. A lane could have "won" 1,892 bytes without changing a fetched
   instruction, which made the gate gameable. `exception_restore` (92 bytes) was counted by neither
   figure.
3. The largest skippable block is the scheduler, not the decoder. `schedule`, `finish_switch`,
   `wake`, `kmem::recycle` and `switch_to` are 2,872 bytes, 38% of the CALL/reply closure. They
   re-pick a thread the IPC already knows.

## What a fastpath may skip and what it must keep

A fastpath may skip the general scheduler re-selection, the error-return plumbing
(`take_ipc_aborted()`), and possibly `trace::record` and `kmem::recycle`. It must keep the
capability lookup, the rights check, the one-shot Reply mint of DECISIONS §12 (a one-shot reply
capability), the `WaitRole::Reply` guard, and generational thread-id revalidation. Eliding any of
those turns a fastpath into a hole.

The Kani harnesses in `crates/inter_process_communication` and `crates/capability` prove the pure
decision core, not `kernel/src/sched.rs`. That leaves phase 4 two options:

1. The fastpath calls the proved `Rendezvous` methods too, and skips only the scheduler and the
   error plumbing. The proofs keep covering it for free. Build and measure this first.
2. The fastpath replicates the decision. It then needs its own harnesses and an equivalence proof
   against the general path, plus a differential test for the plumbing the proof does not model.

Either path must hold five invariants. A message reaches exactly one receiver or none. A capability
is never widened. A Reply is consumed on first use. A blocked thread is on at most one queue. A
mailbox is written only while its thread is parked in a role that expects the write. The fifth is
held today by a comment and a runtime check, not a harness.

Keeping two paths in agreement is a cost that never ends. The block's preference follows the
AGENTS.md ladder: make divergence unrepresentable (option 1), then a gate (the equivalence harness),
and a comment only as the floor.

Six alternatives were priced. Extracting cold arms (A) was already built by milestone 156 for entry.
Reordering the decode (B) and shrinking validation (C) were refused. B moves no bytes, and C's
validation is the security check. A hot/cold decoder split (D) is subsumed by A. Fixing the
measurement first (E) is the recommendation. A hand-written fastpath (F) is deferred, not refused.

## The plan, and where each phase stands

1. Phase 1, measure the shape the system runs. Built: the gate reports `ipc_send_receive` and
   `ipc_call_reply` (provisional names). `ipc_fastpath` survives as the worse of the two, recorded
   rather than checked. notes/benchmarks.md now says three syscalls to seL4's two, not four.
2. Phase 2, make the entry figure honest. Built: aarch64 counts one vector slot plus
   `exception_restore`, and `syscall_entry` went 3,304 to 1,508. Nothing got faster; the accounting
   was wrong.
3. Phase 3, apply milestone 156's method to the closure. Built, and the method did not transfer:
   outlining four cold arms made the closure bigger, because the walk follows the new call. The gate
   now reads `#[cold]` from the source instead of a list inside itself. That took 6 to 10% off both
   closures for +0.17% to +0.24% retired instructions, and `kmem::recycle` left the closure.
4. Phase 4, the hand-written fastpath. Not started, held for calef.

| after phase 3 | `ipc_send_receive` | `ipc_call_reply` | `syscall_entry` | total |
|---|---|---|---|---|
| aarch64 | 5,356 | 7,028 | 1,508 | 8,536 |
| riscv64 | 4,632 | 5,936 | 1,828 | 7,764 |
| x86_64 | 6,236 | 8,122 | 1,637 | 9,759 |

`total` is the larger closure plus entry. The re-measurement also found the block's import counts
had grown with the tree (`call` 35 files, `send` 64), which is why the gate reports two shapes
rather than swapping one for the other.

## Phase 4, which is the question calef is holding

The gap is not closed. `ipc_call_reply` is 48% to 103% over the 4 KiB target after phases 1 to 3.
The original plan said phase 4 was not worth building if phase 3 got under 4 KiB, and it did not.

Three things moved in phase 4's favor. The number it would be measured against can now be trusted.
The scheduler bypass is still the largest skippable block, 2,380 bytes or 34% of aarch64's closure.
And milestone 74's riscv64 half gives radon real cycles through the SBI PMU. Against it: cycles show
whether a round trip got faster, not why. Liedtke's claim is about displacement, which milestone
134's E4 measures.

E3 ran on radon on 2026-09-04 and did not decide it. Padding added +1.49% to `call_reply`, but it
made the EL0 round trip 3.01% faster. Dead code cannot do that, so the layout moved. The layout
effect is about 193 ns and the footprint effect about 19 ns. A Cargo feature that inserts a symbol
moves everything after it, the Mytkowicz result. Until
[milestone 370 (a layout control for the perturbation experiments)](0370-a-layout-control-for-the-perturbation-experiments.md)
runs the padding as a dose, no E3 figure should be quoted as a footprint result. The run did settle
one thing: radon resolves these differences to 0 to 2 units. The measurement is precise; the
attribution is missing.

The recommendation stands. Do not start phase 4 on footprint arithmetic alone. Start it when a layout
control shows a latency difference that footprint explains. Then build option 1 first, aimed at that
34%. If no such difference appears, this milestone closes saying phase 4 was not worth its permanent
verification cost.

## Open questions, and the one that is an architect's

`ReplyRecv` fusion is a syscall-surface change and is explicitly not proposed here. seL4's round trip
is two syscalls because it fuses reply-and-wait. Ours is three because a server issues `REPLY` and
then `RECEIVE_CAP` separately. Fusing them would be a new method on `Reply` or on `Rendezvous`, and
DECISIONS §10 (process model: capability-based, microkernel) and §16 (object revocation) govern that surface. AGENTS.md puts anything two programs agree on in the
irreversible category. It is named here so it is tracked and not so it is planned. Its cost is one
trap per round trip, which the tree has never measured on its own. notes/benchmarks.md says the
kernel-side `call_reply` bench measures the fused shape, but *"there is no EL0 twin of it, so the
structurally matched comparison to seL4's published pair is not currently measured at all"*. Measuring it is phase 1 work; deciding it is an architect's.

Left open, for a lane rather than for calef:

- Whether the two closures should be two gated numbers or one, and if one, which.
- Whether the fastpath predicate should be a function both paths call or a duplicated condition
  with an equivalence proof.
- Whether the data-footprint half of the target (under 1 KiB touched per IPC), which milestone 132
  records as measured by nothing at all, can be estimated from the structures this path touches
  without waiting for a PMU.

## What this cannot deliver

- No speedup anyone can currently observe. Everything above is static footprint. The instrument
  that would show the harm does not exist on the machines in this house.
- No answer for x86_64's `ipc_fastpath` being the largest of the three. Pull request #574
  established that this is the same portable Rust in a denser ISA rather than a different path. So
  there is no x86_64-specific work hiding here, and milestone 161 (the x86_64 kernel port) owns
  anything that turns out to be.
- No change to the syscall surface. A fastpath changes how a call is serviced, not what calls
  exist.
- Nothing about the data half of Liedtke's pair. The under-1-KiB target has no instrument and
  this milestone does not build one.

## BUGS

Phase 4 is not started, so there is nothing of its own to carry a `BUGS` section. Three limitations
of the gate are recorded where a reader meets them, in `script/fastpath-footprint` and milestone
132. Phases 1 and 2 fixed all three: the root list measured a shape userspace does not run, the
aarch64 entry figure counted sixteen vector entries where a syscall fetches one, and
`exception_restore` was counted by neither figure. What remains is in the Follow-on below.

## Scope note

Phases 1 to 3 are tooling, a baseline re-record and a mechanical extraction: no syscall surface, no
dependency, no wire format, no `DECISIONS` section. Phase 4 is none of those things and would need
its own scope note, its own `DECISIONS` section, and calef.

Names: nothing new is minted here beyond phase 1's two provisional figure names. They are calef's
like every other name in the tree.

## Follow-on

- **Recorded.** The closure half of `script/fastpath-footprint` counts bytes no IPC fetches, for the
  same reason its flat half does: whole symbol sizes, so a cold tail inside `ipc_call` or
  `ipc_receive_cap` counts. Phase 3 outlined the arms it could justify; the rest are inside conditional
  branches the script deliberately does not follow. In the script's own `BUGS` section, beside the
  measurement.
- **Recorded.** `--features fastpath_pad` still works on two of three architectures, because
  x86_64 has no `fastpath_pad` module where `kernel/src/arch/aarch64/fastpath_pad.rs` and
  `kernel/src/arch/riscv64/fastpath_pad.rs` exist. In `script/fastpath-footprint`'s `BUGS` section.
  It blocks running milestone 134's E3 on xenon and not on radon, which is the machine that matters
  for the paragraph above.
- **Done.** Run milestone 134's E3 (the footprint-perturbation experiment) on radon with
  milestone 74's cycle counters, which this block named as the measurement that decides phase 4.
  Run 2026-09-04, six interleaved boots; notes/footprint-perturbation.md. It did not decide phase 4,
  for the reason the section above gives: the comparison cannot separate footprint from code layout,
  and the layout artifact is ten times the footprint effect.
- **Milestone 370.** A layout control for the perturbation experiments,
  `design/roadmap/0370-a-layout-control-for-the-perturbation-experiments.md`. E5 (milestone 708)
  reserves its region in the section 370 places. No E3 number is a footprint result, this block's
  included: E3 cannot separate footprint from layout, and calef retired its question for E5.
- **Milestone 380.** DECISIONS §144's 16 KiB ceiling was stated over "the sum of `ipc_fastpath` and
  `syscall_entry`", and this milestone changed both terms. The honest subject is now
  `max(ipc_send_receive, ipc_call_reply) + syscall_entry`, which is what the gate prints as `total`,
  and the headroom §144 recorded was measured on the smaller shape (x86_64 is now 60% of the
  ceiling, not 51%). `design/decisions/` is not a lane's to amend, so it was written up as
  `design/roadmap/0380-the-ceiling-applies-to-a-number-that-moved.md` for whoever holds §144, and
  they folded it into §144 the same day, which is why that block is BUILT.
- **Outstanding.** Phase 4 itself, the hand-written fastpath. Untouched, gated on calef, and the
  section above says what would decide it. Checked against the tree: `kernel/src/sched.rs` has one
  path through `ipc_call`, `ipc_receive_cap` and `ipc_reply` and no second one.
- **Refused.** `ReplyRecv` fusion, which would take the round trip from three syscalls to two. It is
  a syscall-surface change, DECISIONS §10 and §16 govern it, and the block already says it is named
  so it is tracked and not so it is planned. A lane must not take it.
  This refusal is milestone 467 (design/roadmap/0467-replyrecv-fusion.md), which carries it with the
  condition that would change it.

## Index row

Minted from the lane that gated the footprint on x86_64 (#574), whose finding was that `syscall::dispatch` is the largest single item in every architecture's entry figure. Scoping it
found the premise had moved. Milestone 156 (extract the rest and ratchet both ways) already cut `dispatch` from 2,024 to 1,160 bytes on
aarch64. `script/fastpath-footprint`'s roots measure a SEND/RECEIVE shape that 25 of 69 userspace
programs replace with `CALL`/reply (7,516 bytes against the gate's 5,788). And 1,892 of aarch64's
3,304-byte entry figure is vector-table padding no syscall fetches. Recommends three cheap phases
before any hand-written second path. (Measure the right shape, make the entry figure honest, apply
milestone 156's extraction to the closure.) That cannot be justified until milestone 74's PMU
on milestone 127's TX1 can observe the payoff. Gate: DECISION. PARTIAL 2026-09-04: phases 1 to
3 built. Both premises held. The gate now reports `ipc_send_receive` and `ipc_call_reply` separately
(the shape services run is 25 to 29% larger), and aarch64's `syscall_entry` counts one vector
entry instead of sixteen (3,304 to 1,508, accounting, nothing got faster). Phase 3's finding:
milestone 156's extraction does not transfer to a closure, and the gate now reads `#[cold]`
from the source rather than a list inside itself; 6 to 10% off both closures for +0.2%
instructions. The gap is not closed, so phase 4 stays an architect's.
