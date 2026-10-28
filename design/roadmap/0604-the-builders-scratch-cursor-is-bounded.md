---
status: BUILT
raised: 2026-09-26
built: 2026-09-26
promoted_from: the-builders-scratch-cursor-is-bounded
---
# 604. The builder's scratch cursor is bounded

Promoted on 2026-09-26 from the proposal `the-builders-scratch-cursor-is-bounded`.
The lane for milestone 206 (a program image has under 896 KiB) filed it while siting every window on
the address-space map, and lane `milestone/604-scratch-cursor` built it the same day. *(Number
provisional: minted by the lane, to be confirmed at merge. 596 to 603 were held on `main` and open
pull requests when it was taken.)*

## The finding, and a correction to it

`supervision_protocol`'s loader maps every page it fills for a child into the builder's own address
space. Its cursor started at `0x1000_0000` and advanced one page per page built. It never came back,
because nothing in the ABI unmaps (DECISIONS §162 (whether a holder can give up a mapping) was decided as `AddressSpace::UNMAP` on 2026-10-05 and is not built). The proposal said the progenitor's cursor reaches the kernel's initrd window at `0x2000_0000`
after about a hundred `ripgrep`-sized spawns, and every build fails after that. That is on the
customer path: `rg` at the prompt is the goal of milestone 595 (the shell runs a `std` program, and
`rg pattern` works at the prompt).

The arithmetic, computed from the tree's constants rather than measured on a progenitor:

- `ripgrep`'s image spans 668 pages on aarch64 (`0x40_0000..0x69C_000`, notes/ripgrep-on-nife.md),
  and 669 scratch pages a spawn on `x86_64` and `riscv64`, which add the timebase page. Stack pages go
  straight into the child and cost the builder nothing.
- The window to the initrd is 65,536 pages, less the boot servers', which is about 97 spawns.
- But the page tables run out first. Every 512 scratch pages need a new last-level table from the
  progenitor's own budget, `INIT_OWN_PAGES`, 128 pages. `job_undertaker` is built out of the same
  budget (about 24 to 28 pages: its image, twelve stack pages, its tables, address space and thread),
  and the first image request takes two more tables. That leaves about 100 tables, 51,000 pages:
  about seventy-five spawns, failing as out of memory rather than already mapped. The records
  that said "a hundred" now say this.

The count is reasoned, not reproduced on a progenitor: `rg` is in no CI archive (the reasons of
milestone 121 (`ripgrep` on nife)), and the job pool cannot yet hold a program that size (`grant_plan::STD_REGION_PAGES` is
384). The same failure was reproduced in a builder the test controls, below.

## The fix: the kernel was already giving the pages back

The proposal's gate was `DECISION`, on the reasoning that bounding the cursor needs an unmap. It
does not. A scratch page maps a frame retyped from the child's region, and destroying that region
revokes every mapping of its pages, the builder's included (DECISIONS §13 (frame revocation)). So a
scratch page is free again once its child is reaped. Only the cursor did not know it.

- The cursor lives in a fixed window and wraps. `supervision_protocol::SCRATCH_WINDOW`,
  `0x1000_0000..0x1400_0000`, 64 MiB, 16,384 pages, whose last-level tables cost a builder at most
  `SCRATCH_TABLE_PAGES`, 32.
