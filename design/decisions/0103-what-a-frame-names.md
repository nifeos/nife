---
status: SUPERSEDED
raised: 2026-08-19
superseded_by: 102
---

# 103. What a `Frame` names

Kept because the survey outlives the decision.

The fork this was researching was answered while it was being researched. calef chose option 1
on 2026-08-20 (§102, a `Frame` names a run), which is the same answer L4 reached and which this
survey found Barrelfish had already built. So nothing here is waiting on anybody.

**It is kept**, because the four recalled claims it checked are the reason a reader should trust
§102 (a Frame names a run of pages) rather than merely obey it. Two of the four framings a
maintainer offered from memory turned out to be wrong, and finding that out is what separates a
decision from a guess. The pricing in section 3 is also the closest thing this tree has to a cost
model for the system call surface, and §102 does not restate it.

This section was salvaged from a lane the account's spend limit killed mid-run, with 214 lines
uncommitted in its worktree. What follows is the lane's own text, unedited.

The number is **provisional**: this lane cut its branch from `main` at
`22da81ae`, where §100 is the highest section, and the integrator mints the real one at merge.

The fork. `Object::Frame(pa)` names exactly one 4 KiB page and occupies one of sixteen cspace
slots. Milestone 29's lane (PR #364) was briefed to grow the display scanout to 800x600 so that
§100's gohufont-14 would give a usable text grid, and **could not build it**. The blocker is the
capability model rather than memory: the virtio-gpu driver's DMA region already holds nine slots of
the sixteen, so the ceiling is nine frames and 36,864 bytes, and an 800x600 surface needs 469. A
`const` assertion at `kernel/src/user/display_service.rs` fails the build, as designed. Every
non-square shape under the ceiling gives five text rows or fewer, so no scanout reachable today
makes gohufont-14 a terminal.

This section prices four options. It gives no recommendation, deliberately. Three of the four change
what a system call method means, which is the irreversible column of the *move fast on what can be
undone* tenet. A recommendation on that fork is most of the way to a decision.

What is blocked until this is answered. Milestone 29's scanout, and therefore §100's font
landing anywhere a person can read it. Nothing else is waiting; the display path is the only place
in the tree where a single object is a run of hundreds of pages.

## The appendices

This page was split on 2026-10-12 (UTC) under [§212 (a prose
budget)](0212-a-prose-budget-for-every-document.md), word for word. The rest of the survey and the
costs moved, grouped by the option they bear on, to
[`0103-what-a-frame-names/`](0103-what-a-frame-names/README.md):

- [`frame-runs.md`](0103-what-a-frame-names/frame-runs.md): option A. The Fuchsia and Mach
  readings, Barrelfish, Genode and CHERI, revocation, and section 6's entry for A.
- [`large-frames.md`](0103-what-a-frame-names/large-frames.md): option D. The seL4 reading, KeyKOS
  and Coyotos, the encoders, the Kani harnesses, the allocator, and section 6's entry for D.
- [`one-page-frames.md`](0103-what-a-frame-names/one-page-frames.md): options B and C. Section 4's
  premise checks, the L4 flexpage reading, and section 6's entries for B and C.

---

## 1. What else was considered, and why each lost

The four options are set out in full in section 6 below with their measured prices. In summary:

| | What changes | Verdict on the evidence |
|---|---|---|
| **A. A `Frame` names a run** `(pa, pages)` | `Object::Frame`, `frame::MAP`, `frame::REVOKE`, the revocation database's key | Live. The most general answer and the one that most changes what a capability means. |
| **B. Grow `CSPACE_SLOTS`** | one constant | **Dominated, and by arithmetic rather than taste.** See section 5. A TCB is written into one 4 KiB page and a slot is 24 bytes, so **at most 170 slots fit a page even if a `Thread` held nothing else**; 475 frames needs 480. This is not "a one-number change paid in TCB size", which is what `kernel/src/cap.rs` currently says. |
| **C. `aspace::MAP_INTO` at spawn** | nothing in the model | Live, and the only option that changes no system call meaning. Costs 475 map calls, and the client ends up holding **no capability for its own pixels**, which is what milestone 108 (the drivers move onto frame capabilities) spent itself closing on this exact path. |
| **D. A large `Frame`: one 2 MiB page** | `PageFormat` gains block descriptors; `Object::Frame` gains an order | Live, and cheaper than this lane expected. `800x608x4 = 1,945,600` fits one 2 MiB page with 37 pages to spare, and `SURFACE_VA` is already 2 MiB-aligned. |

