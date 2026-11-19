---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1099. `machine_statistics_contract`: what an outside maintainer would fix before it leaves

*(The number is provisional; the recording lane assigns it.)*

Raised by a read-only review of `crates/machine_statistics_contract` as a stranger to this tree
would meet it. The review read the crate's two tracked files, every caller in the tree, the naming
rules, the crate template and the sibling contract crates, and ran the standalone check and a local
dry run of the move. It changes nothing; it lists what the next lanes change.

The review package is [`notes/crate-reviews/machine-statistics-contract.md`](../../notes/crate-reviews/machine-statistics-contract.md),
with its row in `notes/crate-reviews.md`. The review found a baseline thinner than the sibling
reviews had: the crate name, ratified on pull request 1360's table, the identity constants
`VERSION` and `INTERFACE_ID` that milestone 956 (the linked interfaces carry checkable versions)
minted, the fixed address, and the two renames `helpers/name_release.py` carried as pre-ruled,
`PAGE_BYTES` to `PAGE_CONTENT_BYTES` and `PAGE_VA` to `PAGE_VIRTUAL_ADDRESS`. The review
reopened none of these. The fixed address predates the rule that an address is `_VIRTUAL_ADDRESS`
in every location, and the pre-ruled rename already resolves that contradiction; the review flags
nothing else in the baseline against a later rule. Every other public name has no `Name:` block
at all, so the ratification pass this crate still owes is where the architect meets them.

Reuse: the template `nifeos/crate-workflows` and its helpers, `script/crate-move --dry-run`,
`script/crate-standalone`, `helpers/name_release.py`, Kani, Miri, cargo-llvm-cov, and
`block_roster_contract`'s `verification` module and `falsifications/` directory as the pattern.
No new tool.

## Finding status

