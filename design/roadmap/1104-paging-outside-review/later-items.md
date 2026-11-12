# P2: later

An appendix to [the milestone](../1104-paging-outside-review.md).

## P2

12. Error variants the mapper never raises, and a half that does not exist.
    `MapError::WritableAndExecutable` (`src/mapper.rs` lines 39 to 47) comes only from the kernel's mapping
    hold. `MapError::BadRegion` only by `domain::grant_pages`. Both live in the enum a stranger
    reads as the mapper's contract. `Vtd` and `AmdVi` set `SPLIT_SHIFT = 47` so that `is_in_half`
    admits every physical address under 128 TiB (`src/x86_64.rs` lines 259 to 264, `src/x86_64.rs` lines 380 to 381).
    A domain over a frame above that gets `WrongHalf`, an error about a concept IOVAs do not have.
    Fix: either move `WritableAndExecutable` to the kernel (it is the kernel's rule) and document
    `BadRegion` as the domain builder's, or keep both and say so in the enum's doc for the crate's
    reader. Document the IOMMU formats' `SPLIT_SHIFT` as a ceiling. Effort S. Two-way until the
    first release.

13. `Mapper::walk` misreports a walk longer than `MAXIMUM_WALK`. The loop at `src/mapper.rs` lines 461 to 487
    assigns `walk.end` in three branches and `continue`s in the fourth without assigning it, so a
    format with six or more levels would exhaust the loop and return `WalkEnd::OtherHalf` with five
    steps. No such format exists here, and the constant's comment (`src/mapper.rs` lines 491 to 493) says the
    spare level exists to catch exactly this. Fix: a `const` assertion `F::LEVELS <= MAXIMUM_WALK`
    in `walk`, or a `WalkEnd::Truncated` variant. Effort S. Two-way.

14. `PageSize::{Size4KiB, Size2MiB, Size1GiB}` against the units rule. `design/naming.md`, "Units",
    says the IEC binary prefixes are symbols and a name says `kibibytes`, `mebibytes` or
    `gibibytes`. The variants (`src/page_size.rs` lines 16 to 23) cite the `x86_64` crate as prior art, which
    is accurate (verified: `x86_64` 0.15.5 implements its `Mapper` for `Size4KiB`, `Size2MiB` and
    `Size1GiB`). The template's `unit_symbols.py` judges 1,077 names in this crate and refuses none,
    because it splits `Size4KiB` at the digit and never sees `kib` as one component, so the rule and
    its check disagree here. Decision 2 below. BUGS records the template gap.

15. What a user of the comparable crates will reach for and not find. The crate has no way to change
    the flags of a live mapping (`x86_64::Mapper::update_flags`, `pmap_protect`). It has no
    `unmap_block`, no range walk or iterator, and no reclamation of an emptied intermediate table
    (the kernel frees frames wholesale). `MapError` has no `Display` or `core::error::Error`.
    `PageTable.entries` is `pub` (`src/page_table.rs` line 18), so a caller can bypass the format.
    `Flags` derives neither `Hash` nor `Ord`. Each is a defensible non-goal for this kernel. Fix:
    say so in the README's BUGS, and add `core::error::Error` for `MapError`, which costs one
    `impl`. Effort S for the documentation. M for any method. Two-way.

16. Documentation that drifted or stops short. `TlbFlush::flush` names `tlbi vaae1is` and
`sfence.vma` and not `invlpg` (`src/tlb_flush.rs` line 22). `PageFormat::LEVELS` and `SPLIT_SHIFT`
document aarch64 and Sv39 only (`src/page_format.rs` lines 55 to 60), and `Half`'s doc the same
(`src/page_format.rs` lines 13 to 16). `src/flag_tests.rs` line 78 says "the CAP_* bit positions" for constants
now named `ACCESS_*`. `src/x86_64.rs` line 26 says the PAT reset value was "recalled rather than
re-read", which a stranger reads as unverified. Fix: one doc pass. Cite the SDM for the PAT reset
value. Effort S. Two-way.

17. `tests/mapping.rs` frees its tables by an opt-in guard. Its own BUGS (`tests/mapping.rs` lines 35 to 39)
says a test that forgets `let _tables = TableGuard;` leaks, and twenty-one call sites are not
converted to the pool shape `tests/blocks.rs` and `src/domain.rs` use, where the borrow checker
enforces it. Fix: convert to the `Pool` shape. Effort S. Two-way.

18. The Sv39 device bit is software bookkeeping, and the T-Head values have one source. `SW_DEVICE`
    (`src/sv39.rs` lines 26 to 30) records device memory in an RSW bit the hardware ignores. A machine with
    Svpbmt would want bits 62:61, which no format here sets. `Sv39Mae`'s memory-type values come
    from Linux's `pgtable-64.h` alone (`src/sv39.rs` lines 139 to 147). The text cites no vendor document.
    Fix: say both in the README's BUGS. Name the Svpbmt format as future work. Cite T-Head's
    document for the MAEE bits if one exists, or say that the search found none. Effort S. Two-way.

19. Miri warns on the tests' integer-to-pointer casts. `tests/mapping.rs` line 57,
`tests/blocks.rs` line 51, `tests/nth_allocation.rs` line 50 and `src/domain.rs` line 362 turn a `Box` into a
`u64` and back; `notes/undefined-behavior.md` line 93 records the warnings as deliberately left. The
template's weekly Miri job will print them on every run. Fix: `ptr::expose_provenance` and
`ptr::with_exposed_provenance` at those four sites, or a one-line note in the README. Not run under
Miri for this review. Effort S. Two-way.
