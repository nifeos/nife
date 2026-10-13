# Milestone 55 appendix: the reference implementation and what it requires

An appendix to
[milestone 55 (Time Machine: SMB3 with Apple's extensions, and mDNS)](../0055-time-machine.md),
under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main
page is complete without it. It holds the router whose configuration became the requirement list,
and what that list implies. The work is recorded at full size deliberately, because the failure mode
here is starting it while imagining it is "a file server".

## The reference implementation is known, and calef supplied its exact configuration

calef's router is a GL.iNet GL-BE9300 (Flint 3) running OpenWrt. It serves three family Time Machine
targets through Samba with `vfs_fruit` (2026-07-30). So the reference is full Samba, not `ksmbd`, and
the working `[global]` stanza is on the record:

```
fruit:aapl = yes                 fruit:metadata = stream
fruit:time machine = yes         fruit:model = TimeCapsule
vfs objects = catia fruit streams_xattr
fruit:posix_rename = yes         fruit:nfs_aces = no
fruit:veto_appledouble = no      fruit:delete_empty_adfiles = yes
fruit:wipe_intentionally_left_blank_rfork = yes
```

That is a measured feature list rather than a guess, and it decodes into these requirements:

| Setting | What we must implement |
|---|---|
| `fruit:aapl = yes` | The AAPL SMB2 create context. The core of it: macOS negotiates Apple extensions on connect and will not accept the share without them |
| `fruit:time machine = yes`, `model = TimeCapsule` | Advertise the share as a Time Machine target and return the model string |
| `streams_xattr` + `metadata = stream` | Alternate data streams, for Finder metadata and resource forks. See below, this is the expensive one |
| `fruit:posix_rename = yes` | Rename over an open file, POSIX semantics |
| `catia` | Character mapping for names macOS permits and the backing filesystem does not |

## Extended attributes, and the SMB half that is still missing

Milestone 57 (partitioning and formatting a real drive, and extended attributes) built xattrs into
`fs_proto` (`GETXATTR`/`SETXATTR`/`LISTXATTR`/`REMOVEXATTR`, ops 14-17), the FS server and the
store, so the choice below is not a stack-deep one. What is missing is the SMB half: alternate data
streams. That is a stream name in a CREATE path, `FileStreamInformation` in `QUERY_INFO`, and
`FILE_NAMED_STREAMS` in the volume attributes. Where the metadata lands, stream or sidecar, is §99
(where Apple's metadata lands: stream or sidecar). `streams_xattr` stores Apple metadata in
NTFS-style alternate data streams backed by filesystem xattrs.

There is an escape, and it should be chosen deliberately rather than discovered late. Samba's
`fruit:metadata` also accepts `netatalk`, which keeps the same metadata in AppleDouble sidecar files
(`._name`) needing no filesystem support whatsoever. calef's router uses `stream` because ext4 has
xattrs. So this is a design choice between adding xattrs down the whole stack (protocol, FS server,
RedoxFS) and accepting sidecar files. It is not the hard blocker it first appears to be.

The router's stanza sets `fruit:metadata = stream` and does not set `fruit:resource`, whose default
is `file`, meaning a `._` AppleDouble sidecar. So the working reference implementation is a hybrid:
Finder metadata in an extended attribute, resource forks in sidecars on the disk. That was read off
Samba's own manual page rather than recalled. §99 carries this and the rest of the evidence.

## `fruit:posix_rename` is not work at all

The section was first titled "`fruit:posix_rename` lands squarely on work already scoped", and framed
the setting as rename over an open file. That is precisely the territory of §42 (a filesystem
declares what it offers and must be truthful) and of the `mv` section of milestone 47 (navigation and
naming).

Samba's `fruit:posix_rename` switches on two behaviors:

- renaming onto an existing name, which is already `fs_proto::fs::RENAME`'s documented semantics;
- renaming a file that is open, which cannot fail here, because the SMB server consults
  `ShareAccess` nowhere and has neither oplocks nor leases.

So there is no sharing violation for POSIX semantics to be an exception to. What matters is
`RENAME` itself, below, and §42's atomicity split, which is exactly what Time Machine's durability
expectations will test.

`fs_proto::fs::RENAME` is op 11, with its rights and its atomicity documented. The PAL's
`rename` packs both names into the shared page and issues the request. It returns
`unsupported_err()` only when no filesystem is granted at all, which is equally true of `open`.

The correction matters more than the fact. A false blocker on the customer path makes the work look
harder than it is, and the cost lands on whoever reads this block deciding whether to start. It
survived because a milestone block is written once and the tree keeps moving. §42's
concurrency-versus-crash atomicity split was always right.

## Three users, and this is where the thesis gets a concrete demonstration

calef's setup serves two macOS users' Time Machine shares (measured 2026-08-15: the router's
`_adisk._tcp` TXT advertises two disks, one per user).

One partition and one share each, and privacy between users rests on Samba correctly
honoring a "Read-Write User = alice" line in a config file. A Samba bug, a misedit, or a
path-traversal flaw crosses that boundary.

Ours would be one adapter instance per user, each holding one directory capability, and one adapter
cannot name another's partition. Not an ACL check that could be wrong: no capability, no path, no way
to express the request. That is the security claim of the whole project, stated in terms of something
calef actually relies on. It makes this the best demonstration target on the roadmap.

It also means milestone 56 (secrets, credentials, and the entropy to make them safe) holds three
identities, not one, from the start. It was built that way: the store's capacity is three, and the
fourth `PUT` is refused with `FULL` rather than silently replacing somebody, which the tests show.

## mDNS is required after all, measured 2026-07-30

I hoped this could be dropped, on the grounds that calef adds the share manually and the SMB-side
`fruit:time machine = yes` might be what makes it acceptable. Measured, and no: `dns-sd -B
_adisk._tcp` on his network returns `GL-BE9300` in `local.`. So the router runs an mDNS responder and
advertises itself as a Time Machine target. The reference implementation does it. The only way to
prove it unnecessary would be to disable it on a working family backup system, which is not a trade
worth making. Assume required.

So this milestone carries two protocols: SMB3 on TCP, and mDNS/DNS-SD on UDP multicast (`5353`,
`224.0.0.251` / `ff02::fb`). The latter reuses the DNS wire format, plus DNS-SD's PTR/SRV/TXT
convention and the probe-before-claim rules. Check whether smoltcp gives us multicast group
membership before estimating it.

One structural detail from the measurement: there is one `_adisk._tcp` instance for all the shares.
The advertisement is per server, with the disks enumerated inside its TXT record (`dk0=…`, `dk1=…`),
not one announcement per share. Emitting one per share would be wrong.

Three service types are in scope:

- `_smb._tcp`, the server;
- `_adisk._tcp`, the Time Machine flags, which is what populates the backup-disk list;
- `_device-info._tcp`, the model string, where `fruit:model = TimeCapsule` surfaces and which sets
  the icon macOS shows.

Still to capture, and free: `dns-sd -L GL-BE9300 _adisk._tcp local` prints the actual TXT keys and
flag values. Those bytes are the specification for what we must emit, and having the working ones
beats deriving them from the RFC.

## What Time Machine over a network requires

- SMB3, not AFP. Apple deprecated and removed AFP serving; SMB is the supported path.
- Apple's SMB extensions, the `AAPL` create context, which is what Samba implements as `vfs_fruit`.
  Without it macOS will mount the share but not accept it as a backup destination.
- mDNS/Bonjour advertisement, `_smb._tcp` plus `_adisk._tcp` carrying the Time Machine flags, or the
  share is not offered in the Time Machine UI. That is a second protocol (mDNS) on top of the first.
- Durability semantics macOS trusts. Time Machine writes a sparse bundle and depends on the server
  honoring flushes. This is the same clause §42 makes central, arriving as a compatibility
  requirement: a server that lies about durability produces backups that cannot be restored. Built
  ([what-landed.md](what-landed.md)).

Not a port of Samba over the §31 (the foreign-language seam) C seam, because it does not survive
contact. It is superficially the right move, since we already confine a component we did not write
(RedoxFS) and the seam exists for exactly this. But Samba assumes `fork`, threads, and an enormous
POSIX surface, and milestone 52 (subshells without `fork`) records that we have no `fork` and that
getting one is not cheap. That is an honest limit of the C-seam story rather than a gap nobody
noticed.

Decided 2026-08-15: implement the subset of SMB3 that Time Machine needs, not a general SMB3 server.
The general server is the question of milestone 137 (the share as a Mac file server) to reopen if it
ever wants one. Effort to BUILT is bounded by the hardware gate of milestone 53 (the board's own
peripherals), Steps A through C in [path-to-built.md](path-to-built.md).
