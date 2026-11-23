# Reviewing the work of one model, without answering the question in advance

*Name provisional (`model-attribution-review`). This is a plan, not a review. Nothing here is a
verdict on any model, and the reconnaissance below is deliberately reported in the order it was run
rather than in the order that would make a story.*

calef raised a concern about the quality of the code written by Claude Sonnet 5 in this tree and
asked for a plan he could approve, redirect, or decline. This note is that plan. It also carries the
cheap measurements that were run first, because they were expected to narrow the expensive work and
they did something better than that: they moved the whole question.

## The trap this plan exists to avoid

A review that reads only one model's commits cannot answer the question it was sent to answer.

All code contains defects. Reading 180 diffs will find some. Those findings will read as
confirmation whether or not the work is actually worse than anyone else's, because there is nothing
in the exercise capable of producing the other answer. Such a review spends an architect's attention
and hands back a false result with evidence stapled to it, which is worse than handing back nothing.

So every tier below is built to be capable of exonerating as well as indicting: a control group, a
rubric fixed before the reading, blinding where blinding is achievable, and a stated null result. The
sections on what each of those costs here, and where blinding does not work at all, are the load
bearing part.

## First: the numbers this lane was handed do not reproduce

The brief gave a table of commits per model, and on `origin/main` at `0b993b27` it does not come
out that way. The total (2054 non-merge commits) and the untrailered count (648) agree exactly.
Four of the five per-model rows do not, and the handed rows double-count 61 commits. Measured, by
two derivations that agree: Claude Opus 5 (1M context) 623, Claude Fable 5 338, Claude Opus 4.8
233, Claude Sonnet 5 180, Claude Opus 5 32.

This is milestone 125 (a number in the prose is a claim) arriving uninvited. The point is not that
someone miscounted. It is that the first number in a study about trustworthiness was itself
unverified, which is exactly the failure the study would be looking for. Both tables and both
derivations are in [handed-numbers.md](model-attribution-review/handed-numbers.md).

## What the reconnaissance found

Seven findings, in the order they were run. Each, with its tables, is in
[reconnaissance.md](model-attribution-review/reconnaissance.md).

1. Sonnet's work is one week and one session. All 180 Sonnet-trailered commits fall between
   2026-08-22 and 2026-08-27, and no other model has a commit after 2026-08-21. 178 of them name
   the same `Claude-Session`. So model is perfectly confounded with date, tree state and milestone
   range. The concern also has a free alternative explanation: the work calef read most recently is
   Sonnet's because all of it is recent.
2. Attribute by pull request, not by commit. No pull request mixes Sonnet with another model, so
   the 62 untrailered commits inside Sonnet pull requests are Sonnet's too, 242 in all.
3. The rest of the blind spot cannot be recovered. Untrailered commits carry no other Claude
   marker, and the only same-window comparison is 80 pull requests (140 commits) whose author is
   unknown.
4. The work is not mostly code. Sonnet is second only to Opus 4.8 in kernel share and lowest in
   tooling. It wrote 46,223 diff lines, 38,459 of them in commits that touch code.
5. It sat squarely on the irreversible list: 25 commits and 649 diff lines on the system call surface
   and the wire crates, milestone 158 (build the eleven kernel object and identifier renames), and
   new crates and programs. The naming half needs no study, because `script/names` already tracks
   it.
6. Three defect metrics rank the models three different ways. Survival says Sonnet is best,
   file-level fix-up says worst, and line-level blame puts it mid-pack in the kernel. All three rest
   on 77 fix-shaped code commits in the whole tree. The cheap evidence does not discriminate.
7. The sample is uneven, and its good end is very good. So the thing to review is conditions, not
   a model: rushed lanes, thin briefs, unfamiliar subsystems, lanes racing in hot files.

## The design a real answer needs, and where it breaks here

### The control group

A defect count with no base rate is not a finding. The control must be matched on subsystem, commit
size, and date, reviewed by the same reviewer against the same rubric.

Date is the one that cannot be matched. Sonnet's window is disjoint from every other model's. The
options, none of them clean:

1. Same-window control: the 80 untrailered pull requests (140 commits). Perfectly matched on
   date, tree state, milestone range, and lane conditions. Unattributed, so a difference is
   interpretable as "Sonnet lanes versus lanes that did not stamp a trailer" and no further. Cheapest
   and most defensible. This is the recommended control.
2. Subsystem-and-size-matched control from Opus 5 and Fable, drawn from earlier weeks. Matched on
   what the reviewer reads, unmatched on everything the tree was doing. Any difference confounds
   model with three weeks of tree evolution.
3. Both, which is the only way to see whether the two controls disagree, and they will.

### Blinding, and the honest statement that it leaks

