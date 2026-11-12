# P2: later

An appendix to [the milestone](../1101-memory-regions-outside-review.md).

## P2: can follow later

10. A root ceiling under sixteen gives every child zero.
    Finding: `Ceilings::for_child_of` divides each of the root's counts by 16 with integer division.
    A root with a thread ceiling of 15 cannot delegate one thread to any child, verified: the child
    reads `(0, 0)` and the table refuses its carve. The kernel's progenitor ceilings are in the
    hundreds, so nothing in the tree meets it; a stranger's first small example does. Nothing in the
    documentation says so.
    Evidence: `src/table.rs` lines 134 to 137 and 185 to 194.
    Change: say it in `insert_root_with_ceilings` and `for_child_of`, and see Decision 4 for the
    share itself. Door: two-way (the policy is one-way after publish; Decision 4). Effort: XS.

11. `spent` walks the whole table and, for every entry, its parent chain.
    Finding: for `Frames` and each `Object` kind, `spent` filters all `N` slots through
    `descends_from`, which walks up to `N` ancestors each: O(N·depth) under the caller's lock, with
    `N` 1,024 in the kernel. `return_to_parent` scans the table once per return. Both are correct
    and the kernel calls `spent` only for `USAGE`. The cost is not stated.
    Evidence: `src/table.rs` lines 665 to 699 and 782 to 788.
    Change: one sentence of cost in each doc comment. A per-parent child count of pages (already
    `spent.children`) covers `Children` without a scan. Door: two-way. Effort: XS.

12. A maintainer keeps `ObjectKind::COUNT` by hand in four places.
    Finding: the enum has seven variants, `COUNT` says 7, the proofs' `KINDS` lists eight `Counted`
    values and `any_object_kind` takes `% 7`. The kernel's `usage_record` matches four of them. An
    eighth kind is five edits and no compiler error for a missed one.
    Evidence: `src/table.rs` lines 86 to 108. `src/proofs.rs` lines 155 to 178. `kernel/src/memory_region.rs`
    `usage_record`.
    Change: `pub const ALL: [ObjectKind; 7]` on the enum, `COUNT = ALL.len()`, `KINDS` built from
    it, `any_object_kind` indexing it. Door: two-way. Effort: XS.

13. The crate has no `unsafe` and does not say so.
    Finding: no `unsafe` anywhere in the crate; the root carries no `#![forbid(unsafe_code)]`. The
    template's fuzz opt-in and its `unsafe_scan.py` read the attribute, and a reader of a kernel
    crate looks for it first.
    Evidence: `src/lib.rs` line 14.
    Change: `#![forbid(unsafe_code)]` under the `no_std` line. Door: two-way. Effort: XS.

14. The proofs assume what the table must keep, and no harness checks that it does.
    Finding: `coalescing_never_reclaims_a_held_page` assumes `retyped_to <= watermark` and `live_to
    <= watermark`, and the harnesses over `split_new_watermark` and `retype_new_watermark` assume
    `watermark <= pages`. Only the table's discipline keeps those true across `split`,
    `claim_for_destroy` and `return_to_parent`, and no table-level harness states them.
    `destroy_never_frees_a_child_to_the_allocator` restates its function's body. The six subtree
    harnesses cover `charge`, `release`, `split` and the claim at depth three in a table of four,
    and none drives `return_to_parent` from an arbitrary state. `falsification-times.tsv` records
    the four arithmetic harnesses and not the six subtree ones.
    Evidence: `src/proofs.rs` lines 7 to 137 and 553 to 580.
    `notes/project-metrics/falsification-times.tsv` lines 271 to 274.
    Change: one harness from `Run::any(3, _, true)` that claims and returns the grandchild and
    asserts, for every live region, `retyped_to <= watermark <= pages` and that no live child's end
    exceeds its parent's watermark, with its falsification patch. Record the six times. Door:
    two-way. Effort: M.

15. Three pure functions and two observers have no direct host test.
    Finding: `coalesced_watermark`, `retype_new_watermark` and `retype_pages` have tests only
    through the table. `ceiling_reached` is never asked about a dead name or with `levels_up == 0`.
    `Ceilings::with` and `get` live in one doctest. The census records none of their mutants as
    missed, so this is legibility rather than a hole.
    Evidence: `src/tests.rs`. `src/table.rs` lines 1360 to 1381.
    Change: one table-driven test per pure function in `src/tests.rs`, and two asserts in
    `the_nearest_ancestor_at_its_ceiling_is_the_one_named`. Door: two-way. Effort: XS.

16. A dropped claim leaks a region's pages quietly.
    Finding: `DestroyClaim` is `#[must_use]` with a message, which catches an unused result and
    nothing else: a claim bound to a name and dropped on an early return leaks the run, and the
    crate cannot see it. The kernel's `destroy` spends every claim it mints. A second caller may
    not.
    Evidence: `src/table.rs` lines 262 to 269.
    Change: a `Drop` that panics under `debug_assertions` unless the caller spent the claim, with
    `return_to_parent` taking the claim by value and a `spend_root(self)` for the allocator path. Or
    leave it and say so in the type's doc. Door: two-way now. The signature is one-way after
    publish. Effort: S.
