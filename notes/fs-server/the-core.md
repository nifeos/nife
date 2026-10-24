# The server core: handles, names, verbs and the engine boundary

Name: the file's provisional, minted 2026-10-24 (UTC) by `comments/929-redoxfs-server`; calef names
files, and the directory's own block in [README.md](README.md) covers the stem.

An appendix to [notes/fs-server.md](../fs-server.md). It carries the design behind the
`redoxfs_server` crate's handle table, its names and its verbs. It also lists every place the server
departs from upstream RedoxFS. The memo, the block cache, the dispatch's wire shapes and the EL0
binary's IO are in [caches-and-io.md](caches-and-io.md). Crash injection is in
[crash-consistency.md](crash-consistency.md), and the attribute store's format is in
[notes/xattr.md](../xattr.md).

## The handle table

Slot 0 is the bound directory, [`filesystem_contract::fs::ROOT`]. `Server::open` installs it with
every right, through `Rights::root`, whose only caller that is. This server is the whole image's
authority, and narrowing happens at the endpoint above it, never here. Slot 0 makes the directory an
endpoint designates an ordinary handle, so there is no separate current-directory field for a verb
to forget.

Rights ride on the handle rather than on the session. A child directory handle is a different
authority from its parent, held at the same time by the same client. A session-wide field could not
express that. A file handle is attenuated to `READ | WRITE` the moment it is minted, so it inherits
the direction its directory was granted with and can never widen it. That is also why the three
mtime verbs take a name under a directory handle: `SETTIME` could never be checked against a file
handle's rights.

The table is per server, not per client. Two clients sharing an endpoint share these handles, and a
rights-carrying handle attenuates only its holder. The handle is the authority and the endpoint is
the boundary. For the same reason `READDIR` keeps no snapshot per client, so a name added or removed
between two pages of one walk can be seen twice or missed. Confining a program to a subtree is
therefore a caretaker process holding the wider endpoint (`components/src/fs_subtree_caretaker.rs`),
or a badge bound by ruling D ([notes/walk-cost.md](../walk-cost.md)).

### Closing

Handle 0 cannot be closed, and the refusal is `EINVAL` rather than `EBADF`. The client did not open
it; it is the root of the client's namespace, and closing it would fail every later request.

A bound badge's grant root cannot be closed either, while it is bound. The table is per server, so
an open client could otherwise close the handle a bound badge's `ROOT` resolves to. The next open
anywhere would reuse the slot, and the bound badge's `ROOT` would name that object, outside its grant.
`Server::unbind` revokes the badge first and then closes the root. The `redoxfs_server_session` fuzz
target found this on 2026-10-04 (UTC).

### The refusal words

Each verb's refusal says which kind of fact it is:

- A missing naming right (`READ` or `WRITE` to reach a name, `DESCEND` to walk in) answers `ENOENT`.
  A holder that may not reach a name must not learn that it is there.
- A missing mutating right (`CREATE`, `REMOVE`, `WRITE` on a name) answers `EROFS`: through this
  capability the directory takes no change, and no policy could have said yes.
- A read through a write-only file handle is `EBADF`, POSIX's answer and a fact about the handle. A
  write through a read-only one is `EROFS`, `fs_file_caretaker`'s answer and a fact about the
  capability.
- A missing `ENUMERATE` is `EPERM` rather than an empty listing. An empty listing would be a false
  statement about the directory, and DECISIONS §42 (a filesystem declares what it offers and must be
  truthful) says a verb not offered fails loudly.
- Every right is checked before the name is resolved, so a capability that may not act cannot use
  the verb to find out whether a name exists.

`OPENDIR` and `MKDIR` refuse with `EPERM` when the parent's rights would grant less than was asked
for. That check is not what keeps a child inside its parent: delete it and the intersection still
holds. It refuses to hand back less than was asked for without saying so, which is §42's rule against
silent degradation. `MKDIR` needs `DESCEND` as well as `CREATE`, because making a directory you could
not have walked into would mint a capability out of a right that was withheld.

### `STATFS` and `SYNC`

`STATFS` validates the handle and then ignores it. Holding any minted handle is what entitles a
caller to ask, and the answer is about the one image behind all of them. It reads `header.size()` and
`allocator().free()` outside a transaction, because opening one to answer a question would put a
commit in the path of a read-only verb. RedoxFS's own `clone` computes free space from the same pair.
No transaction is in flight elsewhere to move the count, because the server answers one request at a
time.

`Server::sync_permitted` is the permission half of `SYNC` and performs no flush. The flush is a
`blk::FLUSH` to the block server, so it lives in the binary with the rest of the IO. RedoxFS's `Disk`
trait has no sync method, and this tree does not modify vendored code to add one. It takes a handle
of either kind, as `STATFS` does, and needs `WRITE`: a flush stalls the queue for every client, and a
read-only holder has nothing outstanding to make durable.

## Names

