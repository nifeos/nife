---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017, 1034
decision_dependencies: 16, 113
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1101. `memory_regions`: what an outside maintainer would fix before 0.1.0

Raised by a read-only review of `crates/memory_regions` as a stranger to this tree would meet it,
made before any move of the crate to a repository of its own. It changes nothing; it lists what the
next lanes change, in nife before a move where the runbook puts a finding against the crate itself,
and in the new repository after it.

The review found no review package for this crate in `notes/crate-reviews/` and no row for it in
`notes/crate-reviews.md`; `notes/package-boundaries.md` places the crate in the `kernel` base
package. So there is no settled baseline to leave closed, and the first decision below asks
whether the crate leaves at all. What this review does not reopen: the crate's name (`Name:
ratified`, `src/lib.rs`), the rename from `regions`, the kernel's `RETYPE` count rule for the
syscall word, the `MIT OR Apache-2.0` license, and the pin of `generational_table` at its
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

1. A split refused for a full table still bumps the parent, which nothing can ever reclaim.
   Finding: `split` checks the budget, charges the `Region` ceiling, advances the parent's watermark
   and raises its child count, and only then asks the table for a slot. When the table is full the
   child is never made and the ceiling charge goes back, but the watermark and the count stay
   raised. No name exists that could ever bring the count to zero. So `claim_for_destroy` refuses
   the parent for the rest of the boot, and every refusal leaves the table fuller. The crate's own
   `# BUGS` and doctest record it, and `notes/region-split-on-a-full-table.md` proposes the fix and
   records that both reasons for the rule (two lock holds, headroom) lapsed when `split` became one
   borrow. The proposal has waited since its author filed it. Every neighbor refuses without side
   effects (`retype_object_page`, `retype_run`, the kernel's `create`), and the crate's own
   harnesses prove "a refused split moves nothing" for every refusal but this one.
   Evidence: `src/table.rs` lines 440 to 474 (the order), `425-439` (the reproduction), `769-771` (the
   coalescing consequence). `notes/region-split-on-a-full-table.md` option B.
   Change: reorder so every refusable step comes before any irreversible one: resolve the parent,
   compute the new watermark, charge the ceiling, insert the child (on `None`, release the charge
   and return), then bump the parent under the same borrow. The doctest becomes a regression test
   ("a split refused for a full table leaves the parent as it was, and the parent is reclaimable
   once its real child returns"), `a_refused_split_moves_nothing` gains a cover for the full-table
   refusal (a `Run` whose table has no free slot), and the `# BUGS` entries in `split`,
   `return_to_parent`, `notes/memory-regions.md` and `kernel/src/memory_region.rs` go. The kernel's
   `SPLIT` still answers `OutOfMemory`.
   Door: two-way. Effort: S.

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

Four decisions, each with a recommendation first. Decisions 2, 3 and 4 are decided, provisionally,
below. Everything else above is two-way and the maintainer decides it. The names in item 9 wait for
a naming pass.

### Decision 1: Whether `memory_regions` leaves nife now, and in what shape

Background. The crate has one consumer, the kernel, which reaches it by path. Its vocabulary is the
kernel's: `ObjectKind` lists seven nife kernel object kinds and the table counts nothing else.
`retype_pages` encodes the syscall word's rule that zero means one. The pin exists for the kernel's
lock order. No review package exists for it, `notes/crate-reviews.md` has no row, and
`notes/package-boundaries.md` places it in the `kernel` base package. The standing rule is that a
crate leaves only when it is publishable or used by more than one repository. The template cannot
run its loom harnesses (item 5). Creating a repository and publishing are the one-way steps. The
crate's shape when it arrives is what a second consumer acts on.

Recommendation: hold the move until the crate has a second consumer or its object kinds are the
caller's to define, whichever comes first, and meanwhile land items 1, 3, 4 and 6(d) in nife, where
they help the kernel today. If it leaves now, it leaves with `Counted` generic over the caller's
kind set: `RegionTable<T, const N: usize, const KINDS: usize>` or a `Kinds` trait the kernel
implements with its seven, so the published API carries no nife kernel object names. The cheaper
path (leave as is) would publish `ObjectKind::Rendezvous` to crates.io for a crate nobody else can
use as it stands.

Prior art. seL4 fixes its object types in the kernel and ships them to user space as constants in
`libsel4`, not as a library another kernel reuses. Linux's `page_counter` is a generic hierarchical
counter inside one tree. `slotmap` and `generational_table` are generic over the value they hold
(all unverified in wording).

Example. A second kernel, or a hypervisor accounting guest pages against a budget tree, would call
`retype_object_page(name, ObjectKind::Rendezvous)` for a thing that is not a rendezvous, or fork the
crate. With kinds generic it calls `retype_object_page(name, MyKind::Vcpu)`.

### Decision 2: Whether region names cross the crate's boundary as `u64` or as a typed name

Background. The dependency's whole point, in its own words, is that a name carries its table's entry
type so no caller can hand a name from one table to another. This crate undoes that at its own
boundary: every public method takes and returns `u64`, and `entry()` at `src/table.rs` lines 331 to 337 calls
itself "a temporary adapter" that "goes when those methods take `EntryName<Region>` themselves".
`Region` is private, so that cannot happen as written. The crate needs a public opaque marker. A
method signature is one-way once published.

Recommendation: typed names before the first release. `pub struct RegionName(EntryName<Region>)`
with `to_bits` and `from_bits` for the syscall register, or a public zero-sized `RegionTag` and
`pub type RegionName = EntryName<RegionTag>`. The kernel converts once at its syscall boundary, as
it already converts bytes to pages there. The `u64` form is the convenient one for the kernel's
current code and nothing else.

Prior art. `slotmap` keys are a type per map (`new_key_type!`), `thunderdome::Index` and
`generational_table::EntryName<T>` carry `to_bits` and `from_bits` for exactly this boundary
(unverified detail).

Example. `table.unpin(claim.pages())` compiles today, though a name belongs where the page count
sits. With a typed name it does not.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door while the crate stays
with the kernel, which the architect may revisit. Region names cross the crate's boundary as a typed
name. A public marker type lets `EntryName<...>` appear in the signatures, as the generational
table's entry names are a type and not a bare `u64`. The kernel converts once at its syscall
boundary, and `entry()` goes. The names of the marker and the alias are provisional and go through
item 9's naming pass.

### Decision 3: Whether "zero pages means one" is the crate's contract or the kernel's

Background. The kernel's `RETYPE` count rule says a word of zero asks for one page, because the
syscall ignored the argument before the count existed. That ruling stands and this decision does not
reopen it. The crate carries it as `retype_pages` and `retype_new_watermark`, so `retype_run(name,
0)` hands out a page while `split(parent, 0)` refuses. A stranger reading `retype_run(name, 0)`
expects nothing to happen, and a published function's behavior on zero is one-way.

Recommendation: the crate refuses zero everywhere, as `split` does, and the kernel maps the syscall
word's zero to one before it calls (`kernel/src/memory_region.rs` line 255 already computes the count;
it moves one line earlier). `retype_pages` leaves the crate. The kernel's syscall behaves as ruled.

Prior art. seL4's `seL4_Untyped_Retype` refuses a zero object count with an invalid-argument error.
POSIX `read` with a zero count returns zero and moves nothing (both unverified in wording).

Example. `for n in 0..3 { table.retype_run(name, n); }` retypes four pages today (one for the zero)
and three under the recommendation.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door while the crate stays
with the kernel, which the architect may revisit. The syscall behaves as ruled; only the place of the
rule moves. The crate refuses zero everywhere, as `split` does (`src/table.rs` line 433).
`retype_pages` (`src/watermark.rs` line 40) leaves the crate, and the kernel maps the syscall
word's zero to one before it calls (`kernel/src/memory_region.rs` line 262). Under a stricter rule
first, a crate that later accepts zero stays compatible.

### Decision 4: Whether a child's share of its root's ceilings is the crate's constant

Background. `CHILD_SHARE_OF_ROOT` is 16, private, and every split region of every tree gets one
sixteenth of its root's ceilings, with integer division. A root ceiling under sixteen gives every
child zero (item 10). The kernel chose sixteen for the progenitor's tree. A published crate makes
that every user's policy, and a published policy number is one-way.

Recommendation: the share becomes a field of the root's `Ceilings` (or a parameter of
`insert_root_with_ceilings`), with the kernel passing 16. Zero stays zero (the stricter answer;
relaxing later is compatible). The crate keeps the invariant that no child's ceiling exceeds its
parent's.

Prior art. Linux's `pids.max` and `memory.max` belong to each group, and whoever holds the parent
sets them, not by a fixed fraction. Zircon job policies belong to each job (the wording of neither
description has a check).

Example. A root with a thread ceiling of 15 can give one child 8 and another 7 under the
recommendation; today both children get 0.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door while the crate stays
with the kernel, which the architect may revisit. This differs from the recommendation above, which
assumed a published crate. `CHILD_SHARE_OF_ROOT` stays private (`src/table.rs` line 143) while the
kernel is the crate's only user, and item 10's limit becomes a `BUGS` line in the crate. If the
crate is ever offered for publication, the share as a field of the root's `Ceilings` is a
precondition of that offer.

## Done when

The checks are in [the appendix](1101-memory-regions-outside-review/done-when.md).

## BUGS

- Items 2, 5, 6 and 8 describe what the template's gates report from the helpers run by hand at the
  template's head and from a `--no-build` dry run of the move. No workflow ran. A lane checks each
  against a built staged tree before acting.
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
the template refuses. Seven follow later. Four decisions follow, the first being whether the crate
leaves at all; the other three are decided, provisionally.
