---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1034
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1103. `page_frames`: what an outside maintainer would fix before 0.1.0

Raised by a read-only review of `crates/page_frames` as a stranger to this tree would meet it,
made before any move to `nifeos/page_frames`. It changes nothing; it lists what the next lanes
change, in nife before the move where the runbook puts a finding against the crate itself, and in
the new repository after it.

The crate has no review package in `notes/crate-reviews/`, so this review has a thinner baseline
than its siblings had. It takes the following as settled and does not reopen them. The crate name, which the crate-level `Name:` block records as ratified at a kernel-dependency crate naming review. The bitmap design and the all-used start, which `notes/physical-memory.md` argues. The five Kani harnesses and their falsification patches. The three mutation survivors nife's census argued equivalent. The move itself, once a package exists. No `Name:` block below the crate level
exists, so every public item name is provisional, and the naming pass of item 8 is the first time
they meet [design/naming.md](../naming.md). Two of them fail a rule there and are one-way doors
after a release, so they go to the architect (Decisions 1 and 2).

Reuse: the template `nifeos/crate-workflows`, `script/crate-move --dry-run`, Kani, cargo-mutants and
the crate's own harness pattern (a symbolic 8-frame bitmap built without `new`).

## Goal

A stranger who finds `page_frames` can tell in one screen what it allocates, in what unit, and why
everything starts used. The same stranger can follow the README alone to a build, the tests, the
proofs and one falsification, and meets no sentence in the README or the documentation that the
code contradicts. `mark_free` rounds in the direction its documentation describes and the safe one.
The repository's `check` and `proofs` runs are green on the first push for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1103-page-frames-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1103-page-frames-outside-review/prior-art.md).

## The items

Effort: XS is under an hour, S half a day, M a day or two, L longer. A door is one-way when somebody
outside the repository acts on it after a release (a public name, a published behavior, a layout, a
dependency). Everything else is two-way and the maintainer decides it. Line numbers are in
`crates/page_frames/` at `75bb255a`.

### P0: blocks the move

1. The manifest does not stand alone.
   Finding: `standalone.py` on the staged tree reports nine lines. They are no `LICENSE-MIT`, no
   `LICENSE-APACHE`, `license` inherits the workspace, and no `rust-version`. Also, `lints.workspace
   = true` inherits a workspace that is not there, and `publish = false` (allowed through the soak
   by the `allow-unpublished` that the dry run writes into `ci.yml`), and the three citation counts
   of item
   3. The manifest also has no `description`, `readme`, `keywords` or `categories`, and the
   `publish` comment sends a reader to `crates/publish-allowlist.txt`, a file the new repository
   will not have. The scaffold supplies no license files. A sibling prepared for its move carries
   both in its crate directory.
   Evidence: `Cargo.toml` lines 1 to 9. `standalone.py --manifest-dir <staged>`. `crates/entropy_contract/`
   for the shape a move-ready manifest takes.
   Change: in nife before the move, give the manifest a `description` (one line: a physical page
   frame allocator over a borrowed bitmap, every frame used until declared RAM). Add `rust-version`,
   `license = "MIT OR Apache-2.0"`, `readme = "README.md"`, `keywords = ["no-std", "allocator",
   "page-frame", "bitmap"]`, `categories = ["no-std", "no-std::no-alloc", "memory-management"]` and
   `documentation = "https://docs.rs/page_frames"`. Copy the workspace lint set out explicitly (as
   `entropy_contract` did, with `check-cfg = ['cfg(kani)']`). Write a self-contained `publish`
   comment and put the two license files beside it. The `rust-version` is the lowest stable that has
   `u64::is_multiple_of` and `usize::div_ceil`. The review believes that is 1.87 (unverified) and
   the check is `rustup toolchain install 1.87 --profile minimal` then `cargo +1.87 test` on the
   staged tree.
   Door: two-way. Effort: S.

