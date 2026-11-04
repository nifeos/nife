---
status: BUILT
raised: 2026-09-03
built: 2026-09-03
---
# 238. Two scheduled checks have never once succeeded, and one of them is a fatal risk's refresh

Minted the same day by calef, from milestone 232's (audit every check
against two questions: does anything run it, and does it block) findings, on the argument that
fixing the mechanism beats rewording the risk it holds up. *(Number provisional until the merge
queue lands it.)*

It was minted with no gate and needed none: both symptoms were already recorded and both repairs
live in this tree.

In brief. Milestone 232's audit found two scheduled workflows that have never produced a result,
and their consequences are not the same size.

## The mutation workflow, which is holding up a fatal risk

Four scheduled runs, four failures, zero reports: 2026-08-10, -17, -24 and -31, verified against
the API rather than inferred.

`design/fatal-risks/README.md` risk 3 (the tests do not test anything, and the quality is illusory) reads
MEASURED, green, on 92.4% of viable mutants killed, and closes:

> **The remaining experiment is cheap:** re-run it and compare against `.cargo/mutants-baseline.txt`.
> No new milestone; milestone 85 already owns it and the weekly workflow already publishes the
> report.

That last clause is false and has been since the workflow was written. It is also what makes a
stale number acceptable, because it says a refresh is arriving on its own. The 92.4% is from
2026-08-03 and 2,529 commits have landed since.

**The symptom is described and deliberately not diagnosed** by the lane that found it, because the
step logs have aged out of `gh`. On the most recent run all four shards died together about fourteen
minutes in with `The runner has received a shutdown signal`, which is the shape of a run being
reclaimed rather than a defect in a shard. On the three before it, shard 4 died in about twenty
seconds every time while its siblings ran for half an hour. Whether that is a memory kill, an
eviction, or something in `script/mutation --shard 4/4` is the first thing to find out, and the
workflow's own comment already says so.

## The Miri check, which has been red for three weeks on nothing

`every_character_survives` in `crates/documentation/tests/render.rs` reads `CARGO_MANIFEST_DIR` at run time and **Miri does not
forward the environment**, so the test fails there and passes under `cargo test`. **It is not
undefined behaviour and never was.** The fix is one flag. Milestone 232's lane did not apply it
because an audit repairs nothing.

## Why they belong together

Both are the same defect one level up from the code: a check that runs on a schedule, fails, and
tells nobody. Nothing in `script/gates` or CI turns red when a scheduled workflow has been failing
for a month, so the only thing standing between a dead cadence and a stale claim is somebody
happening to look. Milestone 232 was that somebody, once.

## What it needs

- The mutation workflow producing a report, and the shard-4 question answered rather than worked
  around. If the answer is that the runner cannot hold the job, say so and price the alternatives
  (more shards, a longer schedule, a smaller corpus) rather than quietly reducing what is measured.
- The Miri run green, or an honest statement of what it cannot cover.
- Something that notices next time. This block does not say what: a check that reads the API for
  scheduled-workflow health is one shape, and it is also the shape that goes stale itself. Whoever
  takes this should decide with the same suspicion milestone 232 applied to everything else.

## BUGS

- **A green mutation run is not the same as a good one.** The number it produces has to be compared
  against `.cargo/mutants-baseline.txt`, and a run that succeeds while the score falls is a different
  finding that this milestone does not cover.
- **Fixing the cadence does not re-read risk 3.** It removes the reason the verdict is currently
  unsupported; whether the verdict then holds is a question for whoever reads the new number, and it
  is an architect's.
- Neither repair says anything about the other seven scheduled workflows, which milestone 232
  inventoried but did not run.

## What was found and built, 2026-09-03

The shard-4 question had a boring answer and an interesting corollary. `cargo mutants --shard
k/n` requires `k < n`, so `--shard 4/4` was rejected as an invalid argument in zero seconds on every
run: not a memory kill, not an eviction, an off-by-one in the matrix. The corollary is the part
worth keeping: shards are zero-indexed, so **`--shard 0/4` was never run**, and with the default
`slice` sharding that quarter was an alphabetical block (`dtb`, `calendar`, `compositor`, `cred`,
`elf`, `capability`). `script/mutation` had reported the failure correctly and loudly the whole
time, to a scheduled job nothing reads.

