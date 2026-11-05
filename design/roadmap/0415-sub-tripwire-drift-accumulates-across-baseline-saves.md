---
status: BUILT
raised: 2026-09-15
built: 2026-09-26
promoted_from: sub-tripwire-drift-accumulates-across-baseline-saves
---
# 415. Sub-tripwire drift accumulates across baseline saves, and one architecture has no gate at all

Items 1, 2 and 3 are built and item 4 is refused. Item 1 landed as commit `ba99c83`. Item 2 landed in
PR #1126 under milestone 302 (a baseline records what it was saved against, and a stale one fails
loudly). Item 3 landed in PR #1375, as the report calef ruled for in §190 (must an icount baseline
save record why it moved).

calef ruled item 3 on 2026-09-26 as "3b", a weekly report of cumulative drift since a fixed
anchor, not a gate; the ruling is quoted at the top of §190. Item 2 had been ruled under milestone
302 on 2026-09-16, before this block's gate was written.

Premise re-checked 2026-09-26. Item 1: `script/ci-build`'s bench entry still reads
`script/bench --check && script/bench --riscv --check && script/bench --x86 --check`. Item 2:
`cargo xtask bench --save` refuses to run without `--why` and writes one `# why:` line per reason
(`xtask/src/bench.rs`, `save_reasons` and `baseline_header`); the three floors carry them since
2026-09-23. Item 3: `bench/` still holds one baseline per architecture and nothing reports drift
against an anchor. Item 4 stays refused for the reason milestone 25 already established.

## What item 3 shipped

A report, not a gate, as ruled. `helpers/baseline_drift.py` (name provisional) walks the
first-parent history of the three baseline files from git alone and computes, per row and per
architecture, the per-iteration move since a fixed anchor. It compounds on the compiler term
`cargo xtask bench --restamp` has carried since the last `--save`, read back from its `# why:`
ledger, because a restamp moves `# toolchain:` without moving a number. `script/metrics` publishes
it in the bold backlog's shape. They are `notes/project-metrics/baseline-drift.csv` (the largest upward move
and the rows past 5%, per architecture per week, restated from 2026W31), a chart of rows past 5% on
`notes/project-metrics.md` with a generated line under it, and the generated appendix
`notes/project-metrics/baseline-drift.md`. That appendix lists every row, then every save since the
anchor with its `# why:` lines, or its commit subject for saves that predate the ledger.

The anchor is the 2026-09-15 audit's, one commit per architecture: `74431429` for aarch64, the
first save after the one-hart fix and the QEMU pin, and each file's birth for the other two. Refused:
the first save that carries `# why:` lines (2026-09-23). It would read every row as zero on the day
the report started, and riscv64's `ctx_switch`, the +10.78% that opened this block, would vanish from
the one place meant to show it. `python3 helpers/baseline_drift.py --audit` reproduces all five of
the audit's figures exactly from these anchors, and `--selftest` checks the arithmetic against
fixtures in `script/lint`.

What it read at `7b1bab550`: 15 rows sit more than 5% above their anchor. The largest is
`spawn_reap`, +24.79% on aarch64 and +20.91% on riscv64, most of it one +31 to +34% step on
2026-08-16 that tripped the gate and was re-saved on purpose (the thread-stack overflow fix).
riscv64 `null_syscall` is +11.29% and `ctx_switch` +8.81%; x86_64's largest is `spawn_reap`,
+8.09%. No restamp has run yet, so every restamp term is zero.

## In brief

`cargo xtask bench --check` fails at >10% drift against the last saved baseline, and `--save`
rewrites that baseline. So N successive sub-threshold steps accumulate and the gate never fires.
The audit in [notes/benchmarks.md](../../notes/benchmarks.md) walked every `--save` event in the
history of all three baseline files, from git alone with no emulation, and the accumulation is real
and measurable:

| | cumulative drift | largest single step | times the gate fired |
|---|---:|---:|---:|
| riscv64 `ctx_switch` | +10.78% | +6.14% | 0 |
| aarch64 `yield_switch` | +9.16% | +6.49% | 0 |
| riscv64 `ipc_rtt` | +9.25% | +4.34% | 0 |
| aarch64 `ipc_rtt` | +8.73% | +4.95% | 0 |
| x86_64 `yield_switch` | +9.94% | +9.94% | 0 |

riscv64's `ctx_switch` is past the threshold the gate enforces, reached in steps none of which came
close to it. `coremark`, pure compute with no context switches, is flat to four decimal places
across every save on all three architectures, which is what says the rest is the kernel's switch and
IPC paths rather than measurement noise.

Most of that accumulation is honest, disclosed in the commit that caused it, and a good part of
it is whole-crate codegen churn the instrument cannot separate from real cost. The problem is not
that the drift exists. It is that nothing in the tree can tell the honest part from the rest, and
two saves on 2026-09-15 prove it.

## The live instance, which is not hypothetical

