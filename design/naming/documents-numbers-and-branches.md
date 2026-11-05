# Documents, numbers and branches

*An appendix to [`design/naming.md`](../naming.md#documents-numbers-and-branches), which states the
rules. It holds the reasons behind where a document goes, the two numbering schemes and branch
prefixes, and every appendix file-name case.*

## Where a document goes

What decides it is what a document is for, not what it is about.

| | holds | shape |
|---|---|---|
| `design/` | the option space, before a decision | "here are four answers and three are bad" |
| `design/decisions/` | the decision, and the argument that settled it | numbered `§N`, append-only |
| `design/tenets/` | why a rule in `AGENTS.md` holds | one file per theme, linked from the rule |
| `notes/` | what exists, and what building it taught us | a glossary, indexed in `notes/README.md` |

A tenet sits in `design/`, not `notes/`, because it is an argument about how to work and gets cited
on its own. `design/roadmap/` sits in `design/` because a milestone block argues for doing
something, and it stays an argument after the milestone ships. Every concept and finding gets a
note, indexed in [notes/README.md](../../notes/README.md): for a demonstration OS the documentation
is part of the deliverable.

## `§N` is not milestone N

Decision numbers and milestone numbers are separate schemes over the same integers, and both run
into the hundreds, so almost every decision number is also a milestone number. A citation of "§31\'s
FileSpec" once pointed at §31 (the foreign-language seam) when it meant milestone 31's shell.

- Write `§N` only for a decision, and "milestone N" in full, never a bare `N`.
- Name it on first mention: "milestone 31 (the capability shell)". `script/citations` checks the
  name, which is what catches a well-formed citation of the wrong section.
- Two files claiming one number is fatal: every citation to it turns ambiguous.
  `script/decisions --check` and `script/roadmap --check` fail on it.
- A gap prints a report and passes. An unused number misleads nobody, and closing a gap means
  renumbering, the tree's most dangerous edit: a citation rewritten by number can name the wrong
  section and still pass every gate.
- When sessions collide, the later lander takes the next free number and leaves the hole, per
  [§194 (sessions interleave rather than
  serialize)](../decisions/0194-sessions-interleave-rather-than-serialize.md). Only the colliding
  sections move, not the run behind them.
- A number is provisional until the merge queue lands it.

## Branches

A prefix says what kind of work a branch is, one spelling per idea: `milestone/`, `maintainer/`,
`roadmap/`, `fix/`, `bench/`, `audit/`, `integration/`, `finalize/`, `lane/`. Write `feature/`,
never `feat/`. The list is a convention, not a gate. Every failure of an enforced allowlist rejected
legitimate work (`roadmap/`, `gh-readonly-queue/*`, `dependabot/*`, a harness's own `claude/*`),
and nothing reads a prefix except the check itself.

One prefix carries a mechanism. A branch that looks like a milestone claim must parse as
`milestone/<N>-<slug>`, because `script/lint` check 4b reads it to require that the branch touch
milestone N's block. `helpers/branch-name-check.sh` refuses a near miss such as
`milestone-126-pgrep`, per §77 (the branch-prefix list now describes the tree). The claim lives in
the branch name and not a label because the check runs offline from `symbolic-ref`, and because a
branch name cannot change once the lane cuts it.

## File names in an appendix directory

- An appendix takes a slug of the heading it continues, even when the heading is a sentence
  (`functions-that-answer-yes-or-no`). When the slug reads wrong, change the heading.
  [convention-rulings.md](convention-rulings.md) Rule 1 ratifies a stem that follows this.
- When one section has several appendices, each takes its own title, not the shared heading.
- An appendix that continues no single section (a reference table, a log, a dataset, an index) takes
  its own title, cut down to the noun that says what it is.
- An appendix of a `## BUGS` section is `bugs.md`.
- A dated record takes its subject and date. For a bench run the subject is the machine
  (`radon-2026-09-03`, `qemu-2026-09`); otherwise it is what the record examined (`census-`,
  `kernel-stack-overflow-2026-08-15`). Use the month when the record spans days. Leave out a word
  such as "measurements", which claims a role.
- The machine stays in the stem when the directory does not name it: `network-boot-xenon` under
  `x86-uefi-boot/`, and plain `network-boot` under `visionfive2/`.
- A file name says what the file is in the sense its directory gives: `survey-design` under
  `process-view/`, not `survey-method`, which reads as a methodology.
- A document drops its "stem is provisional" note once its stem has a ratification.

A rule stated in a document holds now, so its file name carries no date. A date belongs only in the
name of a record of something that happened on that date.
