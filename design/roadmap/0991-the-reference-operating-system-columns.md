---
status: NOT-STARTED
raised: 2026-10-30
promoted_from: the-reference-operating-system-columns
milestone_dependencies: none
decision_dependencies: none
machine_requirements: riscv64 silicon with a wired network port; x86_64 silicon with NVMe
specific_machine: xenon (the Micron 2450 the 2026-10-04 Fedora 44 fio baseline was taken on)
needs_person: yes
---
# 991. The reference operating-system columns for the cross-OS suite

calef ruled the plan in a top-level session on 2026-10-30 (UTC), while lane
bench/cross-os-reference-columns recorded it. Milestones 826 through 834 plan nife's half of each
cross-OS comparison; the reference side (which operating systems, what every row records, which
machine collects which rows) is the missing piece.
[notes/cross-os-reference-columns.md](../../../notes/cross-os-reference-columns.md) holds it; this
milestone is the evenings that fill it.

Reuse: every benchmark runs unmodified and each operating system installs from its own published
image; this milestone adds measurements and prose only, so nothing needs a fork or a patch.

**In brief.** Install the reference operating systems on the bench machines and take the reference
rows with the pins the note records, controls first. `coremark` and STREAM go
first on every fresh install; then the lmbench subset, hackbench, speedtest1 and ioping; fio with
SPDK on xenon's Debian side; iperf3 and netperf on radon's wire.

## What it adds beyond the existing rows

The Linux comparison sections of milestones 826 through 834 plan Linux columns inside each
milestone. This milestone adds what those rows assume but no record holds: the BSD columns, Debian 13
as the standard column on both architectures, and the rig-validation rule those rows need. FreeBSD 15.1
goes on both machines; OpenBSD 8.0 is exploratory on radon. A control disagreement indicts the rig,
not the operating system. It also adds the unix side's pass protocol, so the suite measures risk 6's
kind of variance (a 16% read spread) deliberately. The xenon evenings also settle Fedora 44's
provenance: one Debian run of the same job file beside the photographed 2026-10-04 baseline tells
whether the distribution moves the number.

## Which fatal risks it informs

[Risk 6](../../fatal-risks/6-a-confined-driver-is-too-slow.md): the fio and SPDK columns on xenon are
its reference set. [Risk 4](../../fatal-risks/4-the-per-crossing-cost.md): the lmbench subset builds
its crossing-cost table across operating systems. Every cross-OS claim gains credibility a stranger
can check: the unix rows run on stock installs anyone can reproduce.

## What each machine's evening collects

xenon, no wire: Debian 13 amd64 plus one or both BSDs, taking every non-network row of the suite;
the Debian side runs fio and SPDK's `spdk_nvme_perf`. radon, the wire (cable in `gmac0`):
Debian 13 riscv64 plus one BSD card, the same CPU and memory rows plus iperf3 and netperf against a
peer. radon reboots only by a manual power cycle, so one Linux plus one BSD bounds an evening.

## Why it needs a person, and what it costs

xenon's POST walk needs a keyboard, radon's OS switches are manual power cycles, and cards move
between machines. The installs are disposable; this bench reimages the machines per experiment, so
the work is reversible by construction.

## Alternatives considered

More operating systems: declined; each adds a bench evening and a maintenance surface, and NetBSD
has no recorded VisionFive 2 success. Folding the collection into milestones 826 through 834:
declined; they wait on the C library, threads and packaging, while the unix columns are collectible
now and block on nothing nife builds.

## Done when

notes/cross-os-reference-columns.md holds, for each machine, a Debian 13 column and at least one
BSD column for every row the note assigns to that machine, each with its controls (`coremark` and
STREAM) taken on the same install, and one Debian fio run on xenon beside the 2026-10-04 Fedora 44
baseline.

## Index row

The cross-OS suite gains its reference side: stock Debian and BSD columns on radon and xenon, taken
the same way every time, so each nife number has a reproducible row to stand beside.