Blinding is implementable and is the difference between a study and a confirmation exercise. The
mechanism already exists in `script/stranger-test`: a separate `claude` process started in the
tree's parent, with its isolation probed before the run. A harness would extract each sampled
commit as a randomized patch with every attribution line stripped, and join attribution to scores
only after every score is committed.

It leaks, and the report must call it partial. Milestone numbers, post-rename vocabulary and
anything else that dates a commit all identify the window, and the window is the model. Option 1's
same-window control is the only configuration where blinding actually holds, because there none of
the leaks discriminate. That is a second, independent reason to prefer it. The scheme, step by
step, and the three leaks are in [blinding.md](model-attribution-review/blinding.md).

### The rubric, fixed before the reading

Written and committed before the first case is opened, so severity cannot be decided post hoc in the
direction the reader expects. Six axes, each scored 0 to 2 with the anchors written out:

1. **Correctness.** Does the change do what its message says, on the paths it touches.
2. **Confinement.** Does it widen authority, and does the message say so if it does.
3. **Irreversibility handled.** If it touches the system call surface, a wire format, a name, or a
   dependency, is the decision recorded and is it calef's where it should be.
4. Test earns its keep. Does the test prove something nothing else would have proved, or is it
   filler.
5. The message explains why. `git blame` is what a commit is for.
6. **Honest limits.** Does a `BUGS` entry or a stated caveat exist where the change fell short.

Axes 3, 5 and 6 are the ones this tree cares about most and the ones a generic code review would not
score at all.

### The null result, stated in advance

The study reports "no differential found" under two conditions. The blinded per-axis mean for the
Sonnet arm is within one standard error of the control arm on every axis. And the count of
severity-2 findings per 1,000 diff lines differs by less than a factor of two. With the sample sizes below, a factor of two
is roughly the smallest effect the study can see, and saying so up front is what stops a null being
written up as a failure to look hard enough.

If the null comes back, the reportable conclusion is not "Sonnet is fine". It is: *this tree cannot
distinguish them at this sample size, and the observed variation is better explained by subsystem and
lane conditions than by model.* That sentence is worth as much as the other one.

## The tiers

Prices are lanes, wall-clock, and rough tokens. They assume the reconnaissance above as done and not
repeated.

### Tier 0: what you already have. 0 lanes, 0 tokens.

This note. It establishes that the window is one session and six days, and that model and date
are perfectly confounded. The cheap metrics disagree with each other, and the naming half is
already tracked by `script/names`. The irreversible surface Sonnet touched is 649 diff lines across
25 commits, rather than anything like the whole 46,223.

If calef declines everything below, the reconnaissance still says: there is no cheap evidence of a
differential, and the expensive evidence would be hard to obtain and easy to fake.

### Tier 1: the irreversible surface, unblinded, no control. 1 lane, 2 to 3 hours, ~400k tokens.

Read every Sonnet diff touching `crates/moho_abi`, `kernel/src/system_call.rs`, and `crates/*_proto`: 25
commits, 649 diff lines. Then the 7 new crates' and 7 new programs' manifests and capability
declarations, and the 14 Cargo manifest changes against §46's rule that a dependency is a decision.

This tier is deliberately not a study and makes no claim about any model. It asks a different and
more useful question: did anything expensive to undo land in that week without the record it
should have carried. A control group would add nothing, because the standard is absolute rather
than comparative.

It should be folded into the security audit that is already due rather than run as its own thing.
`script/audits` currently reports `documentation` and `security` both overdue, each with three
triggers fired, and the last security audit's lens was "newly minted authority, read adversarially"
on 2026-08-17. Pointing the next one at the window is one row in
`design/audit-reports/README.md`, which is milestone 93's own precedent for adding a kind without
adding machinery. Cost in that shape: zero additional lanes, because the audit is owed anyway.

Recommended, and recommended first.

### Tier 2: blinded sample with the same-window control. 2 to 3 lanes, ~1 day, ~2 to 3M tokens.

- Lane A builds the harness: case extraction, scrubbing, randomization, the isolation probe. Roughly
  the shape of `script/stranger-test`, reusing its isolation mechanism rather than reinventing it.
- 60 Sonnet cases, stratified by subsystem (kernel, crates, user, prose) and by size, drawn from the
  242 pull-request-attributed commits rather than the 180 trailered ones.
- 60 control cases from the 80 untrailered same-window pull requests, stratified to match.
- Lanes B and C score independently against the pre-committed rubric, in isolated processes.
  Inter-rater agreement is reported; a rubric two readers cannot agree on is not a rubric.
- Attribution joined last.

What it can conclude: whether Sonnet-trailered lanes differ from same-window untrailered lanes at
an effect size of roughly two-to-one or larger. What it cannot: anything about Sonnet versus Opus
or Fable, since neither is in the window.

That limitation is severe and is the reason this tier is second rather than first.

