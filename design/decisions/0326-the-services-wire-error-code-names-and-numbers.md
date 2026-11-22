---
status: PROPOSED
raised: 2026-11-22
---

# 326. The names and numbers of the services' wire error codes

*(The section number and title are provisional until the merge lands them. Every name and number
below is provisional until ruled.)*

## Background

The ruling of 2026-11-22 (UTC) on §325 (the services' wire error vocabulary, #3918): the services
stop sending negated Linux `errno` and send nife's own codes in `-256..=-4095`, the kernel keeps
`-1..=-255`, and each server translates once at its boundary. It left the names and the numbers to
a follow-on ruling, which comes before any code changes. This is that ruling's draft.

The services send 27 `errno` numbers today (the table in §325). Some of them carry one meaning
under two numbers, and one number carries two meanings, so a one-to-one rename would copy POSIX's
history into nife's table. The proposal below gives one code per meaning instead.

## The decision

Which codes the services' table holds, the name and number of each, and which
`errno` values map to each.

## Proposal

- One code per meaning. A name says what went wrong, spelled out, and takes the standard library's
  `io::ErrorKind` spelling where that kind means the same thing (the ruling of 2026-11-03 on
  standard library names).
- Each code carries its POSIX names as `#[doc(alias = "...")]`, so a reader who knows `ENOENT` finds
  `NotFound`. These are spec-term aliases, which the ruling of 2026-11-03 on old nife names keeps.
- Numbers start at -256 and run down without gaps, in the order of the table. A new code takes the
  next number below the last one in use, and a retired code's number is never reused.
- The table is one enum, `file_system_contract::error::Code`, `#[repr(i64)]`, with
  `Code::from_word` and `Code::word`, in the shape of `abi::Error`.
- An unknown number inside `-256..=-4095` decodes as a failure the client cannot name
  (`Unrecognized`), not as a violation, so a code added later reaches an older client as a failure.
  A word below -4095 stays a protocol violation (the rule of 2026-11-14, block 1090, decision C).
- Two bounds replace `MAXIMUM_ERRNO`: `FIRST_SERVICE_CODE` (-256) and `LAST_SERVICE_CODE` (-4095).

| word | name | `doc(alias)` | meaning | `std` `ErrorKind` |
|---:|---|---|---|---|
| -256 | `PermissionDenied` | `EPERM`, `EACCES` | the capability or the node does not allow this | `PermissionDenied` |
| -257 | `NotFound` | `ENOENT` | no such name | `NotFound` |
| -258 | `InputOutputFailed` | `EIO` | the device or the engine failed under the request | `Other` |
| -259 | `ValueTooLarge` | `E2BIG` | an attribute value over the stored bound | `InvalidInput` |
| -260 | `BadHandle` | `EBADF`, `EBADFD` | a handle this server did not mint, or one that cannot do this now | `InvalidInput` |
| -261 | `OutOfMemory` | `ENOMEM` | the server ran out of memory | `OutOfMemory` |
| -262 | `ResourceBusy` | `EBUSY` | the node is in use | `ResourceBusy` |
| -263 | `AlreadyExists` | `EEXIST` | another node holds the name | `AlreadyExists` |
| -264 | `CrossesDevices` | `EXDEV` | the request crosses a mount point | `CrossesDevices` |
| -265 | `NotADirectory` | `ENOTDIR` | the request needs a directory | `NotADirectory` |
| -266 | `IsADirectory` | `EISDIR` | the request needs a file | `IsADirectory` |
| -267 | `InvalidInput` | `EINVAL` | an argument the protocol refuses | `InvalidInput` |
| -268 | `TooManyHandles` | `EMFILE` | as many handles open as this client may hold | `Other` |
| -269 | `FileTooLarge` | `EFBIG` | a size over the file bound | `FileTooLarge` |
| -270 | `StorageFull` | `ENOSPC` | no space left | `StorageFull` |
| -271 | `ReadOnlyFilesystem` | `EROFS` | the filesystem or the device is read-only | `ReadOnlyFilesystem` |
| -272 | `InvalidAttributeName` | `ERANGE` | an attribute name empty, too long, or holding a zero byte | `InvalidInput` |
| -273 | `Unsupported` | `ENOSYS`, `ENOTSUP`, `EOPNOTSUPP` | this server does not do that operation | `Unsupported` |
| -274 | `DirectoryNotEmpty` | `ENOTEMPTY` | the directory still has entries | `DirectoryNotEmpty` |
| -275 | `FilesystemLoop` | `ELOOP` | too many links followed | `FilesystemLoop` |
| -276 | `AttributeNotFound` | `ENODATA` | the named attribute is not set | `NotFound` |
| -277 | `ProtocolViolation` | `EPROTO` | the decoder's answer to a malformed word; never sent | `InvalidData` |
| -278 | `KeyMissing` | `ENOKEY` | an encrypted filesystem with no key given | `PermissionDenied` |
| -279 | `KeyRejected` | `EKEYREJECTED` | the key given does not open it | `PermissionDenied` |

That is 24 codes for 27 numbers. The `std` column is a proposal for the overlay, which decides its
own mapping; a kind it does not take stays `Other`.

## The map from each `errno` in use

| `errno` | name | code | note |
|---:|---|---:|---|
| 1 | `EPERM` | -256 | |
| 2 | `ENOENT` | -257 | |
| 5 | `EIO` | -258 | |
| 7 | `E2BIG` | -259 | |
| 9 | `EBADF` | -260, or -273 | -273 where a caretaker answers a verb its contract does not carry |
| 12 | `ENOMEM` | -261 | |
| 13 | `EACCES` | -256 | merged with `EPERM` |
| 16 | `EBUSY` | -262 | |
| 17 | `EEXIST` | -263 | |
| 18 | `EXDEV` | -264 | |
| 20 | `ENOTDIR` | -265 | |
| 21 | `EISDIR` | -266 | |
| 22 | `EINVAL` | -267, or -273 | -273 for virtio block's unimplemented opcode, `graphics_contract`'s and `compositor`'s unknown operation (`EBADOP`) |
| 24 | `EMFILE` | -268 | |
| 27 | `EFBIG` | -269 | |
| 28 | `ENOSPC` | -270 | |
| 30 | `EROFS` | -271 | |
| 34 | `ERANGE` | -272 | |
| 38 | `ENOSYS` | -273 | merged with `ENOTSUP` |
| 39 | `ENOTEMPTY` | -274 | |
| 40 | `ELOOP` | -275 | |
| 61 | `ENODATA` | -276 | |
| 71 | `EPROTO` | -277 | decoder only |
| 77 | `EBADFD` | -260 | merged with `EBADF` |
| 95 | `ENOTSUP` / `EOPNOTSUPP` | -273 | |
| 126 | `ENOKEY` | -278 | |
| 129 | `EKEYREJECTED` | -279 | |

The file server's one translating function is this table by number. The caretakers, the block and
graphics servers, and the compositor do not translate: they name the code at the site, so the two
site picks between the split rows by meaning.

## The choices inside it

These are the places where the proposal is more than a rename. An architect can rule each on its
own.

1. `EPERM` and `EACCES` become one `PermissionDenied`. In POSIX `EPERM` is a privilege and `EACCES`
   a mode bit; nife has no users, and both answer "not allowed". `std` already folds both into
   `PermissionDenied`.
2. `EBADF` and `EBADFD` become one `BadHandle`. The engine returns `EBADFD` for a mapping it no
   longer holds, which a client sees as a handle that cannot do this.
3. Every "this server does not do that" becomes `Unsupported`: `ENOSYS`, `EOPNOTSUPP`, and the
   sites that answer an unknown verb with `EBADF` or `EINVAL` today. A client then tells "not here"
   from "bad argument".
4. Codes borrowed for a nearby meaning take the nife meaning's name: `E2BIG` is `ValueTooLarge`,
   `ERANGE` is `InvalidAttributeName`, `ENODATA` is `AttributeNotFound`.
5. `ProtocolViolation` takes a number though no server sends it, so the decoder's whole answer is
   one enum.
6. `Unrecognized` for an unknown in-range number, so adding a code is compatible.

## What else was considered

- A one-to-one rename, 27 codes. It keeps POSIX's two-numbers-one-meaning pairs and the one
  number with two meanings, which is the inherited vocabulary §325 moved away from.
- Numbers that keep the `errno` as an offset (-256 minus `errno`). It is easier to read beside a
  Linux trace, and leaves gaps and a numbering nife did not choose; §325 refused that as option (G).
- Grouped ranges (files from -256, attributes from -512, keys from -768). It reads well, but it
  fixes categories before there are enough codes to need them, and a range can fill.

## Prior art

Fuchsia's `zx_status_t` and WASI's `errno` both chose their own numbers and names by meaning;
WASI kept POSIX names beside its own numbers and added `NOTCAPABLE`. Rust's `io::ErrorKind`
folds `EPERM` and `EACCES`, and `ENOSYS` and `EOPNOTSUPP`, the merges proposed here.

## How reversible it is

Two-way until §325 says it is not: no installs, nothing published, and no server outside nifeos.
A merged code can be split later by adding a number, which older clients read as `Unrecognized`.

## A concrete example

```rust
#[repr(i64)]
pub enum Code {
    #[doc(alias = "EPERM")]
    #[doc(alias = "EACCES")]
    PermissionDenied = -256,
    #[doc(alias = "ENOENT")]
    NotFound = -257,
    // ...
    #[doc(alias = "ENOTDIR")]
    NotADirectory = -265,
}
```

## What was not checked

Whether every engine code reaches the wire (the same gap as §325). The engine's own meaning of
`ERANGE`, which this assumes matches the attribute-name use. Every `EBADF` and `EINVAL` site one by
one: the split rows name the sites found by reading, and the building lane confirms each.
