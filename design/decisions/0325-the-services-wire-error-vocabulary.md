---
status: DECIDED
raised: 2026-11-22
decided: 2026-11-22
ratified_by: calef
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
(`reply.rs`; #3914 renames it `file_system_contract::error`, with `error::word(code)` and
`error::code(word)`) passes those numbers through. The block, NVMe and SD card servers then
copied the convention, and so did the graphics contract.

This matters now for two reasons. The two spaces overlap, so one word can mean two things (below),
though only one collision is live today. And #3914 is about to rename `MAXIMUM_ERRNO` and `EPROTO`,
and needs to know whether their vocabulary stays.

## The decision

Ruled 2026-11-22 (UTC): option (B), nife's own codes in a range disjoint from the kernel's.

- A reply's error word is a nife code. One table names every code a service may send, by meaning,
  with spelled-out names (`NotFound`, `ReadOnlyFilesystem`, `NotADirectory`; the names and
  numbers are a follow-on naming ruling, which assigns them before any code changes).
- This decision fixes the two ranges now and keeps them disjoint. The kernel keeps `-1..=-255` (it
  uses 15). Services use `-256..=-4095`. A word below -4095 stays a protocol violation, the rule of
  2026-11-14 (block 1090, decision C), so the bound 4095 survives with a nife name.
- Each server translates once, at its boundary: the file server maps `syscall::error::Error` to a
  nife code in one function, and nothing below that boundary changes.
- `errno` stays where POSIX is the contract: the C library's `errno` (`c_library_errno`), which is
  already translated from `io::ErrorKind` and never reads the wire.
- The word carries a number, not a string (option (I) below).
- Optional detail text beside a code is not decided. It stays a possible later addition, and a
  compatible one, since the code alone keeps its meaning.

The reason is on the merits; (B) is the dearer option. One collision is live (-1) and six are
latent. (B) removes them by construction and names the codes by nife's meanings, as every newer
contract does. An offset (G) fixes the collision more cheaply, but (G) now and (B) later renumbers
twice.

## Is the premise true

Yes, with one correction. The services do speak Linux numbers (`redox_syscall` 0.9.0's `error.rs`),
but more of them than `file_system_contract` names: 19 more, and `block_driver_roles` writes raw
`-95`, `-5` and `-22`.

## Every code in use on any wire

Command: `git grep` for `reply_err(`, `Error::new(`, `const E[A-Z]+: i32`, and raw negative literals
in `components/`, `crates/`, `redoxfs_server/`; engine codes by
`grep -rhoE "Error::new\(([a-z_]+::)*E[A-Z]+\)" vendor/redoxfs/src`.

The file service (`fs` protocol). Synthesized by nife: the file server (`redoxfs_server`), the
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

That is 27 numbers. The overlay (`sys/fs/nife/errors.rs`) has arms for ten of them, which give nine
kinds (9 and 22 are both `InvalidInput`); the other 17 are `Other`. That is the overlay's mapping
choice, and no wire vocabulary changes it on its own. Whether every engine code is reachable through
the server was not checked.

The block protocol (`filesystem_contract::block`), all synthesized: virtio block
(`block_driver_roles`) `EINVAL` 22, `EIO` 5, `EOPNOTSUPP` 95; NVMe
(`non_volatile_memory_express_data_plane`) and SD card (`designware_mobile_storage`) `EINVAL` 22,
`EROFS` 30, `EIO` 5. The file server, the one production client, turns every negative block reply
into `EIO` except `SYNC`'s, which it passes through.

The graphics protocol: `graphics_contract::EINVAL` and `compositor`'s `EBADOP`, both -22.

Sockets (`socket_contract`): no `errno`, only per-protocol outcomes (`CONNECT_REFUSED` 1) and a
positive sentinel, `REPLY_ERROR` = `1 << 32`.

### The overlap, measured

`abi::Error` uses -1 to -15. Seven numbers in the table share a word with a kernel code: -1
(`EPERM`, `NoSuchSlot`), -2 (`ENOENT`, `WrongObject`), -5 (`EIO`, `BadMethod`), -7 (`E2BIG`,
`OutOfMemory`), -9 (`EBADF`, `StillAlive`), -12 (`ENOMEM`, `NoResetMechanism`), -13 (`EACCES`,
`ResetNotSupported`).

A client meets the kernel's codes on a `CALL` only. `rendezvous_call`
(`kernel/src/system_call/rendezvous.rs`) answers `NotPermitted` -3 without `WRITE`.
`aborted_send_error` answers -3 for an endpoint bound to an interrupt and `Gone` -11 for one stale
or revoked; a server that dies holding the call also answers `Gone` (`strand_reply_caller`).
`invoke` answers `NoSuchSlot` -1 for an empty slot. A slot holding another object runs that
object's method 4 instead. Neither -3 nor -11 is a service number.

So one collision is live: -1, an empty slot or the server's `EPERM`. The overlay resolves it for the
server (`reachable.rs`). That is safe only because every entry point probes reachability first, and
the probe still guards -1, -2 and -3. Codes -2 and -5 could come back only from a misbuilt spawn.
-7, -9, -12 and -13 are latent: no `CALL` answers them, but a new refusal there would collide
silently.

## What else was considered, and why each lost

**(A) Keep Linux `errno` as it is.** Zero cost and familiar to anyone who has read `errno.h`. It
lost because it keeps the overlap, and because its one argument, compatibility, has one consumer:
`rm` makes its exit status the raw `errno`. `std` maps by meaning and the C library maps again from
`ErrorKind`. It also ties the wire to one host's numbering (macOS's `EPROTO` is 100, recalled).

