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
- T4 with T2, the trust handoff. The loader hands over the progenitor digest and the digest of the
  archive's `program_measurements` table, not this system manifest, which stays out of the boot path
  (ruled 2026-11-01 UTC). The kernel checks only the progenitor; the progenitor checks the base set against the
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
3. **The kernel reads the new manifest through the loader.** The updater verifies the system manifest when it
   writes the slot, and a manifest the packages do not match draws a refusal naming the package, the
   same check the composed image already runs at build time. The loader passes the inactive slot's
   `program_measurements` table digest, and the progenitor checks the base set against it before
   anything runs.
4. The chooser tries the slot and the update confirms. `boot_slot` marks the staged slot with
   one try. A boot that reaches the shell confirms it; a boot that exhausts its tries reverts to
   the other slot, as 525 already does.
5. The first update is the base nife ships. Basalt publishes the system manifest with the
   packages, so a machine updates from any pinned release to any later one without rebuilding the
   tree. This is what 607 promised.

## What it does not propose

- No shared store (U2, refused by the ruling), and no collection rule.
- No signed manifest in the boot path (T3, refused): the loader and the slot checksum carry the
  trust, as §235 records. An unattended base update checks a signature before it writes (part 7).
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
- **Done.** Part 2: the composer writes the file a slot holds. On x86_64, `base-image` wraps the
  composed kernel and archive in `uefi_loader` through `uefi_stage_archive`, which `uefi_stage`
  now calls with the tree's own archive, and writes `<out>/esp/EFI/BOOT/BOOTX64.EFI`. With
  `--release` the kernel is stripped of its DWARF by the routine the tree's own image uses, and
  the run ends by holding the file to `script/boot-file-size-check --composed <out>`. A debug run
  wraps the file and skips the check, and says so. The base-image test
  `the_composed_boot_file_is_held_to_the_size_check` holds the script's `--composed` mode to a
  passing file, an over-budget file and a missing one. Checked: `cargo xtask base-image x86_64
  --release` composed a 6,463,488-byte file (50,736 loader, 1,032,656 kernel, 5,380,096 archive)
  that passed the size check, and one bounded OVMF boot of it reached the shell prompt. That
  image carries no `--test` fixtures, so the kernel's `userspace` and `budget` self-tests report
  FAILED for the missing fixture programs; the boot continued to the progenitor and the shell.
- **Done.** Part 3: a gate that updates through a slot, `cargo xtask update-boot` (provisional name).
  It builds and installs the tree's image, composes the x86_64 release image
  (`cargo xtask base-image x86_64 --release`), stages part 2's `BOOTX64.EFI` into slot 1 with
  `rollback::stage_an_upgrade`, and boots three times. It runs `confirm-boot`'s three boots through
  one function the two gates share (`confirm::confirm_an_upgrade`), which now takes the file to
  stage and what boot 2 must show of the catalog. Boot 2 must choose slot 1, confirm it and read
  the file `mkfs` wrote; boot 3 must choose slot 1 with no tries left and write nothing. Boot 2
  also types `jig list`, and the transcript must show every package of the composed archive's own
  catalog (15 lines) and none of the 13 packages that only the installing image's catalog has. A
  slot that held the install image again passes `confirm-boot` and fails here. This is version 1
  slot staging by the host, so it proves the slot path takes a composed image and a boot confirms
  it; it does not prove a device updater, which part 5 builds. It adds no format and no
  dependency. Checked: one run under OVMF on x86_64 ended `update-boot: PASS`. Boot 2 printed
  `slot 1 confirmed. This upgrade will not roll back.` Boot 3 printed `slot 1 was already
  confirmed; nothing written.` The disk ended at slot 1 priority 3, 0 tries, successful. The gate
  is not wired into `script/test` or a workflow. Like `confirm-boot` it takes several minutes under
  TCG, and it builds the install image and the composed image one after the other. A CI leg would
  roughly double `confirm-boot`'s cost for a path that parts 4 and 5 replace. It stays a manual
  gate until part 5's updater supplies the real staging.
- **Recorded.** Ruled 2026-11-01 (UTC): `update-boot` is interim. When milestones 695 (the kernel takes two
  digests from the loader) and 697 (the trust root leaves the kernel binary) land, it folds into
  `confirm-boot`. Staging the slot first becomes a scenario of that gate, and the separate gate is
  deleted or merged. Nothing changes before then: both images exist and the kernel still
  depends on the base. The name `update-boot` stays provisional and is not ratified. 697's block
  records the end state.
- **Done.** Part 4a: `crates/boot_slot` reads and writes the version 2 header (`boot_slot::members`,
  provisional). `parse` reads versions 1 and 2. It refuses each of these with a typed `Refusal` and a fixed
  phrase: an unknown version, an unknown, duplicate or missing tag, a layout break, nonzero
  reserved or padding bytes, and a CRC-32 mismatch. Two more reasons, `NoHeader` and `Truncated`,
  cover an empty partition and a short buffer. `MemberHeader::encode` writes only version 2. Host tests
  cover the round trip, every refusal, version 1 as before, and a flip of every byte of the 188
  and of the padding. A fuzz target is not added, because the fuzz job is at its eighteen-target
  budget; the crate's `BUGS` records that. The ruling leaves two points open, and both are refused
  strictly for now. A zero-length member is a layout break. So is a member inside the header block.
