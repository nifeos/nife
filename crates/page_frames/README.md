# page_frames

[![check](https://github.com/nifeos/page_frames/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/page_frames/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/page_frames/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/page_frames/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/page_frames#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.87%2B-blue.svg)](https://github.com/nifeos/page_frames/blob/main/Cargo.toml)

A physical page frame allocator over a borrowed bitmap, where every frame stays used until it is
declared RAM.

It hands out 4 KiB frames of physical memory, one bit per frame, from a bitmap the caller owns. It
is the bottom of a kernel's memory hierarchy: page tables, the kernel heap, direct memory access
(DMA) buffers and user pages all ask it, and nothing sits underneath it to ask. The crate is
`no_std`, has no `unsafe` and no dependencies, and is dual licensed `MIT OR Apache-2.0`. Every
property it claims is a Kani harness in the tree, and every harness carries a falsification patch
that shows what turns it red.

## EXAMPLES

The wiring an early boot does. Name the span, hand it a bitmap, declare what is real RAM, then
reserve what inside it is already spoken for:

```rust
use page_frames::{FRAME_BYTES, PageFrameAllocator};

const BASE: u64 = 0x4000_0000;
const TOTAL: usize = 64; // 64 frames of address space
let mut bitmap = [0u8; PageFrameAllocator::bitmap_bytes(TOTAL)];
let mut allocator = PageFrameAllocator::new(BASE, TOTAL, &mut bitmap);
assert_eq!(allocator.stats().free_frames(), 0); // nothing is free until someone says so

allocator.mark_free(BASE, TOTAL as u64 * FRAME_BYTES); // all of it is real RAM
allocator.mark_used(BASE, 4 * FRAME_BYTES); // the first four frames hold the kernel image

let frame = allocator.alloc().expect("60 frames are free");
assert!(frame.physical_address() >= BASE + 4 * FRAME_BYTES);
allocator.free(frame);

let run = allocator.alloc_contiguous(8).expect("a run of 8 is free");
allocator.free_contiguous(run, 8);
assert_eq!(allocator.stats().free_frames(), TOTAL - 4);
```

## Design

A bitmap, not a free list. A free list links each free frame to the next through the frame itself,
which costs no metadata. It cannot answer a request for eight physically adjacent frames, and a
device doing DMA needs exactly that, because it reads physical addresses with no memory management
unit (MMU) to hide a scattered buffer. A bitmap also keeps its state outside the memory it manages,
so its logic tests on the host with no memory at all. The cost is one bit per frame: 32 KiB of
bitmap per GiB of RAM.

Everything starts used. `new` marks every frame used, and a frame becomes allocatable only after a
`mark_free` names its range as RAM. A bitmap that started free would hand out a memory-mapped
device's registers as scratch memory. Starting used makes the worst case wasted memory, never a
write to a device.

Reserved memory rounds out and usable memory rounds in. `mark_used` claims every frame its range
touches. A kernel image that ends at `0x4009_7010` still owns the frame at `0x4009_7000`, which
holds its last 16 bytes. Rounding that end down would free the frame, and the next allocation would
overwrite the tail of the kernel. `mark_free` frees only the frames wholly inside its range, because
a frame at an unaligned end also holds bytes nobody said were RAM. Either rule wastes at most one
frame at each end of a region. Getting either one wrong corrupts whatever owns the other bytes.

The bitmap spans from the lowest frame to the highest, holes included. A hole simply never gets
freed, and a frame's index stays one subtraction and one division.

## What is proven

Nine Kani harnesses, each over an 8-frame bitmap with arbitrary contents unless it says otherwise:

- `containing` rounds every address down to an aligned frame that holds it.
- `bitmap_bytes` gives enough bytes for every frame count up to 2^40.
- A frame's address and its bitmap index are inverses, for every aligned base and totals up to 2^20.
- Two back-to-back allocations never return the same frame.
- Every frame `alloc` returns lies on a frame boundary inside the allocator's span.
- `mark_free` frees only frames wholly inside its range, for every aligned base, start and length.
- `mark_used` claims exactly the frames its range touches, for every aligned base, start and length.
- After any one of `mark_free`, `mark_used`, `alloc`, `alloc_contiguous` or `free`, the used count
  equals the number of set bits.
- A successful `alloc_contiguous` returns a run that was all free and is now all used.

No harness covers `free_contiguous` yet; the host tests pin its refusals and its count.

## Building and testing

```sh
cargo build
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none
cargo build --lib --target aarch64-unknown-none-softfloat
cargo build --lib --target riscv64imac-unknown-none-elf
```

The tests run on the host and need no emulator.

### Running the proofs

Install Kani first: `cargo install --locked kani-verifier --version 0.68.0`, then `cargo kani
setup`. The setup step downloads a model checker, so it needs network access.

`cargo kani` runs all nine harnesses. On a shared build machine it took about 75 seconds, the first
build included, with a peak of about 500 megabytes. One harness runs alone with its full path:

```sh
cargo kani --harness verification::two_allocations_are_distinct --exact
```

A passing run prints warnings that mean nothing. They are `use of an unstable feature`, `Found the
following unsupported constructs`, `aborting path on assume(false)`, checks described as
unreachable code, and future-compatibility warnings from Kani's toolchain. The last line is the answer: `Complete - 9
successfully verified harnesses, 0 failures, 9 total.`

### Running one falsification patch

The patch below makes `alloc` mark the frame it hands out as free, so the next allocation can
return the same frame:

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.two_allocations_are_distinct.patch
cargo kani --harness verification::two_allocations_are_distinct --exact   # ends VERIFICATION:- FAILED
git checkout -- src
```

The failing run reports `Status: FAILURE` on the harness's `assert_ne!`, then
`VERIFICATION:- FAILED`. Kani prints the check's message as a placeholder, because it cannot format a message at
run time.

## BUGS

- `alloc_contiguous` takes no alignment beyond one frame. A device that needs a run aligned to a
  larger boundary cannot ask for one.
- The crate needs a 64-bit `usize`. The code casts a frame count and a bitmap index between `u64`
  and `usize`, and a `const` assertion refuses to build on a narrower target.
- `alloc_contiguous` leaves the scan hint where it was, so the next `alloc` scans over the run's
  frames before it finds a free one. The answer stays correct; only the scan is longer.
- Both scans that look for a run, `alloc_contiguous` and `largest_free_run`, walk every frame.

## Minimum Rust version

Rust 1.87, the first stable release with `u64::is_multiple_of` in a `const fn`. Check it with
`rustup toolchain install 1.87 --profile minimal`, then `cargo +1.87 test`, and the bare-metal
builds with the same `+1.87`.

## License

Licensed under either of the MIT license (`LICENSE-MIT`) or the Apache License, Version 2.0
(`LICENSE-APACHE`), at your option.
