# The directory capability (milestone 47)

Milestone 47's keystone. Without it a directory is one authority: hand a program somewhere to
write its logs and you hand it the power to read everything already there and to delete it.
This note is the design that splits that into separable rights, the verb that hands a *directory*
back rather than bytes, and the process that makes a subtree grant checkable from outside the
program being confined.

The contract lives with its code in `crates/filesystem_contract` (the `dir` and `dirent` modules, and the
`OPENDIR`/`READDIR`/`MKDIR`/`RENAME` verbs in `fs`). The engine-side implementation is
`redoxfs_server/src/lib.rs`; the caretaker is `components/src/fs_subtree_caretaker.rs`; the wiring and the
attacks are `kernel/src/user/fs_service/directory_grant.rs`'s `start_granted_dir` and
`system_tests/src/user/dir_capability_tests.rs`. This note is the argument around them. Read
[fs-server.md](fs-server.md) first for the contract this extends. The evidence behind it is in
[dir-capability/](dir-capability/README.md).

What milestone 47 (navigation and naming) builds on this, from `cd` to globbing and completion, is
in [shell-navigation.md](shell-navigation.md). Four were easy; `rm` was not, because
"remove a name" and "destroy an object" are one operation in the engine until you make them two.

## The ladder, and why there are six rungs

The roadmap named five separable rights. There are six.

| Rung | What it is | The verb that consults it |
|---|---|---|
| `ENUMERATE` | list the names in it | `READDIR` |
| `READ` | open a name in it for reading, and read what you opened | `OPEN`, then `READ` |
| `WRITE` | open a name for writing, and write or truncate it | `OPEN`, then `WRITE`/`TRUNCATE` |
| `CREATE` | make a new name in it | `CREATE`, `MKDIR`, `RENAME`'s destination |
| `REMOVE` | take a name out of it | `RENAME`'s source (and `UNLINK`, later) |
| `DESCEND` | walk into a child directory | `OPENDIR`, and `MKDIR` alongside `CREATE` |

`ALL` is every rung together, and it is what the mount binds its root with. Nothing below the root
can ever be constructed with more.

A seventh rung, `SETTIME`, arrived on 2026-08-24 with `touch -t` under §112 (`touch`'s two behaviors
need two rights): setting a name's mtime to an instant the caller asserts ([touch.md](touch.md)). It
splits off a power none of the six carried, so the case for six stands.

### `DESCEND` earns its own rung, and this is the finding

The roadmap's five did not separate walking in from reading. Bundle them and granting a directory
transitively grants its whole subtree, to any depth. The authority a grant carries would then be
decided by *the shape of the tree* rather than by the grant: the same words, `here is somewhere to
put your logs`, hand over a lot or a little depending on what happens to be underneath. That is
ambient authority reintroduced by recursion, which is the exact thing this milestone exists to
refuse.

With `DESCEND` separate, a capability can be exactly one directory deep, and a program that holds
one cannot even learn that a subtree is there.

`MKDIR` needs `CREATE` and `DESCEND` together, for the same reason: making a directory you
could not have walked into would be a way to mint a capability out of a right that was withheld.

## Attenuation is by construction, not by a check

```rust
pub const fn attenuate(self, requested: u64) -> Self {
    Rights(self.0 & requested)
}
```

That `&` is the whole monotonicity property. `Rights::attenuate` is the only constructor for a
non-root rights set (`Rights::root` is the other one, and its single caller is the code that binds a
server to its mount), and `a & b` is a subset of `a` for every `b`. There is no code path that
widens, so there is no check to forget, no branch to get wrong, and no ordering to preserve. Three
Kani harnesses say so for every mask: at one level, at two, and that a root cannot be built carrying
a bit this contract has not defined.

The server *separately* refuses a request whose intersection came up short (`EPERM`), and it is
worth being clear about what that refusal is for. It is not the safety property. Delete it and the
intersection above still holds and the child is still bounded. What it does is refuse to hand back
less than was asked for without saying so. That is DECISIONS §42 (A filesystem declares what it
offers and must)'s rule against silent degradation: a caller that asked for `CREATE` and got a
capability without it should find out now rather than at its first write.

The sabotage check confirms it is load-bearing: replacing the body with `Rights(requested)` turns
three host tests and two Kani harnesses red.

## The refusal errno is part of the design, because it decides what the holder learns

