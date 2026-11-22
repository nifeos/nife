# SMB: the network file service a Mac mounted, and why it is no longer here (milestone 54)

The code this note describes was removed from the tree on 2026-08-30, on calef's decision. Every
present tense below is the tree as it stood at commit `685900ec`, which is the last commit that holds
it; nothing described here can be built or run from `main` any more. The note is kept, and kept in
full, because the thing it records is evidence rather than documentation. Milestone 54 (network) is the only
time this project's first principle was realized end to end: a real customer's real machine running
a real workload against this kernel. And AGENTS.md's own rule is that a finding worth keeping lands in
`notes/` rather than in a commit nobody will check out. Read it in the past tense. It is a record of
what this system did.

## What was demonstrated, and when

| date | what a machine did, not what a test asserted |
|---|---|
| 2026-08-15 | **A real Mac mounted it.** macOS 26's own `mount_smbfs` against the QEMU guest: the share mounted, `ls` listed it, both fixture files read back byte-correct, the volume arrived read-only (macOS honored `READ_ONLY_VOLUME` and refused a write client-side), a clean unmount worked, and a second mount proved the listener re-armed for a real client rather than only for the test prober. |
| 2026-08-15 | The `file_system_contract`-backed share: bytes a *different in-guest process* had put on RedoxFS came back over TCP, so the chain RedoxFS -> `file_system_contract` -> `Share` -> SMB2 -> TCP was checkable rather than asserted. |
| 2026-08-16 | The write path, gated on both ISAs by a file the host wrote over SMB2 and a different in-guest process read back through the FS server. Never met a real Mac (see BUGS). |
| 2026-08-16 | `file_system_contract` grew `STATFS` (op 18); subdirectories, with `smb_proto::path` parsing a share-relative path once at the wire's edge. |
| 2026-08-17 | The `AAPL` create context and the `FULL_SYNC` Time Machine flag. Never met a real Mac. |
| 2026-08-17 | **Identity**: an NTLMv2 proof verified against milestone 65's credential service, with the kernel reading the shared frame through the direct map afterwards and requiring the key to be absent. |
| 2026-08-19 | Throughput measured through a real SMB client: **write 4.8x, read 2.4x** from raising the file transfer from 4 KiB to 64 KiB. See the throughput section. |
| 2026-08-24 | The session/connection split (milestone 152's first buildable piece), the shape `session_reviver` still carries. |

The one number that survives the removal is there because it was measured rather than argued.
Raising `file_system_contract`'s transfer from one page to sixteen took an SMB write from 0.065 to 0.31 MiB/s
and a read from 0.15 to 0.36 MiB/s, debug build under QEMU. The ratio transfers; the rate does
not, and the note refused to convert it into a backup's wall clock for that reason. That refusal is
still the right call and is worth copying.

## The scale of what was deleted

Counted from the tree at `685900ec`, by the lane that removed it:

| piece | lines |
|---|---|
| `crates/smb_proto/` (10 modules: framing, server state machine, `share`, `path`, `ntlmssp`, `spnego`, `create_context`, `apple`, `authenticator`, a client) | 7,218 |
| `user/src/smb_server.rs` (the adapter program) | 1,279 |
| `bench/smb-throughput.sh` | 100 |
| `crates/ntlm/` (removed in the same pass; 10 host tests, all [MS-NLMP] published vectors) | 469 |
| the host prober and `smb-serve` subcommand in `xtask/src/main.rs` | ~226 referencing lines |
| the two-ISA QEMU gate wiring in `kernel/src/user/` | ~170 referencing lines |

86 `#[test]` host tests in `smb_proto` and 10 in `ntlm` went with it, plus one QEMU check per ISA
and the `assert_smb_write_landed` / `assert_smb_held_no_key` kernel-side assertions. Four external
dependency crates left the graph: `md4`, `md-5`, `hmac` and `digest`, each of which had `ntlm` as
its only consumer.

## Why it was removed

The customer moved. Milestone 55's premise was a Time Machine target for the family's Macs, served by
this kernel. That customer now backs up with borg over SSH on cordoba, so journey 2 is retired and the
workload SMB existed to serve does not exist. calef ruled on 2026-08-30 to remove the implementation.
The maintainer argued for keeping the working core as evidence and he reaffirmed. What that argument
was reaching for is this note, which is the cheaper form of the same thing. 8,600 lines of protocol
code carried on `main` cost every future refactor, every dependency audit and every parity sweep,
where the record of what it proved costs a file.

What was deliberately kept, and why, because the boundary is the interesting part of a removal:

- `crates/multicast_dns_protocol` and `components/src/multicast_dns_responder.rs`. Service discovery is a standalone service
  and is useful without a share to advertise.
- `crates/credentialer`, `credentialer`, `session_reviver`, and milestone 49 (users,)'s and 65's identity work,
  minus the NTLM half (see the section below). The credential service's headline property (a server
  answers an authentication without ever holding the key) is proven by `credentialer_test_client`
  against the password verifier and never needed the SMB adapter.
