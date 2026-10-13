---
status: BUILT
raised: 2026-08-23
built: 2026-08-25
---
# 158. Build the eleven kernel object and identifier renames DECISIONS §113 decided

This is the build work for DECISIONS §113 (eleven kernel object and identifier names move from
contraction or borrowed jargon to the plain, standard term). Each of the seven names below, with its
companions, is renamed in the tree.

## What was built

Five lanes renamed the seven names and their four companions, each to completion and each green.
Every name's real surface was far past §113's capitalized-token estimate once its ABI module and
lowercase identifier family were counted; the table below has the measured counts. Each lane's
account, with its scope call, what it left alone and how it was verified, is an appendix:

- [`endpoint-rename.md`](0158-kernel-object-rename-build/endpoint-rename.md): `Endpoint`/`EpId`/`EpFail` ->
  `Rendezvous`/`RendezvousId`/`RendezvousFailure`, done first as the block instructed.
- [`tcb-rename.md`](0158-kernel-object-rename-build/tcb-rename.md): `Tcb`/`Tid`/`TcbPtr`/`TidSet` ->
  `ThreadControlBlock`/`ThreadId`/`ThreadControlBlockPointer`/`ThreadIdSet`, in
  milestone/158-tcb-rename.
- [`aspace-rename.md`](0158-kernel-object-rename-build/aspace-rename.md): `Aspace`/`FreeVas` -> `AddressSpace`/`FreeAddressSpace`,
  with the near-collision it checked and the bug its own build caught.
- [`frame-rename.md`](0158-kernel-object-rename-build/frame-rename.md): `Frame` -> `PageFrame`, with
  the senses of "frame" it told apart and the `DeviceFrame` call.
- [`untyped-rename.md`](0158-kernel-object-rename-build/untyped-rename.md): `Untyped` -> `MemoryRegion`, the last, in
  milestone/158-untyped-rename.

## What is still open

Nothing. All seven names are renamed. `Untyped` -> `MemoryRegion` had the largest gap against the
pre-rename estimate; see the table below.

## The seven renames, and what each one actually touches

| Was | Becomes | Measured surface (§113's own count where a lane hasn't re-measured; done names carry their real count) |
|---|---|---|
| `Aspace` | `AddressSpace` | **done**: 391 occurrences across 37 `.rs` files, plus `FreeVas`'s 10 across 3 |
| `Untyped` | `MemoryRegion` | **done**: 60 occurrences across 25 `.rs` files, capitalized-token pre-rename estimate; 88 files / 620 insertions / 618 deletions actually changed |
| `Endpoint` | `Rendezvous` | **done**: 82 occurrences across 22 `.rs` files by fresh count; 216 for `EpId` across 43 files |
| `Frame` | `PageFrame` | **done**: 51 files, 172 occurrences of the bare capitalized token alone (stale table undercounted, same pattern as `Endpoint`); real diff 97 `.rs` files plus `notes/frames.md`, 918 insertions / 899 deletions, including two file renames and an `abi::frame` -> `abi::page_frame` module rename. See `frame-rename.md` |
| `Tcb` | `ThreadControlBlock` | **done**: 33 occurrences across 12 `.rs` files by fresh count; the lowercase identifier family well past this |
| `EpId` | `RendezvousId` | **done**, with `Endpoint`: 216 occurrences across 43 files, once the type alias's own call sites were counted |
| `Tid` | `ThreadId` | **done**, with `Tcb`: 82 occurrences across 13 `.rs` files by fresh count |

Plus the four companions §113 also decided in the same entry: `TcbPtr` ->
`ThreadControlBlockPointer` (done, with `Tcb`; 11 occurrences, `kernel/src/sched.rs` only), `TidSet`
-> `ThreadIdSet` (done, with `Tcb`; 6 occurrences, `kernel/src/user/survey_tests.rs` only), `EpFail`
-> `RendezvousFailure` (done, with `Endpoint`), `FreeVas` -> `FreeAddressSpace` (done, with
`Aspace`). That test file is now in `system_tests/src/user/`.

Re-measure before starting, not from this table. §113's own count is from 2026-08-23 and this
tree changes fast; a lane that trusts a stale number here repeats §76's own recorded mistake.

## What has to be true before this is safe to do in one pass

Real collisions checked and named by §113 itself, still true and worth restating so a lane does
not have to re-derive them:

- `Frame` collides with the compositor's own use of "frame" for a rendered screen update
  (`crates/compositor/src/lib.rs`). Renaming the kernel object to `PageFrame` is what resolves this;
  do not rename compositor's own "frame" language to avoid the opposite collision.
- `Untyped`/`Region` collides with the compositor's own use of "region" for a damaged screen
  rectangle, the same file. `MemoryRegion` is deliberately not bare `Region` for this reason.
