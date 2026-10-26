# Known residuals: the BUGS list as it stood through 2026-08-18

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

This was the old note's BUGS section, kept with its dated corrections because they are the record of
how each residual was found and closed. The main page's BUGS section carries what is still open.

## BUGS

- A coincidence can mask the `<=` frame assertions. A real leak of `k` frames passes if a neighbor's
  late teardown frees at least `k` frames inside the same window. The window is seconds wide and
  re-rolled every run. A persistent leak (every batch leaks, which is what the milestone 6 (threads,
  the context switch, and preemption) bug was) fails essentially every run regardless, so the trap
  still bites. But a one-shot coincidence pass is possible in a way `assert_eq` did not permit. The
  trade is deliberate. Equality bought that exactness by also asserting that the rest of the machine
  held still, which is false on any loaded run and was producing red CI on documentation PRs.
- `the_page_is_returned_when_the_space_is_dropped` can still be failed by a neighbor, and it takes
  an allocation rather than a free. Asking `is_page_frame_used` about one named frame is immune to
  the freeing neighbor that produced every observed failure of the old global form. But a neighbor
  that allocated this exact frame between the drop and the check would read as a leak. It is a
  microsecond window against one frame out of the machine's free set. The old form was a
  seconds-wide window against every frame in the machine. The two failure modes do not overlap: this
  one needs an allocation that the tests, running sequentially on the boot thread, do not make. It
  is recorded rather than closed. Closing it needs the allocator to tell a test which frame it
  handed out next, which is machinery for an exposure nothing has hit.
- The drift test can still go red on a pathological host, by design. Eight consecutive
  quarter-second windows, each containing a missed tick, fail with a message naming the condition.
  That is rarer by orders than the old failure (one miss anywhere in a single fixed window). The
  message now says "host contention or a genuinely slow handler" instead of reporting drift the
  re-arm logic does not have.

  *Correction (2026-08-17): "rarer by orders" is wrong about a loaded developer laptop.*
  [The acceptance run](first-loaded-acceptance-run.md) fired this assertion four times in
  forty-five runs, twice per ISA. The machine had eight cores and a one-minute load average between
  26 and 63. "Pathological host" is doing work the word cannot do. The condition is an ordinary
  laptop with three lanes gating on it, which is this project's normal condition rather than its
  worst case. The rarity claim was relative to the assertion it replaced and was never measured
  against anything. It stays above because the comparison it makes is still true; the adjective is
  not.

  *Closed 2026-08-18 (milestone 62 (tests that assert on time)): it cannot go red any more, because
  that assertion is gone.* Exhausting the retry budget now prints an `UNMEASURED` line and returns
  without a verdict. The entry stays for two reasons. The two corrections above it record how an
  unmeasured adjective survived in a BUGS section for two weeks. And the new cost mirrors the old
  one: the law can now go unmeasured on a loaded host instead of going falsely red. See
  [the disposition](timer-assertion-disposition.md).
- The handler-latency assertions (`the_handler_keeps_up_when_no_lock_is_held`, both ISAs,
  missed-tick delta over a five-tick window) keep their wall-clock exposure and are not fixed here.
  The aarch64 one is in the milestone's evidence table ("left: 3, right: 2" in a quiet window) but
  outside this lane's five. A deschedule long enough to pass a deadline is counted as a miss, and
  the guest cannot tell that miss from a slow handler. `miss_detail` (added to the aarch64 timer for
  milestone 78 (the load-sensitive assertions)) records how late the re-arm was. That is the
  discriminator a fix would build on: a few hundred cycles late is a slow handler, and a whole
  period late is the emulator.

  *Second round, 2026-08-04:* they are core-scoped now, so a migration is no longer one of the ways
  they can lie. The verdict on the rest is in [the second round](measurement-windows-and-the-load-recipe.md):
  the fix is the icount instrument, and excusing deschedule-shaped misses would be a weaker claim
  rather than a re-aimed one.

  *Settled:* aarch64 took the taxonomy on 2026-08-15 and riscv64 on 2026-08-16. They were a day
  apart because the record said the riscv64 twin did not exist. Both now pass a deschedule-shaped
  miss loudly, with its numbers, and fail a slow handler. `script/icount` makes the
  instruction-count claim on both ISAs (2026-08-17). The handler is bounded deadline-to-re-armed at
  2,500 instructions, against a measured 1,056 on aarch64 and 900 on riscv64. It is a separate boot
  rather than a `#[test_case]`, and notes/instruction-clock.md says why.

  *Closed 2026-08-18 (milestone 62): both assertions are deleted.* The taxonomy did not merely leave
  a window; it inverted with severity. A handler slow by 2.5 tick periods passed it while printing
  "not this kernel's bug, not failed". The injections are in
  [the disposition](timer-assertion-disposition.md). The claim is `script/icount`'s alone now, and
  `script/ci-build` runs it.
