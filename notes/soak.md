# The workload that does not stop, and what a clean run of it is worth

*(Milestones 219 and 221. `kernel/src/soak.rs`, `fixtures/src/soaker.rs`, `crates/soak_page`,
`script/soak-test`, and the `Stage::Soak` half of `crates/serial_console`.)*

`design/fatal-risks/README.md`'s fifth entry, *it cannot be made reliable on multicore, and the bugs appear
only on silicon*, names its decisive experiment as sustained multi-core stress on the boards with
the load-sensitive assertions live. Until this milestone the tree could not sustain anything: the
boot tour ran its checks, printed its last line, and called `arch::halt()`. Captured on radon on
2026-09-01, that is the last thing the board says before it sits in `wfi` indefinitely.

This note is what the workload is, what number it produces, and, more usefully, what it was
measured to be unable to do. The evidence behind each section is in [`soak/`](soak/README.md).

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
which build produced it. Its own rate is pinned to the machine: `TICK_HZ` times the online cores, so
about 400 a second on a four-core QEMU. That makes it a useful liveness check in its own right. A
`wakerate` well under that is the timer or the wake path falling behind, not the workload.

`refused`, `mismatch` and `stalled` must all be zero, and any of them nonzero fails the run: the
supervisor prints `soak-test: FAILED`, dumps the threads (so the per-core event rings are on the log) and
panics.

The first QEMU numbers (2026-09-01 and, with the tick route, 2026-09-02) are a baseline for
comparison, not a benchmark. aarch64 ran about 58,000 round trips a second and riscv64 about 24,000.
The tick route cost aarch64 nothing measurable and riscv64 about 7%. Every figure comes from a
`--features soak_test` kernel, whose `ipc_fastpath` is five to six percent larger than production's,
so compare a soak number only with another soak number. A production build is byte-identical with
and without milestone 221. [`soak/qemu-2026-09.md`](soak/qemu-2026-09.md) has the tables.

## The finding: a saturated workload does not migrate under this scheduler

The cross-core handoff count freezes within the first second and never moves again, across three
topologies and both multicore architectures. The threads stay where `pick_spawn_target` put them. A
rendezvous wake is local on purpose (§28 (SMP placement), part 2), `wake_load_aware` is reachable only from a
device interrupt, a steal needs an idle core and a queued thread at once, and nothing rebalances.

So risk 5's "sustained multi-core stress" is two experiments. The soak sustains concurrent
contention on shared kernel state. It does not sustain cross-core handoff, which is where the
observed defect lived, and `script/soak-test` prints that gap on every run.
[`soak/saturated-workload-does-not-migrate.md`](soak/saturated-workload-does-not-migrate.md) has the measurement, the instrument that
found it, and why this extends `serial_console` rather than the other load instruments.

## The tick route: how the soak was made to cross cores (milestone 221 (the soak never crosses cores, so build the hook that makes it))

calef approved option D of DECISIONS 138 for it on 2026-09-02. Under `--features soak_test` only, `sched::on_tick`
signals one rendezvous per group, round-robin, and one tick waiter per group blocks on it through
`Irq::WAIT`. A tick then runs the same chain a device interrupt runs, through `wake_load_aware` and
the reschedule IPI. It adds no syscall and nothing in a production build, and it is
architecture-neutral, so radon is not left out.

What crosses is the waiters, not the pairs. The callers and responders are as pinned as ever. This
sustains the wake protocol across cores under load; it does not make the IPC workload migrate. The
mechanism had two ordering bugs, both found by running it (one rendezvous for all waiters, and routes
bound after the waiters spawned). [`soak/tick-route.md`](soak/tick-route.md) has the mechanism, the
"Two bugs this mechanism had" in full, and what it does and does not establish about risk 5.

## Where the threads are (milestone 240 (soak))

The soak prints a `soak-test-census:` block at start: where each worker was placed, one line per
core, `R`/`C`/`G`/`W` plus a group number. Every beat carries `drifted=`, the count of threads off
the last census's core, and a fresh block prints whenever it is nonzero. Under QEMU the start census
goes stale within five seconds, and what tracks the rate is how IPC groups share cores.
[`soak/placement-census.md`](soak/placement-census.md) has the format and the four QEMU runs.

## radon, on real silicon, 2026-09-03: the first run off a board

Two runs of the same card and build, twenty minutes apart, differed eightfold: 183,662/s against
22,592/s, on a machine the boot tour proved identical. The slow draw held for three hours, with
`refused=0 mismatch=0 stalled=0` throughout, so it is a throughput draw rather than a fault.
Crossings per second tracked the rate across four runs. The first census off a board showed one
convergence event about 25 seconds in, then a settled arrangement. The reading that fits is that a
grinder sharing a core with an IPC group starves it. So a single run's rate is close to meaningless,
and any rate quoted from this instrument owes a distribution. [`soak/radon-2026-09-03.md`](soak/radon-2026-09-03.md)
has the tables, "The three-hour run", the censuses and the corrections.

## radon, 2026-10-09 to 10

Run table: [soak/radon-2026-10-10.md](soak/radon-2026-10-10.md).

## radon, 2026-10-20

Thirteen 120-second reboot draws from one power-on, every reset clean, zero defects; two draws
never crossed. Run table: [soak/radon-2026-10-20.md](soak/radon-2026-10-20.md).

## radon, 2026-10-21

Twenty-five 120-second reboot draws from one power-on, zero defects; seven draws never crossed.
One draw reset without the kernel's reset sequence, the only such reset in any captured draw. Run
table: [soak/radon-2026-10-21.md](soak/radon-2026-10-21.md).

