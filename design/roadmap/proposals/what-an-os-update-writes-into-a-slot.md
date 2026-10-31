---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: 976
decision_dependencies: 235, 241, 244
machine_requirements: none
specific_machine: none
needs_person: no
---
# What an OS update writes into a boot slot

Raised 2026-10-31 (UTC) by the lane `milestone/976-an-os-update-through-a-slot`, while it broke
milestone 976 (an OS update is a set of packages that lands through a slot) into parts. Parts 1 to
3 of that block need no ruling. Part 4, the program on a running machine that writes the spare
slot, needs one first: what the slot holds. This file asks that one question and lists the
questions behind it, which wait their turn.

Reuse: the slot format and chooser of milestone 525 (a bad upgrade cannot brick the machine: two
boot slots, tries and priority), and the confirmation of milestone 554 (a good upgrade sticks). The
composer is milestone 909 (basalt composes the bootable image from its packages)'s. No new
dependency.

## The background, plainly

An installed nife disk has two boot slots, partitions of 64 MiB each (§244 (the installed disk has
four partitions, and a boot slot is 64 MiB)). A slot holds one file behind a 4096-byte header: a
complete UEFI program that embeds the kernel and the initial ramdisk, the archive of every base
program. The chooser on the EFI system partition picks a slot from bits in the partition table,
spends one of its tries, and starts that file. A boot that comes up confirms the slot; a slot that
runs out of tries without confirming loses to the other one. All of that exists, and an update
inherits it unchanged.

§235 (the OS is built and updated from packages) ruled the update's unit as U4: the machine
downloads packages, and each slot holds its own full copy of the base. Nothing in one slot points
into the other.

