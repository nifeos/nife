# P1: before the tag

An appendix to [the milestone](../1086-domain-name-resolution-contract-outside-review.md).

## P1: before the first tag

4. The limits and offsets carry no unit and abbreviate, against the rules now in force. Finding:
   `NAME_TEXT_MAXIMUM` (`src/wire.rs` line 13) is a limit in bytes with no unit and the limit word
   last. `GRANTS_MAXIMUM` (line 29) is a count with the limit word last. `OFF_NAME` and
   `OFF_ADDRESSES` (lines 21, 24) abbreviate `offset`. `MAXIMUM_ADDRESSES` (line 18) and
   `OFF_ADDRESSES` trip the template's address rule, which wants a qualifier that says the address
   is not a memory location (`IP_ADDRESS`, as `GATEWAY_IP_ADDRESS` passes in that helper's own
   test). The field `text_len` (`src/grants.rs` line 64) is a length with no unit, and the template
   counts fields. Two local bindings `op` (`src/grants.rs` line 124, `src/wire.rs` line 207 as
   staged) fail `spelled_out.py` at a ceiling of zero. The parameters `w0` and `w1` of
   `Grants::control` (line 123) and `control_fields` (`src/wire.rs` line 113) are abbreviations, and
   the rule covers parameters. The first three constants carried a ratification in #2797, before the rule
   that a limit opens `MAXIMUM_` and ends with its unit. The `socket_contract` package treats its
   own `DATA_MAXIMUM` the same way, as a name a release review reopens. Why it matters: the template
   runs these checks at a ceiling of zero, so the first pull request in the new repository is red
   for names, and a renamed public constant after the tag is a break. Change: the proposals in the
   decisions below, applied by one rename lane after the ruling. The lane covers the std copy and
   `domain_name_system::MAXIMUM_ADDRESSES`, which `src/lib.rs` line 50 asserts equal. `xtask` regenerates `resolveproto.rs` from `src/wire.rs`, and the farm's consumer
   `nife.rs` lines 1138 to 1215 follows. Door: one-way at the tag; two-way until then. Effort: S for
   the rename, plus the std farm.

5. Ratify the interface identity before the crate leaves.
   Finding: `INTERFACE_ID` 11 and `VERSION` 1 (`src/lib.rs` lines 28 to 42) are provisional.
   Milestone 1017's ruling (its lines 154 to 158 and 228) holds that ratifying the `INTERFACE_ID`
   values comes before the first contract leaves, and makes the `needs-architect` hold on a
   contract crate turn on exactly these values.
   Change: a ruling, recorded in the pull request that applies it; no code changes if the values
   stand.
   Door: one-way (a peer compares it). Effort: S.

6. The control reply word is not in the contract. Finding: a spawner may `CALL` a grant or forget
   message (`src/wire.rs` lines 83 to 85 say so for `OPERATION_FORGET`), and the resolver answers
   with `refused as u64`, 0 or 1 (`components/src/domain_name_resolver.rs` lines 152 to 155). That
   word has no definition in the crate. The spawner in the tree uses `SEND` for every piece
   (`crates/system_initializer/src/image_request.rs` lines 186 to 188), so nobody sees a refused
   grant (`Refusal::BadZone`, `TooLong`, `OutOfOrder`, `Full`). The client holds a badge that the
   resolver denies every name, and the spawner reports a successful grant. The outcome is
   fail-closed, so this is a diagnosability gap, not an authority gap. Change: define the reply to a
   control `CALL` in `src/wire.rs`, in the same packing as `Outcome`, and a `const fn` pair to pack
   and read it. The packing is a status byte and a detail byte that carries a stable code per
   `Refusal`, as `reject_code` does for `domain_name_system::Reject`. The spawner then `CALL`s the
   last piece of a grant and reads the word; the resolver's `refused as u64` becomes the packer.
   Record the current 0 and 1 as the compatible values of the new status byte. Door: one-way at the
   tag (a wire word). Effort: S in the crate, S in the resolver and spawner.

