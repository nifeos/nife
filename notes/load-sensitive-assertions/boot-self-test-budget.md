# A neighbor's teardown: 2026-10-20

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Written by the `maintainer/budget-selftest-flake` lane.)*

## The red

The x86_64 leg of `boot-check` failed the boot self-test `budget` on pull request #2472, which
changed notes and evidence logs only (run 39497146680, job 118885356612, 2026-10-20 21:09 UTC):

```
boot-check (x86_64): nife self-test: 9 of 13 passed, 3 absent: blockdev clock trng, 1 FAILED: budget
```

The same branch passed at 20:31, and every other `boot-check` around it passed. The job log carries
only the verdict line, so the check's own line, with the frame counts it compared, was lost.

## Which branch of the check failed

`budget` fails three ways: the process did not report within 2 s, it mapped nothing, or the
machine's used-frame count moved between the process's two reports.

- **Not the deadline.** The red verdict arrived 0.8 s into the boot (`7210 bytes in 0.8s`). A green
  local boot reaches its verdict at 0.69 to 0.70 s. The 2 s wait cannot have expired.
- **A detail line the length of a green one.** A green boot's verdict ends at byte 7197. The red
  verdict line is 17 bytes longer, so 7210 fits a `budget` line within a byte or two of the green
  one (`kernel frames used A -> B`). The timeout line is 44 bytes shorter and the "outlived its
  kill" suffix 22 longer; neither fits.

So the count moved, or the mapping count read zero. The register's first question applies: a count
moving under a process that maps only from its own region is state from outside the window.

## What was in the window

`userspace`, the check before `budget`, returned as soon as the fault counter moved. A temporary
print at its end and at `budget`'s start, 24 boots of the debug kernel at 12 concurrent emulators
on patagonia (QEMU TCG, two cores, load average about 19):

| at `userspace`'s return | boots | used frames |
|---|---|---|
| the killed `outlaw` thread still present | 22 | 5468 |
| already reaped | 2 | 5447 |

The teardown returns 21 frames, on whichever core the thread is on, at a moment set by how the two
vCPUs interleave. Usually it finishes while `budget` is still loading its process, before the first
report. When it lands between the two reports, `budget` reads the kernel as having freed frames
during the run.

Load alone did not reproduce the red: 10 serial boots and 36 at 12 concurrent all read
`5523 -> 5523`. The window is narrow, and the interleaving rather than the load average decides it.

## The fix, and what it was measured against

`userspace` waits for its thread to be reaped (`is_thread_present`, rule 1) and fails by name if
the thread outlives 2 s. 36 boots at 12 concurrent: every one returned with the thread gone and the
count at 5447, and every `budget` read `5524 -> 5524`. `script/boot-check` was green on all three
architectures. The "not reaped within 2s" failure path was not exercised by injection.

`boot-check` uploads `target/boot-check-*.log` when it fails, so the next red keeps the check's own
line.

## What is left

`budget` still compares the machine's used-frame count. Its claim, that the kernel allocates
nothing for the process, has no single object to ask, so rule 2 has no fit. It is sound while every
earlier check leaves no teardown in flight; `kernel/src/self_test.rs`'s BUGS says so where a new
check's author meets it, and nothing enforces it.
