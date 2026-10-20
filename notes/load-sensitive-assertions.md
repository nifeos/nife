# Load-sensitive assertions

*(Milestone 78 (the load-sensitive assertions) and milestone 62 (tests that assert on time). This
page is the register: how to recognize one, how to fix one, and every known site with its status.
Each fix's history, runs and injections are in the appendices listed at the end.)*

A load-sensitive assertion is a test that goes red because the host was busy, not because the
kernel was wrong. The guest runs under QEMU on a shared machine. When other emulators, lanes or CI
jobs compete for the cores, the guest's vCPUs are descheduled for milliseconds at a time. A test that measured the wrong thing then fails a pull request that could not have reached it.

The family was named on 2026-08-03, when five assertions had failed pull requests that changed no
executable code. A flaky red trains people to re-run rather than read. On
2026-08-17 a loaded run produced nine reds: eight from two known timer assertions, and one real
double free, wearing the same color
([the first loaded acceptance run](load-sensitive-assertions/first-loaded-acceptance-run.md)).

## How to recognize one

Ask these in order; each came from a site the earlier ones missed.

### 1. Which direction did it fail?

A slow machine produces a deficit, never a surplus. Host contention makes work happen late or not
yet, so it can only make a count lower than the test expected.

- A positive failure ("not yet": a thread has not run, a tick has not arrived) is honest load
  sensitivity. The fix is to wait on the property, with a clock or tick budget as the bound.
- A negative failure (more free frames than at the start, fewer threads than the baseline) is not
  a timeout. The assertion was written against something wider than the property, and state from
  outside the measured window tripped it. The usual source is a neighboring test's teardown
  landing late.

A negative failure cannot be fixed by a margin: widening the bound only hides the defect. That is
§61 (a lint is adopted on evidence), applied to assertions.

### 2. Is a yield count standing in for a duration?

A yield count is not a duration. Since §28 (SMP placement) scatters threads across cores, a yield on a core with an
empty run queue returns in microseconds. Fifty of them can elapse before the awaited thread has been
scheduled at all. The error runs both ways: a loaded host burns yields while another vCPU is
descheduled, and a native host under Hypervisor.framework burns them in nanoseconds
([notes/hvf-leg.md](hvf-leg.md)). The same holds in a user program. A retry loop over a refusal that
a timer tick has to clear cannot buy the tick with syscalls
([the caretaker teardown wait](load-sensitive-assertions/caretaker-teardown-wait.md)).

### 3. Does the window include instructions outside the property?

Some failures are positive and still wrong. The test measures across instructions that are not part
of the claim. Examples: a tick counted between the read and the lock, a probe's execution that
placement never promised, a spinner sampled before it had a turn. Contention does not cause these
races; it stretches the window, so a race that never lost on a quiet machine
loses on a shared runner
([the second round](load-sensitive-assertions/measurement-windows-and-the-load-recipe.md)).

A per-core counter read on both sides of a wait must name its core, or a migration silently changes
the subject. That is why `ticks_on(core)` and `missed_ticks_on(core)` exist on both ISAs.

### 4. What else lands in the band that makes it fire?

An assertion is worth keeping only if its firing band contains the defect and nothing else. Where a
defect and the host produce the same band, no threshold inside it separates them. The choice is then
between a flake and a false pass. From inside the guest, a 30 ms handler and a 30 ms deschedule are
the same observation. That sentence is why `the_handler_keeps_up_when_no_lock_is_held` was deleted
rather than re-cut ([the timer disposition](load-sensitive-assertions/timer-assertion-disposition.md)).

### 5. Is a count standing in for a mechanism?

The first four find assertions that fail when they should not. This one finds assertions that pass
when they should not. A frame count is a weak detector of a page-table leak: eight thread
stacks are 224 KiB against a 2 MiB table span. The reaper test missed the stack-reuse leak of
milestone 6 (threads, the context switch, and preemption) entirely, and an injection proved it
([stack reuse and proxy detectors](load-sensitive-assertions/stack-reuse-and-proxy-detectors.md)).

### What the harness tells you

A guest cannot know eleven other emulators shared its eight cores. So `xtask` samples the host's load average during each emulated leg and prints it when a leg goes red,
with the core count and the oversubscription factor
([the host-load line](load-sensitive-assertions/host-load-line.md)). Read it before theorizing; it covers the host-side checks (`inbound`, `scanout`) too.

## The rule for fixing one

1. Wait on the property, not on a proxy for it. Bound the wait with `smp.rs`'s `wait_for` (a
   clock) or with `testing::TickBudget` (delivered ticks). Assert that the wait succeeded.
