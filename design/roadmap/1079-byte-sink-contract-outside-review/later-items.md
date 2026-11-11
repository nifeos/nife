# P2: later

An appendix to [the milestone](../1079-byte-sink-contract-outside-review.md).

## P2: worth doing, not blocking

11. `len` truncates on a target whose `usize` is narrower than 32 bits.
    Finding: `(w0 & 0xffff_ffff) as usize` drops the high half of the count on a 16-bit target, so
    a first word claiming `0x1_0010` bytes reads as 16 and passes `unpack`'s bound. No target the crate names has the problem (all three are 64-bit). `unpack` would still copy only sixteen bytes
    into a sixteen-byte buffer, so the consequence is a wrong count and not a wrong read. The same
    class as milestone 1048 (what an outside maintainer would fix before 0.1.0)'s item 2.
    Evidence: `src/lib.rs` lines 83 to 85 and 235 to 238.
    Change: compare the count in `u64` inside `unpack` before casting, or `usize::try_from` with a
    failure reading as `Malformed`.
    Door: two-way. Effort: XS.

12. `req` silently discards an opcode above 255 or a count above `u32::MAX`.
    Finding: `operation << 56` keeps eight bits and `len & 0xffff_ffff` keeps thirty-two, so
    `req(256, 0)` is the zero word, a valid empty bytes message. No caller passes such values; the
    function is `const` and `pub`, and an outside writer may.
    Evidence: `src/lib.rs` lines 72 to 75.
    Change: typed parameters, `operation: u8` and `len: u32`, so the type system refuses what the
    masks hide; the two masks become casts. Every call site in the tree passes literals or values
    that fit.
    Door: two-way (signature, before any release). Effort: XS.

13. `pack` returns a bare four-tuple.
    Finding: `(u64, u64, u64, usize)` at 31 call sites, each destructured as `(w0, w1, w2, n)`. A
    reader of a call site learns the meaning from the destructuring pattern and not from the type.
    Evidence: `src/lib.rs` line 132; every `pack` caller.
    Change: a `Copy` struct for the three words (its name is the architect's; `Words` with fields
    `first`, `second`, `third` is one shape) returned beside `taken`, and `unpack` taking the same
    struct. A convenience and a door the architect may decline; the tuple is not wrong.
    Door: one-way at release (a public type) if taken. Leaving it loses nothing. Effort: M
    (call sites).

14. The two equivalent mutants need exemptions in the new repository.
    Finding: `pack`'s `<` to `<=` and `req`'s `|` to `^` survive, and the census classifies them equivalent. The census also lists `with_severity`'s `>` to `>=` and `|` to `^`, equivalent for the same reasons: the mask applies before the or, and 7 clamps to 7. Without entries in
    `.github/mutants-exempts` the weekly `mutation` verdict is red.
    Evidence: `notes/project-metrics/mutation-survivor-classes.csv` lines 13 to 16;
    `notes/mutation-testing/baseline-survivors-grant-plan-to-swish.md` lines 224 to 231.
    Change: the exemptions with one reason each, carried over from the survivor note.
    Door: two-way. Effort: XS.

15. `with_severity` and `severity` apply to any first word, including an end-of-stream word.
    Finding: both docs say they act on "a bytes message's" first word, and neither checks the opcode. `with_severity(eof(), 3)` is a well-formed `Eof` with severity bits set, which no
    consumer reads. Harmless today; worth one sentence so an implementer does not look for meaning
    there.
    Evidence: `src/lib.rs` lines 171 to 188.
    Change: one sentence in each doc comment: a consumer reads the bits only on a bytes message.
    Door: two-way. Effort: XS.