**(B) nife's own codes, disjoint from the kernel's (chosen).**

**(C) One vocabulary shared with `abi::Error`.** Fuchsia's model: one table, one
decoder. It lost on where the table lives: `abi` is the system call surface, §10 (process model:
capability-based, microkernel), and this puts file-system meanings the kernel never returns into
it, so each new service code needs a kernel ABI review.

**(D) Keep the numbers, give them nife names.** Fixes the naming rule and nothing else: the
overlap stays.

**(E) Per-protocol code sets.** Sockets already work this way. It lost as the general rule because
the file and block protocols share most codes and `SYNC` passes a block code through the file
service. (B) still lets a protocol answer its own outcome words in its results.

**(F) A tag bit instead of a range.** A flag in the word (bit 62) would separate the spaces too. It
lost because it changes the ruled 2026-11-14 shape (`-4095..=-1` is an error) and makes every
decoder test a bit as well as a sign.

**(G) An offset: servers send `-(256 + errno)`.** The other fix notes/std/fs.md names. It separates
the spaces as (B) does and keeps every constant: only `reply_err`, `reply_errno`, the three raw
literals and the tests' expected words change. It is the cheapest fix of the collision, and the
strongest rival. It can grow nife's own codes above the Linux ones. It lost on one point: it keeps
Linux's numbering as the base of the table, so a later move to nife's own numbering, which the
naming rule and the newer contracts point to, renumbers twice. Taking (B) once avoids that.

**(H) Move the kernel's codes out of the services' way.** Renumber `abi::Error` to, say,
`-200..=-215`, above Redox's 133 numbers; the services change nothing. It lost because it moves the
system call surface (§10) that every program depends on, to protect a vocabulary nobody chose, and
leaves that vocabulary as it is.

**(I) Strings instead of numbers.** Refused. Programs would compare text to decide what failed. A
string needs the shared page on every failure, where a code fits in the reply word. Free text can
carry across a trust boundary what the client was not granted, such as a name outside its
directory. And neither the compiler nor Kani can check text the way it checks an enumeration.

## What the tree already does

`abi::Error` is nife's own enum with spelled-out names and a decoder (`Error::from_ret`) that
refuses unknown values: the shape (B) asks of the services. Every newer contract defines its own
enum and none uses `errno`: `socket_contract`, `startup_page_contract::Refusal`,
`driver_process_contract`'s `WindowRefusal`, `jh7110_clock_and_reset_contract::Status`.
`subtree_scope::Refusal` is (B) already, with an `errno` table on the far side
(`Refusal::errno`).

## Prior art

- Fuchsia (read: `zircon/errors.h`, the `zx_channel_call` reference, the FIDL API rubric).
  `zx_status_t` is one space for kernel and services: `ZX_ERR_NOT_FOUND` -25, `ZX_ERR_IO` -40,
  `ZX_ERR_NOT_DIR` -51. The rubric says to use `zx.Status` for kernel objects or IO and a
  purpose-built enum when you control the domain. Where nife differs:
  `zx_channel_call` returns the transport status apart from the reply, so Fuchsia never tells a
  refused call from a refused request inside one word. nife must, so disjointness is what to copy.
- seL4 (read: `libsel4/include/sel4/errors.h`). Kernel only: `seL4_NoError` 0 through
  `seL4_NotEnoughMemory` 10. Servers define their own; seL4 has no service vocabulary.
- Redox (read: `redox_syscall` 0.9.0 `error.rs`). Linux numbers; `Error::mux` writes
  `-errno as usize`, and `Error::demux` reads `-word` in `1..133` as an error. One space for kernel
  and schemes, `errno`'s, because Redox chose POSIX as its system call surface. nife did not (§10).
- Plan 9 (read: `intro(5)`). 9P's `Rerror` carries a string, `ename`, and no number.
- Linux (read: `include/linux/err.h`). `MAX_ERRNO` 4095, so a pointer and an error share one
  return value. The source of the 4095 bound kept here.
- WASI (read: wasi-libc `wasip1.h`). WASI kept POSIX names and chose its own numbers in a
  `uint16_t` (`__WASI_ERRNO_NOENT` 44, `ROFS` 69), and added `NOTCAPABLE` 76, a capability
  system's own answer. The closest precedent: it refused the host's numbering and translates at its
  libc.
- Hubris (read: `doc/ipc.adoc`). A reply's `u32` code is 0 for success, the rest the server's; the
  kernel reserves codes with the top 24 bits set for a dead peer. The disjoint-range move this
  recommends.

