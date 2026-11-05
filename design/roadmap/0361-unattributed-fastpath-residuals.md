---
status: SUPERSEDED
raised: 2026-09-03
superseded_by: 188
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 361. The riscv64 and x86_64 fastpath residuals are unattributed, so their baselines cannot be re-saved

Phases 1 to 3 of milestone 188 (the IPC fastpath) cover this block: they re-measured and
re-recorded all three baselines on 2026-09-04 (pull request #732). Checked on 2026-09-19: the numbers this file is written against no
longer exist. Milestone 188 phase 1 split the single `ipc_fastpath` figure into `ipc_send_recv` and
`ipc_call_reply`, because the shape the system runs is `CALL` and the gate was measuring
`SEND`/`RECV`; `ipc_fastpath` survives as the worse of the two, derived and unchecked. All three
baselines were then re-saved against a build that was 6% to 10% smaller on every shape, and
`bench/fastpath-riscv64.txt` and `bench/fastpath-x86_64.txt` now read 5936 and 8122 against the 5106
and 6639 this file asks to bisect. The bisect was never performed and cannot now be performed
against those numbers. What remains of the concern is the standing one milestone 237 owns, that a
baseline can move with nobody attributing it, and 237's block is where it lives.

The measurement runs on the dev machine under emulation, the tooling
(`script/fastpath-footprint`) exists, and bisecting a size delta needs no hardware and no decision.

In brief. `script/fastpath-footprint` compares the IPC fastpath's code size against a stored
baseline per architecture. Two of the three have drifted and nobody knows why: riscv64 sits at
5132 against a 5106 baseline, and x86_64 at 6687 against 6639. Milestone 237 attributed the
aarch64 growth to the cycle-counter grant, fixed it, and re-recorded the aarch64 baseline only. The
work is to bisect each of the two remaining gaps to the milestone that caused it, then re-record
those baselines in the same commit that explains them.

## Why this matters

Milestone 237 exists to refuse a specific move: re-saving a baseline because the number moved, which
launders growth into the new normal and retires the gate that was supposed to catch it. Re-recording
riscv64 and x86_64 today, with the causes unknown, would be exactly that move. So the two baselines
stay stale, and while they are stale the headroom they report is not the headroom that exists.

That is the same trap 237 was minted to escape, one architecture over. Milestone 237's own block
records that two lanes each measured "within bound" against the same stale aarch64 baseline and
neither re-saved it, and aarch64 headroom fell from 3.9 points to 1.5 with nothing firing. Nothing
prevents the other two ISAs walking the same path, and they have a head start.

It is also an architectural parity question. AGENTS.md rule 5 makes parity a gate: a kernel
capability ships on every supported architecture or a scope note records the gap. A footprint budget
that is current on one ISA and stale on two is that gap, in the instrument rather than in the
feature.

## The numbers have moved since this was written, and aarch64 has rejoined them

**Re-measured 2026-09-04** by the PR #716 lane, on `main` at `44e33268`, built under
`nightly-2026-09-03` (the pin this proposal was written against, so the compiler is held fixed and
the deltas are the tree's own):

| ISA | `ipc_fastpath` | baseline | residual | this proposal said |
|---|---|---|---|---|
| aarch64 | 5888 | 5852 | +0.6% | (re-recorded by 237, no residual) |
| riscv64 | 5122 | 5106 | +0.3% | 5132, +0.5% |
| x86_64 | 6767 | 6639 | +1.9% | 6687, +0.7% |

Two things changed and both sharpen the case rather than weakening it.

aarch64 is drifting again. Milestone 237 attributed its growth to the cycle-counter grant, fixed
it, and re-recorded the baseline at 5852. That was eight days ago and it is at 5888. So the fix held
for a week, which is what this proposal predicts: re-recording an instance does nothing to a
mechanism that has nobody remembering it. All **three** ISAs now carry an unattributed
`ipc_fastpath` residual, not two.

**x86_64's residual has nearly tripled**, 0.7% to 1.9%, against a 5% bound. It is the closest of the
three to its ceiling and it is moving the fastest, which makes it the one to bisect first rather
than last. Nothing has fired, and nothing will until it has absorbed the remaining 3.1 points.

`syscall_entry` is a separate story and it is the healthy half. All three sat exactly on their
baselines under that build (3304, 1870, 1637). The drift is entirely in the closure half, which is
worth knowing before bisecting: whatever is accumulating is in the IPC and switch closure, not in
the trap entry path.

## Index row

Two of three fastpath footprint baselines had drifted with nobody able to say why, and milestone 237
refuses to re-save a baseline because the number moved. Milestone 188 overtook the question: phases
1 to 3 changed what the gate measures (two IPC shapes rather than one merged figure) and cut 6% to
10% off every shape by reading `#[cold]` out of the workspace's own source. The lane re-recorded all three baselines against the smaller numbers on 2026-09-04, so nobody can
bisect the residuals named here. When the proposal was measured, all three ISAs were drifting, x86_64 the fastest at 1.9%
against a 5% bound.
