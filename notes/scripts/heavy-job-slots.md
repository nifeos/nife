# Heavy jobs wait for a slot

An appendix to [`../scripts.md`](../scripts.md). Name: provisional, minted 2026-10-19 (UTC) by
lane/heavy-job-slots; calef names files.

A script that builds the workspace or boots QEMU suites (`lint`, `test`, `mutation`, `ci-build` and
the others that source `helpers/heavy-slot.sh`) first takes one of the machine's heavy-job slots.
Every worktree on the machine shares them, so however many lanes run, at most N heavy jobs do. N is
one per 8 GiB of physical memory, at least 1. Two settings override it: `NIFE_HEAVY_SLOTS=<n>` for
one process tree, and a per-machine file, `~/.nife/heavy-slot-count` (one integer, beside the slot
directory and outside every worktree, so every lane reads it with no setup). The environment wins
over the file, and the file is read on every poll, so raising it reaches waiting jobs within seconds.
A job that has to wait says once which jobs hold the slots, and hints when one looks stuck (under 1
percent CPU for over an hour), then waits. `--status` marks a slot whose worktree is gone as orphaned. A nested job (`ci-build`'s tests) runs in
its parent's slot. `python3 helpers/heavy_slot.py --status` lists the holders.
`helpers/heavy_slot.py` has the mechanism and why a killed job cannot leave a slot stuck.

Two take no slot. `script/setup` runs before it installs the Python the slot needs.
`script/lint --no-cargo`, the pre-push hook's mode, runs while git holds a connection to GitHub
open and idle, and a wait there loses the push (`script/lint`'s preamble has the numbers).

## Kani memory is a pool of its own

A heavy slot is one equal share, and a Kani run is several shares at once: `script/verify` runs
`VERIFY_JOBS` harnesses together, and one proof reached 20 GiB resident in a host out-of-memory dump. So Kani has a
second pool, `kani/` under the slot directory, in units of 2 GiB. The pool is physical memory less a
12 GiB reserve for builds, QEMU and the system. A harness weighs 10 GiB by default
(`NIFE_KANI_HARNESS_GIB` changes it for a group of small proofs), and a run takes the weight of every
harness it runs at once, all together or none. A run that does not fit waits and says what holds the
rest. When fewer harnesses fit than `VERIFY_JOBS` asks for, the run lowers `VERIFY_JOBS` and says so.

Every path that starts `cargo kani` takes the pool: `script/verify` (and `--parallel`, whose shard
children run inside their parent's units), `script/verify-riscv64` through it, `script/falsifications`,
`script/crate-standalone` and `helpers/kani_reach.py run`. A hand run of `cargo kani` outside these
scripts takes nothing; run `script/verify --only <crate>` instead. CI takes no pool, because a runner
runs one job on one machine (`GITHUB_ACTIONS`, or `NIFE_KANI_POOL=off`).

Before starting a proof, `python3 helpers/heavy_slot.py --status` shows the heavy slots and the Kani
units, and for every holder its lane, worktree and command line. `helpers/kani-pool-selftest.sh`
proves that the third run waits, that a run weighs its harnesses and that CI runs as before.
