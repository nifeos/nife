# current_cpu_contract

[![check](https://github.com/nifeos/current_cpu_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/current_cpu_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/current_cpu_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/current_cpu_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/current_cpu_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/current_cpu_contract/blob/main/Cargo.toml)

The page that tells a thread which core it is running on, without a system call. `no_std`. A kernel
and the runtime of its processes share this one definition, so the writer and the readers cannot
drift on the page's layout or its fixed virtual address.

The kernel maps the page read-only at `PAGE_VIRTUAL_ADDRESS` in every process, and writes the CPU
word at every switch-in, on the destination core. A reader may be stale one instruction after it
reads, so a per-core cache built on the answer must stay correct when the thread has already moved.

## The contract

- The page is `PAGE_CONTENT_BYTES` (32) bytes: `MAGIC` (the ASCII text `CURRCPU1`), then three
  little-endian `u64` words: the CPU, the allowance (how many cores the thread's process may use)
  and the scheduler tick in hertz. `VERSION` (1) is the digit the magic carries.
- Each word reads as `None` when it is unknown: `None` for a frame without the magic, `None` for a
  thread that never ran (the CPU word holds `UNSCHEDULED`, which is not zero because zero is a valid
  CPU id), and `None` for an allowance or a tick of zero.
- `cpu()` also reads an id at or past `CPU_ID_BOUND` as unknown, so its answer always indexes an
  array of `CPU_ID_BOUND` elements. The bound is part of the page's identity: a change to it re-mints
  the magic and bumps the version in the same change.
- `CurrentCpuPage` is deliberately neither `Send` nor `Sync`: the page is this thread's answer, and
  carrying it to another thread carries a wrong one.
- The layout grows only by appending a word that reads zero where a kernel never wrote it, so a new
  field never changes the magic.
- The crate ships one source file, because a consumer may copy it into a standard library: keep it
  to one file with fully qualified paths.

## EXAMPLES

```rust
use current_cpu_contract::{CPU_ID_BOUND, CurrentCpuPage, build_page, publish};

#[repr(align(8))]
struct Aligned([u8; current_cpu_contract::PAGE_CONTENT_BYTES]);

let mut bytes = Aligned(build_page());
let page_virtual_address = bytes.0.as_mut_ptr() as u64;
// SAFETY: `bytes` is a live, 8-aligned buffer of `PAGE_CONTENT_BYTES`, and this is its only writer.
unsafe { publish(page_virtual_address, 3) };
// SAFETY: as above.
let page = unsafe { CurrentCpuPage::new(page_virtual_address) };

let mut per_core = [0u64; CPU_ID_BOUND]; // size by the bound, never by the cores seen so far
if let Some(cpu) = page.cpu() {
    per_core[cpu] += 1;
}
assert_eq!(per_core[3], 1);
```

## What is proven

Two Kani harnesses cover the crate. `the_readers_answer_only_what_the_page_holds` proves, over every
32-byte frame, that each reader answers `Some` exactly when the magic matches and its word is a real
answer: the CPU when it is neither `UNSCHEDULED` nor at or past `CPU_ID_BOUND`, the allowance and the
tick when they are nonzero. `a_built_page_reads_back_its_words` proves that a page from
`build_page_with` reads back its allowance and tick for every pair of values, and reads the CPU as
unknown. Each has a falsification patch in `falsifications/` that turns it red. No proof covers the
caller's promise that the address names a mapped page, because Kani cannot see a mapping.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The tests drive both halves of the page without a kernel.

## BUGS

- The reader forms an `&AtomicU64` over a read-only mapping, a shape the Rust memory model has not
  settled; the fallback is a raw atomic load that forms no reference.
- A process has at most `THREAD_PAGE_SLOTS` threads, one page each.
- An address space never bound to a thread has no page, so a reader there faults.
- `CPU_ID_BOUND` sizes per-core arrays and does not enumerate the cores that exist.
- A consumer may copy this crate's single source file into a standard library: keep the crate to one
  file with fully qualified paths.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
