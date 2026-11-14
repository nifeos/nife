# P1: before the tag

An appendix to [the milestone](../1088-elf-outside-review.md).

## P1: before the first tag

#### `Segment::page_range` is proved panic-free for one page size and is public for any

Finding: `src/segment.rs` lines 60 to 69 computes `page_size - 1` and `div_ceil(page_size)`. With `page_size`
0 the first underflows and the second divides by zero, and with a non-power-of-two both return a
range that is not page-aligned. The harness `page_range_is_panic_free_and_ordered`
(`src/verification.rs` lines 55 to 65) fixes 4096, and the doc of the harness says "for any segment", which
is true of the segment and not of the page size. The crate's claim, stated in the module doc of
`verification`, is that the public helper is safe on its own without `parse`'s guarantee. Both
callers today pass a constant (`FRAME_SIZE`, 4096 and 16384 in the fuzz target), so no caller is
wrong; the claim is. Inferred from reading; no test shows it.

Change: make the precondition a type. A `PageBytes` (name provisional) that `const fn new(u64) ->
Option<Self>` accepts only a nonzero power of two, taken by `page_range` and by item 6's parse
parameter, removes the panic without a check at every call. The harness then quantifies over
`1 << (kani::any::<u8>() % 64)`. The alternative, a documented precondition, keeps a `pub` function
that can panic. Door: one-way at release (the signature). Effort: S.

#### The overlap check hard-codes 4 KiB pages while the loaders map at their own frame size

Finding: `validate` (`src/parse.rs` lines 177 to 178) calls `page_range(4096)` for both segments. The
kernel's loader maps with `FRAME_SIZE` (`kernel/src/user/loader.rs` lines 66 and 227), and the fuzz target's
comment says 16 KiB pages are on the aarch64 roadmap. On a 16 KiB kernel, two segments in distinct
4 KiB pages of one 16 KiB page pass `parse`, and `map_new` then meets an already-mapped page after
part of the program is in the new address space. That is the failure "validate once, up front"
exists to prevent (`src/parse.rs` lines 35 to 36). No supported kernel runs 16 KiB pages today, so no caller
is wrong yet.

