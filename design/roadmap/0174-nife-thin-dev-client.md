---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: 171
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 174. nife as a thin development client: edit and commit locally, build on a remote service

This is the fourth of four self-hosting milestones. It is the nearer-term alternative to the full
local self-hosting of
[milestone 173 (`rustc`/`cargo`/LLVM natively on nife)](0173-rustc-cargo-self-host.md).
"Daily driver" does not have to mean 100% local compilation. This path reaches it without an LLVM
port or the new primitive of [milestone 172 (a capability-native subprocess primitive)](0172-capability-native-subprocess.md).

[Milestone 171](0171-git-core-userspace.md)'s git core is the structural
precondition: there has to be something to commit and send before a remote-build protocol matters.
[Milestone 169](0169-kilo-editor.md)/[170](0170-nano-editor.md)'s editor work is not a hard gate the
same way, since a first cut of this milestone could in principle pair with an editor running
elsewhere, but that would defeat the "daily driver" point of the exercise, so build order should
treat the editor as practically necessary even though it is not listed as a formal dependency here.

## The shape of it

nife edits and version-controls a working tree locally (milestones 169/170/171), then hands the
actual `cargo build`/`rustc` invocation to a remote build service over the network, rather than
running the compiler on nife itself. The dependency list this needs is much smaller than
milestone 173's: a network client and a remote-build protocol, no
[capability-native subprocess primitive](0172-capability-native-subprocess.md), no threading
questions, no LLVM port.

## What it needs

- **A network client.** Real TCP/UDP already exists as a userspace program (`smoltcp`, milestone
  30's `net_stack`), so the transport layer is not new work; what is new is a client speaking
  whatever remote-build protocol this milestone defines.
- A remote-build protocol. Not designed here. Could be as simple as "rsync the working tree,
  run `cargo build` over SSH, rsync results back" replicated by hand, or a purpose-built protocol;
  sizing that choice is this milestone's own first task.
- **TLS is not a hard prerequisite for a first cut.** nife has no TLS/crypto stack today
  ([milestone 66](0066-vaultwarden.md) names this exact gap for an unrelated program, "TLS: none.
  `rustls` needs entropy (have it) and a large crypto surface"), but a first version of this
  milestone could run over plain TCP on a trusted LAN, the same way calef's own dev machine and a
  build box already share a network today. Real TLS becomes necessary only once this needs to cross
  a network calef does not trust, which is a later, separable hardening pass.

## Why it matters

This is the path that gets calef using nife daily soonest: editing and committing on a real nife
host is real, felt daily use, even while the CPU-heavy build step still happens elsewhere. It also
does not compete with [milestone 173](0173-rustc-cargo-self-host.md) for sequencing: nothing here
blocks or is blocked by the capability-native subprocess primitive or an LLVM port, so both paths
can proceed independently once milestone 171 exists.

## What this does not decide

The actual remote-build protocol's shape, whether it is generic enough to reuse for other projects
or purpose-built for this tree's own layout, and whether `git`'s network operations
(`clone`/`fetch`/`push`, explicitly left open by [milestone 171](0171-git-core-userspace.md)) share a
network-client crate with this milestone's remote-build client or stay separate. Left for whoever
picks this up.

## Index row

The nearer-term alternative to full local self-hosting: edit and version-control on nife, hand the
actual `cargo build` to a remote service over the network. Needs a network client and a
remote-build protocol, not the capability-native subprocess primitive or an LLVM port; could run
over plain TCP on a trusted LAN before nife's still-missing TLS stack matters.
