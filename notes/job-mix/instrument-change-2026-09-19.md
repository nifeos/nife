# What changed in the job-mix instrument on 2026-09-19

An appendix to [notes/job-mix.md](../job-mix.md), which summarizes it. It says how to tell an old
transcript from a new one, and why the statistic changed.

Two things changed at once, and each on its own would make old and new numbers incomparable.

1. The statistic. Each sweep point used to be the best of three repeats. It is now the
   median of 21, with the fastest and slowest beside it. The reason is the five radon boots
   of 2026-09-16. At `tasks=4` the best of three moved 29% between boots of one image, because the
   spread *within* a boot was 37%. A minimum drawn from a wide distribution is the statistic that
   moves most. Resampling those boots' own repeats, the median of 21 puts `tasks=4`'s boot-to-boot
   spread at about 4%, inside the band the stable points already had. [instrument.md](instrument.md) carries
   the table and milestone 168's block the method. Board time was the argument for varying the
   count by point, and it does not survive arithmetic: at 21 repeats the timed windows total
   seconds on radon.
2. The mix. Seven job kinds instead of five: `MAP` (user page mapping) and `SPAWN` (process
   creation) took one slot each from the four non-IPC kinds. See `crates/job_mix`'s header.

How the lines differ, so nobody has to diff a transcript by eye:

| | before 2026-09-19 | from 2026-09-19 |
|---|---|---|
| started line | `one job is one of 5 kinds` | `one job is one of 7 kinds` |
| sampling line | (none) | `job-mix: each point is the median of 21 repeats, ...` |
| point line | `job-mix: tasks=4 jobs=512 ticks=<best> jpm=<from best>` | `job-mix: tasks=4 jobs=512 repeats=21 ticks_min= ticks_median= ticks_max= jpm_median=` |
| repeat lines | three per point | 21 per point, same format |
| per-kind lines | (none) | `job-mix-kind: tasks=4 kind=spawn jobs= ticks= per_job= region_ticks=` |

The field names changed on purpose. `jpm=` and `ticks=` do not occur on a new point line, so a
script written against an old transcript finds nothing rather than silently reading a median as a
best. `jpm_median` is not comparable with an old `jpm` even at the same task count: different
statistic, different mix.

"Finds nothing" is the safer failure and it is still a silent one, which the tree learned the
same day (2026-09-19). Another session was building `crates/serial_console`'s sweep recognizer
against a capture from the old kernel. When this change landed, its parser went on matching the
line's head, read none of the four numbers, and reported zeros. Its tests stayed green, because the
fixture it asserted against had been made from the same old kernel and the two agreed with each
other. The fix re-captured the fixture from a current kernel and the limitation is recorded in
`crates/serial_console/src/progress.rs`'s `BUGS`: the markers are shared through `crates/job_mix`,
the field names inside the line are not.

## What a `job-mix-kind:` line says

It is summed for one sweep point over every released task and all 21 repeats. It says how many jobs
of that kind ran, the ticks they took (self-timed by each task, preemption included), and the
average per job. For `map` and `spawn` it adds the ticks spent inside `SPLIT` and `DESTROY`, which
take the kernel's one memory-region lock. That last column exists because the jobs
were first refused for fear they would measure the allocator rather than the scheduler; it lets a
reader see how much of them does. The kinds whose `per_job` grows fastest with `tasks` are the kernel
paths that get more expensive under load, which is the attribution a risk 4 verdict will want.
