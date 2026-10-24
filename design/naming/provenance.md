# Where a name's provenance lives

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds milestone
115's mechanism: why the record is derived, the states, the `Name:` block forms, the numbers, and
worked `script/names` examples. It exists to verify or challenge the main page. A reader who only
needs to name, ratify or rename something should not have to open it. The directory
`design/naming/` and this file's stem are provisional names; naming is an architect's.*

## Where a name's provenance lives (milestone 115)

The refusals are the valuable half. A ratified name is visible in the tree, because it *is* the
name. A refused one is visible in no file at all unless something records it. The person who most
needs it is the person about to propose it again.

That is not hypothetical. A lane proposed `system_builder` for the crate milestone 96 (one init)
extracted, the maintainer endorsed it, and calef overruled it to `system_initializer`. Only
afterwards did anyone find that milestone 63 (directory and package names) had already refused
`system_builder`, for a reason still true: `components/src/builder.rs` called itself "a minimal
init: the system builder" then, so two programs would claim one phrase. The refusal existed in one
table cell inside one milestone block, invisible at the moment it was needed. A blind rename then
swept the old name out of that very row. The record of the refusal was nearly destroyed by the
rename it should have prevented.

That program was retired on 2026-09-14 (milestone 295 (retire `components/src/builder.rs`)), and the
refusal stands unchanged. A refusal records why a name lost on the day it lost. One rewritten every
time the tree moves is one nobody can check.

The record is derived, not maintained. Not one ratified-names table in this file: calef refused it
on 2026-08-04 for scaling the way the original `DECISIONS.md`
and `design/roadmap.md` scaled, and size is the smaller half of that argument. The conflict shape
is the real one: every lane that adds a name would edit one file, which is exactly what produced
three section-number collisions in a day.

So:

1. Provenance lives at the name. A crate's `lib.rs` header, a program's module doc and a
   `script/` entry point's comment block each carry a `Name:` block. It says when the name was
   ratified and what was refused. Adding a name touches exactly one file, so two lanes naming two
   things cannot collide. It also puts the refusal where the next proposer is already reading.
2. `script/lint` checks presence, never content. 126 names then, and a name with no block fails
   the build.
3. `script/names` is the table, computed on demand, so it cannot drift from the tree. Same family as
   `script/roadmap`, `script/decisions` and `script/catch-up`.
4. The maintainer writes the block at ratification, in the same commit that applies the name,
   while the alternatives are still in mind.

### A document's block is a paragraph, not frontmatter

A document under `design/`, `notes/` or `briefs/` carries its block as a paragraph under its title,
in the grammar of every other kind (calef, 2026-10-24 UTC, who left the format to the lane). So
adding a note touches the note and one index line. A shared README is where concurrent lanes
conflict, and code names `notes/README.md`, so a change to it never takes CI's prose-only skip.

Frontmatter, with fields for status, date and minter, was the alternative. It lost on four counts:

- `script/names` parses the paragraph already, for every kind, and a directory's README block is
  the same paragraph. Frontmatter would be a second grammar for one kind.
- A refusal is prose, with backticks and parentheses, and a ratification quotes calef. YAML would
  need every one quoted, and the date and refusals are derived from the paragraph anyway
  (`name_provenance.classify`).
- GitHub renders frontmatter as a table above the title. The paragraph reads as a line under it.
- No note carried frontmatter. Dozens already carried the paragraph, unread until it became the
  rule.

Where a README paragraph and the document's own block both exist, `script/names` reads them
together and fails statuses that disagree. `helpers/document_name_migrate.py` lists and moves the
README form; its header holds the rules, and `helpers/name_census.py` carries the BUGS line for
the reading that goes when the sweep after milestone 929 (comments in the Linux style across first-party Rust) finds
nothing left.

### The three states

calef works back through the existing names over time. So the record has to hold which ones still
want him, not only what is known. The state is the first word of the block:

