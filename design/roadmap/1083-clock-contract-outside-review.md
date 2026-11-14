---
status: NOT-STARTED
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

## Goal

A stranger who finds `clock_contract` in its own repository can tell in one screen what it is for
and what it is not (a contract three programs link; it sets no clock on its own). The stranger can build, test and model-check it from the README alone. The template's required checks are green for the crate's own reasons from the first commit. Every claim the documentation makes about the seqlock holds. Either the page layout designs out the one liveness hazard the crate documents twice, before that layout freezes, or a recorded ruling keeps it on purpose.

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
   Change: the renames are Decision 3 and land first, under the package's table. The table covers `virtual_address`,
   `NANOSECONDS_PER_SECOND`, the two overlay spellings deleted with the overlay moved to the crate's
   names, `WORD_OFFSET` with its three siblings, the four American spellings, and the loom notes
   that cite the test names moved with them. Then, in the move lane, expand each acronym at its
   first use in the file or add the words every reader knows to `.github/accepted-words` with a reason. `LOW` and `HIGH` are the Goldfish register names and read best in backticks as
   `TIME_LOW` and `TIME_HIGH`. Rewrite the two README sentences in the active voice. Copy the
   scaffold's badge row. The `abbreviations` step is report only but lists 16 components
   (`cmos`, `pl`, `unix`, `rtc`, `seq`, `st`, `req` among them); `cmos`, `pl031` and `unix` belong
   in `.github/accepted-words` with their reasons, and the rest go with the renames.
   Door: two-way before the first tag; Decision 3 decides the renames, provisionally. Effort: S.

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

## Decisions for the architect

Decisions 1 and 2 are one-way doors; Decision 3 is two-way before the first tag, and the
maintainer has decided it, provisionally. Each with plain background first, the recommendation, prior art and a concrete
example. Order matters: Decision 1 before Decision 2, because a layout change is free until the
identity freezes and costs a version afterward.

### Decision 1. Whether `VERSION` 1 of the page lets readers wait on a writer

