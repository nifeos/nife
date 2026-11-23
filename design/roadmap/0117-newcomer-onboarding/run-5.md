# Milestone 117 appendix: run 5 and the two lanes that took its handoffs

*An appendix to [milestone 117 (the stranger test)](../0117-newcomer-onboarding.md), moved out of
that page on 2026-10-12 (UTC) under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it. Run 5's full record and scored rubric are
[notes/stranger-test/run-5.md](../../../notes/stranger-test/run-5.md); this page is what the block
said about it at the time.*

## What run 5 found, 2026-08-18

Status did not move: still `PARTIAL`. Run 4's handoff 1 is the one this run closed, and it closed it
only half way: the mechanism existed, it had now been exercised by somebody who did not write it,
and nothing scheduled it.

What run 5 measured that no previous run could. It is the first run through `script/stranger-test`,
and the first whose stranger was told at turn zero that it was being measured. It is also the first
on an uncontended machine (load average 2.44 on 8 cores at launch, no other lane, no QEMU). 201
turns, about 29 minutes, `$10.56`, plus a `--smoke` run at `$0.09` that exercised every stage
first. Six of eight mental-model rows answered, one partly, one absent.

### The harness isolates a tree and cannot isolate a machine

This result matters most and is not on the rubric. `nife-dev` is an account-wide `rustup` link, so
the clone's first `script/test` compiled `std` out of another worktree's farm. `std-aborts` then
failed naming two files and two line numbers inside a checkout the stranger had been told did not
exist for it. Both fixes the error message suggested would have written a false statement into
`ABORTS_ACCEPTED`. The stranger diagnosed it only by grepping for the foreign path, which led it to
`script/stranger-test` itself, so the harness contaminated the run and then rescued it. Recorded in
notes/std.md's `BUGS` and in `script/stranger-test`'s.

### `script/apropos` did not fire

That is this run's cleanest negative result. It landed 2026-08-18 because three runs could not reach
`notes/net.md`, `notes/capabilities.md`, any `design/decisions/` file, or `crates/moho_abi/src/lib.rs`.
Run 5 never ran it, still reached no `design/decisions/` file, and still never opened
`notes/net.md`. It had the name in front of it three times: in `ls script/`, in the guest builtin in
`crates/swish/src/lib.rs`, and in `SWISH_CHECK_SCRIPT`'s transcript. The only page that says what it
does is `notes/scripts.md`, which five runs have not opened. Recorded in `script/apropos`'s `BUGS`.

### `AGENTS.md` was never opened

The stranger read `CONTRIBUTING.md` third and shipped a working program on both ISAs. Run 4 read
`AGENTS.md` seventh and complained that `CONTRIBUTING.md` was sixteenth. Fixing the second appears
to have cost the first, because a reader who meets a shorter document summarizing a longer one
stops. Its own reason: *"66 KB is a large upfront cost when a task is in front of you, and
everything I actually needed turned out to be reachable from code."* This is the same criticism run
4 made of the tree's prose habit, arriving independently and with a falsifiable instance attached.

## What run 5 handed off

None of it was done in its lane, per the same rule runs 3 and 4 followed.

1. Assert that `std_aborts()`' dep-info paths are under `farm_dir()`. One comparison, and it
   turns a false accusation about somebody else's source into a true statement about the machine.
   Say the recovery (`rm -rf std_exerciser/target`) in the failure message. The failure caches,
   and re-running reproduces it in thirty seconds, which reads as stable rather than stale. This is
   the cheapest item here, and it is on the customer path by way of trust: a gate that names a file
   and a line and is wrong costs more than a gate that says nothing.
2. Decide whether an argument-plus-input manifest is wanted. Today it is possible to declare, and
   it turns `the_arg_line_follows_the_manifest_for_every_program` red, in `crates/swish`, which the
   person adding the program has no reason to open. The test sweeps the enum and asks the manifest
   about the argument, then hard-codes `Holdings::default()` and the line `"<name> 21"`. Either the
   sweep learns the rest of the manifest, or the combination is refused at `plan_against_with` with a
   comment, the way file-plus-input already is. This is a fork rather than a lane. A comment there
   rules out file-plus-input on `ArgSpec` grounds and says nothing about this case, so a newcomer
   cannot tell headroom from a deliberate gap. Recorded in notes/adding-a-program.md's `BUGS`.
