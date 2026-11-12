# P2: later

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md).

## P2: after the first tag

14. `blk::req_blocks` decodes a count the contract refuses.
    Finding: `src/blk.rs` line 80 returns `(w0 & 0xff) + 1`, so a low byte of `0xff` decodes as 256
    where `TRANSFER_BLOCKS` is 16. All three block servers clamp with `.min(TRANSFER_BLOCKS)`
    (`components/src/designware_mobile_storage.rs` line 187,
    `components/src/non_volatile_memory_express_data_plane.rs` line 335, `crates/virtio/src/lib.rs` line 953).
    Change: clamp in the decoder, or return `Option<usize>` and let a server refuse with `EINVAL`;
    the contract then says which. Door: two-way. Effort: XS.

15. `dirent::encode` accepts an empty name; its two siblings refuse one.
    Finding: `src/dirent.rs` line 13 writes a record with length 0 and `iter` yields an empty name;
    `nameset::encode` and `xattr::list::encode` refuse an empty name because zero is their
    terminator. The server never sends one (unverified).
    Change: refuse it in `encode` and say so, or document that a listing may carry one.
    Door: two-way (a reader change could be one-way; refusing in the encoder is not). Effort: XS.

16. The errno vocabulary has three spellings.
    Finding: the doc of `xattr::ENOTSUP` (`src/xattr.rs` line 67) says `EOPNOTSUPP`. `blk::FLUSH`
    (`src/blk.rs` line 33) says `EOPNOTSUPP (95)`. `dir::explain` (`src/dir.rs` line 114, `:120`, `:123`,
    `:124`) and `store::syscall_einval` (`src/xattr.rs` line 340) use bare literals for ENOENT, EEXIST,
    EBADF, EMFILE and EINVAL. `dir` and `grant` each define `EROFS` and `ENOTDIR`, and `EBADF`
    once more under `verb::file_grant`. `redox_syscall` spells 95 `EOPNOTSUPP`.
    Change: one `errno` module naming every number the contract answers, each with its sentence,
    and `explain` matching on names; the duplicates become re-exports.
    Door: the names are public (Decision B covers the spelling); the layout is two-way. Effort: S.

17. `MAXIMUM_LENGTH_OR_OFFSET` describes a field that carries no offset.
    Finding: `src/fs.rs` line 304 says "the largest length or offset that fits the packing"; the
    offset of `READ`, `WRITE` and `TRUNCATE` rides whole in the second word (`src/fs.rs` line 33,
    `:36`, `:49`), and the packing covers only the length. The name has its ratification.
    Change: correct the doc now; the name waits for Decision B's pass. Door: two-way for the doc.
    Effort: XS.

18. Two distinctness tests are stale and redundant.
    Finding: `src/tests.rs` line 7 lists seven verbs; `src/fs/tests.rs` line 83 lists twenty-two and omits
    `STATFS`. The compile-time assertions in `src/verb.rs` line 307 and `:311` already prove every
    opcode in `FIRST..=LAST` is distinct, in order and in the 8-bit field.
    Change: derive one test from `verb::TABLE` plus `BIND`, `UNBIND`, `BIND_SYNC`, or delete both.
    Door: two-way. Effort: XS.

19. `usize` casts on a narrower target.
    Finding: `fs::req_len`, `fs::dst_len` (`src/fs.rs` line 361, `:376`) and `xattr::spec_value_len`
    cast a 40-bit or 32-bit field to `usize`. On a 32-bit target the 40-bit cast drops the high
    byte; a length of `2^32 + 4096` reads as 4096, a smaller successful read, the failure the
    `const` block names. None of the crate's three targets is 32-bit. Inferred, not built.
    Change: `usize::try_from(..).ok()` where a caller can refuse, or a `const` assertion that
    `usize::BITS >= 40` is false on every target and so refuses a 32-bit build with a message.
    Door: two-way. Effort: XS.

20. `in_opcode_order` warns on the minimum Rust version.
    Finding: stable 1.88 reports the function never used, since its only non-test use is inside a
    `const _` assertion that 1.88's dead-code pass does not count; 1.99 does. The weekly `msrv`
    job builds and tests without `-D warnings`, so it stays green.
    Change: `#[cfg_attr(not(test), allow(dead_code))]` with the reason, or make the assertion call
    a `pub` function. Door: two-way. Effort: XS.

21. No fuzz target of the crate's own.
    Finding: the crate has no `unsafe`, so it qualifies for the template's public-crash-inputs
    opt-in; nife's only fuzz target that links it drives `redoxfs_server`. The four walkers and
    `store::set`/`remove` take arbitrary bytes.
    Change: a `fuzz/` with one target per walker (iterate, then re-encode what the walker read and check
    the round trip) and one for the store; opt in. Door: two-way. Effort: S.

22. `xattr::reply` masks a kind it should never receive.
    Finding: `src/xattr.rs` line 101 masks `kind & MAXIMUM_KIND`; `store::set` refuses such a kind, so
    a stored kind never has bit 31, and a caller that bypasses the store gets a different kind back
    without an error. The test at `src/xattr/tests.rs` line 288 calls the masking "rather than lies".
    Change: assert, as item 7. Door: two-way. Effort: XS.
