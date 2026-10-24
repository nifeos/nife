# `std::fs` over the FS-service contract

*An appendix to [`notes/std.md`](../std.md), which is the page to read. This file holds how `File`
and the namespace verbs map onto the file contract, why a path is relative to a granted directory,
and what stays `Unsupported`. The directory `notes/std/` and this file's stem are provisional names.
Naming is calef's.*

The records this file cites by number:

- milestone 27 (Rust `std` on the native ABI)
- §27 (the filesystem service)
- milestone 47 (navigation and naming)
- milestone 122 (a directory handle `std` can hold)
- §22 (Rust `std` on the native ABI)
- milestone 31 (a capability shell)
- milestone 64 (enough `std` to run somebody else's crate)
- §47 (a directory capability carries six rights)
- milestone 108 (the drivers move onto frame capabilities)


## `std::fs` over the FS-service contract (milestone 27 phase two)

`sys/fs/nife.rs` binds std's `File` to the FS server's file contract (DECISIONS §27,
notes/fs-server.md, `crates/filesystem_contract`). Like the net PAL it is a client of a frozen
contract and nothing more, and like the net PAL its wire constants are generated verbatim
(`filesystem_contract` becomes `sys/pal/nife/fsproto.rs` by `std-src`), so the PAL's numbers cannot
drift from the server's.

### The interesting part: `File::open` takes a path, and there is no global namespace

This is the design question the binding had to answer, and the answer is not a compromise. Per §27,
open-by-path exists only inside the FS server, resolved relative to the one directory node the
client's endpoint is bound to. So the honest mapping is:

> a std program holds a directory capability (slot 4), and `File::open("motd")` means *"motd,
> under the directory I was granted"*, not *"motd somewhere in a global filesystem"*.

Four behaviors follow, and each is enforced on the client side, before a byte reaches the wire. The
server enforces the same rule again (it resolves one component in its bound directory and nothing
else). Doing it here as well is not redundant, it is what turns a would-be escape into a legible
`io::Error` instead of an `ENOENT` that reads like a missing file.

- A leading `/` names this process's own root (milestone 47's namespace half, 2026-08-18), which is
  the directory it was granted, so `/motd` and `motd` are one file. It was refused until then, and
  the refusal was the honest state of a system with no namespace to root a path in rather than a
  position. It grants nothing: the slash selects nothing, because a nife process holds exactly one
  directory capability and there is nothing else to select. A Windows-shaped prefix (`C:`) is still
  refused, because unlike a slash there is no root it could name.
- Any `..` is refused, at every position and including `/..`. It would leave the granted
  directory, and no capability designates what is out there. This is what makes the slash safe: an
  absolute path can reach the root and never above it.
- A nested path is a chain of descents (milestone 122). `File::open("a/b/c")` is `OPENDIR a`,
  `OPENDIR b`, then `OPEN c` against `b`'s handle. The grant is exactly as tight as a single
  open: every hop resolves under the capability this process holds, rights
  only ever narrow, and there are no symlinks to escape through. What changed is only whether the
  program has to spell the descent itself.
- A name that IS expressible but absent is an ordinary `NotFound`, which is what makes the three
  refusals above meaningfully different from "no such file".

A `Dir` handle is its own root, and that is a deliberate divergence from `openat`. POSIX makes an
absolute path ignore the `dirfd`, because `/` names one global thing and a `dirfd` is a shortcut
into it; neither half holds here. A handle is a namespace, `/` is the root of whichever one the name
is being resolved in. A rule that let a name climb out of the handle it was asked of would make
`std::fs::Dir` useless for the one thing programs reach for it for. It cannot widen anything either
way, since a process holding a `Dir` holds the root it descended from.

The refusal is `ErrorKind::InvalidFilename`, deliberately not `PermissionDenied`. Nothing
consulted a permission; there is no name here for what was asked, because no capability designates
it. Mapping a capability refusal onto `PermissionDenied` would be a Unix EPERM fiction, and this
whole milestone exists to avoid smuggling POSIX assumptions into std (the §22 reasoning). `NotFound`
was the other candidate (a sandbox commonly reports ENOENT for paths outside its namespace) and was
rejected for conflating the two cases a program actually wants to tell apart.

### Detecting "no filesystem" without touching the shared page

A program that was not granted a directory has no shared page mapped, so a probe that wrote a
name into it would fault instead of returning an error. The probe therefore has to carry no payload,
and it is an `FSTAT` on a handle number the server's table can never contain:

- no capability in the slot: the kernel refuses the invoke itself and answers with one of its own
  small negatives (`NoSuchSlot` -1, `WrongObject` -2, `NotPermitted` -3).
