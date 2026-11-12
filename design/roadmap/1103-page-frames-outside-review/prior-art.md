# Prior art

An appendix to [the milestone](../1103-page-frames-outside-review.md).

How the allocator sits against what a reader already knows. Download figures are crates.io totals
and the last 90 days at the time of the review. Details marked unverified are from memory of the
sources, not from reading them during this review.

| system | unit and shape | mark usable / reserved | contiguous | alignment | rounding of usable memory |
|---|---|---|---|---|---|
| Linux memblock and the buddy allocator | `struct page` per frame, free lists by order | `memblock_add` / `memblock_reserve`; `memblock_trim_memory(PAGE_SIZE)` drops partial pages from usable memory (unverified name) | `alloc_pages(order)`, power of two | by order | inward for usable, outward for reserved (unverified) |
| FreeBSD `vm_phys` | segments and freelists by order | `phys_avail[]` rounded with `round_page` / `trunc_page` at startup (unverified) | `vm_phys_alloc_contig` with alignment and boundary | yes | inward |
| OpenBSD `uvm_pmemrange` | page ranges | `uvm_page_physload` takes page numbers, so the caller rounds inward (unverified) | `uvm_pglistalloc` with alignment and boundary | yes | inward |
| seL4 | no kernel allocator; untyped memory is retyped by user level | the boot info lists untyped regions | by retype of one untyped | by object size | not applicable |
| Zephyr `k_mem_paging` | one `z_page_frame` struct per frame | compile-time RAM bounds | no | no | not applicable |
| `x86_64` 0.15.5 (5.9M / 1.0M) | `PhysFrame<S>`: `from_start_address -> Result<_, AddressNotAligned>`, `containing_address`; `unsafe trait FrameAllocator { allocate_frame }`, `FrameDeallocator` | the implementor's business | no | by page size parameter | the implementor's business |
| `buddy_system_allocator` 0.13.0 (3.9M / 1.4M) | `FrameAllocator`: `add_frame(start, end)`, `alloc(count)`, `alloc_aligned(Layout)`, `dealloc(start, count)` | `add_frame` | yes, power of two | yes | the caller passes frame numbers |
| `bitmap-allocator` 0.4.6 (177K / 61K) | `BitAlloc`: `alloc`, `alloc_contiguous(size, align_log2)`, `dealloc`, `insert(range)`, `remove(range)`, `test`; a segment tree over a bitmap | `insert` frees, `remove` reserves | yes | yes | the caller passes frame numbers |
| `bootloader_api` 0.11.17 (232K / 103K) | `MemoryRegions` with `MemoryRegionKind::Usable`; the blog's `BootInfoFrameAllocator` steps regions by 4096 from `start` | the bootloader | no | no | inward by construction (unverified) |

Where this crate differs: it borrows its bitmap (`&mut [u8]`) and allocates nothing. It has no
`unsafe`. It starts every frame used and asks the caller to declare what is RAM, which no listed
crate does. Its scans are linear with a hint and it offers no order, alignment or boundary. It
carries Kani proofs of distinctness, range and indexing. It is the only one in the table that rounds
usable memory outward (item 4). Its `from_addr` panics where `x86_64` returns a `Result` (item 14),
and it has no `free` for a run where `buddy_system_allocator` has `dealloc(start, count)` (item 12).
