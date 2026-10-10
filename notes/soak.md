# The workload that does not stop, and what a clean run of it is worth

*(Milestones 219 and 221. `kernel/src/soak.rs`, `fixtures/src/soaker.rs`, `crates/soak_page`,
`script/soak-test`, and the `Stage::Soak` half of `crates/board_console`.)*

`design/fatal-risks/README.md`'s fifth entry, *it cannot be made reliable on multicore, and the bugs appear
only on silicon*, names its decisive experiment as sustained multi-core stress on the boards with
the load-sensitive assertions live. Until this milestone the tree could not sustain anything: the
boot tour ran its checks, printed its last line, and called `arch::halt()`. Captured on radon on
2026-09-01, that is the last thing the board says before it sits in `wfi` indefinitely.

This note is what the workload is, what number it produces, and, more usefully, what it was
measured to be unable to do.

## The shape

One kernel feature (`--features soak_test`) replaces the halt at the end of the boot tour with a pool of
user-mode workers and a supervisor that watches them forever.

- The workload is a user program (`fixtures/src/soaker.rs`), so the pressure goes through the real
  syscall boundary. Groups of one responder, three callers, one pure-compute grinder and one tick
  waiter, one group per online core.
- The detection is in the kernel (`kernel/src/soak.rs`), because a user program cannot assert
  about kernel internals and a workload that could reach its own tripwire is not a tripwire.
- The two share one page (`crates/soak_page`), three `u64` per worker with exactly one writer
  each, so the supervisor reads progress without asking for it.
- The tick waiter is milestone 221's and has its own section below. It is the one worker that
  completes no IPC: it blocks on a rendezvous the kernel signals from `sched::on_tick`, which is
  what makes anything on this machine cross cores at all.

A round trip is `CALL` -> `RECEIVE_CAP` -> `REPLY` -> the caller waking: two block/wake handshakes, the
protocol `crates/thread_wake_handshake` models and the one the risk's only real defect was in
(`sched::wake_load_aware` making a receiver `Ready` without a delivery).

Each worker spins a small pseudo-random number of iterations between round trips. That is not
decoration: a soak that repeats one interleaving for eight hours has explored one interleaving, and
the jitter keeps the pairs' phase drifting instead of locking.

## The number, and the only three things it is for

Every five seconds the supervisor prints one line:

```
soak-test: t=25s beat=5 rounds=1151772 rate=43031/s wakes=10032 wakerate=401/s workers=24 refused=0 mismatch=0 stalled=0 crossings=2252 remote=3584 steals=3 deferred=99
```

`rounds` is the figure: cumulative IPC round trips completed by every worker. It exists so that
a run can be compared, and it has three honest uses:

1. Between architectures, so a rate an order of magnitude off on one of them is a question.
2. Between QEMU and silicon, which is the comparison risk 5 is actually about.
3. Against the same machine later, where a large drop is an IPC-path regression no functional
   test would fail on.

`wakes` is not part of `rounds` and never will be: a tick-route wake is not a round trip, and
folding the two together would make the one comparable figure mean something different depending on
which build produced it. Its own rate is pinned to the machine (`TICK_HZ` times the online cores, so
about 400 a second on a four-core QEMU), which makes it a useful liveness check in its own right: a
`wakerate` well under that is the timer or the wake path falling behind, not the workload.

`refused`, `mismatch` and `stalled` must all be zero, and any of them nonzero fails the run: the
supervisor prints `soak-test: FAILED`, dumps the threads (so the per-core event rings are on the log) and
panics.

### First measurements, 2026-09-01, patagonia, QEMU

Taken with `script/soak --for 30s`, four groups per machine except x86, whose runner defaulted to
one core on the day (it defaults to two since 2026-09-23; see this page's `BUGS`). (That command is `script/soak-test` since 2026-09-14, milestone 297 (`soak` becomes `soak-test`). The name is left as it
was typed here and everywhere else on this page that says how a number was taken, because how a
measurement was made is an account of a day.) These are QEMU numbers on a loaded laptop and are a baseline for comparison, not a
benchmark; `script/bench` is the instrument for cost.

| Architecture | Cores | Workers | Round trips/s | Cross-core handoffs in 25s |
|---|---|---|---|---|
| aarch64 | 4 | 20 | ~58,000 | 17 |
| riscv64 | 4 | 20 | ~24,000 | 21 |
| x86_64 | 1 | 10 | ~3,900 | 0 |

### With the tick route, 2026-09-02, patagonia, QEMU (milestone 221)

Same command, same host, one day later, and these rows are not comparable with the rows above
for a reason larger than the date: the build changed. Each pair below was measured back to back, the
"before" leg from the exact commit this work branched from, on an otherwise idle machine, and read
at the 25-second beat (20 seconds on x86, whose beat count is lower).

| Architecture | Cores | Round trips before | after | Crossings before | after |
|---|---|---|---|---|---|
| aarch64 | 4 | 1,623,764 and 1,630,605 | 1,632,746 and 1,632,803 | 15, frozen from beat 1 | 1,452 and 3,779, both rising linearly |
| riscv64 | 4 | 871,047 and 886,428 | 662,787 and 823,783 | 10 and 14, frozen | 2,573 and 4,358, rising |
| x86_64 | 1 | 77,372 | 51,749 | 0 | 0, and one core is the whole reason |

Two runs of each leg on the multicore architectures, because one would have been misleading, and
the first pass of these measurements *was* misleading: it was taken while another lane's test suite
was running on the same laptop, and the numbers it produced (aarch64 47,864 against 43,031 a second)
were the host's load rather than this change. Everything above is from an idle machine.

What the numbers support:

- aarch64 pays nothing measurable. 0.6% more round trips after than before, in the direction of
  faster, which is noise. The tick waiters complete no round trips and the set of workers that does
  is identical in both legs, so the totals are directly comparable.
- riscv64 pays about 7% on the closest-matched pair (886,428 against 823,783) and more on the
  looser one. It is the architecture where a migration costs the most under TCG, and it is the one
  crossing most often, so a cost showing up here and not on aarch64 is consistent rather than
  puzzling.
- x86_64 pays about a third, and that is arithmetic rather than a finding. Its runner was
  single-core on the day, so the two extra waiter threads are two more shares of the one core in a
  round-robin scheduler, and `crossings=0` is what one core means.

The round-trip rate fell far less than DECISIONS 138's spike saw, which reported about 30% on
aarch64 and about 55% on riscv64. That difference is recorded rather than explained away: the spike
was thrown away and cannot be re-measured, so why it was slower is not recoverable, and the worker
mix is the obvious candidate and is a guess.

`wakes` and `crossings` are the two figures milestone 221 added, and neither is a throughput
number. The tick route wakes at `TICK_HZ` times the core count, which is a property of the machine
rather than of the workload, and the crossings are however many of those wakes `wake_load_aware`
chose to place on another core. Somewhere between a seventh and a half of them, varying by run more
than by architecture. That ratio is a fact about the placement policy under this load and nothing in
this tree yet says what it should be.

Which build these came from, and it is not the one that ships. Every figure above is from a
`--features soak_test` kernel, which is the only build in which the counters and `Thread::last_cpu`
exist at all. That is not free, and the size of it is measured rather than assumed:

| Architecture | `ipc_fastpath`, production | with `--features soak_test` | |
|---|---|---|---|
| aarch64 | 5,788 bytes | 6,120 | 1.06x |
| riscv64 | 5,106 bytes | 5,344 | 1.05x |
| x86_64 | 6,639 bytes | 6,995 | 1.05x |

So a soak build is not a production build: its IPC path is five to six per cent larger, and its
round-trip rates are therefore soak-build rates. Compare a soak number with another soak number,
which is what the three comparisons above are; never with `script/bench`, and never as a statement
about how fast this kernel does IPC.

Milestone 221 added nothing to that table, and it was checked rather than assumed. Its kernel
change is a `#[cfg(feature = "soak")]` call in `sched::on_tick` and a module that is not compiled
otherwise, so a production build should be untouched; "should be" is what this tree does not accept.
Built at the base commit and at the merge candidate, on all three architectures, without the
feature: every symbol has the same size, every section has the same size except `.strtab`, which is
not loaded, and `ipc_fastpath` and `syscall_entry` are unchanged at 6,687 and 1,637 bytes.

The loadable image (`llvm-objcopy -O binary`) differs by 45 bytes on aarch64, and all of them are
`core::panic::Location` line numbers below the insertion point, each larger by exactly the ten lines
added to that file. The proof is that rebuilding the base commit with ten comment lines at the
same point gives an image that is byte-for-byte identical to the merge candidate's, on all three
architectures. Any comment added to `sched.rs` would move those bytes, and a hash comparison that
called that a change would be measuring the file's line count.

