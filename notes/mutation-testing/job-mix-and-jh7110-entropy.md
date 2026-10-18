# `job_mix` and `jh7110_entropy`

This appendix holds the `job_mix` and `jh7110_entropy` rows of the 2026-09-20 new-crate backlog
triage, whose table and method are in [new-crate-backlog](new-crate-backlog.md). The summary is in
[notes/mutation-testing.md](../mutation-testing.md).

### `job_mix`: 10 survivors, 6 killed, 3 equivalent, 1 recorded gap

Before: 41 caught, 9 missed, 1 timeout, 3 unviable (80.4% of viable). After: 38 caught, 4 missed,
7 unviable (90.5%). The caught count falls because four of the survivors became unviable. That is
the result rather than an accounting artifact: they no longer compile.

Five of the ten sat in a gap a compile-time assertion had left open. `TASK_BUDGET_PAGES` is
`1 + 8 + max(MAP_REGION_PAGES, CHILD_PAGES)`. The file already carried
`const _: () = assert!(TASK_BUDGET_PAGES > MAP_REGION_PAGES && TASK_BUDGET_PAGES > CHILD_PAGES);`
with a comment citing AGENTS.md's ladder. The assertion is true of 19, of 24 and of 144, so every
one of the five mutants compiled and passed. They were `1 * 8` for `1 + 8`, `9 * 16` for `9 + 16`,
and three ways of picking the *smaller* region. The last three leave a task running `MAP` nine
pages short of what the paragraph above the constant promises it.

The rung was right and the assertion was too weak. This is not an argument for a test: the two
bounds the sizing argument makes are still relations between constants in one file. They are now
written as such. The budget is at least `1 + 8 + MAP_REGION_PAGES` and at least
`1 + 8 + CHILD_PAGES`. It is strictly less than `1 + 8 + MAP_REGION_PAGES + CHILD_PAGES`, because a
task runs one job at a time and gives its region back. Four mutants stopped compiling.

Milestone 250 (an unviable mutant is a hole in the measurement that reads as a pass) applies here.
Moving four mutants from MISSED to unviable improves the rate partly by shrinking the denominator.
It is still the right move, because what now refuses them is the compiler. That is a stronger
checker than a test, and it runs on every build.

`order` keeps three survivors and gains one test, and the test is about the draw rather than the
output. The function takes the LCG's high bits because the low ones cycle short. Taking the low
ones makes every draw a multiple of 2^33, which is zero modulo every power-of-two swap index. The
last position then receives the mix's first job on every seed. Nothing else in the suite noticed:
`an_order_is_a_permutation_of_the_mix` still holds, and
`distinct_seeds_do_not_all_walk_the_mix_in_lockstep` still counts 90-odd differing orders.
`no_position_in_the_order_is_pinned_to_one_job` sweeps 512 seeds and asks that every job kind reach
every position.

The three that remain:

- `while i > 1` under `>= 1` (equivalent). The extra pass has `i` at 0, so `j` is `x % 1`, which is
  0, and the swap is `out.swap(0, 0)`. It adds one LCG step on a value nothing reads afterwards, and
  a self-swap.
- `MAP_REGION_PAGES > CHILD_PAGES` under `>=` (equivalent). `if a > b { a } else { b }` and
  `if a >= b { a } else { b }` differ only where `a == b`. There both branches yield the same value.
  This holds for every pair of operands, not merely for 16 and 10.
- `% (i + 1)` under `% i` (equivalent in contract, by argument rather than proof). The mutant is
  Sattolo's algorithm. It still produces a permutation, still deterministically from the seed, and
  still puts no two tasks in lockstep. That is the whole of what `order` is documented to promise.
  What it loses is uniformity over all `16!` permutations; it draws from the `15!` cyclic ones
  instead, and no task in this benchmark can tell. Re-check it if `order` is ever asked for a
  uniform shuffle, at which point it is a defect and not an equivalence.

The recorded gap is a finding about the code, not the tests. It is in a `BUGS` section on `order`,
where a reader meets the function. `seed | 1` exists to keep the LCG off its degenerate state. It
does that by discarding bit zero, so `order(2k)` and `order(2k + 1)` are the same permutation for
every `k`. Measured, by asserting it for eight pairs. Nothing in the tree hits it.
`kernel/src/job_mix.rs` hands every task "a distinct, odd, well-spread seed", and an odd seed passes
through `| 1` unchanged. A caller seeding tasks by consecutive index would put every adjacent pair in
lockstep, the one property this function exists to prevent. It is recorded rather than fixed,
because changing the arithmetic changes every permutation this function has ever produced. A
published benchmark number is a fact that has left the machine.

### `jh7110_entropy`: 23 survivors, 13 killed, 6 equivalent, 4 recorded gaps

Before: 59 caught, 19 missed, 4 timeouts, 3 unviable (72.0% of viable). After: 72 caught, 6
missed, 4 timeouts (87.8%).

Fifteen of the nineteen were a `<<` becoming a `>>` in a register bit constant, which takes a named
bit to zero. Then `stat & STAT_SEEDED` is false for every status word the device will ever present.
Every existing test still passes, because the tests sample `interpret`'s verdicts and never name
the bit they exercise. Thirteen died to one new test.

The test is a transcription check and says so in its own doc comment. These constants came off the
TRM's register tables by hand, one line at a time. The failure to guard against is a bit copied to
the wrong line. That reads as a device permanently in mission mode, or never seeded, or reporting a
lockup the silicon never raised. The distinctness half is the part to weigh more. Two constants
twenty lines apart, both `1 << 3`, look correct individually, and no other check in the tree would
catch them.

The six that remain are all equivalent, each on one arithmetic fact.

- `IE_RAND_RDY_EN` and `ISTAT_RAND_RDY`, both `1 << 0`, under `>>` (2). `1 >> 0` is `1`. This is the
  degenerate shift [the recurring patterns](baseline-2026-08-03.md#patterns-that-recur-named-once)
  name, in the one crate where two constants sit at bit zero.
- `ISTAT_ALL`'s four `|` under `^` (4). The five bits it unions are bits 0 through 4 of `ISTAT`, and
  no two are the same bit. On disjoint operands `|` and `^` are the same function. The argument is
  the one `capability::note_peak` carries in
  [the 2026-09-19 triage](regressions-2026-09-14-capability-to-device-tree-blob.md#capability-8-survivors-5-killed-3-equivalent).
  An argument a reader has to take on trust is worse than one they can run. So
  `the_clear_mask_is_every_named_istat_bit_and_nothing_else` now asserts the disjointness the claim
  rests on: five bits set, each named constant present.

The four timeouts are `Pool::take`'s gather loop, and they stay recorded gaps. The reason is the
doctest one given in
[a loop that waits](new-crate-backlog.md#the-pattern-the-whole-part-turned-out-to-be-about-a-loop-that-waits).
They are a `refill` that reports success without filling, `got < n` widened to `got <= n`, the
spent-buffer test inverted, and `got += run` turned into a multiply. Each leaves a pass of the loop
that makes no progress, and none of them returns at all. `take_returns_rather_than_spinning` states
the property on a five-second deadline. That helps the person who runs `cargo test`, and does not
move the classification.
