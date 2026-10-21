# The heap

> The crates are gone (2026-07-27): `crates/heap` and `crates/slab` were deleted once
> nothing in the workspace referenced them, superseding this note's earlier "the crates stay"
> stance at calef's direction. The history preserves them; this note remains as the record of
> what was built and why its deletion was earned (milestone 14).

## Why the stack isn't enough

The stack works because function lifetimes nest ([stack.md](stack.md)). If `foo` calls
`bar`, `bar` finishes first. Always. That strict LIFO ordering is what lets the stack be a
single moving pointer, with `sub sp, sp, #32` as its entire allocation algorithm.

Now consider:

```rust
fn read_config() -> Vec<String> { ... }
```

That `Vec`'s buffer must outlive the function that created it. It cannot live on the
stack, because the frame is gone the instant `read_config` returns. Its lifetime doesn't nest
with anything.

The heap is memory you can allocate at any time, in any size, and free at any time, in any
order.

That last clause, *in any order*, is where all the difficulty comes from. The stack gets to
be one pointer because it may assume LIFO. The heap may assume nothing.

## What that costs

| | Stack | Heap |
|---|---|---|
| Allocate | `sub sp, sp, #32`. One instruction. | **Search** a free list. Maybe split a block. |
| Free | `add sp, sp, #32`. One instruction. | Insert back. Merge with neighbors. |
| Fragmentation | impossible | **the permanent enemy** |
| Forgetting to free | impossible | a leak |
| Use-after-free | impossible | a security bug |

`malloc` is roughly a hundred times slower than a stack push. That isn't sloppiness; it's the
price of dropping the LIFO assumption.

Fragmentation is the one that really bites. Allocate and free in a loop and you can end up
with thousands of tiny free blocks, none big enough for a 32-byte request, while the "free
memory" counter reports megabytes. The heap has failed *while claiming to be fine*.

## Rust's whole thesis, restated

Look at that table again. Use-after-free, double-free, and leaks are heap problems. None
of them exist on the stack.

Ownership, `Box`, `Drop`, and lifetimes are the compiler proving you free the heap exactly
once, at the right moment. In a real sense the borrow checker is a heap-correctness checker,
which is why `no_std` feels so strange: take away the heap and half of Rust's reason for
existing goes quiet.

## Two allocators, and why they're different

| | `frames` (milestone 3) | `heap` (milestone 4) |
|---|---|---|
| Hands out | fixed 4 KiB pages | arbitrary sizes, arbitrary alignment |
| Metadata | a **bitmap, outside** the memory | **inside the free blocks themselves** |
| Why | a page might go to a device for DMA, or to userspace. **You cannot store bookkeeping inside memory you are giving away.** | a free block is by definition space nobody is using. Storing the list node *in* it costs zero overhead for allocated memory. |

And they stack:

```
Vec, Box, String, BTreeMap
        |  #[global_allocator]
   kernel heap        arbitrary sizes, free list, coalescing
        |
  frame allocator     fixed 4 KiB pages, bitmap
        |
   physical RAM       read out of the device tree
```

The heap asks the frame allocator for 256 contiguous pages and carves them up. This is the
first real use of `alloc_contiguous`, and it is why `frames` is a bitmap and not a free
list: a free list could not have answered the request. See
[physical-memory.md](physical-memory.md).

## The two things that make ours correct

Everything is 16-byte aligned, in both address and size.

That single invariant is what makes splitting always work. Any gap left over from a split is a
multiple of 16, so it is either exactly zero or big enough to hold a free-block header
(`size_of::<Block>()` is 16). Without it you get slivers too small to track, and you leak them
one at a time until the heap dies.

The free list is sorted by address, and `free` coalesces with both neighbors.

Sorted by address so that "the block before" and "the block after" are the only two to check,
rather than all of them. And merging *forward first, then backward*, so that freeing a block
between two free neighbors collapses all three in one pass.

There is a test (`thrashing_does_not_fragment_the_heap_to_death`) that allocates and frees
2000 times in a churning pattern and then asks for nearly the entire arena. It only passes if
the heap is still one block.

## The alignment gap, which is where naive implementations leak

Ask for 4096-byte alignment inside a block that doesn't start at a 4096 boundary, and you must
step forward to the aligned address. The bytes you stepped over do not disappear.

The obvious implementation aligns forward and forgets. It then leaks a few hundred bytes per
aligned allocation, and the heap slowly dies over hours, in a way no single test catches. Ours
puts the front gap back on the list, and `a_large_alignment_does_not_leak_the_gap_before_it`
is there to keep it that way.

## What it unblocks

Every list in the kernel was a fixed-size array (`MAX_REGIONS = 16`) purely because there
was no heap. `memory.rs` still declares `[Region; 16]` and returns `TooManyRegions` if a
machine has more, a limitation accepted only because `Vec` didn't exist.

