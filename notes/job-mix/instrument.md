# The job mix instrument, constant by constant

`crates/job_mix` defines the job mix once, for the kernel-side supervisor and the EL0 task.
[job-mix.md](../job-mix.md) has what the instrument is for and how a bench evening runs it. This
page holds the reasons behind the crate's constants, which its doc comments link.

## The jobs

The crate keeps AIM7's heterogeneous jobs, random order per task, task-count sweep and throughput.
It keeps AIM7's categories rather than its 53 jobs, most of which name Unix services this system
lacks. Disk-file operations are the missing category: radon cannot read a disk, and one shared file
channel would be raced by 32 tasks.

| job | AIM7 category | what it costs here |
|---|---|---|
| `COMPUTE` | compute-bound arithmetic loops | no syscall at all; the control arm |
| `TOUCH` | user virtual-memory operations | a working set walked in user mode, no syscall |
| `NULL_SYSCALL` | (the trap itself) | EL0 to EL1 and back, the cheapest kernel entry |
| `YIELD` | (scheduling pressure) | a full context switch through the ready queue |
| `ROUND_TRIP` | pipe I/O | `CALL` to a shared server and its `REPLY`: two rendezvous |
| `MAP` | user virtual-memory operations (mapping) | split a region, build a space, 32 `MAP_INTO`s, `DESTROY` |
| `SPAWN` | process creation | build two children from EL0, `RECEIVE` each one's exit, reclaim |

`ROUND_TRIP` answers §96 (process kernel or event kernel, and how to decide it), since a process
kernel's cost is the stack a blocking thread leaves behind. `COMPUTE` and `TOUCH` displace the
cache between kernel entries, which a ping-pong micro-benchmark leaves out. `MAP` and `SPAWN` build
from each task's own budget, with no new syscall, method or object type.

The workfile, `MIX`, gives `ROUND_TRIP` a quarter of its sixteen entries because IPC is the
primitive this kernel exists to be fast at. The other six kinds split the rest evenly, two each.
The weighting is stated rather than derived.

## The console markers are a contract

The markers are text that leaves the machine and is read back by something that is not the
machine: `crates/serial_console` on a bench or in CI, and `cargo xtask job-mix` under QEMU.
`crates/boot_ladder` holds the boot tour's markers for the same reason, and its header carries the
argument. Three copies of a contract agreeing because a reader checked them is milestone 268's
finding 3, and the fix that milestone found is to have one copy.

What is shared is the head; the fields inside the line are not. That gap is what
`crates/serial_console`'s `SweepPoint` carries a `BUGS` entry for. When the point line's tail
gained four field names and lost two, every marker still matched and the recognizer silently read
nothing. They are stable heads, on `boot_ladder`'s rule: the head is what a matcher keys on and
never changes, and the tail carries the numbers and is free to improve. A contract on the whole line
would make every improvement to the diagnosis a breaking change.

## Why the median of 21

The best of N is `kernel/src/bench.rs`'s rule, and it is right there. On a busy dev Mac the minimum
is the least host-contended sample, and everything above it is somebody else's load. Here the
contention is the subject. On radon the tasks' own contention for the echo servers spreads a
subrun's time over a wide distribution (37% within one boot at `tasks=4`). A minimum drawn from a
wide distribution is the statistic that moves most with the sample count, while a median converges.

The count comes from the five radon boots of 2026-09-16. Resampling the fifteen `tasks=4` repeats
those boots produced, five simulated boots of N repeats each give these boot-to-boot spreads of the
reported figure:

| statistic | spread |
|---|---|
| best of 3 | about 22% (29.4% observed) |
| median of 3 | 17% |
| median of 15 | 5.1% |
| median of 21 | 4.2% |

`tasks=16` and `tasks=32` already sat at 2.7 to 6.7% with three. 21 brings the worst point into
the band the stable points were in, and it is odd so the median is a sample rather than an average
of two. The method is in milestone 168's appendix,
`design/roadmap/0168-multitasking-benchmark/holes-closed-2026-09-19.md`.

One count serves every point; a table varying it by point was refused. The whole sweep's timed
windows at 21 repeats come to about eleven seconds on radon, against a boot that takes minutes. A
table would buy seconds and cost a reader a second thing to hold. The cost that does grow is the
TCG rehearsal's, minutes paid once per architecture by a gate.

## The task budget's two bounds

A task's budget holds what it keeps plus the larger of the two transient regions, so no job is
refused a split the budget was sized to allow. Two compile-time bounds hold it: at least the kept
pages plus the larger region, and less than both regions together, because a task runs one job at a
time and each gives its region back. A single bound that the budget exceeds each region on its own
lets an expression that picks the smaller region compile, leaving `MAP` nine pages short.

## Two seeds, one order

`order(2k)` and `order(2k + 1)` are the same permutation, for every `k`. The `seed | 1` keeps the
LCG off its degenerate state, and it does that by throwing away bit zero, which halves the seed
space. Nothing in the tree hits it: the kernel hands every task a distinct, odd, well-spread seed
(`kernel/src/job_mix.rs`), and an odd seed passes through `| 1` unchanged. A caller that seeded
tasks by consecutive index would put every adjacent pair in lockstep, which is the one property the
function exists to prevent.

Fixing it would change every permutation the function has ever produced, and a benchmark's
published numbers are a fact that has left the machine. So the seed contract is written down and
the arithmetic is left alone.