2. Ask about the object, not the machine. A thread is asked about by its generational `ThreadId`
   (`is_thread_present`). A region is asked about by `memory_region::usage`, or by its own frames
   through `testing::RegionRun`. A single frame is asked about through `memory::is_page_frame_used`.
   A held tick is asked about through `timer::is_tick_pending`
   ([preemption window](load-sensitive-assertions/preemption-window-tick.md)).
3. Denominate a budget in what the guest actually received. A descheduled emulator delivers fewer
   ticks per second, so a tick budget stretches under exactly the load that broke a counter
   deadline. A user program has no tick reading yet, so it uses a clock with a measured margin.
4. Never widen a bound to make it pass. If the claim cannot be attributed from inside the guest,
   move it to an instrument where the host is not a term (`script/icount`), or delete it.
5. Print the observation the predicate decided on. A panic that re-samples a global count can print
   an impossible quantity, and that is the message someone reads at 2 a.m.
6. Prove the fix by injection, and prove the giving-up path too. Aim the injection inside the
   test's own window: a whole-machine defect kills the suite before the target test runs. An
   injection that fires shows only that the assertion can fail, not that it fails for the right
   reason.
7. Then run it under load. Two clean matrices proved nothing about the sites that failed on a
   loaded third.

## The instruments

- `script/repeat-under-load [-n runs] [-s spinners]` runs the suite repeatedly with one busy loop
  per core, recording elapsed time, load average and how many QEMUs were up
  ([notes/scripts/load-and-reliability.md](scripts/load-and-reliability.md)). It surfaces a problem well and characterizes one badly: its
  "1 in 45" for the double free was a sighting, and the bug was a deterministic ownership defect.
  A neighboring emulator predicted reds better than the load average did.
- `script/icount` boots under `-icount shift=0,sleep=off`, where virtual time advances only when the
  guest retires instructions. It asserts arrival, whole-handler cost, zero missed ticks and the
  re-arm grid law, all in instructions. It is `-smp 1`, so it cannot host a cross-core claim. See
  [notes/instruction-clock.md](instruction-clock.md) and
  [the icount claims](load-sensitive-assertions/the-icount-claims.md).
- The load recipe: one spinner per core, then `script/cpu-matrix`, which puts an eight-core machine
  at a load average of about 22. Never run it while other lanes are gating here.

## The register

Status is open (known and not fixed), narrowed (the exposure is smaller and a named residual
remains) or fixed. A fixed site can still carry a residual in BUGS below.

