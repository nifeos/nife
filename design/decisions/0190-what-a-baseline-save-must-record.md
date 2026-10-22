---
status: DECIDED
raised: 2026-09-19
decided: 2026-09-26
ratified_by: calef
---

# 190. Must an icount baseline save record why it moved, and does a second fixed anchor earn its cost?

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 415 (sub-tripwire drift accumulates across baseline saves)'s
`DECISION` gate naming no section and its gate prose gone stale. The audit behind it is the
baseline-audit lane's, 2026-09-15, which calef asked for after PR #886 found a regression that had
hidden under the 10% threshold. *(Section number provisional until the merge queue lands it.)*

## The ruling

calef, 2026-09-26, in answer to item 3: *"3b"*. Recorded at 18:36 UTC by milestone 415's lane,
which the maintainer delegated this edit to. The three options were put to him in the
maintainer's session, not in this file, so they are written out here as they were put.

- 3b, taken. Item 3 is a report, not a gate: cumulative drift per benchmark row since a fixed
  anchor, published with the weekly project metrics, with each save's `# why:` reasons beside it.
- 3a, refused: the anchor as a gate. It fires on correct work, such as `spawn_el0`'s legitimate
  -32.7%, and needs its own ledger of intended deltas.
- 3c, refused: not doing item 3. Slow drift is then visible only to someone who looks.
- It can become a gate later, if the `--why` reasons prove good enough to gate on.
- Items 1 and 2 were already done, item 2 in PR #1126 under milestone 302's ruling (next section).
  Item 4 stays refused.