### Tier 3: exhaustive, with a cross-window control. 5 to 6 lanes, 2 to 3 days, ~8 to 12M tokens.

All 242 pull-request-attributed Sonnet commits, plus a subsystem-and-size-matched 242 drawn from
Opus 5 and Fable in earlier weeks, all scored blind against the same rubric.

This is the only tier that produces a Sonnet-versus-other-model number. It is also the tier whose
number is confounded by three weeks of tree evolution and by partial blinding, and it costs an order
of magnitude more than tier 1.

Not recommended. The confound is not fixable by spending more, so the extra spend buys precision
on a quantity that is not the one anyone wants.

### Tier 4: the prospective A/B, which is the only design that isolates the model. Costs no review lanes.

Assign the next N ready milestones alternately between two models, same brief template, disjoint
subsystems, both lanes running in the same days against the same tree. Score the results blind
against the tier 2 rubric.

This is the only design in which model is not confounded with date, tree state, or milestone
difficulty. It costs nothing in review lanes because the lanes were going to run anyway. It costs
scheduling discipline and a delay, since the answer arrives when the milestones do.

If calef wants a real answer about a model rather than about a week, this is the one to approve, and
it is compatible with approving tier 1 today.

## What no tier can find

Stated plainly, because a plan that hides its blind spots is the thing it is trying to review.

- A design decision that was wrong but implemented cleanly. Every tier reads diffs. A diff that
  correctly implements the worse of two options looks exactly like a diff that correctly implements
  the better one. The rename campaign is precisely this shape: whether `CapabilityTable` was the
  right call is not visible in the patch that performs it.
- Work that was never written. A lane that stopped early, declined a hard item, or produced a
  thinner milestone than the block asked for leaves no diff to read. The roadmap blocks would show
  it and the commits will not.
- Anything about the 648 untrailered commits. They are 32% of the tree and no source in the
  repository attributes them.
- Whether a defect was caught or landed, before roughly 2026-08-15. Per-branch check history is
  only reliably available for recent pull requests, so "did the gates catch it" is answerable for the
  Sonnet window and progressively less so going back, which biases any gate-based comparison toward
  finding the recent window worse.
- Effects smaller than about two to one. With 77 fix-shaped commits in the whole tree and 120
  cases in tier 2, the study is underpowered for anything subtler. It will not distinguish a good
  model from a slightly better one, and should not be asked to.

## The six questions

*A fork reaches calef with its questions already answered.* In short, and whole in
[six-questions.md](model-attribution-review/six-questions.md):

1. The alternatives lost on stated grounds. An unblinded read of all 180 diffs cannot produce a
   negative result, metrics alone disagree with each other, and tier 3 loses to tier 4 because only
   tier 4 removes the confound.
2. Tier 1 reuses what the tree already does: `script/audits`, `script/stranger-test` and
   `script/names`.
3. The prior art is the SZZ algorithm, whose known failure modes are the ones observed here.
4. The premise is partly true, and the part that is false is the finding: Sonnet's commits cannot be
   compared with other models' in this tree, because there is no overlap in time.
5. Each option is priced from measured diff sizes, not estimates.
6. The review is reversible and its conclusion is not. A written verdict that a model is worse is a
   fact that leaves the machine, which is why the rubric, the control and the stated null exist.

## BUGS

- **The follow-up window for Sonnet's newest commits is under a day, not six.** The exposure-equalized
  table in finding 6 caps every model at 6 days, but a commit written on 2026-08-27 has had one day for a
  fix to arrive. Sonnet's line-level rates are therefore a floor and will rise as the tree moves
  on. They were not adjusted for this, because any adjustment is a model of arrival rates that
  nothing here has measured.
- The fix-shaped-commit regex is keyword-matched and this tree writes self-critical prose. Words
  like "wrong", "broken" and "stale" appear in commit bodies describing the *reason* for a change
  rather than a defect being repaired. 77 commits matched; they were eyeballed and most look genuine,
  but no one has classified them one by one.
- Nothing here checks whether a defect was caught by CI or landed. The distinction the brief asked
  for, gate-caught versus gate-escaped, was approximated by "was it fixed in a different pull
  request", which is a proxy and not the thing. Several Sonnet fix commits say "caught by CI" in their
  own subjects, which is the gates working and which this note's numbers count as defects anyway.
- **Session id is used as a proxy for a lane and it is not one.** A maintainer session and every
  subagent it spawns share an id, and the model can change inside a session. The claim "Sonnet's work
  is one session" is supported; the claim "one session means one set of conditions" is an inference
  and is not measured.
- This note was written by an agent inside the very session whose output is under review. That is
  a conflict of interest and no mitigation was applied beyond running measurements rather than
  offering impressions and reporting all three of them when they disagreed. Any tier that runs should
  not be scored from this session.
