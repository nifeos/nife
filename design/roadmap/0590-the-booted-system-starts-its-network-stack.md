---
status: BUILT
raised: 2026-09-24
built: 2026-09-24
promoted_from: the-booted-system-has-no-network
---
# 590. The booted system starts its network stack

*(Number provisional: minted by the lane, to be confirmed at merge.)* Promoted
from the proposal `the-booted-system-has-no-network`, filed 2026-09-24 by the rung 3a consumer lane
of milestone 198 (a package manager, and the trivial install that makes a second customer
possible). That lane found its package fetch could run only inside the kernel's test harness. Built
2026-09-24 on aarch64 and riscv64 under QEMU; x86_64 has no NIC under QEMU, recorded below.

The proposal was filed at `Gate: DECISION`, naming three questions. The maintainer promoted it for
building with a standing limit (no new syscall, spawner method or wire format), and each question
got the answer the tree already gives in the analogous case. All three are code and cheap to
reverse; calef can overrule any of them without a migration.

## What it does

The progenitor (`crates/system_initializer`) now builds `net_stack` at boot, from a virtio-net
device the kernel grants it, and a program whose manifest declares `network` is endowed a `WRITE`
view of the stack's endpoint. A person at the prompt can reach the network, and one who types a
program that did not declare it cannot.

```text
progenitor: network stack up; DHCP leased 10.0.2.15
...
$ caps network_echo_client --mem 4
  network_echo_client would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 1  untyped   4 pages  split from this shell's budget
    cap 10 endpoint  network  WRITE. it may open outbound sockets through the network
                              stack, and nothing else: it cannot reach the card, cannot
                              listen, and cannot hand the network to anything it spawns.
                              each socket it opens is a capability of its own; it cannot
                              reach another program's sockets
                              a program without this row reaches no network at all
$ network_echo_client --mem 4
  echo peer 10.0.2.9:7777 answered: nife-net!
$ unreachable_network_witness
  network: refused (no capability at slot 10)
```

That transcript is `script/swish-check`'s, on both legs, and it is the milestone's claim. Every
earlier network test started `net_stack` from the kernel harness
(`kernel/src/user/virtio_service.rs`), so none of them said anything about the system a person
boots.

## The pieces

- The kernel grants the NIC at slots 13 to 15 (`kernel::user::boot_virtio_net_device`). That is
  the confined transport, the interrupt, and a DMA page with its physical base in its last eight
  bytes: the virtio-rng trio's shape. The two devices now share one body,
  `boot_virtio_mmio_device`.
- The progenitor builds `net_stack` (`build_net_stack`) straight after entropy and before the
  terminal plumbing, then deletes the trio on every path. The endowment is
  `wire_net_server`'s slot for slot, because that is the one every network test proves.
- A manifest field and a named slot: `grant_plan::Manifest::network` and `NETWORK_SLOT` (10),
  the entropy pattern one service over. `caps` prints the row; `swish`'s host tests fail if a
  second program declares it or if the row appears for a program that does not.
- Two fixtures (both names provisional): `network_echo_client` reaches the runners' echo peer,
  and `unreachable_network_witness` declares nothing, `CALL`s slot 10 anyway, and prints the
  kernel's refusal. The witness is the negative control the confinement claim needs: a spawn
  service that endowed every child prints `REACHED` there and fails the gate.
- The echo peer's address moved into `socket_protocol::fixture`, because two binaries now dial
  it (rule 7).

## The proposal's three questions, answered

- The slot budget. The proposal counted against a sixteen-slot table; the table is twenty-four
  (milestone 230 (`script/shell-check` is red on `main`, on both architectures, and nothing
  says so)). The NIC trio is spent before the boot's peak, and the one endpoint the
  progenitor keeps for life raises the measured peak from 22 to 23 of 24
  (`kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`, updated with the reason). One slot is left.
- Who holds the NIC. `net_stack` holds it directly, as in every harness test, over the MMIO
  transport. That NIC has no IOMMU in front of it, so its DMA is confined by the shadow-ring
  validator alone (DECISIONS §20 (IOMMU-backed DMA isolation: one seam, two arch
  drivers)). The PCIe NIC behind the IOMMU is attached by the runners and left
  unclaimed. The rng made the same MMIO-first choice for the same reason, and widening both is one
  follow-on.
- Listen grants. None. The stack holds `socket_protocol::NO_LISTEN_GRANT`, so a declaring program
  can connect out and cannot listen. That is because no program the prompt can spawn needs to, and a listen
  grant is a policy somebody should choose (milestone 107 (the socket contract learns to accept)).

## What this unblocks for milestone 198

Rung 3a's package fetch (pull request #1273) runs as a role of `net_stack`'s binary under the kernel
harness. It can now be a program at the prompt: declare `network`, take `--mem` for its page, and
the progenitor endows it the stack this milestone builds. The proposal's exit criterion named that
fetch; the echo peer stands in for it here because #1273 had not landed when this was built. The
move is milestone 198's, and needs no new mechanism from this one.

## BUGS

- A boot with a NIC waits for DHCP before it has a prompt. `net_stack` reports its lease with a
  blocking send, so the progenitor takes it before building anything else. Immediate under QEMU's
  user-mode network; a boot with a NIC and no DHCP server would hang with no console. No real board
  is granted a NIC today. `crates/system_initializer`'s BUGS.
- The progenitor keeps a writable view of the NIC's DMA page, the rng's cost exactly: there is
  no unmap.
- x86_64's network is the kernel's to build, not the progenitor's. Since 2026-10-05 the kernel
  builds `net_stack` on the `e1000e` when there is no virtio-mmio NIC and grants the progenitor
  its endpoint (milestone 198 (a package manager)); it leaves a PCH part such as xenon's I219
  alone. An OVMF runner that names no `-netdev` still has a NIC: QEMU's default fills the gap
  with an `e1000e` on user networking (milestone 494's lane, 2026-10-04 UTC). Milestone 494 (a
  driver for the network card a PC actually has) names it explicitly. `swish-check`'s x86_64 leg
  types the two echo runs.
- A socket client pays for its own page with `--mem 4`. The network grant carries no memory,
  deliberately; a person has to type the budget.

## Follow-on

- **Milestone 646.** Milestone 646 (a typed prompt outruns the undertaker, and the job pool fills with holes). Two findings: per-client socket isolation
  (`design/roadmap/0649-every-client-of-a-network-stack-shares-its-socket-numbers.md`), and
  the riscv64 `swish-check` failure measured on the base commit while this was gated
  (`design/roadmap/0646-a-typed-prompt-outruns-the-undertaker.md`).
- **Recorded.** The package client's move to the prompt is milestone 198's, in this block's section
  on it. The DHCP wait and the DMA view are in `crates/system_initializer`'s BUGS. The MMIO NIC with
  no IOMMU is at `kernel::user::boot_virtio_net_device`, and the last capability slot at
  `kernel::cap::CAPABILITY_TABLE_SLOTS`. x86_64's missing NIC is in
  `swish_check_omits(arch, line)`, and milestone 494 (a driver for the network card a PC actually
  has) closes it.

## Index row

The progenitor builds `net_stack` from a NIC the kernel grants it, and a program declaring `network`
is endowed its endpoint at slot 10. `swish-check` reaches the runners' echo peer from the prompt on
aarch64 and riscv64, and a witness that declares nothing is refused.
