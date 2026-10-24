# The kernel test harness: watchdogs, the frame ledger, skips, filters and repeats

*Name: provisional, minted 2026-10-23 (UTC) by comments/929-kernel-syscall-selftest.*

`kernel/src/testing.rs` is the runner every `#[test_case]` in the kernel and in `system_tests` runs
under. `cargo test` builds the kernel with `cfg(test)`, the runner in `.cargo/config.toml` boots it
under QEMU, and the kernel reports pass or fail by asking QEMU to exit with a status through
semihosting. A panic anywhere lands in the panic handler, which exits with a failure, so the first
failing assertion ends the run. This note carries the design the file's doc comments link to.
`kernel/src/preemption_window_tests.rs` is covered at the end.

## The hang watchdog: two mechanisms for two ways a test never finishes

Both run off the boot core's timer interrupt, so they cost a few atomic loads per tick and do not
perturb the scheduling they watch. Either one firing fails the run with a thread dump.

1. The no-progress heartbeat, for a lost wakeup. `note_progress` counts every observable kernel
   step: a completed rendezvous or device-interrupt wake (`sched::wake`, `wake_load_aware`) and every
   line of console output, which covers each test's `ok`. A core running a real, non-idle thread
   also counts, so a CPU-bound userspace poll with no wake and no output is progress. Sixty seconds
   with none of that fails the run: every thread blocked, every core idle.
2. The per-test wall-clock ceiling, for a livelock. A loop that keeps doing IPC resets the
   heartbeat on every rendezvous. `Testable::run` stamps a start time and a budget per test, and
   the run fails when the budget is exceeded even while progress is being made.

What each can and cannot see:

- The heartbeat catches a total stall fast, wherever it happens, before any test starts included.
  It is blind to any loop that keeps doing IPC.
- The ceiling catches any test that does not end, livelock included, however busy it looks. It
  cannot fire faster than the budget, so it is a backstop rather than a diagnostic.
- Neither can tell a livelock from slow but correct work while it runs. Only the budget separates
  them, and a budget is a person's declaration of expected cost.
- The heartbeat credits work by any thread, leftovers from earlier tests included, so a test whose
  server died can look busy. The kernel cannot attribute a thread to a test. Read the dump's
  address-space roots, not its program counters, to tell a leftover spinner from the process under
  test.
- A ceiling failure reports the ceiling, not the cost. "Ran 900 s" against a 900 s budget says
  nothing about how long the work would take.
- Both depend on the boot core's timer interrupt. The backstop for losing it is `xtask`'s host
  watchdog (`time_record::HOST_GRACE_SECS`, notes/scheduler.md), which kills QEMU at the test's
  budget plus a grace. The ceiling still fires first, and its dump says more.

The per-test budgets live in `crates/test_times`, because `xtask`'s host watchdog reads the same
numbers. The livelock probe (feature `watchdog_probe`) proves the ceiling. It has the shape of the
RedoxFS repeat-write livelock, which spun in an allocator commit while still serving block IPC. That
turned a loud 60-second trip into a silent hang at about 400% CPU.

## Skips