`44890a8a` re-saved the x86_64 baseline and attributed its +5 to +8% to the toolchain:
*"It also folds in the same nightly-2026-09-15 drift the other two carry."* Milestone 300 (decompose
the icount baseline drift) then measured that toolchain term across exactly those two nightlies and
found it ~0, byte-identical counts on the same code. PR #886 names the real cause, a const-`false`
element still threaded through the shared context-switch tuple that the debug build does not fold,
and recovers ~5.9% on x86_64 by deleting it.

So the x86_64 floor carries a removable regression, blessed into the baseline on a stated cause that
measures zero, and `--check` passed the whole time because 5.9% is under 10%. `85edb1ed` did the same
thing on aarch64 and riscv64 the same day, classifying the +6.5% as intended feature cost where #886
shows 91 to 93% of it is removable.

This is the move milestone 237 (the cycle-counter grant costs 136 bytes of IPC fastpath) exists to
refuse, one instrument over. Re-saving because the number moved launders growth into the new
normal and retires the gate that was supposed to catch it.

## What to do, cheapest first

### Item 1: Pull the x86_64 leg into CI. One line, owed already, needs no decision.

`script/ci-build`'s bench entry is `script/bench --check && script/bench --riscv --check`. The third
tripwire is built, its baseline is committed, and nothing pulls it; `ci.yml` already carries a
`BUGS` note saying so. The audit shows what that costs: x86_64 has had exactly two saves ever, and
the window between them is 1,526 commits. On the two gated architectures a gross regression
eventually forces a save, so the record has granularity. On the third nothing forces one, which is
why its single step arrived at +9.94% wearing a false attribution.

Cost: one line. It may fail the first time it runs, which is why `ci.yml`'s note says it wants its
own commit rather than riding along with something else.