- **Done.** Part 4b: the archive writer is `crates/initial_ramdisk_writer` (provisional name), which
  the host composer and the device updater share. It is `no_std` with `alloc`; it takes the
  programs, the catalog and the index sources and returns the image, and it refuses an archive
  without `jig` or `mdr` and a program outside the image band. `xtask/src/archive.rs` reads the
  programs in and writes the file out. Checked: `cargo xtask base-image x86_64` wrote a
  9,391,104-byte archive with the same SHA-256 before and after the move, and a fixture archive in
  the crate's tests holds the digest the old packer gave it.
- **Outstanding.** Part 4c makes the chooser (`uefi_loader/src/chooser.rs`) call `members::parse`,
  load the members and print each refusal's phrase, and teaches xtask to write version 2. It also widens `boot_slot::members` from the 188 bytes part 4a
  wrote to the 252 bytes ruled below: the two digest fields, the CRC range and the padding check.
- **Outstanding.** Part 4: the slot becomes a version 2 header naming members, the kernel, the
  composed archive and the system manifest (ruled 2026-10-31 UTC; the version 1 header stays
  readable). The chooser loads the kernel and archive itself. Basalt
  publishes the kernel as a package, which is interim. Milestones 695 (the kernel takes two digests
  from the loader) and 697 (the trust root leaves the kernel binary) are milestone 607 item 6,
  launched in parallel. They end the kernel's dependence on the base, and the package goes when
  they land. The reasoning and the prior art are in
  `design/roadmap/proposals/what-an-os-update-writes-into-a-slot.md`.
- **Outstanding.** After part 4, part 5 is the device updater, steps 2 and 4 above, writing
  part 4's version 2 slot. Part 6 is the system manifest's name and format (step 1; ruled
  2026-11-01 UTC). Part 7 is the base-update grant under the approval file of milestone 911
  (scripted installs grant only what the owner's approval file allows), ruled 2026-11-01 UTC. The
  file gains a base-update section with `allow` and `unattended`, both false unless a person sets
  them. An unattended update writes only the spare slot, carries a manifest signed by an accepted
  key, has a strictly greater version than the running one, and starts with no slot on trial. A
  base version is MAJOR.MINOR.PATCH, three integers. The accepted keys live beside the trust root;
  the approval file names which the owner accepts and never adds one. The reasoning is in the
  proposal. Step 3, the loader's two digests, is milestone
  607's item 6 and not this block's.

- **Outstanding.** Ruled 2026-11-01 (UTC), part 4's header. Version 2 is 252 bytes in a 4096-byte block, the rest
  zero. The fields are magic 8, version 4, reserved zero 4 and three 56-byte entries (tag 8, offset 8,
  length 8, SHA-256 digest 32). Then come the progenitor's SHA-256 digest 32 and the base digest 32
  (the digest of the archive's `program_measurements` table). A CRC-32 of bytes 0 to 248 sits at bytes 248 to 252. The
  updater writes the two digests from the system manifest and the progenitor. The chooser copies them
  into the kernel command line words and never computes them (option A of question 2 in the proposal
  for milestone 695 (the kernel trusts two digests from the loader)). The base word is `nife.base`; the progenitor word's spelling is open. The tags are KERNEL, ARCHIVE and MANIFEST
  (provisional spellings), exactly three, with no count field. Members start at multiples of 4096 in
  table order, without overlap, inside the partition. The chooser refuses an unknown, duplicate or
  missing tag, a layout break, nonzero reserved or padding bytes (padding is after byte 252), and an unknown header version
  (it reads 1 and 2). It skips a refused slot for that boot, writes nothing to the partition-table
  bits and prints a fixed reason phrase per check. The updater writes only version 2. When
  both slots are refused, the chooser prints each slot's reason and then boots its own embedded
  image, as milestone 525 (a bad upgrade cannot brick the machine) rules for two bad slots. The
  proposal has the full list.
- **Proposed.** `design/roadmap/proposals/what-an-os-update-writes-into-a-slot.md` holds it (provisional milestone, ruled 2026-11-01 UTC): the updater replaces the chooser
  file on the EFI system partition before it writes the first version 2 slot, and only if the new
  chooser reads version 2. It writes the new chooser under a second name, verifies it by digest,
  then renames it over the old name; the rename is the commit. The chooser is also the install-time
  fallback image, so a torn write never leaves no file. No existing milestone updates that file on
  an installed disk, and part 5 waits on this one for disks already installed. Open questions: room
  on the EFI system partition for a second copy, whether the FAT rename is atomic enough for a
  power cut, and the effect on the unattended machines of milestone 858 (lab machines update
  themselves).

## Index row

Item 5 of milestone 607 (the repository divides along what releases together) waited on item 1,
which landed. The update path now has a producer: basalt composes the bootable image from its
packages. The block names what the update writes, what it leaves alone, and the order the pieces
land. It builds on §235 (the OS is built and updated from packages, and the tree divides by what
releases together)'s U4 slot layout and T4 trust handoff.