That the instrumentation is behind a feature at all is a thing this milestone got wrong first and
was caught by a gate. Shipping the counters and the `last_cpu` write unconditionally put
`ipc_fastpath` 5.7% over milestone 132 (fast)'s 5% bound on aarch64 (5,788 -> 6,120), with riscv64 and
x86_64 growing 4.7% and 4.6% behind it: one cause, three effects, and aarch64 merely the one that
tipped. The `last_cpu` write sits in `schedule()`'s switch, the hottest line of the hottest
function. `script/lint` now clippies `--features soak_test` on both ISAs, because a `cfg`-gated
instrument that nothing lints is one that rots (and the first run of that check found two real
warnings in `kernel/src/soak.rs`, which had never been linted).

## The finding: a saturated workload does not migrate under this scheduler

This is the part worth reading, and it is the reason the milestone was worth running rather than
merely worth building. It is still true, and milestone 221 did not repeal it: what that
milestone added is a thread that is *not* part of the saturated workload, precisely because nothing
inside the workload can be made to move.

The cross-core handoff count freezes within the first second and never moves again. Measured
across three topologies (one caller per responder, three callers per responder, twice as many groups
as cores), on both multicore architectures, at up to 65,000 round trips a second. The workload runs
on every core and contends on every shared scheduler structure; the threads themselves stay exactly
where `pick_spawn_target` put them.

The mechanism, and every clause of it is in the tree already:

- A rendezvous wake is local on purpose. `sched::wake` pushes the woken peer onto the *waker's*
  own run queue (DECISIONS §28.2: the message is in registers and the cache is warm). So a
  communicating set converges onto one core within a few exchanges and stays there.
- `wake_load_aware`, the load-aware placement, is for device interrupts only. It is the function
  the one real defect was in, and no user workload can reach it: it takes an IRQ to get there.
- A work steal needs an idle core and a queued thread elsewhere. A rendezvous keeps at most two
  threads runnable per group, so run queues are almost always empty and there is nothing to give;
  add compute threads to fill the queues and no core is idle to ask. Both ends of the condition are
  hard to hold at once, and a steady-state workload holds neither.
- Nothing rebalances periodically. There is no such thing in this scheduler.

### The instrument that found it was the second one

The first version counted `trace::Event::PlaceRemote` and reported 23, frozen, which was read as
"the threads are not moving". That was true, but the counter could not have shown it: a rendezvous
wake queues its peer locally, so the placement is local *even when the thread has moved between
cores*, and a placement counter is structurally blind to the migration this workload performs.

`thread::Thread::last_cpu` and `trace::Event::Migrated` answer the question where it cannot be
dodged, at `schedule()`'s `switch_in`, which is the one place every path to a CPU passes through
whatever moved the thread. The finding survived the better instrument, which is the only reason it
is written here as a finding rather than as a guess.

Take the lesson, not just the number: a counter that is *near* the question is not the same as
one that answers it, and the two agree right up until they matter.

### What this means for risk 5

The decisive experiment as the risk states it, "sustained multi-core stress", is not one
experiment. It is at least two, and this milestone delivers the first:

- Concurrent contention on shared kernel state. Four harts entering `IPC_TABLES` tens of
  thousands of times a second, preempting each other, writing their own trace rings, retiring
  rendezvous. This is real weak-memory pressure and it is what the soak sustains.
- Cross-core handoff. Threads actually moving between cores under load, which is where the
  observed defect lived. The soak does not sustain this, and cannot, for the reasons above.

Saying so is the point. A run that quietly covered one and was quoted as covering both would be
exactly the misuse `design/roadmap/0219-a-workload-that-does-not-stop.md`'s BUGS section warns about,
and `script/soak-test` prints the gap on every run so that nobody has to have read this note to know.

The second half is now runnable, which is a different claim from "has been run". See the next
section.

## Where the threads are, and why a rate moves without the machine changing (milestone 240 (soak))

Two soak runs on radon, same card, same build, twenty minutes apart, differed eightfold in
round-trip rate: 183,662/s against 22,592/s. The machine was proven identical by the boot tour's own
pure-compute check, which ran 6.9M and 7.3M iterations in the first against 6.8M and 7.3M in the
second, with 82 preemptions both times. So the difference was in the workload and not in the
silicon, and the soak printed six counters and not one thread's location, which left placement
as an inference rather than a reading.

The kernel knew the answer the whole time and threw it away: `sched::spawn` calls
`pick_spawn_target`, places the thread, and returns only a thread id.

### What it prints

Three things, all under a `soak-test-census:` prefix of their own. That prefix is not `soak-test:`
on purpose: `crates/board_console`'s recognizer matches two substrings on that one
(`soak-test: started` and `soak-test: t=`), and a census is neither, so giving it its own word means
a block of census lines never has to be proven harmless against a recognizer it has nothing to do
with.

One block at soak start, from the placement `pick_spawn_target` actually made, one line per
online core:

```
soak-test-census: where the kernel placed each worker at spawn: R=responder, C=caller, G=grinder, W=tick waiter, and the number after each letter is its group
soak-test-census: core=0 threads=6 C0 C2 C2 G2 C3 W3
soak-test-census: core=1 threads=6 R0 R1 C1 C1 C2 G3
soak-test-census: core=2 threads=7 C0 C0 C1 G1 W2 R3 C3
soak-test-census: core=3 threads=5 G0 W0 W1 R2 C3
```

A token is a role letter and a group number, so `G0 G3` on one line is that core drawing two
grinders, read off a log by someone who never saw the board. A core with no workers gets a line
too, because four cores online and one of them empty is an explanation and a census that printed
only the occupied cores would hide it.

One field on every beat, `drifted=`: how many responders, callers and grinders are no longer on
the core the last printed census put them on. While it reads zero, that block describes the machine
right now, and the reader is told so rather than assuming it.

A fresh block whenever `drifted` is nonzero, and one more before the thread dump on a failure.
Printing a census every beat would double an dense log; printing one only at the start would
leave a stale block standing, which is exactly what happens (below). Printing on the change carries
a current census whenever one exists and is quiet otherwise.

### The first thing it measured was that the start census goes stale in five seconds

Nine to eleven of the twenty non-waiter threads are off their spawn core by the first beat, on
every QEMU run of it, with `steals=` at three to five. So it is not work stealing, and this file
said what it is, one section up: *a rendezvous wake is local on purpose, so a communicating
set converges onto one core within a few exchanges and stays there.* DECISIONS 138 says it in the
same words. The census measured what the tree had written down and what the first draft of
this instrument's own comments had got backwards.

That settles the question milestone 240's block left open, which was whether the census should also
be reported after the start. It must be, and not because threads might move: because they
provably do, immediately, every time, and a start-only census would have misattributed every run
tonight. The spawn placement is a lottery *result*, not a resting place.

### Four QEMU runs, and what the census does and does not support

aarch64, `script/soak --for 40s` (the old name, as above), same host, same build (the third differs only in which reference
`drifted` compares against, which cannot affect scheduling). The arrangement is the settled one
from the first re-census; the rate is the mean of beats 2 through 7, after convergence.

| settled arrangement | rate |
|---|---|
| three IPC groups on core 1, one core holding only waiters | 21,700/s |
| two IPC groups on core 0, alongside two grinders | 38,600/s |
| two IPC groups on core 2 | 33,500/s |
| one IPC group per core | 32,300/s |

The arrangement varies run to run under QEMU exactly as radon's rate did, which is the first
thing worth knowing: the lottery is real on emulation too, and it is now visible.

The widest spread coincides with the most crowded arrangement, 1.8x between the run that put
three of the four IPC groups on one core and the best of the others. That points the same direction
as radon's eightfold and does not prove it.

And the census partly refuses the inference the block was minted with. That block named the
starvation shape as *a core drawing two grinders*, and the run that did exactly that was the
fastest of the four. What tracks the rate in this small sample is the number of IPC groups
sharing a core, not the number of grinders. Four runs on an emulator settle neither, and saying so
is the point: this is an instrument, and the result it makes possible is a series of boots on
silicon rather than an argument.

### What it cost

Nothing that ships. `sched::spawn_reporting_placement` and `sched::last_cpus` are both
`#[cfg(feature = "soak")]`, so a production build has neither, and `script/fastpath-footprint` reads
the same 6,687 bytes over eight symbols it read before. Within a soak build the census is one lock
acquisition and twenty-four comparisons every five seconds, against a workload doing tens of
thousands of round trips a second in the same window.

## The tick route: how the soak was made to cross cores (milestone 221)

`design/decisions/0138-cross-core-handoff-under-load.md` (*how a saturated workload is made to hand
threads across cores*) put four options in front of calef and he approved option D on 2026-09-02.

