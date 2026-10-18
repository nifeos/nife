# The baseline survivors, crate by crate: `grant_plan` to `swish`

How each survivor of the 2026-08-03 baseline was closed, from `grant_plan` to `swish`, with the test
that killed it or the argument that it is equivalent. The counts are that run's; the current census
is on [notes/mutation-testing.md](../mutation-testing.md), and the first half of the alphabet is in
[abi to ipc](baseline-survivors-abi-to-ipc.md).

"The degenerate shifts" and "disjoint `|` versus `^`" below are the
[patterns named once](baseline-2026-08-03.md#patterns-that-recur-named-once) in the baseline
appendix.

### `grant_plan` (26)

25 real. The tokenizer and parser were only ever fed lines whose cursor arithmetic coincided at
small indices (`i + 1` and `i * 1` agree at 1). So buffer ceilings, a trailing `--mem`, and the
flag-cluster bound could all rot into indexing past the line.

- The recursion bit was only tested on `rm`. Its `r` is bit 0, where shift direction cannot matter.
- `prog_id`'s round trip used id 1, the constant the mutant substitutes.
- `from_id`'s arm list had drifted by one program.
- Refusal sentences and Debug renderings are now pinned exactly.

Equivalent: `RECURSIVE`'s `1 << 0`.

Deferred finding, the integrator's call: `NameSet`'s Debug renders numeric byte lists, not the
legible names its own header promises. It is pinned as-is, because the fix changes rendered output.

### `isa` (22)

12 real, and they are the wrong-accept this crate exists to prevent. Both riscv64 fixtures declared
their narrowest hart first, so the widest-wins fold never replaced an answer it already held.

- rv32 after rv64 could keep rv64, and boot on the rv32 machine.
- A base-less extensions node could clobber the base under one flipped `&&`.

The fixes: a wide-first fixture exercises every replacement arm. `Missing::any` gets one
single-missing case per clause. ASIDBits `0b0000` must decode to the 8 bits
`crates/address_space_identifier` assumes.

Equivalent (10): idempotent assignments at `<=` boundaries, disjoint-operand `|` vs `^`, and
`1 << 0`. Six more are compile-time duplicate-check loops. Their guards are vacuous on a table that
already passes. The weekly report will keep resurfacing that pattern, so it is recorded here to
spare a re-triage.

### `measured_boot` (9)

One real: uppercase hex was dead in the suite, because every test round-tripped through our own
lowercase `to_hex`. The other eight are textbook equivalents that will resurface weekly.

- SHA-256's `ch` has operands masked disjoint by `e` and `!e`, so `|` equals `^`.
- `maj` computed with OR is the same majority function as with XOR. They agree wherever at least
  two inputs agree.
- `update`'s boundary take is the same number down both branches at `len == want`.
- The hex nibble combine ORs into cleared low bits.

### `paging` (39)

Seventeen real, and not one of them a bug, which is the result to want from page-table math.

Eleven are accessors and predicates nothing inside the crate reads back:

- `Flags::bits` and `PageFormat::half_base`.
- `is_user_page_va`. Its `&&` to `||` mutant shorts the user-VA gate into an or-gate that admits
  an aligned *kernel* address to a user `MAP`.
- `Mapper::root`, whose value the kernel writes into TTBR0/satp. A constant there installs the
  wrong table in silicon while every walk test still passes.

Four more are descriptor bits the portable `Flags` do not carry, so the encode/decode round-trips
are structurally blind to them:

- aarch64's `TABLE_OR_PAGE` (bit 1). `is_present` never reads it, so a descriptor that loses it
  walks perfectly in software. In hardware it is a translation fault at L3 and a *block*
  descriptor above it.
- aarch64's inner-shareable `SH` on normal memory. The `delete !` mutant hands it to device memory
  instead. Normal pages are left with a core-private view that another core's write never
  invalidates.
- Sv39's `D`.

The last two are in the DMA domain, where every existing test granted exactly one page. That region
shape cannot tell a page count from the constant `1`, nor `grant_page`'s whole-page test from its
opposite. A grant of two and a half pages kills both.

Equivalent: twenty-two, in two families.

- The degenerate shifts: `1 << 0` to `1 >> 0` (`CAP_WRITE`, aarch64 `VALID`, Sv39 `V`) and
  `0b00 << 6` (`AP_RW_EL1`).
- Every `|` to `^` in the `Flags` constructors and in `table_entry`/`leaf_entry` on both formats.
  The six `CAP_*` are distinct single bits. A descriptor's address mask (aarch64 bits 47:12, Sv39's
  PPN at 53:10) shares no bit with its attributes or with valid/type at bits 1:0.

### `pci` (31: 27 missed, 4 timeouts)

