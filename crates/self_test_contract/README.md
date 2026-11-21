# self_test_contract

What the kernel, the progenitor and `boot_self_test` agree on so the boot self-test runs in two
halves. `no_std`; one dependency, `boot_ladder`, for the tally and the check names.

## The contract

- `kernel_half`: the page the kernel fills and the progenitor maps read-only into the program. It
  opens with `MAGIC` (the ASCII text `NIFESTK1` as a little-endian `u64`), then the kernel's tally
  in three words, the `flag` bits, and the facts listed in `word`. `KernelHalf::to_words` writes
  it and `KernelHalf::from_words` reads it, returning `None` for a page without the magic.
- `self_test_program`: the program's capability slots (`slot`), where the kernel-half page, the
  transcript and the image table are mapped, the image table's layout (`image`, `header`,
  `image_virtual_address`, `images_fit`), the entropy sources (`entropy_source`) and the two
  messages the program sends on its report endpoint (`message`).

## EXAMPLES

```rust
use boot_ladder::verdict::{Outcome, Tally, check};
use self_test_contract::kernel_half::{KernelHalf, flag, word};

let mut tally = Tally::new();
tally.record(check("timer"), Outcome::Passed);
let half = KernelHalf {
    tally,
    flags: flag::INJECT,
    ..KernelHalf::default()
};
let back = KernelHalf::from_words(&half.to_words()).unwrap();
assert_eq!(back.tally, tally);
assert!(back.has(flag::INJECT));
assert_eq!(KernelHalf::from_words(&[0; word::COUNT]), None);
```

## What is proven

No Kani harness covers the crate. Host tests check that each flag is its own bit, that a
`KernelHalf` round-trips through its words and a page without the magic reads as `None`, and that
the images follow the header page by page.

## Building and testing

`cargo test -p self_test_contract` runs the host tests and the example.

## BUGS

- The page carries no version beside its magic: a layout change re-mints `MAGIC`.

## Minimum Rust version

Rust 1.88.

## License

MIT OR Apache-2.0.
