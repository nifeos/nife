# A hole found while counting: three harnesses that nothing ran

*Name: provisional, minted 2026-10-11 (UTC) by lane/sixth-ten-longest-notes.*

The count in [`notes/proof-retrospective.md`](../proof-retrospective.md) found 145 harnesses in the
tree and 140 that `script/verify` ran. This is the account of that gap as it stood on 2026-08-30,
and how it closed the same day.

`script/verify` proves a hand-kept crate list. On 2026-08-16 milestone 125 (a number in the prose is
a claim) found that `multicast_dns_protocol` had landed with three harnesses and never been added to
it. The suite had never run them, and the way it showed up was the suite going green *faster*. The file's own comment now says a missing row *"is
the one way this table can make the proofs wrong"*.

It had happened again. `crates/jh7110_trng` was a workspace member (`Cargo.toml` line 35) carrying
three harnesses, and the string `jh7110_trng` appeared nowhere in `script/verify`. The shard table
held 23 crates summing to 140 harnesses; `crates/` held 143 across 24. Nothing ran
`a_lockup_bit_is_never_overridden`, `ready_requires_rand_rdy_and_carries_the_words_untouched`, or
`neither_bit_set_is_always_not_ready`, and they guard the entropy source on the board this project's
first-silicon milestone runs on.

The counted-claims gate cannot see it. `harness-crates` and `kani-harnesses` are `count-at-least`
floors against prose, so they check that the tree has at least as many harnesses as a note claims;
neither compares the tree against the shard table. The multicast_dns_protocol case was caught only because a prose
claim happened to be one crate off, which is luck rather than a mechanism.

`vendor/redoxfs`'s two harnesses are also unrun, and that one is fine: `script/lint`'s
`_harness_hits` docstring says so on purpose, *"counting them would make the number describe a suite
nobody executes"*. That is a decision. `jh7110_trng` was an omission, and the difference is that
nobody wrote anything down.

The study did not fix it, and the reason is not tidiness. Those three harnesses had never been run,
so nobody knew whether they passed. Adding the row from a lane that cannot run the prover risked
turning `main` red on a check that takes forty minutes. The study asked for a lane that runs
`cargo kani -p jh7110_trng` first and adds the row with a measured cost column. That lane would then
close the gap properly, with a gate comparing `_harness_hits()`'s crate set against
`script/verify`'s table. That is rung two, and it turns a recurring accident into an impossible
state.

## Closed on 2026-08-30 by milestone 193

Milestone 193 (put `kernel/src` within reach of the prover) ran the three harnesses (all pass),
added the row, and gave `script/lint` the check that makes it the last time. Item 1 of the parent's worklist is done.

The crate has since been renamed, and the names above are the ones the count was taken under. The
crate is `jh7110_entropy` now, so `script/verify`'s table carries it under that spelling. calef
ratified `jh7110_entropy_source` on 2026-09-13 and replaced it with `jh7110_entropy` later the same
day; the rename was performed 2026-09-14. The measurement above stays traceable to the run that
produced it.
