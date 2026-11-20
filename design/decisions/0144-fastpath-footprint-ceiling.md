---
status: DECIDED
raised: 2026-09-04
decided: 2026-09-04
ratified_by: calef
---

# 144. The fastpath footprint gate gets a delta and a ceiling, and the ceiling is 16 KiB

<!-- writing-standards: exception. Marked 2026-10-03 (UTC) by the maintainer session. Reason: this change
touches the section only to correct a false claim about what is built (the delta against main is
decided, not built). Bringing a 1,200-word section to 4 bold spans per 1,000 words is a rewrite for
the section's owner, not something to hide inside a correction (the measured class is recorded in
design/roadmap/0586-a-prose-ratchet-in-lint.md). Remove this marker when that rewrite lands. -->

*The delta against main below is decided and not built. Until milestone 336
(design/roadmap/0336-fastpath-footprint-against-main.md, NOT-STARTED) the gate compares against
`bench/fastpath-*.txt`, which is why those files and their re-saves exist.*

calef, 2026-09-04, after three failures of the stored-baseline design in three
days.

## What is being decided

`script/fastpath-footprint` compares the IPC fastpath's code size against a stored baseline file
per architecture and fails at a 5% tolerance. The proposal
`design/roadmap/0336-fastpath-footprint-against-main.md` asked whether it should instead compare
against `main` at pull-request time. The answer is both, and the reason each is needed is that they
catch different failures.

## The shape

A delta check against `main`. No stored state, so nothing can go stale and nobody has to remember
to re-record. This replaces the baseline files.

Plus an absolute ceiling of 16 KiB, per architecture, on `script/fastpath-footprint`'s `total`.
Only an architect raises it.

`total` follows phases 1 to 3 of milestone 188 (the IPC fastpath: the gate measures a shape
userspace does not use):

- The gate reports `ipc_send_recv` and `ipc_call_reply` separately and takes the worse of the two,
  since one round trip is one shape or the other.
- aarch64's `system_call_entry` counts the one 128-byte exception-vector entry an `svc` fetches, not
  all sixteen (`ENTRY_SLICE`).

So the ceiling's subject is `max(ipc_send_recv, ipc_call_reply) + system_call_entry`, which the script
prints as `total`.

## Why a delta alone is not enough

The gate is named for an absolute property, and its own header says so: *"the IPC fastpath must
stay small enough to live in L1i"*, from Liedtke's *On micro-Kernel Construction* (SOSP 1995), which
argued Mach's IPC was slow because of the cache footprint of its hot path rather than anything
inherent to microkernels. A delta cannot express that claim. It answers "did this change grow it",
never "is it still small enough".

Its failure mode is also silent and is the mirror of the stored baseline's: every individual change
is within bound while the absolute size grows without limit, because the reference moves with the
tree.

## Why a stored baseline alone is not enough, and this is now measured three times

- **Milestone 237.** Two lanes each measured "within bound" against the same stale baseline and
  neither re-saved it, so aarch64 headroom fell from 3.9 points to 1.5 with the gate green
  throughout.
- 2026-09-04, the nightly bump. `nightly-2026-09-04` inlined `drivers::plic::disable` into
  `riscv_trap_body` and the entry set grew 194 bytes with no source change anywhere to attribute
  them to. A delta against `main` would have made that a non-event, because both sides compile with
  the same toolchain.
- 2026-09-04, milestone 220. Adding a dependency to the `kernel` crate flipped an inlining
  decision the other way and `system_call::dispatch` (1,160 bytes) vanished from the symbol table. The
  gate failed at 35.1% shrinking, and the script's own advice for a shrink ("just an explicit
  acknowledgment") would have re-recorded an under-measurement as the new truth.

And the drift is not one architecture's: measured with the compiler held fixed, `ipc_fastpath` sits at
+0.6% aarch64, +0.3% riscv64, +1.9% x86_64 against stored baselines, so aarch64 has re-drifted
within a week of milestone 237 attributing it and re-recording 5852.

## Where 16 KiB comes from, because a number without a derivation is a guess

notes/target-hardware.md's L1i requirement gives it. The binding constraint is 32 KB, which is
radon's SiFive U74 and xenon's Core i5-7500T, and is where frontier x86 has sat for two decades
because a virtually-indexed L1 is capped at page size times associativity and the ecosystem's page is
4 KB. 16 KiB is half of that, the point that note names as where Liedtke's argument genuinely
weakens: at a quarter of L1i the hot path leaves the application three quarters, and at half it does
not.

Measured when this section was written, the totals were aarch64 9,156 B, x86_64 8,404 B, riscv64
7,174 B, so the ceiling sat about 75% above the largest. After milestone 188 they are:

| | as recorded here | after milestone 188 | fraction of 16 KiB |
|---|---|---|---|
| aarch64 | 9,156 | 8,536 | 52% |
| riscv64 | 7,174 | 7,764 | 47% |
| x86_64 | 8,404 | **9,759** | **60%** |

x86_64 is the one to watch, and its total rose. Counting the shape services actually run adds
more there than any other correction removes, so the recorded headroom was 9 points optimistic
on that architecture. The ceiling now sits 68% above the largest rather than 75%. **The number is not
moved**: it fires rarely either way, and the correction is to the arithmetic beside it rather than to
the decision. A tighter number was rejected for a specific
reason: 8 KiB, a quarter of 32 KB, was floated and would have been red on the day it shipped,
since two architectures already exceed it. A ceiling that fails on arrival is a ceiling nobody trusts.

## What each half is for

| | catches | misses |
|---|---|---|
| delta against `main` | a change that grows the path | slow accumulation, since the reference moves |
| 16 KiB ceiling | accumulation, whatever caused it | a single change that is large but still under |

Neither is redundant, and 2026-09-04 demonstrated both failure modes within one day.

## BUGS

- The measurement is an upper bound and a loose one. Whole symbol sizes, so a cold tail inside a
  hot function counts. The bytes an IPC actually touches are fewer and nobody knows by how much, so
  16 KiB is a ceiling on a pessimistic figure.
- **`system_call_entry` is not comparable across ISAs.** x86_64's `syscall` goes through `IA32_LSTAR` and
  never touches the IDT, so its entry figure legitimately excludes about 3,330 bytes the other two
  include. The ceiling is applied per architecture for that reason and must not be read as a ranking.
- Nothing has observed the effect either half protects against. No cache is modeled by icount and
  the development host's L1i is several times the boards'. This is a 1995 argument plus a measured
  code size, not a measured miss rate, and the experiment that would settle it wants the performance
  counters milestone 74 landed on radon on 2026-09-04.
- A delta against `main` costs a second build per run. Whether that is material on the merge
  queue's critical path is measurable and has not been measured; the prover is already the long pole.
