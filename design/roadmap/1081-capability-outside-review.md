---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1034
decision_dependencies: 151, 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1081. `capability`: what an outside maintainer would fix before it leaves

Raised by a read-only outside review of `crates/capability`, met as a stranger to this tree would
meet it on the day it becomes `nifeos/<crate>`. The review changed nothing; it lists what the move
lane and the lanes after it change.

The crate has no review package under `notes/crate-reviews/` and no row in
[`notes/crate-reviews.md`](../../notes/crate-reviews.md), so this review has no settled baseline to
inherit. Whether a package is in flight on a branch is unverified. The review treats as settled anyway
whatever a ruling in the tree already covers. That is the crate name as ratified against `caps`
and `cap_space` (reopened below for one reason only, item D1), `CapabilityTable` as the exported
type and `MAXIMUM_SLOTS` as ratified. It also covers the five right bits and their positions (the
syscall wire format, pinned by `abi::rights` and asserted equal in
`kernel/src/capability_object.rs`), the set-means-free word, the 64-slot ceiling and the
publishing hold. Where a later rule contradicts an earlier
ruling, the item says so.

The kernel is the crate's only consumer (`kernel/Cargo.toml` line 52; no other manifest in the tree
depends on it). Milestone 1034 (a crate move runbook) owns the move itself; this block owns the
findings that should land inside that path, and the two findings against the move tool that the
lane will hit first.

Reuse: `script/crate-move`, the template `nifeos/crate-workflows` at its newest release and at head,
`helpers/falsify.py`, `helpers/standalone.py`, the template's naming helpers, cargo-mutants' recorded
rows, Kani, and the crate's own harness builder (`any_small_capability_table`). No new tool.

## Goal

A stranger who clones the crate's repository can tell in one screen what it is for, and can build, test, prove and falsify it from the README alone. The property the README leads with (a capability passed on never carries more than its source held) holds by proof on the code the kernel runs, not on a neighbor of it. The repository's gates are green for the crate's own reasons on its first
pull request.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1081-capability-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1081-capability-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the move or the release (a repository name, a
published name, a published behavior, a layout, a license); everything else is two-way and the
maintainer decides it. Every name proposed below is provisional.

### P0: blocks the move

1. The manifest does not stand alone, and the files the template wants are absent.
   Finding: `Cargo.toml` inherits `license.workspace = true` and `[lints] workspace = true`, which
   name a workspace the new repository does not have; it carries no `rust-version`, `description`
   or `repository`; its `publish = false` comment points at `crates/publish-allowlist.txt`, a file
   that will not be there. There is no README, no `LICENSE-MIT`, no `LICENSE-APACHE`. The template's
   `check / standalone` reports all of this and fails. Outside the workspace the `#[cfg(kani)]`
   module warns under `unexpected_cfgs`, which `-D warnings` turns into an error, because the
   `check-cfg = ['cfg(kani)']` line lives in the workspace's lints. The move tool's `add_repository`
   inserts `repository` after a `license =` line and finds none in this manifest, so the staged
   manifest has no `repository` while the tool reports the step as done (see item 6).
   Evidence: `crates/capability/Cargo.toml` lines 5 to 9; `helpers/standalone.py` on the staged
   tree: 10 lines; `crate-workflows/sample/Cargo.toml` for the shape.
   Change: `license = "MIT OR Apache-2.0"`, `rust-version = "1.85"` (edition 2024 needs it; the
   tests pass there), `description`, `repository`, an own `[lints.rust]` block with `missing_docs`,
   `unsafe_op_in_unsafe_fn` and the `unexpected_cfgs` line, and a comment on `publish = false` a
   stranger can act on. Add both license files. Write the README in the template's section order
   (`readme_sections.py`): summary, EXAMPLES, "What is proven", Building and testing with the Kani
   install line on one line, one real falsification example, BUGS, Minimum Rust version, License.
   The two crate-level doctests are the EXAMPLES.
   Door: two-way. Effort: S.