- a real server: it answers `-EBADF` (-9) for the impossible handle, which is a *reply*, so a
  filesystem is reachable.

The answer is cached, because a capability table slot's contents are fixed at spawn on this ABI.

A wart of the contract. The wire's error space (a
negated errno) overlaps the kernel's invoke-error space (-1..-8), so `-2` is both `ENOENT` and
`WrongObject`, `-5` is both `EIO` and `BadMethod`, and `-1` is both `EPERM` and `NoSuchSlot`.
`EPERM` is in the FS server's vocabulary, so the overlap is not harmless. Milestone
122 resolved `-1` in favor of the server (see [the descent appendix](fs-descent.md)): every entry
point checks reachability first, so a kernel `NoSuchSlot` cannot follow a reply. The cost of that
choice is that a revoked FS endpoint now reads as `PermissionDenied` rather than `Unsupported`,
which is a trade made deliberately, because `EPERM` is reachable every day and revoking the FS
endpoint is milestone 108's open question. `-3` still reads as the kernel's, because `ESRCH` really
is not in the server's vocabulary. The clean fix is a tag or an offset in the reply word, which is a
contract change (`filesystem_contract`, the FS server, and `fs_test_client`), reported up rather
than papered over here.

### What binds, and what stays Unsupported

Bound: `File::open` (`OPEN`), `read`/`read_to_end`/`read_to_string` (`READ`), `write`/`write_all`
(`WRITE`), `seek`/`stream_position`, `metadata`/`len` and `File::size` (`FSTAT`), close on `Drop`
(`CLOSE`), and `std::fs::{metadata, read, exists}` built from open + fstat + close. The file
position lives on the client side because the contract's read and write are both explicitly
positional, so there is no cursor in the server to get out of step with. A seek costs no message at
all except `SeekFrom::End`.

Also bound, since milestone 31 phase 2: `File::create`, `OpenOptions::create_new`, `create(true)`, and `OpenOptions::truncate`, backed by
the contract's new `CREATE` and `TRUNCATE` verbs. So `std::fs::write` works.

Two things about that are worth keeping. The order in `File::open` is POSIX's and it is
load-bearing: open, then create only if the open reported `NotFound` and the caller asked for it,
then truncate after a successful open of an existing file. `std::fs::write` is
`create(true).truncate(true)`, so getting the order wrong leaves the old tail behind on precisely
the path that exists to *replace* a file's contents, which is the confusion DECISIONS §27 was
corrected four times over. And the previous refusal was the right call at the time for a reason
worth remembering: without `TRUNCATE`, a `std::fs::write` would have half-worked, and a write that
half-works reads as a write that failed. `create_new` over a name that exists closes the handle the
probing open minted before returning `AlreadyExists`, because the error path is the one nobody
exercises and therefore the one that leaks.

And bound since milestone 64's second pass: `File::set_len` and `fs::copy`. `set_len` is the same
`TRUNCATE` message `File::open` has been sending since 31, with the requested size in the second
word instead of a 0. It grows as well as shrinks, because the contract's verb is `ftruncate` in both
directions and a binding that only shrank would pass a shrink-only test. `copy` is backed by no verb
at all and needs none. Neither was waiting on the contract, which is the third time this milestone
has found a refusal that outlived its own reason; see notes/crates-io-on-nife.md.

A per-file grant needs no std API at all, which is the payoff of having bound the PAL to a
capability contract rather than to a namespace. A program handed a narrowed file capability (§27's
caretaker, `components/src/fs_file_caretaker.rs`) is an ordinary `std::fs` client: the one granted
name opens, every other name is an ordinary `NotFound`, and a write through a read-only grant
surfaces as `ErrorKind::ReadOnlyFilesystem`. Nothing in the PAL knows whether slot 4 leads to a
directory or to one file, and it does not need to.

And bound since milestone 64: the namespace verbs. `read_dir`, `create_dir`, `remove_file`,
`remove_dir` and `rename`, on `OPENDIR`, `READDIR`, `MKDIR`, `UNLINK`, `RMDIR` and `RENAME`.

None of these was waiting on the contract, which is the part worth recording rather than the
feature. The FS server had been dispatching all six since milestones 47 and 48, while this note and
the PAL's own comments went on saying "no verb in the contract backs it". A refusal outlived its
reason by two milestones, and nothing caught it, because a refusal that is correct-looking reads the
same as a refusal that is correct. Milestone 64 found it by asking fifty crates.io crates what they
actually needed (notes/crates-io-on-nife.md), which put `create_dir` and `read_dir` near the top
of real demand.

Four behaviors are worth knowing before you use them:

