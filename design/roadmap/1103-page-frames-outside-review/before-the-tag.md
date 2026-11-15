# P1: before the tag

An appendix to [the milestone](../1103-page-frames-outside-review.md).

## P1: before the first release

4. `mark_free` rounds outward, and its documentation says the opposite of what the code does.
   Finding: `mark_free` and `mark_used` both call `page_frame_range`, which rounds the start down
   and the end up (`src/lib.rs` lines 313 to 325). The documentation of `mark_free` (`src/lib.rs` lines 179 to 184)
   says this rounding "is the opposite of what `mark_used` must do". It is identical. For a reserved
   region outward is the safe direction and the note argues it well. For usable RAM the safe
   direction is inward: a frame only partly inside a usable region is partly something else. The
   x86_64 front end hands e820 usable regions to `mark_free` clipped at the low megabyte and
   otherwise as the firmware reported them (`kernel/src/arch/x86_64/machine.rs` lines 766 to 777), so the
   direction is live on one architecture. Every listed system in the prior-art table rounds usable
   memory inward. Verified by a throwaway test: a region beginning 16 bytes into frame 2 and
   spanning three frames frees frames 2, 3, 4 and 5, two of which it only touches.
   Evidence: `src/lib.rs` lines 179 to 193, `313-325`. `kernel/src/arch/x86_64/machine.rs` lines 766 to 777. The
   throwaway test described above.
   Change: `mark_free` takes a second private range function that rounds inward (start up with
   `next_multiple_of`, end down, empty when they cross). It keeps the clamp to `[base, base +
   total)`. Its documentation states the asymmetry as it then stands: usable memory rounds in,
   reservations round out, and a region that touches no whole frame frees nothing. A host test pins
   the shape above (frames 3 and 4 free, 2 and 5 still used). Item 6 proves both range functions.
   The kernel's regions are frame-aligned on every board the tree boots today, so the kernel sees no
   change. The test boots confirm it.
   Door: two-way now. A published behavior after a release, which is why this lands first.
   Effort: S.

5. The crate says it has no `unsafe` and does not make the compiler hold it to that.
   Finding: the crate documentation (`src/lib.rs` line 3) claims pure logic with no `unsafe`. There is no
   `#![forbid(unsafe_code)]`. The template's `unsafe_scan` passes today, and the weekly fuzz opt-in
   rests on the same claim. `#![cfg_attr(not(test), no_std)]` (`src/lib.rs` line 13) guards a `cfg(test)`
   module that does not exist. The integration tests and the documentation tests are separate crates
   and link `std` on their own.
   Evidence: `src/lib.rs` line 3, 13.
   Change: `#![no_std]` and `#![forbid(unsafe_code)]` at the top of `lib.rs`. The README's one line
   says "no `unsafe`" because the compiler now refuses it.
   Door: two-way. Effort: XS.

6. The proofs cover allocation, indexing and sizing, not the rounding or the accounting.
   Finding: the five harnesses prove that two `alloc`s differ, that an allocated frame lies aligned
   and in range, that `index_of` inverts the address, that `containing` rounds down, and that the
   bitmap is big enough. Nothing proves `page_frame_range`, the function the note calls the whole
   ballgame. Host tests alone pin it. Nothing proves the `used` counter: every allocator harness
   builds the struct with `used: 0` over an arbitrary bitmap and never reads `used`, so
   `Stats::free` and the counts the kernel publishes rest on tests alone. Nothing proves
   `alloc_contiguous` (that it returns `count` adjacent frames that all started free and all end
   used) or that `free(alloc())` restores the bit. The reach record agrees: 50 viable mutants in
   reach, 38 killed by a proof.
   Evidence: `src/lib.rs` lines 349 to 445. The `page_frames` row of the Kani reach record in `notes/`.
   Change: three harnesses in the same module, each with a falsification patch. One takes a symbolic
   `start` and `size` over the full 64-bit space against a fixed base and eight frames. It asserts
   that both range functions return exactly the indices of frames the range touches (outward) or
   contains (inward), clamped, and empty for `size == 0`. The natural patch turns `div_ceil` into
   `/`. One takes a symbolic 8-frame bitmap with `used` set to its popcount, applies one symbolic
   operation (`mark_free`, `mark_used`, `alloc`, `alloc_contiguous`, or `free` of an allocated
   frame) and asserts `used` equals the popcount afterwards. The patch turns `used += count` into
   `used += 1`. One asserts that a successful `alloc_contiguous(n)` returns a frame whose `n`
   successors were free before and which the allocator marks used after. The patch marks `n - 1`.
   Each stays within the 8-frame bound and the existing `unwind(9)`.
   Door: two-way. Effort: M.

