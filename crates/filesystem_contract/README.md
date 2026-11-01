# filesystem_contract

The wire protocols of a filesystem stack, as code: the **block protocol** between a filesystem
server and a block server, and the **file protocol** between a client and the filesystem server.
`no_std`, no dependencies, no system calls. It was written for the nife kernel's filesystem
stack, whose block server, filesystem server, caretakers and clients share this one definition so
they cannot drift. A new verb changes this crate; the kernel routes the words as any IPC message
and never reads an opcode.

Each request is an endpoint `CALL` of two words, answered through a one-shot reply capability. Bytes
travel only in a region the two parties share. A reply's first word is an `i64`: a non-negative
result, or the negated errno (`reply_err` and `reply_errno` are the one place that convention is
defined). The opcode sits in bits 63:56 of the first word (`OPERATION_SHIFT`, `req`, `operation`).

## The contract

- `blk` (the block protocol, shaped like a block device trait): `READ`, `WRITE`, `SIZE` and `FLUSH`.
  The unit is one block of `PAGE` (4096) bytes, and a request moves up to `blk::TRANSFER_BLOCKS` (16)
  of them. `blk::VERSION` is its version.
- `fs` (the file protocol): the endpoint a client holds is a directory capability, bound in the
  server to one directory node, and every name is resolved relative to a handle under it. There is
  no global namespace and no absolute path. A handle is a small integer the server mints and checks
  against the session's table, so forging one is meaningless; `fs::ROOT` (0) is the bound directory.
  The verbs are `OPEN`, `READ`, `WRITE`, `CLOSE`, `FSTAT`, `CREATE`, `TRUNCATE`, `OPENDIR`,
  `READDIR`, `MKDIR`, `RENAME`, `UNLINK`, `RMDIR`, `GETXATTR`, `SETXATTR`, `LISTXATTR`,
  `REMOVEXATTR`, `STATFS`, `SYNC`, `GETMTIME`, `SETMTIME`, `SETMTIME_AT` and `NARROW`, with `BIND`,
  `UNBIND` and `BIND_SYNC` outside the table's range so no caretaker forwards them. A transfer is up
  to `fs::TRANSFER_PAGES` (16) pages. `fs::VERSION` is its version.
- `dir` (what a directory capability carries): a rights set of `ENUMERATE`, `READ`, `WRITE`,
  `CREATE`, `REMOVE`, `DESCEND` and `SETTIME`. A child's rights are the parent's rights ANDed with
  the request (`Rights::attenuate`), so no child carries more than its parent. A missing right
  answers an errno chosen for what the holder learns: a naming right is `ENOENT`, a mutating right
  is `EROFS`, and `ENUMERATE` is `EPERM`. `explain` gives a fact-only sentence for each errno.
- `verb`: each request's argument shape and the rights a server demands, declared once in `TABLE`,
  so every caretaker dispatches off it. A verb in `fs` without a row is a compile error.
- `dirent` (a listing in the shared page: a flags byte, a length byte, then the name), `statfs`
  (three little-endian `u64`: the allocation unit, the volume size and the free count in those
  units; the reply length is the version), `grant` (a per-file grant: a name that fits two words or
  is longer, plus `READ` and `WRITE`), `nameset` (a directory attenuated to at most eight names and
  512 bytes of name) and `xattr` (extended attributes: limits, encodings and the store) are the
  encodings. A set or name is never silently truncated; a limit is refused with its own errno.
- `READY` is the filesystem server's readiness word, and `DESCENT_REFUSED` a caretaker's refusal.
  `rm` defines what the `rm` program reports.

## Usage

```rust
use filesystem_contract::{dir, fs, operation, reply_err, reply_errno, req};

// A block-protocol request is just an opcode.
assert_eq!(operation(req(filesystem_contract::blk::READ)), filesystem_contract::blk::READ);

// An error is the negated errno; a length is itself.
assert_eq!(reply_errno(reply_err(2)), Some(2));
assert_eq!(reply_errno(4096), None);

// A child directory never carries a right its parent lacks.
let parent = dir::Rights::root(dir::READ | dir::ENUMERATE);
let child = parent.attenuate(dir::READ | dir::WRITE);
assert!(child.allows(dir::READ) && !child.allows(dir::WRITE));
assert_ne!(fs::OPEN, 0, "0 is not a verb: a zeroed word must not decode as one");
```

## What is proven

Three Kani harnesses in `src/dir.rs`, each with a falsification patch in `falsifications/`:
`attenuate_never_widens`, `a_grandchild_is_bounded_by_the_root` and
`a_root_carries_nothing_undefined`. Rerun them with `cargo kani`; applying a patch with
`patch -p3` from the crate root makes its harness fail.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
cargo kani
```

The minimum supported Rust version is 1.88. The tests that pin the nife test fixture against these
protocols live with the fixture, and the ones that pin the fixture against a glob matcher live with
the matcher.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