2. The crate's text refers back into nife.
   Finding: `src/lib.rs` and the fourteen patch headers cite section numbers (15 lines) and milestones (29 lines). They also cite paths into the tree (20 lines: `notes/capabilities/table.md`, `kernel/src/...`,
   `design/roadmap/0778-...`, `script/verify`) and name the architect with dates (5 lines after
   the `Name:` blocks go; 7 before). The rule for a crate that leaves is that it never mentions
   nife, and a document states the present rule without its history. Two of the citations are
   already wrong inside the tree. `kernel/src/capability.rs` (line 213) does not exist; the file is `kernel/src/capability_object.rs`. `[split_never_widens_rights](../src/lib.rs)` (line 138) renders as a link to a file that rustdoc does not serve. `cargo doc -D warnings` does not catch it because it is a URL and not an intra-doc path. The crate documentation also carries kernel
   facts a generic reader cannot use ("Only `Rendezvous` consults it", "Only `Process` consults
   it", lines 70 and 79) and the phrase "A thread's capability table," above (line 43) points at
   text that now sits below it.
   Evidence: `helpers/standalone.py` citation counts on the staged tree. `src/lib.rs` lines 38 to 43, 66 to 79, 123 to 124, 136 to 141, 150 to 155, 173, 186 to 192, 209 to 217, 237 to 241, 496 to 506, 529 to 534 and every harness comment. Also the patch headers.
   Change: rewrite each doc comment to state what the item does and why, in the crate's own words. Move the reasons that need the kernel to `notes/capabilities/table.md`, which stays in nife. Turn the harness links into intra-doc links (`[`verification::split_never_widens_rights`]`
   under `#[cfg(kani)]` needs a plain code span instead). Rewrite the patch preambles to name the
   defect and the assertion only. Keep the dates out: git is the history. Do this in nife before
   the move, so the dry run reads zero citation lines.
   Door: two-way. Effort: M.

### P1: before the first green pull request in the new repository

