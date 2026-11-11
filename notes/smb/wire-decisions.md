# The wire decisions, and why

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/smb.md](../smb.md). The code it describes was removed on 2026-08-30; read every
present tense as "as of `685900ec`". These were the expensive-to-reverse choices (AGENTS.md,
"anything two programs agree on"), listed so review could happen where the cost is.

## Transport, dialect and identity

- Direct TCP on port 445, with the 4-byte zero-type NetBIOS-shaped prefix. No port 139, and no
  NetBIOS session service.
- SMB 2.1 (`0x0210`), only. 2.0.2 predates features macOS wants. The 3.x family drags in signing
  enforcement, encryption and `VALIDATE_NEGOTIATE_INFO`, none needed for a first mount. macOS
  negotiates 2.1 happily; it is the dialect of a decade of NAS boxes.
- NTLMv2, and guest only when a boot asks for it. The server answers the NTLMSSP dance, raw or
  wrapped in SPNEGO, which is how macOS sends it. It takes the AUTHENTICATE apart, and asks the
  `smb_proto::authenticator::Authenticator` seam whether the proof checks out. Three answers give
  three wire outcomes. `Authenticated` is `STATUS_SUCCESS` with `SessionFlags` clear. `Guest` is
  `STATUS_SUCCESS` with `SESSION_FLAG_IS_GUEST` set, the honest label for "nothing was verified".
  `Refused` is `STATUS_LOGON_FAILURE`, with the connection left open so a client can retry, which
  macOS does after prompting.
- `STATUS_LOGON_FAILURE` (`0xC000006D`) is the answer for a bad proof *and* for an anonymous client,
  and it is one status on purpose. Distinguishing them would make session setup an oracle for which
  accounts a store holds. It is what Windows and Samba answer, so a real client's retry logic already
  knows it.
- An anonymous AUTHENTICATE is a distinct thing from a failed one. `Authenticator::anonymous` answers
  it, defaulting to a refusal, and that default is the entire difference between a guest share and an
  authenticated one. It matters because `mount_smbfs -N` and this tree's own prober both send an
  AUTHENTICATE with every field empty. A server that read "no proof" as "nothing to check, therefore
  fine" would admit exactly the caller identity exists to shut out.
- The seam carries no key material in either direction, and `SessionBaseKey` is deliberately not in
  it. An `Attempt` is the server challenge, the presented account and domain (UTF-16LE, as they
  arrived), the `NTProofStr`, and the client's blob. All are public, or a MAC that is worthless
  without the key. The session key of [MS-NLMP] section 4.2.4.1.2 is what a *signing* server would need. This
  one does not sign, so the adapter never asks the credential service for it. Adding it later is a
  widening with a stated reason, rather than a field somebody has to justify removing.
- The presented account name is not a lookup key. `cred_proto::verify::NTLM_PROOF` names a
  *resource*, which the adapter is configured with. So the wire's only contribution is challenge,
  blob and proof. The account is bound cryptographically instead. The stored `NTOWFv2` was derived
  over the account that owns the resource, so a client claiming a different name derives under a
  different key and fails, and nothing anywhere compares strings. See the BUGS on why the resource
  being a *constant*, rather than an implication of the endpoint, is the part that is wrong.
- Sessions are still not signed. A session is proven at setup and unprotected afterwards. Identity
  buys authentication of the client, not integrity of the stream; that is in the BUGS.
- `MaxTransactSize`/`MaxReadSize`/`MaxWriteSize` = 65536. That is the floor mainstream clients are
  written against, and exactly the static buffer the allocator-less server carries.

## The share, and read-only

- One share, named `share`, a tree of directories and files, writable when the boot says so. The
  direction is `smb_server`'s `arg2`. The write path grew it from a flag into three values (fixture,
  fs-backed read-only, fs-backed read-write), because "which backing" and "which direction" are two
  questions and a boolean answered only one. Both boots that exist wire read-write.
- Read-only is refused at the protocol layer, not at the filesystem. Every mutating command asks
  `Share::writable()` *before* the backing hears about it. So a read-only share is read-only even over
  a directory capability that would have permitted the write. `Share::writable` has no default, so a
  backing cannot be written without stating its direction. The mutating trait methods then default to
  a refusal, as an independent second line. The status is `STATUS_ACCESS_DENIED` throughout,
  including for the timestamp write a copy ends with, because a partial refusal is worse than a whole
  one.
- `FILE_OPEN_IF` on a read-only share is demoted to `FILE_OPEN`, not refused. "Open it if it is
  there" is answerable without writing anything. Clients that open everything that way would
  otherwise break on a share they are only reading.
- The status a write refusal carries is `ACCESS_DENIED`, not `MEDIA_WRITE_PROTECTED`. It is what the
  read-only mount was proven against with a real Mac, and what the host tests pin. Changing it would
  be a wire change bought with nothing.
- `DesiredAccess` is not gated. A create asking for write access on a read-only share is refused by
  its *disposition*, and the commands are refused by command. Gating the access mask as well risked
  breaking the proven read mount (macOS asks for generic masks it does not use). It would buy no
  property the disposition gate does not already hold.
- A file is named by an opaque id the backing mints, not by its index in the listing. The read-only
  trait could use an index, because nothing reordered the directory. A writable share reorders it on
  every create. The fs-backed share makes the id the FS server's own handle, which retires the
  open-per-request cost the read path recorded.
