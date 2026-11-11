---
status: PROPOSED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Lint fails in the queue after passing on the pull request

Raised 2026-11-11 (UTC) by lane `milestone/1042-the-merge-queue-is-sized-to-the-runner-pool`,
while it recorded the queue sizing ruling of milestone 1042 (the merge queue is sized to the runner
pool). That ruling defers any change to the queue's size until the `lint` failures inside merge
groups are fixed and the queue is measured again. This proposal says what those failures are.

What drives it: the re-measurement that milestone 1042 and milestone 965 (gating CI rents more
concurrency before it owns a runner) wait for. What it saves: group builds that the queue throws
away, about one in eight of the week's merge-group builds.

## The week

From 2026-11-04 to 2026-11-10 (UTC) the `CI` workflow ran 404 merge-group builds, and 99 of them
failed. The `lint` job failed in 53. Each failed run's log ends with the list of `script/lint`
sections that failed, and the 53 logs name 58 section failures, since five runs failed two sections.

Every one of the 53 runs traces to one of 10 pull requests, the culprits. A culprit is an entry that
the queue removed for `failed_checks` and that appears in every failing group of its window. The
other runs are entries that the queue built behind a culprit:

| | failing runs |
|---|---|
| a culprit was the entry under test | 25 |
| the entry sat behind a culprit in the same group | 28 |

## Is the premise true?

The brief said that `lint` gates every pull request before it enters the queue. That holds in name
only, for two reasons.

- A `pull_request` run tests the pull request merged into the base as GitHub last computed it. For
  #3253 the lint run of 05:27 on 2026-11-05 checked out a merge into `75e1c9a07`, which `main` had
  left 50 minutes and five merges earlier. And no pull-request run can see the entries ahead of it
  in the queue. A check that reads the whole tree can pass on the pull request and fail on the group
  with no flaw in either. That is the case the queue exists to catch, and it accounts for 46 of the
  53 runs.
- A draft's run skips `lint`, and GitHub counts a skipped required check as passed. A pull request
  marked ready and armed at once enters the queue on that skip, before the ready run's `lint` has
  started. That is a gap, and it accounts for the other 7.

## The causes

| cause | section failures | culprits |
|---|---|---|
| 1. two pull requests each add prose to a file at the 3,000-word cap | 29 | 4 |
| 2. a new check, baseline or generated index meets content from another pull request | 22 | 4 |
| 3. the entry reached the queue on a draft's skipped `lint` | 7 | 2 |
| nondeterminism | 0 | 0 |
| a check that reads the pull request's base instead of the merge commit | 0 | 0 |
| a lint section that runs only in a merge group | 0 | 0 |

The last three rows are the other suspects the brief named, checked and not found. `script/lint`
selects no section by event, and the merge-group step in `ci.yml` that reads `base_sha` decides only
whether the change is prose that skips the job.

### 1. Prose that sums past the cap

Two shared documents sit at the §212 (a prose budget) cap of 3,000 words, and
many lanes append to each:

- `notes/crate-reviews.md`: 18 failures. #3314 accounts for 11, #3373 for 3 and #3270 for 4. The
  #3270 failures were the §213 passive rate, not the word count. The file holds 2,990 words today by
  `wc`, and 122 commits on `main` touched it in the last 14 days.
- `design/roadmap/0857-crates-released-on-their-own.md`: 11 failures, all behind #3197, which met
  #3196 ahead of it in the queue. The file holds 2,997 words today.

Each lane's addition fits. Two additions do not, and only the queue ever sees the pair.

The fix: move the per-crate text of `notes/crate-reviews.md` into the per-crate files beside it in
`notes/crate-reviews/`, leaving an index with a line per crate, and give milestone 857's block an
appendix for its record of rulings. Each change is a document edit, reversible, and leaves the cap
as it is. A file that every lane appends to should not sit within a lane's addition of a cap.

### 2. One pull request's check meets another's content

| section | failures | culprit | the other side |
|---|---|---|---|
| the template's report-only name and README gates | 9 | #3336 | #3337, ahead in the queue, bumped the template pin and wrote the first baseline; #3336 added one acronym to `e1000e` prose (49 against 48) |
| naming conventions | 7 | #3253 | #3243, ahead in the queue, moved the word list into the vendored gates; #3253's new note used a word that list refuses |
| the section list | 4 | #3208 | #3208 added the committed section index; a section that #3195 added to `script/lint` was missing from the index |
| the mutation ledger crate table is current | 2 | #3144 | #3211 added the generated table; #3144 changed what it lists |

These are the queue doing its job. The fix moves the detection earlier, not out of the queue:
`script/preflight-queue` already merges each armed pull request onto the green entries ahead of it in
a scratch worktree and runs the cheap end of `script/ci-build`. Nothing runs it on the way into the
queue. `helpers/arm.sh` runs only `script/queue-conflicts`, which finds textual conflicts and not
these. Question A below asks whether arming should run lint over that stack.