Ahead of us, everything is "an unknown number of things, sized only at runtime":

| Milestone | Wants |
|---|---|
| 6 | a thread structure per thread |
| 7 | a process table, and page tables per process |
| 8 | a cache of filesystem inodes |

## The promise from milestone 1, now kept

[no-std.md](no-std.md), written before a single line of kernel existed:

> At milestone 4 we write a `#[global_allocator]`, add `extern crate alloc;`, and `Vec` starts
> working. Not because we imported it. Because we built the heap it needed.

Nothing was imported. Every link in the chain is ours.

*(Unrelated aside, because the collision genuinely confuses people: "the heap" here has nothing
to do with the heap data structure, the binary tree used for priority queues. Same word,
different thing.)*

---

# The slab, and how we chose it

## Measure before optimizing

Both `alloc` (first fit) and `dealloc` (address-sorted insert) walk the free list, so both are
O(n) in the number of free blocks. Before fixing that, we measured how large `n` actually gets
(`crates/heap/tests/fragmentation.rs`):

| Workload | free-list length |
|---|---|
| uniform 64 B, 1000 live | **1** |
| mixed 16-256 B, freed out of order | **3** |
| uniform 64 B, **every other one freed** | **1001** |

So the O(n) is a non-issue for most workloads: coalescing keeps the list at one to three
blocks, and catastrophic for exactly one shape: many isolated, same-sized holes.

And that shape is what a kernel produces. Two thousand threads, half of them exit. A file
descriptor table with gaps. Not hypothetical.

## The trade nobody escapes

You cannot coalesce in O(1) without per-block metadata.

To merge with your physical neighbor you must know whether the block at `p + size` is free, and
where the block before `p` begins. Neither is knowable without a header on every block,
allocated ones included. That is why glibc carries 8-16 bytes of overhead per allocation.

Our heap's virtue is zero overhead on allocated memory. Its price is the O(n).

| Design | alloc | free | overhead/alloc | coalesces |
|---|---|---|---|---|
| `crates/heap`: address-sorted list | O(n) | O(n) | **0** | yes |
| boundary tags + segregated lists (glibc) | O(1) | O(1) | 8-16 B | yes |
| **`crates/slab`** (Linux SLUB) | **O(1)** | **O(1)** | **0** | *doesn't need to* |

Read the third row carefully. A slab holds objects of exactly one size, so a freed object is
immediately reusable by the next request of that size. Coalescing becomes unnecessary rather
than fast, and the pathological case stops existing.

That is why Linux uses SLUB for small allocations and the page allocator for large ones.

## How ours works

Eight size classes: 16, 32, 64, 128, 256, 512, 1024, 2048.

Each class owns a free list of objects of *exactly* that size. Allocation pops the head. Free
pushes onto the head. Both are a couple of pointer writes: no search, no walk, no merge.

When a class runs dry it takes one 4 KiB page from the frame allocator and carves it into
objects. The list node lives inside the free object: the same trick as the heap's free
blocks, and for the same reason: a free object is by definition space nobody is using.

Alignment falls out for free. A page is 4096-aligned, and an object of size `16 << i` sits at
offset `k * (16 << i)` within it, so it is naturally aligned to its own size. The 256-byte class
hands out 256-aligned objects without trying. (Which also sets the limit: a request for 16 bytes
aligned to 4096 fits *no* class, and correctly falls through to the general heap.)

## The split

```
alloc(layout):
    <= 2 KiB, alignment a class can serve  ->  slab   (O(1) both ways)
    everything else                        ->  heap   (coalescing free list)
```

`alloc` and `dealloc` must agree about which one owns a block, and they do, because both ask
the same pure function of the `Layout`. Rust hands us the layout on *both* paths.

C's `free(ptr)` gets no such thing, which is why C allocators need a header on every block just
to remember its size. We get it for nothing, and it is a bigger deal than it looks: it is
half the reason our heap can have zero per-allocation overhead at all.

## What the slab does not do

Slabs are never returned to the frame allocator. Once a page belongs to the 64-byte class it
belongs to it forever, even if every object in it is free. Real SLUB tracks per-slab occupancy
and frees empty ones.

So memory is bounded by the high-water mark of each class, not by current usage. Fine for
now; worth fixing if a class ever balloons.

---

## The heap came back, in userspace (milestone 27)

The kernel stayed heapless (milestone 14 earned that and nothing since has needed to undo it),
but Rust `std` needs a `GlobalAlloc`, and a capability system has an obvious place to get one:
the process's own untyped budget. The pair that landed:

- The algorithm is `talc` 5.1.1's core (milestone 784 (reuse what the tree wrote), item d), carried
  in `vendor/talc` with one upstream fix (vendor/README.md) and generated into std. Boundary tags and size-class bins, so allocation costs the
  same at any number of free blocks; each allocated chunk carries one word above it, on a 32-byte
  chunk grid. `dealloc` finds that word from the `Layout`, so a block must be freed under the
  layout it was allocated under, which `allocation_cache::heap_layout` keeps true in std. The
  measurements that chose it are in
  `design/roadmap/0784-reuse-what-the-tree-wrote/measured-for-the-rulings.md`.

