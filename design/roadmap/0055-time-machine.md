---
status: REMOVED
raised: 2026-07-31
---
# 55. Time Machine: SMB3 with Apple's extensions, and mDNS

Removed 2026-08-30. The premise was retired the same day. calef decided to remove
the SMB implementation: the customer this milestone existed for backs up with borg over SSH on
cordoba, so journey 2 is retired and there is no Time Machine target to finish. The status word is
the vocabulary's closest fit rather than the right one; there is no word for "retired", and minting
one is an architect's.

What is built and stays: the discovery half (pull request #246, 2026-08-16). It is a responder
program that binds UDP 5353 through a grant. It announces `_smb._tcp`, `_adisk._tcp` and
`_device-info._tcp` from a configuration document carrying the values measured off calef's router.
It answers browses and legacy one-shot queries per RFC 6762 section 6.7, gated on both ISAs. It holds
a report endpoint, the stack endpoint and a budget and nothing else. Service discovery is a
standalone service and is useful without a share to advertise.

What is gone: everything that needed an SMB server. The `AAPL` create context and the
`FULL_SYNC` Time Machine flag, the throughput work, the authenticated share. notes/smb.md records
all of it, including the two things this milestone most wanted and never got: no Mac ever saw the
`AAPL` answer, and nobody ever proved macOS would offer the share as a backup destination. Those
needed the kernel on hardware on a real network segment, which never happened.

What survived into the rest of the tree, because it was file-service work rather than protocol
work: `filesystem_proto`'s `STATFS`, `SYNC` and `RENAME`, and the block server's real
`VIRTIO_BLK_T_FLUSH`. See notes/smb.md on the one gate that was lost with the SMB verify role.

## The record kept from before the retirement

The block as it stood before the retirement is kept, because it records decisions and measurements
rather than plans. Its full text moved on 2026-10-12 (UTC) into three appendices under §212 (a prose
budget); this page is complete without them.

- [what-landed.md](0055-time-machine/what-landed.md): the SMB half as built from 2026-08-17 to
  2026-08-19, with the flush witness, the throughput table and the identity it inherited.
- [path-to-built.md](0055-time-machine/path-to-built.md): the 2026-08-22 scoping pass, its three
  hardware steps, and the `ReplaceIfExists` finding that led to §129 (whether `RENAME` grows a
  `NOREPLACE` flag).
- [reference.md](0055-time-machine/reference.md): calef's router as the reference implementation,
  the requirements its Samba stanza decodes into, the three family users, why mDNS is required, and
  the original framing with its corrections.

The block's own summary from the start: this was the actual goal, and probably the largest single
piece of work in the project. It was recorded at full size because the failure mode was starting it
while imagining it is "a file server".

### What was decided

- The SMB3 subset Time Machine needs, not a general server (calef, 2026-08-15). A general server was
  left to milestone 137 (the share as a Mac file server).
- Apple metadata: §99 (where Apple's metadata lands: stream or sidecar), 2026-08-18, chose option 1.
  The server claims no named streams and macOS writes `._` sidecars itself, at zero new code. A Time
  Machine backup is a sparse bundle of directories and band files and never touches this surface.
- `ReplaceIfExists`: §129 priced a no-replace flag in the unclaimed bits 63:56 of `fs::rename_dst`.
  calef's call was "build it when we have a customer". §42 (a filesystem declares what it offers and
  must be truthful) was amended to correct its racy-emulation reason and still declines the feature.
- mDNS is required, measured 2026-07-30: `dns-sd -B _adisk._tcp` returns the router, which
  advertises one `_adisk._tcp` instance listing every disk in its TXT record.
- Porting Samba over the §31 (the foreign-language seam) C seam was refused, because Samba assumes
  `fork` and milestone 52 (subshells without `fork`) records that getting one is not cheap.

### What was built, in order

| date | pull request | piece |
|---|---|---|
| 2026-08-16 | #246 | mDNS discovery, written rather than vendored (stays) |
| 2026-08-17 | #292 | the `AAPL` create context: claims `UNIX_BASED`, `FULL_SYNC` and the model `TimeCapsule`, declines five others with reasons |
| 2026-08-17 | #274 | identity from milestone 54 (a network file service a Mac can actually mount): NTLMv2, with no key in the server |
| 2026-08-18 | #311 | durability: `blk::FLUSH` (op 4) is a real `VIRTIO_BLK_T_FLUSH`, `fs::SYNC` (op 19) backs SMB2's `FLUSH`, witnessed by a device flush count |
| 2026-08-19 | | throughput: the 64 KiB transfer reaches the mount, write 4.8x (0.065 to 0.31 MiB/s), read 2.4x (0.15 to 0.36) |

The `fs_proto` xattr layer (ops 14-17) came from milestone 57 (partitioning and formatting a real
drive, and extended attributes), not from this block. Two apparent blockers are not: `RENAME`
exists, and `fruit:posix_rename` needs no work.

### What stood open when the premise was retired

One gate, and no architect fork: contact with a real Mac. It was blocked on hardware, through
milestone 53 (the board's own peripherals: network and storage on real silicon), in three steps.

1. A real NIC driver. The near-term path was riscv64, through the JH7110 GMAC on the VisionFive 2
   and milestone 163 (the JH7110's PCIe root complex) for NVMe, since no aarch64 board existed yet.
2. Multicast on a real segment: `dns-sd -B _adisk._tcp` from a real Mac against the responder.
3. A real backup and a real power cut, which needs 53's storage half. A narrower mount, small write
   and clean unmount needed only step 1.

Milestone 131 (a share is configured, not compiled, and its secret arrives from somewhere) was
adjacent and not blocking. It closes the guest share, which matters only once this becomes the
family's real backup target.

### Limitations and BUGS as recorded

- Nothing met a Mac. QEMU's user-mode networking cannot carry multicast to the host. An injected
  query escaped slirp and came back from the real router, a false green the gate caught with a
  source-address filter.
- The sync was device-wide, not per file, and nothing fenced, because `fs_proto` has no ordering
  primitive. A device that lies about its own flush is outside what a protocol can check.
- `socket_proto::DATA_MAX` (4080 bytes) became the dominant transfer cost, about seventeen crossings
  per 64 KiB message. `smb_proto::MAX_TRANSACT` (64 KiB) also capped any larger transfer. No
  end-to-end backup time was offered, because a debug build under QEMU is the wrong wall clock.
- The boot a person ran still admitted guests, because nothing could tell a running system a
  password. That provisioning path is the shape of milestone 56 (secrets, credentials, and the
  entropy to make them safe), whose store holds three identities.
- `ReplaceIfExists = 0` was ignored on rename, so a rename could clobber when the client asked it
  not to. notes/smb.md's suggested fix contradicted §42, a note gone stale the way a block can.

## Follow-on

- **Milestone 137.** The Mac file-server workload. Its subject went with the SMB implementation, and
  its own index row says the block needs retiring, which is an architect's call.
- **Recorded.** `notes/smb.md` keeps everything the removal took, including the two things this
  milestone most wanted and never got: no Mac ever saw the `AAPL` answer, and nobody proved macOS
  would offer the share as a backup destination. Those needed the kernel on hardware on a real
  network segment and are facts about what was not reached, not work waiting for a lane.
- **Refused.** Reviving SMB behind a different server (porting Samba over the §31 C seam) is
  refused, and was before the removal. Samba assumes `fork`, threads and an enormous POSIX
  surface, and milestone 52 records that we have no `fork` and that getting one is not cheap.

## Index row

Premise retired 2026-08-30: the customer moved to borg over SSH, journey 2 is retired, and the
SMB half was removed with milestone 54. The mDNS discovery half is built and stays. What the
backup path reached, and the several things it never did, are in notes/smb.md
