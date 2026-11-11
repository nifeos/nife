# entropy_contract

[![check](https://github.com/nifeos/entropy_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/entropy_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/entropy_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/entropy_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/entropy_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/entropy_contract/blob/main/Cargo.toml)

The words a client and an entropy service exchange to move random bytes, and the rules for reading
what comes back. `no_std`, no dependencies, no system calls of its own: the caller supplies the
`CALL`. The service, its clients and a standard library's platform layer share this one
definition, so they cannot drift.

The service holds the random-number device. A client holds an endpoint that names no device, sends
a request word naming a byte count, and gets the bytes back in the reply words. The service
transforms no byte on the way, so the claim stays "these are the device's bytes".

## The contract

A `CALL` carries one request word and returns two reply words.

- Request: the opcode in bits 63:56 (`OPERATION_SHIFT` is 56), the byte count in bits 7:0. There is
  one operation, `GET` (1). Build a request with `request`, read it with `operation` and `requested_bytes`.
- Reply word 1: the count of valid bytes in word 2, from 0 to `MAXIMUM_BYTES` (8). A count of 0 is
  `NO_ENTROPY`: the service has none to give, and it never pads, repeats an earlier answer or
  substitutes a pseudo-random stand-in. A caller that receives no randomness must find out.
- Reply word 2: those bytes, little-endian, in the low bytes. A caller must not read the rest
  of the word, which carries no entropy.
- A request for more than `MAXIMUM_BYTES` gets `MAXIMUM_BYTES`, not a refusal: `request` clamps the
  count it encodes and `requested_bytes` clamps the count it reads, so the caller loops.
- A reply word 1 above `MAXIMUM_BYTES` did not come from the service. The kernel reports a failed
  `CALL` as a small negative number (-1 to -8), which is an enormous unsigned value, so `delivered_bytes`
  returns `None` for it and a caller with no entropy capability finds out without a probe.
- Readiness: a service sends `READY` on its readiness endpoint only after it holds bytes that are not
  all zero (`readiness` decides). Otherwise it sends `BRING_UP_FAILED | step` (`bring_up_failure`), where the
  step is `STEP_NO_FIRST_BYTES` (0x10, the device never answered) or `STEP_FIRST_ALL_ZERO` (0x11,
  every byte was zero). Steps 0x01 to 0x0f belong to each backend. Treating an all-zero first draw
  as a dead device is wrong with probability 2^-64 to 2^-2048 by source, the safe way round.
- `VERSION` (1) is the protocol's version: one number for the operation set and the reply shape. A
  break bumps it; an addition (a new opcode, an optional field) does not.

## EXAMPLES

```rust
use entropy_contract::{GET, MAXIMUM_BYTES, delivered_bytes, request, copy_delivered_bytes, requested_bytes};

// Stands in for the service: it answers with a count and a word of bytes.
fn service(request_word: u64) -> (u64, u64) {
    (requested_bytes(request_word), 0x0807_0605_0403_0201)
}

let mut key = [0u8; 20];
let mut filled = 0;
while filled < key.len() {
    let (first_reply_word, reply_word) = service(request(GET, (key.len() - filled) as u64));
    let byte_count = delivered_bytes(first_reply_word).expect("the service answered");
    assert_ne!(byte_count, 0, "a zero count is NO_ENTROPY: fail, do not retry");
    filled += copy_delivered_bytes(byte_count, reply_word, &mut key[filled..]);
}
assert_eq!(MAXIMUM_BYTES, 8);
```

`fill_with(out, call)` does that loop for you, given a closure that performs the `CALL`. It returns
`None` if any reply is not a count or delivers fewer bytes than asked.

## What is proven

Four Kani harnesses in `src/lib.rs` prove these properties, each within the stated bounds:

- A request word reads back as built: for any opcode that fits the top byte and any byte count,
  `operation` returns the opcode. `requested_bytes` returns the count, clamped to
  `MAXIMUM_BYTES`.
- A copy writes only the delivered bytes: for any count, any reply word and any slice of up to 12
  bytes, `copy_delivered_bytes` writes the least of the count, `MAXIMUM_BYTES` and the slice length.
  It writes them in little-endian order and leaves every other byte alone.
- A fill succeeds exactly when every reply is full: for any slice of up to 17 bytes and any
  replies, `fill_with` asks for the remainder clamped to `MAXIMUM_BYTES` each time. It answers
  `Some` exactly when every reply delivers what it was asked for, and then holds those bytes in
  order.
- Readiness is `READY` exactly when a first byte is nonzero: for any first draw of up to 4 bytes,
  `readiness` answers `READY` when one byte is nonzero. Otherwise it names `STEP_NO_FIRST_BYTES`
  for an empty draw or `STEP_FIRST_ALL_ZERO` for a draw of zeros.

Every harness carries a falsification patch in `falsifications/`, a small defect that turns the
harness red. Unit tests and doctests cover the rest, among them that no byte count collides with a
kernel error.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

## BUGS

- `readiness` reads an all-zero first draw as a dead device: wrong with probability 2^-64 to
  2^-2048 by source, and the safe way round.
- Continuous health testing of the source is not part of this contract; the check is bring-up only.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