The mechanism, and it is short. Under `--features soak_test` and nowhere else, `sched::on_tick`
signals a rendezvous, and one worker per group blocks on that rendezvous through the `Irq::WAIT` a
device driver uses. `on_tick` is called by all three architectures' timer dispatchers in
real interrupt context on every core, so a tick runs the identical sequence a device interrupt runs:

```
soak::signal_waiters -> sched::irq_notify -> Rendezvous::signal -> handshake.serve
                     -> sched::wake_load_aware -> pick_wake_target -> place_on -> the reschedule IPI
```

Each group has its own route and each tick signals one of them, round-robin across the machine.
Both halves of that are fixes rather than flourishes, and the section below says what they fix.

That last chain is why this was worth building rather than the alternatives. `wake_load_aware` is
where risk 5's one observed defect lived, on radon, and it had exactly one caller
(`sched::irq_notify`) that no user workload could reach.

Four properties, each of which was a requirement rather than a bonus:

- No syscall is added. The userspace half already existed: `abi::irq::WAIT` is a method on an
  `Irq` capability and `user_mode_runtime::irq_wait` calls it. Only the *raise* was missing, and the kernel is
  already the thing that raises interrupts.
- Nothing exists in a production build. Proved above, not asserted.
- It is architecture-neutral, and that is load-bearing. riscv64 has no software-raisable line
  that reaches `irq_route` at all, so an aarch64 `send_sgi` or an x86 self-IPI would have left
  radon out. (Radon was believed to be the machine that produced fatal risk 5's defect; that
  reading is retracted, `notes/visionfive2.md`'s fifth bench stop, 2026-08-15, and it never
  happened. Staying architecture-neutral is still the right call on its own merits.) The timer is
  the one source all three share, through a function that is already portable.
- The timer is the one event a saturated workload cannot starve, which is the whole reason this
  works where three existing balancing moments do not.

What crosses is the waiters, not the pairs, and this must not be misquoted. Rendezvous wakes are
local by design whatever else is happening, so the callers and responders are as pinned as they ever
were. This sustains the wake protocol across cores under load; it does not make the IPC workload
migrate, and only a periodic rebalancer would, which DECISIONS 138 declines on
DECISIONS §28's own reopening trigger (*a real workload where fairness visibly fails*), which has
not fired. The kernel says this in words at the start of every run and `script/soak-test` says it again
in its summary, because the flattering reading is available and a summary gets quoted.

The soak-only interrupt numbers. Group `g`'s route is bound to intid `255 - g`, and none of
those names hardware or can be delivered on any of the three architectures: on aarch64 and riscv64 a
routed interrupt arrives only if something enabled it at the controller and nothing enables these,
and on x86_64 the top of the band is the local APIC's spurious vector (answered in its own arm
before `irq_route` is asked) with the rest at the far end of an MSI band allocated upward from 0xc0.
None of that is what makes it safe: `soak::bind_tick_routes` asks `sched::irq_route` about every
number before it takes any of them, and refuses to start a soak whose routes would steal somebody
else's interrupt. A soak boot runs the whole tour first, so every device has already claimed what it
is going to claim by the time that check runs.

### Two bugs this mechanism had, both found by running it, both about ordering

Worth writing down because neither was visible in review and both produced the same symptom, which
is a soak reporting workers as wedged when the defect was in the instrument.

One rendezvous for every waiter starves all but one, on a loaded host. The first version had a
single tick route and four waiters blocked on it. `crates/inter_process_communication`'s `Rendezvous::receive` takes a
pending signal before it looks at the receiver queue, which is right for a driver (an interrupt
that already happened must not be missed), and wrong for four peers sharing a source: when ticks
arrive in a burst, whichever waiter is already running drains the whole backlog through the pending
path and never queues, while the others sit at the head of a queue nothing pops. Three of four
stalled, and the run failed. The fix is a rendezvous per group, so a backlog can only ever belong to
the waiter it accumulated for. The shared version passed several idle-machine runs first, which is
the part worth remembering: the bug needed a busy host to appear at all.

Binding the routes after spawning the waiters is a race, and the reasoning that put it there was
right about the wrong thing. Arming last is correct for the *signaling*, because a route signaled
before anyone waits on it hands the first waiter a backlog and makes the first beat measure setup.
It is wrong for the *routing*: a waiter that reached `Irq::WAIT` before its route existed got
`WrongObject`, and a waiter has no channel to report a refusal on, so it stopped counting and the
run failed a beat later with four workers apparently wedged. aarch64 got away with it and riscv64 did
not, which is the ordinary shape of this class. The two halves are now separate: routes are bound
before the first waiter is spawned, and the signaling is switched on last.

### What it establishes about risk 5, and what it does not

- It makes the second experiment runnable. It does not run it. The run needs an evening at a
  bench on radon, argon or xenon. QEMU cannot show the defects this risk is about; that is the
  risk's premise, not a limitation of the tooling.
- It says nothing about what a crossing rate should be. The numbers above are a shape. There is
  no baseline to compare a board against until a board has produced one, and the first board run is
  what creates it.
- The hook fires on a timer, which is why it works and why it proves nothing about the machine
  without it. A soak with the tick route live is evidence about the wake path under sustained
  cross-core traffic. It is not evidence that a workload would ever generate that traffic on its
  own; measurement says it would not.
- The interrupt controller is not on this path. The timer is not a controller-routed source, so
  the claim, mask and complete sequence (the GIC, the PLIC, the local APIC) is untouched. The
  experiment is about the wake protocol, and that is what it runs.

## radon, on real silicon, 2026-09-03: the first run off a board

The first soak this project has run anywhere but QEMU, and the number that matters is not the
rate. It is the spread.

Two runs, the same card and the same build, twenty minutes apart, both booted hands-free by
milestone 218's (every boot of the VisionFive 2 needs a human typing four commands into U-Boot) boot
script:

| Run | First beat | Rate | Crossings by beat 12 |
|---|---|---|---|
| 13:04 | `rounds=918313` | 183,662/s | ~3,000 |
| 13:24 | `rounds=112960` | 22,592/s | 47 |

Eightfold, and the machine was identical. The boot tour's own pure-compute check is the control:
6,904,828 and 7,271,375 iterations in the first run against 6,831,327 and 7,288,574 in the second,
over the same fixed window, with 82 preemptions both times. Four cores online both times, same
firmware, same timer. The CPU is not throttled; the workload's throughput changed and the machine
did not.

Milestone 221 (the soak never crosses cores, so build the hook that makes it) predicted the shape
and understated it. Its `BUGS` records that the crossing count varies by more than 2x between
identical runs and names the boot-time placement lottery. This is 8x, and it is on `rounds` rather
than on `crossings`.

Why placement is the suspected cause and why that is still an inference. Twenty-four threads,
four groups of a responder, three callers, a grinder and a tick waiter, are placed across four cores
at spawn and nothing rebalances, which is milestone 219's (the boot tour ends and the kernel
halts, so there is nothing to soak) central finding and a deliberate design, per DECISIONS 138 (how
a saturated workload is made to hand threads across cores). A core that draws two grinders starves
its IPC threads, because a grinder is pure compute and never yields. The soak prints six counters
and does not print where a single thread is, which is why this stays a hypothesis and why milestone
240 (the soak reports what happened and not where, so an eightfold difference cannot be explained)
exists.

What this does to a published rate. A single run's figure is close to meaningless as a
comparable number. Had the 13:04 run been taken as *"radon does 183,000 IPC round trips per second"*
and set beside seL4's, it would have been a lucky draw reported as a measurement. Any rate quoted
from this instrument owes a distribution, and it independently reaches the conclusion the section
below reaches from the literature: more starts beat longer running.

### The three-hour run, and the first census off a board

2026-09-03, two further runs on the same card, the second carrying milestone 240's placement
census. The afternoon's eightfold spread is now four runs rather than two, and the slow draw has been
held for three hours.

| Run | Build | Duration | Rate | Crossings/s | Placement known |
|---|---|---|---|---|---|
| 13:04 | pre-census | ~20 min | 183,662/s | ~50 | no |
| 13:24 | pre-census | 2 h 59 m | 23,105/s | 0.51 | no |
| 17:06 | census | running | 188,687/s | 186 | yes |

The slow draw is stable, not a warm-up. 13:24 ran 2 h 59 m, 246,868,985 rounds, and its rate
moved from 22,592/s at the first beat to 23,105/s at the 2,137th. It never recovered and it never
degraded. `refused=0 mismatch=0 stalled=0` for the whole three hours, with `wakerate` pinned at
401/s throughout, so this is a throughput draw rather than a fault: milestone 219 (boot)'s workload was
correct for three hours at an eighth of the speed it reaches on a lucky boot.

And a correlate arrived that is sharper than the placement hypothesis. `crossings` per second
tracks the rate across all four runs, over two and a half orders of magnitude:

- the two fast runs cross 50/s and 186/s
- the slow run crossed 0.51/s, 19 by its first beat and 5,507 by its 2,137th