| State | Means | What it costs to clear |
|---|---|---|
| **unrecorded** | nothing in the tree or its history says why this name was chosen | research, and then a ruling |
| **recorded** | the tree argues the name somewhere (a milestone block, a decision, a header) and calef never ruled | a ruling |
| **ratified** | calef ruled, with the date and what was refused | done |

The first cut of this mechanism had two states, and `unrecorded` was doing both of the first two
jobs. "Nobody in this tree can say why this is called that" and "here is the argument, nobody ever
signed it" are different amounts of calef's time. A worklist that cannot tell them apart is a list
rather than a plan.

The criterion, stated so that a reader can disagree with a case rather than with a mystery: a name
is `recorded` when something *outside its own block* argues for the name it has. Three corollaries
did most of the work in the 2026-08-04 triage:

- A `Name:` block is never its own evidence. All 126 were written in one week by this milestone.
  Reading them as history would make the record prove itself, and every name `recorded` by
  construction. So a `recorded` block must cite somewhere else, and the citation is checked for
  being present.
- "It got here first" is not a reason. `design/naming.md` exempts `abi` from the `*_proto` rule
  because it "predates the suffix". That explains why the crate is not called `syscall_proto` and
  says nothing about why it is called `abi`. Counting an exemption as an explanation would let every
  old name in the tree explain itself.
- An assertion is not an argument. The BUGS section below says of `virtio` that "the crate keeps
  its name, which is right", with no reason attached. A reader learns that somebody agreed rather
  than why.

The gate never keys on `ratified`, and that is deliberate. 54 of the 126 names were unratified
when milestone 115 (the names that were ratified, and the ones that were refused) wrote this. The
tree was at 76 of 229 on 2026-09-20, so the ratio held while the tree nearly doubled. A lint that
demanded the queue be drained would hold every unrelated merge behind a review nobody can hurry.
That is a wall this milestone was written specifically not to build. `script/lint` insists only
that a name say which state it is in.

There are four states rather than these three. §89 (`provisional` becomes the fourth provenance
state) added `provisional` on 2026-08-16: a claim about intent, where the other three are claims
about the record. `provisional` is the largest of the three unratified states. The section *Those two captures print
the worklist two different ways* below is where it is argued. This table is milestone 115's and is
left as it was written.

### The three forms

```
Name: ratified <YYYY-MM-DD> (<who>, <where>). Refused `x` (why), `y` (why).
Name: recorded (<where>). <what the tree already argues, and what it does not settle>
Name: unrecorded. <what the history does and does not say>
```

`recorded` carries a citation for the same reason `ratified` carries a date. The claim is that the
reasoning lives somewhere else, so a block that will not say where has not made the claim. Both are
checked as form and neither as truth. Nothing follows the citation to see whether it says what the
block says it says.

A block runs from its `Name:` line to the next empty comment line, so it may wrap over as many
lines as the reasons need. Two conventions make the refusals machine-readable without a syntax
anybody has to remember: a reason goes in parentheses, and the refusal clause ends at its sentence.
Both exist because the alternative misfires. `capsh(1)` is cited as the Linux tool that made
`capsh` unavailable, so a parser that read every backtick would record the citation as a refusal of
its own. `grant_plan` explains after its list that it is deliberately not named for `swish`.
Neither `swish` nor the `dwarden` it compares itself to is a refused name.

`unrecorded` is a first-class answer. Most of this tree's vocabulary arrived before anyone was
writing naming decisions down. Inventing a ratification to fill a row would put a false claim in the
one record whose entire job is saying who claimed what. So where the history does not say, the
entry says it does not say and cites the commit that introduced the name.

### The numbers, and the one that was not expected

