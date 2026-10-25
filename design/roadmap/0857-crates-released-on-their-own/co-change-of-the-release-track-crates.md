# Co-change of the release-track crates

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md).
A crate that leaves turns every commit touching it and something else into two pull requests, so
the share of its commits that cross its directory is what leaving costs. No crate moves before this measurement
exists, by §235's Fork 3 amendment, which sizes a home by what changes together.

*Name: provisional. The lane that built the per-package co-change column chose it on 2026-10-25 (UTC).*

## The reading

Measured by `helpers/interface_stability.py` over 2026W29 to 2026W43, one Cargo package per row,
from `notes/project-metrics/interface-stability-packages.csv`. A commit crosses when it also
touches code outside the crate's directory; documentation does not count on either side. The
method and its blind spots are in [notes/interface-stability.md](../../../notes/interface-stability.md),
"Co-change per package". The weekly appendix of the metrics page carries the live figures.

| crate (milestone) | commits | crossing | crossing, mass changes excluded | most often crossed into |
|---|---:|---:|---:|---|
| `e1000e` (815) | 19 | 10 (53%) | 5 of 14 (36%) | `kernel` (2) |
| `designware_mobile_storage` (817) | 28 | 15 (54%) | 11 of 24 (46%) | `kernel` (4), `jh7110_clock_and_reset` (3) |
| `extensible_host_controller_interface` (820) | 17 | 10 (59%) | 6 of 13 (46%) | other release-track crates (2 each) |
| `designware_ethernet` (816) | 13 | 8 (62%) | 5 of 10 (50%) | `extensible_host_controller_interface` (2) |
| `file_allocation_table` (813) | 22 | 14 (64%) | 9 of 17 (53%) | `kernel` (5), `uefi_loader` (3), `components` (3) |
| `jh7110_entropy` (818) | 37 | 30 (81%) | 13 of 20 (65%) | `kernel` (4), `capability_plan` (3) |
| `jh7110_clock_and_reset` (819) | 32 | 25 (78%) | 15 of 22 (68%) | `kernel` (8), `designware_mobile_storage` (3) |
| `non_volatile_memory_express` (814) | 44 | 39 (89%) | 24 of 29 (83%) | `kernel` (14), `components` (12) |

The last column counts only the commits outside a mass change.

## What it suggests for the order

Co-change alone would move them in the table's order: `e1000e` first, then
`designware_mobile_storage`, `extensible_host_controller_interface` and `designware_ethernet`,
then `file_allocation_table`, with the JH7110 pair and `non_volatile_memory_express` last. FAT
stays first by ruling, and at 53% it costs about what the middle of the list costs, so the ruling
is not expensive.

The JH7110 pair name each other and `designware_mobile_storage`, the board's three crates moving
together. `non_volatile_memory_express` crosses on most commits because its driver program lives in
`components` and the kernel links it; its cut should separate the program from the crate before it
leaves, or every change to it stays two pull requests.

Every figure rests on at most 44 commits. A lane that edits several of these crates at once makes
them co-change by schedule rather than by coupling, which is most of what
`extensible_host_controller_interface` and `designware_ethernet` cross into.
