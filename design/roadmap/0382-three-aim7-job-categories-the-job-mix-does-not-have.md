---
status: BUILT
raised: 2026-09-04
built: 2026-09-19
---
# 382. Three of AIM7's job categories are missing from the multi-tasking workload

Built 2026-09-19, by milestone 168's own lane, in a different session and on the same
day this block was numbered. Two of the three categories landed: the job mix gained page mapping
and process creation, which are the two the argument below actually rested on. The third, the
disk-file category, is deliberately still absent and has its own proposal; see `## Follow-on`.

**The work and the number were minted by two sessions that could not see each other.** This branch
promoted the proposal to a numbered block while that lane was absorbing its content into milestone
168 and deleting the file. calef's ruling from milestone 433 decides which survives: a proposal is
promoted and then closed, because **a numbered block marked BUILT is a record and a deleted file is
nothing**. So the number stays, the status moves, and milestone 168 cites the block rather than a
path that no longer exists. Filed 2026-09-04 as an unnumbered proposal by milestone 168's lane, from
`crates/job_mix`'s own `BUGS`; numbered 2026-09-19 by milestone 433's drain. *(Number provisional
until the merge queue lands it.)*

It carried `Gate: NONE` for the design, with the *result* it improves still behind milestone 168's `HARDWARE`. The line is gone because a finished block's gate can only be stale. The
jobs themselves are ordinary userspace work and develop under QEMU.

In brief. `crates/job_mix` keeps AIM7's four methodological properties and stands in for its
categories with five jobs: a compute grind, a working-set walk, a null system call, a yield burst and an
IPC round trip. **Three AIM7 categories have no representative**, each refused for a stated reason
rather than overlooked:

| category | why it is absent |
|---|---|
| disk-file operations | needs a disk attached, which would make the instrument's availability depend on the runner's storage; the same reason `script/bench`'s `fs_*` rows are not in the gated set |
| process creation | costs an address space per iteration, and `spawn_el0` exists to reclaim exactly one at a time; at 32 concurrent tasks it would measure the memory-region allocator rather than the scheduler |
| user page mapping | needs a per-task address-space capability the spawn path does not currently hand out |

## Why it matters

DECISIONS §96's question is how much of this kernel's time goes into process-kernel overhead under
multi-tasking load, and the cost lives in kernel stacks left behind by threads that block inside
the kernel. Two of the three missing categories are exactly that: a filesystem call and a spawn
both block deep in a kernel path, where a null system call and a yield do not. So the missing categories
are not a fidelity nicety; they are plausibly where the effect is largest, and a flat result from the
present mix is weaker evidence than a flat result from a mix that had them.

## What it would take

The map job is the cheapest and should go first: it needs the spawn path to hand a task a capability
on its own address space, or a shared target space the way `kernel/src/bench.rs`'s `map_el0` already
builds one. The spawn job wants the reclaim `spawn_el0` already does, done per task rather than per
supervisor. The file job wants the sweep to run with a disk attached and should probably be a second
mix rather than an addition to the first, so a run without a disk is still a comparable run.

## What is blocked until it is answered

Nothing. Milestone 168's instrument works and its result is interpretable, with this limitation named
in `crates/job_mix`'s `BUGS` and in `notes/job-mix.md` where a reader meets the number. What is at
stake is how much weight a flat curve can carry.

## Follow-on

- **Milestone 168.** Where the work landed: *"The second hole"* and *"What changed on 2026-09-19"*
  in that block's appendix
  [`0168-multitasking-benchmark/holes-closed-2026-09-19.md`](0168-multitasking-benchmark/holes-closed-2026-09-19.md), with the refusals this one recorded answered there. The map job did not need a new
  capability, and the spawn job's allocator share is now printed.
- **Milestone 493.** The disk-file category, which is the one of the three that did not land and is
  not simply deferred. A 2026-09-13 correction to `crates/job_mix`'s `BUGS` established that
  Warton's own AIM7 run had the filesystem jobs disabled (section 5.4, the ramdisk was too
  small), so the gap is not a gap against the number this crate exists to chase. Its own proposal is
  milestone 493 (a disk-file job mix needs a disk), `design/roadmap/0493-a-disk-file-job-mix-needs-a-disk-radon-can-drive.md`, which names the
  harder half: a disk radon can actually drive.
- **Recorded.** *No seven-job sweep has run on silicon.* Every claim about the new instrument is a
  resampling of the old one's data, and the map and spawn jobs may change `tasks=4`'s distribution.
  Recorded in milestone 168's `BUGS`, where a reader meets the instrument.

## Index row

`crates/job_mix` keeps AIM7's four methodological properties and stands in for its categories with
five jobs: a compute grind, a working-set walk, a null system call, a yield burst and an IPC round trip.
Three AIM7 categories have no representative, each refused for a stated reason rather than
overlooked. A disk-file job would make the instrument's availability depend on the runner's storage.
A process-creation job would measure the memory-region allocator rather than the scheduler at 32
concurrent tasks. A page-mapping job needs a per-task address-space capability the spawn path
does not hand out. Two of the three are exactly where DECISIONS §96 (process kernel or event
kernel, and how to decide it)'s effect should be largest. A filesystem call and a spawn both block
deep in a kernel path where a null system call and a yield do not. So a flat curve from the present mix
is weaker evidence than a flat curve from a mix that had them. The map job is the cheapest and goes first; the spawn job wants `spawn_el0`'s reclaim
done per task; the file job wants a second mix rather than an addition, so a run without a disk stays
comparable.