Change: the page size becomes an input of the parse (`Elf::parse_with(bytes, PageBytes, ...)` or a
small `Rules` value with the page size and D1's machine; names provisional), with `Elf::parse`
keeping today's defaults. `SegmentsOverlap`'s doc then says "on a page of the size given". Door:
one-way at release (API). Effort: S, M with a test that forges two segments one 4 KiB page apart and
parses them at 16 KiB.

#### Four `as usize` casts truncate on a target narrower than 64 bits

Finding: `src/parse.rs` line 116 (`phoff`), `:202` (`p_offset`), `:205` (`filesz`) and `:246` (`memsz as
usize`) narrow a `u64` from the file with `as`. On a 32-bit target a `p_offset` of `0x1_0000_0010`
becomes `0x10` and the bounds check passes on the wrong bytes. `memsz as usize` turns
`SegmentTruncated` into a comparison of low halves. No read goes out of bounds, because the code checks every slice after the cast, so this is wrong data, not memory unsafety. The note reader already does
it right: `NoteSegment::range` (`src/note.rs` lines 53 to 54) and `from_head` (`:90`) use `usize::try_from`.
The three bare-metal targets the crate names are 64-bit, so no current user feels it. Inferred
from reading; the review built no 32-bit target.

Change: `usize::try_from(...).map_err(|_| Error::...)` at the four sites, `SegmentOutOfBounds` for
the offsets and sizes and `BadProgramHeaders` for `phoff`, and `check_segment_bounds` takes the
converted values as it does now so the three harnesses stay as they are. Door: two-way. Effort: XS.

#### The error types implement neither `Display` nor `core::error::Error`, and `Elf` derives nothing

Finding: `Error` (`src/error.rs` line 44) and `NoteError` (`src/note.rs` line 23) derive `Debug`, `Clone`,
`Copy`, `PartialEq`, `Eq` and nothing else. Each caller writes its own wording:
`uefi_loader/src/image.rs` describes itself as "a refusal-wording table over `elf::Elf`", and
`kernel/src/user/loader.rs` line 47 formats `NotLoadable(elf::Error)` by hand. The in-tree precedent is
`crates/globally_unique_identifier_partition_table/src/error.rs` line 114, which implements `Display`.
`Elf` (`src/parse.rs` line 73) derives nothing, so a caller cannot `Debug`-print, copy or compare one,
though it is four integers and a slice.

Change: `impl core::fmt::Display` with one sentence per variant (the doc comments already hold them)
and `impl core::error::Error` for both enums; `#[derive(Debug, Clone, Copy)]` on `Elf`. Door:
two-way, additive. Effort: S.

#### The public vocabulary against the naming rules

Finding: `design/naming.md` says `va` and `pa` spell out as `virtual_address` and `physical_address`
"in every location", a length carries its unit, and a specification's term of art keeps its spelling
only with the expansion and a `#[doc(alias)]`. The public surface has: `Segment::vaddr`,
`Segment::paddr`, `Segment::memsz` (`src/segment.rs` lines 21, 32 and 35). `PF_R`, `PF_W`, `PF_X` (`:9-15`)
with no alias and a doc that says only "`p_flags`". `NoteSegment::p_align` and `NoteSegment::size`
(`src/note.rs` lines 44 to 46). The parameter `page_size` (`src/segment.rs` line 60). The private names
`MAXIMUM_PHNUM`, `EHDR_SIZE`, `PHDR_SIZE`, `NHDR_SIZE` (`src/parse.rs` lines 24 to 31, `src/note.rs` line 20)
break the unit and count rules too, with no outside reader. `vaddr` has 7 uses in callers, `paddr`
5, `memsz` 3, the flag constants 48. The rename is decision D3's; this item is the sweep once ruled.

Change: one rename lane applies the names D3 decides, provisionally, across the crate and its
eleven dependents. The spec spelling goes in each doc comment's first sentence and in
`#[doc(alias)]`. No alias carries this tree's earlier spelling (the rule before a first release).
Door: two-way before the first tag. Effort: M.

#### A README a stranger can follow, and an example that is not a twenty-line forgery

Finding: no README exists (item 3 makes one). The two doctests (`src/error.rs` lines 11 to 30,
`src/parse.rs` lines 42 to 61) each hand-build a 120-byte image inline. The two test modules carry two
more hand-written ELF writers (`src/tests.rs` lines 8 to 100 `Builder`, `src/note.rs` lines 371 to 431 `image`). A
stranger learns the crate from a byte-offset table four times over, and there is no `examples/`.

Change: one `#[cfg(test)]` forge shared by both test modules. An
`examples/parse_static_executable.rs` that reads a file named on the command line and prints the
entry, each segment's range and permissions and the manifest note if asked. A README with "Building
and proving" (build, test, `cargo kani`, apply one patch, run its harness, `git checkout -- .`),
"What it refuses, and why" (item 1), and "What it does not check" (item 13). Then a stranger run
recorded in `docs/stranger-test.md`. Door: two-way. Effort: S.

#### The hold of the proofs is narrower than the module doc says

Finding: `src/lib.rs` lines 19 to 25 says the proofs "prove the per-segment bounds check panic-free and in
bounds", which is exact, and the wall note says the rest is "narrow and named". Reading what is
named: `segment_at`'s index `self.phoff + i * self.phentsize` (`src/parse.rs` line 194) and the slice
`off..off + PHDR_SIZE` (`:195`) rest on the table bound from `parse` (`:130-138`) with no harness.
`NoteSegments::next`'s `self.table[self.next * self.phentsize..][..PHDR_SIZE]` (`src/note.rs` line 117)
rests on `from_head`'s the same way, and `note_segments_from_a_head_never_panic` covers that one at a 176-byte head. `a_passing_check_has_no_address_overflow` covers the entry check `self.entry < seg.vaddr + seg.memsz` (`:169`) through the type, as
the harness doc says. So one piece of arithmetic has a proof-shaped hole: the program-header index.

Change: factor `program_header_offset(phoff, i, phentsize, table_end) -> Option<usize>` (name
provisional) as a loopless function and prove that a passing value has `off + PHDR_SIZE <=
table_end`, with its falsification patch. `segment_at` and `NoteSegments::next` both call it. This
is the same move the wall note made for `check_segment_bounds`. Door: two-way. Effort: S.
