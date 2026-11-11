---
status: NOT-STARTED
promoted_from: a-running-process-can-name-its-own-address-space
raised: 2026-10-05
milestone_dependencies: 95
decision_dependencies: 162
machine_requirements: none
specific_machine: none
needs_person: no
---
# 789. A running process can name its own address space

Raised by milestone 95 (an unmap primitive, and the mappings init never lets go)'s lane on
2026-10-05 (UTC), while building `AddressSpace::UNMAP` on §162 (whether a holder can give up a
mapping). The method works, and it cannot reach the window it was decided to close.
`notes/unmap.md` has the finding in full. This is a fork on the syscall surface, so the options
below wait for an architect.

Reuse: none exists; searched `kernel/src` for every mint of `cap::address_space_cap` (only
`RETYPE_OBJECT` and tests) and `crates/abi` for a self-naming slot convention (only
`abi::fault::FAULT_EP_SLOT`, which option B below would copy). This is the kernel's object model,
where §46 (thin primitives or whole subsystems) says write it.

## The hole

The progenitor maps every page it writes for a boot server into its own running space with
`PageFrame::MAP`. To give those pages up it needs an `AddressSpace` capability naming that space,
and none exists. `RETYPE_OBJECT(ADDRESS_SPACE)` is the only place one is minted. The kernel grants none
at boot. `ThreadControlBlock::CONFIGURE` consumes the builder's capability and removes the name from
`USER_SPACES`, so a copy made before it resolves to nothing after. Until this is answered, `UNMAP`
serves only a space under construction, and milestone 95's negative control (the progenitor writes
to a boot server's page and faults) cannot be built.

## The options

**A. A bound space stays nameable, and whoever holds a capability to it can use it.** `CONFIGURE`
still consumes the capability it is passed, but stops retiring the name: it is retired when the
thread is reaped. A builder that wants a child to manage its own memory hands it a copy with
`CAP_INSERT` before `CONFIGURE`. The kernel grants init one for its own space at boot. This is seL4's
shape (recalled, not read: `seL4_CapInitThreadVSpace` hands the root task its own VSpace capability),
and a creator decides whether to keep or pass on one for each child. It adds authority only where a holder chose to keep or grant it.
`MAP_INTO` into a running space comes with it, which seL4 also allows.

**B. The kernel grants every thread a capability to its own space, in a reserved slot, at
`START`.** `abi::fault::FAULT_EP_SLOT`'s convention, one slot over. Simpler for a program to find,
but every process gets it whether its builder wanted that or not, which is a default grant rather
than an explicit one.

C. Leave it. `UNMAP` stays a builder's tool for a space under construction, and the
progenitor's residual stays recorded in `notes/trusted-init.md`. §162's ruling then closes nothing
the roadmap block named.

A one-page loader (§162's option C) does not help with any of these on its own: the last page stays
mapped, and giving it up needs the same capability.

## Recommendation

A, because it is the explicit-grant shape the rest of the tree uses, and the precedent outside the
tree is the system this model borrows from. Would we still choose it at equal cost? Yes. B is
probably less work (a slot convention, no registry change), and that is the only argument for it.

It interacts with the running-root fix
(`design/roadmap/proposals/a-destroyed-region-cannot-free-a-running-root.md`). Under A, bound spaces
stay in a registry by name, so `user::reap_address_spaces_in_region` would see them for the first
time, which changes that proposal's options. Decide the two together.

## What it owes once answered

- The progenitor gives up each scratch page after the child is built, and the negative control:
  the progenitor writes to a boot server's page and faults.
- The multicore test `notes/unmap.md`'s BUGS says cannot exist yet: a thread reading a page in a
  loop on one core, `UNMAP` of that page from another, and the reader faulting.

## Index row

A process cannot name its own running address space, so the progenitor cannot give up the pages it mapped for boot servers and `AddressSpace::UNMAP` cannot reach the window it was decided to close. The options are a fork on the syscall surface and wait for an architect.
