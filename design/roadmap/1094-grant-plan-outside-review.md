---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1094. `grant_plan`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/grant_plan` as a stranger to this tree would meet it:
the crate an outside maintainer finds when `nifeos/grant_plan` appears, with no nife checkout
beside it. The review changes nothing; it lists what the move lane and the lanes after it change.

There is **no review package** for this crate in `notes/crate-reviews/`, so there is no settled
baseline of ratified names and open questions to start from. This block starts from the code, from
`design/naming.md`, from the externalization list (`crates/externalization-list.txt`, where the
crate is listed) and from milestone 1017 (the contract crates leave in this order). Its
measurements already name `grant_plan` as the largest coordination cost of the 22 crates (151
commits in 30 days, 120 of them crossing, 227 breaking API items in four weeks). Its order places
the crate in step 6. Where a name below is already ratified, this block does not reopen it unless a later rule
(units spelled out, standard-library spellings) contradicts it, and says so where it does.

Reuse: the template `nifeos/crate-workflows` and its scaffold and helpers (`standalone.py`,
`spelled_out.py`, `register_names.py`, `british_spelling.py`), cargo-mutants, Kani, and the crate's
own `test_support::plan_line`. No new tool.

## Finding status

Every item not Done blocks the first crates.io publish, which waits until every outside-review
milestone reaches delivery.

- P0-1, split the catalog from the planner: Done,
  [#3584](https://github.com/nifeos/nife/pull/3584).
- P0-2, the documentation does not build with warnings denied: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P0-3, the manifest and the repository root are the workspace's: Open. The move writes a
  manifest of the crate's own (no `license.workspace`, no allowlist comment), a README and a
  LICENSE. It also moves the tests off the `program_table` dev-dependency and makes
  `ISOLATION_WORD` (`src/planning.rs` line 42) a value the caller passes, as BUGS records.
- P1-4, the tokenizer drops a seventeenth token: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-5, no proof: Open. Neither `grant_plan` nor `grant_contract` has a Kani harness; a lane writes
  one per claim, and the two `job_windows` patches in `program_table` replay host tests.
- P1-6, limits carry their unit, and four fields named `len`: Open. The new spellings are calef's
  (see "Limits carry their unit" below); `MAXIMUM_WORDS_NAME_BYTES` already carries its unit.
- P1-7, public names that need a decoder: Ruled (maintainer, provisional, 2026-11-14 UTC,
  [#3589](https://github.com/nifeos/nife/pull/3589)). Not built: `nav`, `rmopt`, `BindEntry::pos`,
  `COOP_GRACE_TICKS`, `THE_RIGHTS_BITS_SHARE_NO_BIT`, `spawnproto`, `prog_id` and
  `job_page_frame` keep their spellings; calef ratifies once the rename lane lands.
- P1-8, British spellings: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-9, documentation for a stranger: Open.
  [#3566](https://github.com/nifeos/nife/pull/3566) closes `grant_plan`'s `missing_docs` worklist;
  the move strips the citations, the quotations
  and the 74 `Name:` lines.
- P1-9b, `missing_docs` in the split crates: Open. `grant_contract/src/lib.rs` line 21 and
  `program_table/src/lib.rs` line 16 carry `#![allow(missing_docs)]`; a lane documents the items
  and removes both.
