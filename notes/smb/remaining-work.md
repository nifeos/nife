# What remained for the SMB adapter, as the lists stood

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/smb.md](../smb.md). The code was removed on 2026-08-30; these are the two work
lists as they stood at `685900ec`, kept because they say what was done and what a rebuild would face
first. The first list's numbering is as written, including its duplicated item 5.

## What remained for milestone 54 and beyond, in order

This is milestone 54 (a network file service a Mac can actually mount).

1. ~~The fs_proto-backed share~~ Done (2026-08-15): `smb_server::FsShare`, gated on both ISAs by the
   seeded-file exchange. The rights split of milestone 47 (navigation and naming), a directory
   capability that may write backups but not delete them, becomes expressible the moment writes
   exist.
2. ~~The write path~~ Done (2026-08-16): `WRITE`, all six create dispositions, `SET_INFO`'s
   end-of-file, rename, disposition and basic classes, delete-on-close, the `Share` trait's widening
   and its error channel, and the handle cache (the id is the FS server's handle). It is gated on both
   ISAs by a write the guest reads back through the FS server in a different process. Milestone 47's
   rights split is now expressible end to end. A directory capability carrying `WRITE | CREATE` and
   not `REMOVE` gives a share that takes backups and destroys nothing, and the FS server enforces it
   under an adapter that never sees the mask.
3. ~~A `statfs` verb for `fs_proto`~~ Done (2026-08-16): `fs::STATFS`, op 18, and the SMB volume
   classes report the image's real numbers through it. The wire decisions are in
   [wire-decisions.md](wire-decisions.md) and in pull request #255.
4. ~~Subdirectories~~ Done (2026-08-16): `smb_proto::path`, the `Share` seam's directory ids and its
   `mkdir`/`rmdir`/`open_dir` verbs, and the adapter's per-component walk. It is gated on both ISAs by
   a directory the host makes over SMB2 and a different in-guest process descends into.
5. ~~`fruit:posix_rename`~~ Already true, checked 2026-08-17 rather than built. The two behaviors
   Samba's `fruit:posix_rename` switches on are renaming onto an existing name, and renaming a file
   that is open. The first is `fs_proto::fs::RENAME`'s documented semantics already ("if the
   destination name exists it is replaced, provided it is the same kind"). The second cannot fail
   here, because this server enforces no share modes at all. `ShareAccess` is never consulted, and
   there are no oplocks and no leases, so there is no sharing violation for POSIX semantics to be an
   exception to. The block of milestone 55 (Time Machine) listed this as work; it is not, and the
   real gap next door is `ReplaceIfExists` in [limitations.md](limitations.md).
6. Identity: the NTLMSSP proof check against the `credentialer` service of milestone 65 (a secrets
   service), so a share can be more than guest-readable. The seam is marked in `smb_proto::ntlmssp`.
   Writes raised the stakes: guest means everyone, and on a writable share that means everyone may
   change it.
5. ~~Identity~~ Done (2026-08-17): `smb_proto::authenticator`, the AUTHENTICATE parse in
   `smb_proto::ntlmssp`, and `smb_server::CredentialAuthenticator` over milestone 65's verify
   endpoint. It is gated on both ISAs by a host process computing a real NTLMv2 proof over the guest's
   own challenge, with the two refusals asserted beside it, and the kernel checking the frame
   afterwards. The wire decisions are in [wire-decisions.md](wire-decisions.md) and in pull request
   #274.

## What remained after milestone 54, in order

1. A provisioning path, and it is the one that matters. Until it exists, the boot a person runs
   (`smb-serve`) admits guests to a writable share. Nothing in the tree can tell a running system a
   password: the only provisioner is a test program with a published fixture in it. That is the shape
   of milestone 56 (secrets, credentials, and the entropy to make them safe)
   (design/roadmap/0056-secrets-and-entropy.md), and identity landing made it the head of this path
   rather than a supporting item.
2. The resource should be implied by the capability, not named in the request. The adapter is
   configured with a resource name, which is one authority more than it needs; the endpoint should
   *be* the credential for one resource. It is the argument of DECISIONS §27 (the filesystem service)
   applied to `cred_proto`. It is a change to a contract two programs agree on, so it is an
   architect's.
3. Signing, which is what `SessionBaseKey` is for, and the reason the credential service publishes
   one. A proven session is currently unprotected once established.
4. An entropy capability for the server challenge, which is `now()` today.
5. A frame per verify client, so two shares can be two adapters.