7. One field, three bounds. Finding: the crate root says the client writes at most
   `NAME_TEXT_MAXIMUM` bytes (`src/lib.rs` line 9). `grant_messages` accepts a zone of
   `NAME_TEXT_MAXIMUM + 1` bytes for a trailing dot (`src/messages.rs` lines 9 to 12) and the table
   does the same (`src/grants.rs` line 134). The resolver accepts a resolve of `NAME_TEXT_MAXIMUM +
   1` bytes (`components/src/domain_name_resolver.rs` line 246). The std client refuses anything
   over `NAME_TEXT_MAXIMUM` (`nife.rs` line 1144). Nothing asserts that the name field ends before
   `OFF_ADDRESSES` (0x100, 256 bytes, against a 254-byte name: true today, by three bytes,
   unchecked) or that the addresses end inside a page. Change: state the trailing-dot allowance
   once, at the limit's definition, and have the resolver and the std client read the same
   expression. Add the layout assertions a page contract carries (`const _: () = assert!(NAME_OFFSET
   + MAXIMUM_NAME_TEXT_BYTES + 1 <= IP_ADDRESSES_OFFSET)` and `IP_ADDRESSES_OFFSET + 4 *
   MAXIMUM_IP_ADDRESSES <= PAGE_BYTES`, with `PAGE_BYTES` spelled as the naming rules for a page
   contract ask). Door: two-way (the values do not change). Effort: S.

8. `Outcome::from_word` decodes words the resolver cannot send.
   Finding: `from_word` (`src/wire.rs` lines 201 to 209) refuses only a word with the high 40 bits
   set. A `count` of 255 and a `status` of 200 decode as valid. The std client clamps the count
   itself (`nife.rs` line 1172); the test client reads one address and is not exposed. The bound
   belongs to the contract, where every client gets it.
   Change: return `None` for `count > MAXIMUM_ADDRESSES`, and for a status the `status` module does
   not define; add the two cases to `an_outcome_word_round_trips_and_a_kernel_error_is_not_one`.
   Door: two-way (narrows decoding; the resolver never produces those words). Effort: S.

9. The documentation does not stand alone, and the crate root's table renders wrong. Finding: the crate
   has no README. The crate root's layout table renders its second row as literal pipes (verified in
   the generated HTML; `src/lib.rs` lines 7 to 12 break a row across lines). The sequence a stranger
   needs first (the spawner grants, the client attaches then resolves, the resolver judges then
   asks) is in `notes/name-resolution/the-contract.md`, with its diagram, and only linked from the
   crate. The template's acronym gate lists ten unexpanded acronyms at first use (IPv4, PAL, DHCP,
   UDP, TCP, RFC, DNS, TTL, NXDOMAIN; and `OK`, which belongs in the accepted list). The two tiers
   of zone spelling (item 13) have no single explanation. Change: a README with the three-party
   diagram, the message sequence, the page layout, the reply words and both tiers of zone spelling,
   with `EXAMPLES` (the `Grants` doc example, which already compiles) and `BUGS` (the crate root's
   three, reworded). Fix the table. Expand each acronym once in the crate root. Door: two-way.
   Effort: M.

10. The grant table has an argument for safety and no proof. Finding: `Grants::control` is the one stateful
    parser in the crate. Its `u8` subtraction `e.text_total - e.text_len` (`src/grants.rs` line 157)
    and the slice `e.text[start..start + n]` (line 159) are safe by an invariant the code keeps. An
    incomplete entry always has `text_len < text_total`, so the subtraction never wraps and the
    slice stays inside the 254-byte buffer. No comment states the invariant and no proof checks it.
    The template runs Kani on every pull request and `falsify.py --check` passes trivially with zero
    harnesses. Change: one bounded Kani harness, with its falsification patch. From `Grants::new()`,
    up to four arbitrary `(w0, w1)` messages never panic. `check` returns `Ok` only for a badge
    whose last control result was `Granted`. A badge's window never changes while it holds one. One
    cargo-fuzz target that feeds arbitrary word pairs to a table and asserts the same, so the weekly
    fuzz job has a target when the repository opts in (the crate has no `unsafe`, so it may). Door:
    two-way. Effort: M.

11. The six surviving mutants need a listing in the new repository. Finding: the census records
    6 missed mutants, all `|` replaced by `^` between disjoint bit fields (`port_and_transport`,
    `control_word` three times, `Outcome::word` twice). They are equivalent mutants: no input
    distinguishes `|` from `^` on bits that never overlap. The dry run carries the row into
    `.github/mutants-exempts` as a comment and says the first weekly run will list the survivors. The template's `mutation` verdict is red until they are. Change: list the six in
    `.github/mutants-exempts` with the reason, in the move pull request, so the first weekly run is
    green for the crate's own reasons. Refresh `.cargo/mutants-baseline.txt` line 34, which carries
    the first census's figures, or delete the row at the move as the dry run instructs. Door:
    two-way. Effort: S.
