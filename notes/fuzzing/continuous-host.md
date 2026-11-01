# The continuous fuzzing host: what runs, what it keeps, what it publishes

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md). A dedicated x86_64 machine with KVM runs the two
in-guest drivers all the time. Its specification is kept off-tree by the maintainers. This page
covers what the repository supplies to it.

## What runs

`helpers/continuous-fuzz.sh` runs four guests at once, two of each driver: the kernel-thread driver
(milestone 752, a seeded syscall driver with a shadow model) and the confined user-mode driver
(milestone 779, fuzz the surface a confined process can reach). Four is the size the false-red
measurements in [`rates-on-x86-64-under-kvm.md`](rates-on-x86-64-under-kvm.md) support. Raise
`NIFE_FUZZ_GUESTS` only after a measurement at the new size comes back clean.

Each guest slot owns a git worktree and a `target/` directory, because `cargo xtask test` writes its
disk image under the workspace root and QEMU refuses one image to two writers. A window is one
`cargo xtask test --arch x86_64 --test <driver>` with the seed range set at build time. The driver
stops itself after 60 seconds of guest time, so each window costs a rebuild, a boot and one minute of
seeds. The window length is a limitation, recorded under BUGS below.

`helpers/systemd/nife-fuzz.service` starts the supervisor at boot and restarts it on failure. The
supervisor exits non-zero after five windows in a row fail without a finding, which is how a broken
build reaches systemd instead of looping silently.

## What it keeps

All of it lives under one state directory, `windows.tsv` first: one row per window with the driver,
the slot, the first seed, the seeds run, the time, the status and the commit. A seed replays the
same calls every time, so a clean range is its own corpus.

A window whose status is not `ok` is held. That covers a `replay:` line (a shadow-model mismatch or
an escape), a panic, and a window that ran out its bound. Its log moves to a private directory
and stays there. The next window for that slot starts one seed past the failing one.

## What it publishes

`helpers/fuzz-daily-publish.sh` runs from a daily timer and writes a report of counter totals and a
table of clean seed ranges. It reports held findings by count and age only. It never opens a held
log, and it leaves the held window's own range out of the table, so nothing published reproduces a
held finding. A finding stays private until its fix merges, and starts as a private security
advisory. Findings sorted as public at triage are the exception the architect's ruling names.

Where the output goes is not decided. The proposal
[`where-the-daily-fuzz-output-goes`](../../design/roadmap/proposals/where-the-daily-fuzz-output-goes.md)
sets out the two options. Until an architect rules, the script writes local files and publishes
nothing.

## Install, for a person with root

The lane has no root. A person copies the three files from `helpers/systemd/` to `/etc/systemd/system/`,
then runs `daemon-reload`, `enable --now nife-fuzz.service` and `enable --now nife-fuzz-publish.timer`.

## BUGS

- The seed range is fixed at build time, so each 60-second window pays a rebuild. A runtime seed
  (for example a kernel command line word) would let one image sweep forever. That change belongs
  to the drivers' milestones, not to this host.
- Coverage counters do not exist yet, so the report has no coverage figure. The heartbeat reads
  seeds run.
- A window that hangs is held whole, with no way to tell a hung guest from a slow rebuild except
  the bound. The bound is 30 minutes.
- Only x86_64 runs. aarch64 and riscv64 run under emulation and wait for a larger machine.
- The daily publish and the private advisory step are written but untested end to end, because the
  credential is installed by a person.
