# Physical memory

The bottom of the memory hierarchy. Page tables (milestone 4), the kernel heap, DMA buffers
(milestone 8), and user process pages (milestone 7) all ask this for memory, and there is
nothing underneath it to ask.

## Two questions

1. **Where is the RAM?** Answered by the [device tree](device-tree.md), which the machine
   hands us in `x0` ([boot-protocol.md](boot-protocol.md)).
2. Which parts of it are ours to give away? That's this note.

## Bitmap, not free list

The classic hobby-OS answer is a **free list**: link each free page to the next by writing a
pointer *into the free page itself*. Zero metadata overhead, O(1) alloc and free, genuinely
elegant. It's what xv6 does.

We use a bitmap (one bit per frame, `1` = used). Two reasons:

Contiguity. A free list cannot answer "give me 8 physically *adjacent* frames." A device
doing DMA reads physical addresses directly, with no MMU in the way to hide a scattered
buffer, so virtio at milestone 8 needs genuinely contiguous memory. Retrofitting that means
throwing the free list away.

Testability. A free list stores its metadata *inside the memory it manages*, so testing
it means handing it real memory and doing unsafe pointer writes. A bitmap's logic is **pure**:
given a bitmap and a request, which frame? We test it exhaustively on the host with no memory
at all, in milliseconds, with no emulator (DECISIONS §7).

The cost is 1 bit per frame: 32 KiB of bitmap per GiB of RAM. Or, for QEMU's 128 MiB:
4 KiB. One frame of overhead to manage 32768 frames. Cheap.

## The bootstrap problem

The allocator needs memory for its bitmap. There is no allocator.

The way out: carve the bitmap, by hand, out of the very memory it is about to manage, and then
have the allocator reserve those frames *from itself*. `place_bitmap` scans RAM in order for the
first frame-aligned run that clears every forbidden region, jumping past the end of whatever a
candidate hits. It does not simply take the memory after `__image_end`: `image_size` in the arm64
Image header stops at `__stack_top`, so nothing past the image is memory we told the bootloader we
wanted. QEMU happens to place the device tree and the initial ramdisk 64 MiB higher, and other
firmware need not.

The allocator's first act is to allocate itself.

## Guilty until proven innocent

`FrameAllocator::new` marks every frame used. Nothing is handed out until someone
explicitly says "this address range is real RAM."

This ordering is not fussiness. If the bitmap defaulted to *free*, the allocator would
cheerfully hand out the MMIO hole at `0x0900_0000` and give you the UART's control registers
as scratch space. Default-used means an unknown address is never allocated, and the worst
case is wasted memory rather than a machine that writes garbage to a device.

So `init` goes:

```
1. everything used
2. mark_free()  each /memory region from the device tree
3. mark_used()  the kernel image      (__image_start .. __image_end)
4. mark_used()  the bitmap itself
5. mark_used()  the device tree blob  (it's sitting in the RAM we just freed!)
6. mark_used()  every /memreserve region the firmware declared
```

Free first, reserve second. Reserving has to win.

## The rounding asymmetry, which is the whole ballgame

`mark_used` rounds the start down and the end up. It claims every frame the region so
much as *touches*.

Consider: our image ends at `0x4009_7010`. That is not frame-aligned. The frame at
`0x4009_7000` holds the last `0x10` bytes of the kernel, and 4080 bytes of nothing.

Round that end down and the frame is declared free. The allocator hands it out. Something
writes to it. The tail of the kernel is silently overwritten, and the crash lands
somewhere else entirely, much later, in code that did nothing wrong.

Over-reserving wastes at most 4 KiB per region. Under-reserving corrupts the kernel. The
asymmetry in the cost is why the asymmetry in the rounding.

There is a host test for exactly this shape (`mark_used_claims_partially_covered_frames`) and
a kernel test that states the invariant directly (`every_frame_of_the_kernel_image_is_reserved`,
which walks every frame of the image and asserts each is marked used).

## Span, not sum

We track every frame between the lowest RAM address and the highest, including any holes.

A board with RAM at `0x4000_0000` and again at `0x8_0000_0000` would give us a bitmap covering
the enormous gap between them, and we simply never free those frames. A bit of wasted bitmap
buys a much simpler index calculation: `frame_index = (addr - base) / 4096`, with no region
table to search.

If that ever becomes expensive, the fix is a per-region allocator. It isn't expensive yet.