3. `CONTRIBUTING.md` says `script/gates` runs three stages and it runs five. Five minutes. The
   two it omits are `script/icount` and `script/test --hvf`, and the HVF leg is the slowest and the
   only stage that flaked in this run. It matters more than an ordinary drift because that sentence
   is what earned run 4's best unforced result.
4. Two stale counts, each five minutes. `script/setup`'s comment names `nightly-2026-07-26` when
   the pin is `nightly-2026-08-18`. `README.md`'s reading order counts 403 markdown files and 143
   notes when there are 413 and 145. Both are the failure this tree already names, a duplicated
   fact rotting, and both were the first things run 5 wrote down.
5. Say where `script/apropos` is, somewhere a newcomer reaches. Its own `BUGS` now records that
   five runs missed it; where it should be named is a claim about the reading order, so it is
   calef's rather than a lane's. Until it is answered, the tool built to close the
   `design/decisions/` gap has closed nothing, which is measurable and was measured.
6. Handoff 4 from runs 3 and 4 is unchanged and is now nominated by three successive strangers:
   adding a program should not need eight hand-maintained lists. Run 5 adds that the eighth is not
   even a constant, since it depends on the manifest shape.
7. The cadence for running this is an architect's and nothing else can move the status.
   *Answered 2026-08-18: monthly, and built the same day.* Two things in this handoff were wrong
   and are worth correcting rather than deleting. Milestone 129 (scheduled execution) is not the
   machinery: it is nife's own cron. This run happens on a host with a `claude` CLI and two QEMUs,
   so the machinery is the audit cadence's, one directory over. And the `$10.56` is the CLI's
   `total_cost_usd` for the stranger process alone. It excludes the lane that pre-registers,
   watches, debriefs, scores and writes up the run, which is the larger half. Quoting it without
   that caveat under-prices a run by more than half, and this block had already done so twice.

## The second handoffs lane, 2026-08-22

Status stayed `PARTIAL`. This lane took run 5's seven handoffs and landed the ones that were code or
prose fixes. It gave the one design fork nominated by three successive strangers a tracked home, and
checked whether the cadence built 2026-08-18 actually owed a run. It found run 5 had not, in fact,
fully handed off: five of seven items were still open four days later, which is what this lane
closed.

1. Done: `std_aborts()`'s dep-info paths are now asserted to be under this worktree's own
   `farm_dir()`. `foreign_std_sources()` in `xtask/src/main.rs` canonicalizes every path
   `compiled_std_sources()` found and compares it against the farm, and `std_aborts()` calls it
   before doing anything else. A contaminated build now fails with the machine explanation run 5
   asked for (the account-wide `nife-dev` link pointed at a different worktree). It also gives the
   recovery command (`rm -rf std_exerciser/target && cargo xtask std-exerciser`). It no longer names
   a file and a line inside a checkout that was never handed to the caller. Not yet exercised
   against a real contaminated farm in this lane, since reproducing run 5's race would mean
   deliberately racing another worktree's `xtask std-src`. The existing `std-aborts` host coverage
   and a clean `script/test` are what verify it here.
2. Unchanged, correctly: the argument-plus-input manifest fork. It was already recorded in
   `notes/adding-a-program.md`'s `BUGS` with the file and the test it turns red, and that recording
   is the tracked home the finding needs. Deciding whether the combination is wanted is a design
   fork above a lane, not a gap this lane could close by picking an answer.
3. Done: `CONTRIBUTING.md` now says `script/gates` runs five stages. It names `script/icount` and
   `script/test --hvf` by name and points at notes/load-sensitive-assertions.md for the HVF leg's
   known flake, instead of undercounting by two.