- P1-10, numbers in refusal sentences: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-11, two-kilobyte values by value: Open. The size test is in
  [#3566](https://github.com/nifeos/nife/pull/3566); taking `&Holdings`, `&Expansion` and
  `&Streams` (`src/planning.rs` lines 60 to 73, `program_table/src/planning.rs` lines 14 to 31) is
  two-way, and the maintainer rules it provisionally.
- P1-12, narrow length fields: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-13, an undeclared widening option: Open. `Manifest::is_well_formed` is in
  [#3566](https://github.com/nifeos/nife/pull/3566); the `programs!` assertion and the note
  decoder's call are not written.
- P1-14, the mutation record predates the split: Open. A full run on the three crates gives the
  figure the release gate asks for.
- P2-15, `NameSet`'s `Debug`: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-16, `Result<_, ()>` operands: Open. `src/parsing.rs` lines 212 and 242 allow
  `clippy::result_unit_err`; a lane names the errors.
- P2-17, an unchecked window shift: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-18, tests pin thirteen of twenty-eight wire ids: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-19, no runnable example: Open. The crate has no `examples/` directory; one shows a manifest
  and a planned line.
- Decision, where the crate boundary falls: Done (ruled 2026-11-14 UTC),
  [#3584](https://github.com/nifeos/nife/pull/3584).
- Decision, the names of the split crates: Open. Calef's (see "The names `grant_contract` and
  `program_table`" below).
- Follow-on, a review package: Open. `notes/crate-reviews/` has none for `grant_plan` or
  `grant_contract`; a lane runs `script/crate-review-packet` and writes one before the move.
- Follow-on, the externalization list: Open. `crates/externalization-list.txt` names `grant_plan`
  and not `grant_contract`, which leaves too; `program_table` stays in nife and needs no row.
- Follow-on, review clearance and the repositories: Open. Calef's (see "Review clearance and the
  repositories" below).

## Open decisions

### Limits carry their unit

Background. A limit is the largest size a value may take, and the naming rule says it ends with
its unit (`MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`). Most of this crate's limits had their
ratification before that rule, and a release review rechecks such a name. Seven public limits
count bytes with no unit: `MAXIMUM_FILE_NAME` (252), `expand::MAXIMUM_NAME`, `nav::MAXIMUM_NAME`,
`nav::MAXIMUM_DIRECTORY_NAME`, `nav::RENDER_MAXIMUM`, `program_table::spawnproto::HOLDER_MAXIMUM`
and `grant_contract::verbs::MAXIMUM_VERB`. A lane already renamed `WORDS_NAME` to
`MAXIMUM_WORDS_NAME_BYTES`, matching `filesystem_contract::grant`, where the review had proposed
`MAXIMUM_PACKED_NAME_BYTES`. Four private fields named `len` or `name_len` fail the template's name
gate. After the first publish every public rename breaks a caller.

Recommendation: A.

- A. `MAXIMUM_FILE_NAME_BYTES`; `MAXIMUM_NAME_BYTES` in `expand` and in `nav`;
  `MAXIMUM_DIRECTORY_NAME_BYTES`; `MAXIMUM_RENDERED_PATH_BYTES`; `MAXIMUM_HOLDER_NAME_BYTES`;
  `MAXIMUM_VERB_BYTES`; `MAXIMUM_WORDS_NAME_BYTES` kept as built; the four fields become
  `length_bytes`. `filesystem_contract::grant::MAXIMUM_NAME`, which two of these mirror, moves in
  the same lane.
- B. As A, but `MAXIMUM_WORDS_NAME_BYTES` becomes `MAXIMUM_PACKED_NAME_BYTES` in both crates, the
  review's spelling: it says the name is the one two argument words pack, where "words" reads as
  "words of the line".
- C. Ratify the ratified limits as they are and rename only the fields; the units rule then
  carries a list of exemptions.

Door: one-way at the first publish, because each is a public path; two-way until then.

Prior art. This tree's `program_argument_contract::PAGE_BYTES` and `expand::NAME_BYTES` carry
their unit; POSIX's `PATH_MAX` and `NAME_MAX` are the case the rule avoids, since neither says
whether the terminator counts. Where nife differs: the unit is in the name, not in a manual page.
Example: a test holds `Refusal::FileNotNameable`'s sentence against `MAXIMUM_FILE_NAME_BYTES`, and a
reader knows what 252 counts.

Blocks: the tag.

### The names `grant_contract` and `program_table`

Background. The split ruled on 2026-11-14 (UTC) made two crates out of `grant_plan`.
`grant_contract` holds what a program and its spawner share: the manifest types, the capability
slot numbers, `INTERFACE_ID` 17 and the verb narrowing. It leaves for a repository of its own, and
a program links it without the planner. `program_table` holds nife's own table of 28 programs, the
spawn protocol and the region budgets, and stays in nife. Both names are provisional. §323 (the
binary interface, and the names of the project's layers) settled that crate names stay descriptive
and that the page-contract crates take no common prefix, so nothing waits before these names.

Recommendation: A.

- A. Ratify both as built: `grant_contract` follows the tree's `_contract` pattern for what two
  programs agree on, and `program_table` says what the crate holds.
- B. `grant_contract` and `program_catalog`, the word the review and the code comments use.
- C. `capability_manifest` for the leaf, naming its central type instead of its role.

Door: one-way for `grant_contract`, whose name becomes a repository and a crates.io name; two-way
for `program_table`, which stays in nife.

Prior art. `clap` ships its lexer as `clap_lex` and its derive as `clap_derive`, named for the
part. Where nife
differs: the leaf carries no family prefix, per §323 (the binary interface, and the names of the
project's layers). Example: a `std` program writes `grant_contract::DOMAIN_SLOT` and links no
parser.

Blocks: the move.

### Review clearance and the repositories

Background. Clearing a crate review is the go to create its repository under nifeos. The planner
and the leaf each leave at step 6 of milestone 1017 (the contract crates leave in this order). No
review package exists yet for either, so clearance follows the package and the two decisions above.

Recommendation: A, clear both once their review packages exist and the names above have their
rulings, with no tag and no publication.

- A. Clear `grant_plan` and `grant_contract` together after the packages and the names.
- B. Clear `grant_contract` first, since programs link it and it has no dependency on the planner.
- C. Hold both until the P1 items land.

Door: one-way: a repository that exists has a name others link to.

Prior art. In this tree `generational_table`, `block_roster` and `file_allocation_table` each moved
on a cleared review with no tag. Where nife differs: nothing outside nife pins either crate.
Example: under B, `grant_contract` leaves first and `grant_plan` depends on it by version from its
own repository.

Blocks: the move.

## Goal

A stranger who finds `grant_plan` on its own can tell in one screen what it is. It is a `no_std`,
allocation-free parser and planner that turns one line of bytes into a typed grant or a typed
refusal, against a manifest the caller supplies. The same stranger can build it, run its tests and
read its public names without a glossary. Nothing in the crate names a program, a package, a path or
a person from the tree it came from. The repository's gates are green for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1094-grant-plan-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1094-grant-plan-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two, L is a week or a lane of
its own. A door is one-way when somebody outside the repository acts on it after the release (a
public name, a published behavior, a layout, a wire value, a dependency); everything else is two-way
and the maintainer decides it.

### P0: blocks the move

1. The crate is not separable from nife's program catalog as it stands.
   Finding: `Program` (`src/program/catalog.rs` lines 157 to 437) is an enum of 28 nife programs
   with wire ids, archive names and, in `src/program/manifests.rs`, each one's manifest and verbs.
   `Endowment::program` (`src/command.rs` line 175) ties the planner's one output type to that
   catalog; `NO_NOTE_MANIFEST` is `Program::Uptime.manifest()` (`src/image.rs` line 24) and
   `IMAGE_ROW` is `Program::Uptime` (line 37); `spawnproto::held_shares` takes a `Program`
   (`src/spawnproto.rs` line 925); `ISOLATION_WORD` is the shell's own name (`src/planning.rs` line
   56) and `MOUNTED_TREE_NAME` a mount point of one machine's boot shell (`src/spawnproto.rs` line
   236). The region budgets (`JOB_REGION_PAGES`, `DIRECTORY_JOB_REGION_PAGES`, `STD_REGION_PAGES`,
   `JOB_POOL_PAGES`, `SHELL_BUDGET_PAGES`, `IMAGE_MAXIMUM_PAGES`) are one progenitor's numbers.
   The externalization lint waives 103 consumer-name hits for this crate; `standalone.py` counts 96
   lines naming a path into this tree and refuses the crate. The lint's rule is that a crate on the
   list "may not mention nife, a nife program or a nife package". This crate is a table of them.
   Milestone 1017 already records the half of this that programs feel: the slot constants and
   `Manifest` live here, so `process-tools` and `core-tools` carry the whole planner to read a slot
   number.
   Evidence: the files and lines above; `helpers/lint_externalized_crates.py` lines 32 to 50;
   `design/roadmap/1017-the-contract-crates-leave-in-this-order.md` lines 116 to 121.
   Change: split before the move, as the ruled boundary decision lays out. What leaves as `grant_plan`:
   `word`, `lexing`, `parsing`, `line`, `expand`, `nav`, `planning`, `refusal`, `command` and
   `escalation`. They plan against a `Manifest` the caller passes. `Endowment` carries the
   `Manifest` its plan used instead of a `Program`, which also closes the `IMAGE_ROW` foot
   gun its own doc describes. What becomes a leaf crate: the `Manifest` types,
   the slot constants and `verbs`, with `VERSION` and `INTERFACE_ID` 17 beside them, since that is
   the interface programs declare. What stays in nife: the `programs!` table and its manifests,
   `spawnproto`, `job_windows`, `job_page_frame`, `image` and the region budgets, which are the
   shell's and the progenitor's agreement with each other.
   Door: one-way (crate boundaries and the public API of a released crate). Effort: L.

2. The documentation does not build with warnings denied.
   Finding: `ASK_NETWORK`'s doc comment links `[`pack_name`]`, a private function.
   `cargo doc --no-deps` with `-D warnings` stops there. The template's `standalone` job denies
   only `rustdoc::broken_intra_doc_links` and `rustdoc::invalid_html_tags`, so under the template
   this is a warning in the log, not a red job; whether nife's own lint denies it was not checked.
   Evidence: `src/spawnproto.rs` line 650; `.github/workflows/check.yml` of the template, line 233.
   Change: link `pack_name` as plain code, or make the packing a public function with its own doc,
   which the four `ASK_*` constants and `refused_ask_words` already describe. Then add
   `-D rustdoc::private_intra_doc_links` to the repository's documentation gate so the gate
   catches the next one.
   Door: two-way. Effort: XS.

3. The manifest and the repository root are the workspace's, not a crate's.
   Finding: `standalone.py` lists no README, no `LICENSE-MIT`, no `LICENSE-APACHE`,
   `license.workspace = true`, `lints.workspace = true`, no `rust-version`, `publish = false`, and
   no `description`, `repository`, `keywords` or `categories`. The crate's one dev-dependency on
   `primitive_benchmark_contract` (for `map_el0_declares_the_budget_the_bench_contract_measured`)
   points at a crate that stays in nife. Milestone 1017's table already names it as the move's
   blocker for this crate. It goes with the catalog (item 1), so the planner that leaves needs no
   such test.
   Evidence: `Cargo.toml` lines 1 to 32; the `standalone.py` output above;
   `src/program/tests.rs`.
   Change: add the scaffold's files. Write out `license = "MIT OR Apache-2.0"` and the lints. Set
   `rust-version` at the template's floor. Add a `description` that says "a no_std parser and
   planner that turns one command line into a typed grant or a typed refusal, against a manifest the
   caller supplies". Keep `publish = false` only until the hold on publishing lifts, with a comment
   a stranger can act on.
   Door: two-way, except the license, which the architect ruled for every crate of the organization.
   Effort: S.

### P2: later

The P2 items are in [the appendix](1094-grant-plan-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1094-grant-plan-outside-review/before-the-tag.md).

## Decisions for the architect

The rules in force, with their background and prior art, are in
[the appendix](1094-grant-plan-outside-review/ruled-decisions.md):

- Where the crate boundary falls: ruled 2026-11-14 (UTC) and built. `grant_plan` is the planner,
  `crates/grant_contract` the leaf a program links, and `crates/program_table` the catalog that
  stays in nife.
- Public names that need a decoder: the maintainer's provisional ruling of 2026-11-14 (UTC). The
  eight renames hold, with `job_page_frame` becoming `job_channel_page`; `Cwd` and `argv` stay.

## Done when

The checks are in [the appendix](1094-grant-plan-outside-review/done-when.md).

## BUGS

- This block has no review package to inherit from, so its list of ratified names comes from the
  `Name:` blocks in the source (64 ratified of 201) and not from a packet that an architect has
  read. A recording lane should run `script/crate-review-packet grant_plan` before the split lane
  starts and attach the result beside this block.
- Item 4's defect came from reading `parse_run` and from the shape of the shipped manifests, not
  from a failing test. The change includes the test.
- Item 5's uncertainty about the template's `falsify` pairing was not resolved; the change holds
  either way, since a harness per claim is the stated standard.
- The mutation figures are the tree's records, not a run of this review. The split moves code
  across files, so the first run after it is the one that counts.
- The prior-art table's rows for POSIX, zsh, Plan 9, Shill and Plash come from memory, not a re-read; the
  download figures come from crates.io.
- `grant_plan`'s tests plan against `program_table`'s catalog through a dev-dependency, so the
  crate cannot run its tests outside this tree until they plan against manifests of their own. The
  move lane writes those before 1017 step 6.
- `ISOLATION_WORD` (`b"swish"`) is in `grant_plan`, and is public so the catalog can
  refuse the word before resolving it. It names nife's shell, so the crate cannot leave with it;
  the move lane makes it a value the caller passes.
- The review measured the sizes with the tree's nightly on one 64-bit host. They match on every
  64-bit target by construction (no pointers in the values), and the review measured no other.

## Follow-on

- **Done.** P0 item 1, the split, in #3584: `grant_plan`, `crates/grant_contract` and
  `crates/program_table` build and test on their own.
- **Outstanding.** Every finding marked Open or Ruled under "Finding status", each checked against
  the three crates' sources on main.
- **Recorded.** `grant_plan`'s tests plan against the catalog, and `ISOLATION_WORD` names the
  shell; both are in this block's BUGS.

## Index row

An outside review of `grant_plan` before it leaves. Three items block the move: the crate is a table
of nife's programs and cannot stand alone, the documentation does not build with warnings denied,
and the manifest is the workspace's. Eleven items land before the first release, and five are for
later. The eleven cover these. A tokenizer that drops a seventeenth token. The absence of any proof.
Eight limits without units and eight public names that need a decoder. Five British spellings.
Documentation written for a reader inside this tree. Refusal sentences that spell constants by hand.
Two-kilobyte values passed by value. Three unasserted narrow casts. A silently narrowed manifest. A
mutation run the release gate will ask for. The split along the ruled boundary has landed; the units, the
names of the split crates and review clearance are calef's.