2. The crate has no README, and the reasoning that would fill one stays behind in nife.
   Finding: the dry run stages the scaffold's README shape and reports "needs writing". The
   arguments a stranger needs (why a bitmap and not a free list, why everything starts used, why
   `mark_used` rounds outward) live in `notes/physical-memory.md`. That note does not move. It still
   names `FrameAllocator::new` and an `init` that the crate no longer has. The template's
   `standalone` job requires `## EXAMPLES` and `## BUGS`, the badge row and the proof subsections.
   Evidence: the dry run's `readme` line. `notes/physical-memory.md` lines 48 to 88.
   `crate-workflows/scaffold/README-crate.md`.
   Change: in nife before the move, write `README.md` in the scaffold's shape with the crate's own
   content: the one line. `## EXAMPLES` with the bootstrap wiring already in `lib.rs` lines 94 to 115. A `##
   Design` section carrying the three arguments above in the stranger's words, with the
   kernel-image-tail example (an image ending at `0x4009_7010` owns the frame at `0x4009_7000`). `##
   What is proven` with one line per harness and its bound (an 8-frame bitmap at base 0 or
   `0x4000_0000`; every address for `containing`; frame counts to 2^40 for `bitmap_bytes`; totals to
   2^20 for `index_of`). It also says what stays unproven until item 6 lands. `## Building and
   testing` (host-only tests, the three bare-metal `--lib` builds). `### Running the proofs` with
   the measured 8 seconds and 169 MB. `### Running one falsification patch` using the
   `two_allocations_are_distinct` patch, whose header already explains the counterexample well. `##
   BUGS` (items 13, 15 and 19). `## Minimum Rust version`. `## License`.
   Door: two-way. Effort: M.

