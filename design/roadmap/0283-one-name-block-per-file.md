---
status: BUILT
raised: 2026-09-13
built: 2026-09-13
---
# 283. One provenance block per file, in the spelling the parse reads

Built 2026-09-13. Minted the same day by the maintainer, from being one message away
from asking calef to ratify `serial_driver` a second time. He had ratified it on 2026-09-08.
*(Number provisional until the merge queue lands it.)*

This is a ladder move from rung four to rung two. "One `Name:` block per file, in the parsed
form" was already the rule. It existed as a convention 205 files happened to follow, with nothing
that fires when one does not, which is `AGENTS.md`'s rung four wearing the clothes of a design. The
two files that broke it broke it silently for a week.

## What happened, because the check is designed against it

`script/names --unratified` listed `serial_driver` as `provisional`. It also listed `job_mix_task`,
ratified 2026-09-05. Two defects compounding, and neither alone would have been quiet:

1. **A stale `Name: provisional` block sat above the ratified one.** The lane that *proposed* each
   rename wrote a provisional block arguing the case; when the rename was performed and calef ruled
   it, a second block was added below and the first was never removed. `name_provenance.block()`
   returns the first `Name:` block it finds, so the proposal is what the gate read.
2. **The ratified block could not have been read anyway.** Both were written
   `//! **Name: ratified 2026-09-08 (calef, milestone 264).**` and the parse is
   `^{prefix} ?Name:`, which the bold prefix does not match. These were the only two bolded blocks
   among 207.

Together they were invisible. The gate found *a* block, parsed it cleanly, and reported
`provisional`, which is a legitimate answer nothing disputes. A gate that reports a plausible wrong
answer is worse than one that reports nothing, because the report is what stops anybody looking.

