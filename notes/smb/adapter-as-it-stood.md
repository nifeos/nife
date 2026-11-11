# The SMB adapter as it stood: overview and pieces

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/smb.md](../smb.md). The code it describes was removed on 2026-08-30. This is
the note's account of the adapter as it stood before the removal, unaltered in substance. Read every
present tense in it as "as of `685900ec`", the last commit that holds the code.

## The adapter

The head of the customer path. macOS speaks SMB natively, and the Time Machine target of milestone 55
(Time Machine: SMB3 with Apple's extensions, and mDNS) requires it. So SMB is the one network file
protocol this tree carries; the roadmap block records why NFS and 9P were refused. What milestone 54
(a network file service a Mac can actually mount) builds is the adapter. It is a program holding one
network endpoint and one share, translating SMB2 on the wire into the share seam on the other side.
Its only storage authority is the one directory capability it is granted. So "what can the network
reach" is a statement about its capability table, not about a check it passes.

A real Mac has mounted it (2026-08-15, macOS 26 `mount_smbfs` against the QEMU guest). The share
mounts, `ls` lists it, and both fixture files read back byte-correct. The volume arrives read-only:
macOS honors the `READ_ONLY_VOLUME` attribute, so a write is refused client-side before it reaches
the wire. A clean unmount works. A second mount proves the listener re-arms for a real client, not
only for the test prober. The one correction the real client forced is recorded under "the SMB1
probe" in [wire-decisions.md](wire-decisions.md).

The write path landed on 2026-08-16, and it is gated but not yet Mac-mounted. That run was against a
read-only share, and nobody has repeated it against a writable one; the BUGS say so where it matters.
What exists now is `WRITE`, all six create dispositions, `SET_INFO`'s end-of-file, rename,
disposition and basic classes, delete-on-close, and a share that is writable or not by declaration,
refusing at the protocol layer rather than at the filesystem.

Identity landed on 2026-08-17, which was the last item on this milestone's list. A share can now
require an NTLMv2 proof that the credential service of milestone 65 (a secrets service) accepts. The
SMB server never holds the key that verifies it. It holds one endpoint to a sealed store, and
`smb_proto` takes the `ntlm` crate as a *dev*-dependency, so the shipping protocol code cannot compute
a proof at all. Both ISAs' gates now run an authenticated share. A host process computes a real proof
over the challenge the guest issued, and the kernel then reads the page between the adapter and the
store and requires it to be empty. The demo boot (`smb-serve`) still admits guests, and still says
so, for a reason worth knowing before you read further: there is no way to *tell* it a password. See
the BUGS.

The session/connection split landed on 2026-08-24. It was durable delegation's first buildable piece,
from milestone 152 (durable delegation). `smb_server.rs` was one accept-serve-close loop where
all session state, NTLMSSP proof included, died with the socket. That is the "unprotected afterwards"
line, and meant there was nothing here that could outlive a disconnect even in principle. It now
splits into the transient per-connection protocol handler (`serve_connection`, unchanged), and a
durable `DurableSession`, built once before the accept loop. That is kept alive by the ordinary
parent-with-live-children rule of DECISIONS §16 (object revocation), rather than any new mechanism. No real
scheduled job is registered against one yet: that is milestone 129 (scheduled execution)/#387, and
reconnect-time reattachment, both still open. See `smb_server`'s own module header ("The durable
session") for the full account, and design/roadmap/0152-durable-delegation.md for the design this
closes the first BUGS item of.

## The pieces

| Piece | Where | What it is |
|---|---|---|
| `smb_proto` | `crates/smb_proto/` | The whole wire format: framing, header, every command (both directions since 2026-08-16), NTLMSSP, minimal SPNEGO, create contexts including Apple's `AAPL` (2026-08-17), and the per-connection state machine. Pure logic over byte slices, host-tested, `no_std`. Client-side builders live in the same crate so tests and the prober share every offset with the server. |
| `smb_server` | `user/src/smb_server.rs` | The adapter program: listen/accept through the socket contract (milestone 107), reassemble direct-TCP framing from bounded `RECEIVE` chunks, hand messages to the state machine, chunk the answers back out. |
| The SMB prober | `xtask/src/main.rs` | The host side of the QEMU gate: a real SMB2 client that negotiates, **authenticates with a real NTLMv2 proof it computes itself**, connects the share, opens the seeded file and asserts its bytes, then writes a second file it never reads back, twice over two connections. It is the only party anywhere that knows the password. |
| The authenticator seam | `crates/smb_proto/src/authenticator.rs` | `Share`'s sibling: a trait with no IO, three verdicts, and an `Attempt` carrying only public bytes and a MAC. `NoIdentity` is the guest policy, spelled as a value so a boot has to *say* it wants guests. |
| `CredentialAuthenticator` | in `smb_server` | The implementation that does the IO: one `CALL` on the credential service's verify endpoint. Holds no key and asks for no session key. |

The share behind the adapter is the `Share` trait in `crates/smb_proto/src/share.rs`. There is a
boot-time choice between its implementations: `smb_server`'s `arg2`, which since the write path says
both which backing and which direction.

- `FsShare` (in `smb_server`, where the IPC lives): the real one. The adapter holds a directory
  capability into the FS server (the endpoint IS the capability, DECISIONS §27 (the filesystem
  service)). It answers every `Share` question with `fs_proto` verbs, so what a mounted client reads
  and writes is the RedoxFS image. This is what the test boots and `smb-serve` wire whenever a
  RedoxFS disk is attached, both of them read-write. Landing the read half changed no protocol code,
  which was the seam's whole promise. The write half did change the seam. The two changes are listed
  under the wire decisions, because they are contract changes rather than code.

  The `fs_proto` verbs it uses, at the rights those verbs document, are `OPEN` and `READ`
  (`dir::READ`), `WRITE` and `TRUNCATE` (`dir::WRITE`), `CREATE` (`dir::CREATE`), `UNLINK`
  (`dir::REMOVE`), and `RENAME` (`REMOVE` on the source, `CREATE` on the destination). Nothing was
  invented: the adapter asks, and the FS server refuses what the capability does not carry.
- `FIXTURE` (in `smb_proto`): files baked into the binary, kept as the no-disk fallback. It is what
  lets the protocol path run with no FS service in the boot, and what the host tests drive the state
  machine against, where a share that cannot be wrong is a feature. It is read-only, and it is the
  trait's worked example of a backing that says so. It implements `writable()` as `false` and none
  of the write half, so the trait's defaults refuse everything.
- `MemoryShare` (in `smb_proto`, `#[cfg(test)]`): a writable share in memory, so the write path's
  host tests have something to write *to*. The fixture's argument, one direction over.

The gate proves the distinction rather than asserting it. The combined boot first runs
`fs_test_client`'s seed role, which writes `fs_proto::fixture::SMB_SEED` through the FS server. The
prober then opens that file over the mount and asserts its bytes. Bytes a different process put on
the filesystem through fs_proto, coming back over TCP, is the claim
"RedoxFS -> fs_proto -> `Share` -> SMB2 -> TCP" made checkable. The baked-in fixture could not have
answered it.
