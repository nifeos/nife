---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017, 1034
decision_dependencies: 16, 113
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1101. `memory_regions`: what an outside maintainer would fix before 0.1.0

Raised by a read-only review of `crates/memory_regions` as a stranger to this tree would meet it,
made before any move of the crate to a repository of its own. It changes nothing; it lists what the
next lanes change, in nife before a move where the runbook puts a finding against the crate itself,
and in the new repository after it.

The review found no review package for this crate in `notes/crate-reviews/` and no row for it in
`notes/crate-reviews.md`; `notes/package-boundaries.md` places the crate in the `kernel` base
package. So there is no settled baseline to leave closed, and the crate stays in the kernel base package
until the trigger under "Held" below fires. What this review does not reopen: the crate's name (`Name:
ratified`, `src/lib.rs`), the rename from `regions`, the kernel's `RETYPE` count rule for the
system call word, the `MIT OR Apache-2.0` license, and the pin of `generational_table` at its
`v0.1.0-pre.1` tag. No later rule in [design/naming.md](../naming.md) contradicts the ratified
crate name. Every quantity in the crate is a count of pages or of objects and names the thing, so
the unit rule has nothing to say; `len`, `is_empty`, `get` and `with` take std's spellings.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-move --dry-run`,
Kani, loom, cargo-mutants, and the crate's own harness helpers (`Run::any`, `Run::for_every_depth`,
`RegionSnapshot`). No new tool.

## Goal

A stranger who finds `memory_regions` can tell in one screen what a region, a watermark, a pin, a
claim and a ceiling are, and why a child's pages never reach the allocator. The same stranger can
follow the README alone to a build, the tests, the proofs, the interleaving search and one
falsification, and meets no sentence the code contradicts. No input a caller can pass makes the
crate panic. The repository's `check`, `proofs` and interleaving runs are green on the first push
for the crate's own reasons.

## Finding status