- `Endpoint` has real prior art (seL4's own term) that some doc comments and notes may still cite
  approvingly; a rename should not silently rewrite a citation that correctly names the older
  term. Read `notes/ipc-naming.md` before touching it -- §113 cites it
  directly and it may need updating rather than blind search-and-replace.
- `Aspace`/`AddressSpace` had no collision of this shape (checked directly: `crates/compositor`, the
  file that collided with both `Frame` and `Untyped`/`Region`, has no "aspace" or "address space"
  vocabulary of its own), but it did have a real near-collision nobody had recorded:
  `kernel/src/user.rs` already defines `pub struct AddressSpace`, the kernel-internal page-table
  object, predating the rename. Not a blocker in the end. It is the same pattern
  `Object::Rendezvous(RendezvousId)` already uses naming `struct Rendezvous`: a capability variant
  sharing its name with the object it names, in different modules. It is recorded so the next
  reader doesn't have to re-derive that it's fine; see [`aspace-rename.md`](0158-kernel-object-rename-build/aspace-rename.md).

## Scope and sequencing

Pure rename, no behaviour change, the same discipline milestone 98's `IpcTables` rename used
(itself one of the smaller names in this same family -- read that milestone's own completed work,
once merged, as the template for how to gate and verify a rename of this shape here). Milestone
69's proof obligation applies to any renamed type a Kani harness references by name.

Do the biggest, most-cited name first and measure the real diff before committing to doing all
seven in one lane. `Endpoint`/`Rendezvous` is both the largest measured surface and the one most
likely to touch generated or macro-derived code (IPC message types, proof harnesses) in ways a
smaller rename like `Tcb`/`ThreadControlBlock` will not. If the full set turns out too large for one
safe, reviewable commit, split by name rather than doing all seven partially -- an inconsistent tree
where `Aspace` is renamed and `Endpoint` is not is a worse intermediate state than finishing one name
completely and stopping.

Rename `EpId`/`TcbPtr`/`TidSet`/`EpFail`/`FreeVas` together with the type they name, not as a
separate pass -- §113 decided them as companions specifically so a type and its own identifier never
disagree mid-rename.

## What this does not decide

Whether `Endpoint`'s rename can land in the same commit as `EpId`'s, or must be sequenced, is an
implementation judgment for whoever builds this, guided by how large the diff actually turns out to
be once measured -- not decided here.

## What it unblocks

Nothing else is gated on this; it closes the gap between a decision calef made and the code
actually reflecting it, which is its own reason to exist per DECISIONS §113's whole argument: a
name only works if a reader meets it. All eleven names are done (`Endpoint`/`EpId`/`EpFail`,
`Tcb`/`TcbPtr`/`Tid`/`TidSet`, `Aspace`/`FreeVas`, `Frame`, and `Untyped`); no reader meets a
contraction or borrowed abbreviation from this list anywhere in the tree anymore.

## Follow-on

- **Refused.** `DeviceFrame` was left unrenamed. It is not one of §113's eleven names or the
  `CSpace` amendment, its own doc comment already disambiguates it from the compositor's frame ("a
  device's MMIO page"), and companion-renaming an unlisted sibling is the scope creep AGENTS.md's
  naming section asks a lane not to take on its own initiative.
- **Refused.** Local bindings and parameters that merely spell a renamed concept (`ep`, `aspace`,
  `tid`, `tcb`) were left alone across all seven names. A local binding names nothing a reader meets
  from outside the function, so it is not on the identifier surface §113 decided.
- **Refused.** Informal lowercase prose that uses "endpoint", "tcb" or "untyped" as English rather
  than as a citation of an identifier stayed as written, outside each object's own implementation.
  The exception was "aspace", swept to "address space" wherever the rename touched, because it is
  not standard English and leaving it would have parked the exact contraction §113 retires next to
  its own renamed identifier.
- **Refused.** Historical citations keep the old spelling on purpose: `crates/abi/src/lib.rs`'s
  naming rationale names `Tcb`/`Aspace`/`Untyped` as the abbreviations a review sank, and renaming
  them would make the sentence describe the winning name as the one that lost.
  `design/decisions/0114-aspace-enumerate.md` was left as the point-in-time record it is.

## Index row

All seven names built across five lanes and green on all three ISAs, each measured far larger than
§113's own estimate once its ABI module and lowercase identifier family were counted: `Endpoint`/`EpId`/`EpFail` -> `Rendezvous`/`RendezvousId`/`RendezvousFailure`, `Tcb`/`Tid`/`TcbPtr`/`TidSet` -> `ThreadControlBlock`/`ThreadId`/`ThreadControlBlockPointer`/`ThreadIdSet`, `Aspace`/`FreeVas` -> `AddressSpace`/`FreeAddressSpace`, `Frame` -> `PageFrame`, `Untyped` -> `MemoryRegion`.
