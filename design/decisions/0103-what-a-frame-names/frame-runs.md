# §103 appendix: option A, a `Frame` names a run

Part of the survey behind [§103 (what a `Frame` names)](../0103-what-a-frame-names.md), the reading and the costs that bear on option A, the one [§102 (a `Frame` names a run)](../0102-frame-names-a-run.md) chose. Moved out of that page on 2026-10-12 (UTC) under [§212 (a prose budget)](../0212-a-prose-budget-for-every-document.md), word for word: only headings and line breaks changed. The main page is complete without this one, which exists to verify or challenge it. Its section numbers are the original page's.

## The prior art

### Fuchsia VMOs: correct, and the qualification is where the size lives

The claim was that one handle names an arbitrarily-sized memory object and page granularity is not
in the capability model. **Correct.** From Zircon's own system call definitions
(`zircon/vdso/vmo.fidl`, read at `fuchsia.googlesource.com`):

> `zx_vmo_create()` creates a new, zero-filled, [virtual memory object] (VMO), which represents a
> container of zero to *size* bytes of memory managed by the operating system. [...] The size of the
> VMO will be rounded up to the next system page size boundary [...] One handle is returned on
> success, representing an object with the requested size.

And the framebuffer case is spelled out, privileged: `zx_vmo_create_physical(resource, paddr, size,
out)` turns a `(paddr, size)` run into one handle and requires an MMIO resource, while
`zx_vmo_create_contiguous` requires a Bus Transaction Initiator handle. **Contiguity is a driver
privilege there, not a property of ordinary memory**, which is the same division this tree already
makes between `Frame` and `DeviceFrame`.

### Mach memory objects: partly, and the paternity is folklore

Two halves, and both need correcting.

A Mach memory object is a port right, so it is a capability, but it carries no size. The extent
is a parameter of the mapping call, not a property of the object. GNU Mach's reference manual, §5.5
(`gnu.org/software/hurd/gnumach-doc/Mapping-Memory-Objects.html`):

> *memory_object* is the port that represents the memory object: used by user tasks in `vm_map`
> [...] Within a memory object, *offset* specifies an offset in bytes.

with `size` a separate argument of `vm_map`. And a memory object is backing store served by a
userspace pager, not a run of physical pages: *"A memory manager is a server task that responds to
specific messages from the kernel in order to handle memory management functions for the kernel."*
So it answers "how do I name backing store" rather than "how do I name 469 physical pages", and it is
*further* from this fork than a VMO is. The thing in Mach that resembles a VMO is the kernel-internal
`vm_object`, which is not a port and therefore not a capability at all; conflating the two is the
specific error available here.

### What the four did not include, and one of them is this fork's exact shape

Five more systems were read because a maintainer's list is not a survey. Three change the picture.

### Barrelfish: `(base, bytes)` in the capability, and the honest price

This is option A, built, documented to the bit layout, and shipped. Barrelfish Technical Note
013, *Capability Management*, rev 3.0, §3.2.8 (`barrelfish.org/publications/TN-013-CapabilityManagement.pdf`):

> A frame capability refers to a page-aligned region of physical memory with a size that is a
> multiple of 4096 bytes. A frame capability may be mapped into a domain's virtual address space
> (by copying it to a VNode).

```
datatype frame cap "Frame capability" {
   base   64 "Physical base address of mappable region";
   bytes  64 "Size of the region";
};
```

And it makes a split this tree should copy or refuse deliberately. §3.2.4:

> A RAM capability refers to a naturally-aligned power-of-two-sized region of kernel-accessible
> memory.

So the allocation type keeps the power-of-two, naturally-aligned discipline that makes an
allocator and a derivation tree tractable, and only the mappable type relaxes to
page-aligned-multiple-of-4096. That is precisely the seam this tree has between `untyped` (which is
where the no-double-free proof lives) and `Frame` (which is what gets mapped). It says option A can
be taken without touching `crates/regions`' arithmetic at all, which is the same conclusion
D-narrow reached by a different route.

The price Barrelfish pays, and it is the one to price here. Arbitrary overlapping ranges need a
mapping database to make revoke work, and TN-013 records that its insert *"is logarithmic time in the
size of the mapping database"*. This tree's `revoke` is a flat table keyed on an exact physical
address. Option A does not merely change a key; it changes a lookup from equality to containment,
and containment over overlapping ranges is a different data structure.

