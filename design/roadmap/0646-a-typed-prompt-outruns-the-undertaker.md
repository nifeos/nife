---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: a-typed-prompt-outruns-the-undertaker
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 646. A typed prompt outruns the undertaker, and the job pool fills with holes

*Title and slug are drafts.*
Raised by milestone 590 (the booted system starts its network stack)'s lane. It ran
`script/swish-check --arch riscv64` repeatedly and found it failing on a line its change does not
touch.

Measurement first; the remedy is not chosen here.

## The finding

On this machine (patagonia, 2026-09-24 UTC), the riscv64 leg of `script/swish-check` failed 3 of
4 runs on the base commit `5b8789439`, with no lane change applied: twice with
`rm -v rmtree/rm-solo` answering "could not spawn (the progenitor is out of memory)", once with a
line that never echoed. Milestone 590's branch failed 2 of 4 the same way, once also on its own last
line (`unreachable_network_witness`, a 40-page job). aarch64 passed every run it was given. CI
on `main` shows no such failure in its recent history.

The likely mechanism is reasoning and not measurement. `crates/system_initializer`'s BUGS
already says a job region reclaimed when it is not at the top of the pool's watermark returns
nothing, permanently. A job whose corpse `job_undertaker` has not yet collected when the shell spawns
the next one is exactly that case. `swish-check` types the next line the moment the prompt comes
back. On a four-hart riscv64 guest the undertaker can lose that race. Each loss is a permanent
40-page hole in a 240-page pool, and a directory grant needs 96 contiguous pages, so it fails first.

## What would settle it

Count the holes: have the progenitor say, per spawn, how many pages the pool has left and how many
jobs are unreaped. If the count falls by 40 on the runs that fail and not on the runs that pass,
the mechanism is confirmed. The remedy is a separate choice: the shell waiting for the reap, or the
pool recovering a hole once the region above it goes.

## Index row

riscv64 `swish-check` fails 3 of 4 local runs on `main` with "could not spawn" on a directory grant.
Proposed: measure the pool's holes per spawn before choosing a fix.
