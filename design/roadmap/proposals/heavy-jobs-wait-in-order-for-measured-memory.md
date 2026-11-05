---
status: PROPOSED
raised: 2026-11-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Heavy jobs wait in order for the memory they measured

The lane `lane/heavy-job-admission` wrote this on 2026-11-05 (UTC). The architect asked for a design
that manages heavy local jobs better than a fixed count of slots. The milestones have no numbers; an
integrator mints them at promotion. Every file, command and variable name here is provisional.

This is an architect's call. It changes a tool every lane on a machine shares, and its ledger is a
format that checkouts of different ages read and write at once, which makes it the one part that
behaves like a wire format. The rest is local code and easy to undo.

## Recommendation

Replace the slot count and the Kani pool with one ledger of reservations. A heavy job declares its
whole need before it starts (memory from a measured per-class weight, and cores), takes all of it in
one step or waits, and waits in ticket order. On Linux each admitted job runs in its own memory
cgroup, which measures its peak to correct the weight and caps it so a wrong weight kills that job
and never the host.

Four properties follow. No process waits while an ancestor holds a grant, so hold-and-wait has no
code path. No two processes need to agree on a count. A big request cannot starve. And the kernel's
`flock(2)` still frees a dead job's reservation, with no daemon and no cleanup step.

## Background