The span is RAM's **widened by whatever the forbidden list reaches past it**. On `x86_64`, QEMU's
PVH loader places the initial ramdisk at a fixed offset below the top of guest memory. A boot whose
ACPI tables grow (VT-d plus NVMe) moves e820's usable-RAM boundary down past the ramdisk's last few
hundred bytes. The x86 front end still claims the whole ramdisk as forbidden, so the
bitmap has to reach it, or `is_used` on those frames answers `None` (out of range) instead of
`Some(true)`, and the claim is silently clamped away. The extra frames start used and are never
freed, because they are not in RAM. The top is rounded up to a whole frame, since a forbidden region
need not be frame-aligned and `page_frames_in` truncates. A forbidden region inside RAM, the case
on the other two architectures, leaves the span unchanged.

## The seam with the machine description

`memory::init` is a front end per description: a device tree on aarch64 and riscv64, PVH and ACPI
on `x86_64` (`arch::discovery::init`). Both end in `bring_up_page_frames(ram, forbidden)`, which
does not care where the description came from. What crosses the seam is RAM and reservations. The
device windows the device-tree front end also discovers (the interrupt controller, the RTC, the
UART's interrupt line, the PCIe ECAM range) are stored in `memory`'s statics. They stay `None` on a
machine with no tree, and `x86_64` fills them through `record_*` calls instead (see
[x86-port/acpi-and-pci.md](x86-port/acpi-and-pci.md)). The `record_*` calls for facts the front end
also reserves (the initial ramdisk, the boot file) store and do not reserve. The caller is already
building the forbidden list, and reserving in both places would double-reserve or let each side
assume the other did it.

## The low-water ledger

`memory::allocation_pressure` reports the fewest frames that were ever free at once on a boot,
how many allocations were refused and the largest refused request. The frame ledger in `testing.rs`
reads the allocator before the first test and after the last; what a boot runs out of is the
middle, and an exhaustion shows up far from the tests that spent the memory. Measured 2026-09-26 at
`484f3ebe` plus #1347, from the closing `frames:` lines:

| | free before the first test | lowest free | free after the last | refused |
|---|---|---|---|---|
| aarch64 | 57033 | **23654** | 34790 | 0 |
| riscv64 | 57882 | **24682** | 35780 | 0 |
| `x86_64` | 58841 | **35719** | 46813 | 0 |

Every low sits about 11,100 frames under its own end-of-suite figure, during a progenitor test
(`the_report_child_builder_parses_an_elf_and_builds_a_running_child`, or on riscv64
`the_least_authority_demo_builder_passes_its_demo_an_argument`). Both carve a 12,288-page building
budget (`memory_region::create(12288)` in `kernel/src/user.rs`) and give it back. So the ledger at
the low-water is [frames.md](frames.md)'s held list plus that one reservation: the held list growing
moves the low, and the refusal count catches a load that no longer fits. Contiguity is not tracked
per test: `largest_free_run` never moved from the first test's reading, and a bitmap scan per test
overran the `x86_64` 17 GiB leg's time bound. A refused `alloc_contiguous` is the contiguity failure
itself, and it is counted. The figure reports and does not gate, for `sched::MAX_THREADS`'s reason:
the held list grows on purpose, and the ledger's two end-of-suite gates hold the permanent part to
account.

## No heap, and an interrupt-safe lock

The kernel has no heap: no `Vec` and no `Box` (`kmem` is a fixed carve, see
[kernel-budget.md](kernel-budget.md)). The regions coming out of the device tree go into
fixed-size arrays of `MAX_REGIONS` entries, because a fixed-size array is what you use when there
is no allocator.

`ALLOCATOR` is an `IrqSafeMutex` (DECISIONS §9 (locking: IrqSafeMutex, plus a discipline)), not a
bare spinlock. An interrupt handler that tried to allocate while the interrupted code held a bare
spinlock would spin forever for a lock only the code it interrupted can release. The discipline
that goes with it: interrupt handlers do not allocate. They record what happened and defer the
work.

---

*Add to this file as new memory concepts come up.*

## BUGS

- The seam's type is still `device_tree_blob::Region`, a plain `{ start, size }` pair, so a machine
  with no device tree names a device-tree type. Moving it belongs with widening the seam.
- At most `MAX_REGIONS` (16) RAM regions. Both `virt` boards describe one; q35 describes three. A
  caller with more must decide what to drop.
