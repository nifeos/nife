# Shared-page audit: candidates cleared, and why each is safe

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/shared-page-audit.md](../shared-page-audit.md), which lists these in one
paragraph.

Recorded because "we looked and it is fine" is the other half of an audit, and because each is a
place a future change could break something.

## The FS server's whole request decode

`len`, `offset`, the handle, the opcode, `RENAME`'s
destination handle and length, and `SETXATTR`'s value length and type code all come from `w0`/`w1`.
`len` is clamped to `BLOCK`; `RENAME` and `SETXATTR` refuse rather than clamp when the two payloads
would overrun the page, which is right (clamping a name renames something else). The bit-packed
fields cannot overflow the additions that check them: `req_len` is masked to 40 bits, `dst_len` to
40, `spec_value_len` to 32, and `len` is already at most 4096, so every sum is far inside `u64`. On a
32-bit target `len + dst_len` would wrap; this kernel has no 32-bit target and, if it ever does,
that sum is the line to revisit.

`GETXATTR` copies the name to the stack before writing the reply over it, and says why. That is
this audit's lens applied correctly, in the tree, before the audit existed.

## `credential_contract::read`

Both lengths from the register word, checked against `MAX_IDENTITY` and
`MAX_SECRET`, fixed offsets, one bound against the page. The returned slices alias the page and their
*contents* can change under the store, but their lengths cannot, and the party that would change them
is the one that wrote the secret in the first place. The credentialer also wipes the request area on
every exit path, which is the right discipline for a page a secret passes through.

## The display driver's `FLUSH`

The rectangle is four 14-bit fields of `w0`; the second data word
is explicitly discarded. Nothing in that serve loop reads the shared page at all, which makes it
structurally immune. Its doc comment records that reading the rectangle out of the second word was
the first bug caught there.

## The keyboard ring, read by the kernel

`take_typed` reads `tail` once into a local, fences, and
indexes with `head % ring::CAPACITY`, so a hostile `tail` cannot move the index out of the frame.
`head` is the reader's own field, kept in the reader's memory.

## The compositor's damage handling, considered as a double fetch

The four fields are read once
into a `Rect` local, and both the intersect and the blit use that local. This is the exact place a
double fetch would live and it is not there. (Its problem is finding 5's, which is a different
question: whether the single read is of a consistent value.)

## `line_editor`

Four length sites, four clamps, and `copy_in` is called with
`offset + dst.len() <= PAGE` at every one.

## The C seam

`c_seam_transform` does `strlen` and then `memcpy` of `n + 1` bytes, which is
literally a double fetch. It is harmless: the copy's size comes from the first read and the
buffer is allocated to it, so a changed page changes the bytes and not the bounds. The one thing that
would bite (an input area with no NUL, making `strlen` run off the mapping) cannot happen: the
confiner writes the input and the frame is zeroed at wiring. The Rust side reads only fixed-size
fields and compares against a constant.

The seam also turned out to be better than its reputation. `crates/c_seam` now parses the
`#define`s out of `fixtures/c/c_seam.c` and asserts they equal the Rust constants, so the "written twice
with nothing checking that the two agree" warning in `CLAUDE.md` is stale. Correcting that file is
the maintainer's; it is reported rather than edited here.

## The clock page

Mapped `user_rodata` into the progenitor, the shell and any child whose manifest declares
one. A holder can read the wall clock and cannot set it, which is the point, and read-only is what
makes it a fact about the mapping instead of about the code.

## The nameset grant frame

Mapped `user_rodata` into the caretaker and copied into a local at
`_start` "so nothing that happens to that page afterwards can widen the namespace." That is this
audit's lens, stated in the tree, and it is the model the file page should follow.

## The socket contract's decode

A socket is found by its badge (§255 (each socket is its own
capability)); every payload length refused above `DATA_MAX`; receives
staged through a `[0u8; DATA_MAX]` stack buffer and then copied into the page. No slice is ever
formed over the shared mapping, in the server, in `std::net`'s PAL, or in `network_time_client`, so a concurrent
writer can change the bytes that go out and can corrupt nothing. The PAL's half is generated from
`crates/socket_contract` by `xtask`, so the offsets cannot drift.

One thing there is worth naming before somebody tidies it: `OFF_LEN` is a length field in the page
that nothing reads. The server writes it on receive and no client consults it; every length that
matters travels in a register. It is documented as "in for `SEND*`, out for `RECEIVE`", which is an
invitation. The first change that makes the server honor the header length converts a contract with
no double fetch into one with a double fetch, and the diff will look like a tidy-up.
