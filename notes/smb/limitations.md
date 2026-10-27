# The SMB adapter's limitations, kept in full

An appendix to [notes/smb.md](../smb.md). The code it describes was removed on 2026-08-30; read every
present tense as "as of `685900ec`". These are the limitations of a program that no longer exists,
sentence-split for the prose limits and otherwise as written. This is the list a future reader should
start from if anyone ever decides to build a Mac-mountable share here again. It is what the last
attempt knew it had not solved, written while the code was in front of someone.

## Throughput

- A 64 KiB SMB message still crosses the socket contract seventeen times in each direction.
  `socket_contract::DATA_MAX` is 4080 bytes, because a client and `net_stack` share one frame. So
  `smb_server`'s `send_all` and `recv_into` chunk every message through it. Since milestone 55 (Time
  Machine) put the file transfer at 64 KiB, this is the dominant cost of a transfer. An SMB write went
  from ~985 ms to ~206 ms per 64 KiB message, and the filesystem is no longer what is left. It is the
  defect of milestone 138 (close the read gap) step 3, one contract over, and its fix is demonstrated.
  The shared region is one page because nobody declared it otherwise, `socket_contract`'s request
  word already carries a length, and growing the region is the whole change.

  Promotion trigger (§71 (a limitation is promoted when it stops being a fact and becomes a plan)):
  this becomes a roadmap row the moment anyone measures the SMB path on hardware. It is the number
  that will be in the way there, and this entry is the evidence that it is known rather than
  discovered. Nothing has been sized. A socket frame is per socket where the file channel is per FS
  server, so the memory question is a real one, and is not answered here.
- The throughput leg is not in the gate. `smb_throughput_leg` ran only with its own SMB-throughput
  environment variable set (removed with the prober), because a timing is not a pass or a fail on a
  laptop under an emulator. So a regression in this path fails nothing. What guards it instead is
  `bench/smb-throughput.sh` being cheap to rerun. That is rung four of AGENTS.md's ladder, and it is
  written down as such.

## Real clients

- `mount_smbfs` has ruled; Finder's dialog has not. The command-line mount works end to end, and it
  uses the same smbfs kext Finder does. But nobody has yet clicked through Connect to Server and
  browsed the share in a Finder window. Expect that to exercise `CHANGE_NOTIFY` (answered
  `STATUS_NOT_SUPPORTED`; clients degrade to polling), and possibly more `QUERY_INFO` classes.
  Non-guest accounts are untested and would meet signing expectations; connect as Guest.
- Guest means everyone. Every AUTHENTICATE is accepted. Do not put anything on the share the local
  network may not read. There is no rate limiting and no credit accounting.
- No Mac has seen the `AAPL` answer. The context is gated by host tests and by the QEMU prober. The
  prober is a client this tree wrote against the same constants the server answers with, so it agrees
  by construction. Whether macOS's `smbfs` accepts these bytes, and whether the Time Machine UI then
  offers the share, is unproven. It needs the kernel on hardware on the family network; the discovery
  half needs that too, since slirp carries no multicast.
- The write path has never met a real Mac. The 2026-08-15 mount was against a read-only share. The
  write half is gated by host tests and by the QEMU prober, which is a conforming client this tree
  wrote, and a conforming client is not the same thing as `smbfs`. Expect the first writable Finder
  copy to find something, most likely in the `SET_INFO` classes, or in a `QUERY_INFO` class nothing
  has asked for yet. This is the same gap the SMB1 probe fell into.

## Durability and Apple metadata

- `FULL_SYNC`'s durability is now backed, and here is what it still is not. The device is flushed
  (see [apple-extensions.md](apple-extensions.md)), so that entry is closed. What remains:
  the sync is device-wide, never per file. A client that flushes one handle makes the whole image
  durable. That is more work than it asked for, and is the only thing anything below here can do. And
  nothing fences: there is no ordering primitive on `fs_proto`. So a client issuing a write and a
  flush concurrently gets no guarantee between them. A backup client's own sequence is
  write-then-flush, which is why this has not needed one.
