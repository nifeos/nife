# Watching a job-mix sweep: an example

An appendix to [`notes/board-console.md`](../board-console.md), holding its job-mix example.

Watching a job-mix sweep, which is the same recognizer under QEMU and is where it was proved
(milestone 324 part 2, 2026-09-19, aarch64 `virt` with four cores). The three endings, all real
runs:

```
$ cargo xtask job-mix --arch aarch64 --smp 4
...
job-mix: tasks=32 jobs=4096 repeats=21 ticks_min=228108401 ticks_median=233958411 \
  ticks_max=249234771 jpm_median=65652
job-mix: done

job-mix: reached job-mix sweep complete (15924 bytes in 164.9s)
job-mix: last point tasks=32 jobs=4096 repeats=21 ticks_min=228108401 \
  ticks_median=233958411 ticks_max=249234771 jpm_median=65652
$ echo $?
0

$ cargo xtask job-mix --arch aarch64 --smp 4 --quiet-after 1s
job-mix: went quiet after job-mix sweep running (8545 bytes in 13.7s)
job-mix: last point tasks=2 jobs=256 repeats=21 ticks_min=18753659 \
  ticks_median=20392064 ticks_max=24330391 jpm_median=47077
$ echo $?
2

$ cargo xtask job-mix --arch aarch64 --smp 4 --for 30s
job-mix: time ran out after job-mix sweep running (9437 bytes in 30.0s)
job-mix: last point tasks=2 jobs=256 repeats=21 ticks_min=19380785 \
  ticks_median=23557551 ticks_max=51605248 jpm_median=40751
$ echo $?
3
```

The point and summary lines are one line each on a terminal; they are folded here with a trailing
backslash, which is the only edit made to them.

The second is a wedge manufactured with a one-second quiet window rather than a real one: no sweep
has wedged on a board, and none has been watched on one. The jobs-per-minute figures are a draw and
not a result; `notes/job-mix.md` has why.

These three were re-run on 2026-09-19 after milestone 168 landed, and the numbers moved for a
reason worth knowing. The earlier transcripts here showed a whole sweep in 27.9 seconds and a
`--for 12s` cut at the third point. The sweep now takes 164.9 seconds, because each point is
twenty-one repeats rather than three and the mix has seven job kinds rather than five, so the same
wall-clock windows cut it far earlier. The commands did not change; the workload underneath them
did.

At a board the same question is `script/board-console --until sweep-done --for 30m`, and it returns
the same statuses because it is the same code.