The other deaths are a single mutant exhausting the machine's memory in twenty seconds, and
getting there took one wrong answer worth recording. A resource sampler was added to the job,
because a `shutdown signal` kills the runner agent and every `if: always()` step with it, so no
autopsy can run afterwards. At a 60-second interval it said, unambiguously, that memory was not the
problem: shard 0 died 57 seconds after a sample showing 14.7 GB available of 16 and 107 GB of disk,
and two siblings looked the same. That was written up as runner eviction. At a 10-second interval
the same failure is a straight line into the wall:

```
20:35:31  mem_used_mb=1352   mem_avail_mb=14595
20:35:41  mem_used_mb=6335   mem_avail_mb=9611
20:35:51  mem_used_mb=15763  mem_avail_mb=183
20:36:16  The runner has received a shutdown signal
```

A mutant turns a bound into an unbounded allocation, the
machine goes, and the runner agent goes with it. The per-mutant timeout cannot catch this. It is
auto-derived at 28 to 51 seconds on this tree, and 16 GB is gone well inside that, so a mutant that
allocates rather than spins is a hang the clock never gets to name. A sampler whose interval is
longer than the event it watches for reports innocence, which is worse than reporting nothing.

So `-j 2` is not the constraint, since one runaway takes 14 GB by itself. The workflow now shards
eight ways with `--sharding round-robin`, and that is damage control rather than a repair. It
does not stop a shard dying, it makes a lost shard cost an eighth of every crate instead of all of a
few, so a partial report is still a report about every crate. Nothing is measured less: the same
9,857 mutants are attempted. The repair is a bound on what one mutant may allocate, and it is handed
off rather than guessed at, below.

A number came back, the workflow published its first report ever, and the number is lower. The
eight-way round-robin run had one shard survive, and because round-robin samples every crate rather
than an alphabetical block, that one shard is a uniform one-eighth of the whole corpus across all
60 crates. 83.4% of viable mutants killed, against the baseline's 92.4% from 2026-08-03. (The
earlier four-way run's surviving shard read 74.4%, but it was an alphabetical slice of mostly
post-baseline crates and is not comparable to anything; both are tabulated in
notes/mutation-testing.md.)

The baseline-era crates are broadly stable or better (`gpt` 55/1, `elf` 12/0, `calendar` 46/0,
`grant_plan` 67/2). Three crates added since carry nearly all of the loss: `system_initializer` at
0 caught and 25 missed in the sample and 0 of 191 in the slice run, `uefi_loader` at 15%, and
`manual` at 52%.

**The Miri fix was not one flag, and each layer hid the next because `cargo miri test` stops at the
first failure.** `-Zmiri-env-forward=CARGO_MANIFEST_DIR` works and lands on a
second wall, `read_dir` against Miri's isolation; behind that sat five `board_console` tests doing
host I/O, invisible until the first two were cleared. Clearing all of it needs `-Zmiri-disable-isolation` for the whole workspace run, and the
corpus test costs 0.74 seconds natively against more than 12 minutes under Miri, unfinished when
killed. Both crates have no dependencies and no `unsafe`, so there is nothing there for Miri to
judge. The tests gate themselves out under `cfg(miri)`, which is the convention `gpt`, `glob`,
`calendar`, `cred` and `ntp_proto` already use, with the reasoning at each test.

And a fourth defect, found only because the third was fixed. The `report against the baseline`
job failed even when a shard survived: `download-artifact` gives each artifact its own subdirectory
when it fetches several and unpacks a *lone* artifact flat, so `shards/mutants-shard-*` matched
nothing exactly in the case the job's `if: always()` exists to serve. It cost the one real report
this milestone's first run produced.

Something to notice next time: `script/cadence-check`. It reports any scheduled workflow whose
last successful scheduled run is more than 15 days old, and a workflow too young to have fired is
excused by its own creation date. It is deliberately not a scheduled workflow, which is the
whole design and the reason milestone 232 declined to build it: a cron that watches crons dies the
way its subjects die. Delivery is `helpers/trunk-health.sh`, which already runs under `launchd` and
could not have caught this on its own, structurally: its trunk check filters runs to `main`'s
current tip, and a weekly run matches that tip only until the next merge. It is not in `script/lint`
or `script/gates`, for `script/audits`' recorded reason.