| assertion | file | status | what was done, or what is left | appendix |
|---|---|---|---|---|
| five userspace retry loops over the refusal of §16 (object revocation) | `system_initializer`, `login.rs`, `swish.rs`, `job_undertaker.rs`, `timetable.rs` | open | a yield count waiting on a timer tick; two trap on exhaustion. Milestone 185 (sweep userspace's bounded retry loops onto a clock), not started | [caretaker teardown](load-sensitive-assertions/caretaker-teardown-wait.md) |
| boot self-test `budget` | `self_test.rs` | narrowed | the check before it waits for its own reap; the window stays global | [budget's window](load-sensitive-assertions/boot-self-test-budget.md) |
| `a_process_spends_memory_region_and_the_kernel_never_allocates` | `system_tests/src/user/elf_child_builder_tests.rs` | open | a global `used()` equality across a process run, which the `free_page_frames()` sweep did not read. Never seen red, though its boot twin `budget` has; found by reading on 2026-09-24 | [budget's window](load-sensitive-assertions/boot-self-test-budget.md) |
| `a_user_program_that_never_yields_is_preempted_anyway` | `user/tests.rs` | open | a wall-clock window with no wait; needs all four cores descheduled; never seen red | [known residuals](load-sensitive-assertions/known-residuals.md) |
| `every_secondary_runs_scheduled_work` | `smp.rs` | open | indexes by the core a probe ran on. If it fails, take the placement probe's fix | [known residuals](load-sensitive-assertions/known-residuals.md) |
| `a_finished_thread_is_reaped_and_its_memory_returned` | `sched.rs` | narrowed | per-`ThreadId` reap waits, a waited `used() <= before` (08-03); a one-thread stack-reuse probe (08-17) | [first verdicts](load-sensitive-assertions/global-baselines-and-the-drift-law.md), [stack reuse](load-sensitive-assertions/stack-reuse-and-proxy-detectors.md) |
| `a_dead_user_thread_frees_its_whole_address_space` | `user/tests.rs` | narrowed | per-`ThreadId` reap waits and a waited `used() <= before` (08-03) | [first verdicts](load-sensitive-assertions/global-baselines-and-the-drift-law.md) |
| `kernel_stacks_do_not_touch_the_frame_allocator_in_steady_state` | `sched.rs` | narrowed | each spawn followed to its own reap; a waited `free() >= free_before` (08-16) | [miss taxonomy](load-sensitive-assertions/miss-taxonomy-and-clockless-loops.md) |
| `caretaker_teardown_reclaims_a_full_session_worth_of_memory` | `user/login_tests.rs`, `fixtures/src/login_test_client.rs` | narrowed | `destroy_with_retry` waits on the region under a 5 s clock ceiling, 40x the worst wait (08-28) | [caretaker teardown](load-sensitive-assertions/caretaker-teardown-wait.md) |
| `ticks_arrive_at_the_configured_rate` | both `timer.rs` | fixed | asserts the re-arm grid law (08-03); an exhausted retry budget prints `UNMEASURED` (08-18) | [first verdicts](load-sensitive-assertions/global-baselines-and-the-drift-law.md), [timer disposition](load-sensitive-assertions/timer-assertion-disposition.md) |
| `the_handler_keeps_up_when_no_lock_is_held` | both `timer.rs` | fixed | deleted 2026-08-18; `script/icount` makes the claim | [miss taxonomy](load-sensitive-assertions/miss-taxonomy-and-clockless-loops.md), [timer disposition](load-sensitive-assertions/timer-assertion-disposition.md) |
| `holding_a_lock_masks_the_timer`, the masking window | both `timer.rs` | fixed | both reads moved inside the critical section; `ticks_on(core)` (08-04) | [second round](load-sensitive-assertions/measurement-windows-and-the-load-recipe.md) |
| `holding_a_lock_masks_the_timer`, liveness and release; `the_timer_is_ticking` | both `timer.rs` | fixed | wait for the pending bit, or the property (09-24) | [preemption window](load-sensitive-assertions/preemption-window-tick.md) |
| `unmasking_delivers_the_tick_that_was_held`, `a_masked_window_takes_no_preemption` | `preemption_window_tests.rs` | fixed | waits for `timer::is_tick_pending`; QEMU raised ticks 86 ms late (09-24) | [preemption window](load-sensitive-assertions/preemption-window-tick.md) |
| `scanout check` (host referee) | `xtask/src/scanout.rs` | fixed | QEMU's monitor refused its dumps; one connection now (10-12) | [referee](framebuffer-contract/scanout-referee.md) |
| `inbound check` (host prober) | `xtask/src/inbound.rs` | fixed | an `EINTR` read dropped a held connection, losing its round (09-24) | [inbound EINTR](load-sensitive-assertions/inbound-eintr.md) |
| `work_can_be_placed_on_every_cpu` | `smp.rs` | fixed | asserts arrival at the named core (`PerCpu::adopted`), not execution (08-04) | [second round](load-sensitive-assertions/measurement-windows-and-the-load-recipe.md) |
| `a_thread_that_never_yields_is_preempted_anyway` | `sched.rs` | fixed | the spinner is waited on, not sampled; the budget is 200 delivered ticks (08-04) | [second round](load-sensitive-assertions/measurement-windows-and-the-load-recipe.md) |
| `a_sender_blocks_until_a_receiver_arrives`, `other_threads_run_while_one_is_blocked` | `sched.rs` | fixed | five yield-count waits became `wait_for` (08-04) | [second round](load-sensitive-assertions/measurement-windows-and-the-load-recipe.md) |
| `threads_round_robin` | `sched.rs` | fixed | waits for every counter above zero, then for its own reaps (08-03) | [first verdicts](load-sensitive-assertions/global-baselines-and-the-drift-law.md) |
| five sites found on the physical core, milestone 81 (an HVF leg) | `sched.rs` (3), `user/reap_tests.rs`, `user/supervision_tests.rs` | fixed | yield counts became waits on the property (08-04) | [notes/hvf-leg.md](hvf-leg.md) |
| `reclaim_frees_an_embryo_tcbs_region` | `sched.rs` | fixed | `is_thread_present(tid)` in place of a global headcount (08-16) | [miss taxonomy](load-sensitive-assertions/miss-taxonomy-and-clockless-loops.md) |
| `a_migrated_kernel_thread_keeps_its_hart_pointer` | `smp.rs` | fixed | the drain budget is 200 delivered ticks through `testing::TickBudget` (08-18) | [migration drain](load-sensitive-assertions/migration-drain-tick-budget.md) |
| `a_userspace_driver_reads_a_file_over_the_pcie_transport`, and three siblings | `user/tests.rs`, `user/riscv_virtio_tests.rs` | fixed | the baseline moved before `start_pci`; x86_64's `ROUTED_IRQS` stopped counting timer ticks (09-04) | [PCIe interrupt counter](load-sensitive-assertions/x86-pci-express-interrupt-counter.md) |
| `run_swap`, "returned 277 of 224 pages" | `user/live_swap_tests.rs` | fixed | the region's absence after reclaim is the measurement (09-22, #1101) | [unowned reds](load-sensitive-assertions/unowned-reds.md) |
| `the_page_is_returned_when_the_space_is_dropped` | `user/current_cpu_tests.rs` | fixed | asks the frame's own allocator bit (09-23) | [unowned reds](load-sensitive-assertions/unowned-reds.md) |
| 35 assertions bracketing `free_page_frames()` | `sched.rs`, `user/force_kill_tests.rs`, `user/tests.rs`, `user/cpu_time_tests.rs` | fixed | `testing::RegionRun` asks the region's own frames (09-23) | [free-page-frames sweep](load-sensitive-assertions/free-page-frames-sweep.md) |
| frame hygiene | `user/live_swap_tests.rs` | fixed | removed on 2026-08-03 (#46); the property was already asserted twelve lines above | [first verdicts](load-sensitive-assertions/global-baselines-and-the-drift-law.md) |

Milestone 62's acceptance evidence is two repeat counts on one laptop under eight spinners. On
2026-08-17, 36 of 45 loaded runs were green, and the reds were the two timer assertions plus the
double free. On 2026-08-22, after the disposition and the drain fix, 45 of 45 were green at a peak
load of 90 ([the confirmation run](load-sensitive-assertions/confirmation-run.md)).

### Where to look for the next one

Milestone 78's scope note counted 39 kernel sites of this shape, never audited as a set. Four greps
cover everything the rounds found by reading:

1. a global count taken as a baseline: `thread_count()`, `free_page_frames()`, `memory::stats()`;
2. a loop with no clock in it;
3. a frame count standing in for a mechanism;
4. a bounded retry count in a user program, over a syscall that a timer tick has to clear.

The 2026-09-23 sweep closed the first grep for `free_page_frames()` only. On 2026-09-24,
`memory::stats()` still had 22 matches in `kernel/src` and `thread_count()` had 36, and neither has
been read against these questions.

## BUGS

- Three frame bounds are still global and one-way (the reaper, address-space and kernel-stack
  tests). A one-shot leak of `k` frames passes if a neighbor frees `k` in the same window; a
  persistent leak still fails. [Known residuals](load-sensitive-assertions/known-residuals.md) has
  this and the stack-reuse probe's two residuals.
- `ticks_arrive_at_the_configured_rate` can go unmeasured on a loaded host. It did on 9 of 36 legs on
  2026-08-18 and on 0 of 90 on 2026-08-22. A shorter window would be sound, since the law fails on
  the first drifting tick. It was refused for want of a rate measured on more than one host.
- `script/icount` is `-smp 1`. The tick path is lock-free on both ISAs today, which is why deleting
  the handler-latency assertion was safe. If the tick path ever takes a contended lock, neither
  instrument would notice.
- `work_can_be_placed_on_every_cpu` no longer proves that a specific core ran a specific thread.
  Nothing does, because placement is a hint and §28 deferred pins.
- `ROUTED_IRQS` is a global count. The PCIe test proves some interrupt became some driver's message,
  and it is sound only while no second driver runs in the window. A per-intid delivery count wants
  the three arch handlers to route through one portable call.
- `testing::RegionRun` works only for a root region, and nothing in the type stops a caller using
  it on a child. It reads one frame at a time, so it is not atomic against another core.
- The migration drain loses coverage quietly under load: each worker's lifetime is still counter
  time. And a `TickBudget` that re-anchors on every check would never expire; the harness's per-test
  ceiling is the backstop ([migration drain](load-sensitive-assertions/migration-drain-tick-budget.md)).
- A user program cannot read delivered ticks, so `destroy_with_retry` is safe by margin, not by
  unit. Giving a process a tick reading is an ABI addition, which is an architect's call.
- The host-side post-run checks are emulator-dependent, and naming the emulator does not make them
  less so: on one tree, apt's QEMU 8.2.2 and the pinned 11.0.2 fail disjoint sets of them. Since
  milestone 414 (a red post-run check does not say which emulator produced it) the output carries
  the version, once in the per-architecture banner and on every red check line. Read that version
  before reading a red post-run check as this family or as a regression.
- The host-load line samples only while a leg runs and lands after the leg, not beside the panic
  ([the host-load line](load-sensitive-assertions/host-load-line.md)).

## Appendices

Each appendix verifies a row or a rule above. Every dated section an older citation names is in one
of them, so the citation still resolves.

| appendix | what it verifies | dated sections it holds |
|---|---|---|
| [global-baselines-and-the-drift-law](load-sensitive-assertions/global-baselines-and-the-drift-law.md) | the first verdicts and the direction diagnostic | "The diagnostic that sorts the family", "The verdicts", "Postscript: a fast machine finds the same family" |
| [host-load-line](load-sensitive-assertions/host-load-line.md) | the harness's load report | "The harness now says whether the host was loaded (2026-08-18)" |
| [measurement-windows-and-the-load-recipe](load-sensitive-assertions/measurement-windows-and-the-load-recipe.md) | the window diagnostic and the load recipe | "The second round, 2026-08-04", "The instrument that was missing", "The handler-latency twins", "Recommended here, not built here" |
| [known-residuals](load-sensitive-assertions/known-residuals.md) | the old BUGS list, with its corrections | the former "BUGS" section |
| [miss-taxonomy-and-clockless-loops](load-sensitive-assertions/miss-taxonomy-and-clockless-loops.md) | the handler taxonomy, the embryo and kernel-stack fixes | "The fourth round, 2026-08-16", the 2026-08-15 aarch64 taxonomy |
| [stack-reuse-and-proxy-detectors](load-sensitive-assertions/stack-reuse-and-proxy-detectors.md) | a frame count blind to a stack leak, nine injections | "The fifth round, 2026-08-17" |
| [the-icount-claims](load-sensitive-assertions/the-icount-claims.md) | the timer claims in instructions | "The sixth round, 2026-08-17" |
| [first-loaded-acceptance-run](load-sensitive-assertions/first-loaded-acceptance-run.md) | 45 loaded runs, and the double free | "The acceptance run, 2026-08-17" |
| [timer-assertion-disposition](load-sensitive-assertions/timer-assertion-disposition.md) | the band diagnostic, the deletion, claim 4 | "The disposition, 2026-08-18" |
| [migration-drain-tick-budget](load-sensitive-assertions/migration-drain-tick-budget.md) | a drain budget in delivered ticks | "The migration drain, 2026-08-18" |
| [confirmation-run](load-sensitive-assertions/confirmation-run.md) | 45 of 45 green | "The confirmation run, 2026-08-22" |
| [caretaker-teardown-wait](load-sensitive-assertions/caretaker-teardown-wait.md) | a user-program retry loop, and five siblings | "The fifth round, 2026-08-27", "The disposition, 2026-08-28", "The measurement, and the number that settles it" |
| [unowned-reds](load-sensitive-assertions/unowned-reds.md) | `run_swap`, the current-cpu frame, the liveness check | "Two unowned reds, recorded rather than chased: 2026-08-27" |
| [x86-pci-express-interrupt-counter](load-sensitive-assertions/x86-pci-express-interrupt-counter.md) | a counter sampled late | "A counter that could not have been late, only sampled late: 2026-09-04" |
| [free-page-frames-sweep](load-sensitive-assertions/free-page-frames-sweep.md) | 46 reads of the global counter | "The sweep, 2026-09-23" |
| [preemption-window-tick](load-sensitive-assertions/preemption-window-tick.md) | a tick raised late, read as one dropped | "A tick not yet raised: 2026-09-24" |
| [inbound-eintr](load-sensitive-assertions/inbound-eintr.md) | a signal read as a dead connection | "An interrupted read: 2026-09-24" |
| [boot-self-test-budget](load-sensitive-assertions/boot-self-test-budget.md) | a neighbor's teardown in a global window | "A neighbor's teardown: 2026-10-20" |

## See also

- [design/roadmap/0078-load-sensitive-assertions.md](../design/roadmap/0078-load-sensitive-assertions.md):
  milestone 78's spec and its evidence table
- [design/roadmap/0062-time-sensitive-tests.md](../design/roadmap/0062-time-sensitive-tests.md):
  milestone 62, the acceptance standard
- [notes/instruction-clock.md](instruction-clock.md): the icount instrument and its four claims
- [notes/cpu-models.md](cpu-models.md) BUGS: the load-sensitivity evidence, including the control
  model failing
- [notes/live-replacement.md](live-replacement.md) BUGS: the frame-hygiene analysis, the template
- [notes/riscv-parity-scope.md](riscv-parity-scope.md): where the shape was first named
- [notes/benchmarks.md](benchmarks.md): the two instruments, and why icount cannot host cross-core
  tests
