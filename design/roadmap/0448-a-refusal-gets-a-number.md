---
status: BUILT
raised: 2026-09-20
built: 2026-09-20
---
# 448. A refusal gets a number, a status, and a condition that would change it

Built 2026-09-20, on calef's ruling of that day that refusals which name work are milestones rather
than bullets, backfilled so the inventory is complete. He ratified the status word `REFUSED` in the
same ruling.


## The defect, measured

This tree carried 140 `- **Refused.` bullets across 98 milestone blocks, and exactly 2 of them
named a milestone.** The bullets sit under `## Follow-on`, which is usually the last section of a
BUILT block, which is the last place anybody looks. They are read once, on the day they are written,
by one person.

That is the same burial `## Follow-on` was built to stop one level up, arriving through the gate
itself. That gate routes an intention into a tracked form: a milestone, a proposal, a recorded
limitation. It has no word at all for a **decision not to**, so `**Refused.**` became the
disposition that resolves to nothing, and a refusal that named real work had nowhere to live.

The cost arrived on 2026-09-20 and it is concrete. Milestone 164 (x86_64 userspace can't build
`aes`) refused "Route 2, an SSE-enabled x86 userspace target" in September, for a good reason:
nothing needed FP state to compile `aes`. The reason eroded quietly afterwards. Milestone 442 (a
crypto provider `rustls` can use on all three bare-metal targets) needs five force-soft build flags;
an AVX2-detecting crate dies in ring 3 with `vector 6 (invalid opcode)`; and the bitsliced AES cost
that block declined to measure is starting to have the workload it said it lacked. Nothing compared
any of that against the refusal, because a refusal had no home that anything reads. calef reopened
it by hand, in conversation, which is the medium AGENTS.md exists to abolish.

## What shipped

A status. `REFUSED` joins the vocabulary in `script/roadmap` and in design/roadmap/README.md:
considered and deliberately not taken, where the thing not taken is work. It is not in
`STARTABLE`, so `--ready` cannot offer it; it never reaches the gate parse, so it is outside "the
milestones that are not built"; it is not in `ANSWERS`, because a block nothing will build owes no
`## Follow-on`; and it is not in `DATED`, because the Built column means the date a milestone turned
BUILT and a refusal date in it would be a lie in the field whose whole job is not lying. The date it
was refused goes in the status line's prose, the way `SUPERSEDED` already writes it.

A required section, which is the part calef asked to be *ensured*. His words, the same day:
*"Ensure we capture what would change for a milestone to no longer be refused."* A condition asked
for in prose is rung four of AGENTS.md's ladder, and this tree's own evidence is that rung four does
not hold: milestone 164's refusal stated its condition perfectly ("The number is owed when an
x86_64 workload touches the crypto path") and still went stale, because nothing re-read it. So every
`REFUSED` block carries one `## Revisit` section and `script/roadmap --check` fails without it. Its
bullets open with one of three words:

- `**Condition.**` what would make this worth reopening. **Not a promise to build**, which is what
  DECISIONS §71 (a limitation is promoted when it stops being a fact) refuses to let a limitation
  become. It is the difference between a dead end and a door with a bell on it.
- `**Nothing.**` refused permanently, on a principle rather than on circumstance.
- `**Unstated.**` the original names no condition and none can honestly be inferred. It is meant to
  read as a gap somebody could close.

`Unstated.` is the load-bearing one and it is deliberately cheap to write. An author with no
honest condition and no way to say so will invent one, and an invented condition is worse than a
blank, because it is a bell that fires for a reason nobody meant. Silence and "we thought about it
and nothing would change our minds" look identical to the next reader and mean opposite things, so
they are different words and both pass.

A bell. `script/roadmap --revisit` flags a `REFUSED` milestone whose condition names a milestone
that has since turned BUILT. That is rung two: it fires without anybody remembering.

## The triage, and the criterion it produced

A milestone implies work somebody could take, so not every refusal earns one. The brief's
starting criterion held up, and contact with 140 bullets sharpened it into three tests a refusal
must pass **all** of:

