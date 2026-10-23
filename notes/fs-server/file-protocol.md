# The file and block protocols, verb by verb

An appendix to [notes/fs-server.md](../fs-server.md). The doc comments in
`crates/filesystem_contract` give each verb's wire, its rights and its refusals. This page carries
why each verb is shaped the way it is. The directory rights and their errnos are in
[notes/dir-capability.md](../dir-capability.md), `RENAME` is in
[dir-capability/rename.md](../dir-capability/rename.md), `UNLINK` and `RMDIR` are in
[notes/rm.md](../rm.md), the mtime verbs are in [notes/touch.md](../touch.md), and `STATFS`'s record
is in [statfs.md](statfs.md).

## OPEN takes a path, and reports a size

Both are calef's rulings on #1387 for milestone 606 (a directory walk costs what it does on Linux),
ruling A for the path and ruling B for the size.

A name with a `/` in it means exactly the hop-by-hop walk a client would otherwise send. Each
directory on the way is an `OPENDIR` asking for `DESCEND | w1`, and every intermediate handle is
closed. So a path reaches nothing the walk could not, and the file handle carries what the walk
would have given it. A single-component name ignores `w1`. A server that does not take paths
answers any name with a `/` in it with `EINVAL`. That is how a client knows to fall back to the
walk.

The reply's second word is the file's size at the moment of the open. It lets a client reading a
whole file size its buffer without an `FSTAT`. It is a hint for that and nothing else, because the
file may change after the open. A caretaker or server that does not fill it replies 0. So 0 means
"not given", and a client asks with `FSTAT`, which for an empty file costs one message.

## Create is create

`CREATE` answers `EEXIST` for a name that exists, and modifies nothing. A caller that wants either
outcome must ask for both and say which it got. Create-or-open makes a partly working write read as
a working one, which is the failure §27 (the filesystem service) records. `CREATE` shares `OPEN`'s
shape exactly, so a client that can open can create.

`MKDIR` is descend-with-creation, designed together with `OPENDIR` as milestone 47 (navigation and
naming) asks. It shares `OPENDIR`'s shape and its attenuation, so a program cannot mint itself more
authority by making a directory than by finding one.

## TRUNCATE shrinks

The size rides in the second word because it is an offset, not a payload length. Shrinking is the
point of the verb. Without it, a write shorter than the file leaves the old tail in place. A caller
replacing a file's contents then gets a longer file than it wrote, and a write that half-works
reads as one that failed (§27). The same reasoning puts `SETMTIME_AT`'s seconds in `w1`.

## OPENDIR hands back authority

`OPENDIR` is the first verb in the contract that returns authority rather than data, which is why
its rules sit in the contract rather than in a server. The child's rights are `parent & requested`.
A request whose intersection comes up short is refused with `EPERM` rather than quietly granted the
smaller set. That refusal is a courtesy and not the safety property: delete it and the intersection
still holds. Refusing an empty component, `.` and `..` with `EINVAL` is what keeps `..` from
meaning anything.

`READDIR` returns entries sorted by name, so its cursor means the same thing across calls. A
directory changed between two calls of one enumeration can therefore repeat or skip a name. That is
the ordinary `readdir` caveat, recorded rather than fixed.

## UNLINK and RMDIR, and why neither revokes

An open handle on an unlinked file keeps reading. The FS server registers each open node with the
engine's usage table, so removing the last link queues the node rather than freeing it
(`release_node`). Atomic replace and the temp-file idiom both depend on this.

Revocation is not offered. It would mean invalidating handles the server minted for clients it
cannot enumerate, because the handle table is per server. §13 (capability revocation and untyped
reclamation) and §16 (object revocation) revoke because the kernel owns those tables, and nothing
owns this one that way. A verb that removed a name and called itself revocation would be the silent
degradation §42 (a filesystem declares what it offers and must be truthful) forbids.

`RMDIR` takes only an empty directory, and that is the safety property. The recursion in `rm -r`
lives in userspace, as a loop of single steps, each needing the rights for it at that level. So a
recursive removal stops exactly where the capabilities stop. An interrupted `rm -r` leaves a partial
tree, as on Unix. A transaction spanning requests would mean the server holding one open across
receives. That breaks the run-one-request-to-completion property `RENAME`'s concurrency atomicity
rests on.

## The extended-attribute verbs

`SETXATTR` creates and replaces in one operation, unlike `CREATE`. An attribute has no handle and no
identity of its own, so "it already had a value" is nothing a caller can act on. Linux's
`XATTR_CREATE` and `XATTR_REPLACE` flags are not offered, for §42's reason: above the store they are
emulable only racily.

`LISTXATTR` has no cursor, and that is a property. `MAX_COUNT` names of `MAX_NAME` bytes fit one page
exactly, so a listing is always complete in one reply and cannot be observed half-changed. Removing
an attribute that is not there is an error, because the caller asked about one specific thing.

`MAX_VALUE` is three kibibytes. Apple's `AFP_AfpInfo` is 60 bytes: `AFP_INFO_SIZE` is `0x3c` in
Samba's `source3/include/MacExtensions.h`, and Apple's own SMB client carries `uint8_t
afpinfo[60]`. The 402-byte figure that also appears is a different blob, the AppleDouble-shaped
buffer on Samba's netatalk metadata path (`uint8_t ad_data[402]` in `vfs_fruit.c`). Both fit.

