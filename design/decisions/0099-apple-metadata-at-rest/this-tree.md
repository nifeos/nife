# §99 appendix: what this tree already does

Evidence for [§99 (where Apple's metadata lands: stream or
sidecar)](../0099-apple-metadata-at-rest.md): what this tree already does in the analogous case. The
main page is complete without this one, which exists to verify or challenge it.

## What this tree already does in the analogous case

Six greps, and three of them changed an answer.

**The `nifefs` 32-byte name limit does not bite, at all.** It is the **boot archive**, the flat
read-only initrd format in `crates/nifefs`, and it has nothing to do with what the SMB share stores.
The share is RedoxFS through `fs_proto`, and RedoxFS's limit is `DIR_ENTRY_MAX_LENGTH = 252`. The
brief that produced this proposal listed `NAME_LEN` as in scope; it is not, and saying so is worth
more than a paragraph weighing it.

The binding name limits are `smb_proto`'s own, and they are smaller than anything below them:
`path::MAX_COMPONENT` is 64 bytes and `path::MAX_PATH` is 128, both chosen against the sparsebundle
workload and both stated as this share's bound rather than the filesystem's.

`Path::parse` already accepts a sidecar name and refuses nothing about it. A component is
refused only when it is empty, exactly `.` or `..`, contains `/`, or is over-long. `._name` and
`._.` both parse. So option 1 below needs no change even to the parser.

`Path::parse` also accepts a colon, which is recorded in the module's own BUGS as one of the
reserved characters this server does not check. Today that is a compatibility gap. The day this
server claims `FILE_NAMED_STREAMS` it stops being one: `foo:AFP_Resource:$DATA` would parse as a
single 22-byte component and create a literal file of that name on the image. Any option other
than 1 has to fix that first, and the fix belongs in `path.rs` where the parse is.

`FileStreamInformation` is already answered, at `(1, 22)` in `smb_proto::server`: a file reports
one unnamed `::$DATA` stream and a directory reports none. It is the right skeleton for enumerating
named streams and it is already exercised.

The attribute store exists and its ceilings are the ones that decide option 2. Milestone 57 put
`GETXATTR`, `SETXATTR`, `LISTXATTR` and `REMOVEXATTR` in `fs_proto` as ops 14-17, and
`fs_proto::xattr` sets `MAX_NAME = 255`, `MAX_COUNT = 16` per node, and `MAX_VALUE = 3072`. The
last of those already carries a BUGS entry saying a resource fork larger than it is refused with
`E2BIG`, and §57's own BUGS says the 3 KiB ceiling is "untested against real Time Machine traffic".
It is now tested against the specification instead, and the answer is that `AFP_AfpInfo` fits with
two orders of magnitude to spare and `AFP_Resource` has no bound at all.

The `Share` seam is 16 methods over 3 implementations (`FIXTURE` and `MemoryShare` in
`smb_proto`, `FsShare` in `user/src/smb_server.rs`), and every widening of it has to land in all
three. That is the unit of cost for options 2 through 4.

And this tree already argued the sidecar's central defect, in its own voice. `notes/xattr.md`
records that a rename is free for a node-keyed attribute store and says why a sidecar is not:

> AppleDouble sidecars get exactly this wrong: a `._file` beside `file` has to be moved by hand, and
> every tool that renames without knowing about the convention orphans the metadata.

That argument is correct and it is aimed one layer off from this fork. It refuses a *server-side*
path-keyed store. It does not reach a sidecar the client writes and the client renames, because
the party that has to know the convention is the party that invented it. Where it does reach: any
other holder of the directory capability, meaning this tree's own `mv`, `redoxfs_host`, and a
recovery host.
