---
status: BUILT
raised: 2026-10-02
built: 2026-10-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 626. Drift since the 2026-09-26 floors: one counter, one layering, and three ISAs

*(Number provisional until the merge queue lands it.)* The question came from PR #1474's riscv64
re-save, which recorded `map_new` +7.89%, `spawn_reap` +3.30% and `yield_switch` +2.98% that the
compiler did not cause, and asked why riscv64 alone. The measurement, the tables and the caveats are
in [notes/benchmarks/drift-since-the-2026-09-26-floors.md](../../notes/benchmarks/drift-since-the-2026-09-26-floors.md).
This block holds what was found. The attribution is built; the remedy is milestone 629's.

## What was found

- **The premise was false.** aarch64 and x86_64 drifted with the same shape. Their floors were only
  restamped, and a restamp never compares `main` to the floors. x86_64 `map_new` is at +8.96%, the
  row nearest the 10% tripwire on any ISA.
- **Milestone 126 (the `procps` package)'s per-switch counter is the switch and IPC drift on all
  three ISAs**, commit `0d90e750c` in #1360. Stubbing `machine_statistics::context_switch()` returns
  `yield_switch` and `ctx_switch` to within 0.14% of their floors everywhere. It is about 134
  instructions a switch at the -O0 the gate measures and 10 in release, where it still costs
  riscv64 `yield_switch` 4.5% and `ipc_rtt` 2.4%. The 24-to-32 capability-table raise in the same
  commit is not the cause of those rows; it is part of `spawn_reap`'s.
- Milestone 23 (a capability-routed component OS with live replacement)'s run `RETYPE` is most of `map_new`, commit
  `b4f657d8a` in #1373: about 206 instructions per page at -O0, +0.7% in release.
- Milestone 206 (the user address-space map) adds a fixed 43 ticks to `map_new`, commit
  `7a90fcc59` in #1352, once per run rather than per page. Its mechanism is not found.

## What was built

`script/bench --riscv` no longer inherits `NIFE_DISK`. It failed on a fresh checkout and attached
five disks it never reads; without them rows move by at most 0.05%.

## The remedy, which milestone 629 built

What the per-switch counter should cost was calef's call, because it is a cost a wanted feature pays.
He ruled on #1486 and then on #1489 (2026-10-03 UTC): keep the count exact and make it as cheap as it
can be. Milestone 629 (the context-switch statistic stops costing the switch path) built that, and
591148372 re-saved the floors on all three ISAs with the drift this block attributes named in the
commit.

## Follow-on

- **Done.** The remedy for the per-switch counter, as milestone 629.
- **Done.** Re-saving the aarch64 and x86_64 floors, in 591148372 (2026-10-03 UTC).
- **Recorded.** The +43 fixed term from `7a90fcc59`, in the BUGS section of
  [the note](../../notes/benchmarks/drift-since-the-2026-09-26-floors.md).
- **Milestone 415.** Drift on `main` between saves is invisible. The weekly drift report reads
  committed floors and the restamp measures only the compiler, so nothing compared `main` with the
  aarch64 and x86_64 floors until this block did. A weekly `--check` of `main` on all three ISAs,
  reported in the same file, would have shown it on the day #1360 merged.

## Index row

The riscv64 drift #1474 recorded is on all three ISAs. Milestone 126's per-switch counter costs
`yield_switch` about 3% at -O0 and 4.5% in release; milestone 23's run `RETYPE` costs `map_new`
about 5.6% at -O0 and 0.7% in release; milestone 206 adds a fixed 43 ticks. Milestone 629 built the
remedy calef ruled for: an exact count made as cheap as it can be.