1. It names work somebody could execute (a route, an artifact, a mechanism), rather than a
   decision about how one lane shaped its own diff.
2. The thing refused still exists as a possibility: its subject was not deleted and its premise
   not dissolved.
3. It is **not already homed**: no proposal file, no open pull request, no other milestone, no entry
   in `script/names`, no section in `design/decisions/` that owns the reasoning.

A fourth test fell out of the data and is worth naming, because it is the one that stops this
inventory inflating: a refusal that is terminal on principle stays a bullet. Taking the *debug*
UART out of the kernel, opening a recovery device read-write, dropping `verify` from the required
checks: building any of those would be wrong rather than premature, and minting a number for one
creates an identity that can never be BUILT and invites somebody to finish it.

The counts, against the maintainer's estimate of roughly a third:

| Category | Bullets | What happens to it |
|---|---|---|
| Route refusal, minted | 48 | milestones 449 to 490, 42 blocks (five subjects are refused in more than one place) |
| Terminal on principle, or a lane's own scoping call | 67 | stays a bullet in the block that made it |
| Name refusal | 9 | `script/names` already homes it at the thing it was refused for |
| Moot: the subject was deleted or the premise dissolved | 7 | stays a bullet; there is nothing to revisit |
| Already homed elsewhere | 7 | stays a bullet; a proposal, a pull request or a decision owns it |
| Dependency refusal | 2 | belongs in `design/decisions/` under §46 (thin primitives or whole subsystems); named in the lane's report |

48 of 140 is 34%, which is the estimate to within a rounding error. 42 blocks rather than 48,
because five subjects were refused more than once, in eleven bullets between them. The capability
derivation tree is refused in three separate blocks with one argument; legacy INTx routing, a
self-hosted CI runner, a gate on harness quality and a stack size are each refused twice. Those
repetitions are the clearest single piece of evidence for this milestone: a reader meeting one of
them cannot tell the tree's standing position from one lane's scoping call, and neither could the
second lane, which is why it refused the same thing again.

**`Nothing.` was minted with zero users, and that is a finding rather than an oversight.** Every
refusal that named executable work turned out to have a condition, once somebody sat down and asked.
The refusals that were permanent on principle are exactly the ones the fourth test keeps as bullets,
so the word is there for the block that needs it and the backfill did not.

## What the bell found

Run against the backfill, `script/roadmap --revisit` reports **no refusal whose condition names a
milestone that has since turned BUILT**, and the same run prints why that is weaker evidence than it
sounds: 2 of 42 conditions name a milestone at all. The other 40 are phrased as a workload, a
measurement, a customer or a piece of hardware, and nothing mechanical can read those. The reach is
printed on every run for exactly that reason, and it is written in the tool's own `BUGS` rather than
in a claim.

Its first run found two things and both were false, which is worth recording because the fix is
the check's shape rather than a tolerance. A condition almost always names the block that refused
it, since the reasoning being quoted is that block's, and those citations are context rather than
triggers. So the milestones named in a `REFUSED` block's status paragraph, which are by construction
the ones that refused it, are excluded. After that the run is clean and the two remaining citations
both point at other `REFUSED` blocks, which is the bell wired to something that can ring rather than
to something that already has.

The stale refusal this lane was asked to look for was already known, and it is milestone 461 (an
SSE-enabled x86_64 userspace target): its block opens already rung, because calef rang it by hand on
2026-09-20. No second one turned up. Two came close enough to name: milestone 460 (a riscv64 arm for
the CPU-instruction entropy source) rests on a fact about an ISA at a date, which is the shape most
likely to erode without anyone noticing, and milestone 463 (an MCFG whose first bus is not zero) is
the rare refusal whose condition enforces itself, because the kernel checks the value and refuses
loudly rather than adjusting.

## The denominators, before and after

Adding several dozen blocks moves every count a stranger quotes, and this says so plainly so that
nobody later reads the jump as work appearing.