This inverts the naive expectation and that is why it is worth writing down. A local rendezvous
wake is the cheap one: DECISIONS 28.2 makes it local precisely because it avoids an IPI. A run whose
groups sit on one core each should therefore be *faster*, and the slow run is the one that crossed
least.

The reading that fits, stated as the inference it is. What co-location buys in wake cost it can
lose many times over in scheduling delay, because a core holding a whole IPC group also holds
whatever grinder landed there, and a grinder is pure compute that never yields. Spread groups cross
cores on every exchange and pay an IPI for it, but their threads find a runnable core. That is
milestone 240's block's hypothesis with the sign of the effect corrected: the cost is grinder
co-location rather than group crowding, and 240's own four QEMU runs already pointed this way (the
arrangement with two grinders on one core was the *fastest* of them, and the three-groups-on-one-core
arrangement was the slowest).

What the census showed on the fast run, and it differs from QEMU in a way nobody predicted.
These blocks and the reboot-loop lines quoted further down are what radon printed on the evening
they were taken, under the marker spelling of the day; milestone 297 renamed the prefix to
`soak-test-census:` on 2026-09-14, and a log of that evening will never contain the new word:

```
soak-census: core=1 threads=5 C0 W1 C2 R3 W3
soak-census: core=2 threads=5 C0 C0 C1 C2 G3
soak-census: core=3 threads=7 R0 G0 R1 C1 R2 C3 C3
soak-census: core=4 threads=7 W0 C1 G1 C2 G2 W2 C3
```

Every group is split and no core holds a whole one. The settled arrangement it converged to is
the more interesting one, and it is not what the spawn census suggests:

```
soak-census: core=1 threads=7 C0 C1 W1 R2 C3 C3 W3
soak-census: core=2 threads=7 R0 C0 R1 C1 C2 C2 G3
soak-census: core=3 threads=6 C0 G0 C1 G1 G2 C3
soak-census: core=4 threads=4 W0 C2 W2 R3
```

Three of the four grinders end up on core 3, and core 4 holds four threads and no grinder at all.

That reads at first as a refutation of the grinder-co-location inference above, and it is a
correction to its wording rather than to its substance. Piling grinders together is the efficient
arrangement: it spends one core on pure compute and leaves three for IPC. What starves a group is a
grinder *sharing a core with it*, which is what spreading the grinders one per core would produce.

So the reading now makes a falsifiable prediction about the run nobody has seen. A 23,000/s boot
should show the four grinders spread across four cores. If one does, the mechanism is confirmed; if a
slow boot shows them piled, this reading is wrong and the cause is something else.
The arrangement converges once, early, and then locks in. Over the whole run there was exactly
one drift event: the spawn arrangement held about 25 seconds, then ten of the twenty non-waiter
threads moved at once (`drifted=10`, at `crossings=983`), a replacement census printed, and
`drifted=0` held for the next 24 minutes. Under QEMU milestone 240 measured nine to eleven threads
leaving their spawn core within the *first* beat and churning after it. Both machines converge, as
DECISIONS 28.2's local wake implies; the difference is that on this silicon convergence is a single
event with a settled arrangement on the far side, which makes the census a far stronger instrument
here than the emulator predicted. A boot's arrangement is knowable about thirty seconds in and then
does not change.

An earlier draft of this section said `drifted=0` held from the start and that the spawn arrangement
was the settled one. That was written from the first five minutes of beats and the drift event is at
about thirty seconds; the correction is recorded rather than patched over because it changes what the
instrument is for.

This is one census, on the fast side of the draw. The slow run predates the instrument, so the
arrangement that produces 23,000/s has still never been seen. That is exactly what a series of boots
is for, and until one has run, the grinder-co-location reading above is a hypothesis with one
supporting observation and a plausible mechanism.

### A smaller effect, inside one boot

Detaching `script/board-console` mid-run took the 13:04 boot from a steady 183,130/s to a steady
194,000/s, about 6%. Same boot, so placement was constant and the comparison is fair, which is
more than can be said for the eightfold figure above. It is recorded because a rate owes the
regime it was measured in: whether a reader was draining the serial port is part of the
measurement. It has been seen once and is not confirmed.

## radon, 2026-10-09 to 10: the redraw boots, opened by a self-reset

