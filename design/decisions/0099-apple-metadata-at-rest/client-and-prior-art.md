# §99 appendix: what macOS does, and prior art outside the tree

Evidence for [§99 (where Apple's metadata lands: stream or
sidecar)](../0099-apple-metadata-at-rest.md): read from the source of Apple's SMB client and XNU,
and what Samba, calef's router and ksmbd do. The main page is complete without this one, which
exists to verify or challenge it.

## What macOS actually does, which is the fact the options hang from

This is the part that was worth reading source for, because it inverts the usual shape of a
compatibility decision. **The sidecar is not something the server implements. It is what the client
does when the server says nothing.**

Three files, read on 2026-08-18, not recalled:

1. The client's four extended-attribute entry points refuse outright without the bit. In Apple's
SMB client, `smbfs_vnop_getxattr`, `setxattr`, `listxattr` and `removexattr` each open with the same
guard, under a comment that says in the client's own words that the flag is how it learns the server
supports streams:

```c
	if (!(share->ss_attributes & FILE_NAMED_STREAMS)) {
		error = ENOTSUP;
		goto exit;
	}
```

(<https://raw.githubusercontent.com/apple-oss-distributions/smb/main/kernel/smbfs/smbfs_vnops.c>,
at lines 6724, 6981, 7066 and 7269.)

2. The mount's advertised capabilities follow the same bit. `smbfs_vfs_getattr` sets
`VOL_CAP_INT_NAMEDSTREAMS | VOL_CAP_INT_EXTENDED_ATTR` only when `FILE_NAMED_STREAMS` is present,
and `VOL_CAP_INT_READDIRATTR` needs it as well; `smbfs_vnop_getattrlistbulk` refuses with `ENOTSUP`
without it. So the bit also governs the bulk-listing path, which is the client-side half of the
`READ_DIR_ATTR` capability this server already declines.
(<https://raw.githubusercontent.com/apple-oss-distributions/smb/main/kernel/smbfs/smbfs_vfsops.c>,
lines 1537-1563; `smbfs_vnops.c` line 9308.)

3. `ENOTSUP` from a filesystem is where the kernel's own AppleDouble writer takes over. This is
the load-bearing one. XNU's VFS layer catches exactly that errno and falls back:

```c
	error = VNOP_GETXATTR(vp, name, uio, size, options, context);
	if (error == ENOTSUP && !(options & XATTR_NODEFAULT)) {
		/*
		 * A filesystem may keep some EAs natively and return ENOTSUP for others.
		 */
		error = default_getxattr(vp, name, uio, size, options, context);
	}
```

The same pattern guards `vn_setxattr`, `vn_removexattr` and `vn_listxattr`. The default
implementation writes an ordinary file in the same directory, named with a prefix the file spells
out as `#define ATTR_FILE_PREFIX "._"`, and for a volume root it uses `._.` instead.
(<https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/vfs/vfs_xattr.c>, lines
170-176, 258-262, 310-314, 374-380, 1383, 2113-2141.)

Put the three together and the consequence is this: the sidecar option is already implemented, by
Apple, on the other end of the wire, and this tree is already running it. A Mac writing an xattr
to this share today creates a file called `._name` beside `name`, which the share stores as ordinary
bytes because it is ordinary bytes. Nothing in `smb_proto` needs to know.

The naming, so the options below are unambiguous. The client maps
`com.apple.ResourceFork` to the stream `AFP_Resource`, `com.apple.FinderInfo` to the stream
`AFP_AfpInfo`, and every other extended attribute to a stream of its own name; the stream names on
the wire carry the `:$DATA` type suffix. `AFP_AfpInfo` is a fixed 60 bytes, `0x3c`, which is
`AFP_INFO_SIZE` in Samba's `source3/include/MacExtensions.h` and `uint8_t afpinfo[60]` in Apple's
client. `AFP_Resource` is unbounded: Apple's `AD_XATTR_MAXSIZE` resolves to `INT32_MAX`, and Samba's
module says in its own header that the resource stream "may be arbitrarily large, thus it can't be
stored in an xattr on most filesystem".

A correction this lane owes, and it is in this tree. `crates/fs_proto/src/lib.rs` says in
`xattr::MAX_VALUE`'s documentation that "Apple's `AFP_AfpInfo` is 402 bytes". It is 60. The 402 is a
different blob: it is the AppleDouble-shaped buffer Samba uses on its *netatalk* metadata path,
`uint8_t ad_data[402]` in `fruit_fstatat_meta`. The two were conflated, the number is quoted in a
sizing argument, and it is fixed in this lane's diff.

## Prior art outside the tree, and what the reference implementation actually does

**calef's router does not run either pure option.** Milestone 55's block records the working
`[global]` stanza, which sets `fruit:metadata = stream` and does **not** set `fruit:resource`. Per
Samba's manual page, `fruit:resource` defaults to `file`, described there as "use a ._ AppleDouble
file compatible with OS X and Netatalk". So the reference implementation is a hybrid: Finder
metadata in an extended attribute, resource forks in `._` sidecars on the disk. That is option 3
below, and it is the only option with a working deployment behind it.
(<https://www.samba.org/samba/docs/current/man-html/vfs_fruit.8.html>, opened 2026-08-18.)

Samba gates the extended-listing capability on the same bit the client does. In `vfs_fruit.c`'s
`AAPL` handler, `SMB2_CRTCTX_AAPL_SUPPORTS_READ_DIR_ATTR` is claimed only when the client asked for
it and `handle->conn->fs_capabilities & FILE_NAMED_STREAMS`. This tree declines
`READ_DIR_ATTR` for a stated reason already, and the two reasons turn out to be the same reason.

Samba has a migration for sidecar to stream, and it is on by default. `fruit:convert_adouble`
defaults to `true` and calls `ad_convert` when a file with a `._` companion is opened. That the
reference implementation shipped a converter is the strongest available evidence about how
reversible this decision is, in both directions: it is reversible, and it costs somebody a
converter.

`ksmbd` is the small-server data point. Its stream support is real when enabled, maps a stream to
an extended attribute named with a `DosStream.` prefix matching Samba's `streams_xattr`, and
implements `FILE_STREAM_INFORMATION` by enumerating that prefix. It is roughly 200 lines spread
across create, query-info, set-info and rename. That is the closest available measurement of what
option 2 costs somebody who already has an attribute store, which we do.
