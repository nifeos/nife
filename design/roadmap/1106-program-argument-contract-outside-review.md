---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 672, 994, 1017
decision_dependencies: 170
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1106. `program_argument_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/program_argument_contract` as a stranger to this tree would
meet it, in a fresh shallow clone of `nifeos/nife` at the head of `main` and a clone of
`nifeos/crate-workflows` at the head of its `main`. It changes nothing; it lists what the next
lanes change before the crate moves to a repository of its own (milestone 1017 (the contract crates
leave in this order), step 5).

The crate has no review package in `notes/crate-reviews/` and no row in `notes/crate-reviews.md`.
The settled baseline this review takes instead is the layout proposal, milestone 672 (the argument
page's layout), the interface table of milestone 956 (the linked interfaces carry checkable
versions), which gives the crate `INTERFACE_ID` 18, and milestone 1017's order, which has the crate
leave after milestone 994 (the cross-repository pair is tested where the pin is) lands. Both 672 and
994 are NOT-STARTED. The crate's name is ratified in its own `Name:` block and is not reopened. Every
other public name in the crate is provisional by the crate's own manifest, and this block proposes
the ones the template's gates refuse.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its name helpers
(`spelled_out.py`, `standalone.py`, `unsafe_scan.py`), Kani, cargo-mutants, and the sibling page
contracts (`environment_contract`, `counter_frequency_contract`) as the shapes to match. No new
tool.

## Goal

A stranger who finds `program_argument_contract` in its own repository can tell in one screen what
the page is for. It is one read-only 4 KiB page that carries a program's argv as length-prefixed
bytes, written by whoever spawns the program and read by its runtime. The stranger can build, test and
prove it from the README alone, on the host and for the three bare-metal targets. Every gate the
template requires is green for the crate's own reasons, and nothing in the crate names a path, a
section number, a milestone or a person in another repository.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1106-program-argument-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1106-program-argument-contract-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the move or the release (a published name, a wire
format, an identity value, a layout, a dependency). Everything else is two-way and the maintainer
decides it. Line numbers are in `crates/program_argument_contract/src/lib.rs` unless a path says
otherwise.

### P0: blocks the move

1. The manifest does not stand alone.
   Finding: `license.workspace = true` and `lints.workspace = true` inherit tables from nife's
   workspace, which the new repository does not have. There is no `rust-version`, no
   `description`, no `repository` and no `readme`. `publish = false` carries a comment that sends
   the reader to `crates/publish-allowlist.txt`, a file that stays in nife. The manifest's comment
   block cites a milestone and a decision section by number and calls the name provisional, while
   the crate root's `Name:` block calls it ratified.
   Evidence: `crates/program_argument_contract/Cargo.toml` lines 5 to 18; `standalone.py` reports
   the license, lints, rust-version and publish findings.
   Change: write `license = "MIT OR Apache-2.0"` and a `[lints.rust]` table carrying
   `unsafe_op_in_unsafe_fn`, `missing_docs` and `unexpected_cfgs` naming `cfg(kani)`, as the
   template's `standalone` job requires. Add `rust-version` (the sibling
   `counter_frequency_contract` states 1.88; verify it builds there before stating it),
   `description`, `repository` and `readme`. Keep `publish = false` with one sentence a stranger can
   act on, since a standing ruling holds publishing for every moved crate until its own release milestone. Delete
   the citation comment, since the move commit strips provenance.
   Door: two-way. Effort: XS.

