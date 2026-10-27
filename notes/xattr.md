# Extended attributes (milestone 57)

A named byte string attached to a file or a directory. Four verbs, a store the FS server keeps
above RedoxFS rather than inside it, and one property that is only available where we built it.

**Why this exists at all**: milestone 55 (Time Machine: SMB3 with Apple's extensions, and mDNS)
made the board a Time Machine target, and Samba stores Apple's metadata as extended attributes
(`streams_xattr`), which RedoxFS lacks. That premise is gone: milestone 55 was removed on 2026-08-30,
when its customer chose borg over SSH. The layer is built and works, on nobody's critical path.

The contract is `filesystem_contract::xattr`; the layer is `redoxfs_server/src/lib.rs`; the mechanism was decided
in DECISIONS §34's 2026-07-31 amendment. Every test behind it is listed in
[`xattr/`](xattr/README.md).

## The shape, in one picture

```text
  a client                    the FS server                    RedoxFS
  ────────                    ─────────────                    ───────
  GETXATTR  h "user.x"  ──►   node = handle h names            /.nife-attrs/0000002a
  SETXATTR  h "user.x" v ─►   blob = read the node's file  ──► (one file per node,
  LISTXATTR h            ─►   apply the change                  named for its TreePtr id)
  REMOVEXATTR h "user.x" ►    write the blob back
```

Attributes key on the node, not on a path, because that is what the FS server already works in
(`handles: Vec<Option<TreePtr<Node>>>`). Everything interesting follows from that one choice.

## The four verbs

| Verb | Handle | Page | Second word | Reply |
|---|---|---|---|---|
| `GETXATTR` | the file or directory | the name in | the value out | `xattr::reply(kind, len)` |
| `SETXATTR` | the file or directory | the name, then the value | `xattr::spec(kind, value_len)` | 0 |
| `LISTXATTR` | the file or directory | the names out | 0 | bytes filled |
| `REMOVEXATTR` | the file or directory | the name in | 0 | 0 |

The handle field is the node itself rather than a parent directory, which is the one shape
difference from `OPEN`. An attribute has no name in any namespace, so there is nothing to resolve
it under, and a client that wants one opens the file first exactly as `fgetxattr(2)` does.

### The limits, and the reason for each number

| Limit | Value | Why that number |
|---|---|---|
| `xattr::MAX_NAME` | 255 bytes | Linux's `XATTR_NAME_MAX`. Nothing Samba writes is refused for its name; `user.com.apple.metadata:_kMDItemUserTags` is 40 |
| `xattr::MAX_VALUE` | 3072 bytes | A `SETXATTR` carries a name and a value in **one page**, so `MAX_NAME + MAX_VALUE` must leave 4096 visible room |
| `xattr::MAX_COUNT` | 16 per node | 16 names of 255 bytes plus their length prefixes is **4096 exactly**, which is what lets `LISTXATTR` have no cursor |

Each ceiling answers its own errno: `ERANGE` for a name
this store cannot hold, `E2BIG` for an over-long value, `ENOSPC` for the seventeenth attribute. A
caller can act on which one it hit. DECISIONS §42's rule is that a verb which is offered fails
loudly rather than degrading, and a store that clipped a value to fit would hand back a file that
looked intact and was not.

### There is no attribute right

`GETXATTR` and `LISTXATTR` need `dir::READ` and are refused with `EBADF`. `SETXATTR` and
`REMOVEXATTR` need `dir::WRITE` and are refused with `dir::EROFS`. That is exactly what `READ` and
`WRITE` on the file need and answer, and no rung was added for it to the ladder of milestone 47 (navigation and naming).

An attribute is part of what a file *is*, not a separate object with its own authority, so a
capability that may read a file may read what is attached to it. The other half of the argument is
mechanical. Adding a rung would silently widen or narrow every grant that already exists,
depending on which way the default fell. Milestone 47's whole monotonicity property is that a
capability's meaning cannot change out from under its holder.

## Why a layer, and why that is not the usual bad idea

On Linux, layering metadata above a filesystem is worthless: anything can `open(2)` the file
directly and walk around the layer. Here nothing can. Every path to these bytes goes through
`filesystem_contract`, so a layer above the filesystem is as authoritative as the filesystem.

The argument that actually decided it was reversibility (DECISIONS §34). `filesystem_contract` hides which
implementation was chosen, so if attributes later prove central enough to justify diverging from a
pinned upstream, or if the change is accepted into RedoxFS, the implementation moves and no client
changes. Choosing the layer is therefore low-regret rather than a bet.

And it is crash-atomic, which was the check that had to pass before the layer was viable at all.
`fs.tx(|tx| …)` groups arbitrary mutations into one commit, so the file write and the attribute
write land together and a delete removes both or neither.

## The property only available here: rename is free

Set an attribute on a file, rename the file, read the attribute back. It works, and nothing in
the rename path knows attributes exist, because a rename changes a directory entry and the store
keys on the node.

AppleDouble sidecars get exactly this wrong: a `._file` beside `file` has to be moved by hand, and
every tool that renames without knowing about the convention orphans the metadata. So would any
path-keyed store. This correctness is only reachable inside the FS server, which is an argument for
the layer rather than a consolation for not forking the format.

## The three ways to get this wrong, and what each cost

The amendment to DECISIONS §34 (RedoxFS is the primary filesystem) named all three in advance;
here they are as mechanism.

### 1. A freed node's blob must die with it, or the next node inherits it

Node ids are recycled. If a file's attributes outlive the file, the next node the engine issues
that id gets somebody else's metadata attached to it. That is a correctness bug wearing a
housekeeping costume.

The purge asks the engine rather than guessing: `remove_node` answers `Some(id)` exactly when a
node's last link went, and `None` when a link remains, so `unlink` and `rmdir` purge on `Some`
and the decision is never ours to get wrong.

Rename replacement is the one removal the engine cannot tell us about. `rename_node` calls
`remove_node` on the destination and throws the freed id away, so `Server::rename` notices for
itself, and only when the destination's links were down to one.

The test provokes the reuse rather than reasoning about it (create, set, unlink, then create until
a node comes back with the freed id) and asserts that the reuse happened. Without that
assertion the interesting half of the test would go quietly vacuous the day the allocator changed.

### 2. The store must be invisible, in both directions

`.nife-attrs` is refused by every name-taking verb (`check_component`) and filtered out of every
listing (`Server::read_dir`). A store a client could name would be part of the namespace, and then
"the attributes of a file" would be reachable as ordinary bytes by anything holding the directory
they live in.

The filter runs before the cursor is applied, not after. A filter after `skip(cursor)` would
make one real entry vanish at whichever offset the store happened to sort to, which is the sort of
bug that shows up as "one file is missing from `ls` on Tuesdays".

### The store directory goes with the last attribute, and the reason it was worth doing

An earlier version of this note recorded "the store directory is never removed" as a limitation, on
the grounds that noticing it had emptied would cost a walk. **That was wrong about the cost.**
`remove_node` with `MODE_DIR` already refuses a directory that still has entries, with `ENOTEMPTY`,
so the engine's own check *is* the emptiness test and there is nothing to enumerate. Attempting the
removal and accepting the refusal costs one lookup, on the removals that emptied a blob.

The reason to bother is on the recovery side rather than the byte. `redoxfs_host extract` copies the
store out with the tree, so a leftover empty `.nife-attrs` would land in somebody's recovered
Documents folder as a directory nothing explains. A filesystem with no attributes on it is now
indistinguishable from one that never had any.

Both routes to empty are tested, because they are different code: `remove_xattr` writes an empty
blob and goes through `write_attrs`, while `unlink` purges one and does not. The assertion in the
middle is the one that keeps the test honest: the store must stay while a second node still has
attributes. Without it the test would pass just as well against an implementation
that removed the store on every removal.

### 3. A shrinking blob must be truncated, or the reader walks records nobody wrote

A write does not truncate (DECISIONS §27, four times corrected). So `write_attrs` writes the blob
and then truncates the store file to exactly its length. Without that, removing an attribute leaves
the previous blob's tail behind and the record walk reads it as more attributes. Here that failure
would present as *corrupted metadata* rather than as a longer file, which is worse, because the
checksums cannot see it: every block is exactly what somebody wrote.

## The type code, which nothing reads

Every attribute carries a `u32` kind. The layer stores it, returns it, and never interprets it.
`xattr::RAW` is zero, so a POSIX-style client that knows nothing about kinds writes the right one by
writing nothing.

Carrying a field nothing reads is speculative surface, and it is deliberate. `design/haiku-bfs-and-packages.md`
is the reason: BFS made attributes typed and indexed, with live queries over them, and its
author went on to build Spotlight. That is the ambitious version of this feature and it is a real
destination. A store of untyped blobs cannot become an indexed one later without a format migration
*and* a wire break; a store that round-trips a type code can. Four bytes a record and one packed
word, paid once, buys the option.

The structural link worth carrying: a BFS query returns a set of files, and milestone 47 already
decided that a set of files is granted by an `fs_nameset_caretaker` attenuated to a name set. So if
attributes ever become queryable here, the granting story is already designed.

**BUGS.** The kind is 31 bits, not 32, because it rides in the sign-protected half of a reply word
(the protocol's error convention is that a negative reply is a negated errno). BFS-style
four-character codes are ASCII and fit. A code with its top bit set is refused with `EINVAL` rather
than truncated into a type nobody wrote.

## The caretakers forward them (milestone 61)

The three filesystem caretakers proxy this same contract over a narrowed namespace, and until
milestone 61 all three answered `EOPNOTSUPP` to all four verbs. So a program handed one file by a
command line could read the file and could not read what was attached to it. Closed, and the
rights model above is what makes closing it safe. There is no attribute right, so a capability
that may read a file may read its attributes and a capability that may not write it may not change
them, with no new rung and no new decision.

Where the refusal comes from differs per caretaker, and the difference is each one's design showing
through rather than an inconsistency:

| caretaker | who refuses a write through a read-only grant |
|---|---|
| `fs_file_caretaker` | **the caretaker.** It holds one handle opened with the *directory's* rights, so the grant's direction lives only in this process. `SETXATTR` and `REMOVEXATTR` are refused with `EROFS`, by the same rule that refuses `WRITE` and `TRUNCATE`, derived from `filesystem_contract::verb`'s `mutates()` rather than from a list |
| `fs_subtree_caretaker` | **the FS server.** The caretaker performs no checks at all; a file handle inherits `READ`/`WRITE` from the directory it was opened through, so a read-only subtree grant is refused on the handle the server minted |
| `fs_nameset_caretaker` | **the FS server**, as above. The caretaker filters *directory names*, and an attribute name is not one, so it passes the filter without being compared against the set |

`fs_nameset_caretaker` asks "is this name in the set" on
every verb whose operand is a name in the granted directory, and the four attribute verbs carry a
name in the shared page that is not such a name. Filtering it would have refused a program its
own file's attributes on the grounds that `user.com.apple.metadata` is not a name the pattern
matched. The distinction is a variant of `filesystem_contract::verb::Operand` (`Name` versus `Payload`) rather
than a comment, so it is checked by a host test instead of remembered.

It is proven on both ISAs by three witnesses, a per-file, a subtree and a name-set grant, each
with a control that must fail. They are listed in
[`how-it-is-proven.md`](xattr/how-it-is-proven.md).

## EXAMPLES

Setting and reading one, through the sans-IO core (this is what a host test does):

```rust
let mut srv = Server::open(disk)?;
let h = srv.open_file("Documents.sparsebundle")?;

// 'CSTR' is BFS's spelling of a string type. Samba would write xattr::RAW.
srv.set_xattr(h, b"user.com.apple.metadata", 0x4353_5452, b"...opaque bytes...")?;

let mut page = [0u8; 4096];
let (kind, n) = srv.get_xattr(h, b"user.com.apple.metadata", &mut page)?;
assert_eq!(kind, 0x4353_5452);
```

The same thing on the wire, which is what `fixtures/src/fs_test_client/extended_attributes.rs` does:

```rust
// SET: the name and the value go into the shared page back to back; the value's length and its
// type code ride packed in the second word.
put_page(name);
put_page_at(name.len(), value);
call(FILE, fs::req(fs::SETXATTR, handle, name.len() as u64),
     xattr::spec(kind, value.len() as u64));

// GET: the name goes out on the page and the value comes back on it, so the reply carries the
// kind and the length and the bytes are read out of the page afterwards.
put_page(name);
let (r0, _) = call(FILE, fs::req(fs::GETXATTR, handle, name.len() as u64), 0);
let (kind, len) = (xattr::reply_kind(r0 as i64), xattr::reply_value_len(r0 as i64));
```

Getting them back off a dead board, which is what the whole feature is for:

```console
$ redoxfs_host xattr backup.img photo.jpg
         6  kind 0x43535452 'CSTR'  user.com.apple.metadata:_kMDItemUserTags
        32  kind 0x00000000  user.com.apple.FinderInfo
$ redoxfs_host extract backup.img / recovered
extracted / to recovered: 4 files, 3 directories, 0 symlinks, 165 bytes,
  3 attributes reattached, 1 type codes dropped
```

The attributes are on the recovered files, the raw store comes out beside them (which is where the
type codes stay, since no host filesystem has a field for one), and anything the host refused is
counted and named. See [notes/host-recovery.md](host-recovery.md).

## BUGS

- **You cannot create a file called `.nife-attrs` anywhere**, at any rights, in any directory.
  The refusal is `EINVAL`, the same one `..` gets, because the name is not expressible here rather
  than not permitted. Reserving it in every directory instead of only in the root is a deliberate
  trade: the rule a client has to remember is one sentence.
- ~~**The caretakers do not forward attribute requests.**~~ **Closed by milestone 61.** All three
  used to answer `EOPNOTSUPP`, which was §42-honest and was still a real gap: a program behind a
  per-file grant could not read its own file's attributes. They forward now, and the rights model on
  this page is what makes that safe rather than a widening. See [the caretakers'
  section](#the-caretakers-forward-them-milestone-61) below.
- An unlinked-but-open file loses its attributes immediately. POSIX would let `fgetxattr` keep
  working through the open handle until the last one closed; here the purge is in the unlink's
  transaction, because that is the only place it can be crash-atomic with the removal. Deferring it
  to the close would mean a server that died in between leaked the blob. The name goes and the bytes
  stay (that is `unlink`, and it is measured); the attributes go with the name.
- All blobs live in one flat directory. RedoxFS directories are H-trees, so lookup is hashed
  rather than linear, but a filesystem where most files carry attributes has a directory with an
  entry per file. That is fine for a backup target and is untested at a million entries.
- **No `XATTR_CREATE`/`XATTR_REPLACE`.** Set is set-or-replace, one operation. Linux's flags are
  emulable above a store only racily, and §42 forbids offering a verb whose guarantee we cannot
  meet. An attribute has no handle and no identity of its own, so "it already had a value" is not a
  fact a caller can act on differently.
- `MAX_VALUE` is 3 KiB, and Samba's `streams_xattr` can be asked to hold whole alternate data
  streams. A resource fork larger than 3 KiB is refused with `E2BIG`, loudly. Lifting the limit
  means chunking the transfer across requests, which this contract does not do for anything today.
  Whether Time Machine over `fruit:metadata = stream` ever exceeds it is unmeasured, and the board
  is where it will be measured.

## How it is proven

The layer's semantics are pure functions over a byte slice, tested on the host with no
filesystem, including a sweep that cuts a blob at every byte and reads back short rather than wrong.
`redoxfs_server`'s tests drive the real engine through every property above. Crash consistency is
measured, not argued: milestone 37 (prove RedoxFS's crash consistency)'s sweep now reads attributes as part of the state, interleaved
with writes to the same file. On device, both ISAs, a witness reports a bitmap of eight claims and
the kernel test asserts the exact set. [`how-it-is-proven.md`](xattr/how-it-is-proven.md) has every
test, the sweep's workload, and the eight claims.

## See also

- DECISIONS §34 and its 2026-07-31 amendment: why the layer, and the three things to get right.
- DECISIONS §42: a filesystem declares what it offers and must be truthful about it. Every refusal
  above is an instance.
- [notes/fs-server.md](fs-server.md) for the contract this extends and the transaction guarantee it
  rests on.
- [notes/host-recovery.md](host-recovery.md) for what a recovery host sees.
- [design/haiku-bfs-and-packages.md](../design/haiku-bfs-and-packages.md) for the typed, indexed
  version this deliberately does not foreclose.
