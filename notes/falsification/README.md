# Falsification records: the appendices to notes/falsification.md

[`notes/falsification.md`](../falsification.md) carries the convention, the tooling, the limits and
a summary of every sweep. These files carry the evidence its prose budget has no room for.

- [`inductive-validity-cores.md`](inductive-validity-cores.md): what was examined in Kani and CBMC
  on 2026-08-31, and why neither offers an IVC.
- [`record-limit.md`](record-limit.md): the record that never reached a verdict, the timing sample
  behind the per-record limit, an example run and its limitations.
- [`milestone-212-scope.md`](milestone-212-scope.md): how the reporter's scope moved from one
  directory to `cargo metadata`, and what the correction cost.
- [`six-real-records.md`](six-real-records.md): the first six records, the central theorem's blind
  spot and the patch path `paging` broke.
- [`self-referential-sweep.md`](self-referential-sweep.md): milestone 211 (a harness that states its
  property through the function under test) and its eleven blind harnesses, the narrowing that found
  a third of them, and the patterns that make a harness fine.
- [`duplicated-implementation-sweep.md`](duplicated-implementation-sweep.md): milestone 213 (a
  harness that re-implements the code instead of calling it), its discriminator, its one finding and
  the two that have the shape and are not findings.
- [`machine-discovery-sweep.md`](machine-discovery-sweep.md): the 2026-09-23 sweep of
  `machine_discovery`, and the harness that did not exist.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.
