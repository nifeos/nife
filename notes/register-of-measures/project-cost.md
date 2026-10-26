# What this project costs

*An appendix to [the register of measures](../register-of-measures.md). The name is provisional.*

## Three units, and no fourth that adds them up

Milestone 519 (what this project costs, tracked where it cannot rot) refuses a single headline
number. Three audiences want three different ones. The comparison against the verification
literature wants person-weeks. The argument that the method is getting cheaper wants tokens with a
dated price. calef's own budgeting wants cash. The columns are `human_person_weeks`, `lane_tokens`
with `lane_wall_clock_hours` and `price_per_mtok_at_date`, and `cash_spend`.

### The shape of the numbers

Cash to date is under a thousand dollars all in. Against it stand eleven person-weeks of one
experienced engineer's full attention. Milestone 519's block says "about ten", written two days
before this column existed. The column counts eleven calendar weeks, 2026W29 through the current
partial 2026W39, so the difference is arithmetic rather than a correction. At any plausible rate for
that person's time, **the human cost is on the order of 99% of the economic cost and the machines are
a rounding error.** The honest claim is not that software became cheap. The scarce input is still a
person; what changed is how much one person's attention can be made to carry.

### The chart is a ratio: tokens per milestone

A total would say the project was busy. Millions of tokens per milestone built says whether the
method got cheaper, which is the claim principle 2 of `AGENTS.md` makes. It fell from 307 in 2026W34
to 73 and 68 in 2026W36 and 2026W38. Four caveats come with it:

- A milestone is not a fixed unit, as [landed each week](landed-each-week.md) argues at length. Here
  it is the denominator, so the problem is worse. A week that built one build-flag fix and a week
  that built a three-architecture bring-up divide by the same 1.
- 2026W35's 1,021 is the highest, and that week built 8 milestones against 2026W36's 49. Both numbers
  are real. Whether the week was expensive or merely undeclared cannot be answered from here. The
  pull-request series in [landed each week](landed-each-week.md) is the first place to look.
- The current week is always understated. Its tokens accumulate all week and its milestones land in a
  burst near the end.
- Four weeks are marked as not captured rather than drawn as zero. See the next section.

### The deadline, and what was already gone when the capture started

Every other column in `notes/project-metrics.md` is computed from a git revision. History backfills
by reading the tree at a past commit, which is how the mutation census, the velocity column and the
coverage cells were filled in retroactively. Token and wall-clock records are not in git. They
live in the agent harness's session records under `~/.claude/projects/`, on one laptop, outside this
repository, and nothing promises to keep them.

The first capture ran on 2026-09-21 and read 477 record streams. It found 2026W34 through 2026W39.
It found nothing at all for 2026W29 through 2026W33, the first five weeks of the project, against a
first commit on 2026-07-12. Those weeks are recorded as absent, not zero, in the CSV and on the
chart. An absent week averaged in as a zero would be a false figure that ends up published.

2026W37 is captured but should be treated as suspect. It reads 12.2 machine-hours against 110 to 133
on either side, in a week whose roadmap says 13 milestones were built. Either that week really was
quiet, or its records were rotated away before anyone looked. Nothing can tell those apart after the
fact. That is the argument for the snapshot described in
[reading the weekly series](reading-the-weekly-series.md), under *How the series stays current*.

### What `lane_wall_clock_hours` is, and what it conflates

It is the sum, over every record stream, of the gap between consecutive responses. `script/effort`
drops any gap longer than thirty minutes. A stream is one session or one subagent lane, so parallel
lanes add. Four lanes working for an hour is four hours here, and a week can exceed 168. It is an
effort figure, not an elapsed one.

It conflates queueing with work. A lane waiting on the merge queue, a gate or a rate limit looks the
same here as one thinking. The thirty-minute cut is a judgment, not a measurement. A longer gap is a
session left open overnight, and counting it would put a sleeping laptop in the total.
`IDLE_GAP_S=600 script/effort` shows how much the choice moves.

### Two dollars, and which one is which

Milestone 519 is sharpest about this refusal. This project pays a fixed monthly subscription, so the
marginal cost of one more lane is zero dollars. `cash_spend` is what was actually paid: $200 a month
since 2026-07-12, plus $271.81 of hardware on 2026-08-15.