## BUGS, after the work

- **Fatal risk 3 was not re-read, deliberately.** That is an architect's, and the block above says
  so. What changed is that the clause making a stale number acceptable, "the weekly workflow already
  publishes the report", is now true rather than false: one has been published. What it says is
  that the score has fallen from 92.4% to roughly 83.4%, on a uniform sample rather than a census
  (7 of 8 shards died, so 8,700 of 9,857 mutants remain unrun since 2026-08-03). Whether that moves
  the verdict is his call and not this lane's.
- The memory kill is diagnosed, not solved. Eight round-robin shards make a partial report
  representative; they do nothing to stop a runaway mutant taking a runner, and round-robin arguably
  spreads the runaways across more shards than `slice` concentrated them in. On 2026-09-03, four of
  eight shards still died. See the handoff below.
- The cadence check inherits patagonia's gap. `helpers/trunk-health.sh` runs under `launchd` on
  one Mac, so a machine asleep is a watcher not watching. That cost is already recorded and accepted
  in AGENTS.md; this does not change it.
- `audit cadence` is still red and this does not touch it. It is red because two audits are
  genuinely due, which is the signal working. `script/cadence-check` will keep naming it until
  somebody runs them, and there is no way to tell a correct red from a broken one from outside.
- Seven eighths of the mutant corpus is still unmeasured since 2026-08-03. A full refresh needs a
  run where enough shards survive, and no such run has happened yet.
- The Miri run is no longer red, and does not yet fit a 90-minute budget. All
  three reported failures are fixed and nothing in the workspace fails under the interpreter any
  more; what remains is cost, all of it in `compositor`, handed off above. The budget is raised to
  240 minutes here rather than tuned, because the true total has still not been measured end to end:
  every attempt to time it was stopped by this lane needing the machine back. The 27-minute figure quoted in that workflow and in
  notes/undefined-behavior.md is from 2026-08-03 and has not described the run for some time, which
  nobody could have known while the job was dying in two minutes.

## Handoff: the exhaustive sweeps in `compositor` under Miri

