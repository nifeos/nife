# Descent: nested paths and `std::fs::Dir`

*An appendix to [`notes/std.md`](../std.md), which is the page to read. This file holds milestone
122's walk, `std::fs::Dir`, the two live bugs the walk found, and the write-path correction. The
directory `notes/std/` and this file's stem are provisional names. Naming is calef's.*

The records this file cites by number:

- milestone 122 (a directory handle `std` can hold)
- milestone 47 (navigation and naming)
- §42 (a filesystem declares what it offers and must be truthful)
- §27 (the filesystem service)
- milestone 64 (enough `std` to run somebody else's crate)


## Descent: a nested path, and a directory a program can hold (milestone 122)

Until this milestone the PAL called `OPENDIR` in exactly one place, inside `read_dir`, against
`fs::ROOT`, and then let the handle go. Nothing in `std` held a directory, so a name had to be one
component and a nested path was refused as a class.

The consequence was sharp and is worth stating before the fix, because it is the shape of the bug
rather than its size. `read_dir(".")` yields `./name` and feeding that back to `File::open` works;
one level down an entry's `path()` is `sub/name`, which is two components, which was refused. A
`std` program could list a subdirectory and could not open what it had just been told was in there.
That pair is what every filesystem walker is built out of, so `walkdir` and `ignore` built and could
not walk.

Two answers, and they are not alternatives. The milestone's block argues both and this is what
landed.

### The walk: `File::open("a/b/c")` is `OPENDIR a`, `OPENDIR b`, `OPEN c`

Every name-taking verb goes through one function now (`walk` in `sys/fs/nife.rs`), which resolves a
path to the directory its last name lives in, plus that name. `open`, `create`, `mkdir`,
`unlink`, `rmdir`, `rename` and `readdir` all use it, so the shape is the same everywhere and there
is one place to be wrong.

The rights it asks for are the design, and they look like a widening until you do the arithmetic.
Each hop asks for `DESCEND | needs`, where `needs` is what the *final* verb requires on the
directory it lands in (`filesystem_contract::verb::TABLE` is the list; `OPEN`'s "READ or WRITE" is
the one row a caller has to resolve from its own `OpenOptions`). Carrying `needs` down the whole
chain rather than asking for it only at the end costs nothing that was ever available, because a
child's rights are its parent's *intersected* with the request. A right an ancestor lacks is a right
no descendant of it could have had either. So the walk asks for exactly the maximum the grant could
give at the depth it is going to, and no more.

`..` is refused at every position, not only the first, and that is what makes the walk safe by
construction rather than by checking. There is no verb for an ascent and no capability that would
designate what one reached. A handle names a directory and nothing on the wire names its parent.
`cap-primitives` solves this same problem on Unix and has to work far harder, because there it is
defending against a hostile namespace rather than standing on one that cannot express the attack.

At most two handles are open at once. Each hop drops the one before it, which closes it, so a
path's depth costs round trips rather than handle-table slots. The milestone block forecast one per
level and was wrong in the cheap direction.

### `std::fs::Dir`: the object, and the surprise that it already existed

The other answer is that a program should be able to hold a directory and open one name under
it, which is this system's actual model and what `cap-std` binds to. The surprise was that this is
not an interface anyone here had to invent. `std::fs::Dir` exists upstream behind
`#![feature(dirfd)]` (rust-lang/rust#120426), with `open`, `open_file`, `open_file_with`,
`metadata`, `remove_file` and `rename`, and it *is* `openat`.

nife was getting std's generic fallback, which stores a `canonicalize`d path. `canonicalize` is
`Unsupported` here, so `Dir::open` on this system failed at its first call. The std type most
aligned with what this OS *is* was the one type it could not offer. It is now a held `OPENDIR`
handle: `Dir::open(".")` is the granted directory and costs no message at all, and anything else is
one attenuated descent per component.

Why a `Dir` asks for everything its parent carries, when everything else here asks for the minimum.
The rule in the rest of this PAL is to ask for exactly what the verb needs, because over-asking is
`EPERM` rather than attenuation and a client cannot read its own capability. A held directory has no
single verb to be the minimum of. It is an object, and what will be asked of it later is not
knowable when it is minted. So it asks for the projection of the authority the process already holds
onto one directory inside it, which cannot widen anything (`parent & requested`) and is exactly what
`openat(dirfd, name, O_DIRECTORY)` hands back on Unix.

Knowing what the parent carries is the awkward part, because the contract has no way to say
"attenuate to whatever you have" and no verb reporting a handle's rights. The common case is one
message: ask for `dir::ALL`, and a full-rights grant answers with the handle. A narrowed grant
answers `EPERM`, and the PAL then finds out what is there by asking for one right at a time, one
message per bit of `dir::ALL` (seven today). It does so at most once per `Dir::open`, because every hop after the first descends from a
parent whose rights are now known. That probe is a workaround, not a design. The fix is a sentinel
in `OPENDIR`'s rights word meaning "the parent's, whatever they are", which is a wire-format change
and therefore not a lane's to make.

### The `dirfd` surface grew: `open_for_traversal`, `create_dir`, `open_dir`, `remove_dir`

`nightly-2026-08-26` added four more inherent methods to upstream `std::fs::Dir` under the same
`#![feature(dirfd)]` (rust-lang/rust#120426). They are `Dir::open_for_traversal` (an associated
function, the minimum-permission open) and three `&self` methods, `create_dir`, `open_dir` and
`remove_dir`, each a `self`-relative twin of a path-shaped free function this PAL already had
(`DirBuilder::mkdir`, `Dir::open`/`walk`, and `rmdir`). None of the four needed a new verb; each is
the existing `MKDIR`, `OPENDIR`+descend-loop, or `RMDIR` call, walked from `self.at` instead of
`proto::ROOT`.

`open_for_traversal` is `Dir::open` under another name here, and that is not a shortcut, it is
the same fact the section above already states. This contract has no bit meaning "enough to
descend, not necessarily to enumerate", so the traversal-minimum open and the full open ask for the
same thing.

`open_dir("", opts)` (or `"."`) is the one shape the walk cannot answer, and it is a real gap
rather than an oversight. Reopening `self` needs a second handle to the same server-side node, and
nothing on the wire mints one (`File::duplicate` documents the identical gap for files). Under the
granted directory itself it costs nothing, because `Dir::root()` is the `ROOT` sentinel and any
number of `Dir`s may hold it (its `Drop` never closes it). `Dir::open_dir(".")` on any other held
directory returns `Unsupported`. Nothing in this tree calls it today: `dirfd` is unstable and
`remove_dir_all`'s own implementation goes through the path-shaped free functions, not through
`Dir`. The fix, if a caller ever needs it, is a wire addition: a `DUP` verb, or an `OPENDIR` name
meaning "this node again".

### `remove_dir_all` needed no code

It had been refused with a note saying the recursion has to descend, a nested path is refused, and
the loop therefore belongs where it can hold a directory capability per level, which is
`components/src/rm.rs`. The second half of that was right and is now this module's business, because
the walk holds one per level. std's own generic implementation is written entirely in terms of
`read_dir`, `remove_file` and `remove_dir` on paths it composes with `DirEntry::path`, so switching
one re-export was the whole change.

That is the fourth refusal in this PAL found to have outlived its own reason, after milestone 64's
three. Each looked correct while it was wrong, which is the property that makes this class hard. A
refusal that is correct-looking reads exactly like a refusal that is correct.

### Two live bugs the walk found

Both were in the tree before this milestone and neither could show against the granted directory,
because nothing is *requested* there: `fs::ROOT` carries whatever the endpoint carries.

- A created file was unwritable one directory down. A file handle carries
  `parent & (READ | WRITE)` (`create_file_at`, the same arithmetic as `open_file_at`). So a descent
  that asked only for `CREATE` hands back a directory handle with no `WRITE` in it, which mints a
  file nobody can write. `std::fs::write("a/b")` created the file and then failed `EROFS` on its own
  first write. The fix is that `create_handle` asks for `CREATE` *and* what the caller will do with
  the file.
- `dir::EPERM` was reaching std programs as `Unsupported`. The PAL read a `-1` reply as the
  kernel refusing the invoke, on the recorded ground that `EPERM` is not in the FS server's
  vocabulary. It has been since milestone 47, where it is what a directory capability answers
  when a verb's right is withheld and when a descent asks for more than the parent holds. So the one
  reply meaning "this capability does not carry that right" arrived as "this platform cannot do
  that", which is §42's silent degradation and would have made a narrowed grant indistinguishable
  from no grant at all. It is now `ErrorKind::PermissionDenied`, which is not the EPERM fiction the
  path refusals avoid: those are the client saying no capability designates that name, where nothing
  consulted a permission. This is the server stating a fact about authority. What makes `-1`
  unambiguous is that every entry point checks reachability first, so a server has already answered
  by the time a request is issued.

### What is proven, and what is not

`std_exerciser`'s fs transcript walks all of it on both ISAs. It covers a file one directory down, a
file two down, and a subdirectory listed with every file in it opened through the `path()` the
listing handed back. It refuses a path for walking through a file. `std::fs::Dir` opens a file under
a held directory and refuses `..` through it. It renames between two directories in one message, and
runs `remove_dir_all` over a tree the program builds.

The rights discipline is exercised only under a full-rights grant, and that is the honest gap. Every
std test grants the mount root, so a walk that over-asked would pass all of them. That is precisely
the trap this PAL records nearly falling into once already. A std program spawned on an
`fs_subtree_caretaker` endpoint is the test that would close it.

## The write path: a correction to the record

notes/fs-server.md and §27 recorded an open item, that an end-to-end write "loops inside RedoxFS's
allocator commit on bare metal even on a pristine image". It does not. Driven through `std::fs`, a
write to the file the image ships completes on both ISAs and reads back through the server. The part
a cache cannot fake is that it reads back byte for byte when the host tool reopens the image
afterwards with the pinned engine. That check is in the gate (`redoxfs_check_after_run` compares
`scratch` against the fixture, and `mkredoxfs` rewrites it to a placeholder before every run, so the
check passing means this run's guest write landed).

The likely reason is the fix/irq-delivery change of 2026-07-29, which put the block server back on
the completion interrupt instead of polling the used ring. That is the same correction that note had
already made for the read path. Stated as likely rather than proven: what was measured is that the
write completes, not why the poll path did not.
