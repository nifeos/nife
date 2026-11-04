# Naming things in nife

What the tree's names mean, which conventions are rules, and which of those a machine checks.

This file and the appendices it links are the authority for naming conventions, per §155 (the
naming conventions move out of the constitution, and the note becomes the rule). Where they and
`AGENTS.md` disagree, these are the rule and `AGENTS.md` is the bug. The headline rule is
[§39 (a component is named for what it is)](decisions/0039-component-names.md). Each rule is stated
once, in the appendix for its topic, so a changed rule is edited in one place.

## Who names things

An architect names the crates, the programs, the shared modules and the public functions. A name is
global to the tree, so an architect, who can see the whole tree, decides it. In a capability system
a name is often the only thing that says what a program may do.

- A lane ships a provisional name, marks its `Name:` block `provisional`, and says so in its report.
  The integrator surfaces it.
- Nobody renames on their own initiative; a rename is a naming decision.
- A function name is more reversible than a crate's, so the "recommend on reversible forks" latitude
  applies more freely there.
- A lane that has never read these pages can still name safely, because a provisional name is
  expected to change. The conventions matter at ratification and at rename.

## A name is a claim

A name is a claim, made before a reader sees a line of code. A wrong one is a stale comment nobody
can skip: every reader of every call site reads it. `netd` claimed a Unix daemon this OS does not
have, and `linedisc` claimed a reader who knew the tty term of art. So name a thing for what it is,
in a word that parses without prior Unix exposure.

Name against three failure modes:

- Abbreviations that need a decoder: `capsh`, `uheap`, `vt`.
- Generic words that could name almost anything in an operating system: `compose`, `measure`,
  `regions`, `slots`, `frames`.
- Against both, standard terms a reader already knows are the best names available: `elf`, `pci`,
  `paging`, `glob`. That covers shape too, so `supply-chain` keeps its hyphen. This is a naming
  authority, not a license to rename everything.

## The rules, by topic

### Acronyms and abbreviations

The acronym test, every short form the tree spells out or keeps, names someone else owns, and
aliases: [acronyms-and-abbreviations.md](naming/acronyms-and-abbreviations.md).

### Units

A name spells its unit (`_MILLISECONDS`, `_HERTZ`, `_BYTES`), never an SI symbol:
[units.md](naming/units.md).

### Registers and constants

A hardware register, field or bit constant takes the spelled-out name and cites the source's
spelling, BSD first, plus the shapes constants share:
[registers-and-constants.md](naming/registers-and-constants.md).

### Vocabulary

The words the tree gives one meaning (half, `cpu`, principal, keeper, grant, option) and how to
choose a word: [vocabulary.md](naming/vocabulary.md).

### Shapes

Nouns, not verbs; std's method names; parameters; the property, not the count:
[shapes.md](naming/shapes.md).

### Functions that answer yes or no

`is_`, `has_`, `can_` or a third-person verb phrase, as Rust does, and what is exempt:
[functions-that-answer-yes-or-no.md](naming/functions-that-answer-yes-or-no.md).

### Command-line names

The command line abbreviates and the code behind it does not; `jig`'s verbs; shell builtins:
[command-line-names.md](naming/command-line-names.md).

### Crates

One `snake_case` rule with no second tier, the `_contract` suffix, a crate that shares a program's
name, and page contract constants: [crates.md](naming/crates.md).

### Programs, scripts and directories

The spelling per domain, program conventions, the `_exerciser` and `_test_client` suffixes,
`script/` against `helpers/`, and directories:
[programs-scripts-and-directories.md](naming/programs-scripts-and-directories.md).

### Documents, numbers and branches

Where a document goes, `§N` against milestone N, branch prefixes, and file names in an appendix
directory: [documents-numbers-and-branches.md](naming/documents-numbers-and-branches.md).

### Rules a script can check

The naming gates in `script/lint`, the worklist of names that break a checked rule, and what no
script can judge: [rules-a-script-can-check.md](naming/rules-a-script-can-check.md).

## Where a name's provenance lives

Every name carries its own `Name:` block at the name, in one of four states (`provisional`,
`unrecorded`, `recorded`, `ratified`), with what was refused. `script/names` reads the blocks and
computes the table on demand: [provenance.md](naming/provenance.md). What the record cannot see is
in [provenance-limits.md](naming/provenance-limits.md). Rules that ratify a whole class of names are
in [convention-rulings.md](naming/convention-rulings.md), which `script/names` reads.

## Performing a ratified rename

A rename is cheap to type and expensive in every other way. The checklist and the incidents behind
each step: [rename-what-moves.md](naming/rename-what-moves.md),
[rename-where-names-hide.md](naming/rename-where-names-hide.md) and
[rename-traps.md](naming/rename-traps.md).

## Appendices

[naming/README.md](naming/README.md) indexes them.

## BUGS

- An item without a marker is invisible to the worklist, and nothing gates that. Only the lane
  that minted a name can say it is new.
  [provenance-limits.md](naming/provenance-limits.md) says which kinds of name are uncovered.
- `helpers/` helpers are outside the worklist on purpose, per milestone 446 (the naming worklist
  says what it covers).
- A `ratified` is never checked against an architect, and a `recorded` citation is never followed.
- `design/capsicum-and-the-retrofit-question.md` still names `worker`, `heeder` and `spinner` in a
  present-tense claim. The line waits for a writer with standing to edit it.
