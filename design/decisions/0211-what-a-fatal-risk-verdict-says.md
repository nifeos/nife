---
status: DECIDED
raised: 2026-09-23
decided: 2026-09-23
ratified_by: calef
---

# 211. What a fatal-risk verdict says, and what the chart can plot as a result

The field is Experiment status, with values
RUN, NOT-RUN, CANNOT-RUN. Raised by calef, 2026-09-23, reading
`notes/project-metrics/fatal-risks.svg`: it seems like the wrong graph, because it plots tested
against untested and all nine risks have now been put to an experiment, so it is a flat line at
nine. He asked for risks by status instead. Implementation on branch
`maintainer/experiment-status-ratified`. *(Minted provisionally at 208; moved to 209 on
2026-09-23 when `maintainer/installing-is-granting` landed §208 (installing a package is
granting it, and the activation set is versioned) first; moved again to 211 the same day when a
second lane, `maintainer/state-handoff-is-optional`, held §209 (state handoff is an opaque blob
over a granted frame, and it is optional) and §210 (a correction of error, and its action items
are decisions, proposals or milestones) was already taken on `main`.)*

## What is being decided

What a `**Status:` line in `design/fatal-risks.md` states, in what vocabulary. The chart is
downstream of that and cannot be fixed first: a status series needs a closed set of statuses, and
there is not one. Three questions ride on the answer.

1. What does a verdict assert: that an experiment happened, what it found, or both at once?
2. What is the legal set of words, and who may add to it?
3. What does `notes/project-metrics/fatal-risks.svg` plot, given the answer.

## The evidence, which is that there is no vocabulary

### The five words the script reports

`script/fatal-risks` reads a `**Status: WORD` line per risk. Its output today:

| risk | title, abbreviated | reported word | dated | minted by | said provisional |
|---|---|---|---|---|---|
| 1 | only software written for nife runs on nife | `RUN` | 2026-08-31 | 2026-08-31, risk 1's own lane | no, it was the first |
| 2 | the proofs prove trivia | `RUN` | 2026-08-30 | same word | n/a |
| 3 | the tests do not test anything | `MEASURED` | 2026-09-19 | 2026-09-19, risk 3's lane | no |
| 4 | the per-crossing cost cannot be engineered away | `NOT` | 2026-09-23 | pull request #1129, `risk/4-the-crossing-cost` | no |
| 5 | it cannot be made reliable on multicore | `UNRUN` | 2026-09-23 | pull request #1127, `risk/5-multicore-reliability` | **yes**, in the entry itself |
| 6 | a confined userspace driver cannot drive real hardware | `RUN` | 2026-09-17 | same word | n/a |
| 7 | the confinement claim is false | `RUN` | 2026-09-23 | same word | n/a |
| 8 | nobody needs it | `UNTESTED` | 2026-09-23 | pull request #1128, `risk/8-nobody-needs-it` | no |
| 9 | the HAL is a fiction | `RUN` | 2026-09-23 | same word | n/a |

