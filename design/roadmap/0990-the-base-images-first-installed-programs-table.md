---
status: PARTIAL
raised: 2026-10-29
promoted_from: the-base-images-first-installed-programs-table
milestone_dependencies: 911
decision_dependencies: 208, 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# 990. The base image's first installed-programs table

Raised 2026-10-29 (UTC) by lane `milestone/909-item-3-composition-checks-the-approval`, while
implementing item 3 of milestone 909 (basalt composes the bootable image from its packages). Item 3
asks for two things: each package passes 911's host-side check against the image's approval file,
and the image's first installed-programs table holds the resulting grant records. The check is
host-callable and needs no new format. The second half needs a place to put a generation, and a
composed image has no such place.

Ruled 2026-10-30 (UTC) by calef, on #2894: the first table lives on the launch volume, and a seed
image is not a separate artifact. The options below are the fork that ruling answered. Ruled
2026-10-31 (UTC) by calef: that ruling carries over unchanged to RedoxFS, which is now the launch
store (decision 34). The same day, decision 34 refused UFS2, which the ruling had named. In force
now: the first table is files `activation/1` and `activation/current` on the RedoxFS launch volume.

## Where it stands

Partial. Item 1 ships: `cargo xtask base-image` writes `launch.img` beside the kernel and
`initrd.img`, a RedoxFS volume holding `activation/1` and `activation/current`. Items 2 and 3 do
not exist yet, and the `BUGS` below say what blocks them.

Generation 1 holds one base row per carried program: the program name and the program member's
digest, in the owner's vouch shape (no version, `owner` as package and manager) and with no default
pointer. Ruled 2026-11-01 (UTC) by calef, on #3023: a base row carries the program name and digest
and no pointer. Section 229 (how a bare name at the prompt reaches an installed program) refuses
base names for installed rows as before, and does not refuse them for a base row. The row names no
pointer target. It does not tie to the system manifest of milestone 976 (an OS update is a set of
packages that lands through a slot).

An installed row always has a pointer and a base row never does.
`a_base_row_has_no_pointer_and_an_installed_row_has_one` in `xtask/src/base_image.rs` holds both.
`first_generation` in `xtask/src/base_image.rs` writes it with `installed_programs::with_entries_granting`, the writer
an interactive install calls, and reads the grants with `program_asks::asked_grants` over the bytes
the archive carries. The `answered` line names the image's approval file and its SHA-256, and a
generation with no approval file records no answer. A carried program that asks for a grant when
the image names no approval file refuses the composition, because nobody allowed it.
`write_launch_volume` stages the two files and drives `tools/redoxfs_host`'s `mkfs` and `import`,
the tool the swish-check seed uses.

Measured 2026-11-01 (UTC) on aarch64: the base image's generation 1 is 58 base rows and no
pointers, 5,691 bytes, with no grant lines and no `answered` line, and the test image's is
12,256 bytes. Both are still over the progenitor's 4,096-byte page.

Reuse: the table and its grammar (`crates/installed_programs`), the approval file and its check
(`crates/approval_file`), and the launch filesystem the table lives on, RedoxFS, which nife already builds and runs.
`seed_installed` in `xtask/src/disk.rs` already seeds a RedoxFS disk for the swish-check gate, so
the host-side tool that writes files into a RedoxFS image exists. Nothing here proposes a
second table, a second grammar or a second approval format.

## The failure

The composed image is the kernel plus the initial ramdisk the kernel hands the progenitor
(`xtask/src/base_image.rs`, `xtask/src/archive.rs`). `cargo xtask base-image` writes
`initrd.img` and the kernel, and `boot_check::boot_image` attaches the archive and no disk
(`xtask/src/boot_check.rs`, line 18: "It attaches the archive but no disk"). The initial ramdisk is
a read-only `nifefs` image, parsed by `open_archive` for the programs and the measurement table.

The installed-programs table is somewhere else. It is a generation file under `activation/` on the
writable filesystem (`crates/installed_programs`), read by the progenitor through `FsCalls` on the
file service (`crates/system_initializer/src/image_request.rs`, `with_live_table`). A boot with no
disk answers every table read with `None`, and the installer writes nothing. `seed_installed`
(`xtask/src/disk.rs`) seeds a RedoxFS disk for the swish-check gate, and says plainly that it
writes no table.

