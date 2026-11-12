# Prior art

An appendix to [the milestone](../1100-memory-region-heap-outside-review.md).

How the crate sits against what a `no_std` allocator user already knows. Recent downloads are the
last 90 days at the time of the review.

| allocator | recent / all-time downloads | how it gets more memory | the seam a host test uses | returns pages |
|---|---|---|---|---|
| `dlmalloc` 0.2 (Rust std's wasm allocator) | 2.67M / 19.7M | asks its `Allocator` trait for a system chunk, extends the top chunk when contiguous | the `Allocator` trait (`alloc`, `remap`, `free_part`, `free`, `page_size`), with a host implementation | yes, trims |
| `linked_list_allocator` 0.10 | 1.62M / 7.19M | `Heap::init` once, `extend` by an adjacent run | none; an arena in a test | no |
| `buddy_system_allocator` 0.13 | 1.41M / 3.89M | `LockedHeapWithRescue`: a closure called on exhaustion adds memory | the rescue closure | no |
| `talc` 5.1 (this crate's core) | 1.18M / 2.55M | `Source::acquire` is called when `try_allocate` fails; `Manual` refuses | the `Source` trait; `claim`, `extend`, `truncate` | `truncate`, when the top is free |
| `embedded-alloc` 0.7 | 753k / 3.40M | `Heap::init(start, size)` once, fixed arena | none | no |
| `rlsf` 0.2 | 614k / 2.42M | `FlexTlsf` asks a `FlexSource` for pages (`alloc_page`, `realloc_inplace_grow`, `dealloc_page`) | the `FlexSource` trait | yes |
| glibc `malloc` | the C baseline | `sbrk` extends the main arena's top chunk; `mmap` when `brk` fails | none in-tree | `M_TRIM_THRESHOLD` trims the top |
| musl `malloc` | the C baseline | `__expand_heap`: `brk` first, `mmap` fallback | none | yes |
| seL4 (`sel4runtime`, the Rust `sel4` crates) | the capability baseline | retype an untyped into frames, then map them; a static arena in the runtime | none | at revocation |

Where this crate differs, the memory comes from the process's own untyped budget. It arrives through one kernel method, one page per call. The program sets a cap below that budget, so a leak exhausts the heap visibly before it eats the pages that children need. Growth is geometric with a floor and exact at the cap. `shortfall` sizes a grow so a cap of a power of two still holds an allocation of that size less talc's overhead.

The policy is 290 lines around talc's core and has no seam of its own. The page source is a direct syscall. That is why it has no host test and why std and the host tool each carry a copy.

What it costs: nothing ever returns to the budget. Talc's `truncate` goes unused, and the capability interface offers no per-page return. The spinlock is unfair, and every failure reads as "budget spent".

The concrete example of the gap: `dlmalloc`'s `Allocator` trait lets Rust std test the same allocator on a host and ship it on wasm. This crate's `grow` has a test only under QEMU, through a program that reports one integer.