Today `helpers/heavy_slot.py` gives the machine N slots, one per 8 GiB of memory, as `flock` files
under `~/.nife/heavy-slots`. A heavy script (`lint`, `test`, `mutation`, `ci-build` and the others
that source `helpers/heavy-slot.sh`) holds one slot for its whole run. A nested heavy script runs
in its parent's slot. `NIFE_HEAVY_SLOTS` overrides N, read per process, so two processes with
different values each count only the slots they know. Lane `lane/heavy-slots-per-machine` (#3244)
adds a machine-local count file and an orphan mark.

Beside the slots sits the Kani pool: physical memory less a 12 GiB reserve, in 2 GiB units. A
harness weighs 5 units (10 GiB) by default, and a run takes every harness it runs at once, all or
none. Lane `lane/kani-memory-inventory` (#3214) weighs a harness by its row's measured peak instead.
Its pushed table has 34 rows, 28 of them at one unit, and a fingerprint that falls back to the
default when the crate changes.

So the tool has two currencies (a count and a memory pool), two acquisitions in sequence (the slot,
then the units), and a weight that is a guess for everything except Kani. The two acquisitions in
sequence are the hold-and-wait. The guess is why the cap measures jobs and not resources.

## What went wrong, measured

The [measurements appendix](heavy-jobs-wait-in-order-for-measured-memory/measurements.md) has each
command and its output. In short, from the 80-core, 53.9 GiB Linux host with no swap:

- At 04:52 UTC on 2026-11-05, holders had taken 12 of 15 slots. Eight of the 12 were full lint
  runs, each parked inside a section that ran `script/verify` through the real Kani pool and asked for 20 of
  its 20 units. They had waited from 200 to 4,069 seconds, at 0% CPU and 20 MB each. #3247, now on
  main, stops that section from taking the pool, but those lanes still ran the older script.
- The 10 Kani units in use (20 GiB reserved) belonged to two proofs whose process trees held 1.13
  and 0.81 GiB. Their rows' measured peaks are 5,140 MiB and 410 MiB.
- The host at that moment: load 12.9, 47 GiB available, CPU 29% busy in a three-second sample a
  minute later. The cap was full; the machine was not.
- A request for all 20 units waits until the pool is empty, so any 5-unit run that fits sooner
  overtakes it. Smaller runs starved those eight lints, not only blocked them.
- The kernel's own out-of-memory killer ended nine `cbmc` processes between 2026-11-04 20:32 and
  2026-11-05 01:02 UTC, at 18.5 to 51.5 GiB resident, every one a global kill with no cgroup
  constraint. The user manager's cgroup reached a peak of 52.4 GiB. The pool admitted those runs
  and nothing stopped them.

The maintainer also reported a pool charge of 40 GiB for a run that used 12 to 13 GiB, and a cap
raised by hand from 6 to 10 to 15. This lane did not observe those two; they are the maintainer's.

## The design

### (a) Memory reserved by measured weight

Every heavy job belongs to a class: `lint`, `lint-quick`, `test`, `mutation`, `build`, `verify` and
so on. A class weight is the peak memory of one run of that class, rounded up to whole GiB. A Kani
run keeps #3214's per-row weights and asks for its harness count times its largest row's weight,
plus its compile.

Linux measures the peak from the job's own cgroup. Each admitted job runs as
`systemd-run --user --scope`, and when it ends the wrapper reads `memory.peak` and
`memory.events`. This host allows that without root: the user manager delegates the `cpu`,
`memory` and `pids` controllers, and a probe scope showed every `memory.*` file. `memory.peak`
counts page cache too, so it errs high, which is the safe direction for a reservation.

The weight corrects itself. A machine-local file under `~/.nife/heavy/` keeps the last ten peaks of
each class. The weight is the largest of them plus 10%, so a bigger peak raises it at once and a run
of smaller ones lowers it slowly. A committed table holds defaults so a new machine starts somewhere
sensible. Kani rows stay in #3214's committed table, because there the code changes the peak, and a
fingerprint ties a peak to its code.

macOS has no cgroups. There the wrapper samples the process tree's resident memory once a second
with `ps`, which misses short spikes, and also reads `getrusage(RUSAGE_CHILDREN)`'s maximum, which
covers one process and not the tree. A peak measured on macOS is a lower bound. A macOS host
therefore uses the larger of its own peaks and the committed default.

### (b) CPU

Each request also names a core count, and the grant exports it as `CARGO_BUILD_JOBS` and
`VERIFY_JOBS`. Capacity is 1.5 times the core count, because CPU overcommit only slows jobs down
while memory overcommit kills them.

This lane tested a shared jobserver and recommends against one for now. Cargo honors a GNU make
jobserver in both the pipe and the fifo style, even with `-j8` given, and it honors a bare fifo
named in `MAKEFLAGS` with no make process at all. But every client keeps one implicit token, so two
cargo runs on a two-token fifo ran four build scripts at once: the bound is tokens plus clients. A
cargo killed with `SIGKILL` mid-build returned none of its two tokens, so a leaked token stays
leaked until something refills the fifo. Kani's harness parallelism (`-j`) is a thread pool in
`kani-driver`, whose binary has no jobserver strings, so it would not take part (inferred, not
run). A load gate lags by a minute and lets a burst of waiters through together.

### (c) No hold-and-wait

A job's request is its whole need, decided before it starts. A script states its class at the top;
`script/verify` computes its Kani request from the rows it will prove and `VERIFY_JOBS`, which it
does today for the pool. When `script/lint` reaches a section that would start a heavier class, that
section runs inside lint's grant: it lowers its parallelism to fit, or it fails and says what it
needed. The waiting function asserts that no ancestor holds a grant, so a regression fails loudly
instead of hanging.

A long job with phases of different size (lint's 88 Python sections, then its cargo half) may
release its grant and ask again between phases. It never holds one grant while asking for another.

### (d) Order and starvation

A waiter takes a ticket from a counter under a short lock, and holds a `flock` on its queue file
while it waits. Every second, each waiter takes the ledger lock and computes the same answer: the
oldest waiter that fits may start; a younger waiter may start if it fits and the oldest has waited
under ten minutes. After ten minutes nothing younger starts until the oldest has. The ledger clamps a
request above capacity to capacity, as today, so every request can start eventually.

This is the aging form of backfill. Slurm backfills by estimated end time, and nife has no
estimates. The wait log in (h) could supply them later.

### (e) A safety net

On Linux the scope carries `MemoryMax` at twice the job's weight, `MemorySwapMax=0`, and
`memory.oom.group=1`, so an overrun kills that whole job and nothing else. A probe scope at 64 MiB
killed a 200 MiB allocation and only that process. The wrapper reads `memory.events`, reports the
kill with the class and the cap, and records a peak of twice the old weight. The next run of that
class therefore reserves more. Every heavy scope lives under one slice whose `MemoryMax` is the
machine's capacity, so the sum of heavy jobs cannot reach the memory the desktop, the agents and the
shells need.

`systemd-oomd` is inactive on this host, and it needs root to enable. The slice cap gives the same
protection for heavy jobs without it.

macOS has no equivalent. The wrapper there kills a job's process group when the sampled tree passes
twice its weight, best effort at one-second resolution. The capacity default on macOS is half of
physical memory, against Linux's physical memory less 12 GiB. Admission there also refuses to start
anything while `kern.memorystatus_vm_pressure_level` reads warning or worse (recalled from Apple's
documentation, not tested here). A 16 GiB Mac hung twice on 2026-10-19 with several lanes running,
and it remains the host with no hard backstop.

### (f) Per-machine configuration

Capacity is one machine-local file, `~/.nife/heavy/capacity`, with a memory figure and a core
figure. A waiter reads it under the ledger lock at every decision, so a change reaches every waiter
within a second. The ledger records reservations in bytes, so two processes never disagree about
what a number means. An environment variable may shrink one job's own request; it never changes the
machine's capacity. #3244's count file is the interim form and becomes this file.

### (g) Stale and leaked holders

A dead holder frees its reservation through `flock`, as now. Two leaks remain.

- A holder whose worktree is gone, like the orphan lint that kept a slot after its worktree went.
  The ledger records the worktree. `--reap` ends such a holder by writing to the scope's
  `cgroup.kill` on Linux, which kills the whole tree at once, or by signalling the process group on
  macOS. The recommendation is to reap these automatically at each admission, because a missing
  worktree is unambiguous.
- A holder that runs but does nothing, as the eight lints did. #3244's idle hint names it. With (c)
  the main cause has no code path, so this lane recommends reporting it and leaving the kill to a
  person.

### (h) Observability

`--status` prints one table per class: weight and its source (measured, default or stale), running
count, waiting count, memory reserved, and memory in use from each scope's `memory.current`. The gap
between reserved and in use is the over-reservation this proposal exists to shrink. Below it, the
queue in ticket order with each waiter's age and request. Each job appends one line to
`~/.nife/heavy/log.tsv`: ticket, class, lane, request, seconds waited, seconds run, peak and how it
ended. `--report` totals waits by class and day, so a slow queue becomes a number.

### (i) Migration, and what stays simple

Lanes cut from an older main keep running the old script for days, so the two must coexist. While
the old slots exist, each new holder also takes one old slot file, so old-code lanes still count it.
Old holders appear in the new ledger at today's 8 GiB per slot. The Kani pool goes last, once no
open lane predates the release that folds it in.

These stay as they are: the `flock` liveness rule, a nested job running in its parent's grant, CI
taking nothing (`GITHUB_ACTIONS`), `--no-cargo` taking nothing, and `--status` as the one front
door.

## Options

A, recommended: the ledger above, with memory and cores in one request, tickets, cgroup scopes on
Linux and the weaker wrapper on macOS. It removes the hold-and-wait, the starvation, the
job-not-resource cap and the per-process disagreement on N.

B: keep the count and fix its symptoms. Merge #3247 and #3244, order the slot waiters, and take the
Kani units before the slot in `script/verify`. This costs perhaps a tenth of A. It still caps jobs
and not memory, so the 80-core host idles while its count is full and a 51 GiB proof still reaches
the global killer. It is the right answer for a host that never runs more than two lanes.

C: a load or pressure gate only. Admit while the one-minute load is under the core count and memory
pressure is low. It needs no weights, but load lags, a burst of waiters all see the same quiet
moment, and pressure rises only after the memory is gone.

D: a machine-wide jobserver for CPU on top of A or B. It bounds cargo well and Kani's harness pool
not at all, it leaks tokens on `SIGKILL`, and it bounds clients plus tokens. It answers a CPU
problem this host does not have.

Would this lane still choose A if A and B cost the same? Yes. The case for A rests on the
measurements above, not on effort; B is the cheaper option.

## Worked example: the 04:52 UTC state under the ledger

Capacity: 53.9 GiB less 12 GiB, so 41.9 GiB, and 120 cores. Lint's measured peak from this lane's
own full run is LINT_PEAK. The other classes have no measurement yet and weigh 8 GiB, today's slot.

| holder | today | under the ledger |
|---|---|---|
| eight lint runs | 8 slots, all idle, waiting for 20 Kani units | LINT_ROW |
| `file_allocation_table` proof | 1 slot and 10 GiB of pool | 6 GiB (5,140 MiB rounded up) |
| one `generational_table` harness | 1 slot and 10 GiB of pool | 2 GiB (410 MiB) |
| `script/test`, a crate move | 2 slots | 16 GiB, at the default |

WORKED_EXAMPLE_TOTAL

The example shows the cost too. Until a class has a measurement, its default reserves as much as a
slot does today, so on the first day the ledger admits no more than a slot count of the same size.
The gain arrives with the peaks.

## The cost, and when a slot count is better

The machinery is perhaps 800 lines of Python and a selftest of the same order, against today's 400.
It adds a scope per job on Linux (a `systemd-run` call costs milliseconds), a machine-local state
directory, and a ledger format that must stay readable across checkouts of different ages. Weights
can be wrong in both directions, and the safety net turns a weight that is too low into a job killed
at its cap. That failure is loud and local, but a lane still loses a run.

A slot count stays the better answer on a host with one or two lanes. It also wins where one job
uses most of memory, since a 16 GiB Mac fits two jobs either way. And without cgroups, the macOS
wrapper adds little over a count. Option B is cheap insurance for those hosts.

## Milestones

Each fits one pull request, in order.

1. The ledger. Holders and waiters as `flock` files, tickets, aging, memory in bytes, every class
   at 8 GiB, and the old slots honored both ways. Behavior otherwise matches today's.
2. Requests declared up front. Each heavy script states its class; a waiter with a granted ancestor
   is an error; `script/verify` sizes its request from #3214's rows.
3. The Kani pool folded into the ledger, and `--pool` retired once no open lane predates milestone
   2.
4. Observability: the per-class `--status`, `log.tsv` and `--report`.
5. Linux scopes: a scope per job, `memory.peak` read on exit, the machine-local peaks file, and
   self-correcting weights.
6. The safety net: `MemoryMax`, `memory.oom.group`, the parent slice at capacity, and the overrun
   report. On macOS the sampling wrapper and its kill.
7. Cores in the request, exported as `CARGO_BUILD_JOBS` and `VERIFY_JOBS`.
8. Capacity file and reaping: `~/.nife/heavy/capacity` replaces #3244's count file, and `--reap`
   ends orphans, automatically or by hand as ruled.
9. Retire `NIFE_HEAVY_SLOTS` and the slot files.

Milestones 1 and 2 alone remove the hold-and-wait and the starvation. Milestone 5 is where the
throughput gain arrives.

## Questions for the architect

1. The direction: A, the ledger, or B, the count with its symptoms fixed.
2. A weight that is too low kills its own job at twice the weight. Is that acceptable in place of
   today's risk of the global killer taking a random large process?
3. Should the ledger end a holder whose worktree no longer exists, or only report it?
4. The macOS capacity default: half of physical memory, or something else for a 16 GiB machine?

What each answer unblocks: question 1 decides whether milestones 1 to 9 exist at all; 2 decides
milestone 6; 3 decides milestone 8; 4 decides one constant in milestone 6.

The [prior art appendix](heavy-jobs-wait-in-order-for-measured-memory/prior-art.md) compares this
with the GNU make jobserver, Bazel, Nix and Hydra, Slurm, `systemd-oomd`, ninja pools and GitHub
Actions concurrency groups, and says where nife differs.