A test whose fixture is absent on this machine calls `skip!(reason)`. That is a third outcome
beside pass (return) and fail (panic), and milestone 145 (a test that needs hardware the boot
doesn't have can say so) added it when the suite first ran on the VisionFive 2. About thirty tests
correctly expected a device only `xtask`'s QEMU runners attach. `skip!` is a macro because only a
macro's `return` leaves the calling test; called from a nested helper, it returns from the helper.
The reason should name the missing thing, because it is the only record of why the test did not
run. The final line counts skips apart from passes, and nothing yet compares that count across runs.

A test that prints "skip" and then returns without a reason fails the run. Milestone 214 (a test
that prints "skipping" and returns is counted as passed) found eighty-odd tests printing and
returning, which every count read as a pass. The console checks each fragment printed while a
test runs, so the check fires on what was printed rather than on the source. It costs one
`str::contains` per fragment, in test builds only. A test that prints "skip" for another reason and
passes is a false positive, and the tree has none. A test that returns early having printed nothing
is invisible to it.

## Filtering and repeating

`NIFE_TEST_FILTER`, baked in by `kernel/build.rs`, selects tests whose full path contains it. It is
a compile-time constant because that is the one channel identical on all three architectures: two
boot with a device tree this kernel does not parse, and the third through PVH with none. A filter
change costs about 2.3 s of relink against about 53 s of aarch64 suite. A filter that selects
nothing fails the run, unless `xtask` is counting selections across several images in one leg.

A filtered run is not the suite. The frame ledger's ceiling, the thread peak and the stack
high-water are whole-suite numbers, so a one-test run cannot fail them. And a test that passes only
because an earlier one wired a service will fail alone, which is a true finding about the test.

`NIFE_TEST_REPEAT` repeats the filtered tests in one boot. It exists for a failure that shows once
in many boots on a bench where each boot costs a trip with a USB stick (milestone 321 (a corpse that
did not park, on four real cores)). The first failing iteration ends the boot, because continuing
would need the kernel to survive a failed invariant, and a second failure could be the first one's
wreckage. A repeat is refused without a filter, because the whole suite keeps services for the
boot. The baseline is the second iteration, since a first run may wire a service the boot keeps.
From there the runner projects the average loss over the iterations left, and refuses the run
before it would fail by running out, which would read exactly like the rare failure being counted.

## The frame ledger

A boot has one pool of physical frames, and no test accounts for what it took. That is how the
aarch64 suite once failed in whichever test spawned last, one run in three, with the wrong code
blamed three milestones running (notes/frames.md). The ledger keeps two numbers per test, because
only the second fails a boot: frames never given back, and the longest run still free. A suite can
hold 137 free frames with no run of 128.

- Each reading closes the previous test's account and opens the next, so the charges partition the
  run and every frame lost lands on exactly one test.
- `SUITE_PAGE_FRAME_BUDGET` caps what the suite may keep. [frame-budget.md](frame-budget.md) prices
  what is kept on purpose and holds the conventions and history of the number.
- `SUITE_MIN_FREE_RUN` is the floor on the longest free run at the end, because loading a program
  takes a contiguous run. `redoxfs_server` and `net_stack` are the largest programs the boot loads,
  a few hundred pages each with their page tables.
- The thread, region and rendezvous peaks and the frame low-water print beside the ledger. They
  report and do not gate: what fills those tables is mostly services kept on purpose, so a rising
  peak is usually a boot doing more. The region and rendezvous peaks and the low-water each name the
  test during which the mark moved, where to start reading. The region table once reached 252 of 256 in `timetable_tests`, from a leak many tests
  earlier. On 2026-09-26 no allocation was refused on any architecture, so a run that prints the
  refusal line has met the ceiling somewhere.

`RegionRun` names a root region's frames while it is alive. A frame-return test then asserts that
those frames came back, rather than bracket `memory::free_page_frames()` and claim that nothing
else in the machine allocated in the window. Two such brackets were proved wrong on CI (pull
requests #1101 and #1120, notes/load-sensitive-assertions.md).

## Waiting on ticks: `TickBudget`

A wall-clock deadline keeps running whether the guest executes or not, so on a contended host it
shrinks in the only currency a test cares about. A descheduled emulator misses deadlines, so fewer
ticks arrive over the same stretch, and a tick budget stretches under exactly those conditions.
`sched::tests::within_ticks` must not yield, since it waits for a preemption, and
`smp::tests::a_migrated_kernel_thread_keeps_its_hart_pointer` must, so the two share only the
arithmetic. Ticks are per core, so a change of core re-anchors the budget. A thread that migrates
repeatedly could stretch it without bound, which is why the per-test ceiling is the last line.

## The preemption window tests

`bench/spawn_map.rs`'s `map_new` masks interrupts across a timed window of about 250 µs against a
10 ms tick, because one preemption inside it once read as a 26.4% regression
([benchmarks/preemption-in-the-window.md](benchmarks/preemption-in-the-window.md)). The benchmark
build runs no tests, so `preemption_window_tests.rs` tests the primitive instead: masking holds the
tick back, and `restore` delivers it.

- The count is per core. The global `sched::preemptions()` once read nine preemptions inside a
  masked window, every one another core ticking its own thread.
- Each window ends only once the tick is architecturally pending. The emulator raises the timer
  from its own loop, as late as 86 ms after the deadline, so a window ended on the clock alone could
  hold nothing ([load-sensitive-assertions.md](load-sensitive-assertions.md) has the measurement).
- The tests spin on the wall clock rather than count instructions, so they hold under `-icount`
  and under HVF alike.
- `RAISE_BOUND_PERIODS` is a leak trap rather than a timing claim: a timer that never raises turns
  into a failure that says so rather than a hang.

## BUGS

- The repeat leak check sees frames and the runner's own capability slots, not every table. A test
  that leaks a rendezvous or a thread per run fails in whichever iteration runs out, under its own
  message. The per-iteration line prints the live thread count so the trend is on the page.
- Two test sites keep their endpoints for the boot on purpose: `pipeline_service`'s stub, a kernel
  thread that loops forever, and `a_user_client_moves_data_through_shared_memory`'s client, which
  spins. Reclaiming either would leave a thread spinning on a stale endpoint. Each takes its pages
  from a region of its own, so the cost is charged to that test.
- The kernel's own `user::*_service` wiring still uses the never-freed pool, much of it once per
  test (`fs_service`, `ntp_service`, `display_service` among others), so those tests move the ledger
  in steps of 32. Milestone 670 (test helpers give their clients a region to retype from) and
  milestone 671 (tests retype their rendezvous from their own region, so the registry stops filling)
  are that work.