Of 126 names: **72 ratified, 10 recorded, 44 unrecorded.** So 43% of this tree's most reader-facing
vocabulary arrived without a recorded decision. Only a fifth of that backlog is the cheap kind,
where the argument exists and wants a signature. These are milestone 115's counts; `script/names
--check` prints today's, and the research backlog they measured is nearly cleared.

Every name a rename ever touched is ratified, because a rename is an argument and somebody wrote it
down. What is left over is what nobody objected to at the time.

The distribution is the finding, and it runs opposite to exposure:

| Surface | ratified | recorded | unrecorded |
|---|---|---|---|
| programs (54) | 36 | **0** | 18 |
| crates (43) | 23 | 8 | 12 |
| `script/` (29) | 13 | 2 | 14 |

Programs are 0 recorded of 18. The worklist puts the prompt first, because a wrong name there is
read by everyone who uses the system. That surface is the one with no argued reasoning anywhere in
the tree: not in a header, not in a milestone block, not in an introducing commit. `budgeter` and
`heeder` are cited *as* an established agent-noun family when milestone 63 argues for `benchmarker`,
and neither was ever argued for itself. (`budgeter` was argued and ruled on 2026-09-13, and is
`memory_grant_depleter`. The word stays in this sentence because the sentence is about what 63's
text says, and 63 is BUILT and keeps it.) `sink` is used throughout DECISIONS §51 (the sink
protocol) and defended nowhere in it. The one program whose record says anything useful is
`disk_partitioner`, whose introducing commit calls the name provisional in as many words.

The 10 `recorded` are almost all one rule doing the work: seven `*_proto` crates, where milestone
46's spelling decision plus the service the stem names produces the whole string. There are three
outliers. `intrusive` has its own header grounding the term in Linux's `list_head` and seL4's TCB
queues. `script/fmt` was itself the fix, and the header cites §39 (a component is named for what it
is) for it. And `script/supply-chain` is cited by the naming tenet, which says not to respell it.

Two `*_proto` crates were not recorded, and the split was the criterion working. `gfx_proto` and
`cred_proto` had abbreviated stems, the first of the three failure modes the tenet lists for crate
names. The rule that yields `<service>_proto` does not pick which word goes in front of the
underscore. `gfx_proto` was ratified 2026-08-23 (a kernel-dependency crate naming review) as
`graphics_proto`, spelling the abbreviation out in full. `cred` was the sharper case, and was
renamed to `credentialer` on 2026-09-08. Milestone 63 expanded `credcli` and argued `credentialer`
in full, then left two crates spelled `cred` without saying why. `user_rt` fails the same way twice
over, since the only thing establishing `user_` as a prefix is `user_rt` itself. (`user_rt` is
the census's name; the crate is `user_mode_runtime` now, and that rename answered the abbreviation
half of this finding.)

### EXAMPLES

Has this name been refused before? This is the query the incident above needed and nothing could
answer:

```
$ script/names system_builder
REFUSED for crate system_initializer  (crates/system_initializer/src/lib.rs)
  ratified 2026-08-04 (calef, milestone 96), and it is the ratification that raised milestone 115.
  Refused `system_builder` (milestone 63 had already refused it, for a reason still true:
  `builder.rs` calls itself "a minimal init: the system builder", so two programs would claim one
  phrase) and `system_bootloader` ...
```

Everything that has been turned down, and where the reason lives:

```
$ script/names --refused
REFUSED (85), and what holds each refusal

  allocdemo                    program allocator_exerciser
  allot                        crate grant_plan
  ...
  job_killer                   program job_undertaker
  sanitize                     script undefined-behavior-check
  sheesh                       crate swish
```

What is left, in the order worth working through. This is the deliverable. The ordering is exposure
rather than alphabet or count, because exposure is what makes a wrong name expensive. A program is
typed at the prompt, and a crate is what a newcomer greps before opening anything. A `script/` entry
point is typed by whoever works on the tree rather than in it. Within a tier, a name nobody can
justify comes before one whose reasoning merely lacks a signature.

```
$ script/names --unratified
UNRATIFIED (54 of 126), in the order worth working through
...
  programs, provisional
    address_space_builder        fixtures/src/address_space_builder.rs
    ...
  crates, unrecorded
    abi                          crates/abi/src/lib.rs
    ...
  crates, recorded
    clock_contract               crates/clock_contract/src/lib.rs
    ...
  scripts, recorded
    fmt                          script/fmt
    supply-chain                 script/supply-chain

