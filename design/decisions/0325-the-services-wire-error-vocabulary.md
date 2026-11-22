---
status: PROPOSED
raised: 2026-11-22
---

# 325. The error vocabulary the services speak on the wire

*(The section number and title are provisional until the merge lands them.)*

Counts are from `nifeos/nife@4e82cb0cc`, excluding `vendor/` and falsification patches unless a
line says otherwise.

## Background

A nife reply is a few 64-bit words, and the first one, read as an `i64`, says whether the request
worked: non-negative is a result (a length, a handle), negative is an error. Two parties write that
word. When the kernel refuses the call itself (an empty slot, a missing right), it writes one of its
own codes, `abi::Error`, numbered -1 to -15 (`NoSuchSlot` -1, `WrongObject` -2, `NotPermitted` -3).
When the call reaches a server and the server says no, the server writes a negated POSIX `errno`
with Linux's numbering: -2 for `ENOENT`, -30 for `EROFS`.

Nobody chose `errno` for nife. The file server wraps RedoxFS, whose methods return
`syscall::error::Result` with Linux numbers, and the reply convention in `filesystem_contract`
(`reply.rs`, being renamed to `file_system_contract::error` with `error::word(code)` and
`error::code(word)` on #3914) passes those numbers through. The block, NVMe and SD card servers then
copied the convention, and so did the graphics contract.

This matters now for two reasons. The two spaces overlap, so the same word means two things (below).
And the names `MAXIMUM_ERRNO` and `EPROTO` are about to be renamed on #3914, which needs to know
whether the vocabulary they belong to stays.

## The decision

Whether the services keep Linux-numbered POSIX `errno` as their wire error vocabulary, or nife
defines its own wire error codes and each server translates at its boundary.

## Recommendation: nife's own codes, in a range disjoint from the kernel's

- A reply's error word is a nife code. One table names every code a service may send, by meaning,
  with spelled-out names (`NotFound`, `ReadOnly`, `NotADirectory`; the names are a later naming
  ruling, not this one).
- The kernel's range and the services' range are fixed now and disjoint. The kernel keeps `-1..=-255`
  (it uses 15). Services use `-256..=-4095`. A word below -4095 stays a protocol violation, as ruled
  on 2026-11-14 (block 1090, decision C), so the bound 4095 survives with a nife name.
- Each server translates once, at its boundary: the file server maps `syscall::error::Error` to a
  nife code in one function, and nothing below that boundary changes.
- `errno` stays where POSIX is the contract: the C library's `errno` (`c_library_errno`), which is
  already translated from `io::ErrorKind` and never reads the wire.

The reason is on the merits, not effort; it is the dearer option. The measured defect is the
overlap: seven codes in use collide with kernel codes, and the `std` overlay already trades one
answer away to live with it. A disjoint range removes the collision by construction. The Linux
numbering buys nothing on this wire: no consumer passes a number through to a POSIX program, since
`std` maps by meaning and the C library maps again from `ErrorKind`.

## Is the premise true

Yes, with one correction. The services do speak Linux numbers (`redox_syscall` 0.9.0's `error.rs`
reads `EPERM = 1`, `EPROTO = 71`, `EOPNOTSUPP = 95`). But the vocabulary is wider than
`file_system_contract`'s named copies: servers send 19 numbers the contract never names, and three
servers write raw literals (`-95`, `-5`, `-22` in `components/src/block_driver_roles.rs`). Sockets
do not use `errno` at all.

## Every code in use on any wire

Command: `git grep` for `reply_err(`, `Error::new(`, `const E[A-Z]+: i32`, and raw negative literals
in `components/`, `crates/`, `redoxfs_server/`; engine codes by
`grep -rhoE "Error::new\(([a-z_]+::)*E[A-Z]+\)" vendor/redoxfs/src`.

**The file service** (`fs` protocol). Synthesized by nife: the file server (`redoxfs_server`), the
three caretakers (`fs_file_caretaker`, `fs_nameset_caretaker`, `fs_subtree_caretaker`), `disk_tree`
and `subtree_scope`'s `Refusal::errno`. Passed through: what the RedoxFS engine returns, and, on
`SYNC` only, the block server's code.

| number | name | synthesized by nife | passed through from RedoxFS | `std` `ErrorKind` |
|---:|---|---|---|---|
| 1 | `EPERM` | yes | yes | `PermissionDenied` |
| 2 | `ENOENT` | yes | yes | `NotFound` |
| 5 | `EIO` | yes | yes | `Other` |
| 7 | `E2BIG` | yes | | `Other` |
| 9 | `EBADF` | yes | yes | `InvalidInput` |
| 12 | `ENOMEM` | | yes | `Other` |
| 13 | `EACCES` | | yes | `Other` |
| 16 | `EBUSY` | | yes | `Other` |
| 17 | `EEXIST` | yes | yes | `AlreadyExists` |
| 18 | `EXDEV` | yes | yes | `Other` |
| 20 | `ENOTDIR` | yes | yes | `NotADirectory` (`Unsupported` from an mtime verb) |
| 21 | `EISDIR` | yes | yes | `IsADirectory` |
| 22 | `EINVAL` | yes | yes | `InvalidInput` |
| 24 | `EMFILE` | yes | | `Other` |
| 27 | `EFBIG` | yes | | `Other` |
| 28 | `ENOSPC` | yes | yes | `StorageFull` |
| 30 | `EROFS` | yes | | `ReadOnlyFilesystem` |
| 34 | `ERANGE` | yes | yes | `Other` |
| 38 | `ENOSYS` | | yes | `Other` |
| 39 | `ENOTEMPTY` | | yes | `DirectoryNotEmpty` |
| 40 | `ELOOP` | yes | yes | `Other` |
| 61 | `ENODATA` | yes | | `Other` |
| 71 | `EPROTO` | decoder only, never sent | | `Other` |
| 77 | `EBADFD` | | yes | `Other` |
| 95 | `ENOTSUP` / `EOPNOTSUPP` | yes | yes (and `SYNC` from block) | `Other` |
| 126 | `ENOKEY` | | yes | `Other` |
| 129 | `EKEYREJECTED` | | yes | `Other` |

That is 27 numbers. The overlay (`sys/fs/nife/errors.rs`) distinguishes ten of them; the other 17
are `Other`. It also reads -3 as the kernel's `NotPermitted` and answers `Unsupported`
(`reachable.rs`). Whether every engine code is reachable through the server was not checked.

**The block protocol** (`filesystem_contract::block`), all synthesized: virtio block
(`block_driver_roles`) `EINVAL` 22, `EIO` 5, `EOPNOTSUPP` 95; NVMe (`non_volatile_memory_express_data_plane`)
and SD card (`designware_mobile_storage`) `EINVAL` 22, `EROFS` 30, `EIO` 5. The file server, the one
production client, turns every negative block reply into `EIO` (`redoxfs_server/src/bin/redoxfs_server.rs`,
`mkfs.rs`), except `SYNC`'s, which it passes through. Only `kernel/src/storage_bench.rs` and the
system tests read the other numbers.

**The graphics protocol**: `graphics_contract::EINVAL` and `compositor`'s `EBADOP`, both -22.

**Sockets** (`socket_contract`): no `errno`. Per-protocol outcomes (`CONNECT_REFUSED` 1,
`PORT_CLAIM_DENIED` 1, `PORT_CLAIM_IN_USE` 2) and one sentinel, `REPLY_ERROR` = `1 << 32`, a positive
word, so it never meets the kernel's range.

### The overlap, measured

`abi::Error` uses -1 to -15. Seven numbers in the table share a word with a kernel code: -1
(`EPERM`, `NoSuchSlot`), -2 (`ENOENT`, `WrongObject`), -5 (`EIO`, `BadMethod`), -7 (`E2BIG`,
`OutOfMemory`), -9 (`EBADF`, `StillAlive`), -12 (`ENOMEM`, `NoResetMechanism`), -13 (`EACCES`,
`ResetNotSupported`). notes/std/fs.md calls it a wart. The overlay resolves -1 in favor of the
server, so a revoked file endpoint reads as `PermissionDenied` instead of `Unsupported`, and the note
says the clean fix is a contract change.

## What else was considered, and why each lost

**(A) Keep Linux `errno` as it is.** Zero cost and familiar to anyone who has read `errno.h`. It
lost because it keeps the overlap, and because its one argument, compatibility, has no consumer: the
numbers stop at `std`. It also ties nife's wire to one host's numbering: macOS has `EPROTO` 100 and
`ENOTSUP` 45 (recalled from the brief, not read here), so "POSIX" does not fix a number.

**(B) nife's own codes, disjoint from the kernel's (recommended).**

**(C) One vocabulary shared with `abi::Error`.** Fuchsia's model, and the closest runner-up: one
table, one decoder. It lost on where the table lives. `abi` is the system call surface, §10 (the
capability-based microkernel process model), which every program is written against; this option
puts file-system meanings into it that the kernel never returns, and ties each new service code to a
kernel ABI review. (B) keeps the single word and the disjointness without that coupling, and merging
the two tables later stays compatible because the ranges are already disjoint.

**(D) Keep the numbers, give them nife names.** Fixes the naming rule (spelled-out names over the
spec abbreviation) and nothing else: the overlap stays, and so does Linux's numbering.

**(E) Per-protocol code sets.** Sockets already work this way, and it suits a small vocabulary whose
outcomes are not failures (`PORT_CLAIM_IN_USE`). It lost as the general rule because the file and
block protocols share most of their codes (`EINVAL`, `EIO`, `EROFS`), `SYNC` passes a block code
through the file service, and every client would carry one map per protocol. (B) does not forbid it:
a protocol may still answer its own outcome words in its results.

**(F) A tag bit instead of a range.** A flag in the word (bit 62) would separate the spaces too. It
lost because it changes the ruled 2026-11-14 shape (`-4095..=-1` is an error) and makes every
decoder test a bit as well as a sign.

## What the tree already does

- `abi::Error`: nife's own enum with spelled-out names, `#[repr(i64)]`, and a decoder
  (`Error::from_ret`) that refuses unknown values. This is the shape (B) asks of the services.
- `socket_contract`: its own per-protocol words, no `errno`.
- `subtree_scope::Refusal`: a nife enum of eight meanings (`NotFound`, `Symlink`, `MountCrossing`)
  that is translated to `errno` at the end (`Refusal::errno`). It already is (B) with an `errno`
  table on the far side.
- `domain_name_resolution_contract::grants::Refusal`, `startup_page_contract::Refusal`,
  `driver_process_contract`'s `WindowRefusal` and `BringUpError`, `jh7110_clock_and_reset_contract::Status`:
  every newer contract defines its own enum. None uses `errno`.
- `c_library_errno`: the one place POSIX is the contract, and it translates from `ErrorKind` by
  meaning.

## Prior art

- **Fuchsia** (read: `zircon/errors.h`, the `zx_channel_call` reference, the FIDL API rubric).
  `zx_status_t` is one space for kernel and services: `ZX_ERR_NOT_FOUND` -25, `ZX_ERR_IO` -40,
  `ZX_ERR_NOT_DIR` -51. The rubric says to use `zx.Status` for errors about kernel objects or IO
  (`fuchsia.io` does) and a purpose-built enum when you control the domain. Where nife differs:
  `zx_channel_call` returns the transport status separately from the reply payload, so Fuchsia never
  has to tell a refused call from a refused request inside one word. nife does, which is why
  disjointness is the part to copy.
- **seL4** (read: `libsel4/include/sel4/errors.h`). Kernel only: `seL4_NoError` 0 through
  `seL4_NotEnoughMemory` 10. Servers define their own; seL4 has no service vocabulary.
- **Redox** (read: `redox_syscall` 0.9.0 `error.rs`). Linux numbers, `Error::mux` writes
  `-errno as usize`, and `Error::demux` reads a word as an error when `-word` is in `1..133`. Redox
  uses one space for kernel and schemes, and it is `errno`'s, because Redox chose POSIX as its
  system call surface. nife did not (§10).
- **Plan 9** (read: `intro(5)`). 9P's `Rerror` carries a string, `ename`, and no number; the string
  is advisory.
- **Linux** (read: `include/linux/err.h`). `MAX_ERRNO` 4095, so a pointer and an error share one
  return value. The source of the 4095 bound kept here.
- **WASI** (read: wasi-libc `wasip1.h`). WASI kept POSIX names and chose its own numbers in a
  `uint16_t`: `__WASI_ERRNO_ACCES` 2, `NOENT` 44, `PERM` 63, `PROTO` 65, `ROFS` 69, and added
  `NOTCAPABLE` 76, a capability system's own answer. The header says some codes exist only for
  alignment with POSIX. The closest precedent: a capability system that refused the host's
  numbering, and translates at its libc.
- **Hubris** (read: `doc/ipc.adoc`). A reply carries a `u32` response code, 0 for success, the rest
  application-defined; the kernel reserves codes with the top 24 bits set for a dead peer, and tells
  servers to number from 1 to stay clear. The same disjoint-range move this recommends.

## What each option costs, measured

For (B). (A) costs nothing; (C) costs the same as (B) plus an `abi` change; (D) is the renames alone.

| site | what changes | measured size |
|---|---|---|
| file server, `redoxfs_server/src/dispatch.rs` | one function from `syscall::error::Error` to a nife code; 3 `reply_err` calls use it | 21 engine codes and 17 synthesized ones into one map; the 66 `Error::new` sites stay engine errors |
| three caretakers | constant names | 22 `reply_err`/`reply_errno` lines, 8 local copies |
| `disk_tree` (crate and component) | constant names | 1 helper, 18 `err(..)` calls, 5 local copies |
| virtio block, `block_driver_roles` | constants for three raw literals and two `reply_err` | 5 sites |
| NVMe data plane | constant names | 5 sites |
| SD card, `designware_mobile_storage` | constant names | 5 sites, 2 local copies |
| `identity_provisioner` | one decode, one `EEXIST` compare | 2 sites, 1 local copy |
| `reboot`, `rm`, `swish` | `SYNC` compare; `rm`'s exit status and two copies; one compare | 5 sites, 4 local copies |
| `subtree_scope` | `Refusal::errno` becomes `Refusal::code` | 8 arms |
| `graphics_contract`, `compositor` | one constant each | 2 |
| `std` overlay | `errors.rs` arms renumbered; `metadata.rs` compare; `reachable.rs` loses the -1 trade | 12 arms, 1 compare, 2 functions |
| `kernel/src/storage_bench.rs`, fixtures, system tests | expected values | the rest of the 53 call lines in 19 files outside the contract |
| `c_library_errno` | nothing | 0 |

Outside the contract, 23 local copies of `errno` constants sit in 12 files; (B) replaces them with
one table. Run time is unchanged: decoding stays one comparison on the sign and one on the bound.

`rm` makes its exit status the `errno` of the first failure (`filesystem_contract::rm::status`), so
under (B) the status becomes a nife code. That was not checked against §301 (exit status and output).

## How reversible it is

Two-way today: nife has no installs, no third-party server or client speaks the file or block
protocol, and nothing is published (no first publish until every review milestone is delivered,
ruled 2026-11-18). It becomes one-way at the first of: a release of `file_system_contract` to
crates.io; a server or client written outside nifeos; a basalt image whose packages outside nife
read these words. After that, renumbering is a protocol version.

The disjoint ranges are the restrictive choice: they can be merged later into one table (C)
compatibly; separating two spaces that shared numbers cannot.

## Would we still choose this if both cost the same

Yes. At equal cost (B) wins outright: it removes a measured collision, it follows every newer
contract in the tree and the naming rule, and it gives up nothing a consumer uses. The real cost
difference runs the other way: (A) is free and (B) touches about 150 lines in some 20 files (an estimate from the table). So the
recommendation is not about effort.

## A concrete example

Before. RedoxFS refuses `OPENDIR` on a file:

```rust
// redoxfs_server/src/dispatch.rs
(result.unwrap_or_else(|e| reply_err(e.errno)), size) // e.errno == 20, word -20
// std overlay, errors.rs
20 => io::const_error!(io::ErrorKind::NotADirectory, "that name is not a directory"),
// and -1 is EPERM or NoSuchSlot, decided by reachable.rs's trade
```

After (names provisional):

```rust
// redoxfs_server/src/dispatch.rs
(result.unwrap_or_else(|e| error::word(engine_code(e))), size) // Code::NotADirectory, word -260
// std overlay, errors.rs
Code::NotADirectory => io::const_error!(io::ErrorKind::NotADirectory, "that name is not a directory"),
// -1 is only ever the kernel's NoSuchSlot; reachable.rs needs no trade
```

## What waits on this

- The names `MAXIMUM_ERRNO` and `EPROTO` in `file_system_contract::error` (#3914). Under (B) they
  become a nife bound and a nife protocol-violation code; under (A) they keep their meaning and only
  the naming question remains. #3914 can land the module rename with both names as they are.
- The ten-plus copied constants in `file_system_contract` (`EPERM`, `EROFS`, `EISDIR`, `ENOTDIR`,
  `ENODATA`, `ERANGE`, `E2BIG`, `ENOSPC`, `ENOTSUP`, `EBADF`), and the local copies above.
- §324 (the names of message and result words) does not wait: it names the positions of result
  words, not the codes in them.

## What was not checked

Whether every engine code is reachable through the file server; the macOS numbers, taken from the
brief; §301's view of an exit status; the fixtures' and system tests' sites one by one (counted in
the total only); and the exact numbers inside `-256..=-4095`, which the naming ruling assigns.