Milestone 415 built it in PR #1375. `helpers/baseline_drift.py` does the arithmetic from git
alone, anchored at the 2026-09-15 audit's three commits, and reproduces that audit's five figures.
`script/metrics` publishes it as a generated appendix, `notes/project-metrics/baseline-drift.md`,
and charts the part of it on `main` that no line in `bench/attributions.tsv` accounts for (calef,
2026-10-22 UTC, on #2554).

The rest of this file is the section as it stood before the ruling.

## Item 2 is ruled and built, and item 3 is still open

Recorded 2026-09-26 by the decisions-hygiene lane, which invented no part of it. calef ruled item
2 on 2026-09-16, under milestone 302 (a baseline records what it was saved against, and a stale
one fails loudly), three days before this section was raised: its block reads *"The format is
comment lines, ratified by calef 2026-09-16"*. The lane that wrote this section did not find that
ruling, so the question below ("may a lane build item 2 without further ruling?") was already
answered when it was asked.

It shipped on 2026-09-23 in commit `1c5dee224` ("A baseline save records why the number moved,
beside the number"), PR #1126. `cargo xtask bench --save` now refuses to run without at least one
`--why "<reason>"` (`xtask/src/bench.rs`, the check before the kernel build). It writes one
`# why:` line per reason into the baseline's header beside a `# date:` line. `--check` prints those
reasons back when a row moves. The usage line is `xtask/src/main.rs:276`. So the "Item 2 is not
built" bullet below, and its `xtask/src/main.rs:7460` citation, describe the tree of 2026-09-19.

Item 3, the fixed historical anchor, has no ruling, and the section stays `PROPOSED` for it:
the decisions README has no status for a section half answered. The recommendation below already
sequenced it after item 2, and item 2 has now been in the tree since 2026-09-23.

## What is being decided

Milestone 415 named four items. One has landed and one is refused, so this section is about the
two that are left:

- Item 2. Require `cargo xtask bench --save` to carry a reason and write it into the baseline
  file next to the rows that moved, refusing to write without one.
- Item 3. Keep a second per-architecture file holding a fixed historical anchor, and fail when
  today's number drifts more than some bound from it, in addition to the last-floor check.

## Is the premise true

Checked 2026-09-19 in this worktree, item by item, because milestone 415's gate paragraph no longer
describes what is open.

- Item 1 landed. `script/ci-build:122` reads
  `bench|ci|script/bench --check && script/bench --riscv --check && script/bench --x86 --check`, so
  the x86_64 leg is gated. That was commit `ba99c83` on 2026-09-15, in its own commit as the section
  asked. The gate's clause "the first item is a one-line CI change that is owed already and needs
  nobody's permission" is therefore stale, and is corrected in milestone 415's block by this
  lane.
- Item 2 is not built. `xtask/src/main.rs:7460` parses `--save` as a bare flag beside `--check`
  and `--release`, takes no reason, and `bench/baseline-x86_64.txt`'s header says only that
  *"updating this file is a statement that a performance change is intended and understood; do it in
  the commit that causes the change."* So the only record of why a number moved is still the commit
  message.
- Item 3 is untouched. `bench/` holds one baseline file per architecture (`baseline-aarch64.txt`,
  `baseline-riscv64.txt`, `baseline-x86_64.txt`) and no anchor file.

## The structural hole, measured

`--check` fails at more than 10% drift against the last saved baseline, and `--save` rewrites
that baseline, so N successive sub-threshold steps accumulate and the gate never fires. The audit in
`notes/benchmarks.md` walked every `--save` event in the history of all three files, from git alone
with no emulation:

| | cumulative drift | largest single step | times the gate fired |
|---|---:|---:|---:|
| riscv64 `ctx_switch` | +10.78% | +6.14% | 0 |
| aarch64 `yield_switch` | +9.16% | +6.49% | 0 |
| riscv64 `ipc_rtt` | +9.25% | +4.34% | 0 |
| aarch64 `ipc_rtt` | +8.73% | +4.95% | 0 |
| x86_64 `yield_switch` | +9.94% | +9.94% | 0 |

`coremark`, pure compute with no context switches, is flat to four decimal places across every save
on all three architectures, which is what says the rest is the kernel's switch and IPC paths rather
than measurement noise.

Most of that accumulation is honest, disclosed in the commit that caused it. The problem is that
nothing in the tree can tell the honest part from the rest. Two saves on 2026-09-15 prove it.
`44890a8a` re-saved the x86_64 baseline attributing +5 to +8% to the toolchain. Milestone 300
(decompose the icount baseline drift) then measured that toolchain term across exactly those two
nightlies at ~0. PR #886 found the real cause and recovered ~5.9% by deleting a const-`false`
element still threaded through the shared context-switch tuple. `85edb1ed` did the same on the other
two architectures the same day.

This is the move milestone 237 exists to refuse, one instrument over: re-saving because the number
moved, which launders growth into the new normal and retires the gate that was supposed to catch it.

## What this tree already does in the analogous case

Rung three: the record goes beside the thing a reader meets. That is the shape of milestone 115 (the names that were
ratified, and the ones that were refused) and of §75 (directories under `design/` and `notes/`
carry provenance in their own README), and item 2 is exactly it. Today the attribution is rung four, a commit message read once by
one person on the day it is written; both 2026-09-15 mis-classifications sat in commit messages that
nobody re-read until a lane went looking six weeks later.

A gate that fires on correct work gets dropped. §61 (a lint is adopted on evidence from this tree, not on
its description) is the rule, and milestone 78 (the load-sensitive assertions, and the three that measure the
wrong thing) dropped checks on the same ground. That is item 3's
real cost, and it is not the code: the anchor goes stale for correct reasons. Milestone 139 (drive the unsafe count down) is a
decided feature with a real cost at the switch, and `spawn_el0` legitimately fell 32.7% on both
ISAs when `b918337b` bounded a walk by occupancy. An anchor with no ledger of intended deltas fires
on both.

And the threshold itself is already decided. Milestone 25 (cross-OS performance comparison)
demoted `--check` from a 2% gate to the coarse 10% tripwire deliberately. It did so because adding
unrelated live code moves untouched benchmarks several percent non-uniformly through whole-crate
inlining decisions. The audit re-confirms it, since `9890eb02` moved every kernel-side IPC row by 4
to 8.5% by adding one benchmark. Item 4, tightening the threshold, stays refused, and it would not
have caught either 2026-09-15 step, both of which were real costs honestly measured and wrongly
explained.

## The question this section actually puts to calef

Item 2 may not need a ruling at all, and that should be settled rather than assumed. Milestone
415's own reversibility paragraph says items 1 and 2 are undoable in an afternoon and nobody outside
this tree has acted on them, which by AGENTS.md's test makes item 2 a decision for whoever is
holding the problem. The counter-argument, and the one the gate rests on, is that it changes what
a person has to type on every bench evening on every board, which is calef's own workflow rather
than a lane's.

So: may a lane build item 2 without further ruling? If yes, milestone 415's gate becomes
`NONE` for that item and item 3 is what remains open. The lane that wrote this section did not
correct the token on its own reading, because getting that wrong puts a block on
`script/roadmap --ready` where a lane stalls on the fork, which is the worse of the two failures.

## Recommendation

Do item 2, and hold item 3 until item 2 has been in the tree long enough to say whether the
attributions it collects are good enough to gate on. Item 2 is most of the ledger item 3 would
otherwise have to invent, which is why it comes first rather than because it is smaller.

The honest limit of item 2, stated so it is not oversold: it cannot make an attribution true.
`44890a8a` would still have written its false toolchain reason. What changes is that the false
reason is then in the file the next reader opens, where the audit above would have found it in a
grep instead of a bisect. That is rung three rather than rung one, and this section does not claim
otherwise.

## How reversible, and who has acted on it

Item 2 is an afternoon and nobody outside this tree has acted on it. Item 3 writes a second
committed floor that every future measurement is compared against, which is a fact other work starts
depending on, so it is the one worth deliberating.

## What is blocked until this is answered

Milestone 415's remaining two items. Nothing else: item 1 is gated and the floor it checks
against is the recovered one, because PR #886 had already recovered the ~5.9% x86_64 regression and
re-saved that baseline before `ba99c83` landed.