Nineteen real, with one cause: the fake config space held exactly one device.

- Enumeration's multifunction handling was never exercised. Nine mutants sit on the single line
  that reads the header type.
- The per-function "nobody home" check was never reached, because no device had a function 1.
- The fixture's 64-bit BAR was unassigned and in the last slot pair. So a base that dropped its
  high half, and a walk that ran one slot past BAR5, both read as correct.

Killed by:

- `enumeration_follows_the_multifunction_bit_and_skips_what_does_not_answer`
- `an_empty_slot_costs_one_config_read`. The vendor-id guard changes no output at all, only read
  count, so a count is its only witness: 45 reads on a 32-slot bus against 306 without it.
- `a_64_bit_bar_in_the_first_slot_keeps_its_high_half`
- `the_command_bits_are_the_specified_positions`
- `a_function_without_a_capability_list_is_not_walked`

Equivalent, seven, all one argument. The `|` to `^` flips in `ecam_offset`, `requester_id`, and
both halves of `read_bars`'s 64-bit assembly are ORs of disjoint bit ranges. `bus:8 | dev:5 | fn:3`
packs without overlap for every BDF a caller can produce, which is what the Kani proof assumes.
`size`'s `mask | 0xffff_ffff_0000_0000` is the same case against a constant.

All four timeouts are genuine detected hangs on the BAR cursor. A fifth mutant joined them:
`i += 2` to `i *= 2` differs from the original only at slot 0, so the new first-slot fixture turned
a silent survivor into a hang.

### `line_editor` (48)

45 real, clustered by what the terminal model hid. `Screen` asserts what the user sees, which is
the right contract. But a test that checks only the finished line never proves *where the cursor
is*.

- The movement layer was mutable at will. ^E and ^F could be deleted. `right` could be emptied to
  `()` or have its `<` flipped four ways. `left`'s `> 0` could become `>= 0`.
  `control_key_movement` and `right_arrow_moves_and_stops_at_end` type a character after every
  move and close all nine.
- The history ring's index arithmetic (`(hist_next + HIST - k) % HIST`) had eight mutants across
  `repeats_newest` and `hist_next_entry`. No test crossed the wrap point, where a `+` for a `-` and
  a `%` for a `/` still land on a plausible entry. `wrapped_ring_walks_correctly_both_ways` walks
  ten commands through a ring of eight in both directions.
- `duplicate_entry_is_stored_once` pins the dedup by bell count. The screen cannot tell one stored
  copy from two, which is why `repeats_newest -> false` survived everything else.
  `empty_lines_stay_out_of_history` covers the `len > 0` half of the same guard.
- The repaint arithmetic (`len - cur` in `start_line`, in ^L, in `yank`, in the stash restore)
  needed a cursor left off the end and *then* a keystroke. The echo alone redraws the same glyphs
  either way.

The rest are one test each:

- the CSI `;` arm and its `n += 1`, one of them a `-=` that underflows to a panic
- ^W's two scan loops, whose `>` to `>=` reads `buf[a - 1]` off the front
- the three-way split at `kill_len > 0`
- `csi_move`'s digit loop, which nothing had ever driven past nine columns

Equivalent (3):

- `req`'s `|` to `^` (opcode in bits 63:56, length in 31:0, disjoint).
- `FLAG_EOF`'s `1 << 0`.
- `csi_move`'s `1` arm, which only elides a count. Delete it and `n == 1` emits `CSI 1 D`, which
  ECMA-48 defines as the same motion as `CSI D`. That is byte economy on a serial line, not
  behavior. Its sibling one line up shows the argument is not an excuse: deleting the `0` arm
  makes a zero-column move travel a column, and `backspace_erases_on_screen` kills it.

### `video_terminal` (79)

The largest single block in the run: 64 real, 15 equivalent, none deferred. The real ones split
three ways.

Geometry and color accessors were never called with a *number*. They were only compared against
another value computed the same way. So `cols()`, `width()`, `height()`, `colours()` and
`to_pixels` could return a constant or swap an operator unnoticed.

- A 6 by 3 grid separates `6*8` from `6+8` from `6/8`.
- Three pixels of a glyph at cell (1,1) separate a divide from a remainder. The two agree at (0,0),
  where every existing pixel assertion sat.
- A union with a rect *inside* the first is the only shape that reads the first operand's far
  edges.

