# P2: later

An appendix to [the milestone](../1110-subtree-scope-outside-review.md).

## P2: later

11. `Refusal::errno` restates numbers the contract defines.
    Finding: `EPERM`, `ENOTDIR` and `EINVAL` exist in `filesystem_contract::dir` (and `EISDIR`,
    which item 4 needs); the crate writes 1, 20 and 22 again. `ENOENT`, `ELOOP`, `EXDEV` and
    `EBADF` have no constant in the contract. The numbers are Linux's generic values (`ELOOP` is 40
    here and 62 on macOS), and the doc says the crate speaks no wire while it does speak a
    numbering.
    Evidence: `crates/subtree_scope/src/lib.rs` lines 55 to 65; `filesystem_contract/src/dir.rs`
    lines 84 to 94.
    Change: use the contract's constants where they exist, and one sentence in `errno`'s doc
    saying whose numbering the rest follow.
    Door: two-way. Effort: XS.

12. `admit` takes three `u64` words in a row.
    Finding: `admit(binding, badge, requested, owner)`; swapping `badge` and `requested` compiles,
    and the one call site is the only thing that keeps them straight.
    Evidence: `crates/subtree_scope/src/lib.rs` lines 183 to 188; `redoxfs_server/src/server.rs`
    line 698.
    Change: two newtypes, `Badge(u64)` and `Handle(u64)`, or a `minted_by: Option<Badge>` name
    for the last parameter. The server's `u32` handles widen at the call as they do now.
    Door: two-way before the tag. Effort: S.

13. `#![cfg_attr(not(test), no_std)]` where `#![no_std]` would do.
    Finding: the tests use only `core` macros as far as reading shows; a plain `#![no_std]` would
    say the same thing without the branch. Unverified: that the test module compiles under it.
    Evidence: `crates/subtree_scope/src/lib.rs` line 22.
    Change: try `#![no_std]`; keep the attribute if a test needs `std`.
    Door: two-way. Effort: XS.

14. The doc says nothing about how the hop word masks.
    Finding: `walk` compares `rights.attenuate(DESCEND | hop)` with `Rights::root(DESCEND | hop)`,
    so the comparison silently drops bits of `hop` outside `dir::ALL`, and the contract's `PARENT_RIGHTS`
    sentinel, if it reached `walk` unresolved, would ask for `DESCEND` alone. The directory verb
    resolves the sentinel first (`open_dir`, line 127); the file verb passes its word through
    (`open_file_path`, line 97). Whether a client can put the sentinel in `OPEN`'s second word is
    the server's matter and unverified; the crate should say what it does with undefined bits.
    Evidence: `crates/subtree_scope/src/lib.rs` lines 126 to 130; `redoxfs_server/src/server.rs`
    lines 97 and 127; `filesystem_contract/src/dir.rs` lines 49 to 61.
    Change: one sentence in `walk`'s doc, and a unit test with a hop word that has a high bit set.
    Door: two-way. Effort: XS.

15. The README should say where this crate is stricter than Linux.
    Finding: `..` meets a refusal everywhere, where `openat2` with `RESOLVE_BENEATH` accepts a `..` that
    stays beneath; a symbolic link is never followed, where `cap-std` follows one whose target
    stays beneath. A reader coming from either will file these as bugs unless the README names them
    as the rule.
    Evidence: `crates/subtree_scope/src/lib.rs` lines 81 to 85, 141.
    Change: a short `## How it differs from openat2 and cap-std` section, after `## BUGS` in the
    scaffold's order.
    Door: two-way. Effort: XS.
