# filesystem_contract

[![check](https://github.com/nifeos/filesystem_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/filesystem_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/filesystem_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/filesystem_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/filesystem_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/filesystem_contract/blob/main/Cargo.toml)

The wire protocols of a filesystem stack, as code: the block protocol between a filesystem server
and a block server, and the file protocol between a client and the filesystem server. The crate is
`no_std` and has no dependencies and no system calls. The block server, the filesystem server, the
caretakers and the clients of one stack share this definition, so they cannot drift apart. A new
verb changes this crate. The kernel routes the words as any inter-process communication (IPC)
message and never reads an opcode.

Each request is an endpoint `CALL` of two words, answered through a one-shot reply capability.
Bytes travel only in a region the two parties share. A reply's first word is an `i64`: a
non-negative result, or the negated errno. `reply_err` and `reply_errno` are the one place that
convention lives. A word in `-4095..=-1` is the error code `1..=4095` (`MAXIMUM_ERRNO`). Any other
negative word is a protocol violation, and it reads as `EPROTO` (71), never as success. The
sender's `reply_err` refuses a code outside `1..=4095`. The opcode sits in bits 63:56 of the first
word (`OPERATION_SHIFT`, `request`, `operation`).

## The contract

- `block` is the block protocol, shaped like a block device trait: `READ`, `WRITE`, `SIZE` and
  `FLUSH`. The unit is one block of `PAGE_BYTES` (4096) bytes, and a request moves up to
  `block::TRANSFER_BLOCKS` (16) of them. `block::VERSION` is its version.
- `fs` is the file protocol. The endpoint a client holds is a directory capability, bound in the
  server to one directory node, and every name resolves relative to a handle under it. There is no
  global namespace and no absolute path. A handle is a small integer that the server mints and
  checks against the session's table, so forging one is meaningless. `fs::ROOT` (0) is the bound
  directory. A transfer is up to `fs::TRANSFER_PAGES` (16) pages, and `fs::VERSION` is the version.
  The verbs are `OPEN`, `READ`, `WRITE`, `CLOSE`, `FSTAT`, `CREATE`, `TRUNCATE`, `OPENDIR`,
  `READDIR`, `MKDIR`, `RENAME`, `UNLINK`, `RMDIR`, `GETXATTR`, `SETXATTR`, `LISTXATTR`,
  `REMOVEXATTR`, `STATFS`, `SYNC`, `GET_MODIFICATION_TIME`, `SET_MODIFICATION_TIME`,
  `SET_MODIFICATION_TIME_AT` and `NARROW`. `BIND`, `UNBIND` and `BIND_SYNC` sit outside the
  table's range, so no caretaker forwards them.
- `directory` is what a directory capability carries: a rights set of `ENUMERATE`, `READ`, `WRITE`,
  `CREATE`, `REMOVE`, `DESCEND` and `SETTIME`. A child's rights are the parent's rights combined by bitwise AND with
  the request (`Rights::attenuate`), so no child carries more than its parent. A missing right
  answers an errno chosen for what the holder learns. A naming right is `ENOENT`, a mutating right
  is `EROFS`, and `ENUMERATE` is `EPERM`. `explain` gives a fact-only sentence for each errno.
- `verb` declares each request's argument shape and the rights a server demands, once, in `TABLE`,
  so every caretaker dispatches off it. A verb in `fs` without a row is a compile error.
- `directory_entry`, `statfs`, `grant`, `nameset` and `extended_attribute` are the encodings. A
  listing in the shared page is a flags byte, a length byte, then the name. A `statfs` reply is
  three little-endian `u64`: the allocation unit, the volume size and the free count in those units.
  The reply length is the version. A `grant` is a per-file grant: a name that fits two words or is
  longer, plus `READ` and `WRITE`. A `nameset` is a directory attenuated to at most eight names and
  512 bytes of name. `extended_attribute` holds the limits, the encodings and the store. The crate
  never truncates a set or name silently, and it refuses a limit with its own errno.
- `READY` is the filesystem server's readiness word, and `DESCENT_REFUSED` is a caretaker's
  refusal. `rm` defines what a remove program reports.

## EXAMPLES

```rust
use filesystem_contract::{directory, fs, operation, reply_err, reply_errno, request};

// A block-protocol request is just an opcode.
assert_eq!(operation(request(filesystem_contract::block::READ)), filesystem_contract::block::READ);

// An error is the negated errno; a length is itself.
assert_eq!(reply_errno(reply_err(2)), Some(2));
assert_eq!(reply_errno(4096), None);

// A child directory never carries a right its parent lacks.
let parent = directory::Rights::root(directory::READ | directory::ENUMERATE);
let child = parent.attenuate(directory::READ | directory::WRITE);
assert!(child.allows(directory::READ) && !child.allows(directory::WRITE));
assert_ne!(fs::OPEN, 0, "0 is not a verb: a zeroed word must not decode as one");
```

## What is proven

Eleven Kani harnesses prove the contract, and each has a falsification patch in `falsifications/`.
Three are in `src/directory.rs`: `attenuate_never_widens`, `a_grandchild_is_bounded_by_the_root`
and `a_root_carries_nothing_undefined`. Two are in `src/reply.rs` and cover the reply word's error
range for every `i64`: `every_word_decodes_within_the_bound` and `a_sent_code_round_trips`. Six are
in `src/proofs.rs`. They show that every field a request word carries comes back out unchanged, that
the verb table is total over the opcode space, and that a resolved request never widens.

## Building and testing

```sh
cargo build
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none   # and the other two targets
```

### Running the proofs

Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup
step downloads a model checker, so it needs network access. CI uses version 0.68.0, and another
version may verify differently. Then run `cargo kani`, or one harness with
`cargo kani --harness directory::proofs::attenuate_never_widens --exact`. A passing run prints some
alarming warnings that mean nothing: the unstable-feature warning, the unsupported-constructs
warning and the lines about aborting a path on `assume(false)`. The last line is the answer:
`Complete - N successfully verified harnesses, 0 failures, N total.`

### Running one falsification patch

```sh
patch -p1 --no-backup-if-mismatch < falsifications/directory.proofs.attenuate_never_widens.patch
cargo kani --harness directory::proofs::attenuate_never_widens --exact   # ends VERIFICATION:- FAILED
git checkout -- src
```

The patch swaps the bitwise AND in `attenuate` for an OR, so the failing run reports the
`attenuate_never_widens` check as failed and ends with `VERIFICATION:- FAILED`.

## BUGS

- `STATFS` answers about the whole image, never a subtree. Its free count is a forecast, not a
  reservation, because the store's `ENOSPC` at write time decides. Two subtrees on one image can
  signal through it.
- `SYNC` syncs the whole device and does not fence. A write and a sync issued at once on two
  handles have no order.
- The verb table does not make the caretakers interchangeable. What each one attenuates to stays
  hand-written.

## Minimum Rust version

Rust 1.88. Check it with `rustup toolchain install 1.88 --profile minimal`, then `cargo +1.88 test`
and the bare-metal build with the same `+1.88`.

## License

Licensed under either of the MIT license (`LICENSE-MIT`) or the Apache License,
Version 2.0 (`LICENSE-APACHE`), at your option.
