---
status: BUILT
raised: 2026-08-04
built: 2026-08-14
---
# 108. The drivers move onto frame capabilities

Built 2026-08-14 (PR #141). Raised 2026-08-04 from `notes/frames.md`, whose section "What this
unlocks, and what is left" then closed with the migration it deliberately did not do: "This note
builds the object and proves it; migrating the existing users to it is separate work." That section
has since given way to "The migration (milestone 108)".

The finding. There are two mechanisms for sharing a page between a driver and its client. The older
one is still in use everywhere.

The **`Frame` object** is the general one, and it is proved. `RETYPE` mints a frame from untyped and
hands back a zeroed page. A holder maps it, narrows the rights, and delegates. The test
(`a_frame_capability_shares_a_page_and_a_read_only_view_cannot_write_it`) shows both halves. The
consumer reads the producer's sentinel through its own mapping, and a writable mapping of the
read-only view is refused. The sharing half self-verifies in a pleasing way, because `RETYPE`
zeroes. A consumer that had somehow mapped the wrong page would read zero rather than the sentinel.
Reading the sentinel can only mean it mapped the producer's page. And the confinement half is proved
able to fail. Stub the `WRITE` check in `Frame::MAP` and the read-only view becomes writable.

The **spawn-time static mapping** is the older one. The kernel places a page into both address
spaces when it starts the program, through the `maps:` field of `user::Spawn`.
`start` in `kernel/src/user/console_service.rs` does it for the console, `start_probe` in
`kernel/src/user/disk_service.rs` for the virtio path, and the display and date services do the same. The page is wired at
construction and there is no capability anywhere in the arrangement.

Why two mechanisms for one thing is the cost. The spawn-time mapping is not attenuable: the kernel
decides the rights and nobody can narrow them afterwards. It is not delegable: a driver cannot hand
its buffer on. It is not revocable through the path §13 (capability revocation and untyped reclamation) built, since a `Frame` capability is what
`Frame::REVOKE` and the mapping database are indexed by. More to the point for a demonstrator: a
reader looking at how nife shares memory finds the general capability answer in the note and the
special case in the code. The special case is what every real driver uses. The thesis is that
authority is visible in what a program holds. A page a program was handed at birth is not
visible anywhere.

What it costs. Each migrated service gains a `RETYPE` and a delegation where it had a `Mapping`
literal. That is more code at each site and one fewer mechanism in the system. It also moves the
buffer's provenance into the spawn literal. That is where CLAUDE.md says a process's whole
authority should be readable. Nothing about the driver protocols changes. This is about who holds
the page, not what goes in it.

## Scope note

The console is the awkward one and should probably go last. It comes up before most of the system
exists, which is why its UART base is hardcoded on purpose. A bootstrap that needs a
capability service to print is a bootstrap that cannot report its own failure. Migrate the disk and
display paths first, where the ordering is comfortable, and treat the console as a separate decision
with its own argument.

**Not a change to `Frame` itself.** The object, its rights ladder and its proof are done. If the
migration finds the object short of something a real driver needs, that is a finding worth
recording. It is a design fork rather than a quiet addition.

Related, and distinct: milestone 95 (an unmap primitive). 95 is about a holder giving a mapping
*back*; this is about who holds it in the first place. A driver on frame capabilities is a driver
whose buffer could be revoked with §13's existing machinery. That is a reason to sequence them
in either order and not to merge them.

## Follow-on

- **Recorded.** `notes/frames.md`: the console did not migrate and is still handed its page by the
  spawn-time static mapping. It comes up before most of the system exists. A bootstrap that needs a
  capability service to print cannot report its own failure, which is why the note says it is
  deliberately last and this block calls it a separate decision with its own argument.
- **Recorded.** `notes/frames.md`: the compositor path is not in this milestone at all. So
  `display_terminal` still receives spawn-time mappings in `MODE_WINDOW`, and `date` keeps its
  `Spawn::maps` clock page in the kernel's test wiring. It is the one spawn literal in the tree
  where both mechanisms appear at once. Migrating it means touching the shell's spawn path and
  §67's grant manifest.
- **Recorded.** `notes/frames.md`: each migrated program costs one more untyped region held for the
  life of the process. That is against a region table with a finite number of slots and a page frame pool
  milestone 107 already found at the edge.

## Index row

The `Frame` object is built and proved, including a confinement check proved able to fail. The
console, disk and display services still take a page wired into both address spaces at spawn. Two
mechanisms for one thing. The one every real driver uses carries no capability, so the buffer's
authority is visible nowhere
