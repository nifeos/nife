---
status: PROPOSED
raised: 2026-10-15
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# Code cites the rule, not the work that wrote it

calef raised it on 2026-10-15 (UTC), reading the citation census: *"I do question if the code
should reference the milestones. Is there prior art?"* Nothing here is in force until he rules on
it.

## The question in numbers

1,238 Rust files outside `vendor/` hold 6,656 milestone citations on 6,592 lines, in 967 of the
files. They hold 3,279 decision citations. Of the milestones cited, 5,253 are BUILT, 1,264 PARTIAL,
93 NOT-STARTED and 47 REMOVED. `script/citations --census` counts 2,773 Rust (file, scheme, number)
pairs with no gloss, the largest unglossed group in the tree.

A milestone is a unit of work. A decision is a rule. `AGENTS.md`'s rule for comments already says to
cross-reference the notes, not the roadmap. The tree also knows the difference matters in code: the defect `script/citations` was built for was 28 comments crediting the `^C` design to
milestone 24 (a second aarch64 board) when they meant DECISIONS §24 (interrupting the foreground
process). `notes/citations.md` already shows, as its example of a code comment, one that cites the
decision rather than the milestone.

## Prior art

Code that cites a document works when the document is frozen and states a rule:

- Network stacks cite RFCs by section, and CPython cites PEPs. A published RFC never changes, so
  the citation stays true for the life of the code.
- Avionics and automotive code (DO-178C, ISO 26262) carries requirement identifiers in code and
  tests, because traceability from requirement to test is what certification audits. The identifiers
  come from a register that owns them.

Code that cites a work item tends to be cleaned out:

- rust-lang/rust named regression tests after issue numbers for years, and has been moving them
  into directories named for what they test, because the number told a reader nothing.
- The Linux kernel puts bug references in commit trailers (`Link:`, `Closes:`), not in comments.
  A comment says why the code is the way it is now.
- Google's style guides allow a bug number in a `TODO`, which is open work by definition and is
  expected to leave when the bug closes.

nife's decisions are the RFC case. Its milestones are the issue case, and 79% of those cited in
Rust are closed.

## What the citations are, sampled

40 lines drawn at random (seed 14) from the 6,592, each read and classed by what the number does:

| what the number does | lines |
|---|---|
| names the milestone that built the thing | 16 |
| tells history | 9 |
| points at open work that owns a gap | 4 |
| carries a constraint the comment does not state | 4 |
| ties a test to the milestone it proves | 3 |
| is a `Name:` register marker | 3 |
| is the subject (`kernel/src/milestone_tour.rs`) | 1 |

One line from each of the first six rows, quoted as found, bare numbers and all:

```
crates/timetable/src/lib.rs           a cron whose every entry is a grant** (milestone 129).
crates/clock_contract/src/lib.rs      it was missing here until milestone 80's loom run found it
crates/grant_plan/src/program/catalog.rs
                                      until ProgramArgumentDeclaration grows the positional arity milestone 47 deferred
xtask/src/bench.rs                    still refuses `--arch x86_64`, because milestone 78's claims
                                      compare an interrupt's arrival
system_tests/src/user/c_thread_tests.rs
                                      The C library keeps the promises fio 3.40 needed (milestone 833
kernel/src/iommu.rs                   Name: provisional (milestone 594 (every VT-d unit translates its
                                      own devices)).
```

Forty lines is a small sample. At 25 of 40, the share that could simply be deleted (the first two
rows) is 62%, give or take about 15 points at 95% confidence. The first backfill lane re-measures on
a larger sample before anything is swept.

Each row maps to the rule below. The first two rows lose nothing when the number goes. The history
row is already ruled: §267 (a comment states the constraint as it is now) sends "found by milestone
N" to the commit message, and milestone 860 (comments state the constraint as it is now) began that
sweep. This proposal adds the provenance row, which §267 does not reach, and a gate for both. A design pointer is better made to the note that
describes the subsystem, which is kept current, than to a block that records how the work was
planned. The fourth row is the real debt: the reason lives in a block, and the reader of the code
has to leave the file to learn a constraint the comment should have stated.

## The rule

A comment in Rust code cites a milestone only in one of these forms:

1. An open-work pointer: the milestone owns something this code defers, and its status is not
   BUILT, REMOVED, REFUSED or SUPERSEDED. This is the `TODO(bug)` case.
2. A test or a test workload naming the milestone whose done-means line it proves. This is the
   requirement-traceability case.
3. A `Name:` marker, whose format `script/names` reads.

Everywhere else the comment states the constraint itself. It cites a decision (`§N`) when the rule's
authority matters, and a note by path and heading when the design does. History goes in the commit.

Decision citations are unchanged, and so is every rule about glossing them. Markdown is out of
scope: a note or a block citing a milestone is a document citing the work it describes, and
`script/citations --moved` already tells its owner when that work changes status.

## The mechanism

Two pieces, both inside `script/citations`, so no new gate is named:

- On a Rust line a commit adds, `--ratchet` fails a milestone citation outside the three forms.
  Form 2 is decided by path (`system_tests/`, a `tests/` directory, `src/tests/`, a file ending
  `_tests.rs`), form 3 by a `Name:` on the line, and form 1 by the cited block's status. The failure
  says which form was nearly meant and that the fix is to state the constraint.
- `--moved` reads Rust comments too. When an open-work pointer's milestone is BUILT, the report
  names the code that still says the work is pending. It stays a report, not a failure, for the
  reason DECISIONS §207 (the roadmap is a graph, and the block says so in fields a script can walk)
  gives: a status flip should not turn other branches red. This removes the `notes/citations.md`
  BUGS line that `--moved` does not read code comments.

A rule enforced only on new lines leaves 6,592 old ones teaching the old idiom by example, so this
proposal also asks for a backfill. `notes/citations.md` refuses a gloss sweep, and that refusal was
about adding text nobody needed. This backfill mostly deletes. It runs as paydown lanes, one crate
or directory per pull request, at most two in the queue, largest groups first (`kernel/`,
`system_tests/`, `crates/grant_plan`). A citation in the fourth row gets its constraint written into
the comment; that is the only part that needs judgment. A census mode counts Rust milestone
citations outside the three forms, and a ceiling that only falls holds the count, as
`helpers/line_citations.py` does for line numbers.

Reuse: `script/citations`'s diff reader, its block-status lookup from `--moved`, and its census, all
taken. `helpers/line_citations.py`'s banked ceiling is the pattern for the backlog count. What is
written is the form classifier and the Rust reader for `--moved`.

## Done means

- A decision records the rule, in calef's words.
- `--ratchet` fails a new Rust milestone citation outside the three forms, with a selftest for
  each form and for the refusal.
- `--moved` reports Rust comments, with a selftest.
- The census mode and its ceiling are in `script/lint`.
- The backfill has its own milestone, with the larger sample's measured split recorded in it.

## What is not decided here

Whether form 2 should exist at all. The alternative is that the block names its tests and the test
names nothing, which is how the roadmap's `Done means` lines already point at code. It would make
form 2 unnecessary, and is calef's call. This proposal keeps form 2 because a test that names what
it proves is read far more often than the block it proves.

*Name: provisional, maintainer/code-cites-decisions-not-milestones, 2026-10-15 (UTC).*
