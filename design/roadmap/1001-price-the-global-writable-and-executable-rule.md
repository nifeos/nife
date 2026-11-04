---
status: NOT-STARTED
promoted_from: price-the-global-writable-and-executable-rule
raised: 2026-10-31
milestone_dependencies: 969
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1001. Price the global writable-and-executable rule before anyone rules on it

The lane for milestone 969 (no frame is writable and executable in one address space) raised
this on 2026-10-31 (UTC), when an architect ruled that claim 11 covers one address space for now.
The ruling left a global rule open: no frame writable in any space while it is executable in
another. The architect decides it once it has a price. This milestone produces that price and
builds no rule.

Reuse: milestone 969's per-space check in `MappingHold::record_mapping`, the mapping log in
`kernel/src/revoke.rs`, and the icount bench (`cargo xtask bench`, baselines in `bench/`). No new
dependency.

## The two candidates it prices

- Global at map time (option (b) of the rulings proposal): `record_mapping` scans every live
  space's log, not only the mapping space's, before a writable or executable leaf goes in.
- Global at thread start (option (c)): the scan runs once when a space's first thread starts, and the
  start fails while a writable alias of one of its code frames survives elsewhere.

[`the-rulings-the-alias-refusal-waits-on`](1005-the-rulings-the-alias-refusal-waits-on.md) has both,
with prior art and what each costs the shipped programs.

## What to measure

1. The real size of the scan. Boot the default image and count live mapping records across every
   space at the shell prompt, and the number of live spaces. Every map's global scan walks that
   many entries, inside the one lock every map on every core takes.
2. Each candidate, built on a branch behind a feature, against the 969 baseline: `map_el0` and
   `spawn_reap` on the icount bench, which isolates the scan from timer noise. `spawn_reap` is a
   published number, so its change is the one an outsider would quote.
3. A worst case. A system test that fills the registry toward `revoke::MAXIMUM_SPACES` with spaces
   of a few hundred mappings each, and times one writable map. This bounds the hold time another
   core can wait, which the per-map bench rows cannot show.
4. The program cost, by building it: `process_builder::fill_and_map` reordered to give up its
   writable view before `MAP_CODE`, the two fixtures that keep a writable code page
   (`os_primitives_benchmarker`, `job_mix_task`) restructured, and the suite run to find every
   other builder that breaks.

All four on aarch64, riscv64 and x86_64. The bench rows are per ISA already, and the scan's cost
depends on each ISA's page-walk and lock costs.

## What would favor each outcome

- Stay per space: either global candidate moves `map_el0` or `spawn_reap` past the bench's own
  per-row bound on any ISA, or the worst case has no practical bound. Narrowing rights can then
  close the route listed in 969's `BUGS` instead.
- Option (b): both rows stay within the bench's per-row bound on all three ISAs at the measured
  record count, and the worst case stays under one scheduler tick.
- Option (c): map-time scanning fails those bounds but a once-per-start scan keeps `spawn_reap`
  within them. It also adds a refusal to the thread-start method, which an architect rules on as
  a syscall-surface change.

## The bounds

Provisional decision, made by the maintainer on 2026-11-04 (UTC), before any measurement comes
back, so the result cannot pick its own threshold. On the aarch64 icount bench, against the 969
baseline:

- `spawn_reap` at most +1% over 225,337.
- `map_el0` at most +5% over 299,956.
- The worst case (the registry filled toward `revoke::MAXIMUM_SPACES` with spaces of a few hundred
  mappings each, one writable map) takes less than one scheduler tick.

The same bounds apply on riscv64 and x86_64, each against its own baseline.

The nightly restamp bound (0.5% per row, 2% cumulative) is for compiler drift. It is not this
feature budget, and neither replaces the other.

The bounds change only by a new dated decision made after this one, never after seeing results.

## Index row

The per-space writable-and-executable refusal leaves open a rule that spans every address space. This milestone measures what the global rule costs at map time and at thread start, so an architect rules on a price and not a guess.