`price_per_mtok_at_date` is a shadow price, and it is a rate rather than a total on purpose. It is
that week's own token mix priced at vendor list rates. A reader who wants the shadow figure multiplies
it by `lane_tokens` and knows exactly what they have multiplied. **Quoting one of these two numbers
while implying the other is dishonest, so neither appears without its label.** Both rates and
purchases live in [`notes/project-metrics/ledger.md`](../project-metrics/ledger.md), which is appended
to by hand and never regenerated.

The blended rate moves with the mix, not only with prices. It runs from $0.30 to $0.78 per million
tokens across the captured weeks, while no published price changed. This tree's work divides roughly
half and half between `claude-opus-5` and `claude-sonnet-5` by token. A cache read also lists at a
tenth of an input token, so a week that cached well reads cheap.

$200 of real spend is outside this series, and that is not rounding. calef dates the subscription
2026-07-12, a Sunday, which is 2026W28. The first commit is 2026-07-13 UTC and the series starts at
2026W29. A week gets a row only when a commit fell in it, so `cash_spend` sums to $671.81 against
$871.81 actually paid. `script/metrics` prints the discrepancy on every run rather than folding it
into a neighboring week.

### `human_person_weeks` is a statement, and will stay one

It is 1.0 per calendar week, because calef works on nife full time. It is not a measurement. Nothing
will make it one without time tracking, and milestone 519 refuses that: a person required to log
hours stops volunteering the honest ones, and the metric is then worth less than what it displaced.
The only thing ever asked of him is a correction when the datum stops being true. The column is
carried through every rewrite, so a hand-edited week survives the next backfill. It is accurate to
the bucket the comparisons need and no better: seL4 at about eleven person-years plus nine more for
the proof, and Atmosphere at 1.5 person-years on verification.

## What a turn costs

A turn costs roughly the size of its context, not the size of its thought. This was measured on
2026-09-24 across every session record this machine holds. Cache reads are about 98% of all tokens
spent. Cache creation is about 1%, uncached input effectively 0%, and output about 0.1%. The mean
prompt per request runs between 230,000 and 359,000 tokens across the captured weeks. The largest
single request in every one of those weeks sits between 933,000 and 1,000,000, against a 1M window.

This does not claim that 98% is waste. Carrying context is what makes a long session coherent. It is
why a lane can be told a hazard once, and why a maintainer can resolve a conflict without re-reading
the tree. It is also why the method in `AGENTS.md` principle 2 works at all. The finding is about
where the bill goes, not whether the spending buys anything. It does say the first lever people
reach for is the wrong one: a cheaper model prices the output, and output is a tenth of one per cent
of the tokens.

The columns are flows and a peak, never a stock. `lane_turns`, `lane_cache_read_share_pct` and
`lane_context_per_turn_mean` are per-week events over that week's requests.
`lane_context_per_turn_peak` is the largest single request in the week. It is a maximum rather than a
sum, so adding two of them would invent a request nobody made. The deck has already paid for
confusing the two. [Landed each week](landed-each-week.md) records the same distinction under
*"this chart does not reconcile with the `Built` stock, and that is the design"*. It was found on
2026-09-23 by diffing a snapshot between two rows and getting a different answer from the flow. The
rule is cited here, not re-argued.

### Why the mean is charted, and what happened to the median and the peak

Peak is measured and not drawn, because it saturates. Every captured week peaks within 7% of the
model's 1M context window. So the peak answers one question, "did a session run to the wall this
week", and the answer is yes, every week. That is one bit, and a chart of it would be a flat line
read as a trend. It stays in `notes/project-metrics/context-per-turn.csv` and in `effort.csv`, where
a week that fell well short would show up as news.

Median was refused because of the instrument, not taste. `script/effort` survives a pruned
transcript by merging every column as a maximum against an older machine-local snapshot. That works
because a sum and a peak both only go up as `script/effort` sees more of a week. A median does not
combine that way. Two honest partial views of a week cannot produce its median without the
per-request numbers underneath, and those are what gets discarded. Storing a median would mean
keeping a per-request history, a much larger promise than this measurement has earned.

So the mean is the series. It is arithmetic over columns the file already carries: input plus cache
write plus cache read, over `requests`. That is why it is derived at write time rather than stored
twice.

