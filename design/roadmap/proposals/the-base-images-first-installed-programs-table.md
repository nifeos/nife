---
status: PROPOSED
raised: 2026-10-29
milestone_dependencies: 909, 911
decision_dependencies: 208, 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# The base image's first installed-programs table

Raised 2026-10-29 (UTC) by lane `milestone/909-item-3-composition-checks-the-approval`, while
implementing item 3 of milestone 909 (basalt composes the bootable image from its packages). Item 3
asks for two things: each package passes 911's host-side check against the image's approval file,
and the image's first installed-programs table holds the resulting grant records. The check is
host-callable and needs no new format. The second half needs a place to put a generation, and this
tree has no such place in a composed image.

Reuse: the table and its grammar (`crates/installed_programs`), the approval file and its check
(`crates/approval_file`), and the writable filesystem the table already lives on (`redoxfs_server`,
`crates/nifefs`). Nothing here proposes a second table, a second grammar or a second approval
format.

## The failure

The composed image is the kernel plus the initial ramdisk the kernel hands the progenitor
(`xtask/src/base_image.rs`, `xtask/src/archive.rs`). `cargo xtask base-image` writes
`initrd.img` and the kernel, and `boot_check::boot_image` attaches the archive and **no disk**
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

## The options

**A. A seed disk beside the archive.** `base-image` writes a RedoxFS image (or a nifefs one, for a
test boot) carrying `activation/1` and `activation/current`, and `boot_check::boot_image` attaches
it. The boot path changes from archive-only to archive plus disk, and every boot of a composed
image carries a writable filesystem the installer can succeed.

Cost: a disk image is a new artifact `base-image` writes, the gate must attach it on three
architectures, and the image's definition grows from "kernel plus ramdisk" to "kernel, ramdisk and
seed disk". The seed has to be measured too, or the first table is the one thing in the boot that
no measurement vouches for. Reversible: the disk is a file, and an image that attaches none keeps
working.

**B. The initrd carries a read-only generation.** Add `activation/1` and `activation/current` to
the `nifefs` archive, and teach `with_live_table` to fall back to the archive when no disk is
attached. The image stays the kernel plus the ramdisk, and the first table is measured with every
other archive entry.

Cost: this is a new wire format. The table's grammar is the disk's, the read path is the disk's,
and the archive is a different filesystem with different limits (`nifefs`'s file count and name
rules). The fallback also has to answer a question the current code does not: what happens when a
boot has both an archive generation and a disk generation, and which one `current` names. A
read-only table on the archive and a writable table on the disk are two stores, and the proposal
would have to say which is in force and how an install moves from one to the other. Reversible:
the archive entry is additive, but the read fallback is a semantic change every table reader shares.

**C. The progenitor writes the first generation at boot.** `base-image` puts the grant records in
a new archive entry, and `system_initializer` writes `activation/1` from it on a boot with no disk.
The image stays archive-only, and the table is born on the filesystem the installer already uses.

Cost: the first generation depends on a writable filesystem, which a no-disk boot does not have.
For a boot with a disk, the progenitor writes a generation nobody asked it to write, and the
approval file's `answered` line has to survive the copy. This moves the composition's decision
from the build to the boot, which is the opposite of what item 3 asks: the image's first table is
composed on the host. Reversible: the archive entry is additive.

## Recommendation

**A**, and only after the image's own definition says a composed image carries a disk. The table is
the owner's grant record, and the installer's existing store is the one place it can be succeeded
and rolled back. A seed disk keeps that grammar and that path, and it gives an unattended update
the same filesystem the interactive install uses. B is the smaller change today, but it invents a
read-only second store and leaves the boot with two answers to "what is live"; C moves a build-time
decision to run time for no gain.

The seed disk is the expensive option, and that is not why it wins. It wins because it is the only
one that leaves the table's grammar, its writers and its readers exactly where they are. If both
cost the same, the argument does not change.

## What is not proposed

- No change to `installed_programs`, `approval_file` or the `answered` line. The records the
  composition writes are the bytes an interactive install writes, and the approval's name and
  digest go in the `answered` line.
- No second approval file. The image carries the one file `packages/base-image.toml` names, and
  the composition checks every package against it.
- No signing or pinning of the seed disk. That is milestone 917 (basalt's packages build
  reproducibly) and 607's item 6.

## What this proposal does not do

It writes no code and mints no name. The seed disk, its archive entry, its measurement and its
attach point are provisional until an implementing lane names them. `compose` keeps the check
seam, so a decision for A, B or C changes where the records go and not how they are checked.
