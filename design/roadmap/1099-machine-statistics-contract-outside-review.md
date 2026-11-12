---
status: NOT-STARTED
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

There is no review package for this crate in `notes/crate-reviews/`, and no row for it in
`notes/crate-reviews.md`. The accepted baseline is therefore thinner than the sibling reviews had:
the crate name, ratified on pull request 1360's table; the two provisional identity constants
`VERSION` and `INTERFACE_ID` that milestone 956 (the linked interfaces carry checkable versions)
minted, with the version word placed beside the magic by ruling; the fixed address, ratified on the
same table; and the two renames `helpers/name_release.py` already carries as pre-ruled for this
crate, `PAGE_BYTES` to `PAGE_CONTENT_BYTES` and `PAGE_VA` to `PAGE_VIRTUAL_ADDRESS`. The review
reopens none of these. The fixed address predates the rule that an address is `_VIRTUAL_ADDRESS`
in every location, and the pre-ruled rename already resolves that contradiction; the review flags
nothing else in the baseline against a later rule. Every other public name has no `Name:` block
at all, so the ratification pass this crate still owes is where the architect meets them.

Reuse: the template `nifeos/crate-workflows` and its helpers, `script/crate-move --dry-run`,
`script/crate-standalone`, `helpers/name_release.py`, Kani, Miri, cargo-llvm-cov, and
`block_roster_contract`'s `verification` module and `falsifications/` directory as the pattern.
No new tool.

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
   "What needs the architect".
   Evidence: `script/crate-standalone` output, "34 name(s) in the crate not ratified";
   `crates/machine_statistics_contract/src/lib.rs` throughout.
   Change: one pull request whose body carries the draft table, with the pre-ruled renames applied
   (`PAGE_CONTENT_BYTES`, `PAGE_VIRTUAL_ADDRESS`), the parameters `page_va` and `va` renamed
   `virtual_address` (the spelled-out rule covers parameters), and the review's flags beside the
   names they concern. Items 9 and 10 add names; the same pull request carries them so the table is
   ratified once.
   Door: one-way after the move (every name goes public); the ratification is the architect's by
   rule. Effort: S for the lane.

2. The manifest and the files a stranger needs.
   Finding: `license.workspace = true` and `[lints] workspace = true` come from the workspace; there is no
   `rust-version`, `description`, `readme`, `keywords` or `categories`; no README, `LICENSE-MIT`
   or `LICENSE-APACHE`; and the `publish = false` comment cites `crates/publish-allowlist.txt`, a
   file the new repository will not hold. The dry run's build fails at `cargo generate-lockfile`
   on the inherited `license`. The two dependencies are path dependencies on `current_cpu_contract` and `address_space_map`, which leave at step 3 of milestone 1017 (the contract crates leave in this order). This crate is at step 6, so the order already holds. The move manifest pins both by `rev` on their `nifeos` repositories, which the standing approval for own-crate dependencies covers.
   Evidence: `crates/machine_statistics_contract/Cargo.toml` lines 5, 6, 13 to 18;
   `script/crate-standalone` output, nine manifest and file failures; the dry run's `build` line.
   Change: write the manifest in `block_roster_contract`'s shape (`rust-version = "1.88"`, the literal license, the literal lint tables, a one-sentence `description`, `readme`, `keywords` and `categories`). Add the two license texts. Write the README from the scaffold's `README-crate.md`
   (sections: summary, `EXAMPLES`, `What is proven`, `Building and testing`, `BUGS`,
   `Minimum Rust version`, `License`). Replace the `publish` comment with one sentence a stranger
   can act on, in the shape the `generational_table` review settled; the line itself stays under
   the publishing hold.
   Door: two-way. Effort: S.

3. The source cites this tree in places a stranger cannot follow.
   Finding: the module documentation and the doc comments name milestone 126, milestone 956,
   milestone 326, decision 225, "AGENTS.md rule 7", `notes/machine-statistics.md`, pull requests
   1360 and 2852, two lane names and the architect, and `src/lib.rs` line 16 says "nife does not
   page out". The manifest comment does the same. `script/crate-standalone` counts two section
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

## Decisions for the architect

One one-way door is open. The ratification table of item 1 is the architect's by the naming rule
and sits after it with the review's flags; it is not a decision in the sense of this section.
Everything else above is two-way and the maintainer decides it.

### Decision 1: Freeze the contract's identity before it leaves, with the line count inside it

Background. Three programs under `components/src/` declare this interface by the pair (`INTERFACE_ID`, `VERSION`) in their `INTERFACES` note. The kernel and the spawn service compile in the fixed address. Every reader compares `MAGIC` and the version word. The kernel and every reader compute the layout from `word::LINE` and `CPU_ID_BOUND`. Once a pin outside nife builds one of those programs, a change to any of these values has to move every pin in one image. Milestone 1017 (the contract crates leave in this order) measured that cost from two reverts. The identity
set here has one member the sibling page crates do not have: the number of cpu lines, which today
is another crate's constant (item 6). A kernel and a reader that agree on every identity value can
still disagree on the layout if that constant moves between their pins.

