---
status: PARTIAL
raised: 2026-10-28
milestone_dependencies: none
decision_dependencies: none
machine_requirements: a many-core host, for the measurement this block still owes
specific_machine: none
needs_person: no
---
# 957. The proofs run in parallel on one machine, N shards at a time

Ruled 2026-10-28 (UTC).

`script/verify` shards the Kani proofs (`--shard k/n`, balanced by `helpers/verify_times.py`), and
CI's verify matrix runs the shards on four runners. One machine could not do the same. Four
`--shard` runs typed by hand are four heavy scripts, each waiting for its own heavy-job slot and
racing the others through one target directory, and nothing joins their verdicts. So a lane on a
many-core box still waited out a serial suite of about 40 minutes before it could push.

This block records the measurement, the mode the tree now carries (`script/verify --parallel [N]`,
off by default), and the two alternatives refused.

## What four shards at once cost

Measured 2026-10-28 (UTC) on an 80-vCPU Linux host with about 128 GiB: four shards started by hand,
`VERIFY_JOBS=4` in each, all green.

- Wall clock 967 s (16.1 min) for the whole suite, against about 39.7 min serially (the crate
  table's own serial total), a factor of 2.5.
- Host utilization 9.4%, peak 1-minute load 12.0 on 80 vCPUs.
- The floor crate, `glob`, took 10 to 12 min.

Three caveats decide how far that number travels. The 2.5 belongs to that host and that table. The
ceiling is the floor crate rather than the core count, so a fifth shard proves a smaller slice of
the same table and finishes no sooner. And 9.4% utilization at a load of 12 says the four shards
mostly wait on the solver, one single-threaded BMC formula at a time: the win is the shard balance
CI already computes, not spare parallel capacity.

## The mode

1. A flag, off by default: `script/verify --parallel [N]`. Without a number it takes the smaller
   of CI's shard count and the machine's heavy-job slots, then lowers that to what memory carries.
   Each reason it lowers prints, so no run drops a shard in silence.
2. One heavy slot, not N. The parent sources `helpers/heavy-slot.sh` like every heavy script and
   holds the slot. Each child re-sources it, `helpers/heavy_slot.py --held` finds the parent in the
   child's ancestor chain, and the child runs in place. A one-slot machine carries a four-child run;
   `helpers/verify-parallel-selftest.sh` proves that under a deadline, because a deadlock never
   returns.
3. One install, ordered. The parent runs `script/kani-install` before it forks, so one process
   orders the pinned toolchain. Each child then calls it too and finds the pin, which costs one
   version read, because the script reads the version Kani reports.
4. Children are shard runs: `script/verify --shard k/N` in the same worktree, so they share one
   target directory and one packer. Each writes `target/verify-parallel/<stamp>/shard-k-of-N.log`,
   and the parent writes `summary.txt` beside them.
5. The suite is the union or nothing. Before forking, the parent runs the packer N times and
   refuses unless the N shards partition the crate table. A child that exits non-zero fails the
   parent, which names that child and prints no suite line. The packer's own partition assertion
   stays in force in each child.

## What bounds the count

Each child runs `VERIFY_JOBS` harnesses at once, 4 by default, and the tree records 3.5 GB for the
largest harness. `notes/verification/glob-proofs.md` has one `glob` harness reaching 3.5 GB with the
kill at twelve minutes, and DECISIONS §203 (capacity is rented rather than bought) budgets "about
3.5 GB a harness". That is a peak rather than a typical, so the guard errs high. It reads the figure
as GiB, larger than the recorded GB. On 128 GiB, four children at four jobs budget 56 GiB and fit.
On a 16 GiB Mac, two children budget 28 GiB and do not, so the default falls to one and says so,
and one child is the serial run this flag exists to replace.

## What holds it

- `helpers/verify-parallel-selftest.sh` in `script/lint`: a stub `script/kani-install`, a stub
  `cargo` and a scratch tree, so the children are the real orchestration over stubbed proving. It
  proves the partition, the install order, the failed child, the one slot and the default count, in
  milliseconds, with no Kani and no cargo.
- `helpers/verify_parallel.py --selftest` proves the sizing against fixture slot and memory counts,
  so the rule holds on any machine.
- CI does not move: `verify.yml` still runs `--shard k/4` on four runners, the flag stays off unless
  somebody types it, and the required check keeps the name `verify (Kani proofs)`.

## The alternatives refused

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

`helpers/heavy_slot.py` arbitrates the slots, sets the count from physical memory and
`NIFE_HEAVY_SLOTS`, and walks ancestors for `--held`. `helpers/verify_times.py` packs the shards and
asserts the partition; this work adds one subcommand, `shards`, so one place parses CI's count.
`script/kani-install` and `verify.yml` stay as they are. The one new piece is the sizing arithmetic,
`helpers/verify_parallel.py`, with its selftest.

## Follow-on

- **Outstanding.** The mode's own end-to-end measurement on a many-core host: `script/verify
  --parallel` over the whole table, its number here, beside the 967 s four hand-started shards took.
  The 967 s above measured the shards rather than this flag, so the flag's own figure is owed.

## BUGS

- The 3.5 GiB figure comes from one harness's peak in the pre-fix `glob` loop, so a crate whose
  harnesses all run small pays the largest budget. The guard is conservative rather than precise.
- The children still call `script/kani-install`. At the pin that costs one version read each. A
  tree that reinstalls would pay N installs, which the parent's ordering makes slow rather than
  racy.
- The children share one target directory, so cargo's build lock serializes their compiles. The
  967 s includes that cost, and the solver dominates it.

## Names

`--parallel`, its `N`, `NIFE_VERIFY_MEMORY_BYTES` and `helpers/verify_parallel.py`. Name:
provisional, minted by this lane on 2026-10-28 (UTC). An architect names things.

The lane's branch is `milestone/956-local-parallel-prove`. A milestone number stays provisional
until merge, and the branch cannot follow a renumber without closing the pull request (#1901), so
the number may move again while the branch keeps its name.

## Index row

Four shards at once prove the whole suite in 967 s against about 39.7 min serially, so
`script/verify --parallel` forks the shards a machine can carry, inside one heavy slot, and speaks
only when every child passes.