4. Done, and both stale counts had drifted further since run 5 measured them. `script/setup`'s
   comment now says `nightly-2026-08-22`, the pin as of this lane. It had said `nightly-2026-07-26`
   since before run 5, and the pin had moved twice more in the four days between. `README.md`'s
   reading order now says 425 markdown files and 145 notes. The actual count had grown to 413/145 by
   run 5's own measurement and 425/145 by this one, while the page still said 403/143. A count fixed
   by the first handoffs lane on 2026-08-18 had already gone stale again by 2026-08-22 (a
   different count of a different kind; see [run-3.md](run-3.md)). That is the same finding this
   tree keeps making about duplicated facts, not a new one.
5. Unchanged, correctly: where `script/apropos` should be named. Its own `BUGS` section already
   records that five runs have missed it and says why it is an architect's call rather than a
   lane's: naming a place in the reading order is a claim about what matters.
6. Handoff 4/6, nominated by three successive strangers, now has a tracked home. It was minted as
   milestone 150, "Adding a program should not need eight hand-maintained lists"
   (`design/roadmap/0150-program-declaration-data.md`). The number was provisional; 147, 148 and
   149 were already claimed by other lanes' pull requests, one of them merged into this lane's own
   base commit. `notes/adding-a-program.md`'s `BUGS` section now points at 150 instead of at this
   milestone's own handoff list. That was a circular reference: the mechanism it asked for is not
   something milestone 117 itself builds.
7. The cadence: already answered and built by 2026-08-18, unchanged. `script/stranger-test --due`
   reported "not due for another 26 days" as of 2026-08-22, four days into the thirty since run 5.
   No run was conducted in this lane, on purpose. Inventing one the cadence does not yet owe would
   be exactly the failure the cadence lane names: closing a cadence by doing work that was not due,
   rather than by the calendar actually asking for it. `script/stranger-test --check` and
   `script/roadmap --check` both passed against the record as this lane left it.

What remained for `BUILT`: nothing left in this lane's control. The next thing that could move the
milestone was a run 6, conducted when `script/stranger-test --due` next exits 1, roughly 2026-09-17
on that record. Everything else run 5 asked for either had a tracked home or was never a lane's call.

## The arg-plus-input clarity lane, 2026-08-22

Status stayed `PARTIAL`, and no run was conducted here, on purpose. `script/stranger-test --due`
still reported "not due for another 26 days" against the same record. A run the milestone did not
yet owe would be exactly the failure the cadence lane names. So this lane confirmed the cadence
(`--due` and `--check` both still pass) rather than pre-empting it. It spent its work on the one
item the second handoffs lane left correctly but thinly recorded: run 5's handoff 2, "decide
whether an argument-plus-input manifest is wanted."

That handoff was correctly left undecided, and it was also mis-stated as more undecided than it
is. The second handoffs lane said the fork is "already recorded... with the file and the test it
turns red," which is true. But the recording it pointed at still read as though `ArgSpec` +
`InputSpec` might be the same kind of closed door as `FileSpec` + `InputSpec`. That was because
`plan_against_with`'s comment ruled the second combination out and said nothing about the first. A
newcomer meeting both in the same page had no way to tell headroom from a deliberate gap. That is
this milestone's own recurring failure mode: a fact a reader needs, left only where the person who
already knows it would think to look.

Tested rather than argued, per the six-questions convention. A host test was added to
`crates/grant_plan/src/lib.rs` (`an_argument_and_an_input_stream_compose_by_the_same_fixed_order`).
It plans `nth 21 report.txt` against a manifest declaring both `ArgSpec::Required` and
`InputSpec::Required`, and gets a clean grant back. `arg` claims the fixed first position,
numeric-shaped, before `input`'s bare-name fallback ever looks at what is left. That is exactly the
way `arg` and `file` already compose for `STAMPS_A_FILE`. The combination is not blocked by the
planner today. The only reason it cannot ship is that `crates/swish`'s
`the_arg_line_follows_the_manifest_for_every_program` types a single-operand preview line. It never
supplies the input operand such a program would also need, which is a gap in that sweep rather than
in `plan_against_with`. `plan_against_with`'s comment and `notes/adding-a-program.md`'s `BUGS`
entry both now say this in place, so the distinction is where a reader meets each combination.

What is still genuinely calef's, unchanged by this lane: whether an argument-plus-input program
is wanted at all. Nothing here decides that; it only removes the false impression that deciding it
requires widening `ArgSpec` first.
