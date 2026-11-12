# P1: before the tag

An appendix to [the milestone](../1101-memory-regions-outside-review.md).

## P1: before the first release

3. A caller-supplied root at the top of the page-number space makes the crate panic.
   Finding: `insert_root` and `insert_root_with_ceilings` accept any `base_page` and any `pages`,
   zero included, and nothing checks any later sum. `insert_root(u64::MAX, 4)` followed by
   `retype_page` panics with "attempt to add with overflow" at `src/table.rs` line 604. A root at
   `u64::MAX - 3` with two children, the lower returned, panics at `src/table.rs` line 787 inside
   `return_to_parent`. Both verified with a throwaway test in a debug build. The tree builds its
   release profile with overflow checks on, so a kernel would panic too. A build without them hands
   out a wrong page number. `insert_root(0x100, 0)` succeeds and makes a zero-page region, while
   `split` refuses a zero-page child. The kernel never passes such values (its `base` is a physical
   address over `FRAME_SIZE`), so no program breaks today. A stranger's first `Table` with a
   sentinel base does.
   Evidence: `src/table.rs` lines 377 to 406 (no check), `449`, `604`, `632`, `787`, `792` (the sums).
   `Cargo.toml` of the workspace, `[profile.release] overflow-checks = true`.
   Change: `insert_root_with_ceilings` returns `None` unless `pages > 0` and
   `base_page.checked_add(pages)` is `Some`, and its documentation names that as the table's
   invariant ("a region's run ends inside the page-number space"), which bounds every interior `+`.
   A child's run lies inside its parent's, and a watermark never exceeds `pages`. One host test for
   each refusal, and one Kani harness that inserts any root the table accepts and runs any one
   operation with Kani's overflow checks as the assertion.
   Door: two-way (a refusal where a panic was). Effort: S.

4. A second `release_object` for one object undercounts every ancestor for the rest of the boot.
   Finding: `release` walks the region and its ancestors and saturates each count on its own. A
   region that holds nothing of the kind stays at zero while its ancestors, which hold their own
   objects as well, lose one each. Verified: a root with a thread ceiling of 32 holding one thread,
   a child holding one. Two releases on the child leave the root reporting zero live with one live,
   and two more thread carves succeed where the ceiling allows one. The kernel's three callers
   (`Threads::remove`, `AddressSpace::drop`, a refused insert) should release exactly once. The
   crate cannot tell when one does not, and the ceiling is then one looser per such bug, for good.
   The same shape in `charge` decides first and commits second so that a refusal writes nothing.
   `release` has no decision step.
   Evidence: `src/table.rs` lines 512 to 525 (`release`), `534-536` (`release_object`). The proofs'
   `Run::release` (`src/proofs.rs` lines 388 to 395) skips a release the region does not hold, so the harness
   never drives this case.
   Change: give a release the same two-step shape as a charge. Keep an own count per kind beside
   `live` (`own: [u32; COUNTED_KINDS]`, the ghost the proofs already keep in `Run::own`), raise it
   in `retype_object_page`, and have `release` refuse, writing nothing, when the named region's own
   count is zero. `release_object` returns `bool` so the kernel can log a double release. A host
   test for the case above, and `a_release_keeps_every_count_exact` calls the table's method for a
   kind the region does not hold and asserts `exact` still.
   Door: two-way now. The return type is public after a release. Effort: S.

5. The six loom harnesses lose their runner at the move.
   Finding: `script/interleaving-check` runs the crate's `interleavings` module under `--cfg loom`
   in nife's `interleavings` job, and the template's README records loom as dropped because none of
   the first crates used it. Only the loom model covers this crate's single-winner claim. The model
   covers two destroyers, a retype against a claim, a split against a claim, a parent against a
   returning child and a spending read against a return. It also holds the falsification witness
   `the_pre_fix_protocol_double_frees_and_this_model_finds_it`, which passes only when loom still
   finds the old double free. Kani does not model two callers (the crate says so). After the move
   those six run nowhere, and a change that reopens the check-then-remove gap passes every gate.
   Evidence: `src/table.rs` lines 1385 to 1761. `script/interleaving-check`.
   `.github/workflows/ci.yml` lines 1933 to 1972. `crate-workflows/README.md`, "What each nife gate became",
   the loom row.
   Change: a `loom` job, in the template as an opt-in input (`loom: true`) that runs `cargo test
   --release --lib` with `RUSTFLAGS="--cfg loom"` and the preemption bound nife's script sets, or
   failing that a repository-local workflow the move lane writes. And the README's "What is proven"
   says which properties loom checks and which Kani proves, so a reader does not take the Kani list
   for the whole story.
   Door: two-way. Effort: S in the repository, M in the template.