- The taxonomy's threshold leaves a window on both ISAs, one tick period wide. `miss_detail` reports
  `now - next`, the lateness beyond the period already missed. So the cut at one interval classifies
  "one to two periods late" as a slow handler (red) and "two or more" as the emulator (pass). A host
  deschedule of between one and two tick periods therefore still fails, wearing the message that
  blames this kernel. That was measured, not reasoned: a probe holding a lock across two and a half
  periods produced a lateness of 0.83 of an interval (2026-08-16, in
  [the fourth round's table](miss-taxonomy-and-clockless-loops.md)). Widening the cut would trade
  the flake for a slow handler going unreported. That is the wrong trade while the honest fix is
  available and merely unbuilt: the icount instrument, where "the handler took fewer than N
  instructions" is not falsifiable by the host.

  The instrument was built on 2026-08-17 (`script/icount`). It does not close this entry so much as
  route around it. The ambiguous window is a property of a taxonomy that exists only because the
  host can deschedule the guest. On the instrument nothing can, so that boot asserts
  `missed_ticks == 0` with no taxonomy at all. The window survived only on the test path, where the
  taxonomy still lived. See notes/instruction-clock.md.

  *And on 2026-08-18 it stopped surviving anywhere.* Milestone 62 deleted the taxonomy from both
  ISAs rather than re-cutting it. The band in which it fires is exactly the band in which it cannot
  attribute what it saw.
- The panic message contradicted this note, and the message is what a newcomer reads. The third
  stranger run of milestone 117 (the stranger test), on 2026-08-18, reproduced both assertions
  independently. It ran on an eight-core laptop at a one-minute load average of 45 to 63, caused by
  other lanes gating in other worktrees. It spent about an hour reaching a conclusion this file
  already held. Two of its findings were additions rather than repetitions.

  First, `timer.rs`'s panic text told the reader that a re-arm late by less than one interval "is
  this kernel's bug". That is the claim the entry above corrects. A developer meeting the failure
  meets the sentence, not the note, and is sent into a correct handler looking for slowness that is
  not there. Second, the sibling test's eight-retry loop also exhausts under sustained contention
  (`ticks_arrive_at_the_configured_rate` went red twice at load 60). So retrying the window is not on
  its own the fix the taxonomy entry above leaves open. The run's tally was 2 red in 13 aarch64 legs
  plus 2 sibling reds. Against that, `script/icount` passed on both ISAs at load 46.77, with 1,056
  and 800 instructions against the 2,500 bound.

  The run's own recommendation was the cheapest thing nobody had built. The harness can sample
  `uptime` and print the load average beside a timing assertion's failure, which would make the
  failure diagnose itself in one line. It was recorded rather than fixed, per notes/stranger-test.md.

  *Both are now done, and this entry is kept because its measurement is the evidence.* The harness
  has printed the load average on any red kernel leg since 2026-08-18; see
  [the host-load line](host-load-line.md). The panic text was not reworded, which would have left a
  claim the code cannot support in a shorter sentence. Milestone 62 deleted the assertion carrying
  it on both ISAs, having measured that its true-positive band and its false-positive band are the
  same band.
- Scope was five sites, not 39. The roadmap's scope note counts 39 sites in 7 files matching the
  shape (`wait_for`, or assertions against `free_frames`, `thread_count`, `used()`). The other 34
  were not audited here; the diagnostic is the checklist for reading any of them. *(Rounds two,
  three and four took eight more between them, all found by reading rather than by waiting for a red
  run. The backlog is real, and it is smaller than 34.)*
- The concurrency confound that broke the first reuse assertion also reaches the frame assertion
  beside it. This was found by reasoning after the fact, not by a red run, so it is recorded as a
  fact rather than as a fix. Suppose the second batch's peak concurrency exceeds the first's. Then
  `NEXT_STACK_VA` legitimately bumps. If that bump straddles a 2 MiB boundary, a page table is
  legitimately built and `used()` legitimately sits above `before`. That is a false failure, from
  scheduling, with no leak. It is rare for the same reason the assertion is a weak detector. A couple
  of slots is 56 KiB against a 2 MiB span, so roughly 3% of bumps land on a boundary. And a bump
  needs the batch to be reaped later than the one before it. Both halves of that product would have
  to fire in the same run. Know this before reading a red run at this site as a leak.
- A frame count is a weak detector of a page-table leak, and the reaper test's is the measured case.
  Eight thread stacks are 224 KiB of address space against a 2 MiB L3 span, so a leak charged per
  2 MiB is usually invisible. The [fifth round](stack-reuse-and-proxy-detectors.md) proved it by
  deleting the VA push and watching the leg go green. The reuse assertion added beside it covers
  that defect directly, and the frame bound is now scoped to the leak it can see. The same question
  is unasked at every other site that infers a mechanism from a frame count. That is the third grep
  the fifth round adds to the reading order.
- The reuse assertion reads a global watermark, deliberately. What makes it safe is the unit rather
  than the direction. A one-way failure direction was not enough on its own: the first version had
  one and was still wrong, because the watermark bounds concurrency and the claim was about reuse.
  One thread is the unit because one thread cannot exceed a high-water mark eight deaths just set.
  Two residuals remain, and both need a coincidence rather than mere load.
  - A neighbor that drained `FREE_STACK_VAS` to empty between the watermark read and the probe's
    spawn would fail it falsely. Nothing else spawns during this test (tests run sequentially on the
    boot thread, and the other cores are idle), and it needs the list down to its last slot.
  - A neighbor bumping the watermark could mask a genuine reuse failure for one run. That mirrors
    the `<=` coincidence caveat above. The defect is per spawn and permanent, so it fails every other
    run regardless.
- The `>=` frame assertion in `kernel_stacks_do_not_touch_the_frame_allocator_in_steady_state`
  inherits the coincidence caveat above, in the other direction of the same trade. A real
  regression of `k` frames passes if a neighbor frees at least `k` inside the same window. The
  defect it guards (kernel stacks drawn from the frame allocator instead of the kernel budget) is
  per spawn and persistent, so six spawns fail it essentially every run regardless. A one-shot
  coincidence pass is possible in a way `assert_eq` did not permit, and the equality was demanding
  that the rest of the machine hold still.
- `work_can_be_placed_on_every_cpu` no longer proves that a specific core executed a specific
  thread, and nothing else does either. That is the second round's honest cost. The test asserts
  delivery to the named core exactly. Execution is asserted for every core, but over the population
  of threads rather than per placement. Closing it needs a pin, which §28 (SMP placement)
  deliberately deferred. While placement is a hint, "this thread ran on that core" is not a property
  the scheduler has, and a test asserting it is asserting a coincidence.
- Something other than the placement under test can in principle move the adoption counter. Serving
  a steal pushes into the requester's inbox, so a target that stole from a third core in the same
  window would also increment. The test closes that by construction, not by the counter being
  unambiguous: one placement in flight at a time, each followed to a reap, so nothing else is
  runnable to steal. A version that placed several at once would have to reason about this again.
- Two siblings were checked against the same question and left alone, as the milestone's scope note
  asks.
  - `user/tests.rs`, `a_user_program_that_never_yields_is_preempted_anyway`, spins a tenth of a
    second of counter time and then asserts `preemptions()` rose. That is a wall-clock window with
    no wait, so it is the family's shape. It survives because falsifying it needs the emulator
    descheduled across all four cores for the whole window. The `sched.rs` twin needed only an
    unlucky order on one. Nothing has ever been recorded against it.
  - `smp.rs`, `every_secondary_runs_scheduled_work`, indexes `RAN_ON` by the core a probe ran on:
    the indexing the placement probe had to abandon. It survives because each secondary's probe is
    spawned onto its own queue as that core's first act and exits at once. The window in which an
    idle neighbor could steal it is a few instructions, not a whole placement loop. If it ever does
    fail, suspect this first; the fix is the placement probe's.