Recommendation first: freeze the identity as it stands, and make the line count this crate's own.
`MAGIC` is `MACHSTA1`, `VERSION` is 1, `INTERFACE_ID` is 4, and `PAGE_VIRTUAL_ADDRESS` is `0x005F_F000` (a pair page on the address-space map). A line is eight words. There are eight cpu lines after the header line, and the content is 576 bytes. The header holds the magic at word 0, the frame size at 1, the tick rate at 2, the total and free frame counts at 3 and 4 and the version at 5. Words 6 and 7 of every line are reserved and read zero. A later writer may assign one under the same version only if a reader that does not know it is still right to read zero there. Any other change bumps `VERSION` and re-mints the magic's digit in one change. Every value is already compiled into the kernel, the spawn service and three programs, and no item above needs any of them to change. The line count becomes a constant of this crate with a compile-time check against `current_cpu_contract::CPU_ID_BOUND`. The two cannot drift inside one image, and a change to the bound is a deliberate version decision here.

Prior art. Linux's vDSO data page is matched to the kernel that ships it, so its layout never has to be versioned across a boundary. FreeBSD's `vdso_timekeep` carries `tk_ver` and OpenBSD's `timekeep` page a version constant that libc compares. A mismatch means no page rather than a misread one (both unverified in detail). Linux `/proc/stat` grows by appending columns and readers ignore columns they do not know, which is the growth rule recommended for the reserved words. Fuchsia's `zx_info_cpu_stats_t` is a struct per topic whose size the call checks (unverified in
detail). `block_roster_contract`, the contract pilot, froze its identity as a set before it left.

Concrete example. `components/src/free.rs` declares
`(machine_statistics_contract::INTERFACE_ID, machine_statistics_contract::VERSION)`. Suppose a
release of `current_cpu_contract` raised `CPU_ID_BOUND` to 16 and the kernel took it while `free`'s
pin did not. Both still declare `(4, 1)`, the magic and version match, and `free` reads eight of
sixteen lines: its `Mem:` line is right and its cpu totals are silently half. With the line count
frozen here, the kernel's build of this crate fails at the assertion instead, and the change
arrives as `VERSION` 2 with `MACHSTA2`, which `free` refuses until its pin moves.

The alternative, left open, is to carry the line count in a header word (word 6) so a reader
learns it at run time. It costs a layout change now, with the version bump that implies, for a
flexibility no reader in the tree needs; the review does not recommend it.

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
  they count (provisional; item 9).
- `word::ONLINE` through `word::RUNNABLE`: offsets, not indices, and the module does not say so
  in the name (item 9).
- `counter`: returns the atomic word for a writer; `word_at` or `atomic_word` would say that it is
  the word and not a count (provisional; the review has no strong view).
- `Cpu`, `cpus`, `online_cpus`: consistent with the vocabulary rule; the prose around them says
  "core" (item 10).
- `build_header`, `from_words`, `read`, `total_bytes`, `free_bytes`, `busy_ticks`, `idle_ticks`,
  `context_switches`, `interrupts`, `runnable`, `MAGIC`, `VERSION`, `INTERFACE_ID`, `FRAME_BYTES`,
  `TICK_HERTZ`, `TOTAL_FRAMES`, `FREE_FRAMES`, `BUSY_TICKS`, `IDLE_TICKS`, `CONTEXT_SWITCHES`,
  `INTERRUPTS`, `RUNNABLE`, `cpu`: units spelled out, std spellings where std has one, no flag.

## Done when

The checks are in [the appendix](1099-machine-statistics-contract-outside-review/done-when.md).

## BUGS

- The crate has no review package. This block stands in for one on the items it lists, but it
  did not do what a package does: enumerate every public item with its signature for the
  architect's reading, or run `script/crate-review-packet`. A package lane can still write one
  before the architect reads the crate.
- `cargo +1.88 clippy --all-targets -- -D warnings` is red in `address_space_map` (an unused
  `LOW_HALF_SPLIT_SHIFTS`), which this crate links. In the crate's own repository a git dependency
  is not linted, so this does not block here; it is `address_space_map`'s finding, for that
  crate's review. The same build prints a future-incompatibility notice for `address_space_map`,
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

An outside review of `machine_statistics_contract` before it leaves this tree. Three items block the move: thirty-four names without a `Name:` block, a manifest and file set the standalone check refuses, and citations of this tree throughout the source. Eight land before the first tag. Three concern the code: totals that overflow on a page the reader accepted, a `'static` reference under a weaker contract, and a layout whose line count another crate owns. Three concern the proofs and tests: no Kani harness, two `unsafe` functions no host test reaches, and one namespace for indices and offsets. The last two are a type-width inconsistency and six documentation defects. Four follow later. One one-way door waits for the architect: freezing the identity set
with the line count inside it.
