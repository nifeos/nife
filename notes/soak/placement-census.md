# Where the threads are: the placement census

An appendix to [notes/soak.md](../soak.md). It is milestone 240 (the soak reports what happened and
not where, so an eightfold difference cannot be explained): what the census prints, what it first
measured, and what four QEMU runs of it do and do not support.

## Why a rate moves without the machine changing

Two soak runs on radon differed eightfold in round-trip rate: 183,662/s against 22,592/s. They used
the same card and the same build, twenty minutes apart. The boot tour's own pure-compute check proved
the machine identical. It ran 6.9M and 7.3M iterations in the first, against 6.8M and 7.3M in the
second, with 82 preemptions both times. So the difference was in the workload and not in the
silicon. The soak printed six counters and not one thread's location. That left placement as an
inference rather than a reading.

The kernel knew the answer the whole time and threw it away. `sched::spawn` calls
`pick_spawn_target`, places the thread, and returns only a thread id.

## What it prints

Three things, all under a `soak-test-census:` prefix of their own. That prefix is not `soak-test:`
on purpose. `crates/board_console`'s recognizer matches two substrings on that one
(`soak-test: started` and `soak-test: t=`), and a census is neither. Giving it its own word means a
block of census lines never has to be proven harmless against a recognizer it has nothing to do
with.

First, one block at soak start, from the placement `pick_spawn_target` actually made, one line per
online core:

```
soak-test-census: where the kernel placed each worker at spawn: R=responder, C=caller, G=grinder, W=tick waiter, and the number after each letter is its group
soak-test-census: core=0 threads=6 C0 C2 C2 G2 C3 W3
soak-test-census: core=1 threads=6 R0 R1 C1 C1 C2 G3
soak-test-census: core=2 threads=7 C0 C0 C1 G1 W2 R3 C3
soak-test-census: core=3 threads=5 G0 W0 W1 R2 C3
```

A token is a role letter and a group number. So `G0 G3` on one line is that core drawing two
grinders, read off a log by someone who never saw the board. A core with no workers gets a line too.
Four cores online with one of them empty is an explanation, and a census that printed only the
occupied cores would hide it.

Second, one field on every beat, `drifted=`. It counts how many responders, callers and grinders
are no longer on the core the last printed census put them on. While it reads zero, that block
describes the machine right now, and the reader is told so rather than assuming it.

Third, a fresh block whenever `drifted` is nonzero, and one more before the thread dump on a
failure. Printing a census every beat would double a dense log. Printing one only at the start would
leave a stale block standing, which is exactly what happens (below). Printing on the change carries a
current census whenever one exists, and is quiet otherwise.

## The first thing it measured: the start census goes stale in five seconds

Nine to eleven of the twenty non-waiter threads are off their spawn core by the first beat, on every
QEMU run of it, with `steals=` at three to five. So it is not work stealing. The parent said what it
is: *a rendezvous wake is local on purpose, so a communicating set converges onto one core within a
few exchanges and stays there.* DECISIONS 138 says it in the same words. The census measures what
the tree has written down.

That settles the question milestone 240's block left open: should the census also be reported after
the start? It must be. Not because threads might move, but because they provably do, immediately,
every time. A start-only census would have misattributed every run that night. The spawn placement
is a lottery *result*, not a resting place.

## Four QEMU runs, and what the census does and does not support

These are aarch64, `script/soak --for 40s` (the old name, as above), on the same host and the same
build. The third differs only in which reference `drifted` compares against, which cannot affect
scheduling. The arrangement is the settled one from the first re-census. The rate is the mean of
beats 2 through 7, after convergence.

| settled arrangement | rate |
|---|---|
| three IPC groups on core 1, one core holding only waiters | 21,700/s |
| two IPC groups on core 0, alongside two grinders | 38,600/s |
| two IPC groups on core 2 | 33,500/s |
| one IPC group per core | 32,300/s |

The arrangement varies run to run under QEMU exactly as radon's rate did. That is the first thing
worth knowing: the lottery is real on emulation too, and it is now visible.

The widest spread coincides with the most crowded arrangement. It is 1.8x between the run that put
three of the four IPC groups on one core and the best of the others. That points the same direction
as radon's eightfold, and does not prove it.

And the census partly refuses the inference the block was minted with. That block named the
starvation shape as *a core drawing two grinders*, and the run that did exactly that was the fastest
of the four. What tracks the rate in this small sample is the number of IPC groups sharing a core,
not the number of grinders. Four runs on an emulator settle neither, and saying so is the point. This
is an instrument, and the result it makes possible is a series of boots on silicon rather than an
argument.

## What it cost

Nothing that ships. `sched::spawn_reporting_placement` and `sched::last_cpus` are both
`#[cfg(feature = "soak")]`, so a production build has neither. `script/fastpath-footprint` reads the
same 6,687 bytes over eight symbols it read before. Within a soak build the census is one lock
acquisition and twenty-four comparisons every five seconds. The workload does tens of thousands of
round trips a second in the same window.
