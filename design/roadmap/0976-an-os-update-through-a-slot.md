---
status: PARTIAL
promoted_from: an-os-update-through-a-slot
raised: 2026-10-28
milestone_dependencies: 525, 909
decision_dependencies: 235, 208, 220
machine_requirements: none
specific_machine: none
needs_person: no
---
# 976. An OS update is a set of packages that lands through a slot

Raised 2026-10-28 (UTC) by the lane `proposal/607-item-5-an-os-update-through-a-slot`. This is
item 5 of milestone 607 (the repository divides along what releases together): "An OS update is a set of
packages that lands through a slot (U4 ruled; waits on item 1)." Item 1 is milestone 909 (basalt
composes the bootable image from its packages), which landed 2026-10-28 (UTC). This proposal names
what the update writes, what it leaves alone, and the order the pieces land.

Reuse: the build side is milestone 909's own composer and `helpers/packages.py`, which already
read the image's membership and write each package's digest; basalt publishes what they produce.
The boot side is milestone 525 (a bad upgrade cannot brick the machine: two boot slots, tries and
priority)'s `crates/boot_slot` and `uefi_loader`'s chooser, unchanged. The device side reuses
`jig`'s fetch-and-verify path (§220 (`design/decisions/0220-signed-builds-and-scoped-key-trust.md`)
and §208 (`design/decisions/0208-installing-is-granting.md`, its activation table)). No new
dependency; the system manifest is a file, not a crate.

## What is already ruled

§235 (`design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`) ruled the
shape on 2026-09-27 (UTC), and this proposal executes it rather than reopening it.

- U4, the slot layout. Packages are the download unit, and each slot holds its own full copy of
  the base set. The updater wipes the inactive slot whole before reuse, then fills it from the new
  manifest. The slots share nothing, so no collection rule can delete a blob the other slot needs.
- T4 with T2, the trust handoff. The loader hands over the progenitor digest and the manifest
  digest. The kernel checks only the progenitor; the progenitor checks the base set against the
  manifest digest. The kernel therefore stops depending on the base set and still depends on the
  progenitor.
- 525's slots. Two boot slots, priority, tries and confirmation live in `crates/boot_slot` and the
  UEFI chooser. This proposal reuses them: an update writes the inactive slot and marks it to try.
  Nothing here adds a third slot or a second rollback mechanism.
- The base is a resolved lock. Resolution happens at build time, not on the device, the way
  `Cargo.lock` and a Nix closure work. Basalt publishes the manifest and the packages it names;
  the device fetches and copies them.

Programs outside the base still install through `jig` under §208 and §220, and an installed package
fills a service's role without changing the base list. This proposal touches the base path only.

## What it proposes

An update is one manifest and the packages it names, copied into the inactive slot, then a
confirmation that keeps it or a failure that reverts to the running one.

1. The manifest gets its own name and file. Today "manifest" means two things: a program's ask
   in its ELF note (milestone 597 (a program carries its manifest in an ELF note)) and the list of
   packages an image holds (U4). This proposal gives the second its own name, provisionally the
   system manifest, and a file basalt writes beside the packages. It lists each package with its
   digest and names the progenitor, the same three facts that §235's T4 makes the trust root read.
2. The updater fetches and stages. A program, provisionally `update`, reads a system manifest,
   checks each package's digest against it, and copies each into the inactive slot. U4 allows a
   shortcut: for a package whose digest matches the running slot's copy, the updater copies the
   local file instead of fetching it. It wipes the staging slot whole first, so no stale file
   survives.
3. The kernel reads the new manifest through the loader. The loader passes the inactive slot's
   manifest digest, and the progenitor checks the base set against it before anything runs. A
   manifest the packages do not match draws a refusal naming the package, the same check the
   composed image already runs at build time.
4. The chooser tries the slot and the update confirms. `boot_slot` marks the staged slot with
   one try. A boot that reaches the shell confirms it; a boot that exhausts its tries reverts to
   the other slot, as 525 already does.