A name is copied out of the client's window before anything judges it (milestone 825 (a hostile
client races the file server's name window)). The window is the client's shared page, and a confined
client can run a second thread that rewrites it. Every verb that takes a path name takes a
`NameCopy`, whose only constructor copies into the server's own buffer and validates UTF-8 there.
The check (`check_component`) runs where each verb has always run it, after `dir_at` for the path
verbs, which keeps every verb's error order. That is why it is not in the constructor.

Attribute names are not path names. `SETXATTR` and `REMOVEXATTR` still read theirs, and the
value, from the shared window uncopied; the BUGS entry in `redoxfs_server/src/dispatch.rs` records
the race that leaves open.

`walk_prefix` hands `subtree_scope::walk` the copy's bytes rather than a `NameCopy`, because a shared
crate cannot name this server's type. It is the one caller, and the bytes are never the window's.

`check_component` holds the one-component rule; [notes/fs-server.md](../fs-server.md) has why it is
enforced here and not in the engine. It also refuses `\`, `:` and the attribute store's directory
name, in every directory. A store a client could name would make a file's attributes ordinary bytes
to anyone holding the directory. One rule everywhere is easier to remember than one per location, and
`EINVAL` is the answer because the name is not expressible here. `list` filters the store out before
any cursor is applied. A filter after the skip would make one entry vanish at whichever offset the
store sorted to.

## Verbs whose meaning is ours

### Create is create

`CREATE` and `MKDIR` check existence inside the same transaction as the create, so two clients racing
on one name cannot both be told they made it. An existing name is `EEXIST` and nothing is modified.

### Unlink, not revoke

RedoxFS frees a node the moment its last link goes, unless the node is registered as open in its
usage table. Then `remove_node` queues it on the release list and `on_close_node` collects it later.
`open_file_path` and `create_file_at` register, and `close` deregisters. That pairing is what makes
`UNLINK` an unlink: without it, the read after an unlink reads a deallocated node.

`open_file_path` edits `fs.node_usages` directly rather than calling `Transaction::on_open_node`,
which does the same three lines inside a transaction. Opening a transaction clones the engine's
whole allocator, and that clone was most of the remaining time per request once the memo answered
the lookup (milestone 606 (a directory walk costs what it does on Linux), `examples/walk_replay.rs`).
`create_file_at` is already inside its create's transaction, so it calls `tx.on_open_node`. `close`
drops a use directly, and calls the engine's `on_close_node` in a transaction only for the last use
of a node while the release list is non-empty.

That transaction does not forget the memo. The unlink already forgot every listing, no handle names
the node once `close` returns, and its id can only be reused by a verb that creates a node, which goes
through `change`. Forgetting there would make every walk re-read every file it had just read.

Two limits follow, recorded at the verbs. A handle never closed pins its node for the life of the
server, and the table outlives its clients, so a confined program that dies mid-session leaves its
files on the release list. Directory handles are not registered at all. After `RMDIR`, a client still
holding a handle to the removed directory names a freed tree slot. `release_node` frees the slot at
once when its usage count is zero, and `insert_tree` hands the first free slot to the next node
created anywhere. So the stale handle names whichever directory next takes that id, with the stale
handle's rights. This was read from `vendor/redoxfs/src/transaction.rs` by milestone 633 (an outside
agent attacks the confinement claim) on 2026-10-05 (UTC), not run. Both fixes are per-client handle
ownership, which is also what revocation needs.

### `RMDIR` removes only an empty directory

No single call on this contract can take a subtree away. Recursion lives in userspace
(`components/src/rm.rs`), as a loop of steps that each need the rights at their own level. A walk
therefore stops where the capabilities stop. `remove_node` checks the kind and refuses a non-empty
directory with `ENOTEMPTY` inside the transaction that would remove it. Neither is checked again
outside, because a second opinion that could disagree with the engine is worse than none.

### `RENAME`

Two atomicities, which DECISIONS §42 keeps apart. It is concurrency-atomic structurally: the serve
loop runs one request to completion before receiving the next, so nothing inside the server can
observe the name in both places. It is crash-atomic because it is one transaction and one commit in
the header ring. That is measured, as the last operation of the crash workload.

It needs `REMOVE` on the source directory and `CREATE` on the destination, both on one directory
for a rename within it.

A directory may be renamed in place but not moved between directories (`EINVAL`;
`filesystem_contract::fs::RENAME` has the cycle argument). Keeping the parent fixed is what makes the
in-place case safe without an ancestry walk.

Replacing a destination of a different kind is `EISDIR` or `ENOTDIR`, and that check is ours. The
engine's `rename_node` passes the destination's type to `remove_node`, so it never compares the two.
An offered verb must mean one thing on every backend. A rename onto the same node succeeds without
touching anything, because `remove_node` would take the only link. A replaced destination whose last
link this was has its attributes purged in the same transaction, since `rename_node` discards the
freed id and nothing else learns that a node died.

### Modification times

The server has no real-time clock. `clock` is a logical counter that every mutating verb advances
and stamps. `SETMTIME`'s "now" is that counter, so two touches in sequence observe increasing times,
which is what a staleness check depends on. It need not agree with a wall clock. `SETMTIME` needs
only `WRITE`, because the value is one the server observed. `SETMTIME_AT` needs `SETTIME` as well,
because the value is the caller's assertion about history (§112 (`touch`'s two behaviors need two
rights)). It takes any value: an implausible history is the same authority as a surprising one.

A read passes atime 0, so it never advances the node's access time and never copies the node on
write.

### Extended attributes

The store is a layer, one blob per node in a reserved root directory keyed on the `TreePtr` id. Every
mutation runs in one transaction, so an attribute and its file reach the platter in one commit.
[notes/xattr.md](../xattr.md) has the format and the layer's argument. Rules the store code keeps:

- `ENOENT` is the only lookup error read as absence. A dropped write to the root's tree block makes a
  lookup answer `EIO`, and reading that as "no attributes" would hand a client an empty set for a
  file that has some. The crash harness met the same false negative.
- A blob longer than `xattr::store::MAX_BYTES` is `EIO`, never a partial read of a file the layer did
  not write.
- A write is followed by a truncate to the new length. A write does not truncate, and a shorter blob
  would otherwise leave the old tail for the reader to walk (the failure DECISIONS §27 (the
  filesystem service: a capability-shaped contract) records).
- An empty blob removes the file, so "no file" and "no attributes" stay one statement.
- The store directory goes with its last blob. `remove_node` with `MODE_DIR` already refuses a
  non-empty directory, so the engine's check is the emptiness test and costs one lookup.
  `redoxfs_host extract` copies the store out with the tree, and an empty `.nife-attrs` would
  otherwise land in a recovered folder with nothing to explain it.
- The modes (0o700, 0o600) are decoration: the contract has no uid to check them against.

`GETXATTR` answers `ERANGE` when the buffer is shorter than the value, never a short read. On the
wire the buffer is the shared page, larger than `xattr::MAX_VALUE`, so it cannot fire there.
`LISTXATTR` has no cursor because a complete listing fits one page by `xattr::MAX_COUNT`.

## Where the server departs from upstream RedoxFS

The engine is pinned in `vendor/redoxfs.pin`, and every change to it is a numbered divergence in
`vendor/README.md` with its patch. The server depends on three of them:

- `NodeLevel::new` admitted `12 * 256^4` level-4 records where `NodeLevelData::level4` holds 8
  pointers. A record offset in that band indexed past the array and panicked the engine, which here
  is a dead server. The `redoxfs_server_session` fuzz target found it on 2026-10-04 (UTC). The root
  fix, `12` to `8` in `vendor/redoxfs/src/node.rs`, is a divergence taken by #1606. `MAX_FILE_END`
  stays beside it so the boundary answers `EFBIG` rather than an engine `ERANGE` deep in a
  transaction. It is conservative: every file this build creates has 8 KiB records, which could
  address twice it, and no image this system serves comes within a factor of a million of either.
- The record level is lowered and split in two (`vendor/README.md` divergence 5): `RECORD_LEVEL`,
  the level a new file is created at, is 1 (8 KiB records), and `RECORD_LEVEL_MAX`, which the two
  `BlockTrait::empty` guards compare against, stays 5. `block.rs` asserts `RECORD_LEVEL <=
  RECORD_LEVEL_MAX` at compile time, so a file this build creates stays readable by it.
- The `no_std` creation path takes the filesystem's uuid as an argument, as it already took `ctime`
  (`vendor/README.md` divergence 4, `patches/redoxfs-no-std-create-uuid.patch`). `mkfs` draws the
  bytes from an entropy capability, so no randomness enters vendored code.

Everything else is enforced at the server's boundary rather than patched in, because each is a rule
of this contract and not a bug in an engine whose other callers may want otherwise:

- `check_component`, where RedoxFS's `check_name` passes `/`, `.` and `..`.
- `RENAME`'s kind check, which `rename_node` does not make.
- The write end: RedoxFS adds `offset + len` unchecked, and a wrapped end made its inline-data path
  slice backwards and panic. `write` refuses an end that overflows or passes `MAX_FILE_END`.
- Durability: `Disk` has no flush, so `SYNC` is a block-protocol call in the binary.
- The usage table on the hot paths: `open_file_path` registers a use directly rather than through
  `on_open_node`, and `close` drops one directly unless the engine's `on_close_node` has a release
  list to collect.

One defect is left in the engine. Shrinking a file walks every record pointer in its logical size,
sparse or not, and the server answers one request at a time. A client with a writable handle can grow
a file to near `MAX_FILE_END` cheaply and then shrink it, stalling every other client. The same walk
runs when such a file's last name and handle go. The fuzz target found it as a ten-second timeout on
2026-10-04 (UTC). The two fixes are a divergence (skip a null subtree in `truncate_node_inner`) or a
semantics choice (refuse a sparse size past the image), and both are an architect's. It is proposed as
`design/roadmap/0772-a-client-cannot-stall-the-file-server-with-a-sparse-file.md`.
