# A record that never reaches a verdict: the per-record limit

*Name: provisional.*

An appendix to [`notes/falsification.md`](../falsification.md), which carries the limits and the
override.

2026-09-24. #1156 landed a `nifefs` record, the length guard weakened to one block, whose replay
never finished. Once the guard admitted the harness's one concrete length, CBMC entered `parse`'s
entry loop, whose bound is a `count` read from symbolic bytes, and unrolled it without end. Nothing
stopped it but the job. The per-pull-request `falsify` job was cancelled at its 45 minutes, and the
weekly sweep (falsifications.yml run 36031723556) at its 120, after that one record had run for
1 h 44 min and every record after it had gone unswept.

So each replay now runs under a wall-clock limit, and one that runs out is `ERROR ... no
verdict`, never red and never a survivor. The sweep goes on to the next record and exits non-zero
at the end.

| record kind | limit | where the number comes from |
|---|---|---|
| Kani harness | 15 min | 1.8 times the slowest record measured |
| kernel test | 30 min | `ci.yml`'s bound on building the kernel and running the whole suite |

The Kani timings are the one full-tree sweep whose timestamps survived, the cancelled run above. It
holds 73 records before the hang, median 1.6 s, 90th percentile 8.2 s, and a tail entirely in
`machine_discovery` (116 s, 136 s, 497 s), each including the rebuild its patch forced on a hosted
runner. One record was slower and is not in that sample because the hang came first:
`package_archive::a_short_file_is_refused`, measured locally at 20 to 31 minutes, the same
unbounded-loop shape. It got the harness bound rather than a limit raised to fit it (#1243, red in
1.3 s since), because a record that takes half an hour is the problem, not the limit. Every verdict
line now prints its own time, so the next sweep is a better sample than this one was. The first
such sample, the tail package swept on patagonia under the new limit (2026-09-24): 16 records, all
red, slowest 287.5 s, then 82.6 s and 71.3 s, the other thirteen under 10 s.

#### Example

```
$ FALSIFICATIONS_RECORD_LIMIT=60 script/falsifications --sweep nifefs
==> nifefs::a_short_image_is_refused_not_indexed
    ERROR    no verdict within the 60 s limit; `cargo kani` and its CBMC was killed. ... (60.4 s)
==> nifefs::the_validation_implies_reads_slice_is_in_bounds
    red      the harness caught it (1.9 s)

2 swept, 0 survivors, 0 stale or broken, 1 without a verdict
  no verdict nifefs::a_short_image_is_refused_not_indexed (limit 60 s)
```

That is the limit's own falsification, run on patagonia against #1156's original record (exit 1).
`FALSIFICATIONS_RECORD_LIMIT` (seconds) overrides both limits.

The whole process group is killed, not the child. `cargo` is what the sweep starts; `cbmc` and
QEMU are grandchildren, and killing only `cargo` would leave a solver holding a core and gigabytes
after the sweep had moved on.

#### BUGS

- A wall clock on a shared machine can turn a slow record into a false `no verdict`. The
  headroom is 1.8 times one measured maximum, not a proven margin. A timeout on a busy laptop that
  does not reproduce in CI is load.
- **"Would have finished at minute 16" and "would never finish" are reported alike**, because
  neither is evidence. The fix for either is a faster falsification or a bound on the harness, which
  is what `nifefs::a_short_image_is_refused_not_indexed` got in #1239.
