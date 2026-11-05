# The interim check of 2026-08-18: an appendix to milestone 62

An appendix to [milestone 62 (tests that assert on time)](../0062-time-sensitive-tests.md), kept
apart under §212 (a prose budget). It is the first installment of evidence after the disposition,
the two reds it found, and what was still needed at the time to move the block off PARTIAL.

The first installment of the evidence was taken the same day as the disposition (`script/repeat-under-load -n 18 -s 8`,
tree `01474c8e`, full table in notes/load-sensitive-assertions.md):

| | 2026-08-17, before | 2026-08-18, after |
|---|---|---|
| runs / ISA legs | 45 / 90 | 18 / 36 |
| 1-min load average, min to peak | 26.1 to 63.0 | 16.2 to 116.6 |
| red at either dispositioned assertion | 8 legs | 0 |
| green runs | 36 of 45 | 16 of 18 |
| re-arm law reported `UNMEASURED` | n/a | 9 of 36 legs, 25% |

The load was harsher than the run that produced the finding. That is the direction that makes a
green result mean something. Both reds are a different assertion, the one in
`a_migrated_kernel_thread_keeps_its_hart_pointer` in `kernel/src/smp.rs`. It is a new member of the family this block is
about: it gives 60 migration workers two seconds of counter time to drain, and 59 arrived. Its fix
is this block's own prescription, a budget in delivered guest ticks rather than in counter time
(`sched::within_ticks` exists), and it wants a lane.

What was still needed to move this block off PARTIAL, at the time this was written. Eighteen runs
is not forty-five. The 25% figure was the one that wanted the bigger count. It was the cost this
disposition introduced, higher than a reader would assume, and it came from one host. The candidate
fix was identified and deliberately not taken here. The quarter-second measurement window is far
longer than the law needs. Shortening it is not a wider bound, because the `assert_eq!` is untouched
and the defect fails on the first tick. It was not taken because it would have been a change
justified entirely by a rate resting on 18 runs. The confirmation run on the parent page is that
bigger count. It found the 25% figure did not survive it: zero of 90 legs reported `UNMEASURED` at
45 runs.
