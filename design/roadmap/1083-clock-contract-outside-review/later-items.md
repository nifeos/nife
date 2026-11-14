# P2: later

An appendix to [the milestone](../1083-clock-contract-outside-review.md).

## P2: worth doing, not worth holding the move for

10. `propose::req` truncates silently.
    Finding: `req(operation)` (`src/lib.rs` line 357) shifts by 56, so an operation of 256 or more loses
    its high bits; `operation(w0)` discards the low 56 bits of a word that nothing says must be zero
    there. Both are correct for the two opcodes that exist.
    Change: a `debug_assert!(operation < 256)` in `req`, or a sentence on each saying the opcode is
    one byte and the low bits are the request's to use; the harness in item 9 pins the round trip.
    Door: two-way. Effort: S.

11. `ClockPage::new` takes an integer where a Rust reader expects a pointer.
    Finding: `pub const unsafe fn new(va: u64)` (`src/lib.rs` line 233) casts an integer to
    `*const AtomicU64`. The crate enables `ptr_as_ptr` and `cast_ptr_alignment`, which says it cares
    about pointer hygiene, and an integer-to-pointer cast is the one form those lints cannot see.
    The three callers pass addresses they hold as `u64` (`CLOCK_VA` constants and a physical-to-
    virtual map), so a pointer parameter adds one cast at each.
    Change: with the rename to `virtual_address` (the package's proposal), decide the type too: keep
    `u64`, whose name then says what it is, or take `*const u8` and let the caller own the cast.
    Keeping `u64` is defensible for a value that arrives in a register; the doc should then say the name means an address, not a pointer.
    Door: two-way before the first release; the name is Decision 3's. Effort: S.

12. A test whose name claims more than it checks.
    Done, with Decision 1: the reader has no spin path, and the host test
    `a_writer_stopped_inside_a_publish_leaves_the_last_publish_readable` reads the page with a
    publish filled and not flipped, and checks that the reading is the last publish.
    Door: two-way.

13. `initialize` relies on a zeroed frame for the reserved words.
    Finding: `WORDS` (`src/lib.rs` line 178) says the rest of the frame "is reserved and must read as
    zero"; `initialize` writes the seven words and nothing else; `read` checks nothing past word six.
    Change: say in `initialize`'s doc that the caller hands it a zeroed frame and the reserved words
    are whatever the frame held, or zero them there. The sentence is enough.
    Door: two-way. Effort: S.

14. `offset_for` saturates to an offset of zero.
    Finding: `offset_for(wall, monotonic)` (`src/lib.rs` line 115) returns 0 when `monotonic > wall`, and
    `wall_nanoseconds(0, monotonic)` then reads as uptime since the epoch, which is the reading the
    crate exists to prevent. Unreachable through `policy`, since a plausible wall time exceeds any
    uptime a machine will have, but `publish` accepts any offset from a set-authority holder.
    Change: a sentence on `offset_for` naming the case and why the policy makes it unreachable, or an
    `Option` return. The sentence is enough for a contract crate.
    Door: two-way. Effort: S.

15. The README does not say who can use the crate.
    Finding: the README describes the contract well and never says that it is useful only together
    with a clock service that maps the page and a loader that grants it. The crate on its own
    sets no clock and reads no time. A stranger on crates.io, the audience the README is for, reads
    four paragraphs before learning that.
    Change: one sentence in the first paragraph ("This crate is the definition the service, its
    readers and a standard library's platform layer share; on its own it maps nothing and sets
    nothing") and, under "Building and testing", the sentence from item 8.
    Door: two-way. Effort: S.