### 3. A skipped lint let a red entry in

Both culprits were marked ready and queued two seconds later. The only `lint` result on the head was
the draft run's `skipped`:

- #3320, 2026-11-07: ready at 19:36:29, queued at 19:36:31. The ready run's `lint` started at
  19:37:07 and failed: its change to `script/kani-install` broke `helpers/kani-pool-selftest.sh`
  stage 1 (6 failures, every one in "the CI fuzz job keeps a finding private"). Its own later commit
  `c7a773c49` fixed it.
- #3427, 2026-11-10: ready at 20:07:09, queued at 20:07:11. The ready run's `lint` failed on 65
  literal window addresses against a ceiling of 63 (1 failure, then dequeued by hand).

#3253 entered the same way, two seconds after its ready event, but its ready run's `lint` passed
against its stale base, so it counts under cause 2.

The fix in the tooling: `helpers/arm.sh` refuses while the newest `lint` check run on the head is
skipped or unfinished, and waits or says so. That changes no gate. It closes the path every brief
names, and not a bare `gh pr merge --auto`. Question B asks whether the gate should close it too.

## What multiplied the failures

- All-green grouping. A group that holds a red entry fails, and the queue rebuilds every entry behind
  it. That is why 10 culprits produced 53 failing runs: 28 of them were innocent entries rebuilt
  behind a red one. The 2026-11-11 ruling keeps `ALLGREEN`, so the remedy is fewer red entries, not a
  different grouping.
- Re-entry at an unchanged head. The 10 culprits went back into the queue 10 times after a
  `failed_checks` removal with no new commit: #3314 and #3336 three times each, and #3197, #3208,
  #3270 and #3373 once. Each failed again. The unattended maintainer re-armed #3314 at 22:28 on
  2026-11-07 because "its pull-request checks are green", which is the stale green of the premise
  section. A failure caused by an entry ahead does not clear when that entry merges, so a re-entry at
  the same head helps only a flake, and the week had no lint flake.

The fix in the tooling: `helpers/arm.sh` refuses to arm a pull request whose last queue removal was
`failed_checks` at its current head, with an override that posts its reason, the same shape as
`--despite-conflict`. The unattended maintainer's routine arms through it.

## Questions for the architect

Background first. The merge queue is the only place that builds a pull request on top of the
entries ahead of it. Every lint failure above that is not a skipped check was a pair of changes,
each green alone, that the queue caught. Catching a pair costs one group build per entry in the
group, about 200 job minutes each on the hosted pool that milestone 1042 found full. Catching the
same pair before arming costs one `script/lint` run on the lane's own machine, about 7 minutes on a
hosted runner, and it catches only what is already in the queue when the lane arms.

A. Should arming run `script/lint` over `main` plus the entries ahead, through
`script/preflight-queue`, and refuse on a failure?

- (a) Recommended: yes, lint only, with an override that posts its reason. It would have caught all
  four cause 2 culprits and, at arming time, the cause 1 pairs whose partner was already queued
  (#3197 and #3196, for one). It does not catch a partner that arms later; the queue still does.
- (b) No: keep arming at the textual check, and rely on the queue. This costs nothing per lane and
  keeps the week's 22 failures of cause 2.
- (c) Run the whole cheap end of `script/ci-build`, not only lint. This catches more but costs a
  lane far more wall time per arm.

It is reversible either way. It changes what a lane must pass before its pull request may enter the
queue, which is why it is a question and not a fix. At equal cost the recommendation holds, so it
is not about effort.

B. Should a draft's skipped `lint` stop satisfying the required check?

- (a) Recommended: not now. The `helpers/arm.sh` refusal above closes the documented path. Measure
  for a week whether any entry still reaches the queue on a skipped `lint`.
- (b) Yes: the draft's `lint` job runs a step that fails, or the ruleset requires a check that a
  draft never posts. Every path closes, but drafts show red, and the drain and labelers that read a
  red check need to tell a draft apart.

This one changes what a gate enforces. Restrictive first would favor (b), since a gate is easier to
loosen later than to tighten. The recommendation is (a) because the tooling fix is cheaper to watch
and (b) touches the drain's reading of every draft. That reason is about effort, said as such.

## Steps

1. Split `notes/crate-reviews.md` and give milestone 857's block an appendix (cause 1). No ruling
   needed.
2. `helpers/arm.sh` refuses a skipped or unfinished `lint` on the head (cause 3), and a re-arm at
   the head the queue last removed for `failed_checks`. No ruling needed: tooling only.
3. Arming runs lint over the queue stack, if question A goes to (a).
4. Measure a week of merge groups again for milestone 1042, with the same classification.

## Done when

- A week of merge-group builds after steps 1 and 2 shows the `lint` failures by cause, with the
  culprits counted the same way.
- No entry in that week entered the queue on a skipped `lint`, and none re-entered at a head the
  queue had removed for `failed_checks` without the override's comment.
