# P2: later

An appendix to [the milestone](../1108-socket-contract-outside-review.md).

7. The one piece of logic beyond the grants has no proof. PARTIAL: Kani `a_position_is_a_port_in_range_and_the_clock_never_runs_backwards` with its patch; the fallback is documented. LEFT: the loop harness over `held` (16384 iterations) and the no-reuse arithmetic harness; `Option<u16>` is a signature change.
   Finding: the three harnesses prove the grant packing and checks, which is the authority. The
   frame layout, the badge predicate and the lease round trip are tests of fixed values, which is
   enough for constants. `ephemeral::Ports::next` (lines 331 to 341) is a loop with a clock, a
   modulus and a fallback, proven by nothing. Its central claim is that a stack started a clock tick
   after another hands out none of its ports. A sampled test covers that claim over five start times
   and 1,997 gaps (lines 388 to 401). The fallback at line 340 returns a port the `held` predicate refused
   when every port is in use, silently.
   Evidence: the lines above; `README.md` "What is proven".
   Change: two bounded harnesses. One: for any cursor position, any `milliseconds` and any
   `held` answer, the port returned lies in `LO..=HI` and the cursor never moves backwards. Two:
   for any `last` and any `gap` of at least `1000 / PER_SECOND` rounded up, `clock(last + gap)`
   exceeds the position a cursor reaches after `n` opens `g` apart when `g * PER_SECOND >= 1000`,
   which is the no-reuse claim as arithmetic rather than samples. Each with a falsification patch
   (the existing ephemeral patch falsifies the test and can falsify the harness too). Document
   the fallback, or make `next` return `Option<u16>` and let the stack answer `REP_ERR`; the
   second is the honest shape and costs one `match` in `socket_table.rs`.
   Door: two-way (an unreleased signature). Effort: M.

8. Offsets, lengths and byte orders mix types and go unstated. PARTIAL: `FRAME_BYTES` added and the byte order stated. LEFT: one type for offsets and lengths (touches five component sites).
   Finding: `OFF_*` are `u64` and `DATA_MAXIMUM` is `usize`, so every caller casts
   (`DATA_MAXIMUM as u64` at five sites in `components/`, `OFF_PAYLOAD as usize` in the crate's
   own line 471). `DATA_MAXIMUM = 4096 - OFF_PAYLOAD` hardcodes the page size with no constant for
   it; `user_mode_runtime` brings its own `PAGE`. The text gives the byte order for `dst_port` alone
   ("little-endian", line 456). It gives none for `len`. The std layer comments that it matches
   (`nife.rs` line 219). The lease word is big-endian in the low 32 bits, so one contract carries
   two byte orders without saying it has two.
   Evidence: `lib.rs` lines 454 to 471 and 534 to 537; `socket_frame.rs` line 35.
   Change: one type for offsets and lengths (`usize`, as `core::ptr::add` and slices take). Add a
   `FRAME_BYTES` constant (name provisional) of 4096 that `DATA_MAXIMUM` derives from and the client
   runtime can import. Add one sentence to the layout note: every multi-byte frame field is
   little-endian, and a `lease` word carries the address big-endian in its low 32 bits because that
   is the way an address reads.
   Door: two-way before the release; the types become one-way after. Effort: S.

9. Names where this review reads a later rule differently from the package. LEFT: names, the architect's pass.
   Finding: the package proposes keeping `Ports::next` and `polled_wait::next`. The rule that a
   method takes std's spelling asks for std's meaning too; `Iterator::next` takes no argument and
   returns `Option`, while `Ports::next(milliseconds, held)` takes two and returns a `u16`, and
   `polled_wait::next(now, last_frame, tick, ...)` is a pure decision. `fixture::IN_MSG` and
   `OUT_MSG` abbreviate `message`; the package proposes nothing for `fixture` because its decision
   2 moves the module. `udp`, `tcp` and `ip` appear in public names and are in neither the
   template's `[spelled_out]` nor its `[accepted]` list, so the gate neither refuses nor records
   them.
   Evidence: `lib.rs` lines 239, 331, 488, 490; `vendor/crate-workflows/helpers/naming_words.toml`
   `[accepted]`; the package's names table.
   Change: proposals for the architect's naming pass, all provisional. Rename to `Ports::allocate`
   (it acts and reports, like `claim`), and `polled_wait::decide` or `wait_decision`. Use
   `IN_MESSAGE` and `OUT_MESSAGE` if `fixture` stays long enough to matter. Add `udp`, `tcp` and
   `ip` to the template's `[accepted]` with their reasons ("nobody says User Datagram Protocol in a
   name"), so the rule has a record rather than passing by silence.
   Door: two-way. Effort: S, in the package's rename lane.

10. Inline "name provisional" prose survives the move. LEFT: the prose goes with the `Name:` blocks at the move.
    Finding: the move strips `Name:` blocks. The crate also carries the words inside ordinary
    sentences: "Names provisional." (line 65), "Name provisional." (lines 74, 84), "Names
    provisional." (line 440), "The module's name is provisional." (lines 213, 292), and
    "provisional, as is the name" (line 516). A stranger reads them in rustdoc with no idea whose
    ruling is pending. Line 22 names the decider inside the crate docs.
    Evidence: the lines above.
    Change: move each into the item's `Name:` block, which the move strips, or delete it at the
    move. Line 22 becomes a `Name:` block in the recorded form.
    Door: two-way. Effort: S.

11. The documentation refers to things the crate does not define. DONE (a Vocabulary section in the crate documentation).
    Finding: `PageFrame` (line 451) is a kernel type; `SEND_CAPABILITY`, `CALL`, `WRITE | GRANT`,
    `BADGE`, `UNMAP` (lines 46, 442 to 448) are kernel operations and rights; "the kernel-stamped"
    badge assumes the reader knows the kernel stamps badges. The package lists `PageFrame`. A
    reader of the crate alone has no link to follow.
    Evidence: the lines above.
    Change: one "Vocabulary" paragraph in the crate documentation. It says, in a sentence each, what
    a capability, a badge, a `CALL` and the shared frame are in the system this contract assumes. It
    also says which crate (`abi`) names the operations, without a path into nife.
    Door: two-way. Effort: S.
