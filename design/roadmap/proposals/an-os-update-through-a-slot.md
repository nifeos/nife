---
status: PROPOSED
raised: 2026-10-28
milestone_dependencies: 525, 909
decision_dependencies: 235, 208, 220
machine_requirements: none
specific_machine: none
needs_person: no
---
# An OS update is a set of packages that lands through a slot

Raised 2026-10-28 (UTC) by the lane `proposal/607-item-5-an-os-update-through-a-slot`. This is
item 5 of milestone 607 (nife is built and updated from packages): "An OS update is a set of
packages that lands through a slot (U4 ruled; waits on item 1)." Item 1 is milestone 909 (basalt
composes the bootable image from its packages), which landed 2026-10-28 (UTC). This proposal names
what the update writes, what it leaves alone, and the order the pieces land.

Reuse: the build side is milestone 909's own composer and `helpers/packages.py`, which already
read the image's membership and write each package's digest; basalt publishes what they produce.
The boot side is milestone 525's `crates/boot_slot` and `uefi_loader`'s chooser, unchanged. The
device side reuses `jig`'s fetch-and-verify path (§220's scoped keys) and §208's activation table.
No new dependency; the system manifest is a file, not a crate.

## What is already ruled

§235 (packages build the OS and the tree divides by release) ruled the shape on 2026-09-27 (UTC),
and this proposal executes it rather than reopening it.

- **U4, the slot layout.** Packages are the download unit, and each slot holds its own full copy
  of the base set. The inactive slot is wiped whole before reuse, then filled from the new
  manifest. Nothing is shared between the slots, so no collection rule can delete a blob the other
  slot needs.
- **T4 with T2, the trust handoff.** The loader hands over the progenitor digest and the manifest
  digest. The kernel checks only the progenitor; the progenitor checks the base set against the
  manifest digest. The kernel therefore stops depending on the base set and still depends on the
  progenitor.
- **525's slots.** Two boot slots, priority, tries and confirmation live in `crates/boot_slot` and
  the UEFI chooser. This proposal reuses them: an update writes the inactive slot and marks it to
  try. Nothing here adds a third slot or a second rollback mechanism.
- **The base is a resolved lock.** Resolution happens at build time, not on the device, the way
  `Cargo.lock` and a Nix closure work. Basalt publishes the manifest and the packages it names;
  the device fetches and copies them.

Programs outside the base still install through `jig` under §208 (activation) and §220 (scoped
keys), and an installed package fills a service's role without changing the base list. This
proposal touches the base path only.

## What it proposes

An update is one manifest and the packages it names, copied into the inactive slot, then a
confirmation that keeps it or a failure that reverts to the running one.

1. **The manifest gets its own name and file.** Today "manifest" means two things: a program's ask
   in its ELF note (milestone 597) and the list of packages an image holds (U4). This proposal
   gives the second its own name, provisionally the **system manifest**, and a file basalt writes
   beside the packages. It lists each package with its digest and names the progenitor, the same
   three facts §235's T4 makes the trust root read.
2. **The updater fetches and stages.** A program, provisionally `update`, reads a system manifest,
   checks each package's digest against it, and copies each into the inactive slot. A package whose
   digest matches the running slot's copy can be copied locally instead of fetched (U4 allows it).
   The staging slot is wiped whole first, so no stale file survives.
3. **The kernel reads the new manifest through the loader.** The loader passes the inactive slot's
   manifest digest, and the progenitor checks the base set against it before anything runs. A
   manifest the packages do not match is refused naming the package, the same check the composed
   image already runs at build time.
4. **The chooser tries the slot and the update confirms.** `boot_slot` marks the staged slot with
   one try. A boot that reaches the shell confirms it; a boot that exhausts its tries reverts to
   the other slot, as 525 already does.
5. **The first update is the base nife ships.** Basalt publishes the system manifest with the
   packages, so a machine updates from any pinned release to any later one without rebuilding the
   tree. This is what 607 promised: nife is built and updated from packages.

## What it does not propose

- No shared store (U2, refused by the ruling), and no collection rule.
- No signed manifest (T3, refused): the loader and the slot checksum carry the trust, as §235
  records.
- No third slot and no second rollback path (U3, not taken). `jig rollback` stays the
  installed-programs mechanism; this is the base path beside it.
- No pinned-root key in the kernel (T3's refusal stands).

## Dependencies and order

Item 3 depends on item 1, which is done, and on 525's slots, which exist. It also depends on the
loader's handoff, §235's T4 with T2, which milestone 525 records as one initrd slot on the
device-tree path; the x86_64 path already has the loader's own file. Steps 1 and 2 are host and
device code that can land before the handoff, because they write and stage files the handoff later
reads. Step 3 waits on T4 with T2. Steps 4 and 5 wait on step 3.

## What this proposal does not do

It writes no code. §311 (a contract is code, not prose) reserves the checks for an implementing
lane, and every new name here is provisional.