- `memory_region_heap::MemoryRegionHeap`: the policy and the syscall glue. A `#[global_allocator]`
  static the program wires with one call, `HEAP.init(untyped_slot, base_va, max_bytes)`, first
  thing in `_start`. Growth maps pages at the top of `[base, base+committed)` via `untyped::MAP`
  (one page per invoke, zeroed by the kernel, paid from the budget). It grows geometrically (double,
  at least 8 pages, never past `max_bytes`). The first run is talc's `claim`, holding its bins;
  each later run is `extend`, so the heap stays one contiguous range. Which slot pays, and where the heap lives, are part of the
  program's capability contract like every other slot number (notes/abi.md §4);
  `heap::DEFAULT_BASE` (1 GiB) is a suggested VA clear of every convention in the tree.

What this is *not*: a kernel service. The kernel's only involvement is the `untyped::MAP` it
already had; a process that leaks just exhausts its own budget (`OutOfMemory` is the process's
problem, and the `alloc` OOM handler turns it into a fault the kernel reports). Honest caveat: the
lock is a spinlock, fine while processes are single-threaded, wasteful under real contention; std
puts a per-thread cache in front of it (notes/std/allocation.md). Proven by `allocator_exerciser`
(`fixtures/src/allocator_exerciser.rs`), the first program in the tree linking `extern crate alloc`,
spawned by the test suite on both ISAs with a 96-page budget. It runs Vec/String/BTreeMap churn and
frees in arbitrary order, then a 128 KiB allocation that must fit in already-committed pages,
proving freed memory
is reused rather than leaked. One real bug found by the machine on the way: `load` maps a single 4
KiB stack page. BTreeMap plus the `assert!` fmt machinery overflow it (a data abort one word below
the stack), so the spawn maps three extra stack pages.

---

*Add to this file as new allocator concepts come up.*

## The shell gets a heap, and it may never panic (milestone 47)

calef ruled on 2026-09-26 (UTC) that `swish` gets an allocator, on three conditions. Milestone 47
(navigation and naming) built it with them.

Allocation is fallible. The boot shell is the owner's root console, so running out of heap is a
refusal with a sentence (`swish::Say::HeapFull`), never a panic. `MemoryRegionHeap` already
answers a failed grow with a null pointer, so nothing here needed changing. What needed a
mechanism was the caller. `components/src/swish/main.rs` declares `extern crate alloc` inside `mod heap`
and nowhere else, and a `no_std` crate cannot name `alloc` without that declaration. So the rest
of the shell cannot reach `Vec` at all. `mod heap` hands out `Room`, a slice once filled, built
with `try_reserve_exact` and `push_within_capacity`, and it has no method that grows. `script/lint`
holds that the declaration stays in `mod heap`.

The heap is capped at 32 KiB (`swish::HEAP_MAX_BYTES`). All of it is mapped from the shell's
128-page budget first thing at `_start` (`MemoryRegionHeap::commit_all`), so its pages and their
page tables sit under every later carve and cannot break their last-in first-out return. The cap is
several times what a line needs. A leak therefore exhausts the heap and refuses lines long before
it could touch the pages children are built from.

Not a 12-page region split off the budget, because a region that has been split cannot be
destroyed while its child lives. Every kernel test that ran the shell would keep its whole budget,
247 to 283 frames each, and CI showed a later test on each architecture running out of page frames.

No word splitting, ever. §141 (application is grant) said the `$?` guarantee held only because
there was no allocator. It now holds because substitution has one seam, `swish::pieces`, which
splits the typed line first and emits a substituted value as one word.
`a_substituted_value_is_never_split` fails if that order ever changes.

The first use removes the limit that bit four times (notes/pipes/the-boot.md): a pipeline's
planned stages now live on the heap instead of in `pipeline`'s frame. Measured on aarch64 with
`-Z emit-stack-sizes`, debug build, 2026-09-26:

| | Before | After |
|---|---|---|
| `swish::pipeline` frame | 20,432 B | 16,480 B |
| `swish` text, debug | 294,319 B | 313,975 B |
| `swish` text, release | 145,877 B | 148,557 B |

The debug text grows by the allocator and its glue. The release build grows by 2.6 KiB.

### BUGS

- The heap row in `caps` is printed whether or not the split succeeded. A shell whose budget
  could not spare 12 pages would show a heap it does not have. Every wiring grants 128 pages, so
  this is a statement about a boot nobody builds.
- The set-grant carrier is untouched. A name set still travels by value on the stack, and
  the set-grant ruling in milestone 47's block is where that changes.