- P0-1, a split refused for a full table: Done, [#3525](https://github.com/nifeos/nife/pull/3525).
- P0-2, the crate cannot stand alone: At the move. The doc comments and the manifest comments name
  no nife path, pull request or section. The README, both license files, an explicit license line,
  `rust-version` and inline lints are the move's work, and Decision 1 holds the move until its
  trigger fires.
- P1-3, a root at the top of the page-number space: Done. The refusal landed in [#3534](https://github.com/nifeos/nife/pull/3534), and the
  Kani harnesses `a_root_the_table_accepts_never_overflows_*` (four, one per operation, split to fit
  the shard's memory) drive one operation from any accepted root, each with its falsification
  patch, in [#3825](https://github.com/nifeos/nife/pull/3825).
- P1-4, a second release of one object: Done, [#3532](https://github.com/nifeos/nife/pull/3532).
- P1-5, the loom harnesses at the move: At the move. The template's `proofs` workflow runs a crate's
  loom models when its manifest depends on loom, from
  [v8](https://github.com/nifeos/crate-workflows/pull/65); the README's split of what loom checks
  and what Kani proves is written with the README.
- P1-6, gates of the template: Open. (b) and (c) landed in [#3567](https://github.com/nifeos/nife/pull/3567), and (d) in [#3521](https://github.com/nifeos/nife/pull/3521). (a) is
  open: the template's address rule still refuses `ObjectKind::AddressSpace`. (e) is the move's,
  which pins the template at `v8`.
- P1-7, documentation the code contradicts: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P1-8, manifest metadata and the dependency line: At the move. The manifest has `description`,
  `keywords` and `categories`; `documentation`, `readme`, `repository` and `rust-version` come with
  the move, and the `git` dependency waits for `generational_table` to publish.
- P1-9, public names for the naming pass: Waiting on Decision 1's trigger, a second consumer or
  object kinds the caller defines. The name ratification under "Held decision" is asked then.
- P2-10, a root ceiling under sixteen: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-11, the cost of `spent` and of the return: Open. The doc comments state the cost
  ([#3567](https://github.com/nifeos/nife/pull/3567)); a per-parent page count in place of the scan is a two-way door the maintainer rules.
- P2-12, `ObjectKind::ALL`: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-13, `forbid(unsafe_code)`: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-14, a table-level invariant harness: Done, [#3567](https://github.com/nifeos/nife/pull/3567), with its shard times in [#3647](https://github.com/nifeos/nife/pull/3647).
- P2-15, direct tests of the pure functions: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-16, a dropped claim: Open. The type's documentation says a dropped claim leaks;
  `return_to_parent` still takes `&DestroyClaim`, and `DestroyClaim` has no `Drop`. The signature
  is a two-way door until the first publish, and the maintainer rules it.
- Decision 1, whether the crate leaves nife: Done, [#3803](https://github.com/nifeos/nife/pull/3803), ruled 2026-11-19 (UTC), option A.
  The crate stays in the kernel base package, and the move waits until the crate has a second
  consumer or its object kinds are the caller's to define, whichever comes first.
- Decision 2, typed region names: Done, [#3825](https://github.com/nifeos/nife/pull/3825), ruled by the maintainer, provisional,
  2026-11-14 (UTC). `RegionName` is `EntryName<Region>`, `Region` is the public opaque marker, and
  `entry()` is gone. The kernel converts with `to_bits` and `from_bits` in
  `kernel/src/memory_region.rs`.
- Decision 3, zero pages: Done, [#3825](https://github.com/nifeos/nife/pull/3825), ruled by the maintainer, provisional, 2026-11-14 (UTC).
  `retype_pages` is gone, `retype_run(name, 0)` is refused, and the kernel maps the system call
  word's zero to one.
- Decision 4, the child's share: Done, [#3567](https://github.com/nifeos/nife/pull/3567), ruled by the maintainer, provisional, 2026-11-14
  (UTC). `CHILD_SHARE_OF_ROOT` stays private and its documentation states the limit.

## Held decision

The crate stays in the kernel base package (`notes/package-boundaries.md`). The move to a
repository of its own waits until the crate has a second consumer, or until its object kinds are
the caller's to define, whichever comes first. The kernel is the one consumer today, and
`ObjectKind` lists seven nife kernel object kinds, so publishing now would put
`ObjectKind::Rendezvous` in an interface nobody else can use as it stands. Ruled 2026-11-19 (UTC),
option A of the question whether `memory_regions` leaves nife; the options and their prior art are
in [the appendix](1101-memory-regions-outside-review/decisions.md). Creating a repository and
publishing are outward steps, so the trigger is the one-way door, and a lane that sees it fire
brings the move back as a decision.

### Ratify the crate's public names (held)

Background. Every public name but the crate's own is provisional. A move strips the `Name:`
blocks that say so, so the architect ratifies the names first. `helpers/name_release.py --draft
memory_regions` lists them. Five read wrong to a stranger. `DestroyOutcome::FreeToGlobal` says
"global" where it means the frame allocator. `ReturnToParent { unbump }` is not a word.
`is_lifo_top` carries an acronym the prose gate refuses. `Counted` is a participle naming a type.
`usage` and `bounds` both return `(u64, u64)` with different meanings, so a swapped call
type-checks.

Recommendation: A. The candidates follow the spelled-out and question-shaped rules in force, and
two small structs turn a swapped pair into a compile error.

- A. Rename to `FreeToAllocator`, `ReturnToParent { pages_given_back }`, `is_at_watermark`,
  `CountedKind`, and `Usage { watermark, pages }` and `Span { base_page, pages }`; ratify the rest
  as they stand.
- B. Ratify with other spellings the architect names, applied by one rename lane.
- C. Defer the pass until the crate leaves.

Door: one-way at the first publish, when each public name becomes a breaking change to rename.
Before that it is two-way, but ratification is the architect's by the naming rule.

Prior art. `slotmap` and `thunderdome` return named key types rather than bare integers, and std's
`Range` names its two ends where a tuple would not (recalled). nife differs in ratifying every
public item before it leaves, not only the documented ones. Concrete example: today
`let (base, pages) = table.usage(name)` compiles and reads the watermark as a base page; with
`Span` and `Usage` it does not.

Held: this question is not asked until the trigger above fires, and the move waits on it.
Blocks: the move, and the first publish.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1101-memory-regions-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1101-memory-regions-outside-review/prior-art.md).

## The items

Effort: XS is under an hour, S half a day, M a day or two, L longer. A door is one-way when somebody
outside the repository acts on it after a release (a public name, a published behavior, a wire
value, a layout, a dependency). Everything else is two-way and the maintainer decides it. Line
numbers are in `crates/memory_regions/` at `75bb255a`.

### P0: blocks any publish

1. A split refused for a full table leaves the parent as it was. Done,
   [#3525](https://github.com/nifeos/nife/pull/3525). Door: two-way.

2. The crate cannot stand alone, and the template's `check / standalone` is red on the first push.
   Finding: no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, `license.workspace = true`,
   `lints.workspace = true`, no `rust-version`. 15 lines name a path in this tree, 2 name the
   architect (in two falsification patches), 5 cite a decision section and 9 a milestone. The staged
   tree carries the word `nife` in five files, four of them the scaffold's own. The doc comments send a stranger to `kernel/src/memory_region.rs`, `sched::reclaim_region`, `abi::object_type::*`, `kernel/src/kmem.rs`, `script/lint`, `notes/*.md`, pull request #316 and milestones 126, 135, 326, 601 and 812. They also cite DECISIONS §16 (object revocation: reclaim the objects a process built). They cite §113 (eleven kernel object and identifier names move from contraction or borrowed jargon to the plain, standard term). They cite §225 (`free` sees the machine and your share: a region method and a withholdable memory page). The repository holds none of these. The `publish = false`
   comment points at `crates/publish-allowlist.txt`.
   Evidence: `Cargo.toml` lines 5, 12 to 14 and 16 to 21. `src/lib.rs` lines 1 to 12. `src/table.rs` lines 1 to 11, 47 to 50, 57 to 60, 87 to 104, 231 to 236, 350 to 359, 418 to 424, 585 to 590, 639 to 644, 657 to 663, 762 to 771 and 1387 to 1395. `src/destroy.rs` line 3.
   `src/watermark.rs` lines 4, 35 to 38 and 56 to 57. The `standalone.py` run and the `crate-move` dry run above.
   Change: in nife before the move. A README in the scaffold's shape (`README-crate.md`). It has the
   badge row and one line of purpose. An `## EXAMPLES` section is the `RegionTable` doctest. A `##
   What is proven` section has one line per harness in a user's words and a second list for what
   loom checks (item 5). `## Building and testing` names the three bare-metal targets. `### Running
   the proofs` gives the Kani install line and the measured time and memory. `### Running one
   falsification patch` uses `proofs.split_stays_within_budget_and_progresses.patch` as the example.
   `## BUGS` carries the hole-under-a-live-child limit and the `OutOfMemory` collapse as the crate
   sees it. The last two sections are `## Minimum Rust version` and `## License`. Both license
   files. `license = "MIT OR Apache-2.0"`, `rust-version = "1.88"` (the index's shared choice;
   nobody has verified that the crate builds there), an inline `[lints.rust]` table carrying
   `missing_docs`, `unsafe_op_in_unsafe_fn` and `unexpected_cfgs` for `kani` and `loom`. Every doc
   comment above rewritten to say the thing rather than cite where nife said it: the pin's reason
   ("a kernel object lives in a page of this region; the caller clears the pin after it has torn the
   objects down"), the claim's reason ("the caller holds either the table or a claim, never a
   decision about a table it no longer holds"), the kinds as what they count. The `publish` comment
   reads "set to `true` only in the release that publishes".
   Door: two-way. Effort: M.

### P2: later

The P2 items are in [the appendix](1101-memory-regions-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1101-memory-regions-outside-review/before-the-tag.md).

## Decisions for the architect

The maintainer ruled Decisions 2, 3 and 4, provisionally, as two-way doors while the crate
stays with the kernel; the architect may revisit them. Decision 1 is the held decision above.
The case behind each ruling (background, recommendation, prior art and an example) is in
[the appendix](1101-memory-regions-outside-review/decisions.md).

### Decision 2: Whether region names cross the crate's boundary as `u64` or as a typed name

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door while the crate stays
with the kernel, which the architect may revisit. Region names cross the crate's boundary as a typed
name. A public marker type, `Region`, lets `EntryName<Region>` appear in the signatures as
`RegionName`, as the generational table's entry names are a type and not a bare `u64`. The kernel
converts once at its boundary. The names of the marker and the alias are provisional and go through
item 9's naming pass.

### Decision 3: Whether "zero pages means one" is the crate's contract or the kernel's

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door while the crate stays
with the kernel, which the architect may revisit. The system call behaves as ruled; only the place of the
rule moves. The crate refuses zero everywhere, as `split` does, and the kernel maps the system call word's zero to
one before it calls (`retype_run` in `kernel/src/memory_region.rs`). Under a stricter rule first, a
crate that later accepts zero stays compatible.

### Decision 4: Whether a child's share of its root's ceilings is the crate's constant

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door while the crate stays
with the kernel, which the architect may revisit. This differs from the recommendation in the appendix,
which assumed a published crate. `CHILD_SHARE_OF_ROOT` stays private (`src/table.rs` line 143) while the
kernel is the crate's only user, and item 10's limit becomes a `BUGS` line in the crate. If the
crate is ever offered for publication, the share as a field of the root's `Ceilings` is a
precondition of that offer.

## Follow-on

- **Outstanding.** Each finding that "Finding status" marks Open, checked against
  `crates/memory_regions` and its manifest. Findings it marks At the move or Waiting on Decision 1's
  trigger wait for the trigger under "Held decision".

## Done when

The checks are in [the appendix](1101-memory-regions-outside-review/done-when.md).

## BUGS

- Items 2, 5, 6 and 8 describe what the template's gates report from the helpers run by hand at the
  template's head and from a `--no-build` dry run of the move. No workflow ran. A lane checks each
  against a built staged tree before acting.
- The Kani harness `a_return_keeps_every_run_inside_its_parents_spent_part` assumes that a parent's
  retyped pages never overlap a live child's run. Building the start state from the table's real
  calls runs CBMC out of memory. A host test replays every sequence of up to five calls instead and
  checks that rule after each one. Neither covers a longer sequence or a table past four regions.
- The prior-art rows and the decisions' prior art are from memory where marked unverified; a lane
  confirms the sources before a decision cites them.
- This review reads item 3's release-profile consequence from the workspace manifest and has not
  observed it on a kernel.
- Reading the code places item 6(d)'s cause in `regenerate_patch`. Nobody has verified the exact
  line that drops the blank context line.
- Nobody ran a mutation census; the figures are the tree's records. Items 1, 3 and 4 add branches
  the next census judges.
- Nobody has verified that the crate builds at Rust 1.88, since the manifest has no `rust-version`
  to test.

## Index row

An outside review of `memory_regions` before any move. Two items block a publish: a split refused
for a full table that leaves a phantom child, and a crate that cannot stand alone. Seven land before
a first release. Among them are a panic on a root at the top of the page-number space, a double
release that loosens a ceiling for good, six loom harnesses with no runner after the move, and gates
the template refuses. Seven follow later. Of four decisions, the maintainer ruled three provisionally, and whether the
crate leaves is ruled: it stays with the kernel until a second consumer or caller-defined kinds
exist. The move waits, and the ratification of the interface's names waits with it.