The parser's less-traveled arms were individually deletable because nothing fed them: a tab, a bare
control code, a string terminated by `ESC \`, `CSI 1J`, SGR 27, 39, 49 and 90-97. The switches that
turn something *off* matter most. A terminal that only ever sets attributes passes every test while
leaving a line reversed forever.

Clamps and damage were asserted only where they did not bind. `\x1b[99B` and `\x1b[99;99H` now pin
the clamps. One too generous parks the cursor off the grid, where every later write is silently
dropped. A bare `LF` on the bottom row with the cursor hidden is what makes `damage_all`
non-deletable.

Equivalent (15):

- disjoint-mask `|` versus `^` at three sites
- `DEFAULT_BG << 4` on a zero operand
- the four min/max selections in `union` at equality
- SGR 40-47's `p - 40` versus `p + 40`. 40 is a multiple of 8 and the background is masked to three
  bits, so they agree for all eight legal parameters.
- `erase_display`'s `to > from`, true for every mode
- two match arms that fall through to a byte-identical body
- three range guards no caller can reach, because `col < cols` and `row < rows` are invariants of
  every write. A new caller that broke either invariant turns all three back into real bugs.

A correction from the triage: `csi`'s `>` to `==` at the parameter limit was argued equivalent by
hand and is not. `== MAX_PARAMS` fires on the fourth separator, so it drops a legal `CSI 1;2;3;4 m`
entirely. Only running the mutation caught it.

### `slots` (5)

Four real, all in the half of the API the Kani harnesses never touch.

- `get_mut` could return `None` for a live name with nothing on the host noticing. The kernel
  `unwrap()`s it on the switch path, so the mutant is a kernel panic dressed as a lookup miss.
- `is_empty` could be stuck at `true`, stuck at `false`, or inverted. Nothing in the tree calls it,
  and that is exactly the accessor that rots unobserved.

Killed by `get_mut_reaches_the_live_entry_and_writes_through_it` and
`a_table_is_empty_only_while_it_holds_nothing`.

Equivalent: `name`'s `|` to `^`. The generation is shifted into bits 63:32 and the slot is
`< N <= u32::MAX` by the const assert in `new`, so the two operands never share a bit.

### `socket_protocol` (2)

One real. `DATA_MAX` is `4096 - OFF_PAYLOAD`, and the test only asked whether a full payload *fits*
the frame. `4096 / OFF_PAYLOAD` also fits, so the constant could shrink by 3576 bytes unnoticed. It
is now pinned as the whole page after the header. That refuses both a payload that overruns the
grant and one that leaves granted bytes unreachable.

Equivalent: `req`'s `|` to `^`. The opcode is a byte and the socket id is shifted past it. The
crate's own `every_opcode_fits_in_its_byte` keeps the two ranges disjoint.

### `sink_proto` (2)

Both equivalent, no gaps.

- `req` ORs an opcode shifted to bits 63:56 into a length masked to bits 31:0.
- `pack`'s `bytes.len() < INLINE_MAX` to `<=` is the boundary where both arms return the same
  number. At exactly sixteen bytes `bytes.len()` *is* `INLINE_MAX`, so no slice length
  distinguishes them.

### `user_mode_heap` (6)

Three real, all in the split arithmetic. Nothing saw them because a block's *size* is never
readable. `free_bytes` is an independent counter a wrong split does not touch. Every test asked
only for the block count, which the coalescing invariant expects to be 1.

- `block_count` could return the constant 1 and pass everything.
- `alloc`'s `tail > 0` could become `>=`, writing a zero-length free node one node past the
  donation.
- `bsize - front` could become `bsize + front`. That inflates the remainder by twice the front
  padding and leaves the heap willing to hand out memory nobody gave it.

Killed by `a_block_that_fits_exactly_leaves_nothing_behind` and
`a_split_never_invents_bytes_nobody_donated`. The first donates its arena away from the process heap
on purpose. A mutant writing past the region then lands in slack rather than in the test runner's
own allocations.

The 3 timeouts are detected hangs of one shape: a wrong `front` makes `insert_free` write a node
whose `next` points at itself. Two of the three fail an assertion before anything hangs. They are
recorded as timeouts only because the thrashing test spins in the same binary. That is the caveat on
reading a timeout as "not caught".

### `swish` (2 real, 1 equivalent, 9 timeouts)

The shell's own sentences, where a regression is user-visible and nothing else looks.

- `echo`'s `i > space` could become `>=`, emitting a zero-length whitespace run. In the program
  `out` is the terminal endpoint, so an empty write is a round trip carrying no bytes.
- `write_pwd` could be replaced with nothing at all, because no test called it. A `pwd` that
  printed an empty line would have shipped.

Equivalent: deleting `write_refusal`'s bare `Refusal::NoSuchProgram` arm. The guarded arm above
takes every non-empty program name. The `_` arm it falls into prints a prefix only when
`Program::from_name` resolves, which an empty name never does.

The 9 timeouts are all detected hangs in `echo`'s two-cursor scan.
