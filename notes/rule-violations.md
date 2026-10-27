# The violation ledger: a rule that gets broken three times moves up the ladder

*The name `rule-violations.md`, the ledger's column shape, and `script/rule-violations`'s name are
all provisional. A lane ships a provisional name and says so; naming is an architect's
(AGENTS.md).*

Milestone 118 (claude) found that `AGENTS.md` warns against two things by name, each exactly once: `pkill`-ing
another lane's QEMU, and `git reset --hard` to "take a measurement". Both had already been violated
by the day the warning was measured: one lane killed another lane's emulator mid-test, and four
agents clobbered work with `reset --hard`, `checkout` or `stash` in a single day. The rule was
present and was skipped.
Milestone 118's own roadmap block names the fix for exactly this shape, echoing the ladder
`AGENTS.md`'s "Nobody remembers, so build the mechanism that does not need them to" section already
lays out for everything else: *"A rule that is violated repeatedly is not stated too quietly. It is
on the wrong rung."* Prose is rung four of that ladder. This ledger is what turns "violated
repeatedly" from an impression into a number, so the promotion decision has evidence instead of a
feeling behind it.

## What this is not

It is not an enforcement mechanism, and milestone 118's own honest-limit paragraph says so:
*"The ledger is the counterweight and it is weaker than a gate, because it depends on lanes
continuing to report their own mistakes honestly, which is a culture rather than a mechanism."*
Nothing here can see a violation happen. It can only total what somebody wrote down, the same way
`script/audits` cannot run an audit and can only say one is due. Red means a rule needs a
decision, never that this script made one.

## The convention

A row per incident (or per a dated batch of them, when the source that reported them did not
distinguish individual incidents), in the table below:

| date | rule | instances | status | source |
|---|---|---|---|---|
| 2026-08-04 | squash against `origin/main` instead of the recorded base SHA | 1 | open | AGENTS.md, "Commits" |
| 2026-08-04 | a destructive git operation (`reset --hard`, `checkout`, or `stash`) used to discard changes or "take a measurement" without committing or stashing first | 4 | resolved | design/roadmap/0118-constitution-budget.md, "What it costs, measured 2026-08-05" |
| 2026-08-04 | `pkill` a QEMU process by name/pattern instead of walking the process tree from the harness that owns it | 1 | open | design/roadmap/0118-constitution-budget.md, "What it costs, measured 2026-08-05" |
| 2026-09-23 | a cheap, reversible fix reported to calef instead of made ("owning is not recording") | 1 | escalated | AGENTS.md, "We are all owners" (the incident that wrote the rule) |
| 2026-09-24 | a cheap, reversible fix reported to calef instead of made ("owning is not recording") | 2 | escalated | pull request #1202: a relayed lane report listed a stale line and a merge sequencing instead of doing them, and the correction was answered with a memory restating the rule. #1202 described the Stop hook as living in `.claude/settings.json`, but `.claude/` was gitignored, so the file never reached `main` and the hook was wired for nobody until pull request #1216 |
| 2026-09-24 | a conversation with calef never blocks the queue: work longer than a couple of tool calls goes to a background agent | 3 | escalated | pull request #1216, against AGENTS.md's top-up rule: calef asked "Why am I waiting?" three times in one session, for the bootstrap fix, the #1201 gloss fix, and the earlier #1201/#1179 sequencing, each done in the foreground |

Columns:

- date. When the incident (or the batch) happened, not when the row was added.
- rule. The rule's own words or a close paraphrase, so two rows about the same rule can be
  told apart mechanically (see BUGS: today that means *identical text*, not judgment).
- instances. How many times it happened. Usually 1; a source that reports an aggregate
  ("three agents... in one day") without naming each one gets a single row with that count, which is
  honest about what is known rather than inventing distinct incidents to hit a row-per-count shape.
- status. `open` (nothing has changed about the rule since), `escalated` (a higher-rung
  mechanism now exists, and the row stays as the record of why), or `resolved` (the rule itself is
  gone, e.g. deleted as unenforceable). Only `open` rows count toward a strike.
- source. Where the incident is on the record. A lane report or a pull request is fine. Today's
  three rows are backfilled from `AGENTS.md`'s own prose and from milestone 118's roadmap block
  (design/roadmap/0118-constitution-budget.md). Those are what recorded and measured the
  incidents, not because a lane report is a worse source than a constitution.