5. The first update is the base nife ships. Basalt publishes the system manifest with the
   packages, so a machine updates from any pinned release to any later one without rebuilding the
   tree. This is what 607 promised.

## What it does not propose

- No shared store (U2, refused by the ruling), and no collection rule.
- No signed manifest (T3, refused): the loader and the slot checksum carry the trust, as §235
  records.
- No third slot and no second rollback path (U3, not taken). `jig rollback` stays the
  installed-programs mechanism; this is the base path beside it.
- No pinned-root key in the kernel (T3's refusal stands).

## Dependencies and order

Steps 1 and 2 depend on item 1, which landed, and on 525's slots, which exist. They are host and
device code that can land before the handoff, because they write and stage files the handoff later
reads. Step 3 depends on the loader's handoff, §235's T4 with T2, which milestone 525 records as one
initrd slot on the device-tree path; the x86_64 path already has the loader's own file. Steps 4 and
5 depend on step 3.

## Where it stands

Partial. The lane `milestone/976-an-os-update-through-a-slot` broke the work into the parts below,
each landable and checkable alone, re-derived from the tree on 2026-10-31 (UTC) rather than from
the steps above. Two facts of the tree shape the order. A slot holds one UEFI program that embeds
the kernel and the archive, behind the header §244 (the installed disk has four partitions, and a
boot slot is 64 MiB) ratified. And the kernel compiles every base program's digest into its trust
root, so until milestone 607 item 6 lands, only a host that builds the kernel can make a bootable
slot. Every new name here is provisional.

## Follow-on

- **Done.** Part 1: the composed base image carries `system_installer`. The kernel's trial-boot
  confirmation enters it by name (`kernel/src/user/install_service.rs`), and
  `packages/base-image.toml` left it out. A composed image in a slot would have booted, printed
  that it cannot confirm, and rolled back once its tries ran out. The definition now lists it, the
  approval file has its entry, and `base-image`'s test
  `the_base_image_carries_every_program_the_install_path_enters` holds the definition to every
  program that file names. Checked: the test fails with the program removed and passes with it,
  and `cargo xtask base-image x86_64 --test --boot` composes it from its package and reaches the
  prompt.
- **Outstanding.** Part 2: the composer writes the file a slot holds. On x86_64, `base-image`
  wraps the composed kernel and archive in `uefi_loader`, as `uefi_stage` does for the tree's own
  image. Check: `script/boot-file-size-check` on the composed file, and an OVMF boot of it. Part 3
  is a gate that updates through a slot, provisionally `cargo xtask update-boot`. It installs the
  tree's image, stages part 2's file into slot 1 with `rollback::stage_an_upgrade`, and boots three
  times. Boot 2 must confirm slot 1, boot 3 must choose it with no tries left, as `confirm-boot`
  asserts, and the transcript must show the composed catalog. Both parts are mechanical and add no
  format; part 3 is host-staged, so it proves the slot path and not a device updater.
- **Proposed.** Part 4, which waits on a ruling: what the inactive slot receives. One composed UEFI
  program (today's format), a version 2 slot of named members a device assembles from packages, or
  nothing on the device until item 6. The proposal
  `design/roadmap/proposals/what-an-os-update-writes-into-a-slot.md` asks it, recommendation
  first, with the prior art.
- **Outstanding.** After part 4, part 5 is the device updater, steps 2 and 4 above, writing
  whatever part 4 rules. Part 6 is the system manifest's name and format (step 1). Part 7 is who may
  start a base update, and what bounds an unattended one under the approval file of milestone 911 (scripted installs grant only
  what the owner's approval file allows). Each
  comes to an architect after part 4, one at a time. Step 3, the loader's two digests, is milestone
  607's item 6 and not this block's.

## Index row

Item 5 of milestone 607 (the repository divides along what releases together) waited on item 1,
which landed. The update path now has a producer: basalt composes the bootable image from its
packages. The block names what the update writes, what it leaves alone, and the order the pieces
land. It builds on §235 (the OS is built and updated from packages, and the tree divides by what
releases together)'s U4 slot layout and T4 trust handoff.