Worth knowing for a different reason: Barrelfish's L1 CNodes are resizable to 2^24 slots. **It did
not solve this by growing the cspace**; it grew the cspace for unrelated reasons and solved this by
putting the run in the capability.

### Genode, and one encoding note

Genode's dataspace is L4Re's answer generalised (Genode Foundations 24.05, §3.4.1):

> A dataspace is an RPC object that resides in core and represents a contiguous physical
> address-space region with an arbitrary size.

One capability, arbitrary size, physically contiguous by definition, with the size retrieved by
invoking `size()` rather than read out of the capability. Partial delegation is at the region map:
a holder *"has the option to attach a mere window of the dataspace"*. Same object-identity shape as
Fuchsia, arrived at independently.

And one encoding note, from CHERI Concentrate (IEEE ToC 2019), because it is the only system here
that measured what an explicit run costs. CHERI's 256-bit format stored base, length and address as
three independent 64-bit fields and was abandoned for size; its predecessor's power-of-two
segments were rejected because *"this power-of-two alignment restriction prevents precise enforcement
of irregular object sizes"* and padding *"results in severe memory fragmentation"*. The compromise
the paper credits (Low-fat) stores a run as base-block, top-block and a block-size exponent, which
read with the page as the block is: **store a page count, not a byte length.** If option A wins and
capability width ever matters, that is the shape to reach for.

---

## What it costs

### Revocation, and the invariant that is actually at stake

This is the part of the fork that is not about page tables, and it is the reason A and D are
irreversible rather than merely expensive.

`Object::Frame(pa)` has a property nothing states out loud: two frames are equal or disjoint.
`revoke::record_mapping(phys, root, va)` keys on the exact physical address and `revoke_frame(phys)`
deletes every capability naming *that* page. Give a frame a size and capabilities can overlap: a
2 MiB frame and a 4 KiB frame inside it are two different objects over the same memory, and
`revoke_frame` on either would miss the other.

The invariant survives if large frames are minted only where memory is spent exclusively (the
kernel's own `alloc_contiguous`, or a watermark bump that cannot hand the same page out twice), which
is how seL4 keeps it: retype consumes the untyped. **It does not survive a `Frame::SPLIT`**, the
operation somebody will ask for within a week of a large frame existing. Whichever option wins,
`Frame::SPLIT` is a separate decision and should be refused by default rather than added quietly.

---

## The option

### A. A `Frame` names a run: `(base, pages)`

What changes. `Object::Frame` carries a length, or (following `Untyped`) a generational name into
a kernel table that holds it. `frame::MAP` maps the whole run and needs a page count or a per-page
loop inside the kernel. `frame::REVOKE` and `revoke::record_mapping` key on a run rather than a page.

Cost. The system call surface (`frame::MAP`'s meaning changes for every existing caller), the
revocation database's key, and the overlap invariant above. No paging change at all: a run of 4 KiB
pages is 475 ordinary leaf mappings, so `crates/paging` and its 18 harnesses are untouched.

The price the prior art names and this lane would otherwise have missed: `revoke` currently looks
a mapping up by equality on a physical address, and runs turn that into containment, over
ranges that may overlap. Barrelfish, which built exactly this capability, pays for it with a mapping
database whose insert is *"logarithmic time in the size of the mapping database"*. That is the real
cost of option A, and it is not in the system_call.

And the prior art also names the mitigation. Barrelfish keeps its *allocation* type
(RAM) naturally-aligned and power-of-two and relaxes only its *mappable* type (Frame) to a
page-multiple run. This tree has the same seam: `untyped` is where the no-double-free proof lives and
`Frame` is what gets mapped. Option A can therefore leave `crates/regions` alone, which is the same
place D-narrow lands from the other direction.

What it forecloses. Nothing structurally; it is the most general answer. It is also the one that
cannot be walked back, because "a Frame is a page" is a sentence in a dozen files and in the head of
anyone who has read them.

What it buys beyond this milestone. Any future driver with a large DMA region, which is every
driver worth having: a NIC's ring buffers, an NVMe queue pair, a second display.