7. The acronyms gate fails on three words.
   Finding: `prose_acronyms.py` reads Rust doc comments at a ceiling of zero and reports `DMA`
   (`src/lib.rs` line 2), `MMU` (`15`) and `MMIO` (`86`) unexpanded at first use. The pinned `v7`
   template does not run `passive_voice`, `spelled_out`, `unit_symbols` or `function_length`, and a
   later pin will. The crate passes those four today except the passive count, which does not read
   doc comments.
   Evidence: the helper's output. `crate-workflows/.github/workflows/check.yml` lines 274 to 286.
   Change: "direct memory access (DMA)", "the memory management unit (MMU)" and "memory-mapped input
   and output (MMIO)" at first use, in nife before the move.
   Door: two-way. Effort: XS.

8. The names the release review meets for the first time.
   Finding: no public item carries a `Name:` block, so none has a ratification and every one meets
   [design/naming.md](../../naming.md) for the first time at this review. Two fail a rule and are
   public: the constant `FRAME_SIZE` is a length without its unit, where the rule says a length
   carries its unit and the tree's precedent for the same quantity is `PAGE_BYTES` (Decision 1).
   `addr` and `from_addr` carry an abbreviation a reader decodes, where the one sibling with the
   same accessor spells `address`; Decision 2 decides `physical_address` and
   `from_physical_address`, provisionally. The rest are two-way or private: the parameters
   `size: u64` of `mark_free` and `mark_used` and `span: u64` of `page_frames_in` hold byte counts
   and carry no unit (`bytes`, `span_bytes`). `Stats::total` and `Stats::used` count frames and name
   the count (`frames`, `used_frames`), and `Stats::free()` shares its word with the verb
   `PageFrameAllocator::free`, which the vocabulary rule asks to qualify (`free_frames`). The
   harness locals `bm`, `fa` and `fb`. The crate name, `PageFrame`, `PageFrameAllocator`, `Stats`,
   `containing`, `alloc`, `alloc_contiguous`, `free`, `is_used`, `stats`, `largest_free_run`,
   `bitmap_bytes` and `mark_free` / `mark_used` pass every rule this review can apply.
   Evidence: `src/lib.rs` line 17, 48, 62, `69-74`, `78`, `135`, `185`, `201`, `390`, `409-419`.
   `crates/paging/src/tlb_flush.rs` line 41. `design/naming/crates.md` line 65.
   Change: one rename lane after the architect rules Decisions 1 and 2. The lane performs the two
   public renames across the tree (about 60 `FRAME_SIZE` sites in `kernel/` and `system_tests/`) and
   the two-way ones in the crate, all shipped provisional. The rule that a crate carries no alias
   for this tree's own earlier spelling before its first release refuses `#[doc(alias =
   "FRAME_SIZE")]`.
   Door: one-way after a release for the public names. Two-way for the rest. Effort: S after the
   rulings.

9. `page_frames_in` truncates where its name and documentation promise coverage.
   Finding: the documentation reads "How many frames span `[base, base + span)`" and the body is
   `span / FRAME_SIZE` (`src/lib.rs` lines 134 to 137). For a span that is not a whole number of frames the
   answer is one short, and the kernel rounds the top up by hand before calling it with a comment
   that names the trap (`kernel/src/memory.rs` lines 361 to 362). Verified by a throwaway test:
   `page_frames_in(FRAME_SIZE + 1)` is 1.
   Evidence: `src/lib.rs` lines 134 to 137. `kernel/src/memory.rs` lines 361 to 362. The throwaway test.
   Change: the documentation says "whole frames in `span`", and a second constant function
   `page_frames_covering(span)` returns `span.div_ceil(FRAME_SIZE)` for the span a bitmap must
   reach. The kernel calls it and drops its own rounding. Both names ship provisional.
   Door: two-way now. A public name after a release. Effort: XS.

10. The in-tree falsification patches have drifted, and the move regenerates one of them badly.
    Finding: all five patches apply to the in-tree source only at an offset (25 to 52 lines), so
    their hunk headers no longer describe the file. nife's own replay passes with a note. The dry
    run regenerates them over the stripped source, and the regenerated
    `verification.index_of_inverts_frame_addressing.patch` fails the template's `falsify.py` with
    "corrupt patch at line 23": its hunk header promises seven old lines and the file carries six.
    The missing line is the blank line after `index_of`'s closing brace (`src/lib.rs` line 309), which
    the regeneration drops when it is the hunk's last context line. The other four end their hunks
    on non-blank lines and survive. The dry run therefore cannot finish for this crate until the
    tooling changes.
    Evidence: `patch -p1 --dry-run -F0 < falsifications/*.patch` in the tree. The staged
    `falsifications/verification.index_of_inverts_frame_addressing.patch` (hunk `@@ -298,7 +298,7
    @@` with six lines). `helpers/crate_release.py`, the regeneration (the dropping line was not
    located during this review).
    Change: in nife, the regeneration keeps an empty trailing context line (or emits the hunk with
    one fewer context line and a matching header). Its self-test gains a case whose hunk ends on a
    blank line. Separately, nife regenerates the five in-tree patches so their headers match the
    file, as item 6 adds three more.
    Door: two-way. Effort: S.