A and D are the same decision at different granularity, which is the useful way to see them. Both
make one capability name more than one page. They differ in whether the size is arbitrary
(`pages: u64`) or a power-of-two order the hardware already understands. That is exactly the split
between Barrelfish's frame capability and seL4's frame sizes, and the survey in section 3 is mostly
about what each side of it costs.


---

## 2. What this tree already does in the analogous case

Three answers, and the first one settles more than it looks like it does.

`Object::Untyped` already names a run of pages with one capability, and has since milestone 11.
It does not carry the run in the capability word: it carries a generational name into a kernel-side
table (`kernel/src/untyped.rs`, `MAX_REGIONS = 256`, over `crates/regions`), and the table holds
`(base_page, pages, watermark)`. So the tree has a worked, proved, host-tested precedent for "one
capability, N pages", and its shape is **indirection, not a fatter capability**. Option A can copy it
exactly and inherit the stale-safety that comes with it: reclaiming a region bumps its slot's
generation and every outstanding capability to it stops resolving.

The DMA path already has a `(base, size)` run type, and it is Kani-proved.
`paging::domain::DmaRegion { base, size }` is a physical run a device may touch, with
`grant_pages`/`grant_page` as the "which whole pages does this run contain" arithmetic and six
harnesses over it. Whatever a run-shaped `Frame` decides about partial pages and wrapping, that
argument is already written down and machine-checked one module over.

And the display driver's own DMA region is already a run in physics and nine capabilities in the
model, which is the whole complaint. `display_service.rs` allocates it with one
`memory::alloc_contiguous(DMA_FRAMES)` and then mints nine `Frame` capabilities over consecutive
pages of it (`grant_run`). The kernel *knows* it is one run at the moment it hands it over and throws
that fact away.


---

## 3. The prior art, read rather than recalled

Four systems were offered from memory when this fork was raised, and marked as recalled. This lane
opened the sources. One survived intact, one with a qualification, and two of the four framings are
wrong: the flexpage's verb, and Mach's paternity of the VMO. Five more systems were read because a
recalled list is not a survey, and one of them has already built option A. Everything quoted below
was retrieved from the source named beside it.

### seL4 frame sizes: correct, with one caveat about what the manual actually says

