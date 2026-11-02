# counter_frequency_contract

The page that tells a process how many counter ticks make a second, on the architectures that have
no register that says so (aarch64 reads `CNTFRQ_EL0` and maps no page). `no_std`, no dependencies.
It was written for the nife kernel and its user-mode runtime, which share this one definition so
the writer and the reader cannot drift on the page's layout or its fixed virtual address.

The kernel learns the rate once at boot, writes the page once, and maps it read-only into every
process, so no seqlock is needed. A child space's page is written by its parent. An unknown rate is
a page of zeroes, which reads as `None`; a page never carries a fabricated default.

## The contract

- The page is `PAGE_CONTENT_BYTES` (16) bytes: `MAGIC` (the ASCII text `TIMEBAS1`) and then the rate in
  hertz as one little-endian `u64`. A frame without the magic reads as unknown. `VERSION` (1) is the
  digit the magic carries; a layout break bumps both in one change.
- `build_page(hz)` makes the bytes. `CounterFrequencyPage::new(va)` is `unsafe` and names a mapped page;
  `hz()` returns `Some(rate)` or `None`.
- The fixed address is `PAGE_VIRTUAL_ADDRESS`: `0x0000_7000_0000_0000` on `x86_64`, and `0x0000_0038_0000_0000`
  on `riscv64`, where the low half of the address space ends at `0x40_0000_0000`. Each sits at
  seven-eighths of its low half, far above every other address a program is handed.
- A rate is believable only between `MINIMUM_PLAUSIBLE_HZ` (100 kHz, above the 32.768 kHz watch
  crystal and far below the slowest real counter) and `MAXIMUM_PLAUSIBLE_HZ` (100 GHz). The band
  exists to catch nonsense, not to police hardware, and `is_plausible(hz)` is its one definition, so
  each architecture's boot path calls the same check.
- Not handled: the TSC is assumed invariant, so a part that changes its rate leaves every process
  reading a stale number. A process spawned without the page faults when it reads it, on purpose.
  The rate is read once per process.

## Usage

```rust
use counter_frequency_contract::{CounterFrequencyPage, build_page, is_plausible};

assert!(is_plausible(62_500_000));
assert!(!is_plausible(1)); // firmware wrote nonsense, and `> 0` would have believed it

let bytes = build_page(1_000_000_000);
// SAFETY: `bytes` is a live buffer of `PAGE_CONTENT_BYTES` for the whole of this block.
let page = unsafe { CounterFrequencyPage::new(bytes.as_ptr() as u64) };
assert_eq!(page.hz(), Some(1_000_000_000));
```

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The minimum supported Rust version is 1.88. There are no Kani harnesses. Whether the page address
lies above every address a consumer places programs at is the consumer's check, because this crate
knows nothing of any address-space layout.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