- `FileAllocationInformation` is a no-op, and `FileBasicInformation` is discarded. Both are successes
  that change nothing, and both are in the BUGS. Preallocation is a hint whose obvious implementation
  (truncate) would zero-extend a file the client is about to fill. And there is no clock capability
  here to record a timestamp against.

## Free space

Free space is the image's, through `fs_proto`'s `STATFS` (op 18, milestone 54 (a network file
service a Mac can actually mount)). The record is three little-endian `u64`s in the shared page
(allocation unit, total units, free units), and `r0` is its length. That is `READDIR`'s and
`LISTXATTR`'s existing shape: a reply word carries one `i64`, and this answer is three numbers. The
record's length is its version, so a later field extends it, and a client written against this one
reads its prefix. There is deliberately no version word, because the length already is one.

The verb demands no right, and takes any handle the server minted, file or directory. The handle is
the qualification rather than the subject. Demanding `READ` would leave a write-only grant unable to
answer the one question it has. A backing that cannot ask (the baked-in fixture has no volume) answers
`None`, and the protocol layer falls back to `NOMINAL_VOLUME_BYTES`, stated rather than silent. A
read-only share reports zero free whatever the image says. That is the same statement
`READ_ONLY_VOLUME` makes one field over, and is what makes macOS refuse a write client-side.

## Paths and directories

- A path is parsed once, at the wire's edge (`crates/smb_proto/src/path.rs`). The `Share` seam takes a
  `Path` that cannot be constructed without that parse. What a client is allowed to *say* is wire
  format, so `..` dies where the bytes arrive rather than wherever a backing happens to look at it.
  `.` is refused as well, for a different reason. It is a second spelling of a path, and a handle's
  path is its identity here (rename and delete-on-close both read it back). So one path per name is
  cheaper to hold than a canonicaliser. A forward slash is refused, because SMB's separator is
  backslash, and accepting both would let two clients spell one file two ways. A single leading and a
  single trailing separator are stripped, because clients send `dir\`.
- `fs_proto` resolves a component under a handle and never a path, so the adapter walks. It does one
  `fs::OPENDIR` per component, then the verb on the leaf under the parent's handle. That is the
  contract's shape rather than a limitation to route around. It is why the descent's rights are
  exactly what the share will use rather than `dir::ALL`. `OPENDIR` refuses with `EPERM` when the
  intersection with the parent is smaller than the request. So asking for everything would fail on a
  capability that was correctly narrowed.
- `MKDIR` and `RMDIR` are separate from `CREATE` and `UNLINK`, and `RMDIR` takes only an empty
  directory. A call that removed whatever it found would put a subtree behind one message, and no
  capability check afterwards could undo that. The recursion belongs in whoever is deleting, as a loop
  of individually refusable steps. A client's delete-on-close on a non-empty directory therefore
  leaves it there.
- A directory may be renamed in place but not moved into another directory. That is
  `fs_proto::fs::RENAME`'s own boundary: the cycle guard is an ancestry walk in a server whose stack
  is measured at three quarters used. The one refusal this layer adds is moving a directory into its
  own subtree. It is checked on paths here, because this is the only layer holding both sides as
  paths at once.

## Compounds, Apple, flush and the SMB1 probe

- Compounds are implemented: macOS stats files as CREATE + QUERY_INFO + CLOSE related chains. Credits
  are granted as asked and never accounted.
- The `AAPL` create context is answered, and the bits it claims are the table in
  [apple-extensions.md](apple-extensions.md), from milestone 55 (Time Machine). The chain is walked generically,
  the tag is matched, and the answer echoes the request bitmap and carries exactly the answers it
  asked for. A context this server does not implement is walked past in silence, never refused. An
  unanswered context is how this mechanism says "not implemented", and refusing would trade a working
  mount for a diagnosis nobody reads. A malformed chain is the same: the open still succeeds, with no
  context back.
- `FLUSH` resolves its file id, and then does real work (milestone 55). The file id is checked first,
  so a stale handle is `STATUS_FILE_CLOSED` rather than a blanket yes. Then `Share::sync` is called.
  On the fs-backed share that is `fs_proto::fs::SYNC` and, under that, a `VIRTIO_BLK_T_FLUSH` the
  device completes before the reply. A backing that cannot flush its storage returns an error, and
  the client sees it. See [apple-extensions.md](apple-extensions.md) for why that mattered enough to be worth a
  milestone of its own.
- The SMB1 probe. The machine overruled the assumption that a modern client opens with SMB2. macOS's
  `mount_smbfs` still opens with an SMB1 multi-protocol NEGOTIATE (`\xFFSMB`, command `0x72`, dialect
  strings `NT LM 0.12`, `SMB 2.002`, `SMB 2.???`). The first cut of this server dropped it as
  not-SMB2. That presented as every real mount timing out, while the test suite stayed green, since
  the suite's prober politely opened with SMB2. The fix is [MS-SMB2] section 3.3.5.3.1: answer the probe with
  an SMB2 NEGOTIATE response carrying the wildcard revision `0x02FF`, after which the client
  negotiates properly. The captured bytes are pinned as a host test in `smb_proto::server`, so the
  message a real client sends is now part of the gate. An SMB1-only client (no SMB2 dialect strings)
  is still dropped.
