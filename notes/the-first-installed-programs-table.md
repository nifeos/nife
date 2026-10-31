# The composed image's first installed-programs table

*Name: provisional, minted 2026-10-31 (UTC) by lane milestone/909-item-3-one-reading-of-a-programs-asks.*

Milestone 909 (basalt composes the bootable image from its packages) item 3 has two halves. The
composer checks every package against the image's approval file, and `cargo xtask base-image`
already does that. The other half writes the grant records that check approves into the
image's first installed-programs table. That table lives as files under `activation/` on the launch
volume, currently RedoxFS. The files and the absence of a seed image were ruled on 2026-10-30 UTC on
#2894. The table therefore waits on milestone 990. The fork and its options are in
[milestone 990's block](../design/roadmap/0990-the-base-images-first-installed-programs-table.md).

A lane built the option the ruling replaced, a nife-native container beside the initial ramdisk,
before the ruling landed. Nothing of it merged. This note keeps what that build found, for the lane
that writes `activation/1` onto the launch volume (milestone 990).

## What the first generation holds

The build settled the generation's contents, and none of it depends on the container:

- One row per program the image carries: the program, the image's version, the owning package, the
  program member's digest, and `installed_programs::OWNER` as the manager. The writer is
  `installed_programs::with_entries_granting`, the same one an interactive install calls, with no
  default and no generation before it.
- The grants are `program_asks::asked_grants` over each carried program, the reading the
  progenitor's install uses, so the composed table and an installed one cannot disagree about what a
  program asked for.
- The `answered` line is `Answered::Approval` with the approval file's name and the SHA-256 of its
  bytes, and the image carries those bytes too. The build never settled the entry's name, and its
  last uncommitted state referred to one no crate defines. A later update is then checked against
  the file that answered the first generation.
- The composer refuses before writing anything when any program asks past its package's entry, so
  no table exists for an image the approval file does not allow.

## What measured boot needs from any carrier outside the archive

The measurement table inside the `nifefs` archive vouches for archive entries only. Nothing vouches
for bytes that ride beside the archive unless the kernel's trust root names them. The
container build added that name (one manifest line beside the program measurements) and two
refusals in the kernel's hand-over to the progenitor:

1. A kernel built against a table refuses to hand over when the table is absent.
2. A kernel built without one refuses to hand over when unnamed bytes are present.

The boot needs both refusals. The first stops an image booting with its grants stripped; the second
stops an attacker adding grants to an image that had none. The check runs before the progenitor
starts, because the progenitor grants the network and resolver zones from this table when it
spawns a program. A table read after that point would already have granted what it says.

The launch volume inherits the question in a harder form. A digest fixed at build time vouches for
generation 1 and for nothing an install writes after it, and an install writes the same volume. So
the measurement cannot cover the whole volume. The lane that builds milestone 990 has to decide what it covers. The
options are the seed generation by its own digest, the volume until its first write, or the
approval file alone, with each later generation checked against it as the install already does.

## Why the ruling set the container aside

A second store would have owed a migration onto the launch filesystem, and two stores raise a
question no reader should have to answer: which one `current` names when both exist. One volume, one
format and one writer answer it by having one store. The container also needed a new length
accessor in `nifefs`, so the kernel could find where the archive ended and the container began.
That accessor shows how far the container sat from anything the boot already read.