- Milestone 107 (socket)'s socket work, which is what lets anything accept a connection.
- `file_system_contract`'s `STATFS`, `SYNC` and `RENAME`, and the block server's
  `VIRTIO_BLK_T_FLUSH`. SMB is what motivated them; they are file-service verbs and stand on their
  own.

## The NTLM half went with it, and that is the transferable lesson

`crates/ntlm`, the NTLM path through `crates/credentialer`, two `credential_contract` opcodes, and
`md4`, `md-5`, `hmac` and `digest` were removed in the same pass. DECISIONS §79 (holding
password-equivalent material) had approved them, and two known-broken hash functions, for NTLMv2
compliance. That reason was sound and entirely contingent on there being a protocol to comply with.
Once the SMB implementation was gone, the tree carried a password equivalent and broken hashes for a
consumer that no longer existed, and every gate stayed green. The lesson: re-check a dependency when
the reason it was taken changes. A decision record that names its own premise is where that check
can happen, and §79's premise is now stale and wants an architect's amendment. The removal was safe
because nothing was ever stored and the opcode spaces are not positional.
[`smb/ntlm-removal.md`](smb/ntlm-removal.md) has the full list and both facts.

## What never worked, and it is the honest half

Recorded here rather than lost with the code, because a future reader deciding whether to build this
again should know what the last attempt did not reach:

- The write path never met a real Mac. The 2026-08-15 mount was read-only. The write half was
  gated by a conforming client this tree wrote, which is not `smbfs`.
- No Mac ever saw the `AAPL` answer, so nobody knows whether macOS's Time Machine UI would have
  offered the share. That needed the kernel on hardware on a real network segment; slirp carries no
  multicast, so the mDNS discovery half was equally unproven in situ.
- Finder's Connect to Server dialog was never clicked through. Only the command-line mount ruled.
- The demo boot admitted guests to a writable share, and no boot could be told a password: the
  only provisioner in the tree was a test program carrying [MS-NLMP]'s published fixture. That was
  milestone 131's whole subject and it never landed.
- Sessions were never signed, so a proven session was unprotected afterwards.
- The server challenge was a clock, not entropy, so two connections in one tick repeated it.
- Apple metadata (streams, forks, `READ_DIR_ATTR`) was never implemented; §99 deferred the decision
  and milestone 137 was minted to hold it.

Everything else the implementation did not do is still listed in the BUGS section below, unedited,
which is where a reader should go for the full list.

## The note as it stood before the removal

The rest of the original note is kept, unaltered in substance, in [`smb/`](smb/README.md). Read
every present tense in it as "as of `685900ec`".

- [`smb/adapter-as-it-stood.md`](smb/adapter-as-it-stood.md): the adapter, what was demonstrated
  at each step, and the pieces, including the `Share` trait's three backings.
- [`smb/wire-decisions.md`](smb/wire-decisions.md): every expensive-to-reverse wire choice and its
  reason, from SMB 2.1 only and NTLMv2 to refusing read-only at the protocol layer, parsing a path
  once, and the SMB1 probe a real Mac sent.
- [`smb/apple-extensions.md`](smb/apple-extensions.md): the `AAPL` create context, the bits claimed, and the
  durability gap `FULL_SYNC` exposed and milestone 55 (Time Machine) closed with a real device flush.
- [`smb/throughput.md`](smb/throughput.md): the 64 KiB transfer, measured through a real client, and
  the socket contract's 4080-byte chunk that remained.
- [`smb/testing.md`](smb/testing.md): the host tests, the two-ISA gate leg by leg, and the commands
  that ran it, including the only written-down mount instructions a real Mac was given.
- [`smb/remaining-work.md`](smb/remaining-work.md): the work lists as they stood.

## BUGS

Every limitation of the removed program, kept in full, is [`smb/limitations.md`](smb/limitations.md).
It is where to start if anyone builds a Mac-mountable share here again. The headline entries:

- A 64 KiB SMB message crossed the socket contract seventeen times each way, which was the dominant
  cost of a transfer once the file transfer reached 64 KiB.
- The write path and the `AAPL` answer never met a real Mac, and Finder's dialog was never used.
- The demo boot admitted guests, because nothing could tell it a password; an authenticated share
  covered one account; sessions were not signed; the server challenge was a clock.
- Apple metadata was not implemented at all (no streams). Time Machine does not use that surface, and
  macOS's `._name` sidecar files already landed on the image.
- `ReplaceIfExists = 0` was ignored, so a rename always replaced; fixing it needs §42 (a filesystem
  declares what it offers and must be truthful) amended.
- Timestamps were thrown away, names were folded to lower-case ASCII, and a dropped connection cost a
  15 s stall.
