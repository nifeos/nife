# Where a name's provenance lives

*An appendix to [`design/naming.md`](../naming.md). It holds the `Name:` block: where it lives,
its states and forms, and the commands that read it. The directory `design/naming/` and this file's
stem are provisional names; naming is an architect's.*

## At the name, and nowhere else

Provenance lives at the name, per milestone 115 (the names that were ratified, and the ones that
were refused). A crate's `lib.rs` header, a program's module doc, a `script/` entry's comment and a
Cargo package's manifest each carry one `Name:` block. Adding a name touches one file, so two lanes
naming two things cannot collide, and the record sits where the next proposer is already reading.

The tree keeps no ratified-names table. A table that every lane adding a name must edit is the
conflict shape that produced three section-number collisions in a day, and it scales the way the
single `DECISIONS.md` did. `script/names` computes the table on demand, so it cannot drift from the
tree.

Four more kinds carry a block by marker, in the same grammar:

- `item`: a `/// Name:` paragraph in the doc of a function, constant, type, field or variant.
- `module`: a `//! Name:` block in a Rust file that is not a crate root or a program.
- `directory`: the `Name:` paragraph of a documentation directory's `README.md`, per §75 (naming
  covers directories).
- `document`: the document's own `Name:` paragraph, else its directory's block.

A lane minting any of these writes the marker, so the name joins `--unratified`. An unmarked item
never fails.

The refusals are the valuable half. A ratified name is visible in the tree, because it is the name.
A refused one is visible nowhere unless its block records it, and the person who most needs it is
the one about to propose it again. `system_builder` is the case: proposed, endorsed and overruled to
`system_initializer`, after milestone 63 had already refused it in one table cell nobody found.

### A document's block is a paragraph, not frontmatter

A document under `design/`, `notes/` or `briefs/` carries its block as a paragraph under its title
by convention, and the parse reads it wherever it sits. Adding a note then touches the note and one
index line on its `notes/README/<area>.md` page. Frontmatter lost because `script/names` already
parses the paragraph for every kind, a refusal is prose that YAML would need quoted, and GitHub
renders frontmatter as a table above the title.

Where a README paragraph and the document's own block both exist, `script/names` reads them together
and fails statuses that disagree. A README `` `stem` Name: `` paragraph counts until
`helpers/document_name_migrate.py` moves the last one; its header holds the rules.

## The four states

| State | Means | What clears it |
|---|---|---|
| `provisional` | whoever chose it expects it to change, per §89 (provisional versus unrecorded) | a ruling |
| `unrecorded` | nothing in the tree or its history says why | research, then a ruling |
| `recorded` | something outside the block argues the name, and no architect ruled | a ruling |
| `ratified` | an architect ruled, with the date and what was refused | done |

A name is `recorded` when something outside its own block argues for the name it has.

- A block is never its own evidence, so a `recorded` block cites somewhere else.
- "It got here first" is not a reason. An exemption explains why a name escapes a rule, not why it
  is the name.
- An assertion is not an argument. "The crate keeps its name, which is right" says somebody agreed,
  not why.

## The forms

```
Name: ratified <YYYY-MM-DD> (<who>, <where>). Refused `x` (why), `y` (why).
Name: recorded (<where>). <what the tree already argues, and what it does not settle>
Name: unrecorded. <what the history does and does not say>
Name: provisional. <what was considered>
```

- The date on `ratified` sits outside the parentheses, and `recorded` names its citation. The gate
  checks both as form, never as truth.
- A block runs from its `Name:` line to the next empty comment line. A reason goes in parentheses,
  and a refusal clause ends at its sentence, so the parser does not read a citation such as
  `capsh(1)` as a refusal.
- `unrecorded` is a first-class answer. Cite the commit that introduced the name, and invent no
  ratification to fill the row.
- One block per file, in plain spelling, per milestone 283 (one provenance block per file). The
  parse reads the first block only and cannot see a `Name:` line wearing markdown. `` `Name:` `` in
  backticks is a mention, and a line with an angle-bracket placeholder shows the form; neither is a
  block.
- A lane never writes `ratified`. The maintainer writes the block at ratification, in the commit
  that applies the name, while the alternatives are still in mind.

## The commands

```
script/names <name>        has this name been refused, and where does the reason live?
script/names --refused     everything turned down, and what holds each refusal
script/names --unratified  the worklist, ordered by exposure; provisional first; an item or
                           module another Cargo package reaches comes before an internal one
script/names --check       the gate; script/lint runs it
```

The gate checks presence and form and never keys on `ratified`, so an unratified name never fails a
build. A lint that demanded the queue be drained would hold every unrelated merge behind a review
nobody can hurry.

`--unratified` orders by exposure, because exposure makes a wrong name expensive: a program is typed
at the prompt, and a crate is what a newcomer greps first. The tier is the kind, not "programs a
person types", which is not a stable property. Within the items and modules tier, a name that
crosses a crate boundary comes before one internal to a crate, grouped by defining crate; internal
names still need ratifying, so the order is a priority and not an exemption.
`helpers/name_crossings.py` says what crossing counts and misses, and only `--unratified` pays its
cost.

A rule ratified for a class of names lives in [convention-rulings.md](convention-rulings.md).
`script/names` counts every name a rule covers as ratified, and `script/names <name>` cites the
rule. A name the rule pulls aside keeps the status its own block claims. That page also holds the
one gate that keys on a ruling: a name an architect ruled and nobody renamed fails `--check` after
45 days.

`script/names` reports a refused name that is also live and never fails on it. `video_terminal` is
refused for the program (`display_terminal` names its role) and live as the crate (named for its
protocol). A refused word can survive as ordinary English, and a gate that fires on prose is one
people learn to skip.

## EXAMPLES

```
$ script/names system_builder
REFUSED for crate system_initializer  (crates/system_initializer/src/lib.rs)
  ratified 2026-08-04 (calef, milestone 96) ... Refused `system_builder` ...

$ script/names --unratified | head -1
$ script/names | tail -3
```

The last line of `script/names` prints the totals by state and the refusal count. A rename checks
that count before and after: [rename-what-moves.md](rename-what-moves.md).