<!-- prose-budget: exception. The three rows below are the runs' record, which milestone 225's own
block prescribes for this file ("Record rounds, rate, wakes and crossings for every run, in
notes/soak.md's table"), and the 2026-10-10 table plus its two caveats is about 210 words this
note's budget under §212 (a prose budget) cannot absorb; the narrative was cut to the bone first
and lives in the 225 and 592 blocks. The runs were ruled recorded by calef at the bench,
2026-10-10.
Reason: a run table is a measurement, not prose, and deleting rows to meet a word budget is the
record getting worse to make a gate quiet. -->

Milestone 225's second bench evening on radon, lane `milestone/225-radon-boots` (`bef2b40dc`),
against §259 (a multicore soak counts toward risk 5 at ten million crossings over three boots):
met, 10,288,805 crossings over the 3 plain boots, zero defects. The evening's account, 592's
proven self-reset included, is in the 225 and 592 blocks; this table is the four figures the run
owes, per this note's own rule.

| row | boot | duration | rounds | rate | wakes | crossings |
|---|---|---|---|---|---|---|
| E5 | rebooting soak, 120s | 2m | 36,462,561 | 314,841/s | 46,680 | 20,474 |
| E6 | plain soak | 10h 17m | 6,559,401,285 | 178,629/s | 14,806,027 | 5,569,327 |
| E7 | plain soak | 3h 53m | 4,387,639,929 | 317,457/s | 5,581,447 | 610,897 |

Two recording gaps, stated rather than hidden. E6's watcher hit its 620-minute deadline with the
board healthy, and the board soaked about 7 more hours unwatched: not evidence, not counted. And
one file holds E5 and E6 because the same console watched the reset between them.

## Why this extends `board_console` and not the other two instruments

`script/repeat-under-load` and `script/interleaving-check` are the tree's existing load and
concurrency instruments, and neither was the right place for this.

- `script/repeat-under-load` repeats a terminating suite N times with the host deliberately
  loaded, and reports what the load actually was. A soak has no runs to repeat and does not
  terminate, and the contention it wants is the guest's own rather than the host's. The two are
  complements: that one asks "does the suite still pass when the machine is busy", this one asks
  "does the machine stay correct when it is busy for hours".
- `script/interleaving-check` is loom over the extracted protocols, on the host, searching every
  interleaving the C11 model permits. It is the strongest evidence available about those protocols
  and it says so honestly: loom models C11, not ARM and not RISC-V. A soak on silicon is the
  evidence loom cannot give, not a substitute for it.
- `crates/board_console` was the right one, because the thing a soak needs that did not exist is
  a judgment about *silence*, and that crate already owned it.

## How a hang is told from a slow run

One rule, and both halves of the tree implement it rather than agreeing to:

The heartbeat is on the wall clock, not on the work. A machine doing one round trip a second
still prints on time, with a `rate` that says it is crawling. A machine doing none still prints, and
its `stalled` count fires. So silence means the thing that prints is itself wedged, which is the only
thing silence is allowed to mean.

`crates/board_console` is the other half. Its `Stage::Soak` is reached by the kernel's own
`soak-test: started` line, and reaching it re-arms the quiet check that a completed boot tour
suppresses: a halted kernel is supposed to be quiet and a soaking one is not. That is a one-word
change (`< Stage::Tour` became `!= Stage::Tour`) and it is the whole agreement. Beat interval five
seconds against a fifteen-second default quiet window: three missed beats before a run is called a
hang, exit status 2.

`script/soak-test` runs the QEMU side through the same recognizer and the same policy, so the local
rehearsal and the bench run are one experiment with different deadlines.

## Running it

### Under QEMU, which is the rehearsal

```
script/soak-test                             # aarch64, one minute
script/soak-test --arch riscv64 --for 10m    # radon's architecture
script/soak-test --arch x86_64 --smp 1       # xenon's, forced to one core (see BUGS)
```

Exit statuses are `script/board-console`'s: `0` beat for the whole watch, `1` announced a failure,
`2` went quiet, `3` QEMU exited early or the workload never started, `4` build or arguments.

### On radon at a bench, which is the experiment

This is the procedure, in order. It assumes the runbook in `notes/visionfive2.md` for the cabling
and the U-Boot commands, and changes only two things about it.

1. Build the payload with the soak feature.

   ```
   script/board-image --soak
   ```

   The flag exists rather than a hand-built kernel because that script builds the archive before
   the kernel, and that order is load-bearing: the archive regenerates the measurement manifest the
   kernel compiles in as its trust root, and building them the other way round is what produced
   `MEASURED BOOT REFUSED` at the bench on 2026-08-15. It prints the `dd` commands; it runs
   nothing destructive itself.

2. Copy the image to the microSD card and put it back in the board, exactly as the runbook says.
   The archive must be the one built beside this kernel or the measured-boot gate refuses it.

3. Start the watcher before powering the board, so the boot itself is captured:

   ```
   script/board-console --for 8h --until none --log target/radon-soak-$(date +%s).log
   ```

   `--until none` is what makes it a sustained watch rather than a boot check. Leave
   `--quiet-after` at its default unless the console is noisy.

4. Power the board and type the four U-Boot commands the runbook gives (milestone 218 (every) is about
   removing this step).

5. Watch for `soak-test: started`. Its own line names the worker mix, and on a four-hart JH7110 it
   should read four groups and 24 user threads. If it does not appear at all, the kernel was built
   without the feature or the archive has no `soaker` entry; the tour's last line will be there
   either way.

6. Check the first heartbeat before you walk away, which takes five seconds and is the whole of
   milestone 221's bench procedure. Two fields decide whether the cross-core experiment is actually
   running:

   - `wakerate` should be about `100 * harts`, so roughly 400 on radon. `TICK_HZ` is 100 and
     every online hart signals the tick route on its own timer, so a rate well under that means the
     timer or the wake path is falling behind and the run is measuring something else.
   - `crossings` must be *rising* between beats. Frozen is the pre-221 state and means the tick
     route is not armed: a kernel built without `--features soak_test` cannot get this far, so the
     realistic cause is that the intid was already routed, and the kernel says so and refuses to
     start rather than soaking silently without it.

   If either is wrong, stop and fix it. Eight hours of a soak that is not crossing cores is eight
   hours of the experiment milestone 219 already ran.

7. Leave it. The watcher stops at the deadline, or the moment the board announces a failure, or
   after three missed beats. The log is the artifact; the last `soak:` line in it is the number.

8. Record the numbers in this note's table, beside the QEMU rows, with the date and the
   duration: `rounds`, `rate`, `wakes` and `crossings`, and all four rather than the first two,
   because a later run cannot be compared on a figure this one did not write down. That is the only
   thing that makes an eight-hour vigil worth having sat through.

What a green run on radon would license, stated before it happens so that nobody writes it
afterwards. One sentence: *this board did N cross-core IPC round trips and M cross-core thread
handoffs over H hours without the wake gate refusing a wake, without a wrong reply, and without a
worker stalling.* That is the first evidence this project will have had about the wake protocol on
real silicon under sustained cross-core traffic, and it is a confidence rather than a verdict, which
is what `design/fatal-risks/README.md` says about this whole class.

To confirm a build soaks at all without waiting: `script/board-console --for 3m --until soak`
returns as soon as the workload announces itself.

xenon takes this procedure (its stick: `cargo xtask uefi-image --features soak_test`).
argon cannot yet boot nife; milestone 225's block says why.

## The rebooting soak on radon, which is milestone 249's experiment

*(Milestone 249. `--features reboot_soak_test`, `script/board-image --soak --reboot`,
`script/board-console --tally`.)*

Nothing in this section has run on radon. It was written on 2026-09-03 with the board powered
off and no bench session available, which is the same condition `notes/x86-uefi-boot.md` was written
in and the same reason its procedure is as detailed as it is. Every claim below is either about code
in this tree, which was built and host-tested, or is a question for the bench, which is marked as
one. The first four steps answer questions nobody here can answer.

### Why a rebooting soak, in one paragraph

The section above records four runs on radon whose rates span fifteenfold, and milestone 240's
census explains them: the rate tracks the number of cores that hold an IPC thread and no grinder.
Counting the nine boots of 2026-09-03 that way, six landed on two clean cores, two on one, and one
on none. Three and four clean cores have never been drawn, and nothing says whether that is rare
or structurally impossible. The distribution is the missing thing, and it is missing because every
draw cost a person a walk to the board.

### The hazard, and the four things that answer it

A board that reboots itself on a timer is a board nobody can get back. Every boot runs the same
image and reboots again, so without an escape the only way back is pulling power and rewriting the
card. That is worse than the problem being solved, and it is why this is a milestone rather than a
one-line change. Four mechanisms, strongest first, in AGENTS.md's own ladder:

1. The loop exists only in a build that asked for it, by a name with `reboot` in it.
   `--features reboot_soak_test`; `script/board-image --soak --reboot`. An ordinary card, a `--soak`
   card, and every QEMU run are untouched, which means the failure cannot arrive by accident.
2. Any build of it for a non-riscv64 target is a compile error, not a card that quietly never
   resets. The reset is SBI's and the escape is the NS16550's line-status register; neither exists
   elsewhere, and a card that silently never rebooted would look exactly like a board that drew the
   same placement fifty times.
3. The kernel polls the console UART's data-ready bit every beat (five seconds) and again
   through the five seconds before each reset. The bit is sticky: it is set while a byte sits
   unread and is cleared only by reading that byte, and nothing in a soak boot reads it. So the
   question being asked is *"has anybody typed since this armed"*, not *"is anybody typing right
   now"*, and a poll every five seconds cannot miss a keypress. Any byte counts, so no character has
   to be agreed on between the board and whoever is at the terminal.
4. Stopping disarms the reboot and leaves the soak running. It does not halt the kernel. That is
   deliberate and it is the better half of the design: a halted kernel is silence, and `Stage::Soak`
   has already told `board_console` that silence after a soak starts is a hang, so stopping the loop
   would have reported itself as the failure this whole instrument exists to detect. Disarming
   leaves the board in milestone 219's well-understood state, still beating, and the run is not
   thrown away to get the board back.

**And the fallback that needs no cooperation from this kernel at all**: U-Boot's autoboot countdown
runs on every one of these boots, and anything typed into it drops the board at `StarFive #`. That
is the escape a person had before milestone 218 removed the need for it, it is a two-second window
rather than a five-second one, and it is what remains if the kernel's own escape turns out not to
work. The card is the one after that.

**A bounded reboot count was considered and cannot be built here.** A cap of fifty would be the
obvious rung-one answer, and there is nowhere to keep the count: a cold reset takes the RAM, and the
only persistent store on the path is the U-Boot environment in the SPI flash of the only board of
its kind this project owns, which milestone 218 already refused to write to for the same reason.
What is bounded instead is the *wall clock per draw*, which is a weaker property honestly stated:
the board never wedges in the loop, it only stays in it.

### Verifying the reset before anything is left unattended

**Two facts this tree does not have, and the bench gets both in the first four minutes.**

**Answered on the bench, 2026-09-04, and the answer is a third outcome this note did not predict.**
radon's OpenSBI **accepts** SRST reset type 1 and never returns.
**The board does not come back.** Corrected 2026-09-24: there is no reset. OpenSBI's reset is an I2C
write to the PMIC; it fails and OpenSBI hangs (notes/board-reboot.md). From `target/board/radon-2026-09-04-srst-reset-pmic.log`, in file order:

```
line   3: U-Boot SPL 2021.10 (Feb 12 2023 - 18:15:33 +0800)     <- the power-on boot
line 241: nife on RISC-V (rv64, S-mode, Sv39)
line 399: soak-reboot: rebooting now (SBI SRST system_reset, reset type 1, cold reboot).
line 401: i2c read: write daddr 36 to
line 403: i2c read: write daddr 36 to            (repeating)
     ...: cannot read pmic power register
```

**So the outcome table below is incomplete**, and the missing row is the one that actually happened:
the firmware neither refuses with `-2` nor goes dark at the `ecall`. It accepts, resets, and the
*firmware on the way back* fails. Something the PMIC needs is not reinitialised by a warm SoC reset
the way it is by removing power, and U-Boot 2021.10's SPL has no recovery for it.

**What it settles.** An unattended series is not available on radon by this route. The escape works
and was verified the same evening (`soak-reboot: DISARMED at t=75s`, sent mid-soak, with the soak
carrying on past the 120s mark it would otherwise have rebooted at), so the mechanism is sound and
the firmware is the wall.

**And it retires a guess.** Milestone 249's block refused a smart-plug series as *"a lane spent on a
guess until the firmware has actually refused reset type 1"*. The firmware has now effectively
refused, in a way no amount of reading could have predicted, so milestone 224 (nothing can
power-cycle radon, so a hung soak needs a person) moves from a convenience to the only remaining
route to an unattended series.

**A correction, recorded because it cost the architect an hour of a late evening.** The maintainer
reported this working, twice, from the `U-Boot SPL` banner at line 3 of that log, read out of `grep`
output whose order is the file's rather than the event's. That banner is the power-on boot. Nothing
in the transcript ever showed a boot on the far side of a reset. **The tell is that the reboot line
is at 399 and the banner at 3**, and the only reliable reading is line order within one segment.

**Does the escape work on this cable?** Nothing in the kernel can prove it: a UART cannot receive a
byte it sends, so a receive path that is miswired, unpowered at the adapter, or held by something
else reads "nobody typed" forever and is indistinguishable from nobody typing. **This is the one
mechanism in the design that rests on a procedure rather than on a machine**, and the procedure is
step 4 below. Do not skip it because the first boot looks healthy; a healthy boot is exactly what a
board with a dead receive line looks like.

### The procedure, in order

Steps 1 and 2 need no board. It assumes the cabling and the U-Boot behavior in
notes/visionfive2.md, and milestone 218's boot script, **which has itself never run on the board**:
if the card lands at `StarFive #` instead of booting, that is 218 and not this, and the manual
commands `script/board-image` prints still work from there.

1. **Build the rebooting payload and write the card.**

   ```
   script/board-image --soak --reboot --card /Volumes/NIFE
   ```

   The archive is built before the kernel by that script and the order is load-bearing; the section
   above says why. It prints a warning block naming what the card will do and how to stop it. *If it
   refuses with "--reboot needs --soak"*, that is the flag pair, not the board.

2. **Read the boot script that will run**, so step 5's transcript is being compared against
   something: `cat target/board/boot.cmd`.

3. **Start the watcher before powering the board.** Two hours is fifty draws at two minutes each
   plus boot time; make it longer than you think and stop it with a key.

   ```
   script/board-console --for 3h --until none --log target/radon-lottery-$(date +%s).log
   ```

   `--until none` is what makes it a sustained watch. Leave `--quiet-after` alone: a reboot's dark
   period is a few seconds of SPL and U-Boot output rather than silence, so the fifteen-second
   window is not at risk, and shortening it would make a slow boot look like a hang.

4. **Power the board, and on the FIRST boot press a key, once, after `soak-test: started` appears.**
   This is the step that verifies the escape and it is not optional.

   Expect, within five seconds, `soak-test-reboot: DISARMED at t=Ns: a byte arrived on this console.`
   The board then keeps soaking and never reboots. **If that line does not come**, the escape does
   not work on this cable and **nothing further in this procedure should be run**: power the board
   off, find out why the receive path is dead, and only then start again.

   Then power-cycle to start the series for real, and type nothing at it after this.

   **Milestone 324 makes this step a command rather than a keystroke**, and the check it performs
   is the same one:

   ```
   script/board-console --stop
   ```

   It sends the byte itself, on the board's own arming announcement, prints it into the log in hex,
   and waits fifteen seconds for the `DISARMED` line. Exit `0` is this step passing; exit `3` with
   *sent the escape and the board did not acknowledge it* is this step failing, which is the stop
   that matters. It is worth preferring to a keystroke for one reason beyond convenience: a key
   pressed at a terminal leaves nothing in the capture, and this leaves both halves of the
   exchange in the artifact the run is judged from. **No byte of it has yet reached a board**; see
   notes/board-console.md.

   A whole series can be ended the same way in place of step 3's deadline:
   `script/board-console --stop-after 50` watches, counts draws, and sends the escape on the
   fiftieth, which is a series with exactly the sample it was asked for rather than one cut off by
   a clock. That has not been run on a board either.

5. **Watch the first two draws before you walk away.** The whole cycle should read:

   ```
   soak-test-reboot: THIS BUILD REBOOTS THE BOARD. It soaks for 120s, then asks the firmware ...
   soak-test-census: core=1 threads=... (the spawn placement)
   soak-test: t=5s beat=1 rounds=... rate=.../s ... drifted=0 ...
   soak-test-census: where the workers are NOW, ...        (about 25s in, once)
   soak-test: t=120s beat=24 ...
   soak-test-reboot: window reached at t=120s. Cold-rebooting in 5s ...
   soak-test-reboot: rebooting now (SBI SRST system_reset, reset type 1, cold reboot). ...
   U-Boot SPL 2021.10                                  (the next draw)
   ```

   The two beats worth checking are still milestone 221's, and for its reasons: `wakerate` about
   `100 * harts` (roughly 400 here) and `crossings` rising between beats.

6. **Leave it.** The watcher stops at the deadline, on a failure, or after three missed beats.

7. **Tally the log.** This needs no board and can be run on a partial capture at any time:

   ```
   script/board-console --tally target/radon-lottery-....log
   ```

   It prints one row per draw (clean cores over online cores, the last rate, how it ended) and then
   the distribution. **A core is clean when it holds a responder or a caller and no grinder**, from
   the *last* census that boot printed, because the spawn placement is the lottery's ticket and the
   settled arrangement is what the machine ran.

8. **Record the table in this note**, beside the nine hand-drawn boots, with the date and the build.
   The comparison that matters is against those nine: they are the control, they were drawn by a
   power cycle rather than by a warm reset, and if the automated distribution does not overlap them
   where it should, **that** is the finding rather than the distribution.

### What each outcome means

Read this against the log, in this order; the first row that matches is the one to act on.

| What the console shows | What it means | What to do |
|---|---|---|
| `soak-test-reboot: DISARMED` on boot 1 after you press a key, or after `script/board-console --stop` reports exit 0 | The escape works. This is step 4 passing. | Power-cycle and start the series. |
| No `DISARMED` after pressing keys for a beat or two, or `--stop` exiting 3 having sent the byte | The receive path is dead, and the escape does not exist on this cable | **Stop.** Power off. Check the adapter's TX into the board's RX and the ground; nothing else here is safe until this works. |
| `--stop` exiting 3 having sent **nothing** | No armed reboot loop announced itself: the card may not carry a `--reboot` build, or the watch ended before a draw came round | Check `target/board/boot.cmd` and the build flags, and give `--for` longer. Nothing was written to the board. |
| `soak-test-reboot: DISARMED` on boot 1 with nobody typing | Something wrote to the port, or U-Boot left a byte the arming drain did not catch | Detach anything else holding the port. Harmless: it fails toward not rebooting. |
| `rebooting now`, then `U-Boot SPL` a few seconds later | **The mechanism works.** SRST reset type 1 is implemented and the loop is running. | Nothing. This is the series. |
| `rebooting now`, then `i2c read` retries and `cannot read pmic power register` | **What radon actually does** (2026-09-04). OpenSBI's PMIC write fails and it hangs before any reset (notes/board-reboot.md). A third outcome. | Power-cycle to recover. See notes/board-reboot.md for the kernel-side fix; milestone 224 is the alternative. |
| `rebooting now`, then `soak-test-reboot: FAILED ... sbiret.error=-2` | This OpenSBI implements SRST shutdown and **not** cold reboot | The route is closed. The soak keeps running and the board is fine. A smart-plug power cycle is the alternative mechanism; raise it. |
| `rebooting now`, then nothing, and the board is dark | The firmware treated reset type 1 as a shutdown | Power the board back on. Same conclusion as the row above; record which of the two happened, because they are different firmware bugs. |
| `rebooting now`, then nothing, and the board is powered but silent | It reset and hung before SPL, or the console dropped | Power-cycle. If it recurs at the same point, that is a finding about the reset path and worth more than the distribution. |
| `soak-test: FAILED ...` then `[PANIC]` and the series stops there | **The best possible outcome.** Risk 5's decisive experiment found something | Do not restart it. The board holds the state and the log holds the census of the arrangement that produced it. |
| `U-Boot SPL` with no `soak-test: started` after it | A boot that never reached the workload | `--tally` counts these separately. Read the log around it: `MEASURED BOOT REFUSED` is a mismatched pair, `### ERROR ###` is milestone 218. |
| The watcher exits 2 (went quiet) mid-series | Three beats missed with no reboot announced | A wedge, which is what this is all for. Leave the board alone and read the last census in the log. |

### What a completed series licenses, written before it runs

One sentence, and it is narrower than it will feel: *over N unattended boots of this board, with
this firmware and this build, the settled arrangement had k clean cores this many times, and the
round-trip rate at each k was this.*

It licenses nothing about argon or xenon, nothing about why placement lands where it does (that is
DECISIONS 138), and nothing about whether three or four clean cores are *impossible* rather than
merely unseen: fifty draws that never show four is evidence about a probability and not a proof of
zero. **And every draw is a warm reset rather than a power cycle**, so anything that survives a warm
reset is held constant across the whole series in a way the nine hand-cycled boots did not hold it.
Those nine are the control and the overlap is the check.

## How long to run it, and why nobody can tell you

This note and milestone 225 (run the soak on radon, argon and xenon, which is the only place its
answer means anything) both say no duration is prescribed because nobody knows what would be
persuasive. That was written as an admission. It went unchecked until 2026-09-03, when a lane went
looking for whoever does know, and the honest result is that **the admission was correct, and it is
the field's condition rather than this project's.** Nothing found prescribes a duration for a
concurrency soak, and the one place a duration *is* derived derives it from a thermal model that has
nothing to do with interleavings.

Everything below was fetched and read on 2026-09-03. Where a thing was not found, it is written as
not found rather than as absent.

### seL4 runs nothing sustained, and its multicore tests are measured in milliseconds

This is the kernel this project measures itself against, so it is the first place to look and the
most surprising answer.

`seL4/sel4test`'s test directory (`apps/sel4test-tests/src/tests`, read at `master`) contains no
stress, soak, load or endurance file. Its multicore coverage is `multicore.c`, and the shape of
every test in it is the same: start a helper, `sel4test_sleep(env, 10 * NS_IN_MS)`, check a counter
moved or did not. Ten milliseconds is the whole observation window, and the property under test is
functional (a suspended thread stops, a resumed one runs, an affinity change takes effect) rather
than statistical.

`seL4/ci-actions` (the repository holding seL4's GitHub Actions, directory listing read at `master`)
has 40-odd actions and none of them is a soak or a stress run. The hardware ones are `sel4test-hw`,
`sel4test-hw-run` and `sel4test-hw-matrix`, which run the terminating suite above on real boards, and
`sel4bench-hw`, which is a benchmark. `sel4test-hw`'s own `action.yml` describes itself as
*"Runs sel4test builds for all hardware test platforms."*

On why there is not more, the project's own words, from Gerwin Klein on the seL4 Discourse thread
*Testing infrastructure* (2021-02-09): the CI is *"pull request checks (style, compile, licenses,
etc)"* and *"continuous integration test (either on the master branch of a specific repo, or, more
commonly on repo collections/manifests)"*, and, plainly, **"hardware tests are harder, proposals
welcome"**.

The reading to take from this is not that seL4 is careless. It is that **a project with a functional
correctness proof does not buy much from a soak**, because the thing a soak samples is the thing the
proof already covers. nife has 145 Kani harnesses and no refinement proof, so the trade is not the
same one, and copying seL4's answer here would be copying a conclusion without its premise.

### stress-ng picks a round number and says so

`stress-ng(1)` is the closest thing Linux userland has to a standard soak tool. Its `-t, --timeout T`
option reads, verbatim (Debian testing manual page, fetched 2026-09-03):

> run each stress test for at least T seconds. One can also specify the units of time in seconds,
> minutes, hours, days or years with the suffix s, m, h, d or y. [...] A 0 timeout will run stress-ng
> forever with no timeout. The default timeout is 24 hours.

**Twenty-four hours, with no stated reason.** The manual page's only account of what the tool is for
is that *"stress-ng was originally intended to make a machine work hard and trip hardware issues such
as thermal overruns as well as operating system bugs that only occur when a system is being thrashed
hard"*, and its one strongly worded caveat is about throughput rather than duration: *"it has never
been intended to be used as a precise benchmark test suite, so do NOT use it in this manner."*
Nothing in it says how long is long enough, or what a clean run licenses.

### The Linux Test Project prescribes nothing either

LTP's documentation (`setup_tests`, read 2026-09-03) treats runtime as a resource to be capped, not a
target to be reached: tests that run for more than a second or two must declare a `runtime` and check
actively how much is left, and `LTP_RUNTIME_MUL` and `-I` scale it. **The knobs are all for making
runs shorter.** No recommended soak length was found.

### Hardware is the exception, and its number is derived

Semiconductor qualification is the one practice found where "run it for N hours" is a real
requirement rather than a habit, and it is worth reading closely because **the derivation is the part
that does not transfer.**

JEDEC Standard No. 47G, *Stress-Test-Driven Qualification of Integrated Circuits* (fetched
2026-09-03), Table 1, requires High Temperature Operating Life at Tj at or above 125 C, Vcc at or
above Vccmax, 3 lots of 77 units, **"1000 hrs / 0 Fail"**. That is a hard number with a hard accept
criterion. And note 5.5(a) says where it comes from:

> with apparent activation energy of 0.7 eV, 125 °C stress temperature and 55 °C use temperature, the
> acceleration factor (Arrhenius equation) is 78.6. This means 1000h stress duration is equivalent to
> 9 years of use.

The same note is careful that the number is not self-justifying: *"The duration listed here is
generally acceptable to qualify for the given Application Level. However, it does not necessarily
imply the demonstration of the lifetime requirement for a particular use condition."*

So the hardware world has what the software world does not: **a model that converts stress hours into
a claim about the field.** Arrhenius does that for a wearout mechanism at a raised temperature. There
is no analogous model that converts soak hours into interleavings explored, and the reason is the next
section.

Part of what a board soak tests genuinely is the board, and this row of the table is the one that
applies to that half: radon under sustained load is closer to an operating-life sample than to a
concurrency test. It is also the half nife is least equipped to judge, having one unit per
architecture where JEDEC wants 231.

### The academic angle exists, and its finding is that clock time is the wrong axis

There is a literature here, and it is not neutral about stress testing.

Burckhardt, Kothari, Musuvathi and Nagarakatte, *A Randomized Scheduler with Probabilistic Guarantees
of Finding Bugs*, ASPLOS 2010 (the PCT paper, PDF fetched 2026-09-03), opens by describing exactly the
practice this milestone is about:

> Popular testing methods involve various forms of stress testing where the program is run for days or
> even weeks under heavy loads with the hope of hitting buggy schedules. This is a slow and expensive
> process. Moreover, any bugs found are hard to reproduce and debug.

Two of its results bear directly on choosing a duration.

**The state space is not the thing to cover, and bug depth is.** The paper defines *"the depth of a
concurrency bug as the minimum number of scheduling constraints that are sufficient to find it"*, and
proves that a run of a program with n threads and k steps finds a bug of depth d with probability at
least `1/(n k^(d-1))`. It observes that a naive bound over schedules is useless (*"This program, to
the first-order of approximation, has n^k possible thread schedules"*) and rests on the claim that
real bugs are shallow: *"Concurrency bugs typically involve unexpected interactions among few
instructions executed by a small number of threads."* Their examples put ordering errors at depth 1
and atomicity violations and lock-cycle deadlocks at depth 2.

That claim is independently measured. Lu, Park, Seo and Zhou, *Learning from Mistakes: A Comprehensive
Study on Real World Concurrency Bug Characteristics*, ASPLOS 2008 (PDF fetched 2026-09-03), examined
105 real concurrency bugs in MySQL, Apache, Mozilla and OpenOffice, and reports as finding 3 that
**"Almost all (96%) of the examined concurrency bugs are guaranteed to manifest if certain partial
order between 2 threads is enforced"**, and as finding 8 that **"Almost all (92%) of the examined
concurrency bugs are guaranteed to manifest if certain partial order among no more than 4 memory
accesses is enforced."** Their own caveat is attached and should be carried: the findings *"are
associated with the four examined applications and the programming languages these applications use"*.

**And stress-test coverage saturates, measurably.** This is the single most useful thing found, because
it is a measurement of the exact question this note asks. PCT section 5.3.3 instrumented a work
stealing queue with twenty events, 168 possible event pairs, and compared coverage against run count:

> We restrict the horizontal axis to the 8192 runs as stress did not explore any new event pair beyond
> those already explored in the new runs after that and PCT eventually explored all the event pairs.
> [...] Fig. 11 shows that stress does not cover more than 20% of the event pairs, few of which result
> in a bug. Thus, stress's inability/ineffectiveness to detect the bug is highly correlated with the
> event pairs not covered.

Their stress infrastructure was not a strawman by their account: it inserted *"random sleeps, thread
suspensions, and thread priority changes"*, which is a superset of what this soak's jitter does. It
still stopped finding anything new, and then ran forever without improving.

**That is this note's own line, measured by somebody else.** *A soak that repeats one interleaving for
eight hours has explored one interleaving* was written here as an intuition. PCT put a number on the
shape of it: coverage climbs, flattens, and the flat part is free.

### So the field has a habit, not a standard, and here is what to do instead

Stated plainly, because it is a real finding and dressing it up would be worse than useless:
**24 hours, 48 hours and overnight are round numbers.** The one prescribed duration found anywhere
(1000 hours) is prescribed for a thermal wearout model, states its own derivation, and warns that the
number does not by itself demonstrate the requirement. For concurrency, nothing found in tooling
documentation, in seL4's practice, or in the literature converts clock time into a claim.

**The alternative is to reason from this workload's own counters, which is available and is not
available to most people asking this question.** `script/soak-test` already prints, every five seconds,
`rounds`, `rate`, `wakes`, `wakerate`, `crossings`, `remote`, `steals` and `deferred`. Three questions
those support, none of which is "how many hours":

1. What is the run buying per hour, in the units that matter? Not round trips, which saturate the
   machine by construction, but `crossings`, since the cross-core wake path is what the tick route exercises (the defect
   once recorded there was retracted; see `multicore-defect-curve.md`, row D6) and the crossing rate is one to two orders of magnitude below the round-trip rate. A radon run's
   crossing rate is the honest denominator: at the QEMU aarch64 figures (about 3,779 crossings in 25
   seconds on the better of two runs) an hour is a few hundred thousand crossings, and a second hour is
   another few hundred thousand of the same kind. Decide the duration against a target crossing count,
   arrived at deliberately, and then say what it was.
2. Is the run still producing new behavior, or is it flat? This is PCT's saturation question and
   this tree cannot currently answer it, because nothing here counts distinct behavior. `remote`,
   `steals` and `deferred` are the closest available and are volumes rather than varieties. **This is
   the gap worth closing before the duration argument is worth having**, and it is a milestone rather
   than a note: something like a coarse histogram over the placement decisions, so a beat can be
   compared with the beat before it and a flat run can be recognized as flat.
3. **Would the time be better spent on more starts than on longer running?** The crossing count varies
   by more than a factor of two between identical runs, which is recorded in this note's BUGS and is
   evidence that the initial conditions matter more than the tail. Under PCT's model, independent runs
   multiply the probability of finding a shallow bug and a single long run does not; ten one-hour boots
   are ten samples of the boot-time placement lottery, and one ten-hour boot is one. Nothing here
   proves that trade for this workload, and it is the question a duration decision should be made
   against rather than around.

`script/interleaving-check` is the complementary instrument and this section sharpens why: loom over
the extracted protocols searches the state space directly, which is the thing a soak samples badly and
saturates at. The two are not competitors and the soak is not the weaker one; the soak is the only one
that runs on silicon at all, which is where risk 5 says the defects are.

**What none of this decides is the number**, deliberately. It says the number is an architect's and
gives them the axis to pick it on: a crossing target on real silicon, chosen and written down,
rather than an hour count inherited from a tool's default.

## BUGS

- A soak that finds nothing is weak evidence, and this is the sentence to repeat. A clean eight
  hours licenses exactly one claim: *this machine did N cross-core IPC round trips without the wake
  gate refusing one, without a wrong reply, and without a worker stalling.* It licenses nothing about
  the interleavings that did not occur, and the ones that did not occur are where the remaining bugs
  are. `script/soak-test` prints this on every green run because a number quoted without it is a number
  quoted wrongly.
- No duration is prescribed, because nobody knows what duration would be persuasive. The risk's
  own text says this class "produces a confidence rather than a verdict". Eight hours is a night;
  it is not an argument. Checked against the field on 2026-09-03 and the admission stands: see *How
  long to run it, and why nobody can tell you* above, which is why it is a section rather than a
  longer version of this line.
- Nothing here counts distinct behavior, only volumes of it, so a soak cannot say whether it is
  still finding new interleavings or has gone flat. That is the measurement the duration question
  actually wants and this tree does not have it; the section above names it as the thing to build
  before arguing about hours.
- The heartbeat is guest time and the watcher's deadline is host time. Under heavy host load a
  QEMU guest's clock runs slower than the wall, so beats arrive later in host seconds than the
  kernel thinks it printed them. The three-beat margin absorbs the ordinary case; a machine running
  a mutation sweep beside a soak can produce a false `WentQuiet`. `--quiet-after` is the knob, and
  not running a soak beside other heavy work is the better answer (`AGENTS.md`'s memory ceiling).
- `--arch x86_64` soaked one core until 2026-09-23 unless `--smp` said otherwise, because that
  runner defaulted to one. Milestone 315 (a port revoke that reaches every core) closed the
  port-revocation window that was the last thing holding it there and moved the default to 2 per
  DECISIONS §153 (how a two-core x86_64 test earns its place), so an x86 soak now crosses cores
  like the other two. **Every x86_64 number in the tables above predates that**, was taken at one
  core, and its `crossings=0` says so out loud; they are single-core soaks and should not be reread
  as multicore ones.
- A soak build is not the binary that ships, so its timing is not the shipping binary's timing.
  The numbers above quantify it. This is normal and accepted, and it is stated here because the
  round-trip figures would otherwise read as IPC benchmarks, which they are not.
- The supervisor yields in a loop rather than sleeping, because this kernel has no
  sleep-until primitive a kernel thread can use. It is one more thread contending, which is not
  entirely a cost, and it is why these round-trip rates are not comparable with `script/bench`'s IPC
  numbers.
- A worker that dies looks exactly like a worker that wedged from the shared page. Both fail the
  run; the thread dump the supervisor prints before panicking is what separates them.
- A tick waiter's wakes are not round trips, and mixing the two figures is the misreading this
  workload is most likely to suffer. `rounds` counts IPC round trips and `wakes` counts tick-route
  wakes; they are separate fields because they are separate quantities.
- The crossings are the waiters, never the pairs. Repeated here because it is the claim a reader
  most wants this tool to be making and it is not making it.
- `wakerate` is a property of the machine, not of the workload, so it is not a throughput number
  and a run cannot be tuned to raise it. It is `TICK_HZ` times the online cores, and its use is as a
  liveness check on the timer and the wake path.
- The crossing count varies by more than a factor of two between otherwise identical runs
  (1,452 and 3,779 on the same aarch64 build, same command, same host). Whether a wake goes remote is
  `wake_load_aware`'s call and it depends on where everything happened to be; nothing here is wrong,
  and it means a single run's crossing count is not a figure to compare two builds on.
- A waiter whose `Irq::WAIT` is refused spins instead of saying so. It has no channel to report
  on, so it stops counting and the stall check speaks for it one beat later; the report then says
  "stalled" where "refused" would be more use.
- The census is `last_cpu`, so it is where a thread last *ran*, not where it is queued. A thread
  that has been placed on another core's inbox and not yet switched to still reads its old core, and
  a thread that has never run at all reads as unplaced. Both are honest answers to "where did this
  thread last execute" and neither is an answer to "where will it run next"; the census says
  `not-yet-run` for the second case rather than guessing.
- `drifted=` excludes the tick waiters, whose movement is milestone 221's whole point and is
  already `crossings=`. Folding them in would make the number rise on a healthy run and mean
  nothing. The cost is that a waiter which stopped moving does not show up here; the crossings rate
  going flat is what says that.
- A machine that genuinely thrashes prints a census every beat, which is four or five extra
  lines per beat and roughly doubles the log. Nothing rate-limits it beyond the one-per-beat check,
  on the argument that a run whose arrangement changes every five seconds is a run whose arrangement
  is the finding. No such run has been seen.
- The census counts by group and role and says nothing about priority, quota or how long a thread
  has held its core. Two arrangements that look identical here can still differ in ways this
  cannot show, so it narrows the space of explanations rather than closing it.
- The rebooting soak's escape is a poll of one bit, and nothing verifies the bit can ever be set
  (milestone 249). A receive path that is miswired or held by something else reads "nobody typed"
  forever, which is indistinguishable from nobody typing, and a UART cannot receive a byte it sends.
  What closes it is step 4 of the procedure above, and **milestone 324 moved that step up a rung**:
  it was a person pressing a key, which is rung four wearing a procedure's clothes, and it is now
  `script/board-console --stop`, which sends the byte and reads the board's `DISARMED` line back as
  an exit status. The kernel still cannot verify its own receive path, and nothing here changes
  that; what changed is that the host at the far end of the cable can, and now does it without
  anyone remembering to. **No `--stop` has yet run against a board**, so until one does, the
  verification is a tested decision attached to an untested wire.
- Nothing about the reboot has run on radon, including whether that OpenSBI implements SRST
  reset type 1 at all. The whole of milestone 249's mechanism is code that builds and host tests
  that pass. The tally is judged against one real capture with a census in it
  (`qemu-2026-09-03-riscv64-soak-census.log`, one clean core of four at 18,963/s), and **every
  multi-boot case it asserts on is text this project wrote**, because no multi-boot capture exists
  anywhere yet. That is the same gap `crates/board_console`'s own `BUGS` records for its recognizer,
  one milestone later, and the first bench log closes it.
- A rebooting series and a long run are different experiments and neither substitutes. Fifty
  two-minute draws measure the distribution over placements; the three-hour run above measures what
  one placement does over time, and it is the only evidence here that a slow draw is stable rather
  than a warm-up. Do not replace one with the other.
- The tally counts a boot by U-Boot's SPL banner, so it counts boots of the *board* and reports
  zero attempts on a QEMU capture, which then looks like fewer boots than draws. Honest and odd.
- **Nothing runs a soak in `script/test`.** A twenty-second leg per architecture would gate the
  build against bitrot, and it is not there: the soak is exercised by `script/soak-test` and by
  `board_console`'s host tests over a real capture. If the feature stops compiling, nothing will say
  so until someone runs the script.
