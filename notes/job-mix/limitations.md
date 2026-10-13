# The job mix: the BUGS list in full

An appendix to [notes/job-mix.md](../job-mix.md), whose BUGS section lists each entry in one line.

- The sweep has no wall-clock heartbeat, so a watcher's wedge timer is a guess with headroom.
  This entry replaces *"`crates/board_console` has no recognizer for this run"*, which was true when
  the page was written and stopped being true on 2026-09-19 (milestone 324 (the bench console cannot speak to any of the three boards) part 2). The
  recognizer has `Stage::Sweep` and `Stage::SweepDone`, reading `crates/job_mix`'s own marker
  constants. `script/job-mix` judges with it and returns `script/board-console`'s five exit
  statuses. What that milestone could not fix is the thing that makes a sweep harder to watch than a
  soak. `kernel/src/soak.rs` prints every five seconds whatever the workload is doing, so a missed
  beat is a missed deadline. `kernel/src/job_mix.rs` prints only when a subrun ends. So the longest
  legitimate silence is the slowest subrun, and a watcher has to allow for it. The default is sixty
  seconds against a 4.0-second subrun measured under TCG, fifteen to one, and a board outside that
  margin reads as wedged when it is merely slow. `--quiet-after 0` is the escape and it gives up the
  detection. A heartbeat in the supervisor is the real fix and is a kernel change; milestone 324's
  block records it as follow-on.
- The trap still slows with busy cores, a little. On radon `null_syscall` grew 94 ticks a job from
  one task to four, then 48 after the reaper fix, then 10 after milestones 761 and 766. Its one-task
  level moves up to 8 ticks with code placement alone.
  [`null-syscall-under-load.md`](null-syscall-under-load.md) has every evening.
- There is no committed baseline and no `--check`. `script/bench` gates because its icount counts
  are deterministic; a sweep whose entire subject is scheduling under contention is not, on any
  accelerator this tree has. A gate here would be asserting a tolerance nobody has measured.
- The sweep tops out at 32 tasks, where milestone 134's E1 reaches 96 threads. E1's were kernel
  threads; these are processes, each with an address space and a loaded image, and the memory is
  what binds. If the knee is above 32 this instrument will not see it. The honest response is
  to record that rather than to raise the number blind. A `FAILED: could not spawn` line is the
  measurement that says how far it can go on a given board.
- The pool is built once and released in slices, where AIM7 forks a fresh set per subrun, so the
  parked tasks' kernel stacks exist during every subrun even though nothing touches them.
  `kernel/src/job_mix.rs`'s own header argues why that is the right choice for §96's question and why
  it is still a departure from AIM7.
- The mix proportions are chosen, not derived. AIM7 ships workfiles for four machine roles and
  nobody here has one for a capability microkernel. A different mix gives a different number, and no
  result from this instrument should be quoted without saying which mix produced it.
- The repeat count was sized from the five-job mix's distribution. On radon it is enough: the
  seven-job run of 2026-10-04 read every point within 1.67% across five boots, `tasks=4` within
  1.12% (the Results table in [notes/job-mix.md](../job-mix.md)). Step 6.1 of the procedure is the
  check on another machine, and if 21 is not enough the count is one constant (`job_mix::REPEATS`).
- `--hvf` boots (milestone 227 (a GICv3 driver)), and the full mix completes there
  ([`spawn-destroy-gone.md`](spawn-destroy-gone.md)).
- The supervisor is in the kernel, and that is what makes the build special (milestone 523 (moving the job-mix supervisor into userspace, and the five permissions it turns out to need)). A userspace supervisor would delete the whole
  class of problem a diverted boot has, and would measure the more honest thing. Its
  own releases and drains would be real `svc` round trips inside the timed window. And it would be
  one more process the scheduler has to place, which is precisely what DECISIONS §96 (process
  kernel or event kernel) asks the cost of. It is blocked on five decisions that are an architect's,
  and milestone 523's block prices each one. If it is ever built, the Results table in the parent needs a
  column saying which supervisor produced a row: same mix, same statistic, same field names,
  different timed window, so nothing in a transcript would look different.
- The parent page has not been followed end to end by its author, the same caveat
  `notes/bench-runbook.md` carries about every procedure it points at.