3. The proved `derive` is not the delegation the kernel runs.
   Finding: `CapabilityTable::derive`, the subject of `derive_never_widens_rights` (which the
   crate, `notes/verification/core-harnesses.md` and the lifecycle note all call the central
   theorem), has no caller in the kernel. Neither do `put` and `get_with`. Delegation runs in
   `kernel/src/sched/capabilities.rs` (`Delegation::derive`, lines 366 to 383), which rereads the
   source, requires `GRANT`, requires the subset, and builds the copy by hand; the two `REVOKE`
   arms in `kernel/src/syscall.rs` (lines 198 and 212) check `GRANT` the same way. None of those
   three sites is under a Kani harness (`kernel/src/syscall/proofs.rs` has none for them). Inside the crate, the docs describe `Rights::GRANT` as the right to pass a capability on, and nothing in
   the crate consults it: an outside user who calls `derive` gets a copy of a `GRANT`-less
   capability. Within nife this widens nothing, because the kernel never calls `derive`; it means
   the proof and the code it describes have drifted apart, and the crate's own API contradicts its
   own documentation of `GRANT`.
   Evidence: `src/lib.rs` lines 62 to 64, 453 to 472, 717 to 758; `kernel/src/sched/capabilities.rs`
   lines 340 to 400; `kernel/src/syscall.rs` lines 196 to 214; `grep -rn '\.derive(' kernel/src`.
   Change: put the rule in the crate once, as a method on `Capability<O>`: `delegate(&self,
   rights: Rights) -> Result<Capability<O>, Error>` (name provisional), refusing without `GRANT`
   and refusing a widening, with a new error variant for the first refusal (`NotDelegable`,
   provisional). Have `Delegation::derive` and `CapabilityTable::derive` call it, so the kernel's
   lock discipline stays where it is and the rule has one body. Prove it: a harness over all 2^32
   source and requested rights that `Ok` implies `GRANT` held and the result's bits are within the
   source's, stated in raw bits as `derive_never_widens_rights` is, with a falsification patch that
   drops the `GRANT` check. Then either give `get_with` a caller or delete it; `put` stays as the
   proof builder's door and says so.
   Door: two-way (no behavior changes on the kernel's path when the rule is the same). Effort: M.

4. A slot number truncates on a 32-bit target.
   Finding: `get`, `put`, `delete` and (through them) `derive` cast `slot as usize`
   (`src/lib.rs` lines 389, 439, 448, 476; its range check guards `insert_at` first and `insert_at_or_above` uses `try_from`). On a target with 32-bit `usize`, slot `2^32 + k` names
   slot `k`. The caller only reaches its own table, so the caller gains no authority. The claim that "the
   entire security mechanism is a bounds check" stops being true as stated, and `get(u64::MAX)`
   passing the test (`usize::MAX` is out of range) hides it. No target nife builds is 32-bit, so
   this is a hole in the claim, not a defect a current user can reach.
   Evidence: the four lines above; `cargo clippy -W clippy::pedantic` names five
   `cast_possible_truncation` sites; `tests.rs` line 103.
   Change: make `storage::slot`, `fill` and `empty` take `u64` and convert with
   `usize::try_from(slot).ok()?`, so an unrepresentable slot is `NoSuchSlot` everywhere; one test
   with `1 << 32` beside `u64::MAX`; one sentence on `get`.
   Door: two-way. Effort: S.

5. `len` means capacity.
   Finding: `CapabilityTable::len` returns `N`, the capacity, while `is_empty` and `used` speak of
   occupancy, so a table with `len() == 64` is `is_empty()`. The naming rule says a method takes
   std's spelling and meaning where std has one, and `len` in std is the number of elements held.
   The ruling that standard-library names win came after this name, so this review rechecks it as the release-review rule allows. The kernel reads it as capacity at `kernel/src/sched/
   capabilities.rs` lines 88, 149, 303 and 328.
   Evidence: `src/lib.rs` lines 377 to 385; `design/naming.md`, "Shapes".
   Change: rename to `capacity` (std's word for exactly this on `Vec` and `HashMap`), keep
   `is_empty` and `used`, add `len` only if it returns `used`. Four kernel call sites move with it.
   Door: two-way before the release; a published name after it. Effort: S. Name provisional.

6. The move tool breaks every regenerated patch and skips the manifest step while reporting both
   as done.
   Finding: in the staged tree all 14 regenerated patches fail `git apply --check` with "corrupt
   patch" (the template's `proofs / falsify` uses `git apply`), while the tool's own `patch -p1
   --dry-run -F0` passes them. The regenerated hunk header for `subset_is_reflexive` says 7 old
   lines and the body carries 6: the trailing blank context line is gone. `regenerate_patch` calls
   `run`, which returns `(stdout + stderr).strip()`, and `strip()` removes a final line that is a
   single space, which is how `diff -u` writes a blank context line. Separately, `add_repository`
   matches only `^license\s*=` and this manifest has `license.workspace = true`, so the tool writes no `repository` line while the "rewrite" step reports it set.
   Evidence: `helpers/crate_move.py` lines 72 to 118 (`regenerate_patch`), 156 to 163
   (`add_repository`), 574 to 576 (`run`); the staged tree's `falsifications/
   verification.subset_is_reflexive.patch` lines 26 to 32; `crate-move-static.log` of the dry run.
   Change: have `regenerate_patch` take `diff`'s output without stripping (or strip newlines
   only), add a selftest patch whose hunk ends in a blank line, and make `add_repository` insert
   after `license.workspace` too, or fail loudly when it inserted nothing. Any crate whose hunk
   ends at a blank line hits the first; any crate that inherits its license hits the second, so
   both belong in the tool, not in this crate. The template's `standalone` job would have caught
   both on the first pull request; the point of the dry run is to catch them before it.
   Door: two-way. Effort: S. This item is against nife's tooling, not the crate.

7. Two mutators are outside every harness.
   Finding: `insert_at_or_above` (the floor insert the `std` runtime relies on) appears in no
   harness, so no proof covers the free word or the count after it. Only the builder's first fill exercises `put` over an occupied slot. One assertion covers the mark's monotonicity, as `peak() >= used()` after a delete, not as "never falls across any operation". A test, not a proof, reaches the `floor >= 64` arm of `first_free_from`.
   Evidence: `src/lib.rs` lines 294 to 302, 414 to 427, 601 to 677.
   Change: add `insert_at_or_above(kani::any(), ...)` and a second `put` to
   `the_free_mask_is_the_empty_slots` and `the_count_is_the_slots`. Add a harness that applies one
   symbolic operation of any kind to a symbolic table and asserts `peak` did not fall and
   `insert_at_or_above` never landed below its floor. Each new harness arrives with its
   falsification patch. The unwind bound stays N + 1 by the existing argument.
   Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1081-capability-outside-review/later-items.md).

## Decisions for the architect

One one-way door is open. Everything else above is two-way until the first release, and the review records it as provisional.

### D1. The crate's name is taken on crates.io

Background. Ratification chose the name `capability` over `caps` and `cap_space`. On crates.io
the name `capability` belongs to an unrelated published crate (a skill-tree library for games,
version 0.1.0, about 520 downloads all time, last updated more than a year before this review). The
move creates `nifeos/<name>`, the kernel's manifest pins that repository by `rev`, and every later
document names it. A crate repository exists to release on its own (§151). The publishing hold is a hold, not a decision never to publish. If the move creates the repository as `nifeos/capability` and a later rename lets the crate publish, the rename is a second move: a new repository, a new pin, and
a dead name in every record between.

Recommendation first: rename before the move, to `capability_table`. It is free on crates.io, it
names the thing the crate is (a table of capabilities with their rights), it reads at the use site
(`capability_table::CapabilityTable`, `capability_table::Rights`), and it keeps `capability` as the
word the type names carry. The earlier refusal of `cap_space` was that it named one of five exports
and stuttered; `capability_table` names the central export, and the stutter is the one `slotmap::
SlotMap`, `slab::Slab` and `generational_table::Table` all accept. Also free at the time of the
review: `capability_rights`, `capability_space`, `capability_model`, `capability_slots`,
`rights_table`. Refused in this review: a `nife_` prefix (a crate that leaves does not mention
nife), and keeping `capability` as a repository name that can never publish (it closes the door
§151 opens).

Prior art. Fuchsia publishes its handle and rights types as `fuchsia-zircon`, a name for the system and not for "handle". seL4's term is `CSpace`, which this review already refused. The Rust crates for a slot structure take the structure's name (`slab`, `slotmap`, `thunderdome`, `generational_table`).
Rust's convention for a taken name is a qualifier, not a prefix (`cap-std`, `cap-primitives`).

Concrete example. After the rename the kernel's line is `capability_table = { version = "0.1.0",
git = "https://github.com/nifeos/capability_table", rev = "<tag commit>" }` and the two kernel files
that say `capability::` (`kernel/src/capability_object.rs` and `kernel/src/sched/region_reaping.rs`,
17 lines) say `capability_table::`; `capability_object.rs` keeps its alias `pub(crate) type
CapabilityTable = capability_table::CapabilityTable<Object, CAPABILITY_TABLE_SLOTS>`. Effort: S in
nife (one dependency line, two source files, the workspace member list, the notes and roadmap pages
that name the crate), before the dry run.

Door: one-way (a repository name and a published crate name). This review rechecks the name under the rule that a later fact reopens a ratified name at the release review. The later fact is the collision, which the original ruling could not have weighed if the name was free then (nothing confirms that).

## Done when

The checks are in [the appendix](1081-capability-outside-review/done-when.md).

## BUGS

- Item 4 came from reading and from clippy; the review ran no 32-bit build, because the host has no 32-bit target and the review adds nothing to a machine.
- Ten of the fourteen falsification patches were not replayed; the four that were each failed
  exactly their harness. The tree's own `script/verify` replays all fourteen on its cadence
  (unverified for this head).
- The dry run ran with `--no-build` and with the template at its newest release. The template's
  head carries checks the release lacks (spelled-out words, unit symbols, passive voice, function
  length); the review ran the two naming ones by hand and they pass, and it did not run the other two.
- The review did not look for a review package on any branch; if one exists, its rulings win over
  the provisional names here.
- Item 16 is optional, and the review measured it only on a 64-bit host.
- Nothing confirms the claim in D1 that the name was free when ratified: crates.io's API gives the
  other crate's creation date and the review did not compare it.

## Index row

An outside review of `capability` before it leaves. Two items block the move (a manifest that leans on the workspace and a README that does not exist; text that cites nife on 69 lines). Five land before the first green pull request. Three are the proved `derive`, which is not the delegation the kernel runs, a 32-bit slot truncation, and `len` that means capacity. The other two are two defects in the move tool that corrupt all fourteen patches and skip the manifest step, and two mutators outside every harness. Nine items are for later. One one-way door remains: another crate already holds the name `capability` on crates.io.