3. The documentation cites this tree, and the move refuses it.
   Finding: the dry run counts 2 section citations (`src/lib.rs` line 4, 342). It counts 6 milestone citations (`src/lib.rs` line 226, 259, 342, `tests/allocator.rs` line 221, 270, and a falsification patch header). It counts 6 paths into the nife tree (`src/lib.rs` line 5, 15, 86, 199, 348, `tests/allocator.rs` line 272). `src/lib.rs` line 261 names a nife program, `tests/allocator.rs` line 215 names
   an emulator board, and `src/lib.rs` line 10 names a sibling crate. The move strips the `Name:`
   paragraph at `src/lib.rs` lines 8 to 11 and leaves the rest.
   Evidence: the dry run's `citations` lines. `grep -n '§\|milestone\|notes/' src/lib.rs
   tests/allocator.rs`.
   Change: in nife before the move, rewrite each sentence to carry its reason instead of its
   pointer. "Milestone 107 (the socket contract learns to accept) found 137 frames free and no run of 128" becomes "a boot can hold 137
   free frames and no run of 128, and a total-free reading alone cannot tell exhaustion from
   fragmentation". The `[notes/mmu.md]` link at `src/lib.rs` line 15 becomes a sentence. The
   falsification headers keep their defect and expectation text and lose the milestone numbers.
   Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1103-page-frames-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1103-page-frames-outside-review/before-the-tag.md).

## Decisions for the architect

Two decisions, each with a recommendation first. Decision 2 is decided, provisionally, below.
Everything else above is two-way and the maintainer decides it. The names in items 8, 9 and 14 ship provisional and wait for a naming pass.

### Decision 1: Whether `FRAME_SIZE` becomes `FRAME_BYTES` before the first release

Background. `FRAME_SIZE` is the crate's one constant, 4096, and the most used name it exports: about
sixty sites in `kernel/` and `system_tests/` multiply or mask by it. It is a length in bytes.
[design/naming.md](../naming.md) says a length or a limit carries its unit, spelled out, and gives
`MAXIMUM_NAME_BYTES` as the form. The tree's own precedent for the same quantity is `PAGE_BYTES` in
`program_argument_contract`. The constant has no `Name:` block, so no ruling covers it. A published
constant is a one-way door. A renaming after 0.1.0 breaks every dependent. The rule that a crate
carries no alias for this tree's earlier spelling before its first release means the old name cannot
stay as a redirect.

Recommendation: rename to `FRAME_BYTES` now, in the rename lane of item 8, with the two-way names in
the same sweep. The cost is one mechanical sweep inside a tree that is still the only user. The
alternative is a published name that fails the tree's own rule and that a future release review
reopens at a price nobody can pay then.

Prior art. `x86_64` spells the quantity `Size4KiB::SIZE` through a type, which is a different shape.
Linux and the BSDs say `PAGE_SIZE`, the spelling this tree's rule refuses for its own names because
it names no unit. The tree already made the choice once for `PAGE_BYTES`.

Example. `kernel/src/revoke.rs` line 157 reads `size_of::<LogPage>() == page_frames::FRAME_SIZE as usize`
and would read `page_frames::FRAME_BYTES as usize`, which names the quantity the comparison measures.

### Decision 2: Whether `addr` and `from_addr` become `address` and `from_address`

Background. `PageFrame` has three methods: `from_addr`, `containing` and `addr`. The first and last
carry `addr`, an abbreviation [design/naming.md](../naming.md) counts among those that need a
decoder, and the template's abbreviations report names it five times in this crate. The list of
words the gate refuses outright does not include `addr` yet, so nothing mechanical fails. The one
sibling accessor with the same meaning spells `address` (`crates/paging/src/tlb_flush.rs` line 41). The
kernel calls `from_addr` at about a dozen sites and `addr()` at more. Both are public methods, so
after 0.1.0 the names are a one-way door.

Recommendation: rename to `address` and `from_address` now, in the same lane as Decision 1.
`containing` stays. It reads well and `x86_64` spells the same operation `containing_address`, which
this tree would shorten anyway because the receiver type already names what the frame contains.

Prior art. `x86_64::PhysFrame` has `start_address` and `from_start_address`; `bitmap-allocator`
and `buddy_system_allocator` work in frame numbers and have no accessor. No listed crate
abbreviates the word.

Example. `kernel/src/memory.rs` line 569 reads `.is_used(PageFrame::from_addr(pa & !(FRAME_SIZE - 1)))`
and would read `.is_used(PageFrame::containing(physical_address))` after the rename lane also
takes the chance to use the method the crate already has for that rounding.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, since the crate stays
with the kernel and is unpublished, which the architect may revisit. The names go one step past the
recommendation: `physical_address` and `from_physical_address` (`src/lib.rs` lines 48 and 62),
because [design/naming.md](../naming.md) asks every address to say which kind it is. The std rule
does not bind here: std's pointer `addr` returns a pointer's integer without its provenance, a
different meaning. `containing` (line 57) stays. The kernel and `system_tests` call `from_addr` and
`addr()` at 191 sites, all renamed in item 8's lane, with no alias of the old spelling. Ratifying
the names stays the architect's.

## Done when

The checks are in [the appendix](1103-page-frames-outside-review/done-when.md).

## BUGS

- Items 1, 2, 3, 7 and 10 describe what the template's gates report from the helpers run by hand
  at the template's head and from one dry run at `v7`, not from a workflow run. A lane checks each
  against the staged tree before acting.
- The prior-art rows for Linux, FreeBSD, OpenBSD and `bootloader_api` are from memory and marked
  unverified where the function name or the rounding direction matters. A lane confirms the sources
  before a decision cites them.
- The `rust-version` of item 1 is a belief, not a measurement; the item says how to measure it.
- Item 10 names the regeneration helper by its role. The exact line that drops the blank context
  line was not located, and the lane that fixes it finds it.
- Nobody ran a mutation census; the figures are the tree's records. Items 4, 6, 9, 12 and 14 add
  branches the next census judges.

## Index row

An outside review of `page_frames` before its move. Three items block the move: a manifest that does
not stand alone, no README while the design arguments stay behind in nife, and documentation that
cites the tree. Seven land before the first release. Among them are `mark_free` rounding usable
memory outward against its own documentation and every comparable system, and proofs that leave the
rounding and the accounting unproven. Another is a malformed regenerated falsification patch that
stops the dry run, and two public names that fail the naming rules and go to the architect. Ten
follow later.