6. Gates of the template refuse the shipped source and one regenerated patch.
   Finding: (a) `register_names` refuses `ObjectKind::AddressSpace` under the rule that an address
   is `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS`. The variant names an address space, not an address,
   so the finding is the helper's, but the ceiling is 0 and `check / standalone` fails on it. (b)
   `question_shape` refuses `same_snapshot` and `unmoved_since` in `proofs.rs`, both
   `bool`-returning and not named as questions. The gate reads every tracked `.rs` file, `cfg(kani)`
   or not. (c) `prose_acronyms` refuses LIFO, TCB and IPC, each used before its words. (d) The move
   regenerates `proofs.a_release_keeps_every_count_exact.patch` for `-p1`, and the regenerated hunk
   has lost its trailing blank context line while its header still counts ten lines. GNU `patch`
   accepts the short hunk with no note, and the template's `falsify.py --check` refuses it as
   "corrupt patch at line 24" at `v7` and at head, so `proofs / falsify` is red. Any patch whose
   hunk ends on a blank line meets this. (e) `orphan_modules` at template `v7` reports
   `src/table/verification.rs` as in no module tree (the file sits under `#[cfg(kani)]`). The helper
   at head passes it, so a pin at a release after that fix clears it (nobody has verified which
   release).
   Evidence: `src/table.rs` line 90. `src/proofs.rs` lines 199 and 337. `src/destroy.rs` line 14, `src/table.rs` lines 48 and 87.
   The staged tree's `falsifications/proofs.a_release_keeps_every_count_exact.patch` against the
   original at `falsifications/proofs.a_release_keeps_every_count_exact.patch:24`.
   `helpers/crate_move.py` lines 72 to 116 (`regenerate_patch`).
   Change: (a) in the template, exempt `address_space` as a compound the rule does not cover, with a
   selftest. (b) `is_same_snapshot`, `is_unmoved_since`. (c) "last in, first out (LIFO)", "thread
   control block (TCB)" and "inter-process communication (IPC)" at each first use, or the plain
   words with no acronym. (d) in nife's `helpers/crate_move.py`, keep a hunk's trailing blank
   context line through regeneration and add a selftest patch whose hunk ends on one. (e) pin the
   template at the release that carries the `orphan_modules` fix.
   Door: two-way. Effort: S.

7. The documentation says things the code does not do.
   Finding: `RegionTable`'s doc says "the kernel picks 256". `MAXIMUM_REGIONS` is 1,024. `usage`
   says it returns "how many pages the region has retyped". The first figure is the watermark, which
   counts pages carved into children too (the test at `src/table.rs` line 848 reads 12 of 16 after three
   splits and no retype). `NO_PARENT`'s doc argues from the dependency's slot bits. The dependency
   names the same word `EntryName::NEVER_MINTED` and documents it as never minted, which is the fact
   to cite. `ceiling_reached`'s `levels_up` is "how many splits above the refused region", which
   reads as a count of splits rather than of ancestors. The crate root's doc names the two
   arithmetic functions and the `destroy_outcome` decision and not the table, the claim, the pin,
   the return or the ceilings, which is most of the crate.
   Evidence: `src/table.rs` line 297, `kernel/src/memory_region.rs` line 32. `src/table.rs` lines 651 to 655.
   `src/table.rs` lines 22 to 27. `src/table.rs` lines 205 to 206. `src/lib.rs` lines 1 to 7.
   Change: "`N` bounds the regions live at once; the loom harnesses use 2 or 4". "the watermark
   (pages spent on retypes and on live children) and the region's size". `pub const NO_PARENT: u64 =
   EntryName::<Region>::NEVER_MINTED.to_bits()` with the dependency's guarantee cited. "how many
   ancestors above the refused region". And a crate-level doc that gives the model in one screen. It
   covers page units, the bump watermark, a pin while an object lives in a page, a claim as the only
   right to reclaim, a child's return to its parent and never to the allocator, and the subtree
   ceilings. The `RegionTable` example follows.
   Door: two-way. Effort: M.

8. The manifest's metadata and its dependency line.
   Finding: no `description`, `repository`, `documentation`, `readme`, `keywords` or `categories`.
   `cargo package` would warn on the first three (inferred, not run). The dependency on
   `generational_table` is by `git` and `rev`, at the commit the `v0.1.0-pre.1` tag names (verified
   with `git ls-remote`). Crates.io refuses a `git` dependency at publish, so the crate cannot
   publish until `generational_table` does, and the own-crate dependency rule already covers that.
   Evidence: `Cargo.toml` lines 1 to 14.
   Change: `description`, `documentation = "https://docs.rs/memory_regions"`, `readme =
   "README.md"`, `keywords = ["no-std", "allocator", "capability", "kernel", "memory"]`, `categories
   = ["no-std", "no-std::no-alloc", "os", "memory-management"]`. The move sets `repository`. At
   publish, `generational_table = "0.1"` and the pull request on the blocked side names the release
   it waits for.
   Door: two-way now. The dependency is one-way at publish and is already ruled. Effort: XS.

9. Public names for the naming pass.
   Finding: every public name but the crate's is provisional, and the move deletes the blocks that
   say so. Five read wrong to a stranger. `DestroyOutcome::FreeToGlobal` says "global", which a
   reader takes as scope, where it means the frame allocator. `ReturnToParent { unbump }` is not a
   word (the abbreviations report flags it) and names the mechanism rather than the quantity.
   `destroy_outcome`'s `is_lifo_top` carries an acronym the prose gate refuses. `Counted` is a
   participle naming a type. `usage` and `bounds` both return `(u64, u64)` with different meanings
   (`(watermark, pages)` and `(base_page, pages)`), so a swapped call type-checks.
   Evidence: `src/destroy.rs` lines 9 to 20 and 26 to 31; `src/table.rs` lines 115, 653 and 704.
   Change: candidates for the architect, not renames: `FreeToAllocator`; `ReturnToParent {
   pages_given_back }`; `is_last_carved` (or `is_at_watermark`); `CountedKind`; `Usage {
   watermark, pages }` and `Span { base_page, pages }` as two small `Copy` structs. The private
   `CHILD_SHARE_OF_ROOT` is a divisor named as a share; `ROOT_CEILING_DIVISOR_FOR_CHILDREN`.
   Door: one-way after publish for every public item, which is why the pass comes before it.
   Effort: S.
