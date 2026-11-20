# memory_region_heap

A global allocator for a process that pays for its own memory. The heap lives at one virtual range
the program picks, maps nothing until the first allocation, and then grows one page at a time from
a page source, under a cap. The allocator inside is [`talc`](https://crates.io/crates/talc) 5.1.1.

The default page source is the `MAP` call on a capability microkernel's memory region: the budget
of pages the process holds (the kernel's own word for that capability is "untyped"). The region
pays for every page, so nothing else supplies memory. Any type that implements `PageSource` can
stand in for it, which is how the growth policy runs on a host. The crate is `no_std`.

A grow maps as many pages again as the heap already holds, at least 8 (32 KiB)
and never past the cap, so a burst of small allocations costs a logarithmic number of page-mapping
batches. `commit_all` maps the whole cap at once.

An allocation returns null at the cap, when the region has no page left (the
kernel's `OutOfMemory`), or after the region refused for any other reason. The first two leave the
heap serving what it holds. The third stops growth for good, because a retry would meet the same
refusal and might spend a page each time. `last_refusal` reports which answer the region gave.

The smallest useful cap is one page (4,096 bytes), and `initialize` refuses less. `talc`'s first
heap takes 1,575 bytes of bookkeeping on a 64-bit target, so one page serves about 2,500 bytes of
allocations.

The heap never shrinks. It never calls `talc`'s `truncate`, and the memory-region interface has
no call that returns one page, so a heap holds its peak until the region's owner destroys it.

## EXAMPLES

On the kernel, one static per program, wired first thing in the entry point:

```rust,ignore
#[global_allocator]
static HEAP: memory_region_heap::MemoryRegionHeap = memory_region_heap::MemoryRegionHeap::new();

// With the capability table slot that holds the memory region, the suggested base, and a 4 MiB cap:
HEAP.initialize(SLOT, memory_region_heap::DEFAULT_BASE_VIRTUAL_ADDRESS, 4 << 20).unwrap();
```

On a host, over a page source that hands out memory the test already owns:

```rust
use core::alloc::{GlobalAlloc, Layout};
use memory_region_heap::{MemoryRegionHeap, PageSource, Refusal, WiringRefusal};

/// Pages that already exist, so mapping one has nothing to do.
struct Preallocated;

impl PageSource for Preallocated {
    fn map_page(&mut self, _virtual_address: u64) -> Result<(), Refusal> {
        Ok(())
    }
}

let arena = Layout::from_size_align(16 * 4096, 4096).unwrap();
// SAFETY: the layout has a nonzero size.
let base = unsafe { std::alloc::alloc(arena) };
assert!(!base.is_null());

let heap = MemoryRegionHeap::with_source(Preallocated);
heap.initialize_source(base as u64, 16 * 4096).unwrap();
assert_eq!(heap.initialize_source(base as u64, 16 * 4096), Err(WiringRefusal::AlreadyWired));

let small = Layout::from_size_align(100, 8).unwrap();
// SAFETY: the layout has a nonzero size.
let p = unsafe { heap.alloc(small) };
assert!(!p.is_null());
assert_eq!(heap.committed_bytes(), 8 * 4096); // the first grow maps the minimum step

let too_big = Layout::from_size_align(17 * 4096, 8).unwrap();
// SAFETY: the layout has a nonzero size.
assert!(unsafe { heap.alloc(too_big) }.is_null()); // past the cap: null, not a hang

// SAFETY: `p` came from this heap under `small`.
unsafe { heap.dealloc(p, small) };
drop(heap);
// SAFETY: allocated above with the same layout, and the heap that used it is gone.
unsafe { std::alloc::dealloc(base, arena) };
```

## What is proven

- A grow never commits past the cap and keeps the committed range a whole number of pages, for any
  cap, any request, and a page source that refuses at any point.
- A grow at the cap is a refusal whatever the request, so an allocation there returns null rather
  than retrying forever.
- The bytes a grow asks for never overflow, for any valid layout.

The harnesses prove the growth arithmetic in `src/growth.rs`, which holds no allocator and no lock.
The host tests drive the whole heap, `talc` included, over page sources that refuse in each of the
ways a region can.

## Building and testing

```sh
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

### Running the proofs

Install Kani first: `cargo install --locked kani-verifier --version 0.68.0`, then `cargo kani setup`,
which downloads a model checker and needs network access. `cargo kani` then takes about 10 seconds
and peaks near 280 megabytes of memory. One harness:

```sh
cargo kani --harness verification::a_grow_at_the_maximum_is_a_refusal --exact
```

A passing run also prints warnings about unstable features and unsupported constructs, and checks
described as unreachable. They mean nothing; the last line is the answer:
`Complete - 3 successfully verified harnesses, 0 failures, 3 total.`

### Running one falsification patch

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.a_grow_at_the_maximum_is_a_refusal.patch
cargo kani --harness verification::a_grow_at_the_maximum_is_a_refusal --exact   # ends VERIFICATION:- FAILED
git checkout -- src
```

The patch lets a run of no pages count as a success, so the harness finds a grow at the cap that
would retry.

## BUGS

- The lock is a spinlock with no queue. A thread preempted while it holds the lock keeps every
  other allocating thread spinning until it runs again.
- A `PageSource` must not allocate from the heap it feeds, and no signal-like upcall may allocate
  from the heap, since either would wait on a lock its own thread holds.
- crates.io's `talc` 5.1.1 lacks upstream
  [pull request 57](https://github.com/SFBdragon/talc/pull/57), which fixes two fit checks that
  compared addresses where they meant sizes. It matters only for a free gap within one chunk of the
  top of the address space, which a 64-bit user process never maps. A consuming workspace can build
  a patched copy through `[patch.crates-io]` until a release carries the fix.
- The heap never returns a page. The memory-region interface has no call that unmaps one, so the
  day it gains one, the heap can call `talc`'s `truncate` and hand the top back.
- The growth arithmetic restates `talc`'s chunk rounding. Were `talc` to round otherwise, each short
  grow would lead to another, larger one, until the cap refuses.

## Minimum Rust version

Rust 1.88. To check it: `rustup toolchain install 1.88 --profile minimal`, then `cargo +1.88 test`,
and the bare-metal build with the same `+1.88`.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
