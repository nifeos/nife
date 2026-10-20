# Heavy jobs wait for a slot

An appendix to [`../scripts.md`](../scripts.md). Name: provisional, minted 2026-10-19 (UTC) by
lane/heavy-job-slots; calef names files.

A script that builds the workspace or boots QEMU suites (`lint`, `test`, `mutation`, `ci-build` and
the others that source `helpers/heavy-slot.sh`) first takes one of the machine's heavy-job slots.
Every worktree on the machine shares them, so however many lanes run, at most N heavy jobs do. N is
one per 8 GiB of physical memory, at least 1; `NIFE_HEAVY_SLOTS=<n>` overrides it. A job that has
to wait says once which jobs hold the slots, then waits. A nested job (`ci-build`'s tests) runs in
its parent's slot. `python3 helpers/heavy_slot.py --status` lists the holders.
`helpers/heavy_slot.py` has the mechanism and why a killed job cannot leave a slot stuck.

Two take no slot. `script/setup` runs before the Python the slot needs is installed.
`script/lint --no-cargo`, the pre-push hook's mode, runs while git holds a connection to GitHub
open and idle, and a wait there loses the push (`script/lint`'s preamble has the numbers).