Background. The clock page is a seqlock: a writer flips the sequence word odd, stores the state and
the offset, and flips it even again. A reader that finds the sequence odd spins until it is even.
The crate documents, twice, that a writer stopped between the two flips leaves every reader spinning
with no bound. The readers include the standard library's `SystemTime::now` in every process that
holds a read mapping. The writer is not the kernel; in this tree it is the `clock` program, a user
process, and the design allows any process holding the page read/write to publish. The move will freeze the identity values that name this layout (`VERSION` 1, the magic's digits) so the crate can leave. After that, changing the layout is a new version, a new magic and a move of every pin.
Nothing outside nife has acted on the layout yet.

Recommendation: change the layout before the freeze so that a reader never waits on a writer, and
freeze that. Two slots instead of one: the writer fills the slot the current publish count does not
name, then advances the count with a release store. A reader loads the count, reads the slot the
count names, fences, and loads the count again; equal means the pair is whole, and unequal means a
publish completed during the read, so the reader goes again. A reader therefore retries only when a writer finished, never while a writer stalls. Writers still exclude each other, through a separate
claim word, so a writer that stops with the claim held blocks later writers, which is the right
party to block. The page grows from four words to seven (magic, publish count, claim, two pairs of
state and offset), still one frame. `generation` keeps its meaning (the publish count). The policy,
the propose protocol and every public name but `WORDS` stay as they are. The four loom models are
rewritten to the new shape and keep their claims; the model that checks two writers serialize gains
the claim word.

Why this over the alternatives. Keeping the seqlock and documenting the hazard is the cheapest and
is what Linux's vDSO does, but Linux's writer is the kernel. Bounding the spin and returning
`UNKNOWN` after some number of turns changes no layout. It turns a stall into a wrong answer in
every process at once and, through the overlay, into a panic in `SystemTime::now`. The right number of turns is a guess that differs per machine. The two-slot layout removes the hazard rather
than bounding it, costs three words, and is the shape two operating systems with wait-free readers
already use. If the architect prefers the cheaper path, the second alternative is the one to take,
and item 5 then also records the bound.

Prior art (recalled, not re-read for this review): FreeBSD's `timehands` is a ring of records; the
writer fills the next and swaps a pointer, and `binuptime` retries only when the generation it read
moved. Windows's `KUSER_SHARED_DATA` publishes `SystemTime` as three words the writer stores in a
fixed order, and a reader retries while the two high words differ. Both make the reader wait-free
without a lock. Linux's vDSO keeps the seqlock because its only writer cannot die.

Concrete example. The clock service is publishing an accepted proposal, and its supervisor stops it between the claim and the release. Today: every later call to `SystemTime::now` in every
process spins forever, and `date` hangs. With two slots, every reader keeps reading the previous
pair with its generation. The service's restart finds the claim word held and the magic present. The restart's first publish (after item 6's idempotent `initialize`) is the only thing that has
to deal with a stale claim. It can do so because the claim word can carry the writer's intent to
reclaim after a restart.

### Decision 2. Freeze the identity: `INTERFACE_ID` 8, `VERSION` 1, magic `CLOCKv01`

Background. Milestone 1017 asks for ratified identity values before a contract crate leaves,
because they are what a peer compares. A reader built against one magic reads a page of another as
no clock at all, and every program's `INTERFACES` note declares the pair of id and version. The
review package records that the three values match row 8 of milestone 956 (the linked interfaces carry checkable versions)'s table and that fifteen
programs already declare the pair (not re-verified here). Their `Name:` blocks read provisional.
The two contract crates frozen before this one took the same shape: id, version 1, and a magic
where the contract is a page.

Recommendation: freeze all three as they stand, after Decision 1's lane lands, so that `VERSION` 1
names the layout that ships. If Decision 1 keeps the seqlock, freeze them now. Either way the host
test `the_identity_is_the_one_readers_were_built_against` already pins the literals and is the
regression guard.

Prior art: the identity table of milestone 956 and the two earlier freezes in this tree; outside
it, a magic plus a version word is the shape of every on-disk and shared-page format a reader
refuses rather than misreads.

Concrete example. A program built against `CLOCKv01` meets a page stamped `CLOCKv02`: `read` returns
`UNKNOWN`, the program says it does not know the time, and the program does not claim it is 1970.

### Decision 3. The public names, under the rules now in force

Background. The review package proposes a name per item and ratifies the rest as written. Two of its
findings rest on rules that postdate names the package records as ratified. An earlier unit ruling ratified `NS_PER_SECOND`, and the rule now in force spells every unit out and accepts no symbol. The release review reopens such a name. The template's `unit_symbols` and
`register_names` steps refuse it and `NANOS_PER_SEC` at ceiling 0 (item 1), so the move cannot be
green without the ruling.

Recommendation: ratify the package's table as it stands. `NANOSECONDS_PER_SECOND` for
`NS_PER_SECOND`, with no alias for the old spelling. Delete `NANOS_PER_SEC` and `wall_nanos` and move the overlay to the crate's names in the same change. `offset_nanoseconds` for `Reading::offset_nanos`; `virtual_address` for `va` (and item 11's type question with it); `request` for `propose::req`; `word` for `w0`; `SYNCHRONIZED` for `SYNCED`. `REAL_TIME_CLOCK` and `real_time_clock` for `RTC` and `rtc`, each carrying `#[doc(alias = "RTC")]`. Keep `CMOS`, `PL031`, `GOLDFISH` and `WORDS`. This review adds no name to the table and dissents from none. The one
addition is a check, not a name: the four `MAXIMUM_STEP_*` and `NOT_*_NANOSECONDS` constants pass
every rule now in force and need no reopening.

Prior art: the rule itself (a quantity's name ends with its unit spelled out), and the two
contract-crate name tables ratified before this one, which took the same spelled-out forms.

Concrete example. After the rename, the overlay's `monotonic_nanos` reads
`d.as_secs() * clockproto::NANOSECONDS_PER_SECOND + d.subsec_nanos() as u64`, and the std method
keeps its own spelling as the rule says.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag, which the architect may revisit. The package's table holds as written: `NS_PER_SECOND` becomes
`NANOSECONDS_PER_SECOND`; `NANOS_PER_SEC` and `wall_nanos` are deleted; `Reading::offset_nanos`
becomes `offset_nanoseconds`; `va` becomes `virtual_address`; `propose::req` becomes `request`; `w0`
becomes `word`; `state::SYNCED` becomes `SYNCHRONIZED`; `RTC` and `rtc` become `REAL_TIME_CLOCK` and
`real_time_clock`, each with `#[doc(alias = "RTC")]`; `CMOS`, `PL031`, `GOLDFISH` and `WORDS` keep
their names. No alias carries an old spelling. The std overlay reads the old names on more than one
line: `NANOS_PER_SEC` three times, `wall_nanos` once and `offset_nanos` once
(`patches/std-nife/overlay/std/src/sys/time/nife.rs` lines 48, 138, 140 and 141), and all of them
move in the same change. Item 1 is the work.

## Done when

The checks are in [the appendix](1083-clock-contract-outside-review/done-when.md).

## BUGS

- The prior-art box comes from memory, not a re-read. A lane that acts on Decision 1 should read FreeBSD's
  `kern_tc.c` and the Windows `KSYSTEM_TIME` layout before copying either shape, and should record
  what it finds in the crate's notes, not in this block.
- Item 3's line numbers wait on the first full mutation run in the new repository; the reasons are
  the regression note's and stand.
- Item 6 assumes a restart of the `clock` program on the same frame is possible or will be; the review found no supervisor entry and looked for none beyond `crates/system_initializer`.
- The review did not reproduce item 8 on a 32-bit target; the review machine has none installed.
- The review did not run nife's `script/crate-standalone`, `script/crate-move --dry-run` or
  `script/names`; the review package's runs cover them and no name has changed since.
- Clippy at 1.99.1 and 1.88 agree on item 2; a later stable may add `doc_markdown` findings this
  review cannot see.

## Index row

An outside review of `clock_contract` before it leaves. Two required checks are red on the first commit (the template's naming and prose gates, and clippy without nife's `clippy.toml`). Seven items land before the move finishes, and six wait for later. One layout question needs settling before the identity freezes: whether a reader may ever wait on a writer.