- Apple metadata is not implemented at all. No alternate data streams, so no `AFP_AfpInfo` and no
  `AFP_Resource`. Finder labels, resource forks and the extended-listing capability (`READ_DIR_ATTR`,
  deliberately not claimed) all rest on that surface. The layer under them is not missing: milestone
  57 (partitioning and formatting a real drive, and extended attributes) added the four
  extended-attribute verbs to `fs_proto`, ops 14-17. What does not exist is the SMB half: a stream
  name in a CREATE path, `FileStreamInformation` in `QUERY_INFO`, and `FILE_NAMED_STREAMS` in the
  volume attributes.

  The stream-versus-sidecar decision milestone 55's block frames is therefore still open. It is now a
  smaller question than that block assumed, since the layer that was missing when it was written is
  not missing any more. The decision is §99 (where Apple's metadata lands: stream or sidecar), and it
  is waiting on calef, with two findings a reader of this entry should have. Time Machine does not use
  this surface at all: a backup is a sparse bundle, which is directories and band files with no
  extended attributes and no forks. And the sidecar half is already working. macOS's own VFS falls
  back to `._name` files when a server does not claim `FILE_NAMED_STREAMS`, which this one does not,
  and the files land on the image as ordinary bytes. So "not implemented at all" is true of the stream
  surface and false of the metadata reaching the disk.

## Rename

- `ReplaceIfExists = 0` is ignored: a rename always replaces. `FileRenameInformation`'s first byte
  says whether the client will accept clobbering the destination. This server does not read it,
  because `fs_proto::fs::RENAME` replaces an existing name of the same kind and offers no way to
  refuse. So a client that asked for a rename to fail on a collision gets a silent overwrite, which is the
  wrong direction to fail in.

  Corrected 2026-08-22: it is not a fix this layer can answer, and not "add `NOREPLACE` to
  `fs_proto`" either. §42 (a filesystem declares what it offers and must be truthful)
  (design/decisions/0042-truthful-filesystem.md) already decided not to offer `renameat2`'s
  `NOREPLACE`. Its stated reason is that emulating it with link-then-unlink is racy and
  backend-specific. That reason does not describe this backend. `redoxfs_server::rename`
  (redoxfs_server/src/lib.rs) already looks up the destination inside the same `fs.tx` that performs
  the move. Its own doc comment states why that check needs no lock: "the serve loop runs one request
  to completion before it receives the next, so inside this server there is no concurrent observer at
  all." A `replace: bool` read there costs one branch before `tx.rename_node`, in a server that
  already resolves the destination on every call to decide `EISDIR`/`ENOTDIR`.

  The wire side is free too. `fs::rename_dst`'s second word packs a 16-bit handle and a 40-bit length
  into a 64-bit word (`fs_proto::fs::rename_dst`), leaving bits 63:56 unclaimed, so a `NOREPLACE` bit
  costs no wire growth. §42's racy-emulation concern is real for a POSIX host filesystem reached over
  `link`/`unlink`. It is not a description of a from-scratch, single-request-at-a-time server
  transacting against its own B-tree. This is a wire-format change on a verb two programs already
  agree on (`fs_proto::fs::RENAME`). So it needs a decision that amends or narrows §42, which is an
  architect's call and not a lane's; see design/roadmap/0055-time-machine/path-to-built.md for the writeup.

## Identity

- The demo boot still admits guests, so the thing a person runs is still open to everyone who can
  reach the port. `--features smb_serve` wires `SHARE_FS_READ_WRITE`, not `SHARE_FS_AUTHENTICATED`,
  and its banner says so. The reason is not laziness and not a flag: there is no way to *tell* that
  boot a password. The only thing in the tree that provisions the credential store is
  `credentialer_test_client`'s provisioner role, carrying the published fixture of [MS-NLMP] section
  4.2.1. A demo whose password Microsoft printed would be worse than a labeled guest share. What
  closes this is a provisioning path. That is the territory of milestone 56 (secrets, credentials,
  and the entropy to make them safe), and it is the entry on [remaining-work.md](remaining-work.md).
- An authenticated share authenticates exactly one account, because it is configured with one
  resource. Several accounts mean several adapters, one directory capability each. That fits Time
  Machine (one share per Mac), and it is a real limit on anything else.
- The adapter's resource name is a constant naming a test fixture, in
  `smb_server::CredentialAuthenticator::resource`. The right fix is not a configuration string; it is
  a narrower capability. A request that names its resource is the adapter choosing which record to ask
  about, which is one authority more than it needs. The endpoint should *be* the credential for one
  resource, so the name is implied and unforgeable. That is the argument of DECISIONS §27 (the
  filesystem service) applied to `cred_proto`, and it is a change to a contract two programs agree
  on.
- Sessions are not signed. A proven session is unprotected once established, so an attacker on the
  path can inject into it. Nothing here is worse than the guest share it replaces. The honest reading
  is that identity buys authentication of the *client*, not integrity of the *stream*.
- The server challenge is the adapter's `now()`, a clock rather than entropy. Two connections in the
  same tick would repeat a challenge, and a repeated challenge is what makes a captured proof
  replayable. The fix is an entropy capability (milestone 56's service) and one more slot.
- There is one verify page, so one verify client. The credential service maps a single frame for its
  client side, so two SMB adapters would interleave requests in one page and read each other's
  answers. The gates are fine (one adapter, a single-threaded runner), and a deployment with two
  shares is not. The fix is a frame per client, in the service's wiring rather than in the contract.
- No rate limiting and no credit accounting. Nothing costs an attacker anything to retry. An Argon2id
  verification is deliberately expensive, so a login flood is a denial of service against the
  credential service that every other client shares.

## Paths, names and space

- A path costs one descent per component, on every call. `open("a\\b\\c")` is two `fs::OPENDIR`s and
  an `fs::OPEN`, and the two directory handles are opened and closed again for the next call. There
  is no cache, because a cache here would have to be invalidated by every other client of the same FS
  server, and this adapter cannot see them. Reads and writes are unaffected: they go through the
  handle CREATE minted.
- Free space is a forecast, not a reservation. The numbers are the image's now. But two clients
  writing concurrently both see a count that was true when it was read, and a write past the real end
  still fails with `STATUS_DISK_FULL` at the write. That is what `statfs` is everywhere. `STATFS`
  answers about the whole image, never about a subtree. So a share served over a narrow directory
  capability still reports the volume's free space; there are no quotas in this filesystem, so there
  is no smaller number that would be true.
- A directory moved into another directory is refused, and the status is unhelpful.
  `fs_proto::fs::RENAME` answers `EINVAL`, which this share has no word for and reports as
  `STATUS_UNEXPECTED_IO_ERROR`. Renaming a directory in place works, which is the case a client
  performs; moving one between folders in Finder is the failure to expect.
- The reserved characters SMB forbids in a name are not checked (`:` `*` `?` `"` `<` `>` `|`). A
  client that creates `a?b` gets a file called `a?b` on the image, which no Windows client can open
  afterwards. A compatibility gap rather than a safety one; nothing macOS sends contains them.
- Nothing bounds a path's depth, only its total length (`smb_proto::path::MAX_PATH`, 128 bytes). A
  path of many one-byte components is legal, and slow, per the descent cost above.
- A single name is capped at 64 bytes and a whole path at 128 (`smb_proto::path`'s `MAX_COMPONENT`
  and `MAX_PATH`). A handle keeps its own copy of its *path*, and the table lives on the adapter's
  stack: `MAX_HANDLES * MAX_PATH` bytes of the `Connection`. Either bound exceeded is
  `STATUS_OBJECT_NAME_INVALID`, said out loud rather than truncated into some other file's name.
- Only lower-case names are reachable over the mount. The wire folds names to lower-case ASCII before
  lookup, and RedoxFS is case-sensitive, so an upper-case name on the image can be listed but never
  opened.
- ASCII names only. A name with any non-ASCII UTF-16 unit is not found.

## Metadata

- Timestamps are accepted and thrown away. `SET_INFO`'s `FileBasicInformation` succeeds and changes
  nothing: there is no clock capability here, and `fs_proto`'s `FSTAT` carries no times. So a client
  that sets a modification time and reads it back gets the epoch. Refusing it instead would make
  every copy report failure, which is worse.
- `FileAllocationInformation` does nothing, deliberately. Preallocation is a hint, and turning it into
  a truncate would zero-extend a file the client is about to fill.
- All timestamps are zero, since the server holds no clock capability and fs_proto's FSTAT does not
  carry times. macOS renders that as January 1601 or similar nonsense dates. Cosmetic, and honest:
  nothing here has a date to report.
- `FILE_SUPERSEDE` is `FILE_OVERWRITE_IF` with a different `CreateAction`. Doing it properly
  replaces a file's identity, attributes and all, and this model has no attributes to replace.

## Handles, listings and connections

- A handle leaks if a connection dies mid-file. The adapter releases an FS handle at CLOSE, or when
  the connection's state machine is dropped. A connection torn down between a CREATE and its CLOSE
  leaves one handle in the FS server's table for the life of that server. It is bounded per connection
  by `MAX_HANDLES`, and unbounded across them.
- A listing still costs a walk. `QUERY_DIRECTORY` re-walks `READDIR` from cursor 0 per entry, and pays
  an OPEN + FSTAT + CLOSE to learn each size, because `fs_proto`'s dirent records carry name and kind
  only. Reads and writes no longer pay it: the id is the FS server's handle.
- A dropped connection costs a 15 s stall before the listener re-arms (`net_stack`'s bounded `RECEIVE`
  wait). A clean unmount (LOGOFF) costs nothing. One connection is served at a time.
- The test-boot listener is port 7779, not 445. It shares the inbound gate's listen grant range, and
  `hostfwd` remaps ports anyway; the serve boot listens on 445 proper.
- `smb-serve` binds `127.0.0.1:10445` fixed, so two serve boots on one machine collide; the test boots
  pick free ports and do not.
