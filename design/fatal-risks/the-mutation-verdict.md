---
risk: 3
color: amber
updated: 2026-09-19
---
# Appendix to risk 3: The tests do not test anything, and the quality is illusory

*An appendix to [`3-the-tests-do-not-test.md`](3-the-tests-do-not-test.md) (risk 3). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim

AGENTS.md's principle 2 says the method works because of the gates, the proofs and the review
discipline. If the suite would not notice the code being wrong, that sentence is decoration.

### The verdict of record

RUN, 2026-09-19, and MEASURED rather than merely observed. AMBER, and the ground shifted under it on
2026-09-20. calef ruled amber on the 2026-09-14 numbers, milestone 326 (nobody has been assigned to
turn a mutation score upward) triaged everything the amber half named, and he asked for a fresh
census before deciding whether it went green. The verdict stands and the reason it was given does
not, which is the useful part.

The fall this entry was built on did not happen. Milestone 518 (a census that cannot be attributed)
captured both censuses into a committed per-crate record and recomputed them consistently.
Like-for-like reads 94.7%, not 92.6%; the whole corpus 93.7%, not 91.4%. The runs themselves are
unchanged: run [35421192143](https://github.com/nifeos/nife/actions/runs/35421192143), eight
shards, all green.

| | crates | viable | killed |
|---|---|---|---|
| baseline, 2026-08-03 | 38 | 5,141 | 92.4% |
| census, 2026-09-14 | 64 | 9,277 | 91.7% |
| census, 2026-09-19, read the same way | 62 | 8,925 | 93.7% |
| like-for-like, 2026-09-14 | 38 | 6,552 | 93.6% |
| like-for-like, 2026-09-19, read the same way | 38 | 6,604 | 94.7% |

### The 2026-09-21 census

Re-read on 2026-09-24. The scheduled run of 2026-09-21,
[35589550926](https://github.com/nifeos/nife/actions/runs/35589550926), went green and was not
captured for three days. `script/mutation-census --add-run` recorded it then.

| | crates | viable | killed |
|---|---|---|---|
| census, 2026-09-21 | 66 | 10,178 | 92.4% |
| the same, without `uefi_loader`'s unbuilt modules | 66 | 10,046 | 93.6% |
| like-for-like, 2026-09-21 | 38 | 6,747 | 96.1% |

Like-for-like rose again, 94.7% to 96.1%. The corpus fell for two reasons, and neither is decay:

- 132 of `uefi_loader`'s 182 missed sat in `src/arch/`, modules of a `[[bin]]` no host build
  compiles. It was the 2026-09-04 trap in files the exclusion did not name. Pull request #1232
  excludes them and makes `script/lint` walk a gated target's module tree.
- Four crates were measured for the first time. `stick_maker` (64.9%, 101 missed) and
  `portable_executable` (72.6%, 76) carry most of it. The proposal
  `design/roadmap/proposals/triage-the-crates-the-2026-09-21-census-measured-first.md` holds that
  triage.

771 missed survivors stand, and 206 timeouts. The amber reads the same on this census as on the last.

### The 2026-10-03 census

Run [37108924347](https://github.com/nifeos/nife/actions/runs/37108924347), a dispatch on 2026-10-03
(milestone 636 (the scheduled workflows are failing, and nothing says so)), is the first complete census since 2026-09-21. The scheduled mutation workflow had been
red from 2026-09-28 because `system_tests` became a bare-metal crate and was not excluded; milestone 636 excluded it.

| | crates | mutants | viable | missed | timeouts | killed |
|---|---|---|---|---|---|---|
| census, 2026-09-21 | 66 | not tabulated here | 10,178 | 771 | 206 | 92.4% |
| census, 2026-10-03 | 85 | 14,853 | 13,734 | 1,004 | 255 | 92.7% |

It ran at the milestone 636 branch's head, which differs from `main` only by that milestone's commits.

### Two joins, each worth about a point

The published rows disagree about what a timeout is: the baseline and the 2026-09-14 figures follow
`notes/mutation-testing.md`'s rule that a timeout is a kill, while the 2026-09-19 figures (now in
`notes/mutation-testing/regressions-2026-09-14-capability-to-device-tree-blob.md`) scored that run's 205 timeouts as
survivors. And the crate those rows call `cred` is `credentialer`, so the join dropped it: 103 viable mutants at
100%. Reproducing the published 37 crates and 6,472 viable requires resolving two crates by hand and
missing that one.

The direction reverses under either definition read consistently, timeouts-as-survivors giving 89.5%
then 91.4%, and survivors across the corpus fell 771 to 563.

### Verified rather than taken on a lane's word

The maintainer recomputed the corpus and like-for-like rates from 518's committed record on
2026-09-20 and reproduces the reversal and both causes. The 94.7% figure is 518's, since reproducing
it needs that milestone's rename mapping. And an unresolved intersection lands just under it at
94.2%. The published 92.6% is reproducible exactly as a survivor-basis number, against a 93.6% that
is kill-basis, which is the mixing stated above seen from the other side.

It stays amber, on the standard this entry actually holds. On 2026-09-19, 563 survivors were untriaged.
The rule is milestone 85 (mutation testing over the host crates)'s: every survivor becomes a test, an
exclusion carrying its reason, or a recorded gap. That was the honest ground all along. The fall was
never needed to reach amber, and leaning on it meant this entry asserted a cause it could not
attribute, which its own next paragraph admitted in the same breath.

What green now requires, ruled 2026-09-20 and measured 2026-09-21. calef ruled that the condition
should be inflow: new code cannot arrive less tested, with the corpus rate as a lagging indicator.
Milestone 517 (what fraction of survivor growth arrives on lines a pull request touched) then
measured the premise rather than assuming it, and the answer is decisive. Of the census's 771
survivors, 629 sit on lines a pull request wrote and one is a genuine regression on a line nobody
edited (`compositor`'s `Rect::area`, caught in August, surviving in September). Old-code decay runs
about three orders of magnitude behind inflow.

> Green when both hold. (a) Inflow: the survivors a merged pull request adds on its own
> lines, measured by `cargo mutants --in-diff` on the merged diff, are zero or triaged into a test,
> an exclusion with a reason, or a recorded gap, under milestone 85 (mutation testing over the
> host crates)'s rule, for every pull request since the last census.
> (b) Trailing: the like-for-like census rate has not fallen between the two most recent
> censuses. Amber if (a) holds and (b) does not, because that is coverage decaying on code
> nobody is editing, which is a different defect wanting a different repair.

Three things about that wording are deliberate. It does not say "the blocking gate is on", because
the verdict is a measurement and not a settings read: clause (a) is answered by
`cargo mutants --in-diff` runs whether or not a gate consumes them. The gate exists (milestone 924
(a survivor on a pull request's own lines cannot merge untriaged), the reopening of milestone 479
(a blocking `--in-diff` mutation gate) that the 2026-10-14 census's 43 untriaged survivors
earned); the measurement, at four seconds on a documentation change, is still what the verdict
reads. It keeps clause (b) even though inflow dominates, because an inflow-only condition cannot
see `Rect::area` at all and that failure is silent by construction: nobody is editing the code, so
nothing prompts anyone to look. And it is a floor rather than a target, since a percentage target
can be met by excluding awkward crates, which is what milestone 85's rule exists instead of.

What it does not carry, stated where the verdict is read. The kernel. `kernel` generates 7,529
mutants and `components` 3,482, and a kernel mutant costs a relink plus a full QEMU suite, about 55
seconds each: roughly 500 runner-hours per census across three architectures against 52 minutes
today. A kernel *census* is refused on that arithmetic; a kernel *diff-scoped* check is minutes on a
kernel pull request and is the affordable half. So this condition speaks for the host-testable
corpus and not for the kernel, which is the largest thing it does not say.

The cost of adopting it, measured rather than estimated: 62 of the 761 pull requests merged in the
six-week window (8.1%) would have carried untriaged survivors, median 6 each.

What this cost, recorded because it is the second time. Milestone 512 (the census blamed one pull
request for 55 survivors it did not write) holds the first: a delta read from `script/mutation
--report`'s `(baseline missed)` column, which diffs against `.cargo/mutants-baseline.txt` from
2026-08-03 rather than against the previous census. So six weeks of growth read as two days of
regression. Both errors are one shape, a comparison across two records that were never made
comparable, and both were available because the per-crate numbers were never written down. They are
now, which is what made this correction possible at all.

2026-09-23: milestone 512, built, names the crate the "second time" paragraph above only pointed at.
`machine_discovery` read 22 survivors in the fixed 2026-08-03 baseline and 77 in the census that ran
on 2026-09-19, two days after milestone 319 (the crate that parses firmware) landed, which is what
made "two days of regression" look plausible. Replaying `cargo mutants --in-diff` against milestone
319's own pull request finds 4 survivors, and the crate already carried 73 on the commit immediately
before that pull request merged: 73 + 4 is 77, the census's own number, to the unit. The other 73
predate the pull request. They accumulated while the crate grew from 212 mutants at the baseline to
693 at the census, over six weeks the `(baseline missed)` column cannot see, because it always diffs
against 2026-08-03 rather than against the census before it. `script/mutation`'s own comments and
`notes/mutation-testing.md`'s `Scope and honest caveats` section now name that trap where a reader
of `--report` meets it.

One convention is now load-bearing and unchecked. Whether a timeout counts as a kill moves this
entry by about two points, and the rule rests on a hand-check of the baseline's 96 timeouts six
weeks ago. There were 205 on 2026-09-19 and 206 on 2026-09-21, and none of them has been checked.

The first amber half: seven crates regressed, and three of the baseline's five perfect crates lost
that score. `memory_regions` 100% to 88.9%, `elf` 100% to 94.2%, `capability` 97.4% to 88.2%, with
`clock_contract`, `swish`, `dtb` and `filesystem_contract` behind them. Each is a property that used
to hold and no longer does, which is a different object from a crate that was never covered. A
sample cannot surface these at all, because it cannot tell an absent mutant from a killed one, so
this is the first time in the project's life that the question has been askable.

The second: new code arrives less tested than old code, and nothing pulls it up. The 1.9-point gap
between 93.6% and 91.7% is exactly the 26 crates that did not exist at baseline. And the eight worst
crates in the tree are all of them new, led by `work_steal_slot` at 54.2% and `timetable` at 73.6%
with 48 survivors. `timetable` holds `next_after`, the property risk 2 below names as its strongest
counterfactual, which makes it the single survivor set most worth a person's afternoon.

Why amber rather than green, which is the part worth arguing with. 91.7% over a full census is a
good number and a green verdict would be defensible on it. It is refused because this file's job is
to be the place a green number cannot hide in. Milestone 85 (mutation testing over the host
crates)'s own rule is that every survivor is triaged into a test, an exclusion with a reason, or a
recorded gap. And that was done for the baseline's 391 survivors and has not been done for the
census's 771. A verdict of green would claim the discipline held when what held was the instrument.

And why not provisional, which was the other option. Declining a verdict until the 771 are triaged
would postpone the reading on the grounds that the evidence is good enough to want more of it. The
census *is* the experiment this entry has been waiting for; it is read here, and what it found is
recorded as owed work rather than as a reason not to read it.

What is owed, and it now has a block rather than a sentence. The 771 survivors of the 2026-09-14
census are untriaged, the seven regressions above have no owner, and the workflow has succeeded
exactly once, so a cadence is claimed by one data point. Nothing in `design/roadmap/` owned any of
it when this verdict was written: milestone 85 built the instrument and triaged the baseline, 238
repaired the workflow's shard indices, 277 bounded the runaway mutant. And 280 explained the two
crates that made the fall look real. So three of the four were repairs to the instrument and only
one ever turned a score. Milestone 326 was minted the same day for exactly that gap, ordered
`capability` first among the regressions and `timetable` next for its 48 survivors. And its own
definition of done is milestone 85's rule rather than a target percentage, because a percentage
target can be met by excluding the awkward crates.

What would move this entry now, stated better than the condition it replaces. Not a finished
worklist, and not a single number either. Two consecutive censuses where the like-for-like rate does
not fall, which is the smallest claim that distinguishes a suite keeping up from a triage pass that
happened recently. One census is a point; two is a direction, and the direction is what this entry
is about. The instrument now runs weekly and has completed twice, so this costs waiting rather than
work.

The refresh arrived on 2026-09-14 and it is not what the entry below predicts. The weekly workflow
completed for the first time, all eight shards, once milestone 277 (bound what one mutant may
allocate)'s memory bound stopped the runaway mutant: 10,012 mutants over 64 crates, 91.7% of viable
mutants killed. Against the 38 crates the baseline covers, like for like, 93.6% against 92.4%: the
score went *up*. `notes/mutation-testing.md` has the tables.

So the "fall to 85.3%" was an artifact, and the entry below is kept as the account it is. That
reading came from a one-eighth sample taken while two crates were being scored against suites that
could not run. And milestone 280 (unexplained holes in the published score) fixed both:
`uefi_loader` now scores 100% and `documentation` 95.4%, the two crates the drop had been blamed on.
The 1.9-point gap between the like-for-like 93.6% and the corpus 91.7% is the 26 crates that did not
exist at baseline, which is a worklist rather than a verdict.

`script/mutation` (milestone 85) ran 5,551 mutants over 38 host crates on 2026-08-03. 4,654 caught,
391 missed, 96 timed out, 410 unviable, which is 92.4% of viable mutants killed, with every survivor
triaged into a test, an exclusion with a reason, or a recorded gap. Five crates scored 100%.

What that does not settle, recorded because a green number is where inflation starts: the run is
from 2026-08-03 and the tree has grown since. It covers host crates only. So the kernel and the arch
trees, where risks 5 and 9 live, are not in it at all. And mutation testing measures the test suite,
not the code.

### The remaining experiment was cheap

Re-run it and compare against `.cargo/mutants-baseline.txt`. No new milestone; milestone 85 already
owned it, and it ran on 2026-09-14.

Correction, 2026-09-11, and its second half closed three days later. That paragraph used to close
"and the weekly workflow already publishes the report", and the workflow had published nothing.
`mutation.yml`'s own `BUGS` section records it: the workflow had never once succeeded, four
scheduled runs red from 2026-08-10, found by milestone 232 (does anything run it, and does it
block)'s audit on 2026-09-03. Milestone 238 (two scheduled checks have never once succeeded)
repaired one of the two causes (shard indices counted from one, so a job died in twenty seconds
every run and shard 0 was never tested). The other was repaired by milestone 277 on 2026-09-12 (a
runaway mutant exhausting the runner's memory inside the timeout meant to catch it, which had taken
the 2026-09-07 run), and the next scheduled run, 2026-09-14, was the workflow's first success. The
cadence is alive; `script/cadence-check` is what reported it dead, and one success is not yet a
cadence.

One number published in between, and it read worse: 83.4%, corrected to 85.3%. It came from the
single shard that survived, a uniform one-eighth sample across all 60 crates rather than the 38 host
crates the 92.4% figure covers, so it was never a like-for-like reading. Two crates carried most of
the apparent fall and neither was explained at the time: `uefi_loader` at 15% and `manual` at 52%.
Both turned out to be measurement rather than quality (milestone 280, built 2026-09-13), as did a
third, `system_initializer`, before them, in milestone 244 (the largest crate in the tree is proved
by nothing a mutation can reach). The census of 2026-09-14 above replaces this number; it is kept
here because it is what this entry was ranked on for eleven days.

### Both repairs landed and the reading arrived

The runaway mutant was milestone 277, built 2026-09-12, which made the clean full run possible for
the first time since 2026-08-03; the run happened two days later, and calef read it on 2026-09-19.
The proposal that had owned the re-read since 2026-09-03 was drained with the ruling, which is what
a proposal is for.

The proposal asked the wrong question, and that is worth keeping. It was written against a fall from
92.4% to 83.4% and offered three options about how bad the fall was. By the time it was read the
fall had been corrected twice, first to 85.3% and then out of existence, so none of its three
options described the tree. The reading above is against the census instead. It is the fifth
proposal in two days whose premise expired between filing and reading; milestone 323 (the
falsification record is incomplete in five ways) carries the argument that promotion, not filing, is
where that is cheapest to catch.
