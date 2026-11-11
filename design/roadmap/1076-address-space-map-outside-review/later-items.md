# P2: later

An appendix to [the milestone](../1076-address-space-map-outside-review.md).

## P2: later

13. The template's `register_names.py` misreads harness names.
    Finding: it refuses five verification functions whose names contain the word `address`
    (`every_address_below_the_ceiling_lies_in_exactly_one_band` and four `..._refuses_every_address_its_doc_forbids`)
    as addresses that must be virtual or physical. A function is not a location in memory. This is
    the template's bug, and with the names as they are the crate's `check / standalone` job is red
    on the first run.
    Evidence: the `register_names.py` run above; `helpers/register_names.py:18-24` in the template.
    Change: in the template, skip the address rule for `fn` definitions, or read `address` only as
    the last component before a qualifier. Until it lands, the harnesses could say `page` in place
    of `address`, but renaming correct names to satisfy a wrong check is not recommended.
    Door: two-way. Effort: S (template).

14. `BANDS` fixes the band count in its type.
    Finding: `pub const BANDS: [(&str, Band); 8]`. Adding or splitting a band changes the type of
    a public item, so any caller holding it by type breaks even if it only iterates. Nothing in
    nife reads `BANDS` today, so the cost is zero now and grows after the first tag.
    Evidence: `src/lib.rs:132`.
    Change: `pub const BANDS: &[(&str, Band)]`. The `const` tiling assertion and the Kani helpers
    index and iterate it unchanged. Before the first tag this is a free change; after it, a minor
    bump.
    Door: two-way now, one-way after the first tag. Effort: S.

15. A caller can build `Band` inverted, and `bytes` then underflows.
    Finding: the fields are public so callers can build one in a `const` (`process_builder` does).
    `Band { start: 5, end: 1 }.bytes()` panics in a debug build and wraps in release; `contains`
    and `holds` return `false`, which is harmless. No caller builds an inverted band today.
    Evidence: `src/lib.rs:43-54`; `crates/process_builder/src/lib.rs:51-54`.
    Change: a `pub const fn new(start, end) -> Band` that asserts `start < end` and page alignment
    at both ends, so a `const` built through it fails the build the way `pair_page` does. The fields stay public for destructuring, and the doc says `new` is the way to build one. Two-way because
    adding a constructor removes nothing.
    Door: two-way. Effort: S.

16. `#![forbid(unsafe_code)]` is absent.
    Finding: the crate has no `unsafe`, and the template's scan passes it. The attribute makes the
    claim a compiler error rather than a scan result, which is what a stranger checks first.
    Evidence: `src/lib.rs:20`.
    Change: add the attribute under the `no_std` line.
    Door: two-way. Effort: S.

17. nife's checks that walk `crates/` lose this crate.
    Finding: `helpers/interface_declarations_check.py` finds `pub const INTERFACE_ID` by reading
    `crates/*/src/lib.rs` against the table in milestone 956's block; after the move the crate's `lib.rs` is not in the tree and the row for id 2 has no source to check. `LOW_HALF_SPLIT_SHIFTS`
    restates `crates/paging`'s `SPLIT_SHIFT` with no test tying the two. After the move the restatement lives in another repository. Milestone 1017 names the class ("nife's tools that walk
    `crates/` also lose each crate").
    Evidence: `helpers/interface_declarations_check.py:58, 74`; `src/lib.rs:289-292`.
    Change: in nife, the declarations check reads the pinned checkout through `cargo metadata`, and
    `crates/paging` gains one test asserting its three `SPLIT_SHIFT`s against the crate's constants
    (which means `LOW_HALF_SPLIT_SHIFTS` becomes `pub`, a small addition).
    Door: two-way. Effort: S.

18. Milestone 1017's blocker table has a stale row.
    Finding: the table says `counter_frequency_contract` has a dev-dependency on
    `address_space_map` and must leave after it. No manifest or source in
    `crates/counter_frequency_contract/` names the crate now; the relation it guarded (the timebase
    page lies above the ceiling) holds as an assertion in `kernel/src/user/loader.rs:14` instead.
    Evidence: `design/roadmap/1017-the-contract-crates-leave-in-this-order.md`; a grep of
    `crates/counter_frequency_contract/`.
    Change: drop the row when 1017 is next edited.
    Door: two-way. Effort: S.