Done, on 2026-09-15, in its own commit. `ba99c83` ("ci: gate the x86_64 icount baseline, which
nothing ever ran") made the bench entry
`script/bench --check && script/bench --riscv --check && script/bench --x86 --check`, on the
argument this section makes and with the same 1,526-commit window as its evidence. It was safe to
gate at that point because PR #886 had already recovered the ~5.9% x86_64 regression and re-saved
that baseline, so the floor it checks against is the recovered one. That is what makes this block
`PARTIAL` rather than `NOT-STARTED`.

### Item 2: Make a save record its own attribution, beside the number. Recommended.

Require `--save` to carry a reason and write it into the file next to the rows that moved, refusing
to write without one. Today the only record of why a number moved is the commit message, which is
rung four of AGENTS.md's ladder: read once, by one person, on the day it is written. Both 2026-09-15
mis-classifications are in commit messages that nobody re-read until a lane went looking six weeks
later.

Writing the attribution into the baseline file moves it to rung three, beside the thing a reader
meets, which is milestone 115's shape. It also makes the wrong state visible rather than
unrepresentable, and that is the honest limit: a lane can still write a false reason, as
`44890a8a` did. What changes is that the false reason is then in the file the next reader opens,
where the audit above would have found it in a grep instead of a bisect.

Cost: roughly the `--save` writer plus a flag, and the rows it already knows have changed. Small.

### Item 3: A cumulative check against a fixed historical anchor. Real, and more expensive than it looks.

Keep a second per-architecture file holding a historical anchor and fail when today's number drifts
more than some bound from it, in addition to the last-floor check. This is the mechanism that
directly answers the structural hole, and the audit is the evidence that it would fire.

The cost is not the code, which is a second file and a second comparison. It is that the anchor
goes stale for correct reasons. Milestone 139 (drive the unsafe count down) is a decided feature
with a real cost at the switch. `spawn_el0` legitimately fell 32.7% on both ISAs when
`b918337b` bounded a walk by occupancy. An anchor with no ledger of intended deltas would fire on
both, and a gate that fires on correct work is the shape §61 and milestone 78 already dropped
checks for. So this option is really "an anchor plus a per-benchmark ledger of what was intended",
and the ledger is the expensive half. Item 2 is most of that ledger, which is why it comes first.

### Item 4: Tighten the threshold. Refused, and the tree already knows why.

Milestone 25 (cross-OS performance comparison) demoted `--check` from a 2% gate to the coarse 10%
tripwire deliberately, because adding unrelated live code moves untouched benchmarks several percent
non-uniformly through whole-crate inlining decisions. The audit re-confirms it: `9890eb02` moved
every kernel-side IPC row by 4 to 8.5% by adding one benchmark. Tightening the threshold rebuilds
the false-positive problem that demotion was written to escape, and it would not have caught either
2026-09-15 step, both of which were real costs honestly measured and wrongly explained.

A per-benchmark drift budget consumed across saves is item 3 with extra bookkeeping: it needs the
same anchor and the same intended-delta ledger, and adds a consumption rule on top. Not recommended
separately.

## Recommendation

Do 1 now, as its own commit. Do 2 next, because it is small and because it is the ledger that
item 3 would otherwise have to invent. Hold 3 until 2 has been in the tree long enough to say
whether the attributions it collects are good enough to gate on. Do not do 4.

The reversibility test says the same thing. 1 and 2 are undoable in an afternoon and nobody outside
this tree has acted on them, while 3 writes a second committed floor that every future measurement is
compared against. That is a fact other work starts depending on.

## How this bears on the toolchain-bump decision, which has since been answered

Corrected 2026-09-19. This section was written against calef's then-open decision 2 from PR #883
(`icount-baselines-drift-after-a-toolchain-bump.md`, since promoted as milestone 577 (the icount
baselines predate the pinned nightly) and closed in the same act). Should a toolchain bump
re-baseline in the
same pull request, or fail loudly? He ruled on 2026-09-16 and it is milestone 302, a baseline
that records what it was saved against and fails loudly when it is stale, which is the "fail
loudly" half. The audit below remains the evidence for that ruling rather than an argument toward
it.

That proposal's premise is that a new nightly's codegen invalidates the baselines. Milestone 300
measured it: across `nightly-2026-08-27` and `nightly-2026-09-15` the toolchain term is ~0, and
the QEMU upgrade term is also ~0. On that evidence, a bump-triggered automatic re-baseline would
have written a new floor for a cause that measured zero, and in doing so would have absorbed
milestone 139's regression under a toolchain label. That is exactly what `44890a8a` did by hand.
That is an argument for the "fail loudly" half of the proposal's own option and against the
"re-baseline in the same pull request" half, and it is a measurement rather than a preference. It is
the half calef took.

## BUGS

- The cumulative numbers are anchored, and the anchor is a judgment. aarch64 is anchored at
  `74431429` (2026-07-30) rather than at its first save, because `60e75545` pinned the bench to one
  hart after finding the `-smp 4` counter was fiction, which re-means every earlier number. A
  different anchor gives different cumulative figures. The per-save tables are anchor-free and are
  the checkable record.
- This audit ran no benchmarks. Every figure is arithmetic over committed text, which is what
  made it cheap and is also its limit: it says what each save *recorded*, not what the tree measured
  between saves. A regression that appeared and was fixed inside one window is invisible here.
- The report inherits both limits above, and adds two. The restamp term is read by string shape,
  mirroring `prior_cumulative` in `xtask/src/restamp.rs`; if that ledger's marker or listing
  changes there, this reads a zero term silently, and nothing compares the two. The page's line
  and the appendix read HEAD while the CSV restates each week, so between runs they can differ.
- Item 2 cannot make an attribution true. It moves a claim from a commit message to the file, so
  a reader meets it. `44890a8a` would still have written its false toolchain attribution; it would
  just have been findable.

## Follow-on

- **Done.** Item 1, the x86_64 leg in CI, landed 2026-09-15 as commit `ba99c83` ("ci: gate the
  x86_64 icount baseline, which nothing ever ran"), in its own commit as the section asked, against
  the floor PR #886 had already recovered.
- **Done.** Item 2, a save that records its own attribution beside the number: PR #1126, commit
  `1c5dee224`, 2026-09-23, under milestone 302's ruling.
- **Done.** Item 3, as a weekly report of drift since a fixed anchor rather than a check: PR #1375,
  `helpers/baseline_drift.py` and the drift panel on `notes/project-metrics.md`.
- **Decision.** Whether the anchor becomes a gate, which calef left open "if the `--why` reasons
  prove good enough to gate on". It is his call and lives in
  `design/decisions/0190-what-a-baseline-save-must-record.md`.
- **Done.** The metrics workflow dropped the page's generated lines every morning, because the
  pathspec `notes/project-metrics` does not match `notes/project-metrics.md`. Fixed in PR #1375.
- **Refused.** Item 4, tightening the 10% threshold. Milestone 25 demoted `--check` from a 2% gate
  deliberately, the audit re-confirmed why, and it would not have caught either 2026-09-15 step.
- **Milestone 302.** The toolchain-bump question this block's last section was written against.
  calef ruled on 2026-09-16 and the work is milestone 302, a baseline that records what it was saved
  against and fails loudly when it is stale; the section is corrected to say so.

## Index row

`cargo xtask bench --check` fails at more than 10% drift against the last saved baseline and
`--save` rewrites that baseline, so successive sub-threshold steps accumulate and the gate never
fires. riscv64's `ctx_switch` is +10.78% cumulative in steps that never reached +6.2%, and the
gate has fired zero times on any of the five rows audited. `coremark` is flat to four decimal
places across every save on all three architectures. Two saves on 2026-09-15 blessed a removable
regression into the floor on a toolchain attribution that milestone 300 later measured at ~0. Item
1, pulling the ungated x86_64 leg into CI, landed the same day as commit `ba99c83`. Item 2 made a
save record its reasons beside the number (PR #1126). Item 3, ruled by calef on 2026-09-26 as a
report rather than a gate, publishes cumulative drift per row since the audit's anchors with every
save's reasons in the weekly metrics (PR #1375). 15 rows were past 5% on the day it shipped.
