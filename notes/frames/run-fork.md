# A PageFrame that names a run: how the fork was found, priced and decided

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/frames.md](../frames.md), resolved on 2026-08-26. The entries it sits beside are in [limitations.md](limitations.md). Everything below
the first section describes a state this tree no longer has, and is kept as the record.

## Resolved 2026-08-26

Resolved by DECISIONS §102 (a Frame names a run of pages), built by the terminal-size lane of milestone 142 (a text display good enough that people use it
instead of a GUI). `Object::PageFrame` now carries a page count (`PageFrame(phys, count)`). `page_frame::MAP`
and `page_frame::REVOKE` operate on the whole run, and `cap::page_frame_run_cap` mints one
capability for a multi-page region instead of one per page.

The display driver's DMA region moved to this. So did the painting client's and display terminal's
surface grant, and the compositor's screen (plus a capture client's read-only mirror of it). That is
what makes the grown scanout fit a sixteen-slot capability table at all. It is 683 page frames at
1120x624 (900 at the 1280x720 this was first built and measured against), and
`crates/graphics_contract/src/lib.rs`'s `WIDTH` has the reasoning. At one capability per page no
size past 128x64 would have fit.

`display_service::DRIVER_SLOT_DMA`'s `const` assertion (below, and the error it used to produce) is
retired along with the pressure it guarded against. It was replaced rather than deleted. The
reformulated assertion beside those slot constants makes the same claim in run terms: no grant list
on that path reaches the fault slot. Deleting it outright was the state milestone 142's review
found. The hole it left was a future lane pushing a service's highest slot onto `FAULT_EP_SLOT` and
booting instead of failing the build.

## A `PageFrame` names one page, and a DMA region is a run of them

The virtio-gpu driver's region is nine contiguous pages. So it holds nine capabilities and issues
nine `MAP` calls for memory that is adjacent in physics and adjacent in its address space. The
IOMMU domain the kernel programmed for it covers it as a single range. That is slots 5 through 13 of
a sixteen-slot capability table (`cap::CAPABILITY_TABLE_SLOTS`), one of which is reserved for the
fault endpoint. It fits with one slot spare, and a wider scanout would not fit at all.
`display_service::DRIVER_SLOT_DMA` carries a `const` assertion so that someone who widens the
surface fails the build rather than the boot.

The milestone's scope note called this out in advance ("if the migration finds the object short of
something a real driver needs, that is a finding worth recording, and it is a design fork rather
than a quiet addition"), so it is recorded and not fixed. The fork is whether a `PageFrame` should
be able to name a run of pages. seL4 has no answer to copy here. It retypes N frames and you hold N
capabilities, and its capability tables are radix trees rather than sixteen slots, so the pressure
lands somewhere else. Growing `CAPABILITY_TABLE_SLOTS` is a one-number change paid in TCB size, and
is the other half of the same question.

## The fork stopped being hypothetical on 2026-08-19

Here is what it costs, measured rather than asserted. The terminal-sized scanout of milestone 29 (a display terminal) needs
800x600 to reach 100x42 characters. The terminal-font decision chose gohufont-14, which is 8x14, so
the 128x64 surface gives a 16x4 grid that is not a terminal. The lane that tried it got the
assertion above, at 640x480, before it got anything else:

```
error[E0080]: evaluation panicked: the display driver's DMA region no longer fits its capability table
beside the fault slot: a PageFrame names one page and this region is a run of them
  --> kernel/src/user/display_service.rs:50:15
```

## The ceiling is nine page frames, and it is the capability table rather than the memory

The driver's DMA region starts at slot 5 and must end below the fault slot at 15. So
`SURFACE_PAGE_FRAMES <= 9`: at most 36,864 bytes, or 9,216 pixels. Every non-square shape that fits
(128x72, 144x64, 192x48) gives five text rows or fewer at 8x14. 800x600 needs 469 page frames, which
no sixteen-slot capability table can hold under any arrangement of the other slots. The same is
true of every size a person would call a terminal.

The other budgets on the path are all comfortable by comparison, which is the part that surprises.
469 page frames is under one percent of the free pool. The mapping records are two pages against
`AS_OVERHEAD`'s sixteen of slack. `MAP_BUDGET_PAGES`'s eight pages still cover the page tables (see
below). Nothing else on this path is short. Only the slots are.

## Two of those numbers have moved since