2. The repository files the template requires are absent.
   Finding: the crate has no `README.md`, no `LICENSE-MIT`, no `LICENSE-APACHE`, no
   `CHANGELOG.md`, no `_typos.toml`, no `docs/stranger-test.md` and no `falsifications/`
   directory. `check / standalone` fails on a missing README, a README without `EXAMPLES` and
   `BUGS` headings, a missing license text or a missing typos configuration, and `release / gate`
   refuses while the stranger record reads `none`.
   Evidence: the crate directory holds `Cargo.toml` and `src/lib.rs` only; `standalone.py` reports
   the README and both license files; the template's `scaffold/` and `scaffold/README-crate.md`.
   Change: copy `scaffold/` and write the README to the scaffold's section order. Start with one
   line of purpose. Then add `EXAMPLES` (the builder-and-parser example from `PageBuilder`'s
   documentation, as compiling code) and "What is proven" (item 6's harnesses, or an honest "none
   yet" until they land). Then add "Building and testing" with the three `rustup target add` lines,
   `BUGS` (items 9, 10 and 14's facts), the minimum Rust version and the license section. Record a stranger run before the
   first tag.
   Door: two-way. Effort: S.

3. The crate's text reaches back into nife.
   Finding: `src/lib.rs` names `notes/foreign-program-arguments.md` (line 20) and
   `design/roadmap/0672-the-argument-page-layout.md` (line 38), cites decision sections (§170 at
   lines 2 and 7, §111 at line 31) and milestones (205, 326, 956) by number. It carries four `Name:`
   provenance blocks (lines 35, 59, 68, and the manifest's line 13), and the block at line 35 names
   the architect and quotes the question asked. The template's rules say a crate carries no
   `Name:` block, and `standalone.py` fails on each class: 5 section citation lines, 4 milestone
   citation lines, 4 provenance block lines, 3 path lines and 1 architect's name line.
   Evidence: lines 1 to 38, 59, 68, 358; `crates/program_argument_contract/Cargo.toml` lines 8 to
   13; the template README's "What each nife gate became" table, row "name provenance blocks".
   Change: rewrite the crate root documentation to state the layout, the all-or-nothing rule and
   the reason the bytes carry no authority in the crate's own words, with no section or milestone
   number and no path. Move the measurement note's conclusions a stranger needs (why
   length-prefixed, why `argv[0]` is present, why one page) into that text. Strip every `Name:`
   block in the move commit, as the standing rule for extracted crates has it. Drop the architect's
   name and the quotation. Where the crate must point at nife (the `std` port copies this file),
   say "the nife repository" without a path.
   Door: two-way. Effort: S.

4. Seven names fail the template's spelled-out gate, at a ceiling of 0.
   Finding: `ArgPage` (line 148), the parameter `arg` of `push` (line 119), `from_va` and its
   parameter `va` (line 195), the binding `arg` in `Iter::next` (line 234), and the test helper's
   `args` (lines 258 and 370). In nife the two public ones sit in
   `design/naming-rule-baseline.txt` (lines 148 and 150), a baseline the template does not carry.
   `va` also breaks the address rule (an address is `_VIRTUAL_ADDRESS` or `virtual_address` in
   every location) and the page-contract family rule, which names the parameter `virtual_address`.
   Evidence: `spelled_out.py` output; `design/naming.md`, "Acronyms and abbreviations",
   "Registers and constants" and "Crates"; `design/naming/crates.md`, "Page contract crates".
   Change: `ArgumentPage`, `push(argument)`, `from_virtual_address(virtual_address)`, and
   `argument` and `arguments` for the bindings. The public two are names, so the architect
   ratifies them in the crate's name review before the move (decision 2 below); the lane renames
   the bindings and the test helper without a ruling. The `std` overlay's `sys/args/nife.rs` and
   its generator follow the rename, since the file arrives verbatim.
   Door: the two public names are one-way once released and two-way today; the bindings are
   two-way. Effort: S, counting the overlay.

### P1: before the first tag

5. `CAPACITY` carries no unit and is not the family's name.
   Finding: the constant is the number of record bytes one page holds (4,080). The naming rule says
   a length or a limit carries its unit, and the page-contract family table names the length of a
   page's content, which is less than a page, `PAGE_CONTENT_BYTES`. `grant_plan` spells the unit
   for it in prose ("4,080 bytes") because the name does not.
   Evidence: line 49; `design/naming/crates.md`, "Page contract crates";
   `crates/grant_plan/src/refusal.rs` line 187.
   Change: `PAGE_CONTENT_BYTES`, with `CAPACITY` gone (no alias for a nife spelling before the
   first release). `grant_plan` follows in the same pull request.
   Door: a public name, one-way once released; two-way today. Ratified in decision 2. Effort: XS.

6. The crate has no proof, and the template's front page promises one.
   Finding: no `#[kani::proof]` and no `falsifications/`. Three sibling contract crates
   (`block_roster_contract`, `entropy_contract`, `socket_contract`) carry harnesses. The scaffold's
   README states that every property the crate claims is a harness with a falsification patch; for
   this crate that sentence would be false. `proofs / kani` passes with no harness, so nothing
   fails; the claim does.
   Evidence: no match for `kani` in `src/lib.rs`; `scaffold/README-crate.md` lines 8 to 10.
   Change: three harnesses, each with a falsification patch that `proofs / falsify` replays red.
   First, over a short symbolic page (40 to 64 bytes; `parse` takes any length), `parse` never
   panics and `iter()` yields exactly `len()` items, with `is_empty()` agreeing: verified feasible
   here in 37 seconds. Second, over the same page, the all-or-nothing property as the parser
   states it: a page that parses walks `count` records whose lengths sum to `total` exactly, so
   `rest` is empty, and any other page is `EMPTY`. Third, a round trip of two short symbolic
   arguments through the builder and the parser; this needs item 12 or a stub, because the fixed
   4,096-byte page exhausted 12 GiB here. State the harness names and their run time in the
   README's "What is proven".
   Door: two-way. Effort: M.

7. The one `unsafe` function has no test.
   Finding: `from_va` (lines 195 to 198) has one caller, the `std` overlay, which no host test
   reaches. The sibling `environment_contract` tests its reader through `ConfigPage::new(page.as_ptr()
   as u64)` in a documentation test. Here nothing exercises the raw-pointer path, so Miri in the
   weekly job would have nothing to check.
   Evidence: line 195; `crates/environment_contract/src/lib.rs` lines 199 and 324.
   Change: one test that builds a page and reads it back through
   `from_virtual_address(page.as_ptr() as u64)`, asserting the same argv the slice parser gives;
   a second that reads a zeroed frame the same way and gets `EMPTY`.
   Door: two-way. Effort: XS.

8. The mutation survivor needs its exemption, and nobody has measured the coverage floor.
   Finding: the census records one missed mutant, `ArgPage::check` replacing `<` with `<=` at
   line 170, classified equivalent: a 16-byte page with the magic, count 0 and total 0 is an empty
   argv, and refusing it also yields `EMPTY`. The template's `mutation / mutation` fails on one
   survivor unless `.github/mutants-exempts` lists it with its reason, and `check / mutation-diff`
   judges the changed lines of every pull request. The coverage job's floor is 85 per file; the
   lines no test reaches today are `from_va` (item 7) and `EMPTY` (item 11), which the review did
   not measure against the floor.
   Evidence: `notes/project-metrics/mutation-survivor-classes.csv` line 187; the template README's
   mutation row.
   Change: an exempts entry carrying the equivalence argument above; run `cargo llvm-cov` once
   after items 7 and 11 land and record the figure in the pull request.
   Door: two-way. Effort: XS.

9. The all-or-nothing claim says more than the layout can keep.
   Finding: the crate root says a count that disagrees with the records answers an empty argv.
   That holds for a change to one field: the test flips one byte at a time and truncates. It does
   not hold for a header whose `count` and `total` both describe a prefix of the records: the
   parser walks exactly `count` records, finds `rest` empty, and answers that prefix. There is no
   checksum, so the layout cannot tell a shorter line from a consistently shortened one. The page
   has one writer and maps read-only, so this is a property of the format under corruption, not an
   attack surface, and it belongs in the README's `BUGS`.
   Evidence: lines 22 to 27 and 182 to 186; the test at lines 324 to 345 changes one byte at a
   time.
   Change: say "no single-field damage yields a partial argv" and record the two-field case in
   `BUGS`. A checksum would be a wire-format change; this block does not recommend one (decision 1
   says why).
   Door: two-way for the text; a checksum would be one-way and is not proposed. Effort: XS.

### P2: later

The P2 items are in [the appendix](1106-program-argument-contract-outside-review/later-items.md).

## Decisions for the architect

Only the one-way doors. Everything else above is the maintainer's.

### The layout leaves as built, with the magic still unnumbered

Background. The page is eight bytes of magic, a `u32` count, a `u32` total, then records of a
`u32` length and bytes, in one 4 KiB page at a fixed slot and address. Two programs agree on it:
the shell writes it and `std` reads it, and `ripgrep` from basalt's published archive reads it
today. Milestone 672 proposes ratifying six choices (where, header, record shape, `argv[0]`,
bytes not UTF-8, one page) and is NOT-STARTED. Milestone 956 ruled that `nifeargv` stays
unnumbered until milestone 994 moves the pair test to basalt, because minting digits once broke
the prebuilt `rg`. Milestone 1017 has this crate leave after 994. Moving the crate publishes the
layout to a second repository, which is the moment others start acting on it.

Recommendation: ratify the six rows of 672 as built, keep `nifeargv` unnumbered until 994 lands,
and let the crate leave with `VERSION` 1 and `INTERFACE_ID` 18 as its identity. Do not add a
checksum (item 9) and do not narrow the length prefix to two bytes. Both change the wire for a
property the single writer and the read-only mapping already give, and the four-byte prefix keeps
every field the same width.

Prior art: WASI's `args_sizes_get` hands a count and a total before the strings, the same two
numbers this header carries; Fuchsia's `processargs` carries no version digits in its strings
either and versions the message as a whole. Example: after this ruling the page for `rg pattern
src` stays byte-for-byte what the shell writes today, and a reader that compares the first eight
bytes with `nifeargv` keeps working across the move.

### Three public names to ratify in the name review before the move

Background. The template refuses `arg` and `va` in any name the crate defines, with no baseline,
so the crate cannot pass `check / standalone` under its current names. The family rule for page
contracts names the content length `PAGE_CONTENT_BYTES` and the address parameter
`virtual_address`; `counter_frequency_contract` already follows it. The names are provisional and
unreleased, so renaming costs one pull request here and one line in the `std` overlay.

Recommendation, one per line, each a two-way change today that becomes one-way at the first
release:

- `ArgumentPage` for `ArgPage`. Refused: `Argv` (an abbreviation), `ArgumentsPage` (the family
  names the page by what one record is).
- `from_virtual_address(virtual_address: u64)` for `from_va(va: u64)`. Refused: `from_address`
  (the rule wants the kind of address in the name); `new` (the sibling's spelling, but `parse`
  already builds the value and `new` would read as a second constructor).
- `PAGE_CONTENT_BYTES` for `CAPACITY`. Refused: `MAXIMUM_RECORD_BYTES` (true but outside the
  family table); keeping `CAPACITY` with an alias (no alias for a nife spelling before the first
  release).

Prior art: Rust's standard library spells `std::env::args` and `Args`, which is why the template's
word list tells an author to say whose arguments; `counter_frequency_contract::CounterFrequencyPage::new(virtual_address)`
is the family's ratified form. Example: `let page = unsafe {
ArgumentPage::from_virtual_address(ARGS_PAGE) };` in the `std` overlay replaces the one call that
exists.

### The repository

Background. Milestone 1017 ruled one repository per contract crate, created when the crate moves
and only with the architect's approval of that exact repository, and no lane creates one. No
`nifeos/program_argument_contract` repository exists today. The standing rule is that clearing a
crate's review is the approval to create it.

Recommendation: `nifeos/program_argument_contract`, created after decision 2's renames land and
after milestone 994 lands, with `publish = false` kept until the crate's own release milestone. Tag
`v0.1.0-pre.1` after the move check and a release dry run, as the other moved crates take.

## Done when

The checks are in [the appendix](1106-program-argument-contract-outside-review/done-when.md).

## BUGS

- The round-trip harness here failed for memory, not for a property. Whether item 12's builder
  shape makes it fit is a guess, not a measurement; a lane measures it before promising the third
  harness in the README.
- Nobody measured the coverage floor. The two unreached public lines have names; the percentage
  does not.
- The mutation figures are the weekly census's, read from the metrics files. This review ran no
  census.
- Milestone 994 is NOT-STARTED, and this crate's order in 1017 waits on it. Nothing in this block
  shortens that wait; items 1 to 15 can land in nife before the move, and most should, since each
  is cheaper inside one repository.
- The prior-art rows for Linux, WASI, Hubris and Tock are from memory and unverified against
  source in this review; the four marked "from source" rest on the nife note on foreign program
  arguments.

## Index row

An outside review of `program_argument_contract` before it leaves. Four items block the move (a
manifest and a repository that do not stand alone, text that reaches back into nife, and seven names
the template's gate refuses). Five land before the first tag (the content-length name, three Kani
harnesses, a test for the one `unsafe` function, the mutation exemption, and an honest statement of
what all-or-nothing covers), and six come later. Three decisions go to the architect:
the layout leaves as built with the magic unnumbered, three public names, and the repository.
