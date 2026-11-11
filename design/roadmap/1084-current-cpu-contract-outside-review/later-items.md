# P2: later

An appendix to [the milestone](../1084-current-cpu-contract-outside-review.md).

## P2: later

13. `#![cfg_attr(not(test), no_std)]` where `#![no_std]` would do.
    Finding: the tests use nothing from `std`; the attribute only means the library is never
    type-checked as `no_std` in a test build.
    Evidence: `src/lib.rs` line 23.
    Change: `#![no_std]`. Door: two-way. Effort: XS.

14. `CPU_WORD_OFFSET` is a `u64` and the private offsets are `usize`.
    Finding: the one public offset has the type its one caller wants (an x86 test program that adds it to
    the address); `OFF_CPU` is a `usize`. A stranger reaching for `page[CPU_WORD_OFFSET..]` casts.
    Evidence: `src/lib.rs` lines 100 to 115.
    Change: none recommended before the name review; recorded so the review sees it.
    Door: one-way (a published type). Effort: XS.

15. The kernel's safety comment at the write site says the frame view is "16 bytes wide".
    Finding: the page is 32 bytes since the tick word arrived; the store itself stays inside 16,
    so nothing is unsound, but the comment is a copy of an older layout.
    Evidence: `kernel/src/user/address_space.rs` line 200.
    Change: say `PAGE_CONTENT_BYTES`. A nife lane, outside the crate. Door: two-way. Effort: XS.

16. Two notes describe the page.
    Finding: `notes/current-cpu-page.md` (the crate's design) and
    `notes/scheduler/the-current-cpu-page.md` (the kernel tests). The system test cites the second;
    the crate cites the first. Neither links the other.
    Change: a cross-link, in nife. Door: two-way. Effort: XS.
