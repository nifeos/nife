# 2. The timer wheel is the wrong question

*Name: provisional.*

Section 2 of [`notes/timed-wait.md`](../timed-wait.md) in full: three deadline structures modeled on the host, and why the idle tick
cannot tell them apart.

The block offers "a timer wheel or an ordered deadline list" as if those were the options. At
`MAX_THREADS = 128` and `TICK_HZ = 100` there is a third that beats both, and the reason is not
asymptotic.

Three candidates, modeled on the host over 200,000 ticks (2,000 seconds of kernel time) with a
deterministic operation count as the currency and wall clock beside it:

- A. No structure at all. A per-thread deadline word, plus one cached `earliest`. The tick
  compares `now` against `earliest`; only when that fires does anything walk the table.
- B. A sorted deadline list, intrusive and doubly linked, ordered by deadline. The head is the
  minimum, so the tick is one compare; insert walks to its position.
- C. A hashed timer wheel, 256 slots at one tick each (2.56 s of range) plus an overflow list
  rehomed at each wrap.

Comparisons per tick, averaged over the pass. `occ` is how many threads hold a deadline; `churn` is
the chance per tick that an armed wait is cancelled before it fires, which is the case that actually
dominates (an ACK arrives and the retransmit timer is thrown away):

| occ | churn | A/no-structure | B/sorted list | C/wheel |
|---|---|---|---|---|
| 0 | - | 1.00 | 1.00 | 1.00 |
| 1 | 90% | 1.95 | 2.90 | 5.50 |
| 4 | 90% | 4.99 | 16.39 | 19.01 |
| 16 | 90% | 18.06 | 162.21 | 73.02 |
| 64 | 90% | 75.33 | 2179.20 | 289.04 |
| 128 | 90% | 156.48 | 8435.52 | 577.04 |
| 16 | 10% | 6.76 | 31.41 | 9.34 |
| 128 | 10% | 149.69 | 1701.46 | 67.65 |

Three things fall out, and the first is the one that collapses part of the fork:

The idle tick is one comparison, and it is the same one comparison for all three. Every shape can
cache the earliest deadline in a word; the tick loads it, compares, and returns. That is the number
paid on every tick on every core whether or not anyone is waiting, and it does not distinguish
between a wheel, a list, and no structure at all. Measured over 100,000 ticks at occupancy zero: 1.000
comparisons and 0.000 writes for A and B, 1.004 for C (the wrap check). So "the scheduler carries
a timer wheel" is not a cost the fork has to weigh: whatever is chosen, the always-paid cost is
identical.

The sorted list is the worst option at every occupancy above one, which is the opposite of what
the block's "or an ordered deadline list" implies. Its tick is cheap and its *insert* is O(k), and
inserts outnumber expiries by the churn rate. At occ=128 it costs 8,435 comparisons per tick where
scanning costs 156.

Scanning wins until about 64 threads hold deadlines simultaneously, and the wheel wins past that.
The crossover is where it is because a scan is O(live threads) *per expiry event*, while a wheel
is O(1) per insert and per expiry. At a handful of deadline holders the scan's constant factor is
smaller than the wheel's bookkeeping. The five known consumers (net_stack's retransmit window,
`thread::sleep`, `Rendezvous::RECEIVE`'s no-timeout limitation, milestone 103 (`^C` stops spinning)'s `^C` watch, milestone
106's `Irq::WAIT`) are on the order of one deadline each.

The honest error bars. These are *modeled* operation counts, not machine instructions: the host
model counts comparisons and pointer writes at the points a kernel implementation would perform them,
which is load-independent and reproducible but is a model. Wall-clock ns/tick was recorded beside them
and is not reported as a number, because the machine was not quiet: another lane's `script/test` and
`script/lint` were running throughout and load average sat between 9 and 25. Two runs at different
loads produced the same ordering at every row, which is what the table is being read for.