So no boot of a composed image reads a generation. Putting one there is a choice about the image's
shape and the boot path, not a line of code inside `compose`.

## The options the ruling weighed

### A. The launch volume carries the first generation

`base-image` writes the launch filesystem's image, a RedoxFS volume carrying `activation/1` and
`activation/current`, and `boot_check::boot_image` attaches it. The boot path changes from
archive-only to archive plus a launch volume, and every boot of a composed image carries the
filesystem the installer succeeds on and rolls back.

Cost: the launch volume is a new artifact `base-image` writes, the gate must attach it on three
architectures, and the image's definition grows from "kernel plus ramdisk" to "kernel, ramdisk and
launch volume". The gate must measure the volume too, or the first table is the one thing in the
boot that no measurement vouches for.

The alternative container for A, a bespoke nife-native store beside the archive, holds the same
shape with a smaller format of its own. It is the option the 2026-10-29 ruling first chose, and the
2026-10-30 ruling replaced the container with the launch filesystem, now RedoxFS.

### B. The initrd carries a read-only generation

Add `activation/1` and `activation/current` to the `nifefs` archive, and teach `with_live_table` to
fall back to the archive when the boot attaches no disk. The image stays the kernel plus the
ramdisk, and the measurement covers the first table with every other archive entry.

Cost: this is a new wire format. The table's grammar is the disk's, the read path is the disk's, and
the archive is a different filesystem with different limits (`nifefs`'s file count and name rules).
The fallback also has to answer a question the current code does not: what happens when a boot has
both an archive generation and a disk generation, and which one `current` names. A read-only table
on the archive and a writable table on the disk are two stores, and the proposal would have to say
which is in force and how an install moves from one to the other.

### C. The progenitor writes the first generation at boot

`base-image` puts the grant records in a new archive entry, and `system_initializer` writes
`activation/1` from it on a boot with no disk. The image stays archive-only, and the first
generation appears on the filesystem the installer already uses.

Cost: the first generation depends on a writable filesystem, which a no-disk boot does not have. For
a boot with a disk, the progenitor writes a generation nobody asked it to write, and the approval
file's `answered` line has to survive the copy. This moves the composition's decision from the build
to the boot, which is the opposite of what item 3 asks: the host composes the image's first table.

## The ruling

**A**, with the container changed to the RedoxFS launch volume. The table lives on that volume as files
under `activation/`, and a seed image is not a separate artifact: generation 1 is present on the
volume at build time. Three facts follow.

- The launch volume is the installer's store too, so the seed and the interactive install share one
  filesystem. The question the options left open, which store is live when both exist, does not
  arise, because there is one.
- The table's grammar stays nife's. `installed_programs` owns the generations and the `current`
  pointer, and `approval_file` owns the check. RedoxFS owns only the container.
- The volume ships with the composed image, so the first table arrives with the same measurement as
  every other boot artifact.

A bespoke nife-native store would have been a second format beside the launch filesystem. The
launch volume wins because it leaves one store, one format and one writer, and because a second
store would owe a migration to the launch filesystem later. RedoxFS is already running, so this
milestone no longer waits on a filesystem server that does not exist.

## The work

1. `cargo xtask base-image` writes a RedoxFS launch volume beside the kernel and the initial ramdisk.
   Generation 1 under `activation/` holds the grant records `compose` produced, one per package,
   and `activation/current` names it.
2. `boot_check::boot_image` attaches the volume on all three architectures, and the progenitor's
   `with_live_table` reads generation 1 from it with no change to its read path.
3. The image's measurement covers the volume, so the first table carries the same measurement as
   every other boot artifact.
4. Milestone 909's item 3 closes on this: the composed image's first table holds the records its
   approval file allowed.
5. Its grant rows read the base programs' own manifest notes (ruled 2026-11-01 UTC on #2981), so they
   wait on milestone 992 (the base programs carry their manifests in notes), which follows milestone 697
   (the trust root leaves the kernel binary).

## Done when

A composed image boots on all three architectures with its launch volume attached, the progenitor
reads generation 1 from it, and a `jig install` on that boot writes generation 2 to the same
volume.

## What this milestone does not do

- No change to `installed_programs`, `approval_file` or the `answered` line. The records the
  composition writes are the bytes an interactive install writes, and the approval's name and
  digest go in the `answered` line.
- No second approval file. The image carries the one file `packages/base-image.toml` names, and
  the composition checks every package against it.
- No signing or pinning of the launch volume. That is milestone 917 (basalt's packages build
  reproducibly) and 607's item 6.
- No new filesystem server. RedoxFS is the launch store (decision 34), and this milestone uses the
  server and the host tool that already exist. It starts once milestone 911 (scripted installs grant only what the owner's approval file allows) lands.

Name: provisional. `launch.img`, `first_generation`, `base_row`, `test_approval_text`, `write_launch_volume` and the volume's
16 MiB size. Items 2 and 3 name the attach point and the measurement. `compose` keeps the check seam, so
the ruling changes where the records go and not how the composition checks them.

## BUGS

- **Nothing attaches the launch volume.** `--boot` boots the kernel and the archive alone, so no
  boot reads generation 1 yet. Four things stand between it and item 2:
  - **The progenitor reads one page of a generation.** It reads the live generation into 4,096
    bytes, and the base's generation 1 is 5,691. On a boot with this volume attached, the
    progenitor would vouch for no image spawn and the installer could not read the generation
    it succeeds. Item 2 cannot keep its read path unchanged. Ruled 2026-11-01 (UTC) by the architect:
    every reader takes as many pages as the generation holds, up to a named bound of 64 KiB (16
    pages), and refuses a generation past it by name, never truncating; the installer writes within
    the same bound. Home: the proposal
    [a-generation-grows-past-one-page](proposals/a-generation-grows-past-one-page.md).
  - **A base row claims no name at the prompt.** Ruled 2026-11-01 (UTC) by calef, on #3023: a base
    row carries the program name and digest and no pointer. So the shell's `Bare::Both` refusal
    (§229 (how a bare name at the prompt reaches an installed program)) never meets a base name in
    the live generation. Done: `base_row` in `xtask/src/base_image.rs`.
  - **No ruling settles the measurement** (item 3).
    [notes/the-first-installed-programs-table.md](../../notes/the-first-installed-programs-table.md)
    lists the options: the seed generation by its own digest, the volume until its first write,
    or the approval file alone. Any of them reaches the kernel's trust root, which milestone 697
    (the trust root leaves the kernel binary) is changing.
  - **The kernel mounts the second block device.** Its file service serves virtio block slot 1
    (`kernel/src/user/fs_service/wiring.rs`), and the runners put their `nifefs` fixture disk at
    slot 0. A boot that attached the launch volume alone would put it where nothing mounts it.
    Item 2 chooses the slot on all three runners.
- **A host of another architecture reads no manifest notes.** `elf::Elf::parse` refuses an ELF
  built for a machine other than the host's (`WrongMachine`), and `program_asks::asked_grants_of`
  turns that refusal into "asks for nothing". `cargo xtask base-image aarch64` on an x86_64 host
  therefore skips the approval check and writes no grant lines, where the same command on an
  aarch64 host refuses an image whose fixtures ask for the network. The composition cannot tell
  "asks nothing" from "unreadable". Build image checks on a host of the image's architecture
  until the composer reads a foreign note or refuses when it cannot.
- **The volume is 16 MiB**, the size of the RedoxFS fixture the kernel tests mount. Nothing yet
  installs onto a composed image's volume, so nothing has measured what it needs.

## Follow-on

- **Outstanding.** Item 2, attaching the volume and reading generation 1 at boot. Checked
  2026-11-01 (UTC) against `xtask/src/boot_check.rs`, which attaches no disk; the `BUGS` above
  list what blocks it.
- **Proposed.** A generation larger than a page, ruled 2026-11-01 (UTC) to read up to a 64 KiB
  bound (16 pages) and refuse past it:
  `design/roadmap/proposals/a-generation-grows-past-one-page.md`.
- **Outstanding.** Item 3, the measurement, waits on a ruling and on milestone 697 (the trust
  root leaves the kernel binary). Checked 2026-11-01 (UTC): the note names three options and no
  ruling picks one.
- **Outstanding.** Item 4 closes when item 2 does. Item 5's grant rows fill as the base programs'
  notes ask: `first_generation` already reads the notes, and today they ask for nothing.

## Index row

The composed base image carries its first installed-programs table on the launch volume, so the
first boot already holds the grants its approval file allowed.
