---
experiment_status: RUN
experiment_run: 2026-09-19
---
# 3. The tests do not test anything, and the quality is illusory

*Risk 3 of [the nine](README.md). The status vocabulary, the rule an entry meets and the running order are there.*

The claim: AGENTS.md's principle 2 says the method works because of the gates, the proofs and the
review discipline. If the suite would not notice the code being wrong, that sentence is decoration.

**The experiment:** milestone 85 (mutation testing over the host crates), read as a census and
re-read against the baseline.

Re-read 2026-09-24. MEASURED
rather than merely observed, and AMBER. calef ruled amber on the 2026-09-14 numbers; the fall behind
it did not happen. On 2026-09-21 the 38 baseline crates read 96.1% against
92.4% in August. The corpus reads 92.4%, or 93.6% without 132 mutants no host build
compiles ([`notes/mutation-testing.md`](../../notes/mutation-testing.md)).

It stays amber on the standard this entry holds: milestone 85's rule that every survivor becomes a
test, an exclusion carrying its reason, or a recorded gap. The latest census, 2026-10-12, counted 1,577 missed
survivors, and every one of its survivor keys has a row in the triage record (the facts below), and
milestone 326 (nobody has been assigned to turn a mutation score upward) owns the repair. Green is a ruled
condition rather than a number (calef, 2026-09-20): inflow, meaning the survivors a merged pull
request adds on its own lines are triaged.

Two caveats. This verdict speaks for the host-testable corpus and not for the kernel, where a census
is roughly 500 runner-hours against 52 minutes today. And one convention is load-bearing and
unchecked: whether a timeout counts as a kill moves this entry two points. That rule rests on a
hand-check of 96 timeouts in August; 303 stood on 2026-10-12.

Fact, 2026-10-20: milestone 326 is BUILT. Every survivor of the 2026-10-19 census (run
[39238962856](https://github.com/nifeos/nife/actions/runs/39238962856)) has a triage row, and
`.cargo/mutants-baseline.txt` is that census (101 crates), so the weekly report diffs against a
triaged corpus rather than the August run. It does not move this entry's status, which is a ruled
condition and calef's: from here the weekly inflow check and the per-pull-request diff gate
(milestone 924 (a survivor on a pull request's own lines cannot merge untriaged)) keep new survivors triaged.

Fact, 2026-10-04: milestone 745 (count the error paths no test reaches), a provisional number, found that no test executes 586 of the host crates' 1,150 Result-family error paths (51%). 490 of them are a `?` whose error side never ran. A further 1,159 are in the kernel and services, where no coverage run reaches ([untested error paths](../../notes/untested-error-paths.md)).

Fact, 2026-10-03: scheduled-workflow run
[37108924347](https://github.com/nifeos/nife/actions/runs/37108924347) (a dispatch, milestone 636 (the scheduled workflows are failing, and nothing says so)) is the
first complete census since 2026-09-21: 85 crates, 14,853 mutants, 13,734 viable, 1,004 missed, 255
timeouts, 92.7% killed against 92.4% on 2026-09-21.
[Appendix](the-mutation-verdict.md).

Fact, 2026-10-03: milestone 517 (what fraction of survivor growth arrives on lines a pull request touched)'s inflow measurement ran once and is not a weekly report; between the 2026-09-21 and 2026-10-03 censuses 600 new survivors sit on lines 58 merged pull requests wrote, 364 in crates with a triage ledger section and 236 in crates with none ([inflow](../../notes/mutation-testing/inflow-2026-10-03.md)).

Fact, 2026-10-04: the inflow check exists (milestone 740 (the survivors a merged pull request adds are
checked against a triage record), provisional). The weekly mutation workflow diffs each census
against the previous one, blames new survivors to the merged pull request that wrote the line, and
fails listing any with no row in `notes/project-metrics/mutation-triage.csv`. Against the 2026-09-21
census, 472 survivor keys are blamed to merged pull requests since: 162 have a triage row and 310
do not.

Fact, 2026-10-04: of the last 60 merged pull requests, 4 merged with the per-pull-request falsification replay red, because it was not a required check; milestone 742 (every test is falsified as routine) made it part of `verify (Kani proofs)` ([coverage](../../notes/falsification-coverage.md)).

Fact, 2026-10-07: every one of run 37108924347's 1,004 missed survivors has a disposition: 621 killed
by a test, 359 equivalent with a written reason and 24 recorded gaps, with none left untriaged
(milestone 637 (triage the crates the 2026-09-21 mutation census measured for the first time),
provisional; [part five](../../notes/mutation-testing/census-2026-10-03-triage-part-5.md)). The census
runs on Linux and lane re-runs on macOS, and one function's `cfg!` branch made three mutants read
caught locally and missed in every census. A per-crate re-run of the last 39 crates also found 35
missed survivors in code merged after that census, with no triage row.

Fact, 2026-10-14: two censuses followed (`script/mutation-census --list`). Run
[37307759312](https://github.com/nifeos/nife/actions/runs/37307759312), 2026-10-05: 89 crates,
16,660 mutants, 15,447 viable, 728 missed, 272 timeouts, 95.3% killed. Run
[38252397727](https://github.com/nifeos/nife/actions/runs/38252397727), 2026-10-12: 98 crates,
19,771 mutants, 18,366 viable, 1,577 missed, 303 timeouts, 91.4% killed, and 93.6% over the 89
crates in both. Most of the rise is three drivers merged in the window: `e1000e` went from 60
missed to 285, and `designware_ethernet` and `designware_mobile_storage` arrived with 257 and 205.
Both inflow lists are triaged, 710 keys with 554 killed, 153 equivalent and 3 gaps
([inflow triage](../../notes/mutation-testing/inflow-2026-10-12-triage.md)). The 61 keys of the
2026-10-12 census that neither list named, 102 mutants, are triaged too: 13 killed and 48
equivalent ([residue](../../notes/mutation-testing/census-2026-10-12-residue.md)). Of the 16 gaps
recorded before that, 9 are now killed, 2 are equivalent and 5 remain. Run
[38654094250](https://github.com/nifeos/nife/actions/runs/38654094250), 2026-10-14: 98 crates,
20,065 mutants, 18,588 viable, 752 missed, 296 timeouts, 96.0% killed. Its inflow job listed 45 new
keys on lines merged pull requests wrote, 43 of them untriaged. Those 43 and one unattributed key
are triaged: 36 killed and 8 equivalent
([inflow triage](../../notes/mutation-testing/inflow-2026-10-14-triage.md)).

Fact, 2026-10-15: the per-pull-request gate exists (milestone 924 (a survivor on a pull request's
own lines cannot merge untriaged), provisional): a pull request fails the check when
`cargo mutants --in-diff` finds a surviving mutant on a line it changed and the triage ledger has
no `equivalent` or `gap` row for it (`helpers/mutation_diff_gate.py`,
`.github/workflows/mutation-gate.yml`). A `killed` row never clears one, a timeout is untriaged
the same as a survivor, and a diff whose mutants exceed the 20-minute cap fails. It covers the
host corpus; `kernel/` and `components/` generate no mutants and pass with the summary that says
so. The check is in the required set: `gh api repos/nifeos/nife/rules/branches/main` lists "mutation gate (a survivor on a changed line cannot merge untriaged)" (corrected 2026-10-27, UTC; §216 (fatal-risk facts are correctable, and verdicts are the architect's)).