- The kernel's page table is the free list. `supervision_protocol::map_scratch` tries
  `PageFrame::MAP` at the cursor. The kernel refuses a page that is already mapped, and refuses it
  whole, so that refusal (`BadPointer`) means "taken" and the next page is tried. Any other refusal
  is returned. A whole lap with nothing free is `Err`, which means the builder holds 16,384 pages of
  live children at once. Pages that stay mapped for good (the boot servers') cost one refused probe
  per lap each.
- The image request moved onto it. The progenitor's `receive_image` took a contiguous run with
  `scratch_pages`, which is gone; it maps each of the caller's frames with `map_scratch`. The shell
  destroys its staging region once the answer is in, so those pages come back too. The argv page
  of milestone 205 (how a foreign program is told what to do), which landed on `main` while this was in
  review, moved the same way: `receive_args` and `copy_args` call `map_scratch`.
- The progenitor's numbers are checked against the window at compile time. `INIT_OWN_PAGES` must
  be at least twice `SCRATCH_TABLE_PAGES`, and the job pool plus an image request's pages must fit in
  half the window, leaving the other half for the boot servers' permanent pages. Milestone 595 will
  grow the pool for `rg`, and a pool grown past that fails the build rather than a spawn.

No syscall surface, no new method, no capability slot: the progenitor's table stays at 23 of 24.

Would we still choose this if the alternatives cost the same? Yes. The alternatives were the two
the proposal named, an unmap after each build and a fixed per-build window. Both need §162's unmap,
which is a permanent addition to the syscall surface and calef's call. The window needs nothing new
and uses a guarantee the kernel already makes and already tests. What it does not do is remove the
builder's mappings of a *live* child, which is §162's question and stays open.

## The test

`kernel::user::scratch_window_tests::a_builder_reuses_scratch_its_reaped_children_gave_back`, with
`fixtures/src/scratch_window_exerciser.rs` (name provisional). The fixture forges a 668-page ELF and
builds a child from it 40 times, each out of a region it destroys before the next. That is 26,720
scratch pages, 1.6 laps of the window. Its own page tables come from a 48-page region, which covers
the window's tables and not the 53 a climbing cursor needs. It was 110 builds, past the progenitor's
old 65,536 pages, until CI's aarch64 leg failed to finish them inside the test's 60 seconds.

The negative control, measured on aarch64 on 2026-09-26, with the wrap taken out of
`map_scratch`: 36 builds, 24,048 scratch pages, then the 37th refused. With the wrap: all 40, in 5 seconds
locally (110 took 35 seconds on a host 5.6 times oversubscribed).

It runs only when named (`script/test --test scratch_window`), not in the whole suite. There it
took 104 seconds on aarch64 (CI run 36279440107) and ran past the `x86_64` leg's timeout, because
every region reap scans every live mapping log once per page (the proposal below). Run by name on
2026-09-26, it passed on all three: 3 seconds on aarch64, 10 on riscv64, 2 and 3 on the two
`x86_64` legs. That is a weaker gate than CI, recorded as an exception in the test's BUGS.

Cross-ISA: one portable test body on all three architectures. The loader is portable userspace code
and the revocation is portable kernel code (DECISIONS §19 (architectural parity is a tenet)).

## BUGS

- The count above is computed, not measured on a progenitor. The boot servers' share of the
  window was never measured; the whole archive is under 2,400 pages of file, so the compile-time
  check leaves half the window for them.
- The probe is one thread's. `map_scratch` reads, probes and writes the cursor without a lock.
  Every builder in the tree is one thread; two threads building at once would waste a probe, not
  share a page, because the kernel refuses the second mapping.
- A builder still holds a mapping of every live child's pages, readable and writable. Bounding
  the cursor does not change what DECISIONS §162 is about, and `notes/trusted-init.md`'s honest
  limits still apply.
- `BadPointer` means "taken" only because the address is always valid here. The kernel returns
  the same error for a misaligned or kernel-half address. A distinct `AlreadyMapped` error would make
  the probe say what it means; it is an ABI change and not worth one on its own.

## Follow-on

- **Recorded.** The count is computed rather than measured on a progenitor, the probe is one
  thread's, a builder still maps every live child, and `BadPointer` doubles as "taken": the BUGS
  above, and beside the code in `crates/supervision_contract/src/lib.rs`.
- **Decision.** Removing a builder's mapping of a live child is still
  `design/decisions/0162-giving-up-a-mapping.md`, unchanged by this.
- **Milestone 595.** Its pool growth for `rg` is now checked against the window at compile time.
- **Milestone 658.** Milestone 658 (a region reap scans every mapping on the machine, once per page). A region reap scans every live mapping log once per page, which made this test take
  over a minute in the whole suite and makes every job reap cost more the more is running:
  `design/roadmap/0658-a-region-reap-scans-every-mapping-on-the-machine-per-page.md`.
- **Recorded.** The test keeps 32 frames (33 on riscv64 and `x86_64`) that nothing here explains,
  and CI does not run it: `system_tests/src/user/scratch_window_tests.rs`.

## Index row

The tree's one userspace loader maps every page it builds into the builder's own space and cannot
unmap, so its cursor only climbed, and the progenitor would have stopped spawning after about
seventy-five `ripgrep`-sized programs. The cursor now wraps inside a fixed 64 MiB window and lets
the kernel say which pages a reaped child gave back, with no new syscall surface; a guest test
builds past the old limit on all three architectures.
