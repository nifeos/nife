# What this lane measured, and what it inferred

An appendix to [the proposal](../heavy-jobs-wait-in-order-for-measured-memory.md). Everything here
ran on 2026-11-05 (UTC) on the 80-core x86_64 Linux host, while other lanes ran. This lane started
no proof. Its probes read `/proc` and cgroups, ran a few small user scopes, and built a throwaway
eight-crate workspace in a scratch directory.

## The host

| fact | value | source |
|---|---|---|
| cores | 80 | `nproc` |
| physical memory | 53.9 GiB | `sysconf`, the pair `heavy_slot.py` reads |
| swap | none | `free`, `swapon --show` |
| Kani pool | 20 units of 2 GiB | (53.9 less 12) over 2 |
| default slot count | 6 | 53.9 over 8; the environment set 15 |
| kernel, systemd | 7.0, 259 | `uname -r`, `systemctl --version` |
| controllers delegated to the user manager | `cpu memory pids` | `cgroup.controllers` and `cgroup.subtree_control` of `user@1000.service` |
| `systemd-oomd` | inactive | `systemctl is-active` |
| `vm.overcommit_memory` | 0 | `/proc/sys/vm` |

## The slots and the pool at 04:52 UTC

`python3 helpers/heavy_slot.py --status`, then a script that summed each holder's process tree
from one `ps` listing.

- 15 slots, 12 held: nine `lint` (eight full runs and one `--quick`, which a crate move replaced
  within a minute), one `test`, one `crate-standalone` and one `verify`.
- Each of the eight lint trees: about 20 MB resident, no CPU. `pstree` showed each one inside a lint
  section, in `heavy_slot.py --pool kani --units 20 verify -- script/verify --only capability
  --harness lint_probe_harness`. Their wait ages from `ps`: 4,069, 4,064, 3,901, 2,184, 2,099,
  1,954, 1,847 and 200 seconds. All eight started before #3247 merged at 04:51:55 UTC.
- Kani pool: 10 of 20 units held. `crate-standalone` on `file_allocation_table` held 5, with a tree
  of 1.13 GiB; `script/verify --only generational_table --harness ...` held 5, with a tree of
  0.81 GiB. #3214's table gives those rows 5,140 MiB and 410 MiB.
- `/proc/loadavg` 12.92, 10.96, 8.93. `MemAvailable` 47.2 GiB. Memory pressure (`some avg10`) 0.00,
  CPU pressure 0.08%.
- A three-second `/proc/stat` sample a minute later read 29% busy, with load then at 30.

## Out-of-memory kills

`journalctl -k` since 2026-11-01: nine `Out of memory: Killed process` lines, all `cbmc`, all
`oom-kill:constraint=CONSTRAINT_NONE` (global, not a cgroup limit), between 2026-11-04 20:32 and
2026-11-05 01:02 UTC. Resident at the kill: 18.5, 20.5, 20.8, 21.1, 25.9, 31.7, 50.5, 51.2 and
51.5 GiB. The user manager's `memory.peak` reads 52.4 GiB. The one cgroup-constrained kill in the
same window was this lane's own probe.

The journal also shows one of #3214's measurement scopes for `inter_process_communication`: 34 s of
CPU over 30 min 23 s of wall time, peak 330 MiB. The scope wrapped the whole `script/verify`,
waiting included, so about 29 minutes of it was a wait. The journal does not say for which resource.

## A user scope as a measurement and a cap

- `systemd-run --user --scope -p MemoryMax=64M -p MemorySwapMax=0` started with no password. Inside,
  `memory.max` read 67108864 and the scope offered `memory.peak`, `memory.events`,
  `memory.oom.group`, `memory.current`, `cgroup.kill` and `memory.pressure`.
- In the same kind of scope, a Python allocation of 200 MiB died with `SIGKILL` (exit 137), and
  the kill touched nothing outside the scope. The kernel log counts it as the one cgroup kill.

## Lint's own peak

This lane ran its full `script/lint` once inside a user scope with no memory cap and read the scope
at the end. LINT_MEASUREMENT

## The jobserver probe

A workspace of eight empty crates, each with a build script that sleeps 1.5 s and logs its start
and end; a short script counts the most build scripts alive at once. Cargo 1.101.0-nightly
(2026-10-27), GNU Make 4.4.1.

| how cargo ran | most at once |
|---|---:|
| alone | 8 |
| `make -j2` | 2 |
| `make -j2 --jobserver-style=fifo` | 2 |
| `make -j3 --jobserver-style=pipe` | 3 |
| `make -j2` around `cargo build -j8` | 2 |
| `CARGO_BUILD_JOBS=3` | 3 |
| bare fifo with 2 tokens in `MAKEFLAGS`, no make | 3 |
| two cargo runs sharing that fifo | 4 |

Token return: after a clean build on the two-token fifo, both tokens were back. After `SIGKILL` to a
cargo 0.9 s into the build and a 2 s wait, none were.

Kani 0.68.0: `kani-compiler` carries the string `jobserver` four times, as rustc does;
`kani-driver`, `cbmc` and `kissat` carry none. `cargo kani --help` describes `-j` as a thread pool.
This lane infers from the binary and the help text, without a run, that `kani-driver` ignores a
jobserver.

## Inferred, not measured

- The macOS side. This lane had no macOS host, so it recalls rather than tests the absence of
  cgroups, `ps` sampling, `getrusage`, the memory-pressure `sysctl` and fifo support in Homebrew's
  make.
- The peaks of `test`, `mutation`, `build` and the other classes. None has a measurement, and the
  worked example weighs them at the 8 GiB default.
- The line count for option A is an estimate from today's 400 lines of `heavy_slot.py`.
