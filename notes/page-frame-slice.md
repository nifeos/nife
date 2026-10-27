# `PageFrame::SLICE`: a capability naming part of a run

*(Milestone 599 (a frame per filesystem client channel), built 2026-09-27 by lane
`milestone/599-pool` on calef's ruling of that day, option 4 of
[the pool's fork](a-frame-per-filesystem-client-channel.md). The method's name and number, and this
page's stem, are provisional. The `design/decisions/` record is the maintainer's to mint; this page
is its source, written in the shape §102 (a Frame names a run of pages) was.)*

## What it is

`invoke(frame, SLICE, first, count, _)` derives a new `PageFrame` capability naming the `count`
pages that start `first` pages into `frame`'s run, with `frame`'s rights. It returns the slot the
new capability landed in, the way `Rendezvous::BADGE` does.

| | |
|---|---|
| Needs | `GRANT` on `frame` |
| Makes | `PageFrame(base + first * 4096, count)`, same rights, in a fresh slot of the caller's table |
| Refuses | `NotPermitted` without `GRANT`; `BadPointer` for `count` of 0, a range past the run's end, or `first + count` overflowing; `OutOfMemory` when the table is full |
| Changes | nothing about `frame`, its mappings or any other capability |

§102 anticipated it: a consumer "can hold two capabilities: `Frame(phys, 401)` and
`Frame(phys + 401 * 4096, 74)`". §102 did not say how the second is made from the first, and until
now nothing could make it.

## Why it exists

The file service keeps eight client windows (§230 (badged endpoint capabilities)), and the
progenitor has to put exactly one of them in each client. `MAP_INTO` maps a capability's whole run,
so a capability per window was the only way, and the progenitor's 24-slot table cannot hold seven
more. Now the kernel grants the whole pool as one run in the slot that held window 0's page, and the
progenitor slices one page of window `w`, maps it into the client, and deletes the slice. That costs
no permanent slot and one transient one per build.

Option 5, an offset on `MAP_INTO`, was refused in the ruling: its narrowing lasts one call while the
holder keeps the whole pool, and it changes an argument shape §102 kept fixed. A slice narrows by
deriving a capability, the same move as a badge.

## Why `GRANT`

Revocation in this tree is scoped to the exact object: `PageFrame::REVOKE` deletes the capabilities
equal to its own and unmaps what was mapped under that object (§132 (what `PageFrame::REVOKE` owes
an overlapping run), option C). A slice is a different object. So revoking the pool does not delete
a slice, and does not unmap what a client mapped through one. Making a capability that its source's
revocation cannot reach is a delegation-class power, so it takes the delegation right, as `BADGE`
does.

## Where the pool is used

- `kernel/src/user/fs_service.rs` allocates all eight windows as one contiguous run, and
  `kernel/src/user/progenitor_boot.rs` grants the progenitor a run capability over it at slot 6.
- `crates/system_initializer`: a job behind a directory grant gets window `w` (round robin over
  1 to 7), sliced for its caretaker and the job, with the file service's endpoint badged `w`. The
  shell, `login`, the identity provisioner and the progenitor's own activation calls keep window 0,
  each through a slice.
- The identity provisioner is built at the capability table's peak, so its slice is made after its
  address space is built and `req_frame` has been dropped, not before. `script/swish-check` reads 23
  of 24 at peak on aarch64 and riscv64, unchanged. The x86_64 leg passes too, and its gauge reads
  the hand-over mark rather than the peak, as that leg's own line says.
- The test is `kernel::syscall::tests::a_slice_maps_only_its_window`: a slice of one page maps that
  page and neither neighbor, and the refusals refuse.

## BUGS

- A slice escapes its source's revocation, for the reason above. Nothing revokes the pool: the
  kernel never takes it back and the progenitor holds its `GRANT` only to delegate. So this is a
  record, not a hole. The day something revokes a pool, it must also revoke the slices, and the way
  is §132's overlap sweep (`delete_page_frame_capabilities_overlapping`), which reclamation already uses.
  The derivation tree does not give it for free, because this tree has no derivation tree: an
  object's identity is its base and length.
- The progenitor is not told when a job dies, so a window goes back into rotation after seven more
  granted jobs whether or not its last holder has exited. An eighth concurrent granted job shares
  a window with the oldest one. `Windows` in `crates/system_initializer` records it.
- `login`'s sessions share window 0 with the shell. Nothing in the shipped boot connects to
  `login`, so no two of them are live at once today.