One fact of the tree decides what U4 can mean today. The kernel compiles the digest of every base
program into its trust root (`kernel/build.rs`). So a new `swish` makes a new kernel, and the
kernel's bytes depend on the whole base. Item 6 of milestone 607 (the repository divides along
what releases together) removes that coupling: the loader hands over two digests, and the
progenitor checks the base set. Until it lands, no device can
assemble a bootable slot from packages, because only a host that builds the kernel can produce a
kernel that vouches for the new base. And basalt does not yet publish the kernel as a package at
all (milestone 909's `BUGS`: the kernel is built, not packaged).

## The decision: what the inactive slot receives

**Recommendation: B.** The slot keeps its header and becomes a small set of named members: the
kernel, the composed archive, and the system manifest that lists each package by digest. The
chooser loads the kernel and the archive itself, as `uefi_loader` already does for the files it
embeds. The device fetches packages from basalt's index, checks each digest, takes unchanged
packages from the running slot, writes the archive with the composer's own code, and writes the
slot. Basalt publishes the kernel as a package, built against the base it vouches for, until item
6 makes it independent.

| option | what the slot holds | what the device downloads | what it costs |
|---|---|---|---|
| A. One boot file (interim) | today's format, one UEFI program | basalt's composed boot file, about 4.6 MB, whole | U1 in practice, which §235 did not take; a second migration when item 6 lands |
| B. Named members (recommended) | header version 2: kernel, archive, system manifest | each changed package, and the kernel package while item 6 is open | a slot format version and a chooser change; the composer's archive writer becomes a crate the device links |
| C. Hold the device side | today's format | nothing yet | no update on a running machine until item 6, and B's work waits behind it |

Why B. It is the format U4 describes, and it does not depend on item 6: item 6 only stops the
kernel package changing with every base change. A would ship the refused unit first and migrate
every installed disk later. C loses nothing in format but serializes two independent lanes. Would
I still choose B if A cost the same? Yes; A is smaller only because it reuses a file format, which
makes it an effort argument.

What B touches that two programs agree on, and therefore what the ruling covers:

- The slot header gains a version 2 that names members by offset and length. Version 1 stays
  readable, because installed disks carry it (§244 ratified version 1's bytes).
- The archive writer moves from `xtask/src/archive.rs` into a crate, since the host composer and the
  device updater must write identical bytes (AGENTS.md rule 7).
- The kernel package: `packages/kernel.package.toml` declares crates and no programs, and gains a
  member that is the built kernel.

What happens if the answer is no. Under A, part 4 writes basalt's composed boot file through the
existing `rollback::stage_an_upgrade` path, and a later block migrates the format. Under C, the
block stops after part 3 and records the wait on item 6.

## How others solved it, and where nife differs

- Android A/B (`update_engine`, the `boot_control` HAL). An update payload carries partition
  images, often as deltas; the engine writes the inactive slot's partitions, marks it active, and
  `markBootSuccessful` confirms it from userspace. The unit is a partition image. Nife's
  confirmation follows this, and its unit does not.
- ChromeOS (`update_engine`, depthcharge). The same GPT attribute bits nife uses (priority,
  tries, successful); the payload writes a whole kernel partition and a whole root partition.
  Firmware selects; on generic UEFI nife's chooser does instead.
- Fuchsia (`system-updater`, the paver). An update package lists images and packages by
  Merkle root. The paver writes the kernel image (ZBI) into an A/B/R slot, and the packages go
  to one shared content store, `blobfs`. That shared store is U2, which §235 refused.
- OSTree. One content-addressed repository, with each deployment a hard-linked tree. Shared
  storage again, so a collection error reaches both deployments.
- RAUC. A signed bundle carries one image per slot class; the installer writes each whole into
  the inactive slot group. The unit is the image, as in option A.
- Mender. A/B root file systems, an artifact that is a whole image or a delta, and a commit
  after the new system boots. Again option A's unit.
- Nix generations. The store holds every package once; a generation is a profile of links into
  it, and the bootloader lists one entry per generation. A package is the download unit, and the
  store is shared.

Nife takes the package as the download unit from Fuchsia and Nix, and the full independent copy per
slot from Android, ChromeOS, RAUC and Mender. None of the seven does both. The cost is two copies
of the base on disk, which §244's slot size already pays for: a release image measured 4,640,256
bytes against a 64 MiB slot.

## A concrete example

A machine runs base 0.1.0 from slot 0. Base 0.1.1 changes only `swish`.

1. Basalt publishes 0.1.1's system manifest, 58 programs from eighteen packages plus the
   `kernel` package, each by digest. The `swish` package's digest differs, and so does the `kernel` package's, because its
   trust root names the new `swish`.
2. The updater reads the manifest and finds seventeen packages whose digests match slot 0's copy. It
   reads those from slot 0 and fetches two packages from the index, checking each digest.
3. It wipes slot 1 whole, writes the kernel, the archive and the manifest as version 2 members,
   and marks slot 1 on trial: priority 3, one try.
4. On the next boot the chooser picks slot 1 and spends its try. The filesystem server mounts the
   disk, the kernel confirms the slot, and slot 1 stays chosen. Had the kernel hung, the next boot would have
   picked slot 0.

Under A the device downloads 4.6 MB in step 2. Under B it downloads two packages. Once item 6
lands, it downloads one.

## The questions behind this one

Each waits for the ruling above, and each will come as its own question:

1. The system manifest's name and format. 976 calls it the system manifest, provisionally; basalt
   writes it and the updater and the progenitor read it.
2. Who may start a base update, and what bounds an unattended one. Milestone 911 (scripted installs
   grant only what an approval file allows) gives every base package `update = false` in
   `packages/base-image.approval.toml`, because a slot replaces the base whole. Milestone 858 (lab
   machines update themselves) wants an unattended trigger, so the approval file needs a
   base-update entry or a separate grant.
3. What a failed boot does. Milestones 525 and 554 already answer this (tries, then the other
   slot; confirmation once the disk mounts). This proposal changes none of it, and lists it so
   nobody reopens it by accident.

## Index row

The update's unit is the package, and each slot holds a full copy. What the slot holds is the open
question: today's single UEFI program, or a version 2 slot of named members (kernel, archive,
system manifest) that a device can assemble from packages it fetched and packages it already has.
