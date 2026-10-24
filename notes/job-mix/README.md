# The job mix: the appendices to notes/job-mix.md

[`notes/job-mix.md`](../job-mix.md) is the page to read: the instrument, the bench procedure and the
Results table. The parent is over its word budget, so a bench evening's full reading lives here and
the parent's Results row points to it.

- [`radon-2026-10-04.md`](radon-2026-10-04.md): five boots of the seven-job instrument on radon,
  read in step 6's order and judged against step 7.
- [`null-syscall-under-load.md`](null-syscall-under-load.md): why that evening's `null_syscall`
  nearly doubled from one busy core to four (the reaper freed kernel stacks under `IPC_TABLES`),
  the fix, and the radon procedure that will size it.
- [`null-syscall-first-radon-run.md`](null-syscall-first-radon-run.md): what TCG could not say about
  that fix, and the procedure radon sized it by.
- [`null-syscall-off-radon.md`](null-syscall-off-radon.md): the rest was the global lock on every
  capability lookup, measured under TCG and on four Apple cores under HVF (2026-10-05).
- [`null-syscall-hvf-full-mix.md`](null-syscall-hvf-full-mix.md): the same, spawning included.
- [`radon-2026-10-05.md`](radon-2026-10-05.md): milestones 761 and 766 on radon, and a bisect
  that found the single crossing moves about 8 ticks with code placement alone.
- [`spawn-job-miscompiled-yield.md`](spawn-job-miscompiled-yield.md): why the full mix failed under HVF. A yield
  wrapper declared no output. The spawn job then trapped with stale arguments.
- [`warton-aim7.md`](warton-aim7.md): what Warton ran AIM7 on, and why the 20% is not a target.
- [`instrument-change-2026-09-19.md`](instrument-change-2026-09-19.md): the statistic and the mix
  that changed that day, and how to tell an old transcript from a new one.
- [`hvf-cross-check-2026-09-19.md`](hvf-cross-check-2026-09-19.md): the HVF cross-check that did
  not run.
- [`supervisor.md`](supervisor.md): the kernel supervisor's shape, its pool and its own BUGS.
- [`limitations.md`](limitations.md): the parent's BUGS list in full.

*Name: provisional, for the directory and every stem in it. Naming is an architect's;
`script/names --unratified` lists each stem. Each stem is named for its content, and
`instrument-change-2026-09-19` is dated like `radon-2026-10-04`.*
