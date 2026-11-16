# allocation_cache

A bounded stash of freed small blocks in front of a shared heap, so most allocations and frees
never take the heap's lock. It is the per-owner cache every fast allocator has (glibc's `tcache`,
jemalloc's `tcache`, TCMalloc's thread cache), written as a `no_std` library with no dependencies:
the algorithm only. It never locks, maps or reads an owner's identity. The owner decides who
touches a `Cache`, and the heap is whatever the owner allocates from.

A size class is a block size on a 16-byte grid up to 256 bytes. A `Cache` holds a stack of freed
blocks for each class, at most 16 deep. A free pushes onto the stack and an allocation pops from it,
both without a lock. When a class is empty the owner refills it with a batch of eight blocks from the
heap, and when it is full the owner spills eight back, each batch under one acquisition of the
heap's lock. Blocks of one class are fungible, because the heap sees the same layout
(`class_layout`) at both ends.

## What the owner promises

1. One owner at a time. A `Cache` is neither `Send` nor `Sync`.
2. `refill` and `spill` run under whatever excludes the other users of the heap.
3. Every block a `spill` or `give_back_all` hands back goes to the heap under
   `class_layout(class)`, and every block the heap hands to `refill` was allocated under it.

The heap also sees `heap_layout` on the path that never touches a cache, because a block may be
allocated before its owner has a cache and freed into one afterward. A heap that rounds or tags its
own way would otherwise have given a shorter block to the shorter request.

An all-zero `Cache` is empty, so one may live in memory a runtime zeroes. A `Class` is a one-byte
newtype that only `class` makes, so no method takes an index past the last class.

## EXAMPLES

A cache with one block, and the requests that have no class:

```rust
use allocation_cache::{Cache, class};
use core::ptr::NonNull;

let mut cache = Cache::new();
let c = class(48, 8).unwrap(); // a `[u8; 48]`
assert_eq!((class(16, 64), class(257, 8)), (None, None)); // over-aligned, too big
let mut block = [0u128; 3]; // 48 grid-aligned bytes, as a heap would hand out
let p = NonNull::from(&mut block).cast::<u8>();
// SAFETY: `p` is 48 writable, grid-aligned bytes nobody else uses while the cache holds them.
assert!(unsafe { cache.push(c, p) }.is_ok());
assert_eq!((cache.pop(c), cache.pop(c)), (Some(p), None)); // then the caller goes to the heap
```

The cache in front of a heap of your own. This one is `talc`, which tags each block and rounds to a
32-byte chunk, so the heap sees `heap_layout` even for a request that is never cached:

```rust
use allocation_cache::{Cache, class, class_layout, heap_layout};
use core::alloc::Layout;
use talc::base::Talc;
use talc::base::binning::DefaultBinning;
use talc::source::Manual;

let mut arena = vec![0u128; 4096];
let base = arena.as_mut_ptr().cast::<u8>();
let mut heap = Talc::<Manual, DefaultBinning>::new(Manual);
// SAFETY: `arena` is ours and outlives `heap`.
unsafe { heap.claim(base, 4096 * 16).unwrap() };

let mut cache = Cache::new();
let layout = Layout::from_size_align(40, 8).unwrap();
let c = class(layout.size(), layout.align()).unwrap();

// Allocate: the cache first, then a batch from the heap on a miss.
let block = match cache.pop(c) {
    Some(block) => block,
    None => {
        // SAFETY: each block is the heap's, allocated under the class's layout.
        unsafe { cache.refill(c, || heap.try_allocate(class_layout(c))) };
        cache.pop(c).unwrap()
    }
};

// Free: into the cache, and a batch back to the heap when the class is full.
// SAFETY: `block` is the class's size, from the heap, and ours again.
assert!(unsafe { cache.push(c, block) }.is_ok());

// The owner's end: give everything back under the class's layout.
// SAFETY: every cached block came from the heap under `class_layout`.
cache.give_back_all(|c, b| unsafe { heap.deallocate(b.as_ptr(), class_layout(c)) });

// A request with no class goes to the heap as `heap_layout` says, at both ends.
let big = Layout::from_size_align(1000, 8).unwrap();
// SAFETY: `heap_layout` is never empty.
let p = unsafe { heap.try_allocate(heap_layout(big)) }.unwrap();
// SAFETY: from this heap, under the layout it was allocated with.
unsafe { heap.deallocate(p.as_ptr(), heap_layout(big)) };
```

## What is proven

The Kani harnesses prove the size-class arithmetic and the layout the heap sees for every input, and
a bounded model of `push` and `pop` over a fixed arena. The host test walks a cache in front of
`talc` with random allocations, frees, refills and spills, and checks that no block is handed out
twice and that every byte coalesces after the cache is emptied. The harnesses prove the properties
of the code they model; the heap walk tests the cache against a real allocator, which no harness
models.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
cargo kani                                       # the proofs
```

## BUGS

- A cached block cannot coalesce until it is spilled: up to `MAXIMUM_CACHED_BYTES` (34,816) per
  cache. The owner empties the cache with `give_back_all` at its end. A `Cache` dropped while it
  holds blocks leaks them, because it cannot free; a debug build panics in that drop.
- `BLOCKS_PER_CLASS` (glibc's `tcache` count) and `MAXIMUM_CLASS_BYTES` are chosen, not measured per
  workload, and they do not move: they are constants, not parameters of `Cache`.
- A block freed by an owner other than the one that allocated it lands in the freeing owner's cache,
  bounded by `BLOCKS_PER_CLASS` and spilled at the bound.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