Later the same day, 36 more draws on a build that prints what the PMIC recorded at each arming:
zero defects, every reset the kernel's, every PMIC report clean. One drew a placement with no
clean core, at about 16% of the usual rate. Run table:
[soak/radon-2026-10-21-pmic.md](soak/radon-2026-10-21-pmic.md).

## How a hang is told from a slow run

One rule, and both halves of the tree implement it rather than agreeing to:

The heartbeat is on the wall clock, not on the work. A machine doing one round trip a second
still prints on time, with a `rate` that says it is crawling. A machine doing none still prints, and
its `stalled` count fires. So silence means the thing that prints is itself wedged, which is the only
thing silence is allowed to mean.

`crates/serial_console` is the other half. Its `Stage::Soak` is reached by the kernel's own
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
   the kernel, and that order is load-bearing. The archive regenerates the measurement manifest the
   kernel compiles in as its trust root. Building them the other way round is what produced
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
     route is not armed. A kernel built without `--features soak_test` cannot get this far, so the
     realistic cause is that the intid was already routed. The kernel says so, and refuses to
     start rather than soaking silently without it.

   If either is wrong, stop and fix it. Eight hours of a soak that is not crossing cores is eight
   hours of the experiment milestone 219 already ran.

7. Leave it. The watcher stops at the deadline, or the moment the board announces a failure, or
   after three missed beats. The log is the artifact; the last `soak:` line in it is the number.

8. Record the numbers in a table beside the QEMU rows, with the date and the duration: `rounds`,
   `rate`, `wakes` and `crossings`. Record all four rather than the first two, because a later run
   cannot be compared on a figure this one did not write down. That is the only
   thing that makes an eight-hour vigil worth having sat through.

What a green run on radon would license, stated before it happens so that nobody writes it
afterwards. One sentence: *this board did N cross-core IPC round trips and M cross-core thread
handoffs over H hours without the wake gate refusing a wake, without a wrong reply, and without a
worker stalling.* That is the first evidence this project will have had about the wake protocol on
real silicon under sustained cross-core traffic, and it is a confidence rather than a verdict, which
is what `design/fatal-risks/README.md` says about this whole class.

To confirm a build soaks at all without waiting: `script/board-console --for 3m --until soak`
returns as soon as the workload announces itself.

xenon takes this procedure (its stick: `cargo xtask uefi-image --features soak_test`). That
build arms the Intel TCO watchdog, as every x86_64 soak does, so a wedged xenon is reset 60 s after
its last beat (milestone 593 (a wedged kernel resets itself)). `--features unwatched_soak_test` in
place of `soak_test` is the opt-out.
radon's soak arms the JH7110's watchdog the same way, and a wedged radon is back at U-Boot SPL
about 62 s after its last beat (milestone 593's bench step). `script/board-image --soak
--extra-features unwatched_soak_test` is radon's opt-out.
argon cannot yet boot nife; milestone 225's block says why.

## The rebooting soak on radon, which is milestone 249's experiment

`--features reboot_soak_test` and `script/board-image --soak --reboot` make the board soak for 120
seconds, then reset itself through SBI SRST, so the boot-time placement lottery can be sampled
without a person walking to the board. A byte on the console disarms the loop and leaves the soak
running, and `script/board-console --stop` sends that byte and checks the acknowledgement.

On radon OpenSBI's own I2C write to the PMIC fails and hangs (notes/board-reboot.md). So the kernel
writes the AXP15060 directly (milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write)).
That reset came back through U-Boot on 13 of 13 draws on 2026-10-20. A board that hangs instead
still needs a person, since nothing can power-cycle radon (milestone 224 (nothing can power-cycle
radon, so a hung soak needs a person)). [`soak/rebooting-soak.md`](soak/rebooting-soak.md) has the hazard,
its four answers and "Verifying the reset before anything is left unattended";
[`soak/rebooting-soak-procedure.md`](soak/rebooting-soak-procedure.md) has the procedure and the
outcome table for a board whose firmware can reset.

## How long to run it, and why nobody can tell you

No duration is prescribed, and that is the field's condition rather than this project's. seL4 runs
no soak, stress-ng defaults to an unexplained 24 hours, and LTP's knobs only shorten runs. JEDEC's
1000 hours comes from a thermal model that says nothing about interleavings. The literature (PCT,
ASPLOS 2010) measures stress coverage flattening while the run goes on.

So pick a target crossing count on real silicon, chosen and written down, rather than an hour count
inherited from a tool's default. Prefer more starts to longer running, since each boot is a fresh
draw of the placement lottery. The number is an architect's call.
[`soak/how-long-to-run-it.md`](soak/how-long-to-run-it.md) has every source with its quotations.

## BUGS

The full list, every entry as written, is [`soak/limitations.md`](soak/limitations.md). The ones a
reader of a number most needs:

- A soak that finds nothing is weak evidence. A clean run licenses exactly the one-sentence claim
  above and nothing about the interleavings that did not occur. `script/soak-test` prints this on
  every green run.
- Nothing here counts distinct behavior, only volumes, so a soak cannot tell a flat run from a
  productive one.
- A soak build is not the binary that ships, so its round trips are not IPC benchmarks.
- The crossings are the waiters, never the pairs, and the crossing count varies by more than 2x
  between identical runs.
- Every x86_64 number in the baselines predates the two-core default of 2026-09-23 and is a
  single-core soak.
- The heartbeat is guest time and the watcher's deadline is host time, so a loaded host can produce
  a false `WentQuiet`.
- The census is where a thread last ran, not where it is queued, and says nothing about priority.
- No `--stop` has yet run against a board, and every multi-boot case the tally asserts on is text
  this project wrote.
- Nothing runs a soak in `script/test`, so a soak build that stops compiling goes unnoticed until
  someone runs the script.
