# machine_statistics_contract

[![check](https://github.com/partsfoundry/machine_statistics_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/partsfoundry/machine_statistics_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/partsfoundry/machine_statistics_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/partsfoundry/machine_statistics_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/partsfoundry/machine_statistics_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/partsfoundry/machine_statistics_contract/blob/main/Cargo.toml)

The read-only page in which a kernel keeps its machine-wide counters: physical frames, and for each
cpu the ticks spent busy and idle, the context switches, the interrupts taken and the run queue.
`no_std`. The kernel is the one writer. It and every reader link this crate, so they agree on where
each word lives and cannot drift on the layout or the page's fixed virtual address.

The page is never ambient: a program holds it only by grant. A parent maps it read-only at
`PAGE_VIRTUAL_ADDRESS` in the one child that declares it.

## The contract

- The page is `PAGE_CONTENT_BYTES` (576) bytes: `PAGE_WORDS` (72) words of eight bytes. The first
  line is the header, and one line of `word::LINE_WORDS` (8) words follows for each of
  `PROCESSOR_LINES` (8) cpus.
- The header holds `MAGIC` (the ASCII text `MACHSTA1`) at word 0, the frame size, the tick rate, the
  total and free frame counts, and `VERSION` (1) at word 5. Words 6 and 7 of the header and of every
  cpu line are reserved and read zero.
- Header indices live in `word::header` and cpu-line offsets in `word::line`. A cpu word is
  `word::cpu(id) + word::line::OFFSET`.
- A later writer may assign a reserved word under the same `VERSION` only if a reader that does not
  know it is still right to read zero there. Any other change bumps `VERSION` and re-mints the
  magic's digit in the same change.
- `PROCESSOR_LINES` is part of the page's identity. A compile-time assertion keeps it equal to
  `current_cpu_contract::CPU_ID_BOUND`, so a change to that bound fails this crate's build until
  the version moves too.
- Each word is an `AtomicU64` the writer updates in place, one writer per word, with relaxed stores
  and no sequence counter. A reader loads the words one at a time, so the words are not consistent
  with each other, and `Snapshot` carries that same limit.
- `Snapshot::from_words` answers `None` for a frame without the magic or the version, so an
  unprepared frame never reads as zero free memory. Every total saturates instead of overflowing.
- `Snapshot::read` and `atomic_word` are `unsafe`: the caller promises that the address names a
  mapped, 8-byte-aligned page that stays mapped. A reader on a read-only mapping loads with
  `Ordering::Relaxed` only, because the standard library guarantees nothing else there.

## EXAMPLES

```rust
use machine_statistics_contract::{PAGE_WORDS, Snapshot, VERSION, word};

let mut words = [0u64; PAGE_WORDS];
words[word::header::MAGIC] = u64::from_le_bytes(*b"MACHSTA1");
words[word::header::VERSION] = u64::from(VERSION);
words[word::header::FRAME_BYTES] = 4096;
words[word::header::TICK_HERTZ] = 100;
words[word::header::TOTAL_FRAMES] = 32768;
words[word::header::FREE_FRAMES] = 30000;
words[word::cpu(0) + word::line::ONLINE] = 1;
words[word::cpu(0) + word::line::BUSY_TICKS] = 25;

let snapshot = Snapshot::from_words(&words).expect("a page the kernel prepared");
assert_eq!(snapshot.total_bytes(), 32768 * 4096);
assert_eq!(snapshot.online_cpus(), 1);
assert_eq!(snapshot.busy_ticks(), 25);
```

## What is proven

Three Kani harnesses cover the crate. `the_reader_admits_exactly_the_pages_it_names` proves, over
every page of 72 words, that `Snapshot::from_words` answers `None` exactly when the magic word or
the version word differs, and otherwise copies every field from its word. `every_reading_is_total_over_an_admitted_page`
proves that no accessor panics on any admitted page and that the byte totals saturate.
`a_built_header_reads_back_as_an_empty_page` proves that a header from `build_header` followed by
zeroes reads back as an empty page. Each has a falsification patch in `falsifications/` that turns
it red. No proof covers the caller's promise that the address names a mapped page, because Kani
cannot see a mapping.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The tests read and write a page on the host through an aligned buffer. The end-to-end test, which
drives a kernel's writer and this crate's reader together, lives with the kernel that maps the page.

## BUGS

- The words are not consistent with each other, and no sequence counter lets a reader detect a torn
  read.
- Busy time is not split into user and system time.
- The writer's tick bounds a sample's age, not the page: a counter the writer
  publishes once per tick is up to one tick old, and a parked cpu keeps its last sample.
- `PROCESSOR_LINES` fixes the line count; a machine with more cpus does not fit.
- There is no swap, page-cache or block I/O counter.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
