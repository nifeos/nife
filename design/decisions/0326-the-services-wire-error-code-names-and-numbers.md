---
status: DECIDED
raised: 2026-11-22
decided: 2026-11-22
ratified_by: calef
---

# 326. The names and numbers of the services' wire error codes

*(The section number and title are provisional until the merge lands them.)*

Counts are from `nifeos/nife@c9d63ae99`.

## Background

The ruling of 2026-11-22 (UTC) on §325 (the services' wire error vocabulary, #3918): the services
stop sending negated Linux `errno` and send nife's own codes in `-256..=-4095`, the kernel keeps
`-1..=-255`, and each server translates once at its boundary. It left the names and the numbers to
a follow-on ruling, which comes before any code changes. This is that ruling.

§325's table lists 27 `errno` numbers found in the source. Not all of them can reach a reply, and a
few carry a nife meaning that differs from the POSIX one, so a rename number for number would copy
both problems into nife's table.

## The decision

Ruled 2026-11-22 (UTC): the 19-code table below, as drafted, with the map from each `errno`, the
leaf crate and the decoder's three outcomes. The names `service_error` and `Decoded` stay
provisional. Whether an unknown verb answers `Unsupported` is its own decision.

## Is the premise true

Partly. Seven of the 27 cannot reach the wire, and an eighth source cannot either, measured by
reading which engine files nife compiles. `redoxfs_server` builds `vendor/redoxfs` with
`default-features = false`, and `mount/redox/` and `disk/ring.rs` build only for
`target_os = "redox"`, so neither is in nife's build:

- `EACCES`, `EBUSY` and `EBADFD` appear only under `mount/redox/`; `ENOMEM` only there and in
  `disk/ring.rs`. No nife site writes any of them.
- `ENOKEY` and `EKEYREJECTED` come only from `FileSystem::open` (`filesystem.rs`), which runs when
  the server mounts its volume, before it answers any request.
- `ENOSYS` comes only from `create_reserved_with_uuid`, which formats a volume (`mkfs`).
- The engine's `ERANGE` (`transaction.rs`, three sites) is a record offset past what a node can
  address. The server answers `EFBIG` first for any end past `MAXIMUM_FILE_END`
  (`redoxfs_server/src/lib.rs`, checked in `server.rs` before the engine runs), so it never fires.
  `ERANGE` keeps its code for nife's meaning, which is the same `syscall::Error` 34, so a map by
  origin could not tell the two apart anyway.

`EROFS` carries two meanings: mostly a capability without the mutating right (`grant.rs`,
`disk_tree`), and on the SD card and NVMe servers a read-only device.

## The rule

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
- `Code::from_word` returns `Decoded` (provisional name), an enum of three outcomes, and only the
  first has a number. `Decoded::Code(Code)` is a known code. `Decoded::Unrecognized(i64)` is an
  unknown number in the range, so a code added later reaches an older client as a failure it cannot
  name. `Decoded::ProtocolViolation` is a word below -4095 (the rule of 2026-11-14, block 1090,
  decision C). The last two sit outside the `#[repr(i64)]` enum, since no server sends either.

| word | name | `doc(alias)` | meaning | `std` `ErrorKind` |
|---:|---|---|---|---|
| -256 | `PermissionDenied` | `EPERM` | the capability does not carry that right | `PermissionDenied` |
| -257 | `NotFound` | `ENOENT` | no such name | `NotFound` |
| -258 | `InputOutputError` | `EIO` | the device or the engine failed under the request | `InputOutputError` |
| -259 | `ValueTooLarge` | `E2BIG` | an attribute value over the stored bound | `ArgumentListTooLong` |
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
| -270 | `InvalidAttributeName` | `ERANGE` | an attribute name refused, or a value buffer too short | `Uncategorized` |
| -271 | `Unsupported` | `ENOTSUP`, `EOPNOTSUPP` | this server or capability does not do that | `Unsupported` |
| -272 | `DirectoryNotEmpty` | `ENOTEMPTY` | the directory still has entries | `DirectoryNotEmpty` |
| -273 | `FilesystemLoop` | `ELOOP` | a symbolic link, which a grant never follows | `FilesystemLoop` |
| -274 | `AttributeNotFound` | `ENODATA` | the named attribute is not set | `Uncategorized` |

The `std` column is what the standard library answers for that `errno` on Linux. The overlay
chooses its own mapping and may do better (`AttributeNotFound` as `NotFound`, for one).

## The map from each `errno`

Every number maps by itself.

| `errno` | code | | `errno` | code |
|---|---:|---|---|---:|
| 1 `EPERM` | -256 | | 27 `EFBIG` | -267 |
| 2 `ENOENT` | -257 | | 28 `ENOSPC` | -268 |
| 5 `EIO` | -258 | | 30 `EROFS` | -269 |
| 7 `E2BIG` | -259 | | 34 `ERANGE` | -270 |
| 9 `EBADF` | -260 | | 39 `ENOTEMPTY` | -272 |
| 17 `EEXIST` | -261 | | 40 `ELOOP` | -273 |
| 18 `EXDEV` | -262 | | 61 `ENODATA` | -274 |
| 20 `ENOTDIR` | -263 | | 95 `ENOTSUP` / `EOPNOTSUPP` | -271 |
| 21 `EISDIR` | -264 | | 71 `EPROTO` | none: `ProtocolViolation` |
| 22 `EINVAL` | -265 | | 24 `EMFILE` | -266 |

Left out, with no number: `EACCES`, `EBUSY`, `EBADFD`, `ENOMEM`, `ENOSYS`, `ENOKEY` and
`EKEYREJECTED`, because none can reach a reply (above), nor the engine's `ERANGE`. Giving a number
to a code nothing sends is the loose choice; adding one when it can be sent stays compatible.

The file server's one translating function needs an answer for an engine number with no row, and
`InputOutputError` would hide a new engine code. A test lists every `Error::new(E...)` code in the
engine files nife compiles. It fails when one has no row here or in the left-out list above, so a
vendor update that adds a code fails the test before it reaches a client.

## The choices inside it

These are the places where the rule is more than a rename.

1. Seven numbers, and the engine's use of `ERANGE`, get no code, since none can reach a reply.
2. `ERANGE` becomes `InvalidAttributeName`, nife's meaning, at two sites: a name the server
   refuses (`dispatch.rs`, `server.rs`) and a value buffer too short for the value
   (`server.rs`, `get_xattr`). The second cannot fire on the wire: the reply window is 4,096 bytes
   and a stored value is at most 3,072. The name is for the first.
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

- Rust `std` (read: the pinned toolchain's `std/src/sys/io/error/unix.rs`). Maps `EACCES` and
  `EPERM` both to `PermissionDenied`, and `ENOTSUP` and `ENOSYS` both to `Unsupported`. Maps
  `EMFILE` to `TooManyOpenFiles` and `EIO` to `InputOutputError`, the spellings taken here.
- Fuchsia (read for §325: `zircon/errors.h`). Its own numbers and names by meaning, one table.
- WASI (read for §325: wasi-libc `wasip1.h`). POSIX names on its own numbers, and its own code
  `NOTCAPABLE`, which is what `PermissionDenied` means here.

## What it costs

At `c9d63ae99`, 243 code lines in 59 files match the pattern `R` of §325's cost count, by this
command:

```sh
git grep -hP "$R" c9d63ae99 -- '*.rs' ':!vendor' ':!c_library' ':!crates/c_library_errno' \
  ':!notes' ':!bench' ':!tools' ':!fuzz' ':!fixtures' ':!system_tests' \
  ':!crates/filesystem_fixture' ':!**/tests.rs' ':!**/tests/**' ':!**/test_*.rs' \
  ':!**/falsifications/**' | grep -vE '^\s*(//|\*)' \
  | grep -cvE 'Error::new\(|syscall::error|^\s*use '
```

The command counts lines; 59 files hold at least one counted line. A re-count with other test
exclusions found 49 files; which test-only trees count moves the figure
by about ten files, not its order. Add the bare
numbers (seven `reply_err(22)`-style calls in components, `block_driver_roles`' raw literals,
`subtree_scope`'s three) and the tests, about 490 lines by §325's estimate. Nearly every change is a
constant renamed; the new work is the leaf crate, the file server's one function and its test.

## Who has acted on it

Nobody outside this tree. #3914 renames the module and keeps the `errno` constants as they are.
§325's example writes `Code::NotADirectory` and its word, marked provisional. No code uses any name
here.

## Would we still choose this if both cost the same

Yes. The cheapest table renames all 27 numbers in one pass, and this one costs a little more: a
reachability test and a leaf crate. Each of those is on the merits. Leaving out
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