Two corrections to what was assumed alongside the claim. The manual states the **virtual**
address alignment explicitly (section 7.1.2: *"The virtual address for a Page mapping must be aligned to the
size of the Page"*, and `seL4_ARM_Page_Map` returns `seL4_AlignmentError` for it). This lane
found no sentence stating a physical alignment requirement. That follows from retype allocating
size-aligned within an untyped rather than from a rule anyone wrote down, so the manual should not be
cited for it. And the AArch64 page-size table is **not** qualified by hypervisor configuration: the
guess that `arm_hyp` or a 40-bit PA changes the frame sizes is not supported by this revision.

### Fuchsia VMOs: correct, and the qualification is where the size lives

**The qualification, and it is the one that matters here: the size is not in the capability either.**
The handle is opaque; the extent is kernel state, reached by `zx_vmo_get_size()`. That is the
**object-identity** shape, and it has a consequence this fork should weigh: delegating a subrange
costs a second kernel object. `zx_vmo_create_child` with `ZX_VMO_CHILD_SLICE` takes an arbitrary
page-aligned `(offset, size)` and mints a new handle with a parent/child relationship the kernel
tracks. Attenuating a *mapping* is free (`zx_vmar_map` takes `(vmo_offset, len)`); attenuating the
*object* is not.

### Mach memory objects: partly, and the paternity is folklore

**And the ancestry claim is not documented.** Fuchsia's own record of where Zircon came from
(`docs/concepts/kernel/zx_and_lk.md`) says:

> Zircon was born as a branch of LK and even now many inner constructs are based on LK while the
> layers above are new.

Neither the VMO page nor the ancestry page mentions Mach, and this lane found no Fuchsia primary
source claiming it. **Do not write that VMOs descend from Mach memory objects.** The defensible
sentence is that they occupy the same design slot, convergently.

### What the survey settles

**Two shapes exist and they are not a spectrum.**

- Object identity (Fuchsia VMO, Genode dataspace, L4Re dataspace, Mach memory object, and this
  tree's own `Untyped`): the capability is an opaque name, the extent is kernel state. Delegating a
  subrange costs a new kernel object.
- The run in the capability (Barrelfish frame cap): `(base, bytes)` with no rounding. Delegating
  a subrange is a retype at an offset. Pays for it in a mapping database with containment lookups.
- The power-of-two family (L4 fpages, KeyKOS, EROS, Coyotos, seL4's frame sizes) is a third
  thing rather than a middle: a region that is not naturally aligned and power-of-two sized is not
  expressible as one capability at all.

Option A is the second shape or the first; option D is the third. The tree already has a worked
example of the first (`Untyped`), which is the cheapest fact in this whole section and the one that
should probably decide the encoding once the shape is chosen.

---

## 4. Is the premise true?

Moved to [`one-page-frames.md`](0103-what-a-frame-names/one-page-frames.md).

---

## 5. What each option costs, measured

### The walk is the shared cost, and it is shared with the IOMMU

Everything below that touches page size lands in `crates/paging`, and the crate has exactly three
walks: `Mapper::map`, `Mapper::unmap`, `Mapper::translate`. All three are the same fixed loop:

```rust
for level in 0..F::LEVELS - 1 {
    let i = F::index(va, level);
    let entry = /* read */;
    if !F::is_present(entry) { /* create, or fail */ }
    table_pa = F::entry_pa(entry);
}
```

`is_present` cannot tell a table from a block, and the walk assumes table. That is the hazard,
and it is sharper than "unsupported". If a 2 MiB block descriptor existed at level 2 today and
anything mapped a 4 KiB page beneath it, `F::entry_pa` would return the *block's* physical address.
The mapper would then write page-table entries into the framebuffer. `translate` would return
pixels as a page-table address. So block support is not additive; every walk needs a new
discrimination step, and `PageFormat` needs a method it does not have.

The crate's own aarch64 test already names the trap, in
`a_descriptor_carries_the_table_or_page_bit_as_well_as_valid`: *"at L0-L2 it is a block descriptor,
which maps a 1 GiB or 2 MiB span from bits the walk meant as a table pointer."*

And the same three walks build the IOMMU domains. `paging::domain::build_identity_domain` is a
`Mapper` over the same formats, because SMMUv3 walks VMSAv8-64 and the RISC-V IOMMU walks Sv39. So a
bug in the new discrimination is a bug in device confinement as well as in process isolation. That is
the argument for why this belongs in a decision rather than in a lane's judgment.

### Option C's price, stated honestly

Option C changes no model and is therefore the cheapest to build, which under the *elegance and
performance beat implementation convenience* tenet is an argument that must say out loud that it is
an argument from effort. Its real costs:

- The client holds no capability for its own pixels. It cannot delegate the surface, cannot hand
  a peer a read-only view, and cannot have it revoked at the granularity of the object, because there
  is no object. Milestone 108 migrated this exact path away from that state and
  `notes/frames.md` records what it bought.
- 475 mappings, 475 revocation records. The records are two pages against `AS_OVERHEAD`'s sixteen
  of slack, so the cost is real and not binding.
- `MAP_INTO` is a user-built-space operation (`kernel/src/user.rs`, `user_aspace_map`, over the
  `USER_SPACES` table), and it takes a `Frame` capability in a slot as its `a1`. The display path
  spawns through `sched::run` with a kernel-built `Spawn`, not through the userspace loader. So "at
  spawn" here means the kernel calling the recorded mapping engine 475 times, which keeps
  revocability but is a different code path from the one the option's name suggests. That is a real
  change of shape, not a no-op.

### Option B is dominated, and the constant's own comment is wrong

`kernel/src/cap.rs` says: *"Growing it is a one-number change here, paid in TCB size."* Measured, the
TCB size is not a slope, it is a wall.

A `Thread` is written into one 4 KiB page (`sched.rs`, `insert_from_page` /`insert_at`, milestone
19c.2: *"Each `Thread` lives at the start of one page"*). A cspace slot is
`Option<Cap<Object>>` = 24 bytes (`Object` is 16, `Rights` is a `u32`, and `Option` takes the
discriminant's niche; measured, not estimated). Sixteen slots is 384 bytes.

- 475 surface frames plus the driver's control page needs at least 480 slots = 11,520 bytes.
- 170 slots is the absolute ceiling if a `Thread` contained nothing but its cspace, and it
  contains a saved context, a name, queue links and a good deal more.

So option B cannot reach this workload without making a TCB multi-page, which unpicks milestone
19c.2's page-resident TCB and the retype path built on it. It remains a sensible *small* change (16
to 32, say) for the ordinary pressure of a program holding a dozen objects; it is not an answer to
this fork.

Two things fall out of that measurement and want a home regardless of which option wins,
recorded here rather than left in a lane report:

1. `kernel/src/cap.rs`'s comment on `CSPACE_SLOTS` should say what the real ceiling is, because
   as written it invites exactly the change that would silently break.
2. Nothing asserts that a `Thread` fits in its page. `insert_at` does `ptr.write(thread)` into a
   4 KiB page with no `const` guard, so a `CSPACE_SLOTS` raised past the fit would scribble the next
   page and fail arbitrarily far from the edit. That is rung one of the ladder available for free:
   `const _: () = assert!(size_of::<Thread>() <= FRAME_SIZE)`.

---

## 6. The four options

Each option's entry is in the appendix for its shape: A in
[`frame-runs.md`](0103-what-a-frame-names/frame-runs.md), D in
[`large-frames.md`](0103-what-a-frame-names/large-frames.md), and B and C in
[`one-page-frames.md`](0103-what-a-frame-names/one-page-frames.md).

---

## 7. How reversible is it, and who has already acted on it

A, C and D all touch things two programs agree on, which is the *move fast on what can be undone*
tenet's first irreversible category:

- A and D change `frame::MAP`'s meaning, and `Frame::MAP` is called by seven migrated programs
  (`disk_surveyor`, the roster probe, `disk_partitioner`, `mkfs`, the virtio-gpu driver, `painter`,
  `display_terminal`). A change here is not a change to one call site; it is a change to what those
  programs mean.
- The revocation key is a wire fact, because §13's guarantee is asserted in prose, in tests and
  in `notes/frames.md`, and rests on a frame being a page. Weakening it by admitting overlapping
  frames is the kind of fact that leaves the machine.
- C changes nothing anybody has agreed on, and is therefore the reversible option. That is its
  strongest argument and it should be weighed as one.

Nobody outside this tree has acted on any of it. There is no published claim about what a
`Frame` names, so the cost is the internal one above and not a retraction.

---

## What I need from you

Which of the four. The lane can build any of them; only the choice is yours, and A and D are the two
that change what a capability means.

If the answer is D-narrow, milestone 29 is unblocked without touching `crates/regions` or the
no-double-free proof, and `Untyped::RETYPE` keeps its current meaning. If the answer is C, the
display becomes the one service in the tree whose memory is not a capability, and that should be
recorded in `notes/frames.md`'s `BUGS` rather than left to be rediscovered. If the answer is A,
`Frame::SPLIT` needs refusing in the same breath, or the equal-or-disjoint invariant goes with it.

Two small things are yours only if you want them; they block nothing:

- the `size_of::<Thread>() <= FRAME_SIZE` assertion and the corrected `CSPACE_SLOTS` comment (section 5),
  which are worth doing whichever option wins;
- whether a large frame, if D wins, is spelled as an order (seL4's shape: 4 KiB / 2 MiB / 1 GiB
  and nothing between) or as a page count (A's shape at a coarser grain). They are different
  claims about what memory is.