Three strikes on an `open` rule, and it must move up the ladder or be deleted as unenforceable.
`AGENTS.md`'s own ladder (Nobody remembers...) names four rungs; a rule stuck at rung four (prose)
after three strikes has demonstrated that prose is not the rung it needs. Moving it is an edit to
`AGENTS.md`, which is an architect's or the integrator's, never a developer lane's: this ledger's
job ends at naming which rule has crossed the line, not at deciding what replaces it.

## What it found on its first run, 2026-08-22

Backfilled from the incidents `AGENTS.md` and milestone 118's own roadmap block record, listed
above. **One rule was past the threshold**: *"a destructive git operation... without committing or
stashing first"* carried four open strikes from a single day (2026-08-04), one more than the
"three strikes" the convention describes, and exactly the kind of decision this ledger exists to
surface rather than make. `AGENTS.md` already contains a lot of prose about this (the "move fast on
what can be undone" section, the squash-against-base-SHA scar, the worktree-pruning warnings) but
no mechanism, so the ledger correctly surfaced a real open decision rather than making one.

The other two rules (`squash against origin/main`, `pkill` a QEMU process by pattern) sit at one
strike each and are informative rather than urgent.

## What changed, 2026-08-25: the git-clobber row marked `resolved`

[DECISIONS §128](../design/decisions/0128-git-clobber-enforcement.md) researched the four candidate
mechanisms named above and priced them. Git has no `pre-checkout`/`pre-reset`/`pre-stash` hook to
build a genuine pre-command check on. A shell shim can't distinguish `git checkout <path>`
(dangerous) from `git checkout <branch>` (ordinary) by prefix alone, so neither is a clean win. But
the decision that actually closed the row was evidence, not a new mechanism: all four incidents
trace to a single day (2026-08-04), and **zero repeats in the three weeks since**. calef's own
call: "No repeat in three weeks seems like it isn't a problem any longer." The row is marked
`resolved` on that basis, matching the convention's own reading of the word (the *concern* is
resolved). The rule itself was not deleted from `AGENTS.md`, which stays exactly as written. The
prose that already exists appears to be doing the job, so nothing further was added above it on the
ladder. Should the pattern recur, this row (or a fresh one, since exact-text
matching means a differently-worded recurrence starts its own count) would reopen the same
question with three more weeks of context behind it.

## EXAMPLES

Add a row when a lane's report honestly says it violated a documented rule, or when a maintainer
finds one in a merged pull request's history:

```markdown
| 2026-08-22 | never `pkill` another lane's QEMU | 1 | open | PR #309's report |
```

Check the ledger, against the three rows this note carries today:

```
$ script/rule-violations
rule violations: 3 rows, 2 open instances across 3 rules
   1 strike                   [open     ] squash against `origin/main` instead of the recorded base SHA
   1 strike                   [open     ] `pkill` a QEMU process by name/pattern instead of walking the process tree from the harness that owns it
   0 strikes                  [resolved ] a destructive git operation (`reset --hard`, `checkout`, or `stash`) used to discard changes or "take a measurement" without committing or stashing first
```

```
$ script/rule-violations --check
rule violations: 3 rows, 2 open instances across 3 rules, none past 3 strikes
$ echo $?
0
```

## BUGS

- **A rule that crosses three strikes turns every pull request red until the ledger answers it.**
  `--check` is a `local` row of `script/ci-build` and a step of the `lint` job. Milestone 341 (give
  the three instruments nothing runs a caller) wired it, under §174 (a caller for each of the three
  uncalled instruments), ruled 2026-10-27 UTC. It takes 0.07 seconds. It passes clean today. When a rule
  crosses, the fix is not a lane's. Moving the rule up the ladder or marking it `resolved` belongs
  to an architect or the maintainer. Until one does, the check fails for every lane. The ruling
  accepted that cost against a ledger that nothing reads.

- **"The rule" is matched by exact text, not by meaning.** Two rows describing the same rule in
  different words are counted as two different rules. A human curating the table has to normalize
  wording by hand; nothing catches a near-duplicate.

- **Depends on honest self-reporting**, restated from the top of this note because it is the whole
  limitation: a rule violated and never mentioned in any report leaves no row and no strike. The
  ledger's number is a floor on how often a rule was actually broken, never a ceiling.

- **A `resolved` or `escalated` row's instances still print in the report, but do not count toward a
  strike.** That is deliberate (a resolved rule should not keep failing `--check`), but it means the
  human-readable total ("6 open instances") is not the same as "6 instances ever recorded" the moment
  the first row is marked otherwise, and nothing calls that out beyond this paragraph.
