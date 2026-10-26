# The parser and device-input harnesses: ELF, nifefs and PCI

An appendix to [notes/verification.md](../verification.md). These three crates parse input the
kernel did not write: a binary, a boot archive, and a PCI function's answers. Why whole-parse
totality was out of reach is in [elf-parser-wall.md](elf-parser-wall.md).

## `crates/elf`

Four in `crates/elf/src/lib.rs`:

| Harness | Property |
|---|---|
| `check_segment_bounds_never_panics` | the per-segment bounds/overflow arithmetic never panics, for any file length and any hostile field values |
| `a_passing_check_yields_an_in_bounds_range` | if the check passes, `p_offset <= end <= file_len`, so the segment's data slice is in bounds (what the whole-parse totality proof was really reaching for) |
| `a_passing_check_has_no_address_overflow` | if the check passes, `vaddr + memsz` did not wrap, so `validate`'s later unchecked add cannot panic |
| `page_range_is_panic_free_and_ordered` | for any `vaddr`/`memsz`, the saturating page arithmetic neither panics nor returns an inverted range (a `pub` helper that must be safe on its own) |

## `crates/nifefs`

Two in `crates/nifefs/src/lib.rs`, the initrd parser the kernel runs on boot input. It is the archive
format the initrd and disk images use. It was kept over tar by the reuse record in
notes/prior-art.md, and it is proved because the kernel-side parse is TCB code.

| Harness | Property |
|---|---|
| `the_validation_implies_reads_slice_is_in_bounds` | for every entry value and image length, parse's acceptance check makes `read`'s slice arithmetic safe: no panic, bytes inside the image |
| `a_short_image_is_refused_not_indexed` | any image under one block is `Truncated` before a byte past the length check is touched |

Whole-parse totality hit the same wall as ELF and `device_tree_blob`: a one-block symbolic image put
CBMC past 20 CPU-minutes. It was decomposed the same way. The module comment records what is
deliberately unproved, and why it is sound anyway.

## `crates/pci`

Four in `crates/pci/src/lib.rs`, the config-space decode the kernel runs on device input. A hostile
or broken PCI function can answer the closures with anything.

| Harness | Property |
|---|---|
| `ecam_offset_stays_inside_the_window` | any BDF's config page lies inside the 256-bus ECAM window, so the kernel's volatile accessors cannot escape a correctly-sized mapping |
| `intx_interrupt_number_is_total_and_bounded` | the swizzle is total (the pin-0 underflow that panicked debug builds is gone, hardened with saturating arithmetic) and lands within `base..=base+3` |
| `read_bars_is_total_for_any_device` | the BAR size probe never panics on garbage device answers (`!mask + 1` cannot overflow: the type bits are masked first) |
| `the_capability_walk_terminates_on_any_device` | a capability list forming ANY graph, cycles included, is walked at most 64 hops; the bounded-walk discipline proved rather than argued |
