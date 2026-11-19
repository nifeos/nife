---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1083. `clock_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/clock_contract` as a stranger to this tree would meet it,
on a fresh clone of `main` at `cf1328af` with the template `nifeos/crate-workflows` at `2e651d8`.
It changes nothing; it lists what the next lanes change.

The review took its baseline from
[the review package](../../notes/crate-reviews/clock-contract.md) and reopens none of what it
settles. That is the crate name, the three-authority design (read is a mapping, set is a writable
mapping, propose is an endpoint), the `UNKNOWN` state as a real answer and the asymmetric step
bounds. It also covers the license, `publish = false` under §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit) until the crate's own release
milestone, and the package's proposed name table. Where a rule now in force contradicts a name the package records as ratified,
this block says so and otherwise defers to the package. Milestone 1017 (the contract crates leave
in this order) owns the move; this block owns the findings that should land inside that path.

Reuse: the template and its scaffold, loom, Kani, cargo-mutants, cargo-llvm-cov. No new tool.

## Finding status

- 1, template naming and prose gates: Open. The renames landed in
  [#3640](https://github.com/nifeos/nife/pull/3640) and
  [#3727](https://github.com/nifeos/nife/pull/3727). Now `spelled_out`, `unit_symbols`,
  `register_names` and `british_spelling` refuse nothing. Four things remain for the move.
  `prose_acronyms` still flags RTC, POSIX, TLS, UART, ISA and LOW; the README has no badge row;
  `cmos`, `pl031` and `unix` are not in `.github/accepted-words`; and `question_shape` refuses the
  private `try_claim`, which the review did not see. The maintainer rules a new private name for
  `try_claim` provisionally.
- 2, clippy `doc_markdown` outside nife: Done, [#3640](https://github.com/nifeos/nife/pull/3640).
- 3, mutation exemptions and loom in the tests job: Open. The move's first `mutation` run writes
  `.github/mutants-exempts`, and the tests job runs loom under `--cfg loom`.
- 4, the manifest outside nife: Open. The `publish` comment names no nife file
  ([#3640](https://github.com/nifeos/nife/pull/3640)); `Cargo.toml` gains `repository` at the move.
- 5, a reader never waits on a writer: Done, [#3576](https://github.com/nifeos/nife/pull/3576).
- 6, `initialize` on every start: Done, [#3559](https://github.com/nifeos/nife/pull/3559), carried
  onto the two-copy layout by [#3576](https://github.com/nifeos/nife/pull/3576).
- 7, doc comments that assume nife: Done, [#3640](https://github.com/nifeos/nife/pull/3640).
- 8, 64-bit atomics: Done, [#3640](https://github.com/nifeos/nife/pull/3640).
- 9, two Kani harnesses with falsifications: Done,
  [#3640](https://github.com/nifeos/nife/pull/3640).
- 10, `propose::request` truncates silently: Done,
  [#3640](https://github.com/nifeos/nife/pull/3640).
- 11, `ClockPage::new` takes an integer: Done, [#3640](https://github.com/nifeos/nife/pull/3640).
- 12, a test name that claims more than it checks: Done,
  [#3576](https://github.com/nifeos/nife/pull/3576).
- 13, `initialize` relies on a zeroed frame: Done,
  [#3640](https://github.com/nifeos/nife/pull/3640).
- 14, `offset_for` saturates to zero: Done, [#3640](https://github.com/nifeos/nife/pull/3640).
- 15, who can use the crate: Done, [#3640](https://github.com/nifeos/nife/pull/3640).
- Decision 1, two copies and a publish count: Done,
  [#3576](https://github.com/nifeos/nife/pull/3576).
- Decision 2, identity 8, 1, `CLOCKv01`: Done, [#3576](https://github.com/nifeos/nife/pull/3576).
- Decision 3, the public names: Done (ruled 2026-11-19 UTC, option A). Built
  ([#3640](https://github.com/nifeos/nife/pull/3640),
  [#3727](https://github.com/nifeos/nife/pull/3727)) and ratified as built.
- Review package: Done. [The package](../../notes/crate-reviews/clock-contract.md) names the
  ratified spellings.
- Review clearance: Done (ruled 2026-11-19 UTC, option A), as
  [Decision 4](#decision-4-the-review-is-cleared-and-the-crate-moves) records. The move is next.
- Stranger test: Open. The move lane runs it and records it in the new repository.

## Goal

A stranger who finds `clock_contract` in its own repository can tell in one screen what it is for
and what it is not (a contract three programs link; it sets no clock on its own). The stranger can build, test and model-check it from the README alone. The template's required checks are green for the crate's own reasons from the first commit. Every claim the documentation makes about the clock page holds, and a reader of the page never waits on a writer.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1083-clock-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1083-clock-contract-outside-review/prior-art.md).

## The items

Effort: S is half a day for a lane, M a day or two, L more. A door is one-way when somebody outside
the repository acts on it after the move (a public name, a page layout, an identity value, a
dependency); everything else is two-way and the maintainer decides it.

### P0: the template's required checks are red on the first commit

1. The `check / standalone` job refuses this source as it stands.
   Finding: five steps of the job fail, each at ceiling 0 with no baseline file in the new
   repository. Naming: `spelled_out` refuses the parameter `va` (`src/lib.rs` line 233). `unit_symbols`
   refuses `NS_PER_SECOND` (line 57) and `NANOS_PER_SEC` (line 101). `register_names` refuses
   `NANOS_PER_SEC` (line 101) and the private `W_OFFSET` (line 184). `british_spelling` refuses one test name (line 940), another test name (line 985) and two constants (lines 986 and 987), which use the British forms of serialize, unrecognized and initialized. Prose acronyms in doc comments: RTC (line 125), ARM (line
   204), POSIX (line 382), TLS (line 418), UART (line 536), ISA (line 537), LOW and HIGH (line 550)
   are not expanded at first use. Passive voice: `README.md` lines 36 ("is refused") and 43 ("be
   told") are two passives in 640 words where the helper allows one. Badge row: the README has
   none; the scaffold's `README-crate.md` supplies it. nife tolerates the first three classes
   through `helpers/template_report_baseline.txt` (`prose_acronyms 8`, `register_names 2`), which
   does not move with the crate.
   Why it matters: `standalone` is a required check, so the repository's first pull request is red
   for reasons that have nothing to do with the change it carries.
   Change: the renames land first, under the package's table (Decision 3). The table covers `virtual_address`,
   `NANOSECONDS_PER_SECOND`, the two overlay spellings deleted with the overlay moved to the crate's
   names, `WORD_OFFSET` with its three siblings, the four American spellings, and the loom notes
   that cite the test names moved with them. Then, in the move lane, expand each acronym at its
   first use in the file or add the words every reader knows to `.github/accepted-words` with a reason. `LOW` and `HIGH` are the Goldfish register names and read best in backticks as
   `TIME_LOW` and `TIME_HIGH`. Rewrite the two README sentences in the active voice. Copy the
   scaffold's badge row. The `abbreviations` step is report only but lists 16 components
   (`cmos`, `pl`, `unix`, `rtc`, `seq`, `st`, `req` among them); `cmos`, `pl031` and `unix` belong
   in `.github/accepted-words` with their reasons, and the rest go with the renames.
   Door: two-way before the first tag; Decision 3 ratifies the renames. Effort: S.

2. `check / clippy` refuses the crate outside nife.
   Finding: `cargo clippy --all-targets -- -D warnings` fails on `doc_markdown` at `src/lib.rs` line 537
   ("VisionFive 2 is riscv64"), on stable, on 1.88 and under `--cfg kani`. The crate enables
   `doc_markdown` itself in `[lints.clippy]`. Inside nife the word is on `clippy.toml`'s
   `doc-valid-idents` list; the template's scaffold carries no `clippy.toml`, so the moved crate
   loses the allowance. The review package's clippy run was inside nife and could not see this.
   Why it matters: a required check is red from the first commit, and the lint's suggested fix
   (backticks around a board's name) is wrong, since backticks say "an identifier in this program".
   Change: add a two-line `clippy.toml` to the crate (`doc-valid-idents = ["VisionFive", ".."]`)
   with a comment saying why, or reword the sentence to name no proper noun with an interior
   capital. The first keeps the sentence, which is the better one.
   Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1083-clock-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1083-clock-contract-outside-review/before-the-tag.md).

## Decisions

Decisions 1 to 4 have rulings, and those rulings are the rules in force.

### Decision 1. A reader of the page never waits on a writer

Ruled 2026-11-14 (UTC), option A: the page keeps two copies of the time and a publish count, and
`VERSION` 1 is that layout.

The writer takes a separate claim word, fills the copy the count does not name, advances the count
and gives the claim back. A reader loads the count, reads the copy its low bit names, fences, and
loads the count again. It retries only when they differ, which means a publish finished during its
read, so a writer that stops never makes a reader wait. Writers exclude each other through the claim
word; a writer that stops holding it blocks later writers, which is the right party to block. The
page is seven words: magic, publish count, claim, then the state and the offset of copy 0 and of
copy 1. `generation` is the publish count. The policy, the propose protocol and every public name
but `WORDS` are as they were.

The reason is who the writer is. Linux's vDSO keeps a seqlock because its only writer is the
kernel. Here the writer is the `clock` program, a user process, and any process holding the page
read/write may publish, so a writer stopped mid-publish must not stop `SystemTime::now` in every
process. Bounding the reader's spin instead would have turned a stall into a wrong answer everywhere
at once. The orderings, what each pairs with, and the comparison with Linux's latch and FreeBSD's
`timehands` are in the crate and in notes/clock.md.

Concrete example. The clock service is publishing an accepted proposal, and its supervisor stops it
between the claim and the release. Every reader keeps reading the previous pair with its
generation, and `date` answers. The next writer waits on the claim; item 6's restart is the one
place that has to deal with a claim its own earlier run left held.

### Decision 2. The identity: `INTERFACE_ID` 8, `VERSION` 1, magic `CLOCKv01`

Ruled 2026-11-14 (UTC), option A: the identity freezes as interface 8, version 1, magic
`CLOCKv01`, taking effect with Decision 1's layout, so version 1 is the seven-word page.

Milestone 1017 asks for ratified identity values before a contract crate leaves, because they are
what a peer compares. A reader built against one magic reads a page of another as no clock at all,
and every program's `INTERFACES` note declares the pair of id and version. A later layout break
ships as a new magic and a new version together, never a new id. The crate header and README state
the identity, `MAGIC`, `VERSION` and `INTERFACE_ID` carry ratified `Name:` blocks, and the host
tests `the_identity_is_the_one_readers_were_built_against` and `the_layout_keeps_its_frozen_words`
pin the values and the words as literals.

Concrete example. A program built against `CLOCKv01` meets a page stamped `CLOCKv02`: `read` returns
`UNKNOWN`, the program says it does not know the time, and the program does not claim it is 1970.

### Decision 3. The public names stand as built

Ruled 2026-11-19 (UTC), option A: every public name of `clock_contract` stands as built.
`NANOSECONDS_PER_SECOND`, `virtual_address`, `propose::request`, `word`, `state::SYNCHRONIZED`,
`REAL_TIME_CLOCK` and `real_time_clock` with `#[doc(alias = "RTC")]`, and
`Reading::offset_nanoseconds` are final. `CMOS`, `PL031`, `GOLDFISH` and `WORDS` keep their
spellings. No alias carries an old spelling, and the std overlay reads the crate's names.

A public name is what every program that links `clock_contract` writes in its source, so once the
crate ships on crates.io a rename breaks each of them. Every name passes the template's naming
helpers, and each follows a rule in force: units spelled out, no alias for an old nife spelling,
spec terms kept as aliases.

Prior art. Linux names the same constant `NSEC_PER_SEC` (`include/vdso/time64.h`), and Rust's
standard library spells the unit out in method names (`Duration::as_nanos`,
`Duration::subsec_nanos`) while keeping the short form. nife differs by spelling the unit in full in
every constant. Concrete example: the overlay's `monotonic_nanos` computes `d.as_secs() *
clockproto::NANOSECONDS_PER_SECOND + d.subsec_nanos() as u64`, and the std method keeps its own
spelling.

### Decision 4. The review is cleared and the crate moves

Ruled 2026-11-19 (UTC), option A: the review of `clock_contract` clears on the condition that
[the review package](../../notes/crate-reviews/clock-contract.md) names the landed renames and no
longer says it waits on the names. The package does. The clearance approves creating
`nifeos/clock_contract` and moving the crate there. It approves no tag and no publish; each is a
separate question.

A crate leaves nife for its own repository in the `nifeos` organization only once its review
package clears, and the clearance is the go to create the repository. Decisions 1 and 2 froze the
page layout and identity, Decision 3 ratified the names, and the hostile-input
and documentation fixes have landed. Items 1, 3 and 4 and the stranger test are work the move
itself does.

Prior art. Servo moved crates it shared with others, such as `url` and `html5ever`, into
repositories of their own, each with its own CI and owners (from memory, not re-read). nife differs
by writing a review package that a stranger can read and having one person clear it before the
repository exists. Concrete example: `file_allocation_table` and `e1000e` moved under the same
sequence after their clearances.

## Done when

The checks are in [the appendix](1083-clock-contract-outside-review/done-when.md).

## Follow-on

- **Outstanding.** Items 1, 3 and 4, the rewrite of the provisional `Name:` blocks, the move and
  the stranger test, each as [Finding status](#finding-status) gives it. Checked
  against the crate's code and the template's naming helpers.

## BUGS

- A writer that was only paused, not stopped, can resume after a restarted writer's `initialize`
  gave its claim back, and the two then write the page together. `initialize` trusts its caller to
  know the old writer is gone; the crate's BUGS says the same.
- The Windows row of the prior-art appendix comes from memory, not a re-read; this lane checked the Linux
  and FreeBSD rows against their sources.
- Item 3's line numbers wait on the first full mutation run in the new repository; the reasons are
  the regression note's and stand.
- Item 6 assumes a restart of the `clock` program on the same frame is possible or will be; the review found no supervisor entry and looked for none beyond `crates/system_initializer`.
- The review did not reproduce item 8 on a 32-bit target; the review machine has none installed.
- The review did not run nife's `script/crate-standalone`, `script/crate-move --dry-run` or
  `script/names`; the review package's runs predate the renames, and the move runs them again.
- Clippy at 1.99.1 and 1.88 agree on item 2; a later stable may add `doc_markdown` findings this
  review cannot see.

## Index row

An outside review of `clock_contract` before it leaves. Two required checks are red on the first commit (the template's naming and prose gates, and clippy without nife's `clippy.toml`). Seven items land before the move finishes, and six wait for later. The page's layout and identity are fixed: two copies so a reader never waits on a writer, frozen as interface 8, version 1, magic `CLOCKv01`. The public names stand as built, and the review clears the crate for the move to `nifeos/clock_contract`, with no tag and no publish.