### This panel is the checkpoint, by design

calef asked on 2026-09-24: *"How do we set a checkpoint to re-evaluate that will not be forgotten?"*
The question was about a decision deliberately deferred: whether to impose a session length limit on
the agent harness, and at what threshold. No threshold has been set. `AGENTS.md`'s *measure first,
then decide* is the reason. A threshold chosen before the data exists is no better than one chosen
under attachment, and one measurement is not a distribution.

A checkpoint in a calendar reminder or a conversation is in the medium this project keeps
abolishing. This panel is the mechanism instead. Nobody has to remember to look, because the number
arrives in front of whoever reads the deck (`notes/project-metrics.md`). Each week it moves or it
does not. A reader who has never heard of the deferred decision still sees the quantity it depends
on.

The decision itself has a home with a trigger, in `design/roadmap/proposals/`. The trigger is
*enough weeks to show a distribution, judged by reading these bars*, not a week count. A count would
be the same threshold chosen in ignorance that the tenet refuses.

### This instrument has the same deadline as the cost columns above

This is stated here as well as in the proposal, because a reader meets the number here.

- These records are not in git. They live under `~/.claude/projects/` on one laptop, and nothing
  promises to keep them. `script/effort`'s header records 2026W29 through 2026W33 as unrecoverable.
  Those are the project's first five weeks, already gone, and a per-turn context history has the
  same deadline.
- It cannot backfill. Every other column in the deck is computed from a git revision. These four
  cannot be, so a week nobody captured is a week nobody will ever capture.
- It measures one machine. With a second contributor, or work done anywhere but patagonia, this is a
  sample of one workstation presented as a project figure. It is honest today because there is one
  machine, and it stops being honest silently.
- `--snapshot` is what makes forgetting survivable. It is rung two of `AGENTS.md`'s ladder, not rung
  four. The plist is in
  [How the series stays current](reading-the-weekly-series.md#how-the-series-stays-current).

## Known limitations

These moved here from `notes/project-metrics.md`'s `BUGS` on 2026-09-24, when that page passed its
3,000-word budget; they describe the columns this appendix argues, so this is where a reader of them
arrives.

- **The cost columns can stop updating and the charts will not say so.** They will simply stop
  gaining weeks, and the charts draw an absent week as absent, which is correct and is also exactly
  what a dead capture looks like. `script/cadence-check` is the thing that speaks, and it speaks on
  patagonia through `helpers/trunk-health.sh`, which inherits that watcher's own recorded gap: a
  machine asleep is a watcher not watching.
- **`lane_tokens` counts this project's whole session, not its lanes.** Every response in a record
  stream filed under the nife project directory counts, including a maintainer answering a question,
  a review, and this page being written. It is the cost of the project, not the cost of the code,
  and the name is narrower than the thing.
- **Machine effort cannot be attributed to a milestone**, so the chart's ratio is a weekly average
  over everything that happened rather than a per-milestone cost. The raw material for the join is on
  disk (every record carries a `gitBranch`, and a lane's branch is named for its milestone).
  The join is deliberately not built: a branch is not a milestone, and a wrong attribution is worse than
  none.
- **`price_per_mtok_at_date` restates old weeks at the newest rate in the ledger.** The ledger is
  appended to, and nothing reads a rate as of a week; the last row for a model wins. A rate change
  would therefore re-price history, which is the same restatement hazard this page opens with and is
  worse here because a dollar figure reads as a measurement.
- **$200 of the subscription is outside `cash_spend` and stays outside it.** It was paid in 2026W28
  and the series has no row for that week, because no commit fell in it. `script/metrics` prints the
  amount on every run; nothing folds it into a neighbor, because that would put money in a week it
  was not spent in to make a column sum tidily.
- **`lane_context_per_turn_peak` is pinned to the model's context window and therefore says less
  than it looks like it says.** Every captured week is within 7% of 1M. It answers whether a session
  reached the wall, not how wide the spread is, and the mean beside it is the column with a trend in
  it. The distribution between the two is not recorded anywhere and cannot be recovered once the
  transcripts are gone.
- **The four `lane_*` context columns see one machine, the same one the cost columns see.** They are
  a fair figure for this project today because there is one workstation. Nothing detects the day that
  stops being true; it would show up as a drop that reads like an improvement.
