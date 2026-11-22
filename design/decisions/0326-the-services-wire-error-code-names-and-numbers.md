---
status: PROPOSED
raised: 2026-11-22
---

# 326. The names and numbers of the services' wire error codes

*(The section number and title are provisional until the merge lands them. Every name and number
below is provisional until ruled.)*

Counts are from `nifeos/nife@c9d63ae99`.

## Background

The ruling of 2026-11-22 (UTC) on §325 (the services' wire error vocabulary, #3918): the services
stop sending negated Linux `errno` and send nife's own codes in `-256..=-4095`, the kernel keeps
`-1..=-255`, and each server translates once at its boundary. It left the names and the numbers to
a follow-on ruling, which comes before any code changes. This is that ruling's draft.

§325's table lists 27 `errno` numbers found in the source. Not all of them can reach a reply, and a
few carry a nife meaning that differs from the POSIX one, so a rename number for number would copy
both problems into nife's table.

## The decision

Which codes the services' table holds, the name and number of each, where the table lives, and
which `errno` values map to each.

## Is the premise true

Partly. Seven of the 27 cannot reach the wire, measured by reading which engine files nife
compiles. `redoxfs_server` builds `vendor/redoxfs` with `default-features = false`, and
`mount/redox/` and `disk/ring.rs` build only for `target_os = "redox"`, so neither is in nife's
build:

- `EACCES`, `EBUSY` and `EBADFD` appear only under `mount/redox/`; `ENOMEM` only there and in
  `disk/ring.rs`. No nife site writes any of them.
- `ENOKEY` and `EKEYREJECTED` come only from `FileSystem::open` (`filesystem.rs`), which runs when
  the server mounts its volume, before it answers any request.
- `ENOSYS` comes only from `create_reserved_with_uuid`, which formats a volume (`mkfs`).

Two more carry a nife meaning by number that differs by site. The engine's `ERANGE`
(`transaction.rs`, three sites) is a record offset past what a node can address, while nife's
`ERANGE` is an attribute name it refuses. `EROFS` is mostly a capability without the mutating right
(`grant.rs`, `disk_tree`), and on the SD card and NVMe servers a read-only device.

## Proposal

- One code per meaning that can reach a reply: 19 codes. A name says what went wrong, spelled out.
  It takes `io::ErrorKind`'s spelling where the kind means the same thing, per the ruling of
  2026-11-03 on standard library names.
- Each code carries its POSIX name as `#[doc(alias = "...")]`, a spec-term alias the ruling of
  2026-11-03 on old nife names keeps.
- Numbers start at -256 and run down without gaps in the order of the table. The space past the
  last code, down to -4095, is for codes added later. Each takes the next free number by ruling,
  with no per-protocol ranges. A retired number is never reused.
- The table lives in a new leaf crate (provisional name `service_error`), one `#[repr(i64)] enum
  Code` that every contract depends on. Putting it in `file_system_contract` would make
  `graphics_contract` and the compositor depend on the file service for one constant. Rule 7 asks
  for a shared crate, and a leaf keeps it small.
- `Code::from_word` and `Code::word` replace `error::code` and `error::word`, and the bounds
  `FIRST_SERVICE_CODE` (-256) and `LAST_SERVICE_CODE` (-4095) replace `MAXIMUM_ERROR_NUMBER`, the
  names ruled on 2026-11-22. The kernel's `abi::Error` decodes with `from_ret`; this follows the
  contracts' own word.
- `from_word` answers one of three things, and only the first has a number: a `Code`. It answers
  `Unrecognized` for an unknown number in the range, so a code added later reaches an older client
  as a failure it cannot name. It answers `ProtocolViolation` for a word below -4095 (the rule of
  2026-11-14, block 1090, decision C). Both sit outside the `#[repr(i64)]` enum, since no server
  sends either.

| word | name | `doc(alias)` | meaning | `std` `ErrorKind` |
|---:|---|---|---|---|
| -256 | `PermissionDenied` | `EPERM` | the capability does not carry that right | `PermissionDenied` |
| -257 | `NotFound` | `ENOENT` | no such name | `NotFound` |
| -258 | `InputOutputError` | `EIO` | the device or the engine failed under the request | `InputOutputError` |
| -259 | `ValueTooLarge` | `E2BIG` | an attribute value over the stored bound | `Uncategorized` |
| -260 | `BadHandle` | `EBADF` | a handle or a verb this capability never minted | `Uncategorized` |
| -261 | `AlreadyExists` | `EEXIST` | another node holds the name | `AlreadyExists` |
| -262 | `CrossesDevices` | `EXDEV` | the request crosses a mount point | `CrossesDevices` |
| -263 | `NotADirectory` | `ENOTDIR` | the request needs a directory | `NotADirectory` |
| -264 | `IsADirectory` | `EISDIR` | the request needs a file | `IsADirectory` |
| -265 | `InvalidInput` | `EINVAL` | a request the protocol cannot resolve | `InvalidInput` |
| -266 | `TooManyOpenFiles` | `EMFILE` | as many handles open as this client may hold | `TooManyOpenFiles` |
| -267 | `FileTooLarge` | `EFBIG` | an offset or size past what a file can hold | `FileTooLarge` |
| -268 | `StorageFull` | `ENOSPC` | no space left | `StorageFull` |
| -269 | `ReadOnlyFilesystem` | `EROFS` | read-only through this capability, or on this device | `ReadOnlyFilesystem` |
| -270 | `InvalidAttributeName` | `ERANGE` | an attribute name empty, too long, or holding a zero byte | `Uncategorized` |
| -271 | `Unsupported` | `ENOTSUP`, `EOPNOTSUPP` | this server or capability does not do that | `Unsupported` |
| -272 | `DirectoryNotEmpty` | `ENOTEMPTY` | the directory still has entries | `DirectoryNotEmpty` |
| -273 | `FilesystemLoop` | `ELOOP` | a symbolic link, which a grant never follows | `FilesystemLoop` |
| -274 | `AttributeNotFound` | `ENODATA` | the named attribute is not set | `Uncategorized` |

The `std` column is what the standard library answers for that `errno` on Linux. The overlay
chooses its own mapping and may do better (`AttributeNotFound` as `NotFound`, for one).

## The map from each `errno`

Every number maps by itself except `ERANGE`, which maps by origin.

| `errno` | code | | `errno` | code |
|---|---:|---|---|---:|
| 1 `EPERM` | -256 | | 27 `EFBIG` | -267 |
| 2 `ENOENT` | -257 | | 28 `ENOSPC` | -268 |
| 5 `EIO` | -258 | | 30 `EROFS` | -269 |
| 7 `E2BIG` | -259 | | 34 `ERANGE`, nife's | -270 |
| 9 `EBADF` | -260 | | 34 `ERANGE`, the engine's | -267 |
| 17 `EEXIST` | -261 | | 39 `ENOTEMPTY` | -272 |
| 18 `EXDEV` | -262 | | 40 `ELOOP` | -273 |
| 20 `ENOTDIR` | -263 | | 61 `ENODATA` | -274 |
| 21 `EISDIR` | -264 | | 95 `ENOTSUP` / `EOPNOTSUPP` | -271 |
| 22 `EINVAL` | -265 | | 71 `EPROTO` | none: `ProtocolViolation` |
| 24 `EMFILE` | -266 | | | |

Left out, with no number: `EACCES`, `EBUSY`, `EBADFD`, `ENOMEM`, `ENOSYS`, `ENOKEY` and
`EKEYREJECTED`, because none can reach a reply (above). Giving a number to a code nothing sends is
the loose choice; adding one when it can be sent stays compatible. The file server's one translating
function maps any engine number without a row to `InputOutputError`. That fallback is a foot gun: a
vendor update that adds an engine code reaches clients as `InputOutputError` until someone adds the
row. The proposal is that a vendor update's review checks the engine's codes against this map.

## The choices inside it

These are the places where the proposal is more than a rename.

1. Seven numbers get no code, since none can reach a reply.
2. The engine's `ERANGE` becomes `FileTooLarge` and nife's becomes `InvalidAttributeName`: one
   number, two meanings, mapped by meaning.
3. `EROFS` stays one code, `ReadOnlyFilesystem`, for a capability without the mutating right and for
   a read-only device. `grant.rs` chose it over a permission answer on purpose: no policy could have
   said yes, the capability carries one direction, so the answer is about the thing. A code of its
   own for the capability case would split what `std` callers already read as one kind.
4. `EBADF` stays one code. `fs_file_caretaker` refuses a forged handle and a forged opcode with one
   number at one site on purpose, and `verb/tests.rs` pins that. Other servers answer a verb they
   do not carry with `EINVAL` (`fs_subtree_caretaker`, `fs_nameset_caretaker`, NVMe, SD card, virtio
   block, `graphics_contract`, the compositor), each by its own reasoning. Moving every unknown
   verb to `Unsupported` would overturn those choices and change the tests that pin -22
   (`compositor/tests.rs`, `graphics_contract`). That is its own question; this table renames each
   site's answer as it stands.
5. `ENOTSUP` and `EOPNOTSUPP` are one number on Linux and one code here, `Unsupported`, which
   already covers "this capability does not carry extended attributes" (`directory.rs`).
6. `ReadOnlyFilesystem` and `FilesystemLoop` keep `std`'s one-word "Filesystem", though the
   module spells `file_system`. The standard library rule makes `std`'s spelling win where the
   meaning matches, and a reader moving between `io::ErrorKind` and `Code` meets one spelling.
   `FilesystemLoop` matches by meaning too: `std` reports an `O_NOFOLLOW` refusal, the case
   `subtree_scope`'s `Symlink` copies, as `FilesystemLoop`.
7. `InputOutputError` and `TooManyOpenFiles` are `std`'s spellings, so they win over shorter nife
   ones.

## What else was considered

- Every number in §325's table, 27 codes. It numbers codes that nothing can send, which are then
  part of the contract forever.
- Folding every unknown verb into `Unsupported`. Item 4 above leaves it for its own ruling.
- `-256 - errno` as the number. Easier beside a Linux trace, but it keeps a numbering nife did not
  choose; §325 refused that as option (G).
- Grouped ranges (files from -256, attributes from -512). They fix categories before there are
  enough codes to need them, and a range can fill.
- The enum in `file_system_contract`. Simpler today, but it makes graphics depend on the file
  service.

## Prior art

- Rust `std` (read: the pinned toolchain's `std/src/sys/io/error/unix.rs`). Folds `EACCES` with
  `EPERM` and `ENOTSUP` with `ENOSYS`; maps `EMFILE` to `TooManyOpenFiles` and `EIO` to
  `InputOutputError`, the spellings taken here.
- Fuchsia (read for §325: `zircon/errors.h`). Its own numbers and names by meaning, one table.
- WASI (read for §325: wasi-libc `wasip1.h`). POSIX names on its own numbers, and its own code
  `NOTCAPABLE`, which is what `PermissionDenied` means here.

## What it costs

At `c9d63ae99`, 192 production code lines in 54 files name an `errno` constant or call
`reply_err`, comments excluded, by §325's pattern restricted to non-test files. Add the bare
numbers (seven `reply_err(22)`-style calls in components, `block_driver_roles`' raw literals,
`subtree_scope`'s three) and the tests, about 490 lines by §325's estimate. Nearly every change is a
constant renamed; the new work is the leaf crate and the file server's one function.

## Who has acted on it

Nobody outside this tree. #3914 renames the module and keeps the `errno` constants as they are.
§325's example writes `Code::NotADirectory` and its word, marked provisional. No code uses any name
here.

## Would we still choose this if both cost the same

Yes. The cheapest table renames all 27 numbers in one pass, and this one costs a little more: a
reachability test, one map by origin and a leaf crate. Each of those is on the merits. Leaving out
unreachable codes is the restrictive choice, and keeping `EBADF` and `EINVAL` as they are avoids
deciding a second question inside a naming ruling.

## How reversible it is

Two-way until §325 says it is not: no installs, nothing published, and no server outside nifeos. A
code left out can be added later, which older clients read as `Unrecognized`.

## A concrete example

```rust
#[repr(i64)]
pub enum Code {
    #[doc(alias = "EPERM")]
    PermissionDenied = -256,
    #[doc(alias = "ENOENT")]
    NotFound = -257,
    // ...
    #[doc(alias = "ENOTDIR")]
    NotADirectory = -263,
}
```

## What was not checked

Whether each reachable engine code can arrive through every verb. Each `EINVAL` site's reasoning,
beyond those named in item 4. Whether the overlay should map more kinds than `std` does.
