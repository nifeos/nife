---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: 919
decision_dependencies: 208
machine_requirements: none
specific_machine: none
needs_person: no
---
# A generation of the installed-programs table grows past one page

Raised 2026-11-01 (UTC) by lane `milestone/990-the-base-images-first-installed-programs-table`.
Every name below is provisional.

Reuse: the table's own grammar and readers. `crates/installed_programs` already parses a
generation of any length from a `&str`, so the work is the readers' buffers, not a second format.

## The failure

The progenitor and the shell read the live generation of the installed-programs table into one
4,096-byte page. `with_live_table` in `crates/system_initializer/src/image_request.rs` reads it
that way, and so do the installer's read of the old generation in
`crates/system_initializer/src/activation.rs` and the shell's `live_generation_listing` in
`components/src/swish/capabilities.rs`. The progenitor takes a larger generation for no table at
all, so it vouches for no image spawn, and the installer cannot read the generation it would
succeed. `jig` reads the file through std and holds no such limit.

An installed row with its default pointer takes about 170 bytes, so a page holds about two dozen
installed programs. A base row has no pointer and takes about 100.
`crates/installed_programs` records that limit in its `BUGS`. Milestone 990 (the base image's
first installed-programs table) measured the base image's generation 1 on aarch64 on 2026-11-01
(UTC), after the ruling that a base row carries no pointer: 58 rows, 5,691 bytes. The test image,
which adds the `fixtures` package's programs, holds 121 rows in 12,256 bytes. Neither fits a page.

## Ruled

Ruled 2026-11-01 (UTC) by the architect. Every reader of a generation takes as many pages as the
generation holds, up to a named constant bound of 64 KiB (16 pages). A reader refuses a generation
past the bound with a phrase that names the generation and the bound. It never truncates a
generation and never reads one as a shorter table. The readers are `with_live_table` in
`crates/system_initializer/src/image_request.rs`, the installer's read in
`crates/system_initializer/src/activation.rs`, and `live_generation_listing` in
`components/src/swish/capabilities.rs`.

Not taken: one 64 KiB buffer, and chunked streaming. Streaming is revisited if a table nears
64 KiB, about 650 rows.

## The work

1. Every reader of a generation takes it in as many pages as it holds, up to the 64 KiB bound, held
   as one named constant, and refuses a generation past the bound by name and by the bound.
2. The installer writes within the same bound and refuses to write a generation past it.
3. A host test in `crates/installed_programs` runs at the composed base image's size (5,691 bytes
   for generation 1; the test image is 12,256 bytes), and the swish-check gate installs a package
   over one.

## Done when

A boot whose live generation holds the composed base image's rows vouches for an installed
program, and `jig install` writes the next generation over it, on all three architectures.

## Index row

The installed-programs table holds as many programs as an image carries, not two dozen.
