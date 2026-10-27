# Extended attributes come back on the files (milestone 57)

*Name: provisional. After the parent's own heading.*

How the recovery tool [`notes/host-recovery.md`](../host-recovery.md) describes puts a file's extended attributes back on the extracted
file, what it is honest about, a transcript, how it is proven, and its BUGS. It was built as part
of milestone 57 (partitioning and formatting a real drive, and extended attributes).

An image written by nife carries a directory in its root called `.nife-attrs`, holding
one small file per node that has extended attributes, named for that node's `TreePtr` id in hex.
`redoxfs_host ls` shows it, `extract` copies it out, and upstream's FUSE mount would too.

That the store is *visible* here is deliberate rather than a leak, and both halves are worth saying:

- It is unreachable through the contract. No client of the FS server can open, create, list, or
  descend into it, in any directory. The confinement is the *contract's*, and a recovery host is not
  a client of the contract; it holds the image file.
- And a backup that carries the store carries the metadata. The format is written down in
  `filesystem_contract::xattr::store` precisely so a person holding a damaged image can read it: a record is a
  name length, a `u32` type code, a `u16` value length, then the name and the value, little-endian.

But a blob called `0000002a` is not a recovery. The tool now puts the attributes back on the
extracted files, which is the half that decides whether the backup did its job. Time Machine's
sparsebundle carries Apple's metadata in exactly these attributes. So the part of the backup a Mac
needs in order to make sense of the rest was the part that used to come out unreadable.

`setxattr` on macOS, `lsetxattr` on Linux, neither following a symlink. Directories get theirs as
well as files. `ls` marks an entry that carries attributes with `@`, which is the same marker macOS
`ls -l` uses, and `xattr` renders or dumps them without extracting anything.

## Three things it is honest about, because §42 is the rule here too

§42 (a filesystem declares what it offers and must be truthful) applies to the recovery tool too.

The type code cannot come along. No host filesystem has a per-attribute type word, so the `u32`
kind is dropped, each non-`RAW` one is named on stderr, and the count is in the summary. It is not
lost: the raw store is still extracted beside the tree, and that is where the codes live. That is
what makes "dropped" honest rather than lossy.

Nothing about attributes can fail an extraction. A damaged blob, a name Linux refuses for want
of a `user.` prefix, an attribute on a symlink (which Linux refuses outright), a destination
filesystem that holds none at all: each is reported, counted, and walked past. A recovery that
abandoned a hundred thousand files over one bad blob would be worse than the gap it fixes.

The counts are printed even when they are zero, and that is the important one. "0 attributes
reattached" on a backup you know carried some is the line that tells you the destination filesystem
cannot hold them. A summary that mentioned attributes only when the number was non-zero would read
identically to a backup that never had any, which is the failure this whole feature exists to
prevent: a recovery that looks complete and is not.

## EXAMPLES

A whole recovery, on a Mac, with the attributes on the other end. This is a real transcript.

```console
$ redoxfs_host ls backup.img /
dir         4096  .nife-attrs
dir         4096  nested
file@         13  photo.jpg

$ redoxfs_host xattr backup.img photo.jpg
         6  kind 0x43535452 'CSTR'  user.com.apple.metadata:_kMDItemUserTags
        32  kind 0x00000000  user.com.apple.FinderInfo

$ redoxfs_host xattr backup.img photo.jpg user.com.apple.metadata:_kMDItemUserTags
Family

$ redoxfs_host extract backup.img / recovered
redoxfs_host: recovered/photo.jpg: attribute user.com.apple.metadata:_kMDItemUserTags kept its
  6 bytes but not its type code 0x43535452; host filesystems have no field for it (see
  .nife-attrs in the extracted tree)
extracted / to recovered: 4 files, 3 directories, 0 symlinks, 165 bytes,
  3 attributes reattached, 1 type codes dropped

$ xattr -l recovered/photo.jpg
com.apple.provenance:
user.com.apple.FinderInfo:
user.com.apple.metadata:_kMDItemUserTags: Family
```

The last command is macOS's own `xattr(1)`, not ours, which is the point of running it: the
attribute is on the file according to a program this project did not write. (`com.apple.provenance`
is the Mac's own addition to a freshly written file, not something out of the image.)

## How the reattachment is proven

`tools/redoxfs_host/tests/attributes.rs`, and the shape of it is again the argument.

- The fixture is written by `redoxfs_server::Server`, the sans-IO core that runs on the board, driven
  over a `DiskFile`. Not by a second writer in this crate: a reader and a writer in one crate can
  share a misunderstanding of the format and agree perfectly. This is the same reason `recovery.rs`
  fills its image through upstream's archiver rather than through our own `put`.
- The tree is not flat: a 1 KiB attribute, a typed one, one on a directory, and one on a file a
  level down.
- The round trip is closed on the host file, and then confirmed by `/usr/bin/xattr` where it exists.
- A host that cannot hold attributes is a case, not a skip. If the destination refuses them the
  test asserts that the tool *said so*, with a count. That path is real: `/tmp` is `tmpfs` on many
  Linux CI runners, and `user.*` attributes on `tmpfs` need kernel 6.6.

## BUGS

- A Linux host refuses a name with no `user.` prefix. The store holds bytes and requires no
  namespace (`filesystem_contract::xattr::is_valid_name` refuses only NUL and over-length), because
  there is no privilege here for a namespace to mean. Linux does have one, and `lsetxattr` answers
  `EPERM` for a name outside it. The tool reports the errno rather than rewriting the name: silently
  turning `foo` into `user.foo` would hand back a file whose metadata does not say what the backup
  said. In practice Samba writes `user.`-prefixed names, so this bites a name nife invented, not a
  name that came from a client.
- Linux refuses attributes on a symlink at all, for any `user.*` name. macOS takes them
  (`XATTR_NOFOLLOW`). So the same image extracted on the two hosts can differ in exactly that one
  place, and only the Linux run says so.
- A value larger than the destination filesystem's ceiling is refused by the host, counted, and
  named. `MAX_VALUE` here is 3 KiB, well under any host's limit, so this is a guard rather than a
  case anyone has hit.
- The tool has no attribute call for a platform that is neither macOS nor Linux. It refuses with
  a message saying the attributes are still in the extracted `.nife-attrs`, rather than compiling
  to a silent success. FreeBSD spells this `extattr_set_link` and is not wired up.
- `extract` still copies `.nife-attrs` out, even now that the attributes are also on the
  files. That is deliberate (it is the only home the type codes have, and the last-resort record if
  a host refused everything) and it does mean a recovered tree carries one directory a user did not
  put there. An image with no attributes left on it no longer has the directory at all, since
  milestone 57 made the last attribute take the store with it.
