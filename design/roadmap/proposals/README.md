# What this directory is for

This is the inbox for identified work that does not have a number yet. It is prose, not an
index: nothing here is generated, and no gate reads this file. The list of what is currently in the
pile is `script/roadmap --proposed`, which reads the files themselves and cannot go stale.

## Why it exists

A lane that finds work it is not doing has to leave it somewhere a script can see, or it is gone.
Milestone 247 (follow-on work named by a finished milestone goes nowhere) created this directory
after the third time a finding died in a pull request body. `AGENTS.md` states the rule it
serves: identified work leaves the lane in a tracked form, or the merge waits.

The number is the thing a lane cannot mint, because concurrent lanes cannot see each other and
two of them reaching for "the next milestone number" is a collision nobody notices until merge. So a
proposal carries no number, and an integrator assigns one at promotion. That is the whole reason
this directory is separate from `design/roadmap/` rather than being part of it: a lane can add to
the roadmap here without coordinating with any other lane.

## What a file here looks like

`<slug>.md`, lowercase and hyphenated, no number. Inside, the frontmatter a numbered block opens
with, `status: PROPOSED` and the date it was written as `raised`, checked by `script/roadmap --check`:

```
---
status: PROPOSED
raised: <YYYY-MM-DD>
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A title, with no milestone number in it

Who raised it and what they were doing at the time.
```

`notes/roadmap.md` says what each dependency field means.

A proposal that is a correction's action item carries one more line, on its own directly under the
title: `COE action item: notes/coes/<file>.md, action <N>`. The correction's action table links back
to it, and `script/roadmap --check` fails either half without the other (`helpers/coe_actions.py`).
The line moves with the file at promotion.

A proposal is under the same limits as every other document: 3,000 words, and §213 (writing
standards)'s sentence and bold rules, checked by `helpers/prose_ratchet.py` from the day it is
written. calef ruled this on
2026-10-06 (UTC): a promotion carries no exemption, so a block promoted from here
needs no exception marker. A proposal that will not fit takes an appendix, like any block.

`raised` is not decoration. A proposal nobody promotes is the same burial in a new place, and age is
the only tell a script has.

## What happens to one

Promotion is how a proposal reaches any disposition at all, including being refused or found
moot: this directory carries exactly one status by design, so nothing can be retired in place. An
integrator gives it a number, `git mv`s it up a directory as `NNNN-<slug>.md` (four digits,
zero-padded), sets its `status`, and adds
`promoted_from: <slug>`. `script/roadmap --check` holds that field to the rule that the proposal is
gone. The block that named the work updates its `**Proposed.**` bullet to `**Milestone N.**`, so
neither record orphans the other.

A proposal is not a promise to build. It is a claim that somebody found something real and wrote
it where the next person can find it. DECISIONS §71 (a limitation is promoted when it stops being a
fact and becomes a plan) draws the same line for `BUGS` entries, and it is deliberate. A pile that
implied commitment would be kept short by not writing things in it, which is the failure this
directory exists to prevent.

## Why this file is here at all

Partly to say the above. Partly because git does not track an empty directory. On 2026-09-20
every proposal in it was promoted at once, and the directory vanished from the index. Git's rename
detection then read two lanes' new proposals as belonging in `design/roadmap/` instead, quietly
moving them a level up. A committed file keeps the inbox on disk and the intent legible.

`README.md` is exempt from the slug rule for that reason, in
`helpers/roadmap_proposals.py`. It is the only exemption, and it is a file rather than a pattern so
that a second one has to be argued for.

*Name: recorded (milestone 247). Milestone 247 (follow-on work named by a finished milestone goes
nowhere) created this directory as the inbox for work without a number. The stems here are not tracked by
`script/names`: a proposal's slug becomes its milestone's, and calef ruled milestone titles drafts
on 2026-08-04.*
