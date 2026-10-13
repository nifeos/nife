# Milestone 57 appendix: extended attributes

An appendix to
[milestone 57 (partitioning and formatting a real drive, and extended attributes)](../0057-partitioning-and-xattrs.md),
under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main
page is complete without it. This is the extended-attribute fork, the two ways it could go, the
transaction check that decided it, and what is built.

## Decided in direction, and then in mechanism

calef decided 2026-07-30: extended attributes, not AppleDouble sidecars, on the grounds that we
will want them anyway. Agreed, and it does not reopen §34 (RedoxFS is the primary filesystem, on
three conditions). That entry surveyed ext2, FAT32/exFAT,
littlefs, btrfs, ZFS and F2FS before choosing RedoxFS, and xattrs were never the deciding axis. So
the requirement adds a gap to fill rather than a comparison to redo. ext4 works on the router, but
importing it means importing C, which §34 chose RedoxFS specifically to avoid. There is no
`no_std` Rust ext4.

Verified: RedoxFS has no xattr support. The fork is closed: the layer
(§34's amendment). Reversibility decides it: `fs_proto` hides which implementation was chosen, so
the format extension stays available later without any client changing. Attributes key on
`TreePtr<Node>`, so rename is free and correct, which sidecars get wrong.
Before designing the attribute layer, read `design/haiku-bfs-and-packages.md`. BFS made attributes
typed and indexed, with live queries over them. The point of knowing that is to avoid designing
something that forecloses indexing later, even though SMB only needs opaque blobs now.

BUILT 2026-07-31: the layer, on both ISAs (notes/xattr.md). Four verbs in `fs_proto`
(`GETXATTR`, `SETXATTR`, `LISTXATTR`, `REMOVEXATTR`) and a store the FS server keeps in a reserved
directory of the image, one blob per node, keyed on the `TreePtr` id. No new rung on the rights
ladder: reading an attribute takes what reading the file takes, changing one takes what writing
takes. There are three limits with a reason each, and the third is load-bearing rather than
arbitrary. Sixteen attributes of 255-byte names is exactly one page. That is why `LISTXATTR` needs
no cursor and therefore cannot be observed half-changed. Every ceiling refuses with its own errno
(§42 (a filesystem declares what it offers and must be truthful)).

BFS is not foreclosed. Every attribute carries a `u32` type code the layer stores, returns, and never
interprets. So an indexed store later is a change of implementation rather than a format migration
plus a wire break.

There were three ways to get it wrong, all of which §34's amendment named in advance. The purge
rides the same transaction as the removal and asks the engine (`remove_node` answers `Some(id)`
exactly when a node's last link went). The store's name is unnameable and unlistable in every
directory. A shrinking blob is truncated to length, so the reader never walks records nobody
wrote. The
rename-replacement case is the one removal the engine cannot report, and the server notices it.

BUILT 2026-08-01: the recovery side, and two of the three named gaps closed. `redoxfs_host
extract` puts the attributes back on the extracted files (`setxattr` on macOS, `lsetxattr` on Linux,
neither following a symlink). `ls` marks an entry that has them with `@`, and `xattr IMAGE PATH
[NAME]` renders or dumps them without extracting. The type code cannot come along, because no host
filesystem has a field for one. So each is named and counted, and the raw store still comes out
beside the tree as its only home. Nothing about attributes can fail an extraction. A damaged blob, a
name Linux refuses for want of a `user.` prefix, a destination filesystem that holds none: each is
reported and walked past. The counts print even when zero. "0 attributes reattached" is what tells
you the destination cannot hold them, and a summary that hid the zero would read like a backup that
never had any (§42). The fixture is written by `fs_server::Server` itself, for the same reason
the tree fixture goes in through upstream's archiver.

The store directory now goes with the last attribute on the filesystem. That closes a limitation
recorded for a reason that was wrong: `remove_node` on a directory refuses with `ENOTEMPTY`, so the
emptiness check is the engine's and costs no walk. It matters because `extract` copies the store
out, and a leftover empty `.nife-attrs` would land in a recovered Documents folder. And crash
atomicity is measured rather than inherited. The sweep of milestone 37 (prove RedoxFS's crash
consistency) now carries each name's attributes in its state, and four attribute operations in its
workload, interleaved with a write to the same file. So "the file and its metadata land together" is
decided rather than argued.

What was still not done here, and is now: the caretakers (`fs_file_caretaker`,
`fs_subtree_caretaker`, `fs_nameset_caretaker`) answered `EOPNOTSUPP` to all four verbs rather than
forwarding, so a program behind a per-file grant could not reach its file's attributes. Milestone
61 closed it, and found the general defect underneath: nothing made a caretaker and the contract
agree. So a whole contract addition reached none of them and nothing failed.

## The two ways it could have gone, and the check that decided it

- Extend the on-disk format. Correct, and atomic by construction since the metadata rides
  RedoxFS's own copy-on-write transaction. The cost is that §34 chose RedoxFS partly for being
  maintained upstream, pinned at 0.9.1 with a patch discipline that is currently two `Vec` imports. A
  format extension is a materially larger divergence that every future pin bump pays for. Upstreaming
  is the mitigation.
- Layer xattrs in the FS server. Normally dismissible, because on Linux anything can open the file
  directly and bypass the layer. Here nothing can: all access goes through `fs_proto`, so a layer
  above the filesystem is as authoritative as the filesystem. A genuine capability-system advantage.

The check that decided it, and it was small: does RedoxFS let us group a file write and a metadata
write into one transaction? If yes, layering is safe and much cheaper. If no, atomicity between a
file and its metadata cannot hold across a crash (§42's exact territory, and a rename must move both
together). Then the format extension is the only correct answer.

Answered yes (notes/xattr.md): `fs.tx(|tx| …)` groups arbitrary mutations into one commit. So the
file write and the attribute write land together, and a delete removes both or neither. Milestone
37's crash sweep measured it rather than inheriting it.