44 unrecorded (research, then a ruling), 10 recorded (a ruling only).
```

That capture keeps `address_space_builder`, which is `address_space_witness` since calef's ruling of
2026-09-18. It is a transcript with measured counts in it, so it is evidence and stays. Sweeping it
would have been doubly wrong: the new name is ratified, so it appears on no `--unratified` listing
that command will ever print.

The tier is the kind, and not "programs a person actually types". That second split is the two-tier
rule calef rejected on 2026-08-01, keyed on a property that is not stable: `wc` went from internal
plumbing to a prompt-typed pipeline stage inside a day. Every program in `components/src/` and
`fixtures/src/` is in the initrd and can be typed. So the kind is the honest tier and needs no
classification anybody could get wrong. This is a sort order rather than a naming convention, so
the cost of being wrong about one entry is that it is read in the wrong minute.

Then one name at a time, with what the history does and does not say about it:

```
$ script/names bitmap_font
crate bitmap_font  (crates/bitmap_font/src/lib.rs)
  ratified 2026-08-23 (calef, a kernel-dependency crate naming review). Renamed from `bitfont`:
  spell out the contraction fully, consistent with this session's other renames, even though
  `bitfont` was already transparent.
```

The narrower slice, for the names where the research is still owed:

```
$ script/names --unrecorded
UNRECORDED (44 of 126): nothing outside the block says why
```

The whole table, and the gate:

```
$ script/names | tail -3
total: 126 names, 72 ratified, 10 recorded, 44 unrecorded, 85 refusals
54 still want an architect: script/names --unratified
refused but live: video_terminal

$ script/names --check
names: NOTE 'video_terminal' is recorded as refused and is also a live name
names: 126 names carry provenance (43 crates, 54 programs, 29 scripts)
names: 72 ratified, 10 recorded, 44 unrecorded, 85 refusals recorded beside them
names: 54 still want an architect (script/names --unratified), which is a worklist and not a failure
```

That `video_terminal` line is the mechanism working rather than a defect. The name was refused for
the program (`display_terminal` is named for its role) and is live as the crate (named for the
protocol it implements). Both facts are true and the pair is deliberate. A reader who meets only one
of them would get it wrong. The check reports the contradiction and never fails on it. A refused
word can legitimately survive as ordinary English, and a gate that fires on prose is a gate people
learn to skip.

Those two captures print the worklist two different ways, and that was a defect, fixed 2026-09-19.
The `--check` line counted `recorded + unrecorded`. The table's last line and `--unratified` counted
`provisional` as well. Neither census line listed `provisional` at all, so the three numbers shown
did not add up to the total. The captures above keep their numbers because they are evidence of what
the tool printed. At 126 names there happened to be no provisional ones. That is why the two lines
agree there, and why nobody saw it until the fourth stranger of milestone 117 (the stranger test)
added a program with a provisional name and watched it vanish from the gate's count.

Every line now prints the one worklist, which is exactly what `--unratified` lists, `provisional`
included. That command sorts provisional names first in every tier, because their author has
already said they are wrong (§89). A count that dropped them hid the part of the worklist worth
reading first. The census names all four states and all four kinds, so it sums:

```
$ script/names --check 2>/dev/null
names: 222 names carry provenance (68 crates, 89 programs, 10 packages, 55 scripts)
names: 153 ratified, 41 provisional, 27 recorded, 1 unrecorded, 273 refusals recorded beside them
names: 69 still want an architect (script/names --unratified), which is a worklist and not a failure

$ script/names --unratified | head -1
UNRATIFIED (69 of 222), in the order worth working through
```
