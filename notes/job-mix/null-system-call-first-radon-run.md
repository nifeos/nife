# The first radon run of the reaper fix

*An appendix of [the null-system-call note](null-system-call-under-load.md), moved out of it unchanged on
2026-10-05 (UTC) to keep the note under §212 (a prose budget)'s cap. Written 2026-10-04 by
`lane/null-system-call-contention`. The file name is provisional (see the README).*

## What TCG cannot say, plainly

- **The `null_system_call` job itself shrank less than its lock wait did under TCG.** After the fix it
  read 671 / 1,125 ticks at one and four tasks: the growth fell from about 600 to about 450, where
  the wait alone fell by about 440 ticks a job. Under TCG a remote fence stops every vCPU whether or
  not anyone holds a lock, and TCG's own costs rise with busy vCPUs, so the rest is not evidence about
  radon either way. The host was also loaded by other lanes for part of the session, and a later run
  read `compute` at one task as 555 rather than 330; it is not used.
- x86_64's TCG throughput roughly doubled (`jpm_median` 293,839 to 561,875 at four tasks, 316,469
  to 751,072 at 32, one run each), because TCG's NMI shootdowns are very expensive. That is a
  property of the emulator, not a claim about xenon.
- aarch64's job mix wedged under TCG on `main` too, so it gave no comparison. The cause was a
  miscompiled yield, not this change ([`spawn-job-miscompiled-yield.md`](spawn-job-miscompiled-yield.md)).

## The radon run that decided it

Two payloads, built from this branch's tip, so no build happens at the bench:

```sh
cd <worktree root>/null-system-call-contention
script/board-netboot --root target/board-fix          # boots 1 to 3: the kernel that ships
script/board-netboot --root target/board-lock-wait    # boots 4 and 5: the same plus lock_wait
```

Power-cycle radon's own outlet for each boot, and capture each boot with step 2 of
[`notes/job-mix.md`](../job-mix.md)'s procedure. Never another outlet, never a USB hub. Steps 3 and 5
apply unchanged. Then read, in this order:

1. `null_system_call`'s `per_job` at 1 and 4 tasks on boots 1 to 3, against 108 and 202 from 2026-10-04
   (every boot then read 195 to 207 at four).
2. On boots 4 and 5, `job-mix-lock: ... site=current_cap`: `wait_ticks / calls` at 4 tasks, in ticks
   per trap, against the job's per-trap growth `(per_job(4) - per_job(1)) / 64`.
3. `site=reap`: `stack_free_ticks / reaps` is what the lock was held for per reap, on
   silicon, for the first time.

What each outcome means, written before the numbers exist:

| Boots 1 to 3 at 4 tasks | Reading |
|---|---|
| 130 or below | the excess was this defect; the remainder is in line with `compute`'s preemption share |
| 130 to 180 | the defect was part of it; boots 4 and 5's residual wait says how much of the rest is the IPC path's own contention |
| 180 or above | this was not radon's cause. Boots 4 and 5's `current_cap` wait says whether it is still the lock. If that wait is small, suspect the line traffic the counters cannot see (BUGS) |