- P0-1, names and the ratification table: Done, [#3801](https://github.com/nifeos/nife/pull/3801). The table is ratified
  (2026-11-19 UTC) and built, and every public name carries a ratified block.
- P0-2, manifest, license files and README: Done, [#3801](https://github.com/nifeos/nife/pull/3801). The move changes the two path
  dependencies to pinned git dependencies and adds `repository`.
- P0-3, citations of this tree: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-4, saturating totals: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-5, `counter` lifetime and read-only contract: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-6, line count owned by this crate: Done, [#3582](https://github.com/nifeos/nife/pull/3582).
- P1-7, three Kani harnesses: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-8, host tests over an aligned frame: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
  Miri over the same tests ran on the package branch; the package records the result.
- P1-9, header and line namespaces: Done, [#3801](https://github.com/nifeos/nife/pull/3801). The header indices live in
  `word::header` and the per-cpu offsets in `word::line`.
- P1-10, `online_cpus` type and the word core: Done, [#3566](https://github.com/nifeos/nife/pull/3566)
  and [#3801](https://github.com/nifeos/nife/pull/3801). `online_cpus` stays `usize`, and the prose says "cpu".
- P1-11, documentation defects: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-12, dry run against the standalone check: Open. `helpers/crate_move.py` runs
  `cargo generate-lockfile` before it inlines the inherited manifest keys, so the dry run stops
  before it can report the path dependencies.
- P2-13, end-to-end test note: Done, [#3801](https://github.com/nifeos/nife/pull/3801). The README says the test lives with the kernel.
- P2-14, unconditional `no_std`: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-15, `Snapshot` size: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Decision 1, identity with the line count inside it: Done (option A, 2026-11-14 UTC),
  [#3582](https://github.com/nifeos/nife/pull/3582).
- Review package: Done, [#3801](https://github.com/nifeos/nife/pull/3801). The repository stays Open: the clearance is an open
  decision below, and the move waits for `address_space_layout` and `current_cpu_contract` to leave
  first (milestone 1017 step 6).

## Open decisions

### Ratified: the names of item 1's table (ruled)

Ruled 2026-11-19 (UTC), option A. The table stands as the maintainer set it: `PAGE_CONTENT_BYTES`,
`PAGE_VIRTUAL_ADDRESS`, the parameters `page_va` and `va` as `virtual_address`, `PAGE_WORDS`,
`word::LINE_WORDS` and `atomic_word` for `counter`. `Snapshot` stays and the module sentence is
reworded. Every other name stays as written, and no alias carries an old spelling. The names are
built, and each public name's `Name:` block reads ratified 2026-11-19 (UTC).

### Clear the review package and approve the repository

Background. A crate leaves this tree for its own `nifeos` repository only when the architect
clears its review package; that clearance is the approval to create the repository. A package
lists every public item with its signature for the architect's reading.
[The package](../../notes/crate-reviews/machine-statistics-contract.md) is written, and it shows the
names as they ship. The kernel, the spawn service and three programs share
the contract, so it meets the bar for leaving.

Recommendation: A. The package shows the names as they ship, so one reading covers the clearance.

- A. Clear the package; the clearance creates the repository.
- B. Clear on this outside review alone, without a package.
- C. Keep the crate in nife for now and revisit when a second repository needs it.

Door: one-way. Creating a public repository and moving the crate there are outward steps that
other pins then follow.

Prior art. Rust moved `libc` and `compiler-builtins` out of `rust-lang/rust` into their own
repositories once their interfaces had settled, each with its own review. Fuchsia's SDK moves a
library into the published SDK only after an API review. nife differs in asking for the review
before the first publish rather than before an SDK release. Concrete example:
the architect cleared `block_roster_contract` on 2026-11-09 (UTC), and it moved to its own
repository.

Blocks: the move.

## Goal

A stranger who finds `machine_statistics_contract` in its own repository can tell in one screen what the page is and who writes it. The stranger can build and test it from the README alone. The stranger can read its one `unsafe` reader with a safety contract that matches the signature. The stranger can trust that every accessor is a total function over any page that carries the magic. The repository's gates are green on the day the crate arrives for the crate's own reasons. Nife's own build keeps working the day it leaves.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1099-machine-statistics-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1099-machine-statistics-contract-outside-review/prior-art.md).

## The items

Effort: S is half a day for a lane, M is a day or two, L is longer. A door is one-way when
somebody outside the repository acts on it after the move (a published name, a wire value, a
layout, a fixed address, a dependency); everything else is two-way and the maintainer decides it.

### P0: blocks the move or the first green run

1. Thirty-four public names have no `Name:` block, and the standalone check refuses the crate at
   step 0.
   Finding: `python3 helpers/name_release.py --draft machine_statistics_contract` prints one
   ratified row (the crate), two provisional rows (`VERSION`, `INTERFACE_ID`) and thirty-one rows
   reading "no block". `script/crate-standalone` fails first on this. The move commit strips every
   block, so each name must read `ratified` before it. The review's flags for that table are in
   "Decisions for the architect".
   Evidence: `script/crate-standalone` output, "34 name(s) in the crate not ratified";
   `crates/machine_statistics_contract/src/lib.rs` throughout.
   Change: one pull request whose body carries the draft table, with the pre-ruled renames applied
   (`PAGE_CONTENT_BYTES`, `PAGE_VIRTUAL_ADDRESS`), the parameters `page_va` and `va` renamed
   `virtual_address` (the spelled-out rule covers parameters), and the review's flags beside the
   names they concern. Items 9 and 10 add names; the same pull request carries them so the table is
   ratified once.
   Door: two-way before the first tag; the maintainer decides the names, provisionally, under "The
   ratification table of item 1", and the ratification is the architect's by rule. Effort: S for the lane.

2. The manifest and the files a stranger needs.
   Finding: `license.workspace = true` and `[lints] workspace = true` come from the workspace; there is no
   `rust-version`, `description`, `readme`, `keywords` or `categories`; no README, `LICENSE-MIT`
   or `LICENSE-APACHE`; and the `publish = false` comment cites `crates/publish-allowlist.txt`, a
   file the new repository will not hold. The dry run's build fails at `cargo generate-lockfile`
   on the inherited `license`. The two dependencies are path dependencies on `current_cpu_contract` and `address_space_layout`, which leave at step 3 of milestone 1017 (the contract crates leave in this order). This crate is at step 6, so the order already holds. The move manifest pins both by `rev` on their `nifeos` repositories, which the standing approval for own-crate dependencies covers.
   Evidence: `crates/machine_statistics_contract/Cargo.toml` lines 5, 6, 13 to 18;
   `script/crate-standalone` output, nine manifest and file failures; the dry run's `build` line.
   Change: write the manifest in `block_roster_contract`'s shape (`rust-version = "1.88"`, the literal license, the literal lint tables, a one-sentence `description`, `readme`, `keywords` and `categories`). Add the two license texts. Write the README from the scaffold's `README-crate.md`
   (sections: summary, `EXAMPLES`, `What is proven`, `Building and testing`, `BUGS`,
   `Minimum Rust version`, `License`). Replace the `publish` comment with one sentence a stranger
   can act on, in the shape the `generational_table` review settled; the line itself stays under
   the publishing hold.
   Door: two-way. Effort: S.

3. The source cites this tree in places a stranger cannot follow.
   Finding: the module documentation and the doc comments name milestone 126 (the `procps` package, now `process-tools`: who else is running, and who is allowed to ask) and milestone 956. They also name milestone 326 (nobody has been assigned to turn a mutation score upward), decision 225, "AGENTS.md rule 7", `notes/machine-statistics.md`, pull requests 1360 and 2852, two lane names and the architect. Also `src/lib.rs` line 16 says "nife does not page out". The manifest comment does the same. `script/crate-standalone` counts two section
   citation lines, six milestone citation lines, three architect's-name lines and three path lines;
   the externalization lint counts one `nife` hit. The `Name:` block on the module is the only one
   the move strips; the `Ratified` sentence on `PAGE_VA` (line 76) and the ruling sentence on
   `VERSION` (line 35) are prose, not blocks, and survive the rewrite.
   Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 1 to 2, 7, 9, 16, 19 to 21,
   33 to 39, 44 to 47, 69 to 76, 94 to 98, 351, 365 to 369; `Cargo.toml` lines 8 to 11.
   Change: rewrite each sentence in the contract's own terms. The module documentation says what
   the page is, who writes it and why the words are not consistent, and that a writer and a reader
   agree by linking this crate. The BUGS section states only what the contract cannot express
   (item 11 has the list). The design note's reasoning that a stranger needs (why relaxed, why one
   writer per word, why no sequence counter) moves into the crate's documentation in its own
   words; the rest stays in nife.
   Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1099-machine-statistics-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1099-machine-statistics-contract-outside-review/before-the-tag.md).

## Follow-on

- **Outstanding.** Each finding that "Finding status" marks Open or Ruled, checked against the
  crate's source and manifest.

## Decisions for the architect

Decision 1 below is the one one-way door, and it carries its ruling. The table of item 1 follows it
with the review's flags and the names ratified on 2026-11-19 (UTC). Everything else above is
two-way and the maintainer decides it.

### Decision 1: Freeze the contract's identity before it leaves, with the line count inside it

Ruling of 2026-11-14 (UTC), option A. The rule in force: the identity stays as built. `MAGIC` is
`MACHSTA1`, `VERSION` is 1, `INTERFACE_ID` is 4, and `PAGE_VIRTUAL_ADDRESS` is the fixed pair page. The page
holds one header line of eight words plus one eight-word line per processor, 576 bytes, and header
words 6 and 7 stay reserved and read zero. The processor-line count belongs to this crate as its
own constant, `PROCESSOR_LINES`, and replaces the re-export of
`current_cpu_contract::CPU_ID_BOUND`. A compile-time assertion keeps it equal to that bound, so a
change to the bound fails this crate's build until this page's magic and version move too.

The background, the recommendation as reviewed, prior art, a concrete example and the alternative
are in [the appendix](1099-machine-statistics-contract-outside-review/identity-decision.md).

### The ratification table of item 1, with the review's flags

The architect ratifies thirty-four names in one pass. Where the review has something a stranger
might misread, it says so here; the lane carries these beside the rows.

- `PAGE_BYTES` and `PAGE_VA`: pre-ruled renames to `PAGE_CONTENT_BYTES` and
  `PAGE_VIRTUAL_ADDRESS`; the parameters `page_va` and `va` follow as `virtual_address`.
- `Snapshot`: the module documentation says the words "are not a consistent snapshot of each
  other" and the type carries the name `Snapshot`. The review recommends keeping the name, because the
  type is one copy taken once, and rewording the sentence to say a reader loads the words one at a time
  and are not consistent with each other.
- `WORDS` and `LINE`: generic words under the naming rules; `PAGE_WORDS` and `LINE_WORDS` say what
  they count (item 9).
- `word::ONLINE` through `word::RUNNABLE`: offsets, not indices, and the module does not say so
  in the name (item 9).
- `counter`: returns the atomic word for a writer; `word_at` or `atomic_word` would say that it is
  the word and not a count.
- `Cpu`, `cpus`, `online_cpus`: consistent with the vocabulary rule; the prose around them says
  "core" (item 10).
- `build_header`, `from_words`, `read`, `total_bytes`, `free_bytes`, `busy_ticks`, `idle_ticks`,
  `context_switches`, `interrupts`, `runnable`, `MAGIC`, `VERSION`, `INTERFACE_ID`, `FRAME_BYTES`,
  `TICK_HERTZ`, `TOTAL_FRAMES`, `FREE_FRAMES`, `BUSY_TICKS`, `IDLE_TICKS`, `CONTEXT_SWITCHES`,
  `INTERRUPTS`, `RUNNABLE`, `cpu`: units spelled out, std spellings where std has one, no flag.

Ratified 2026-11-19 (UTC). The names the table carries into item 1's pull request are these.
`PAGE_CONTENT_BYTES` and `PAGE_VIRTUAL_ADDRESS`, with the parameters `page_va` and `va` as
`virtual_address`. `PAGE_WORDS` and `word::LINE_WORDS`. `Snapshot` stays, and a lane rewords the module
sentence. `counter` becomes `atomic_word`. The rest stay as written. There is no alias of the
old spellings.

## Done when

The checks are in [the appendix](1099-machine-statistics-contract-outside-review/done-when.md). The finding status above says, item by item, what is done and what each open item still needs.

## BUGS

- `cargo +1.88 clippy --all-targets -- -D warnings` is red in `address_space_layout` (an unused
  `LOW_HALF_SPLIT_SHIFTS`), which this crate links. In the crate's own repository a git dependency
  is not linted, so this does not block here; it is `address_space_layout`'s finding, for that
  crate's review. The same build prints a future-incompatibility notice for `address_space_layout`,
  which the review did not chase.
- The dry run never reported the two path dependencies because its build stopped earlier (item
  12). The move lane reads `script/crate-standalone` as well as the dry run.
- Items 5 and 14 came from reading, not from a build for a narrow target or with the
  unconditional attribute.
- The mutation census was not re-run; the carried row reads 33 mutants, none missed. Item 4's
  saturating fold adds mutants the next census will judge.
- The template's scaffold still mentions nife in six lines (`.githooks/commit-msg`, `deny.toml`,
  `docs/stranger-test.md`, `rustfmt.toml`, `CONTRIBUTING.md` and the pull request template); the
  dry run reports them against every crate, and they are the template's to fix.
- The review did not run nife's QEMU system tests; the review read the kernel's writer and did not exercise it.

## Index row

An outside review of `machine_statistics_contract` before it leaves this tree. Three items block the move: thirty-four names without a `Name:` block, a manifest and file set the standalone check refuses, and citations of this tree throughout the source. Eight land before the first tag. Three concern the code: totals that overflow on a page the reader accepted, a `'static` reference under a weaker contract, and a layout whose line count another crate owns. Three concern the proofs and tests: no Kani harness, two `unsafe` functions no host test reaches, and one namespace for indices and offsets. The last two are a type-width inconsistency and six documentation defects. Four follow later. The one one-way door, freezing the identity set with the line count inside it,
has its ruling of 2026-11-14 (UTC); the names are ratified (2026-11-19 UTC), and clearing the review package stays open.