The two records were fixed by [milestone 264's follow-up](https://github.com/nifeos/nife/pull/825).
This milestone is the mechanism, and it deliberately touches neither file.

## Designed against the general fault, not against the two instances

Checking literally for `**` would have caught yesterday's bug and missed `//!  Name:` with two
spaces, `//! *Name:*`, `//! # Name:`, a second plain block, or `///` where the file uses `//!`. The
question underneath all of those is one question, and it is the one a person answers by looking at
the file:

> Does anything in this file read as a provenance header without being the one that was read?

So `helpers/name_provenance.py` grows `headers()`, which finds every comment line whose content,
after the marker and any leading markdown, begins `Name:`, and `strays()`, which returns the ones
that are not the line `block()` parsed, each with a token saying why it could not have been. Empty
is the only healthy answer. `script/names --check` phrases those for a contributor, names the file
and line, shows the line, and fails; `script/lint` already calls `--check`, so it is wired by
existing.

**The detection lives in the shared module, beside the parse it is defined against.** Milestone
236's rule (three derivations were copied between scripts and nothing noticed when they drifted),
and the sharper form of it here: a checker that carries its own copy of "what a header looks like"
would be a second definition of the thing it is checking, which is the same defect one level up.
The one header spelling is now a single `_head(prefix)` used by both `block()` (to find it) and
`strays()` (to say why a line is not it).

**The parse was deliberately not widened to admit bold.** 205 files use the plain form and
design/naming.md documents it. Admitting the second spelling would make both legal, which is the
opposite of the fix.

## The two lines a check like this has to draw, and where they went

**A mention is not a claim.** `` `Name:` `` at the start of a comment line is how the scripts that
implement this convention talk about it, constantly. Backticks are therefore absent from the markup
that gets stripped, and `script/names`' own line 6 (*"see the `Name:` block at the end of this
comment"*) does not fire.

**A template is not a claim.** `script/names`' header documents the three spellings as
`Name: ratified <YYYY-MM-DD> (<who>, <where>)` and two more like it. A line carrying an
angle-bracket placeholder is showing the *form*, so it is dropped, and that is applied only to
strays: the line `block()` actually read is never dropped by it, so a block that somehow did carry
a placeholder still reports rather than vanishing. The alternative was rewriting `script/names`'
own documentation to appease its own gate, which would have left the next person writing an example
in a comment hitting the same wall with no escape they could see.

## `helpers/name_provenance.py`'s own docstring: scope, not an exemption

That file's line 24 begins a paragraph `Name: provisional, minted by milestone 276's lane on
2026-09-11.` inside its module docstring, deliberately, because `script/names` puts `helpers/` out
of its own scope so the module carries no real block and that paragraph is the record instead.

It is handled by scope and needs no exemption, which is worth saying plainly because the
opposite would have been a foot gun `AGENTS.md` requires marking as one. `script/names` enumerates
four surfaces (`crates/*/src/lib.rs`, `user/src/*.rs`, `script/*`, and every `Cargo.toml` outside
`crates/`) and opens only those files. `helpers/name_provenance.py` is none of them and is never
read. There is no allow-list in this change, and the intended steady state is that there never is
one: the same posture `script/lint` check 5 keeps.

## What it found that nobody knew about

A third instance, `crates/measured_boot/src/lib.rs:297`:

```
/// Name: **provisional**. Under `nifefs`'s `NAME_LEN = 32` with room to spare.
```

That is a real provenance record, written at the thing exactly as milestone 115 asks, for
`PROGRAM_MEASUREMENTS` (the archive entry name `"program_measurements"`). It is not the crate's
block, it is on `///` rather than `//!`, and it is for a name that is **not one of the four
surfaces**: a string two programs agree on, which `script/names` has never enumerated and cannot
report on. Nothing was wrong with the record; it was wearing a spelling that now means something
else.

Reworded to say the same things in prose and to say why it does not wear the header spelling. The
gap it stands for is a follow-on below, not a widening done here: the rule this milestone makes
enforceable is *one block per file*, and a second kind of named thing in the same file is a scope
question for calef rather than a lint to loosen.

## What was built

- `helpers/name_provenance.py`: `_head(prefix)` (the one header spelling, used by both readers),
  `headers()`, `stray_reason()`, `strays()`, and four `STRAY_*` tokens in the same shape as the
  existing `NO_STATUS` / `NO_DATE` / `NO_CITATION`, so the judgment is shared and the wording stays
  at the caller.
- `script/names`: `stray_problems()`, wired into `--check` and therefore into `script/lint`, and
  reported before the "no `Name:` block" case rather than instead of it, because a file whose
  only block is unreadable is exactly the file whose unreadable block is worth pointing at.
- `crates/measured_boot/src/lib.rs`: the item-level record above, reworded.
- design/naming.md's *What is checked, and what cannot be*, item 7.

Verified in both directions. Five faults were injected into a scratch copy of a clean file
(`crates/glob/src/lib.rs`) one at a time and the gate was watched to fire on each with the right
reason: a stacked second block (`second`), a bolded header (`markup`), a two-space indented header
(`indent`), a `///` header in a `//!` file (`marker`), and a `### Name:` heading (`markup`). The
same two shapes were run against the `#` surfaces (`script/citations`, `user/Cargo.toml`) and fire
identically. Against `origin/main` it catches both real specimens, at the right lines, with the
right reason.

## BUGS

- It checks the header, not the block. A file with exactly one well-spelled `Name:` line and
  three paragraphs of continuation contradicting it passes. That is the same limit `script/names`
  already records for the reason itself, and it is not closeable by a script.
- A continuation line that begins `Name:` would be read as a stray. No block does this and it
  would be a strange thing to write, but the check reads lines rather than blocks, so a block whose
  second sentence started a line with the word "Name:" would report. The fix in that case is to
  reflow the sentence, and the gate names the line.
- An angle-bracket placeholder is the escape hatch and is not spelled out anywhere a contributor
  meets it. It is documented in the module and here. A lane that hits the gate on a genuine
  example will read the module, which is one hop further than ideal.
- Nothing checks the fifth surface, because there is no fifth surface. Names on things that are
  not a crate, a program, a `script/` entry point or a Cargo package (archive entries, wire strings,
  public functions since 2026-08-23, types, `helpers/` helpers) carry no gate at all. This milestone
  makes their records *quieter* rather than louder: the header spelling is now reserved for the
  file's own block, so an item-level record must say what it is in prose. design/naming.md's `BUGS`
  carries the uncovered list.

## Follow-on

- **Milestone 398.** `PROGRAM_MEASUREMENTS` is one instance of a kind `script/names` does not
  enumerate: a name two programs agree on, carried in a string constant, ratifiable by nobody
  because nothing lists it. 398 argues the scope question rather than assuming the answer is
  "widen". Numbered on 2026-09-19 by milestone 433's drain of the pile.
- **Refused.** Fixing `user/src/job_mix_task.rs` and `user/src/serial_driver.rs` here. They are the
  two specimens this gate was designed against and they belong to pull request #825, which was
  already open when this lane was briefed. Duplicating a two-file record fix to make one's own gate
  green is how two lanes produce a conflict in the file they both care most about, and the mechanism
  is worth more separated from the records than merged with them.
- **Recorded.** The four limitations above stay limitations and live in this block's `BUGS`, which
  is where the next person changing `strays()` meets them. The one most likely to bite is the
  angle-bracket escape hatch being documented in `helpers/name_provenance.py` rather than in the
  gate's own failure message.

## Index row

A stale `Name: provisional`
block above a bolded `**Name: ratified ...**` was silent both ways: the parse stops at the first
block and cannot see a bolded one, so the gate reported a plausible wrong answer. Rung four to
rung two: `strays()` in the shared module finds every comment line a reader would take as a header
and is not the one that was read, with the reason it could not be. Verified by injecting five
fault shapes. Found a third instance nobody knew about, on a surface `script/names` does not
enumerate.