## What each option costs, measured

For (B). (A) costs nothing; (C) costs (B) plus an `abi` change; (D) is the renames alone; (G) is
the reply helpers, three literals and expected words; (H) is `abi::Error`, its decoder and the
overlay's checks.

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
| `filesystem_contract` itself | `fs.rs`, `verb.rs`, `directory.rs`, `extended_attribute.rs`, `reply.rs`, their tests and falsification patches | the largest share, below |
| `gpu_driver`, `framebuffer_driver` | `gfx::EINVAL` | 6 sites |
| overlay `path_request.rs`, `path_operations.rs` | the `EINVAL` fallback; the mtime decode | 2 functions |
| `kernel/src/storage_bench.rs`, fixtures, system tests | expected values | below |
| prose naming an `errno` | notes | 150 Markdown files |
| `c_library_errno`, `c_library` | nothing | 0 |

The rows are a floor. This count is an estimate, not a bound: it catches comments and engine-side
lines that would not change, and misses bare numbers (the overlay's numbered arms, `reachable.rs`'s
`-1 | -2 | -3`, `block_driver_roles`' raw numbers):

```sh
R='\bE(PERM|NOENT|IO|2BIG|BADF|NOMEM|ACCE[S]|BUSY|EXIST|XDEV|NOTDIR|ISDIR|INVAL|MFILE|FBIG|NOSPC|ROFS|RANGE|NOSYS|NOTEMPTY|LOOP|NODATA|PROTO|BADFD|NOTSUP|OPNOTSUPP|NOKEY|KEYREJECTED|BADOP)\b|reply_errno?\(|MAXIMUM_ERRNO|\berrno\b'
git grep -hP "$R" -- '*.rs' '*.patch' ':!vendor' ':!c_library' ':!crates/c_library_errno' \
  ':!notes' ':!bench' ':!tools/redoxfs_host' | grep -cvE 'Error::new\(|syscall::error|^\s*use '
```

At 0150291fd that finds 1,077 lines in 113 files: 488 in 59 production files outside the file
server, 98 in 10 file server files, and 491 in 44 test, fixture and falsification files. The real
change is some hundreds of production lines plus the tests, nearly all a constant renamed. Run time
stays the same: decoding is one comparison on the sign and one on the bound.

`rm` makes its exit status the raw `errno` of the first failure (`filesystem_contract::rm::status`),
the one place a number leaves the wire unmapped. Under (B) it needs a map to a status of its own;
that was not checked against §301 (exit status and output).

## How reversible it is

Two-way today: nife has no installs, no third-party server or client speaks the file or block
protocol, and nothing is on crates.io (the ruling of 2026-11-18 holds every first publish until the
review milestones deliver). It becomes one-way at the first of: a release of `file_system_contract`
to crates.io; a server or client written outside nifeos; a basalt image whose packages outside nife
read these words. After that, renumbering is a protocol version.

The disjoint ranges are the restrictive choice: a later decision can merge them into one table (C)
compatibly; separating two spaces that shared numbers cannot.

## Would we still choose this if both cost the same

Yes, against each rival. At equal cost (B) beats (A) and (D) because it removes the collision and
(H) because it leaves the system call surface alone. Against (G) the margin is thin: both separate
the spaces, and (B) wins only by numbering from nife's own table, as the newer contracts and the
naming rule do, so the table is not renumbered a second time. The real costs run the other way: (A)
is free, (G) and (H) are small, and (B) is the hundreds of lines above. So the choice is not
about effort.

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
(result.unwrap_or_else(|e| engine_code(e).word()), size) // Code::NotADirectory, word -263
// std overlay, errors.rs
Code::NotADirectory => io::const_error!(io::ErrorKind::NotADirectory, "that name is not a directory"),
// -1 is only ever the kernel's NoSuchSlot; reachable.rs needs no trade
```

## What waits on this

- The follow-on naming ruling: one spelled-out name per meaning, numbers from -256, and the map
  from each of the 27 `errno` values. Its draft is #3923.
- The names `MAXIMUM_ERRNO` and `EPROTO` in `file_system_contract::error` (#3914) become a nife
  bound and a nife protocol-violation code. #3914 can land the module rename with both as they are.
- The constants `file_system_contract` copies (`EPERM`, `EROFS`, `ENOTDIR` and the rest).
- §324 (the names of message and result words) does not wait: it names the positions of result
  words, not the codes in them.

## Stale records this found

`reachable.rs` (`no_capability`) and notes/std/fs.md ("A wart of the contract") give the kernel's
range as -1..-8; it is -1..-15. notes/std/fs.md also says a revoked endpoint reads as
`PermissionDenied`; revocation answers `Gone` -11, which the overlay reads as `Other`. The lane that
builds this ruling corrects both.

## What was not checked

Whether every engine code can reach the wire through the file server. What every object's method 4
answers when a misbuilt spawn puts it in a service slot. The
fixtures' and system tests' sites one by one, counted in the total only. And the exact numbers
inside `-256..=-4095`, which the naming ruling assigns.