- `read_dir(".")` lists the granted directory itself and costs no `OPENDIR`; the handle is
  `fs::ROOT`. `read_dir("sub")` descends first (`OPENDIR` mints a capability to `sub`, the
  enumeration runs through it, and it is closed afterwards), because there is no other way to name
  what is inside `sub`. There are no `.` and `..` entries: they would be names for things no
  capability designates.
- The listing is drained whole at `read_dir` time, not streamed. std hands the caller an
  iterator they may hold across arbitrary work including opening the files they just listed, and
  the listing arrives through the one page every other operation also locks. So a huge directory
  costs a `Vec` of its names, and the listing is a snapshot: an entry removed before the caller
  reaches it opens as `NotFound`. That is the ordinary readdir caveat rather than something this
  choice introduced.
- Neither remove verb removes the kind the other one is for. `remove_file` on a directory is
  `IsADirectory` and `remove_dir` on a file is `NotADirectory`, and `remove_dir` takes only an empty
  one (`DirectoryNotEmpty` otherwise). A single "remove whatever you find" is what makes `rm -r`
  dangerous and this contract does not offer it at any opcode.
- `EROFS` from any of them maps to `ErrorKind::ReadOnlyFilesystem`, and it is a capability
  answer, not a mode bit: the directory capability this process holds does not carry the right
  that verb needs (§47's ladder). It is the one error in the map that is about *you* rather than
  about the name.
- `metadata` answers for a directory now, at no extra message. `OPEN` refuses one with `EISDIR`, and
  that refusal *is* the answer to "what kind of thing is this name", so it is read as one rather
  than propagated. That is what makes `std::fs::create_dir_all` idempotent: it recovers from
  `AlreadyExists` by asking whether the name is already a directory. The `metadata(".")` call answers without
  a message at all, because the granted directory is what the endpoint is bound to rather than a
  name inside it. The size reported is 0 and is a placeholder. `modified` answers for a directory as
  for a file since milestone 64's last pass ([file times](file-times.md)); `accessed` and `created`
  still refuse, so nothing here invents a fact the contract does not carry.

Over-asking for rights is a refusal, not an attenuation, and it is the trap in this half of the
PAL. `OPENDIR` and `MKDIR` carry the rights the caller wants on the child. The server answers
`EPERM` when the intersection with the parent's comes up *short of the request* (§47's monotonicity
is the intersection; the refusal is the server telling the truth about it). A PAL cannot know what
its own capability carries, so it asks for the minimum the operation needs: `ENUMERATE` to
enumerate, and nothing at all for `create_dir`, which closes the handle it gets back. The first
version of this binding asked for `dir::ALL` and would have worked through every test in the suite,
because they all grant the root's full rights, and failed through every narrowed one.

Still Unsupported, and now genuinely because no verb in the contract backs it:

- `canonicalize`, `hard_link`, symlinks and `read_link`. `copy` is bound (milestone 64's second
  pass) and needed no verb: it is an open, a read/write loop and two closes, both names under the
  granted directory. `remove_dir_all` is bound since milestone 122 and needed no code either;
  see [the descent appendix](fs-descent.md#remove_dir_all-needed-no-code).
- Permissions, access and creation times, and setting a time through an open `File`.
  `Permissions::readonly` is honestly `false`: authority here is a capability, not a mode bit. The
  modification time is bound by name ([file
  times](file-times.md)); the rest of the time surface has no verb.
- `rename_noreplace` (unstable, from the 2026-10-13 nightly). `RENAME` always replaces, and
  [§129 (whether `RENAME` grows a `NOREPLACE` flag)](../../design/decisions/0129-rename-noreplace-flag.md)
  holds the flag until a customer needs one. The server could check it atomically; a client-side
  stat-then-rename could not, so the PAL refuses rather than emulate it.
- File locks and `File::try_lock`.
- `File::duplicate`. A handle is a token the server minted for one session; copying the number
  would forge a second owner of the same handle, including its close.
- `fsync`/`datasync` succeed rather than refuse, and that is honest rather than a shrug. Nothing is
  buffered on the client side, and the server commits a RedoxFS transaction per write (that is what
  makes a kill mid-write recoverable), so a returned write is already durable.

### Creating a file is not creating a filesystem

§27 keeps *filesystem* creation host-side, because RedoxFS's std-gated core APIs are exactly
creation, which needs a UUID and `getrandom`. Creating a *file* is a different operation.
`Transaction::create_node` is not std-gated, so the server makes a file through `CREATE` without
entropy ever becoming a userspace dependency.

### A leading slash selects nothing

A leading `/` is Plan 9's answer rather than DOS's, and less than Plan 9's: a nife process has
exactly one root, so the slash has nothing to select between.