**Proposed milestone (provisional, number is the integrator's).** *Sample `compositor`'s per-pixel
sweeps under `cfg(miri)`, the way `glob`, `ntp_proto`, `calendar` and `gpt` already sample theirs.*

Clearing the three reported failures exposed a fifth layer that no report had ever mentioned,
because a job that dies in two minutes never reaches the code that takes an hour. `compositor` has
six full-screen sweeps, each 317,856 pixels, several doing per-pixel work heavy enough to matter
(a `Vec` allocated per pixel in `every_screen_pixel_distinguishes_its_owner`, a call into
`expected_screen_pixel` in others). One of them,
`a_damaged_composite_leaves_the_rest_of_the_screen_alone`, was measured at over 44 minutes without
finishing; it is strided here, with a pinned sample, and dropped to 57 seconds. The other five
are not, and they are why the run still does not fit its budget. With that one fixed, a later run
sat in `the_checksum_catches_a_wrong_surface` alone for more than 40 minutes, and the whole rest
of the workspace outside this crate takes well under a minute to reach. `compositor` is the entire
remaining cost of `script/undefined-behavior-check`.

Each needs its own judgment rather than one blanket stride, which is why this is a lane and not a
line in this milestone. `the_scene_exercises_overlap_clipping_and_background` asserts *counts* of
visible pixels per window, so a stride changes the numbers it checks, and
`every_screen_pixel_distinguishes_its_owner` is making a completeness claim that a naive sample
weakens. The strided one shows the shape to copy: never skip the rows that carry the claim, and pin
the sample size so a stride that stopped covering the screen cannot pass quietly.

**Do not answer this by excluding the crate.** That was considered and refused. `compositor` has no
`unsafe` today, which is the argument that retired `xtask` and `board_console` from the run, but it
is central system logic under active development, and an excluded crate is one where a future
`unsafe` block is silently uncovered. A bench tool and the build system can carry that; the
compositor should not.

## Handoff: bound what one mutant may allocate

**Proposed milestone (provisional, number is the integrator's).** *A mutant that allocates without
bound should be a killed mutant, not a killed machine.*

The mechanism is small and the choice between three shapes is the work:

- A memory cgroup around the run (`systemd-run --scope -p MemoryMax=...`). The kernel's
  OOM killer takes the test binary, which is inside the scope, rather than the runner agent, which
  is outside it. Most precise; needs `sudo` and careful environment inheritance.
- `ulimit -v` before `script/mutation`. One line, inherited by every child. The risk is that it
  also bounds `rustc`, which bundles jemalloc and reserves address space generously, so the cap has
  to clear the compiler while sitting under half of physical memory (`-j 2` means two of them).
  Cheap to falsify: a run's baseline build either survives the cap in the first minute or does not.
- A `cargo` runner wrapper, so only the test binaries are bounded and the compiler is untouched.
  Cleanest in principle; `target.<host>.runner` is only consulted when `--target` is passed, so it
  needs checking before it is chosen.

Whichever wins, more gets measured, not less: a runaway mutant that today destroys a shard's entire remaining run would instead be
recorded as one caught mutant, and the other 1,200 in that shard would finish.

This was not built here because each attempt costs a twenty-minute CI round trip and the failure
mode of a bad cap is a build that breaks confusingly rather than loudly. That is the wrong thing to
ship under a feedback loop that slow.

## Follow-on

- **Milestone 244.** `system_initializer`, the crate carrying most of the score's fall (0 caught and
  25 missed in the sample, 0 of 191 in the slice run). Milestone 244 was minted from this
  milestone's first published report, measured the crate, and closed RECORDED because the pure
  fraction a mutation can reach is small.
- **Recorded.** `design/roadmap/0238-the-scheduled-checks-that-never-run.md` says in `BUGS` that a
  green mutation run is not the same as a good one.
- **Recorded.** `notes/mutation-testing.md` tabulates what the two partial runs did and did not
  cover. The seven eighths still unmeasured since 2026-08-03, and why, are in `BUGS, after the
  work`.
- **Recorded.** `AGENTS.md` already accepts the gap `script/cadence-check` inherits from
  `helpers/trunk-health.sh`, told in `BUGS, after the work`.
- **Recorded.** `design/decisions/0074-audit-cadence.md` holds the cadence and what it is for. Why
  `audit cadence` is red is told in `BUGS, after the work`.
- **Recorded.** `design/roadmap/0238-the-scheduled-checks-that-never-run.md` records in `BUGS` the
  boundary around the other seven scheduled workflows milestone 232 (audit every check against two
  questions) inventoried and did not run.
  `script/cadence-check` now names any of them that has gone more than 15 days without a scheduled success.
- **Refused.** Excluding `compositor` from the Miri run to make it fit its budget, for the reason
  given in the `compositor` handoff above.
- **Milestone 349.** Milestone 349 (sample `compositor`'s five remaining full-screen sweeps under
  Miri) is the first handoff above, with its measurements.
- **Milestone 277.** Bound what one mutant may allocate, so a runaway allocation kills the mutant
  instead of the machine. Promoted out of the proposal queue 2026-09-11. The second handoff above
  holds the three shapes and the measurement behind them.
- **Done.** Re-read fatal risk 3 against the new number. calef ruled it on 2026-09-19 and
  `design/fatal-risks/README.md` now reads MEASURED, AMBER. The 83.4% this bullet was written against
  turned out not to exist. The workflow this milestone repaired went on to complete all eight shards
  on 2026-09-14, and the census put the tree at 91.7% over 64 crates with the baseline's own 38 up
  from 92.4% to 93.6%. The amber half is the two findings a census can show and a sample cannot, and
  they are named in the entry.
- **Milestone 280.** Promoted 2026-09-13. Measure and answer
  for `uefi_loader` at 15% and `manual` at 52%, the two crates this block names beside
  `system_initializer` as carrying nearly all of the fall from 92.4% to 83.4%. Milestone 244 took
  `system_initializer` alone. Nothing tracks these two, and no `BUGS` entry anywhere accepts their
  scores, so the published number has two unexplained holes in it.

## Index row

four mutation runs, four failures, zero reports, while fatal risk 3 rests partly on the claim that
the weekly workflow publishes one
