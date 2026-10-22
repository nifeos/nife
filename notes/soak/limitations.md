# The soak's limitations, in full

An appendix to [notes/soak.md](../soak.md), whose BUGS section keeps the short list. This is every
entry as it was written, sentence-split for the prose limits.

- A soak that finds nothing is weak evidence, and this is the sentence to repeat. A clean eight
  hours licenses exactly one claim: *this machine did N cross-core IPC round trips without the wake
  gate refusing one, without a wrong reply, and without a worker stalling.* It licenses nothing about
  the interleavings that did not occur, and the ones that did not occur are where the remaining bugs
  are. `script/soak-test` prints this on every green run, because a number quoted without it is a
  number quoted wrongly.
- No duration is prescribed, because nobody knows what duration would be persuasive. The risk's own
  text says this class "produces a confidence rather than a verdict". Eight hours is a night; it is
  not an argument. It was checked against the field on 2026-09-03, and the admission stands: see
  [how-long-to-run-it.md](how-long-to-run-it.md).
- Nothing here counts distinct behavior, only volumes of it. So a soak cannot say whether it is still
  finding new interleavings or has gone flat. That is the measurement the duration question actually
  wants, and this tree does not have it. The duration survey names it as the thing to build before
  arguing about hours.
- The heartbeat is guest time and the watcher's deadline is host time. Under heavy host load a QEMU
  guest's clock runs slower than the wall, so beats arrive later in host seconds than the kernel
  thinks it printed them. The three-beat margin absorbs the ordinary case. A machine running a
  mutation sweep beside a soak can produce a false `WentQuiet`. `--quiet-after` is the knob. Not
  running a soak beside other heavy work is the better answer (`AGENTS.md`'s memory ceiling).
- `--arch x86_64` soaks two cores by default, per DECISIONS §153 (how a two-core x86_64 test earns
  its place), but every x86_64 number in the tables was taken at one core, and its `crossings=0`
  says so out loud. They are single-core soaks and should not be reread as multicore ones.
- A soak build is not the binary that ships, so its timing is not the shipping binary's timing.
  [qemu-2026-09.md](qemu-2026-09.md) quantifies it. This is normal and accepted. It is stated
  here because the round-trip figures would otherwise read as IPC benchmarks, which they are not.
- The supervisor yields in a loop rather than sleeping, because this kernel has no sleep-until
  primitive a kernel thread can use. It is one more thread contending, which is not entirely a cost.
  It is why these round-trip rates are not comparable with `script/bench`'s IPC numbers.
- A worker that dies looks exactly like a worker that wedged, from the shared page. Both fail the
  run. The thread dump the supervisor prints before panicking is what separates them.
- A tick waiter's wakes are not round trips, and mixing the two figures is the misreading this
  workload is most likely to suffer. `rounds` counts IPC round trips and `wakes` counts tick-route
  wakes. They are separate fields because they are separate quantities.
- The crossings are the waiters, never the pairs. This is repeated here because it is the claim a
  reader most wants this tool to be making, and it is not making it.
- `wakerate` is a property of the machine, not of the workload. So it is not a throughput number,
  and a run cannot be tuned to raise it. It is `TICK_HZ` times the online cores, and its use is as a
  liveness check on the timer and the wake path.
- The crossing count varies by more than a factor of two between otherwise identical runs: 1,452
  and 3,779 on the same aarch64 build, same command, same host. Whether a wake goes remote is
  `wake_load_aware`'s call, and it depends on where everything happened to be. Nothing here is wrong.
  It means a single run's crossing count is not a figure to compare two builds on.
- A waiter whose `Irq::WAIT` is refused spins instead of saying so. It has no channel to report on,
  so it stops counting, and the stall check speaks for it one beat later. The report then says
  "stalled" where "refused" would be more use.
- The census is `last_cpu`, so it is where a thread last *ran*, not where it is queued. A thread
  placed on another core's inbox and not yet switched to still reads its old core. A thread that has
  never run at all reads as unplaced. Both are honest answers to "where did this thread last
  execute", and neither answers "where will it run next". The census says `not-yet-run` for the
  second case rather than guessing.
- `drifted=` excludes the tick waiters, whose movement is the whole point of milestone 221 (the soak never crosses cores, so build the hook that makes it) and is already
  `crossings=`. Folding them in would make the number rise on a healthy run and mean nothing. The
  cost is that a waiter which stopped moving does not show up here; the crossings rate going flat is
  what says that.
- A machine that genuinely thrashes prints a census every beat. That is four or five extra lines per
  beat, and roughly doubles the log. Nothing rate-limits it beyond the one-per-beat check. The
  argument is that a run whose arrangement changes every five seconds is a run whose arrangement is
  the finding. No such run has been seen.
- The census counts by group and role, and says nothing about priority, quota or how long a thread
  has held its core. Two arrangements that look identical here can still differ in ways this cannot
  show. So it narrows the space of explanations rather than closing it.
- The rebooting soak's escape is a poll of one bit, and nothing verifies the bit can ever be set
  (milestone 249 (the boot lottery is sampled by a person walking to the board)). A receive path that
  is miswired or held by something else reads "nobody typed" forever. That is indistinguishable from
  nobody typing, and a UART cannot receive a byte it sends. What closes it is step 4 of the
  procedure, and milestone 324 (the bench console cannot speak to any of the three boards) moved
  that step up a rung. It was a person pressing a key, which is
  rung four wearing a procedure's clothes. It is now `script/board-console --stop`, which sends the
  byte and reads the board's `DISARMED` line back as an exit status. The kernel still cannot verify
  its own receive path, and nothing here changes that. What changed is that the host at the far end
  of the cable can, and now does it without anyone remembering to. No `--stop` has yet run against a
  board. Until one does, the verification is a tested decision attached to an untested wire.
- Nothing about the reboot has run on radon, including whether that OpenSBI implements SRST reset
  type 1 at all. (The bench answered that on 2026-09-04: see [rebooting-soak.md](rebooting-soak.md).)
  The whole of milestone 249's mechanism is code that builds and host tests that pass. The tally is
  judged against one real capture with a census in it
  (`qemu-2026-09-03-riscv64-soak-census.log`, one clean core of four at 18,963/s). Every multi-boot
  case it asserts on is text this project wrote, because no multi-boot capture exists anywhere yet.
  That is the same gap `crates/serial_console`'s own `BUGS` records for its recognizer, one
  milestone later. The first bench log closes it.
- A rebooting series and a long run are different experiments, and neither substitutes. Fifty
  two-minute draws measure the distribution over placements. The three-hour run measures what one
  placement does over time, and it is the only evidence here that a slow draw is stable rather than
  a warm-up. Do not replace one with the other.
- The tally counts a boot by U-Boot's SPL banner. So it counts boots of the *board*, and reports zero
  attempts on a QEMU capture, which then looks like fewer boots than draws. Honest and odd.
- Nothing runs a soak in `script/test`. A twenty-second leg per architecture would gate the build
  against bitrot, and it is not there. The soak is exercised by `script/soak-test` and by
  `serial_console`'s host tests over a real capture. If the feature stops compiling, nothing will
  say so until someone runs the script.