| | Before | After |
|---|---|---|
| Milestones with a block | 444 | 487 |
| BUILT | 224 | 225 |
| Not built, classified by gate | 217 | 217 |
| Ready to start | 120 | 120 |
| REFUSED | 0 | 42 |

**The 42 are not a backlog and no count treats them as one.** "Ready to start" is unchanged, which
is the number that matters: no lane is offered a single piece of new work by this milestone.
notes/project-metrics.md carries the same before and after, because its chart reads the index rows
and would otherwise show 42 milestones appearing out of nowhere.

## What this deliberately does not do

**`BUGS` sections and DECISIONS §71 are untouched.** A `BUGS` entry is a present defect a reader
meets at the feature; a refusal is a past judgment about work. The FreeBSD posture §71 records is
working, and this is a separate mechanism beside it rather than a replacement. Nothing here makes it
more expensive to write an honest limitation, which is the failure mode that would cost more than
the burial does.

**It does not gate the quality of a condition**, only its presence. See the `BUGS` section in
`script/roadmap`: the check cannot prove a condition is true, honest, current, or thought about for
longer than it took to type.

## Revisit-condition vocabulary is provisional

`REFUSED` is ratified (calef, 2026-09-20). The section name `## Revisit` and the three disposition
words `Condition.`, `Nothing.` and `Unstated.` are this lane's, shipped provisionally and named as
such, in the shape AGENTS.md asks for: a provisional name converts a naming decision from expensive
to cheap by saying out loud that it is not settled.

## Follow-on

- **Decision.** Two dependency refusals belong under §46's pricing rather than in a roadmap block,
  and a lane may not mint the section: `comrak` for GFM tables and `ratatui` for the pager, both
  refused by milestone 40 (design/roadmap/0040-documentation-service.md). The reasoning is a
  dependency judgment and the record that judges dependencies is
  `design/decisions/0046-dependency-rule.md`, which is where the maintainer should mint them.
- **Recorded.** In `script/metrics`: `MILESTONE_STATUSES` did not carry `SUPERSEDED` before this
  milestone and therefore undercounted seven blocks in every weekly row, with `milestones_total`
  short by the same seven. `REFUSED` and `SUPERSEDED` were both added and the history restated, so
  the gap is closed and named here because the numbers it produced have already been quoted.
- **Recorded.** In `script/roadmap`: `--revisit` reads prose, so it fires only on a condition that
  cites a milestone number, which is 1 of 42 today. Every other condition in this backfill is
  phrased as a workload, a measurement, a customer or a piece of hardware, and nothing mechanical
  can read those. The two counts print on every run so the reach is visible rather than assumed.
- **Recorded.** In the status vocabulary, now `notes/roadmap.md`: the table did not list `SUPERSEDED`
  either, seven blocks after the word was minted. `REFUSED` was added with its own row and its own
  paragraph; `SUPERSEDED`'s row is still missing, and writing it means putting words in the mouth of
  a decision this lane did not make.
- **Refused.** A gate that a `REFUSED` block's condition is still *true*, as against present. No
  check can read a sentence about a workload and say whether the workload exists, which is the same
  measurement AGENTS.md prices for `git grep -w TODO` at an 82% false-positive rate and the reason
  `## Follow-on` checks a section rather than prose. The reach that a gate does have is `--revisit`,
  and its limit is written in the tool's own `BUGS`.
- **Refused.** Sweeping the 68 terminal refusals into blocks anyway, to make the inventory a round
  number. An identity that can never be BUILT invites somebody to finish it, and a roadmap that
  names 140 things nobody should build is worse at answering "what is there to do" than one that
  names 42.

## Index row

140 refusal bullets sat in finished blocks where nothing reads them, and exactly two named a
milestone; one of them went stale in a fortnight and calef caught it by hand. `REFUSED` gives a
refusal that names work a number, a status excluded from every backlog count, and a `## Revisit`
section the gate requires, so a refusal with no stated condition is unrepresentable in a passing
tree.