## STATFS and SYNC, and the write volume they leak

`STATFS` answers about the whole image, and the number moves when somebody else writes. So it is a
channel and not only a leak, and that is recorded and accepted. Two programs confined to disjoint
subtrees of one image can communicate: one writes and truncates to move the free count, and the
other polls `STATFS`. The bandwidth is unmeasured. It cannot be fixed at the verb, because a true
answer moves when the volume moves. Per-subtree quotas would replace it with a private number.
Until then, two subtrees on one image are not isolated from each other's write volume, and isolation
needs two images.

`SYNC`'s count is a fact about the device, not about the caller. So two clients of one image can
watch each other sync, which is the same channel at a lower bandwidth.

`SYNC` needs `dir::WRITE`. A flush stalls the device queue for every client of the image, and a
holder that may not write has nothing outstanding to make durable. When the device cannot flush, the
block server's `EOPNOTSUPP` passes through unchanged. It is the only errno on the file contract that
does not come from the FS server or RedoxFS. Answering 0 would be §42's silent degradation, and the
consequence would be somebody's backup.

It syncs the whole device, never a subtree or a file, because RedoxFS commits across the image;
hence `SYNC` and not `FSYNC`. It does not fence: a write and a sync issued at once on two handles
are unordered, and the sync covers what the server had acknowledged when it ran.

## The block flush answers a count

Every other block verb answers 0 for "done", and `FLUSH` does not. "The device was told" is a claim
a caller cannot otherwise check, and a stub would satisfy it just as well. A count that strictly
increases across each successful call is falsifiable. Two syncs that return the same number mean the
second did not reach the device. Milestone 55 (Time Machine) has its gate observe this, and it costs
one `u64` the server was incrementing anyway.

The `EOPNOTSUPP` case is the one a silent zero would ruin. A server that answered "done" there would
tell a backup client its data is on the platter when nothing asked the platter anything.

## BIND, UNBIND and BIND_SYNC

`BIND` is ruling D of milestone 606, and [notes/walk-cost.md](../walk-cost.md) has its design. Only
a caller whose own badge is unbound may bind, because only such a caller already holds everything a
binding could hand out. In practice that is the progenitor, or the kernel's own test wiring.

The three verbs sit above the directory verbs, outside the verb table's range. So `verb::of` knows
nothing of them and no caretaker forwards them. A client behind a caretaker could otherwise re-bind
or revoke another client's badge. A server takes `BIND` only if ruling D finds it eligible: it is
memory-safe, and it resolves every path and handle through `subtree_scope`. `redoxfs_server` is the
first. Any other server answers `EINVAL`, as for any unknown opcode.

`UNBIND` is revocation for a server-enforced grant. The kernel cannot take a capability back out of
a client's table, so the client keeps an endpoint that answers `EBADF`. A caretaker grant is revoked
by ending the caretaker instead.

`BIND_SYNC` serves `reboot`, under §251 (restarting the machine is a kernel object the progenitor
hands out). Every other client needs `dir::WRITE` on a handle to `SYNC`. The binder holds it, which
is what lets it hand a badge the sync alone.

## How much one request moves

The file request word carries a 40-bit length, so the packing never limited a transfer to a page.
The shared region did. Bulk rides in the region the two parties share, and a request longer than the
region would have the server write past its end. Growing the region to `TRANSFER_PAGES` contiguous
pages was therefore the whole of the multi-page transfer. It needed no new opcode, no second reply
word and no descriptor. Sixteen pages make 64 KiB, the size milestone 38 (filesystem throughput, and
the comparison) measured ext4 moving for about what it charges for 4 KiB.

The FS server maps every page, because it must serve any request the contract permits. A client
maps as many as it intends to use. A client that only moves 4096 bytes maps one page and cannot tell
the region behind the server grew, which is why `swish` and the caretakers need no change.

The foot gun is that nothing checks a client asked for no more than it mapped. A client that sends
`len = TRANSFER_MAX` with one page mapped takes a data abort on its own second page, after the
server has done the work. It is the agreement every client already has with its wiring about where
the region lives (each hardcodes its own `FILE_VA`), one number wider. Making it unrepresentable
needs the channel size on the wire, which is a new concept for the contract.

The block channel grew the same way. The device DMAs straight into the shared region, so the region
limited a request to one block. The count rides in the request word's low eight bits as `blocks - 1`.
The all-zero word that a single-block caller sends still decodes as one block. That is why
`disk_surveyor`, `disk_partitioner` and `mkfs` need no change. `blk::TRANSFER_BLOCKS` equals
`fs::TRANSFER_PAGES`, so a full file request becomes one block request per RedoxFS `Disk` call.

## Eight client windows

`fs::CLIENT_WINDOWS` is eight. That covers the heaviest path the tree wires: a shell, a set-grant
caretaker, a `>` file caretaker, and headroom. It costs the server 512 KiB of mapped frames. The
production value is a capacity decision nobody has made. The design of a frame per channel is in
[notes/a-frame-per-filesystem-client-channel.md](../a-frame-per-filesystem-client-channel.md).