Three of the five words were minted on one day, 2026-09-23, by three separate lanes, none of
which could see the other two. That is the lane isolation working as designed and the naming rule
(`AGENTS.md`: anything global to the tree is the integrator's, and a lane ships a provisional name)
half-applied: one of the three said provisional out loud, two did not.

### The words are not even the words

Three findings that the table above hides, each of which is its own argument that the vocabulary is
not a vocabulary.

**`NOT` is a parsing artifact.** Risk 4's file says `**Status: NOT YET, 2026-09-23.`. The script's
`STATUS_LINE` regex is `\*\*Status: ([A-Z]+)\b`, so it captures `NOT` and drops `YET`. The report
prints a word nobody wrote. A two-word status is not illegal today because nothing says what is
legal.

**There is a sixth word, and the script cannot see it.** Risk 7 carries two status lines:
`**Status: RUN, 2026-08-31` and, eighteen lines later, `**Status: AUDITED, 2026-09-17`. The script
takes the first match per section, so `AUDITED` is invisible to every consumer. Risk 5's entry, the
one lane that stopped to think about vocabulary, writes that *"this file's vocabulary is `RUN`,
`MEASURED` and `AUDITED`"*. The script recognises `RUN` and `MEASURED` and has never heard of
`AUDITED`. The file's own statement of its vocabulary and the gate over it disagree.

The running order table is a third vocabulary. The same document's closing table states each
verdict again in free prose: `**RUN 2026-08-30: amber.**`, `**RUN 2026-09-17: GREEN.**`,
`**RUN 2026-08-31, extended 2026-09-16, AUDITED 2026-09-17.**`, and for risk 8
`**none, and none available.**`. Risk 5's row carries no verdict word at all. Two places in one
file state the same fact in two shapes, and `script/fatal-risks` reads neither against the other.

### What the words actually mean, sorted

Laid side by side, the five (six) words answer four different questions:

| question | words answering it |
|---|---|
| did an experiment run? | `RUN`, `UNRUN`, `NOT YET` |
| was the result quantified? | `MEASURED` |
| was the result adversarially reviewed? | `AUDITED` |
| can the experiment be run at all? | `UNTESTED` |

And the thing a reader most wants, what it found, is in none of them. It is in a separate
capitalized word that follows on the same line for three risks (`GREEN` on risks 1 and 9, `AMBER`
on 2 and 3) and in an English sentence for the rest: risk 6's *"all three of its parts are measured
on silicon"*, risk 7's *"it found the thing this risk exists to find"* and *"a qualified yes with
one exception"*.

### Nothing stops any of this

`script/fatal-risks`' only test of a status word is one expression:

    verdict = sm.group(1) if sm and sm.group(1) in ("RUN", "MEASURED") else None

It is a filter for the two words that mean *the experiment happened*, used to decide whether check
1 (an experiment that ran must name a milestone that started) applies. Anything else parses as
`None` and is silently exempt. A lane inventing a word costs nothing and is told nothing, which is
exactly what happened three times in a day.

### The chart, and why its refusal has changed rather than expired

`script/metrics`' `fatal_risks()` counts a risk as tested when its section has any `**Status:` line
at all, and produces `fatal_risks_tested` and `fatal_risks_untested`. All nine have a status line as
of 2026-09-23, so the series is nine and zero and stays there. calef is right that it carries no
information.

That function already refused a verdict series, in its own docstring, and the refusal was correct
when written:

> A verdict column was considered and refused: three of the four statuses written so far say GREEN
> or AMBER in capitals, and risk 7's says neither ("RUN, 2026-08-31, and it found the thing this
> risk exists to find"), so a colour series would be reading a sentence and guessing.

**The objection has moved, not lapsed.** The status words parse cleanly now, so a series over them
would not be guessing. It would be charting a set that mixes *did it run* with *was it quantified*
with *can it run*, which is a chart whose categories are not alternatives to each other. Guessing
was the old problem. Incoherence is the current one, and it is worse, because a wrong reading of a
sentence is visible as a wrong reading and a category error looks like data.

## The decision

Axis A, split into two fields. The status field is renamed Experiment status and takes three
values: RUN, NOT-RUN, CANNOT-RUN, gated by an enumeration in `script/fatal-risks`.
These are enforced, so that adding a fourth value forces a ratification rather than happening by
accident.

**Axis B was considered and not adopted.** The proposal could not keep to its own three values,
writing *"inconclusive leaning real"* twice and *"looks real in part"* once, which is three hedges
in nine rows. That evidence is what axis B was written to find, and finding three that the
framework cannot express is evidence that the finding does not compress into one word yet. The
existing findings (`GREEN`, `AMBER`, `MEASURED`, `AUDITED` and the sentences around them) stay in
the prose.

Why axis A won on its own merits. It needed no hedges; it partitions the nine cleanly. And
`CANNOT-RUN` is exactly the value the current tested-versus-untested chart cannot express for
risk 8 (which is untestable by this project's own policy until milestone 198 lands). One field,
three values, one question each value answers: does an experiment have a status, and what is it. The
chart can then ask a real question: over time, how many risks remain unrun, and for how long.

The proposal's table was read from entries that already say all this. Applied to the nine as they
stand, using only what the entries already say:

| risk | Experiment status |
|---|---|
| 1 | RUN |
| 2 | RUN |
| 3 | RUN |
| 4 | RUN |
| 5 | NOT-RUN |
| 6 | RUN |
| 7 | RUN |
| 8 | CANNOT-RUN |
| 9 | RUN |

## The one-word alternative, and its refusal

Keep a single verdict word and define the legal set precisely. Something like `RUN-GREEN`,
`RUN-AMBER`, `RUN-RED`, `UNRUN`, `UNTESTABLE`, gated by an enumeration in `script/fatal-risks`.

**Its argument is real and is not a rounding error.** A single verdict is what a reader remembers,
and this file exists to be remembered: nine claims, nine answers, and the entire point of a
falsification list is that you can hold the state of it in your head. Two fields is potentially
eighteen cells. A reader scanning it has to do the join themselves, and the join is exactly the
thing the one-word version does for them.

And the second half of its argument is sharper: two fields may be two things nobody fills in
honestly. This tree has measured that failure. `script/fatal-risks` exists because risk 2 carried
`RUN, 2026-08-30. AMBER` for eleven days while the milestone it named read `NOT-STARTED`. A field
that is easy to leave at its last value is a field that goes stale, and doubling the fields doubles
the surface. Under the one-word shape there is exactly one thing per risk that can be wrong, and a
reader comparing the word against the prose beneath it catches a lie in one glance.

**Why it was not chosen.** It loses on risk 8 and it loses for a reason this file itself calls the
most dangerous state a fatal risk can be in. `UNTESTABLE` is not a point on a scale that runs from
green to red; it is a statement that the scale does not apply, because the observation is gated
behind a precondition calef set and milestone 530 (name a customer, or admit the ranking function
has nothing to rank) ruled on, and milestone 198 (a package manager, and the trivial install that
makes a second customer possible) is what would lift it. Put it on the same axis as `RUN-GREEN` and
it renders as one more colored band, which is the chart telling a reader that a risk nobody can
look at is a kind of result. Risk 4 has the same shape one step less severe: it is the
best-measured entry on the list and has no finding, and a one-word scale must either call that
amber (false, it is not a finding) or call it unrun (false, milestone 168 (a multi-tasking workload
benchmark, the number that would decide the event-kernel question) is built, gated and rehearsed on
three architectures).

Two further options, refused shorter. *Keep the existing five and document them* is the cheapest
thing on the table and is refused because `NOT` is a truncation of `NOT YET`, `AUDITED` is a sixth
word the script cannot see, and documenting a set that mixes four questions ratifies the category
error rather than fixing it; the one thing it has going for it is that it is honest about what the
file says today, and that honesty is available under the chosen shape. *A numeric
confidence*, a probability per risk, is refused because nothing here is estimated that way and a
number invites arithmetic across nine claims that share no scale, which is the overclaiming this
file's rule 1 exists to prevent.

## What it costs

Measured against the tree as it stands, not asserted.

- Nine entries rewritten, one status line each, plus risk 7's second line and the nine rows of
  the running order table, which states every verdict a second time in free prose. Call it eleven
  status lines and nine table cells. The prose underneath each entry does not move: every value in
  the proposal's table above was read straight out of text that is already there.
- **`script/fatal-risks`: a new check, not a rewrite.** Today `STATUS_LINE` is one regex and the
  only validation is a two-element membership test used as a filter. The work is an enumeration of
  legal values, a parse of the second field, and a failure when a status names a word outside the
  set. The selftest fixture convention is already there (`--selftest`, one fixture per check), so a
  new check costs a fixture. The existing `experiment-ran` and `experiment-pending` checks keep
  working: they need to know *ran versus not ran*, which is axis A exactly, and under the current
  shape they infer it from a two-word allowlist.
- `script/metrics`: replace `fatal_risks()` and one chart spec. The function is nineteen lines
  and its docstring is the refusal quoted above, which has to be rewritten rather than deleted,
  because the reason it was right in 2026-09 is part of why the new shape is what it is. The series
  names `fatal_risks_tested` / `fatal_risks_untested` are in the column allowlist at line 208 and in
  the chart table; both change.
- History. The chart is drawn from a metrics series over time. Re-deriving axis B for past dates
  means reading past revisions of nine entries, and it is not obvious it is worth doing: the honest
  option is to start the new series at the ruling date and say so on the chart, the way the effort
  chart already marks weeks with no surviving records rather than zeroing them.
- **One thing that is not a cost.** Nothing outside these three files reads a status word. The
  vocabulary is local to `design/fatal-risks.md` and its two readers.

## The effort test, answered out loud

§92 (a caretaker is supervised by the client it serves) carries the test this tree applies to any
recommendation: *would I still choose this if both options were the same amount of work?*

Yes. The one-word option is the cheaper of the two by a small margin (one field to parse, nine
lines to rewrite rather than eleven, one chart rather than two), and cost is not what decides it.
Risk 8 breaks a single scale on the merits: a risk that cannot be observed is not a shade of a risk
that has been. If the two shapes cost the same I would still split them, because the split is what
lets the chart show a risk going unanswered for a reason, which is the thing this file is for.

The place cost *does* decide something, said plainly: the history question. Starting axis B at
the ruling date rather than re-deriving it backwards is an argument from effort, and it is recorded
as one.

## What is an architect's

The words. `run`, `not run`, `cannot run yet`, `looks real`, `looks false`, `inconclusive` are
provisional and are a naming decision under `AGENTS.md`, whichever shape wins. They are also
unusually load-bearing for names: they are the vocabulary a reader uses to say whether this project
should stop, and the one-word alternative's whole argument is that a reader remembers one word.

The shape, because two agents already disagreed about it implicitly by minting three words in a
day, and because a chart published from it is a fact that leaves the machine.

And a smaller one that is still an architect's: whether `MEASURED` and `AUDITED` survive as anything. They
are real distinctions that three entries currently carry, and the proposal drops them.

## What is blocked until this is answered

- `notes/project-metrics/fatal-risks.svg` stays a flat line at nine.
- Any lane touching a risk entry mints another word, as three did on 2026-09-23.
- `script/fatal-risks` keeps a status check that is a two-element allowlist with no enumeration
  behind it, which is milestone 275 (a gate that diffs `design/fatal-risks.md` against the roadmap
  it cites) leaving its own vocabulary ungated.

## BUGS

- **This section proposes a vocabulary and does not prove anyone will fill it in.** The one-word
  alternative's second argument, that two fields are two things that go stale, is not answered here.
  It is answered by a gate or it is not answered, and the gate is named in the costs above rather
  than designed.
- Axis B's values are read out of nine prose entries by one agent, on one evening. Risk 7's
  "looks real in part" and risk 2's "inconclusive leaning real" are readings, and a reader who
  disagrees with either is disagreeing with this section rather than with the file.
