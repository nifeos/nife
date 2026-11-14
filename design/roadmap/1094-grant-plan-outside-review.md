---
status: NOT-STARTED
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
   Change: split before the move, as the first decision below lays out. What leaves as `grant_plan`:
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

Only the one-way doors. Each gives the background first, the recommendation second, the prior art
and one concrete example after.

### Where the crate boundary falls

Background. `grant_plan` holds three things that are one crate today. The first is a parser and
planner: bytes in, a typed grant or a typed refusal out, against a manifest. Nothing in it needs
to know a program's name. The second is the vocabulary programs and their spawner share: the
`Manifest` struct and its declaration enums, the capability slot numbers (`DOMAIN_SLOT` and its
neighbors, `INTERFACE_ID` 17), and the verb narrowing. The third is nife's own table: 28 programs
with wire ids, their manifests, the spawn protocol's word layout, the progenitor's window pool and
the region budgets. The externalization rule says a crate that leaves may name no nife program; the
third part is a list of them. Milestone 1017 found the second part from the other side: programs
link the whole planner to read one slot number. The planner's output type, `Endowment`, carries a
`Program` today, so the first part cannot leave without the third unless `Endowment` changes.

Recommendation. Three parts. `grant_plan` leaves as the planner alone, with `Endowment` carrying
the `Manifest` its plan used (`plan_against_with` already takes one); callers that need
the row keep it beside the endowment, as they keep the `ParsedInvocation`. The manifest types, the
slot constants, `VERSION`, `INTERFACE_ID` and `verbs` become a small leaf crate that `grant_plan`
depends on and that a program can link without the planner; its name is yours. The program table,
`spawnproto`, `job_windows`, `job_page_frame`, `image` and the region budgets stay in nife, where
the two programs that speak that protocol live. The alternative, leaving everything together and
widening the lint's waiver, is less work this quarter and makes every later edit to the catalog a
cross-repository bump of a published crate, which is the coordination cost 1017 measured.

Prior art. `clap` ships its lexer as `clap_lex`, a crate with no knowledge of any command, and
builds the catalog on top. Fuchsia's "petals" kept interfaces in repositories apart from the
programs that implement them and rolled each by a pinned revision (1017's prior-art appendix).
Shill separated the contract language from the scripts.

Example. After the split, `swish` plans a line with
`grant_plan::plan_against_with(&run, manifest, &holds, &expanded, &streams)` and sends
`spawnproto::request(program.id(), endowment.program_argument, ...)` from its own table; a
`std` program reads `DOMAIN_SLOT` from the leaf crate and links no parser.

### Public names that need a decoder

Background. The naming page spells out `prog`, `pos`, `opt`, `proto` and `dir` in every name this
tree authors, and lists `tick` and `frame` among words whose kernel meaning differs. Eight public
names in this crate carry one of them. Each is a path somebody outside will write after the first
release.

Recommendation. `nav` becomes `navigation`. `spawnproto` becomes `spawn_protocol` (it stays in nife
under decision 1, so this one is cheap). `rmopt` becomes `rm_options`. `prog_id` becomes
`program_id`. `BindEntry::pos` becomes `position`. `COOP_GRACE_TICKS` becomes
`COOPERATIVE_GRACE_POLLS` (the doc already says it counts watch-loop iterations and not time).
`THE_RIGHTS_BITS_SHARE_NO_BIT` becomes `RIGHTS_BITS_DO_NOT_OVERLAP` and the mutation exclusion moves
with it. `job_page_frame` becomes `job_page` or `job_channel_page` (it is one shared page of four
words, and `frame` is the kernel's word for something else). `Cwd` and `argv` stay as terms
of art. The ratified names stay closed: none of the eight has a ratification.

Prior art. The standard library writes `position`, `program` and `options` in full; Plan 9 and
POSIX write `argv` and `cwd` as they are.

Example. A reader of `grant_plan::navigation::Bindings::lookup(name).position()` knows what came
back without opening the module.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The eight renames hold, with `job_page_frame` becoming
`job_channel_page`, and no alias of the old spellings. `Cwd` and `argv` stay. Item 7 is the work.

### Limits carry their unit, and four fields named `len`

Background. The units rule post-dates most of this crate's ratified limits, and the release review
reopens a name ratified before a later rule. Eight public limits are byte counts with no unit; the
template's gate refuses four private `len` fields.

Recommendation. `MAXIMUM_FILE_NAME` becomes `MAXIMUM_FILE_NAME_BYTES`; `WORDS_NAME` becomes
`MAXIMUM_PACKED_NAME_BYTES` (it is the longest name two argument words carry, and `WORDS_NAME`
says neither); `expand::MAXIMUM_NAME` and `nav::MAXIMUM_NAME` become `MAXIMUM_NAME_BYTES` in each
module; `nav::MAXIMUM_DIRECTORY_NAME` becomes `MAXIMUM_DIRECTORY_NAME_BYTES`; `nav::RENDER_MAXIMUM`
becomes `MAXIMUM_RENDERED_PATH_BYTES`; `spawnproto::HOLDER_MAXIMUM` becomes
`MAXIMUM_HOLDER_NAME_BYTES`; `MAXIMUM_VERB` becomes `MAXIMUM_VERB_BYTES`. The four fields become
`length_bytes`. `filesystem_contract::grant::MAXIMUM_NAME` and `WORDS_NAME`, which two of these
mirror and a test pins, take the same spelling in the same lane, since names that follow a renamed
thing move with it. The counts (`MAXIMUM_NAMES`, `MAXIMUM_DEPTH`, `MAXIMUM_STAGES` and the rest)
are counts of things and stay.

Prior art. `program_argument_contract::PAGE_BYTES` and `expand::NAME_BYTES` in this tree; `libc`'s
`PATH_MAX` and `NAME_MAX` are the counterexample the rule exists to avoid.

Example. A test holds `Refusal::FileNotNameable`'s sentence against `MAXIMUM_FILE_NAME_BYTES`
(item 10), and a reader of the constant knows what 252 counts.

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
- The review measured the sizes with the tree's nightly on one 64-bit host. They match on every
  64-bit target by construction (no pointers in the values), and the review measured no other.

## Index row

An outside review of `grant_plan` before it leaves. Three items block the move: the crate is a table
of nife's programs and cannot stand alone, the documentation does not build with warnings denied,
and the manifest is the workspace's. Eleven items land before the first release, and five are for
later. The eleven cover these. A tokenizer that drops a seventeenth token. The absence of any proof.
Eight limits without units and eight public names that need a decoder. Five British spellings.
Documentation written for a reader inside this tree. Refusal sentences that spell constants by hand.
Two-kilobyte values passed by value. Three unasserted narrow casts. A silently narrowed manifest. A
mutation run the release gate will ask for. Three decisions go to the architect: where the crate boundary
falls, the eight names, and the units.