A missing right could always answer `EACCES` and be done. It must not, and which word it uses is a
decision rather than a detail.

- A naming right withheld answers `ENOENT`. `READ`/`WRITE` for `OPEN`, `DESCEND` for `OPENDIR`.
  *In this scope there is no such name.* Nothing consulted a permission, and a holder that may not
  reach a name must not be able to learn that the name is there. This is the sentence
  `fs_file_caretaker` already says, for the same reason (DECISIONS §27).
- A mutating right withheld answers `EROFS`. `CREATE`, `REMOVE`, and `WRITE` on a file handle.
  *Through this capability, that directory is read-only.* `EACCES` was rejected on purpose, and §27
  had already rejected it for files: it implies a policy that could have said yes, and there is no
  policy here, only what the capability is.
- `ENUMERATE` withheld answers `EPERM`, and it is the one rung where neither of the other two
  works. "No such name" is nonsense when you are holding the directory. An empty listing would be a
  statement about *the directory* rather than about the capability, and it would be false. That is
  §42's silent degradation exactly: a verb that is not offered has to fail loudly rather than
  degrade into a plausible lie.

Two more are facts about a handle rather than about a right. Reading through a write-only handle is
`EBADF` (POSIX's own answer), and opening a directory as a file is `EISDIR` rather than a handle
onto a directory's raw bytes.

## Handle 0 is the bound directory

The bound directory is an ordinary entry in
the handle table at `fs::ROOT`, which is 0 because every client that ever sent an `OPEN` already
sent 0 in that field and meant exactly this. So there is no separate "current directory" for a verb
to forget to consult: every name-taking verb resolves under a handle, always. File handles start at
1, and the root cannot be closed (`EINVAL`, because it is not something the client opened).

That is Plan 9's answer in one number, and it lines up with the roadmap's "every shell has its own
root": `/` is the root of *your* namespace, and two clients on two endpoints both say `0` and mean
different directories.

A file handle inherits the `READ`/`WRITE` bits of the directory it was opened under, so what may
be done to a file was decided when the directory was granted rather than by the code that opened it.
That is the roadmap's "a program handed a directory to write logs into should not thereby be able to
delete what is there", made structural.

## The structural finding: the handle is the authority, the endpoint is the boundary

The FS server's handle table is per *server*, not per client. Two clients sharing one endpoint
share those handles. A rights-carrying handle therefore attenuates only *its holder*: anyone holding
the FS-service endpoint can name `fs::ROOT` and be back at the image root, whatever narrow handle
they were given. Rights on a handle are not confinement.

So confining a program to a subtree is not "give it a narrow handle". It is give it an endpoint
that reaches nothing else, which means a caretaker process, exactly as a per-file grant needs one
(§27's amendment, notes/grant-expression.md). The narrowing is an address space, not a branch.

Serving a second, narrower endpoint from the FS server itself would need a receive over a *set* of
endpoints, which this kernel does not offer. Adding it means giving endpoint capabilities a badge
(seL4's answer), which is a design fork and is recorded as the alternative rather than taken.

`fs_subtree_caretaker` is that process, and it performs no rights checks by design. At startup it
sends exactly one `OPENDIR` for the granted name with the granted rights, and everything the client
reaches afterwards goes through the handle that request minted. It only translates handle numbers.
Since milestone 61 (the caretakers: one verb table) its dispatch is
`filesystem_contract::verb::of(op)`, so a verb with no row is a
compile error, and the table shares the dispatch and never the attenuation. One frame is shared by
all three processes; at startup that needed an ordering fix, because the caretaker's name and the
client's first name shared the page. The detail is in
[dir-capability/caretaker.md](dir-capability/caretaker.md).
## `RENAME`

`mv`'s verb, and the one that made `REMOVE` real: until it existed nothing on the wire consulted
that rung, and a right nothing enforces is a right the contract is lying about.

The wire. The only verb here that names two directories, so it is the only one whose second word
is not a scalar. The source is the request word's handle and length as usual; the destination rides
in the second word packed by `fs::rename_destination` with the same two fields. Both names are in the shared
page, source first, back to back. A pair longer than the page is `EINVAL` rather than a clamp,
because clamping a name renames something else.

Rights. `REMOVE` on the source (its name goes away) and `CREATE` on the destination (a name
appears), each refused with `EROFS`. Within one directory a rename needs both on that one. The
rights are checked before anything is resolved, so a capability that may not move a name cannot
use the verb to find out whether one is there.

It is concurrency-atomic, because the server runs one request to completion before the next. It is
crash-atomic, measured by `redoxfs_server/tests/crash_consistency.rs` cutting the device at every
write (§42 (a filesystem declares what it offers)). `renameat2`'s `EXCHANGE` and `NOREPLACE` and
cross-filesystem moves are not offered, and moving a directory between directories is `EINVAL`. Two
checks are ours rather than the engine's: file over directory is `EISDIR`, and directory over file
is `ENOTDIR`. The design in full is in [dir-capability/rename.md](dir-capability/rename.md).

## What the tests prove

Three `#[test_case]`s run on both ISAs. Each grants `sub` with one rights set (read-only, full,
append-only) to an attacker that is told nothing about its grant, tries every verb, and reports a
bitmap of what got through; the test asserts the exact set. The three runs are each other's
controls, and two bits stop the result being vacuous. A host-side check then reads the image the
run left behind and confirms nothing escaped upward. The FS server's stack high-water mark went
down by about 3.7 KB with `RENAME`, unattributed, and two thirds of its grant stays free. The runs,
the host check and the stack table are in
[dir-capability/tests-and-stack-usage.md](dir-capability/tests-and-stack-usage.md).

## BUGS

Known limitations, next to the feature rather than only in a tracker. Each is in full in
[dir-capability/limitations.md](dir-capability/limitations.md).

- A directory cannot be moved between directories (`EINVAL`).
- `READDIR`'s cursor is an index, so a name changed mid-enumeration can be seen twice or missed.
- A client of a dead caretaker blocks forever, as a client of a dead FS server does.
- `fs_subtree_caretaker` holds at most 16 handles per client.
- A single-name grant is still the directory the name is in.
- A grant on the root of a shell's namespace cannot be narrowed at all.
- A missing directory reads as "the progenitor is out of memory" at the prompt.
- A grant more than one level down is not delivered: nothing builds a chain of caretakers.
- `caps` does not print the rights as bits.
- A wrong row in `filesystem_contract::verb` is wrong in three programs at once.
- No verb reports what rights a handle carries.
- Closed: `std::fs::rename` is bound, `UNLINK` and `RMDIR` exist, and the caretakers forward the
  extended-attribute verbs.

## EXAMPLES

Grant a subtree to a confined program, read-only, and attack it:

```rust
// system_tests/src/user/dir_capability_tests.rs
let report = fs_service::start_granted_dir(
    blk_server_image(),
    program("redoxfs_server").unwrap(),
    program("fs_subtree_caretaker").unwrap(),
    program("fs_test_client").unwrap(),
    fs_service::DirectoryGrant {
        name: filesystem_contract::fixture::tree::SUB,
        rights: dir::DESCEND | dir::READ | dir::ENUMERATE,
        role: 5,  // ROLE_DIR_ATTACKER
        arg: 1,   // the run index
    },
)?;
let [tag, verdict, ..] = sched::ipc_receive(report);
```

Descend and then descend again, from a client, asking for less each time:

```rust
// the granted directory is always fs::ROOT on your own endpoint
put_page(b"deeper");
let child = call(FILE, fs::request(fs::OPENDIR, fs::ROOT, 6), dir::DESCEND | dir::READ).0 as i64;
// `child` can read and descend. It cannot create, remove or enumerate, and asking for
// dir::ALL here would have been EPERM rather than a quietly smaller capability.
```

Move a name inside your grant:

```rust
put_page(b"tmp");           // the source name
put_page_at(3, b"report");  // the destination, immediately after it
let r = call(
    FILE,
    fs::request(fs::RENAME, fs::ROOT, 3),
    fs::rename_destination(fs::ROOT, 6),
).0 as i64;
// 0, or -EROFS if this capability carries no REMOVE (source) or no CREATE (destination).
```

Run the whole thing:

```sh
script/test                         # both ISAs, plus the post-run host confinement check
cargo test -p filesystem_contract   # the contract and the ladder's arithmetic
cargo test --manifest-path redoxfs_server/Cargo.toml   # the engine side, and the crash sweep
cargo kani -p filesystem_contract   # attenuation never widens, at any depth
```
