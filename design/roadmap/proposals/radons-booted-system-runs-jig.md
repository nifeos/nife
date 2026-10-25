---
status: PROPOSED
raised: 2026-10-24
milestone_dependencies: 53, 801
decision_dependencies: unwritten
machine_requirements: riscv64 silicon with a wired network on the internet
specific_machine: radon (801's riscv64 leg is ruled to run on it, and its TRNG and NIC are JH7110 parts)
needs_person: yes
---
# radon's booted system runs `jig`: entropy, a disk and the time

Raised 2026-10-24 (UTC) by `maintainer/record-2026-10-24-b`, recording calef's ruling that milestone
801 (packages over the internet) closes on real silicon on three machines, radon among them. The
lane was asked to make `jig update` and `jig install ioping` runnable on radon. Checked against the
tree and radon's last ordinary boot, they are not, for three reasons, and the third is a design
question. This proposal is that work. The bench step it ends with is written:
[notes/packages/jig-on-radon.md](../../../notes/packages/jig-on-radon.md).

## What radon's ordinary image has today

From `bench/radon-2026-10-22/53-ordinary-and-reboot.log`, `script/board-image --tftp`, features
`board`:

| what `jig` needs | radon | the line |
|---|---|---|
| a NIC and a lease | yes | `network : the JH7110's Ethernet port, link up`, then `DHCP leased <address>` |
| `jig` in the archive | yes | `SHIPPED_STD_PROGRAMS` refuses an archive without it |
| basalt's index sources | yes, when built so | `NIFE_INDEX_SOURCES=basalt` reaches `initial-ramdisk-riscv` through `xtask/src/package.rs` |
| entropy at the prompt | no | `entropy : NONE. No virtio-rng device, and this CPU has no seed instruction` |
| a name resolver | no | `no name resolver (no lease name server, no entropy, or not vouched)` |
| a file service | no | no `directory grants` line; `fs_rights` is 0 with no disk |
| the wall clock | no | the clock service drives PL031, goldfish and CMOS; radon has none of them |

## The work

1. Entropy at the hand-over. The kernel's tour already wires the JH7110 TRNG through
   `entropy_service::ensure(.., Bus::Jh7110)` and serves a client from it (`self-test : entropy
   ok`). The hand-over grants only virtio-rng or the seed instruction
   (`boot_instruction_entropy`), so the progenitor gets nothing. The `entropy` self-test's own
   doc comment says the hand-over would use the TRNG, so this is the code catching up with its
   comment. Grant slot 16 from the TRNG's service when the tree names one, and make the
   progenitor's announcement name the source instead of "the CPU's seed instruction". With entropy,
   the lease's name server gives the resolver too. §137 (a hardware TRNG with no published
   health-test claim) is `PROPOSED`; it blocks a claim about these bytes, not their use.
2. A disk the file server can mount. §260 (radon's block server serves a USB drive) puts radon's
   persistent filesystem behind milestones 163, 242 and 859. Until then, a bench image can carry a
   blank RedoxFS image in its archive, made host-side by `tools/redoxfs_host` as every fixture is.
   The kernel can serve it from RAM through a small block server answering
   `filesystem_contract::blk`, the way the NVMe and MSHC servers do. The archive entry is the
   switch, so no CI boot changes. Nothing survives a power cycle, and the bench note says what that
   leaves unproven.
3. The wall clock. TLS checks a certificate's dates against the clock page, and radon's reads
   `UNKNOWN`. Which process may set the time at boot is calef's, below.

## Needs calef: where a machine with no RTC gets the time

`notes/ntp.md` refuses an `init` endowment for the NTP client: ambient time synchronization would be
ambient authority, as visible as reaching the network. So the choice is a decision, not a
lane's.

- A (recommended). The progenitor runs `network_time_client` once at boot, when the clock is
  `UNKNOWN` and the boot has a network, entropy and a resolver, and announces it like the
  resolver. The server is the lease's (DHCP option 42), else one pool name the image's sources
  carry. The client already holds only a proposal right; the service bounds what it may change.
- B. A build-time floor: the clock service starts at the image's build date when there is no RTC.
  It needs no network, but a certificate issued after the build is "not yet valid", so a Let's
  Encrypt chain breaks an image within about two months.
- C. The owner sets the time at the console, through a verb that proposes it to the clock service.
  Honest and manual, and every radon boot then needs a person before `jig update`.

A does not settle NTS (`notes/ntp.md` keeps that a separate fork). An Oracle A1 instance has the
same gap, and may close it without this ruling: its firmware has a clock the loader can read before
it exits boot services
([the A1 proposal](an-aarch64-cloud-instance-drives-its-pcie-devices.md)).

Reuse: everything but the RAM block server exists. The TRNG's service is milestone 159's, the
resolver 892's, `network_time_client` 290's, the RedoxFS image tool and server milestone 32's. The
block server is about a hundred lines over `filesystem_contract::blk`, modeled on
`virtio::run_blk_server`; no RAM-disk server exists in the tree to take.

## Exit criteria

1. A radon boot built as the bench note says reaches `$` with the entropy service, the name resolver
   and a file service announced.
2. On that boot, `jig update` reads `basalt.nifeos.org`'s index over pinned TLS, and `jig install
   ioping` fetches, judges and installs it. The log is committed under `bench/radon-<date>/`.
3. Items 1 and 2 are gated under QEMU riscv64 where QEMU can show them: the RAM disk in place of
   virtio-blk, booted with the test peer. The TRNG and the DesignWare port have no QEMU model.

## BUGS

- The RAM disk is a stand-in. Milestone 801's riscv64 leg on it proves the fetch, the digest check
  and the installer on radon's own NIC, not that radon keeps an install.
- None of the three items has been built, so the predictions above are readings of the tree.
