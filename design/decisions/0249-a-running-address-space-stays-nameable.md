---
status: AMENDED
raised: 2026-10-05
decided: 2026-10-05
ratified_by: calef
---

# 249. A running address space stays nameable, and a capability never decides when it dies

calef, 2026-10-05 14:39 UTC, on [#1678](https://github.com/nifeos/nife/pull/1678), the pull request of milestone 95 (an
unmap primitive, and the mappings init never lets go). Quoted from
[ruling 3 of 3](https://github.com/nifeos/nife/pull/1678#issuecomment-5996726490): "Yes" to option A
of `design/roadmap/0789-a-running-process-can-name-its-own-address-space.md`. It carries the two
amendments from `notes/naming-a-running-address-space.md` (#1685). Recorded by the maintainer.

This is a section rather than a second amendment to §162 (whether a holder can give up a mapping)
because it changes what an existing method promises. `ThreadControlBlock::CONFIGURE` stops retiring
the name it is passed, and refuses a case it used to make unreachable. That is the syscall surface
of §10 (the capability-based microkernel process model) and §16 (object revocation), and §246 (a
plain `RECEIVE` never takes a capability) is the precedent for recording such a narrowing as its own
section.

## The ruling

A process's address space can be named while a thread runs on it.

- `CONFIGURE` still consumes the capability it is passed, but no longer retires the name. A copy
  made before `CONFIGURE` keeps resolving to the bound space.
- The kernel grants init a capability to its own space at boot.
- A builder hands a child one only if it chooses, with `CAP_INSERT` before `CONFIGURE`, as every
  other authority a child holds is handed. Nothing grants one by default. A builder that keeps its
  own copy past `START` is retaining authority over the child, which §142 (what a spawner retains
  over a child after `START`) says a spawn path declares.
- `MAP_INTO` and `UNMAP` work on a running space for whoever holds a capability to it.

**Amendment (a): deleting a capability never frees a space.** A space dies when its thread is
reaped or when the region its root came from is destroyed. After that every capability still naming
it, in any process, fails, the way a consumed name answers today. There is no final-capability rule,
as there is none anywhere else here (§16's model: region ownership and generational staleness, not a
derivation tree). The registry owns every space, bound or not, and a thread caches its immutable root
and tag for the context switch, which must not take the registry's lock. `MAX_USER_SPACES` (32)
grows, since every running process now holds an entry.

**Amendment (b): a second bind of a bound space is refused explicitly.** Today the consumed
capability is the only thing stopping a second `CONFIGURE` on one space, and §105
(`std::thread::spawn` stays declined) rests on that. Under this ruling the name survives, so the
refusal has to be stated: a bound space is marked in its registry entry, and a second bind is refused
with `WrongObject`, as a second `CONFIGURE` of a started thread already is. §105 stands unchanged.

**Still open, and owed its own ruling:** which slot init's own-space capability occupies. A slot
convention is a wire format. The candidates the research names are a fixed slot after
`abi::fault::FAULT_EP_SLOT`'s pattern, or a grant through `boot_progenitor`'s `Spawn::grants` at a
slot the progenitor is told.

## What was refused

- B, every thread gets a capability to its own space at `START`. A default grant: every process
  would hold authority over its own memory whether its builder wanted that or not, and a default
  cannot be withdrawn program by program. No surveyed system does it. seL4, Microkit, CAmkES and
  Fuchsia all make the grant explicit and optional. B's only argument was that it is less work.
- C, leave it. `UNMAP` would serve only a space under construction, the progenitor would keep
  its window into every boot server, and §162's ruling would close nothing the roadmap named.

## The prior art

Read, not recalled; the sources and line numbers are in `notes/naming-a-running-address-space.md`.

seL4 hands its root task its own VSpace capability (`seL4_CapInitThreadVSpace`, slot 3) and lets any
holder map into a running space. Its `ThreadControl` copies the capability into the thread rather
than consuming it, and the space lives until the last copy in the derivation tree goes. So option A
takes seL4's authority shape, and Zircon's lifetime shape (handles are names that go dead; the owner
decides lifetime), which nife already had.

seL4 gives a page up through the frame capability (`Page_Unmap`), not the VSpace one. That is
the path §162 refused as its option B. So `AddressSpace::UNMAP(va)` rests on §162's ruling, not on
seL4 precedent, and nife needs a running space named where seL4 would not, because nife's builders
delete frame capabilities right after mapping.

One divergence is deliberate. When seL4 loses a running thread's space, it leaves the thread on an
empty space to fault. nife kills it, because `CONFIGURE` refuses anything but an embryo, so a nife
thread that lost its space could never be given another.

## How it composes with milestone 765

§16's 2026-10-05 amendment and milestone 765 (a destroyed region cannot free the root a running
thread walks) stand as ruled: a live thread whose bound root is in the destroyed span is a resident,
refused, killed, retried. This ruling adds nothing to the surface there. It changes the bookkeeping.
With the registry owning every space, `reap_address_spaces_in_region` sees bound spaces for the
first time. The existing order keeps it from dropping a space a thread can still run on:
`reap_region_objects` goes first, and 765 makes it refuse while such a thread lives. On the retry the sweep frees
the root in one place.

Registry ownership also closes a gap the research found by reading, not by running. A thread already
dead but not yet reaped still owns its space today. If its TCB is outside region R and its root
inside, R can come back while the corpse holds that root. The corpse's later drop would then forget a
page that may be someone else's root. With the registry as owner, the sweep takes the space from
under the corpse first. Milestone 765's lane owes the confirmation.

## What this unblocks

Milestone 95 reaching BUILT: the progenitor gives up each scratch page after the child is built, the
negative control (the progenitor writes to a boot server's page and faults), and the multicore unmap
test, once init's slot is ruled.

## Amendment (2026-10-05): init's own-space capability is slot 28, WRITE only

calef, 2026-10-05 (ruled at the time `date -u` read 16:51 UTC): "A, slot 28, WRITE only". This
answers the "still open" paragraph above. Recorded by the maintainer.

- Where. The kernel grants init a capability to its own address space at boot as a new
  `BootEndowment` field, `own_space` (provisional name), at slot 28 in `components/src/progenitor.rs`'s
  table. It is granted on every boot and appended past the highest slot in use (27,
  `usb_keyboard_attach`), per the table's convention.
- Rights: WRITE only, no GRANT. `UNMAP` needs WRITE. Without GRANT, init cannot delegate
  authority over its own space to anyone it spawns. GRANT can be added later; taking it away once a
  program has relied on it is harder, so the narrower grant goes first.

Refused:

- B, a tree-wide constant beside `FAULT_EP_SLOT` (63), such as 62. Builders would adopt it for
  children, a process-wide convention drifting toward the default grant this section refused as its
  option B.
- C, seL4's slot 3. It renumbers every slot from 3 up for no gain.

This unblocks milestone 95 (an unmap primitive, and the mappings init never lets go) reaching BUILT.

## Amendment (2026-10-10): a space has one holder, and threads share it through a process

calef ruled on #1892 on 2026-10-10 (UTC) that `BIND` lives on a process capability and that this
section's bound mark retires; §276 (the thread syscalls as built) records it. What changed in the
merged code (`ddd38f0b0`):

- Amendment (b) applies per holder, not per thread. A holder is a thread that is its own process,
  or a process object. A second `CONFIGURE` with a held space, or a `RETYPE_OBJ` of a process
  around one, is still `WrongObject`, so two processes cannot share a space. More threads run in
  one space by joining the process that holds it.
- Amendment (a)'s "a space dies when its thread is reaped" now reads: when its holder is gone, which
  for a process means it has ended and its last member is reaped, or its region is destroyed.

