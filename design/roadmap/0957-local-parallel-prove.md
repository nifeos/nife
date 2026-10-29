---
status: IN-PROGRESS
raised: 2026-10-28
branch: milestone/956-local-parallel-prove
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 957. Local parallel prove of the Kani verification suite

## Rationale and Measurement

`script/verify` runs the Kani proof suite serially, which takes about 39.7 minutes on the
development host. Measured on 2026-10-28 (UTC) on an 80-vCPU, 128 GiB host: four shards started
concurrently took 967 s (16.1 min) for the whole suite, all green. That is a factor of 2.5 against
the serial run.

Three caveats decide how far that number travels. The 2.5 belongs to that host and that table. The
ceiling is the floor crate (`glob`) rather than the core count, so a fifth shard proves a smaller
slice of the same table and finishes no sooner. And 9.4% utilization at a peak load of 12.0 on 80
vCPUs says the four shards mostly wait on the solver, one single-threaded BMC formula at a time. The
win is the shard balance CI already computes, not spare parallel capacity.

## Ratified Design

1. A flag, off by default: `script/verify --parallel [N]`. Without a number it takes the smaller
   of CI's shard count and the machine's heavy-job slots, then lowers that to what memory carries.
   Each reason it lowers prints, so no run drops a shard in silence.
2. One heavy slot, not N. The parent sources `helpers/heavy-slot.sh` like every heavy script and
   holds the slot. Each child re-sources it, `helpers/heavy_slot.py --held` finds the parent in the
   child's ancestor chain, and the child runs in place. A one-slot machine carries a four-child run.
3. One install, ordered. The parent runs `script/kani-install` before it forks, so one process
   orders the pinned toolchain. Each child then calls it too and finds the pin, which costs one
   version read, because the script reads the version Kani reports.
4. Children are shard runs. `script/verify --shard k/N` in the same worktree, so they share one
   target directory and one packer. Each writes its own log, and the parent writes a summary beside
   them.
5. The suite is the union or nothing. Before forking, the parent runs the packer N times and
   refuses unless the N shards partition the crate table. A child that exits non-zero fails the
   parent, which names that child and prints no suite line. The packer's own partition assertion
   stays in force in each child.

## Refused Alternatives

**A documented recipe of `--shard k/n` runs.** Slot semantics diverge by machine size. On a one-slot
box the runs deadlock, because each waits for the slot and none releases until it finishes; on a
two-slot box the third run queues behind the first pair. Nothing guards memory, so four runs on a
small machine invite the OOM kill CI already paid for. The install workaround dies with the version
check in `script/kani-install`, and a recipe carrying a fix for a bug the tree no longer has rots
into a trap. And nothing joins the verdicts, so a person reads N logs to learn whether the suite
proved.

**CI only.** It leaves a 40-minute serial wait on the machine whose purpose is parallel work. A lane
gates locally before it pushes, and CI tells it afterwards. More runners cost money and concurrency,
while the 80-vCPU box on the desk idles at 9.4%.

## Reuse

`helpers/heavy_slot.py` arbitrates the slots and walks ancestors for `--held`.
`helpers/verify_times.py` packs the shards and asserts the partition. `script/kani-install` and
`verify.yml` stay as they are. The one new piece is the sizing arithmetic and the fork-and-join
orchestration, which no existing script carries.

## Index row

Proves the whole Kani suite on one machine in a quarter of the serial wall time by forking the
shards a lane's machine can carry, inside the one heavy slot its parent holds, and speaks only when
every child passes.
