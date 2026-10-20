---
status: NOT-STARTED
raised: 2026-10-20
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: a machine with two NICs; QEMU attaches two, and silicon needs a second network cabled
specific_machine: none
needs_person: no
---
# 941. A machine on two networks at once

*Name: provisional. The number 941 is provisional until the merge queue lands it, and the title
and slug are too.*

calef, 2026-10-20 (UTC), on #2470, choosing between ways radon could use its two Ethernet ports:
"Lets do a and create a milestones for b." Option a, one port at a time, is the
milestone #2470 adds (radon's second Ethernet port). This is b: both ports up at once, each a network a program can be handed.
A writing-only lane wrote it the same day and built nothing.

## When to build it

Not until a workload needs two networks. #2470's milestone gives a person a lease from whichever
jack they cable. The cases that would justify this:

- A management LAN and a data LAN. An SSH server or a monitoring agent listens on one, a service
  on the other, and neither can be reached from the wrong side because it was never handed it.
- A router or firewall: forwarding between the two is a program holding both.
- A machine with an untrusted segment, a camera network or a lab bench, that a few programs may
  reach and the rest may not.

Each is a customer-path workload in AGENTS.md's sense only once someone runs it. Until then this
stays NOT-STARTED.

## What the tree has

- One stack per NIC, one NIC per stack. `crates/designware_ethernet/src/process.rs` and
  `crates/e1000e/src/process.rs` place the DMA region at one address, and each says a `net_stack`
  drives one NIC, never two.
- One kernel-built stack. `grant_kernel_network_stack` in `kernel/src/user/progenitor_boot.rs`
  fills slots 29 and 30 with the first of the `e1000e` and the JH7110 port, and virtio's path
  fills the same role. There is no slot for a second.
- A program's network is one bit. `crates/capability_manifest_note` carries `network` as byte 34,
  0 or 1, and `crates/installed_programs/src/grants.rs` grants the word `network` with no argument.
  Both are formats the packager and the kernel share.
- Each socket is its own capability (§255 (each socket is its own capability)), minted from a `Stack` front door. A program already
  reaches exactly the stack whose front door it holds. Nothing in `socket_contract` names an
  interface, so the contract can stay as it is if the choice is made by which front door is handed.
- Resolver zones are a grant over the socket contract's shape (§252 (a resolver grant is a zone per badge over the socket contract's shape)), so a resolver is also per
  stack.

## The decision it waits on

What a program asks for and what it is handed, when there are two networks. This is the socket
contract's edge and the manifest's format, so it is calef's, and it is unwritten. The questions a
lane writes up for it:

- Is a network named in the manifest (a label per network, `network = "management"`), chosen by
  the person at install, or granted by whoever spawns the program? The capability model says the
  spawner hands a front door; the open part is where the name lives.
- What does a program that asks for "the network" get on a two-network machine: one chosen by
  policy, or a refusal until it says which?
- Is there a default route, or does each stack reach only its own subnet?
- Do the two stacks share a port namespace? Today the listen and bind grants are per stack
  (`socket_contract`'s `listen_grant`), which suggests no.

The capability answer is the one to beat: a program reaches the network whose `Stack` it holds,
and no program holds one it was not given. That keeps the wire format as it is and moves the
choice to the manifest and the progenitor.

## Exit criteria, once the decision is written

1. Two NICs up at once, each its own `net_stack` with its own DMA region and its own lease. The
   fixed DMA address becomes one per stack.
2. The progenitor holds a `Stack` per network and hands each program only the ones its grant
   names, under the ruled format.
3. A QEMU test with two NICs on two networks: a program granted one cannot open a socket that
   reaches the other, and a program granted both reaches each.
4. On radon, once #2470's milestone has brought up its second port, with both jacks cabled to
   different networks, both lease, and a listener granted only
   one network is unreachable from the other.

## BUGS

- **The decision is unwritten.** Everything above it is a sketch of what it would govern.
- **A router is more than two stacks.** Forwarding between them needs a program that holds both
  and moves frames or connections across, and that program's shape is not designed here.

Reuse: the tree's own `net_stack`, run twice, is the stack; no multi-interface stack was searched
for, because the capability answer needs one stack per network. Prior art, recalled rather than read: Linux network namespaces put each
interface in its own stack and hand a process one namespace; FreeBSD's VNET jails do the same; Plan
9 binds a network's `/net` into a process's namespace. All three make the network a thing a process
is given, which is the capability answer above.

## Index row

Both of a machine's NICs up at once, each its own stack, and a program reaching only the network
whose `Stack` capability it was granted: a management LAN and a data LAN, or a router. It waits on
an unwritten decision about how the manifest names a network and what the progenitor hands out.
