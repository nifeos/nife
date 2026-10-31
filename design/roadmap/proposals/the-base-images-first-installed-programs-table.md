---
status: PROPOSED
raised: 2026-10-29
milestone_dependencies: 909, 911, 981
decision_dependencies: 34, 208, 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# The base image's first installed-programs table

Raised 2026-10-29 (UTC) by lane `milestone/909-item-3-composition-checks-the-approval`, while
implementing item 3 of milestone 909 (basalt composes the bootable image from its packages). Item 3
asks for two things: each package passes 911's host-side check against the image's approval file,
and the image's first installed-programs table holds the resulting grant records. The check is
host-callable and needs no new format. The second half needs a place to put a generation, and a
composed image has no such place.

Ruled 2026-10-30 (UTC) by calef, on #2894: the first table lives on the UFS2 launch volume, and a
seed image is not a separate artifact. The options below are the fork that ruling answered.

Reuse: the table and its grammar (`crates/installed_programs`), the approval file and its check
(`crates/approval_file`), and the
launch filesystem the table lives on: UFS2, through milestone 981 (UFS2, read and write: a Rust
server with FreeBSD's tools as the oracle). Nothing
here proposes a second table, a second grammar or a second approval format.

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

So there is no generation in a composed image. Putting one there is a choice about the image's
shape and the boot path, not a line of code inside `compose`.

## The options the ruling weighed

### A. The launch volume carries the first generation

`base-image` writes the launch filesystem's image, a UFS2 volume carrying `activation/1` and
`activation/current`, and `boot_check::boot_image` attaches it. The boot path changes from
archive-only to archive plus a launch volume, and every boot of a composed image carries the
filesystem the installer succeeds on and rolls back.

Cost: the launch volume is a new artifact `base-image` writes, the gate must attach it on three
architectures, and the image's definition grows from "kernel plus ramdisk" to "kernel, ramdisk and
launch volume". The gate must measure the volume too, or the first table is the one thing in the
boot that no measurement vouches for.

The alternative container for A, a bespoke nife-native store beside the archive, holds the same
shape with a smaller format of its own. It is the option the 2026-10-29 ruling first chose, and the
2026-10-30 ruling replaced the container with the launch filesystem.

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

**A**, with the container changed to the UFS2 launch volume. The table lives on that volume as files
under `activation/`, and a seed image is not a separate artifact: generation 1 is present on the
volume at build time. Three facts follow.

- The launch volume is the installer's store too, so the seed and the interactive install share one
  filesystem. The question the options left open, which store is live when both exist, does not
  arise, because there is one.
- The table's grammar stays nife's. `installed_programs` owns the generations and the `current`
  pointer, and `approval_file` owns the check. UFS2 owns only the container.
- The volume ships with the composed image, so the first table arrives with the same measurement as
  every other boot artifact.

The choice is not the cheaper one. A bespoke nife-native store could land today, and the launch
volume waits for a UFS2 server that does not exist. It wins because it leaves one store, one format
and one writer, and because a second store would owe a migration to the launch filesystem later.

## What is not proposed

- No change to `installed_programs`, `approval_file` or the `answered` line. The records the
  composition writes are the bytes an interactive install writes, and the approval's name and
  digest go in the `answered` line.
- No second approval file. The image carries the one file `packages/base-image.toml` names, and
  the composition checks every package against it.
- No signing or pinning of the launch volume. That is milestone 917 (basalt's packages build
  reproducibly) and 607's item 6.

## What this proposal does not do

It writes no code and mints no name. The launch volume's attach point, its measurement and the
`mkfs` that writes `activation/1` are provisional until an implementing lane names them. `compose`
keeps the check seam, so the ruling changes where the records go and not how the composition checks
them.

The UFS2 server this waits on is milestone 981, whose phase 2 (make, judge, read) is what a launch
volume needs. That milestone carries the store as one of its deliverables.