The constants a reader would quote should be the current ones (updated 2026-09-02, milestones 230
and 231). The table is twenty-four slots rather than sixteen, `Option<Cap<Object>>` is 32 bytes
rather than 24, and `MAX_THREADS` is 256 rather than 128. The paragraphs above are left as written,
because they are the record of an investigation on the day it was done. None of the arithmetic's
*conclusions* survive being updated any differently. 469 page frames does not fit in twenty-four
slots either, and option 2 now prices 512 slots at 16 KiB a thread across twice as many threads.
`kernel::cap::CAPABILITY_TABLE_SLOTS`' own doc carries why it moved, which is worth reading before
quoting any figure here.

A boot now says what it used, which is the thing this argument was conducted without.
`kernel::cap::report_peak` prints `capability slots: N of M at peak` from the scheduler's idle loop,
and `script/swish-check` echoes it on every run: the interactive boot reaches 22 of 24. That number
cost milestone 230 (`script/shell-check` red) four instrumented boots to learn and now costs a boot.
That is the difference between a fork priced against a guess and one priced against a measurement.
See design/roadmap/0231-capability-slot-high-water-mark.md.

Milestone 111 (a shell that can endow a child with entropy) gave the progenitor a reason to hold the entropy
service's request endpoint for the whole boot rather than release it after the login block. The gate
caught the move on the first run rather than at the next silent halt. Two of the three slots of
headroom are left.

## Three ways out, priced

1. A `PageFrame` that names a run. `Object::PageFrame(u64)` carries a page count,
   `page_frame::MAP` maps the run, `page_frame::REVOKE` unmaps it. Measured surface: 4 sites match
   on `Object::PageFrame` and 21 construct one through `cap::page_frame_cap`. It is the option this
   tree's own reasoning points at. The run is already one range in physics, one range in the address
   space and one range in the IOMMU domain. It collapses 469 capabilities, 469 syscalls and 469
   mapping records into one of each. It is a change to the meaning of a syscall method, which is a
   boundary rather than a habit (`AGENTS.md`, DECISIONS §10 (process model) and §16 (object revocation)).
2. Grow `CAPABILITY_TABLE_SLOTS`. One number in `kernel/src/cap.rs` and its twin in `crates/abi`,
   paid in TCB size. `Option<Cap<Object>>` is 24 bytes, so 512 slots is 12 KiB of capability table
   per thread against today's 384 bytes, and `MAX_THREADS` is 128. It moves
   `abi::fault::FAULT_EP_SLOT`, which is defined as `CAPABILITY_TABLE_SLOTS - 1` and which every
   supervised program agrees on. It leaves 469 `MAP` calls and 469 mapping records in place, so it
   buys the pixels without buying any of the elegance.
3. Map the run into the client's space without giving it capabilities, which
   `address_space::MAP_INTO` already does. The spawner holds the `AddressSpace`, maps each page frame
   into it, and deletes its own cap between iterations. So one slot serves the whole run and the
   client holds none. This needs no model change and no new method. Unlike the `Spawn::maps`
   mechanism it replaced, every mapping it makes is recorded and therefore revocable (§13 (capability revocation and untyped reclamation), §67 (a program's second stream is a declaration)). Its
   cost is 469 kernel-side map operations at spawn, and a client that cannot delegate or revoke its
   own surface, which is authority it has no use for. This is the cheapest correct option, and the
   one to reach for if the answer to 1 is "not yet".

## Two sizing facts the same investigation established

They will be the next questions. The page-table budget survives 800x600. One L3 covers 512 pages
and 469 fits, so `MAP_BUDGET_PAGES`'s eight pages still hold. But that holds only because
`SURFACE_VA` is 0x60_0000 and therefore 2 MiB-aligned. The comment justifying that constant says
"every mapping here lands inside one 2 MiB window", and that sentence is load-bearing. 1024x768 is
768 pages and does not fit one L3, which makes 800x600 the last size the current budget justifies.

And the userspace VA map does not survive it. `display_terminal` puts `OUT_VA` at 0x68_0000, only
128 pages above `SURFACE_VA`. So a 469-page surface would run straight through it and through
`CTL_VA` at 0x69_0000. Both have to move above 0x80_0000. That puts them in a second 2 MiB window
and costs a second L3, still inside the eight-page budget.

## One more arithmetic trap in the chosen size

800 x 600 x 4 is 1,920,000 bytes, which is 468.75 pages. So the surface does not fill whole page
frames, and `graphics_contract`'s build-time assertion that it must
(`SURFACE_BYTES.is_multiple_of(4096)`) fails. With an 800-pixel width, the height must be a multiple
of 32 for the surface to be a whole number of page frames. 608 is the nearest, giving 475 page
frames exactly and a 100x43 grid. The alternative is to grant `div_ceil` page frames and let the
last one be three quarters used. That means relaxing an assertion that exists so a client is never
handed a partial page.
