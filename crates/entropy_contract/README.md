# entropy_contract

[![check](https://github.com/nifeos/entropy_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/entropy_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/entropy_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/entropy_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/entropy_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/entropy_contract/blob/main/Cargo.toml)

The words a client and an entropy service exchange to move random bytes, and the rules for reading
what comes back. `no_std`, no dependencies, no system calls of its own: the caller supplies the
`CALL`. A `CALL` is the kernel's synchronous message call, one word in and two words back, sent to
an endpoint: a capability that names the service. The service, its clients and a standard
library's platform layer share this one definition, so they cannot drift.

The service holds the random-number device. A client holds an endpoint that names no device, sends
a request word naming a byte count, and gets the bytes back in the reply words. The service
transforms no byte on the way, so the claim stays "these are the device's bytes".

## The contract

A `CALL` carries one request word and returns two reply words.

- Request: the opcode in bits 63:56 (`OPERATION_SHIFT` is 56), the byte count in bits 7:0. There is
  one operation, `GET` (1). Build a request with `request`, read it with `operation` and `requested_bytes`.
  A service answers any other opcode with `NO_ENTROPY`. Bits 55:8 are reserved: a client sends zero
  there, and a service answers a request with any of them set with `NO_ENTROPY`
  (`request_is_well_formed` says whether a word is well formed).
- Reply word 1: the count of valid bytes in word 2, from 0 to `MAXIMUM_BYTES` (8). A count of 0 is
  `NO_ENTROPY`: the service has none to give, and it never pads, repeats an earlier answer or
  substitutes a pseudo-random stand-in. A caller that receives no randomness must find out.
- Reply word 2: those bytes, little-endian, in the low bytes, and zeros above them. A caller must not read
  the rest of the word, which carries no entropy.
- A request for more than `MAXIMUM_BYTES` gets `MAXIMUM_BYTES`, not a refusal: `request` clamps the
  count it encodes and `requested_bytes` clamps the count it reads, so the caller loops.
- A reply word 1 above `MAXIMUM_BYTES` did not come from the service. The contract asks the
  caller's kernel to report a failed `CALL` as a word above `MAXIMUM_BYTES`. A kernel that returns a
  small negative error number meets that, since each reads as an enormous unsigned value. Then
  `delivered_bytes` returns `None` for it, and a caller with no entropy capability finds out
  without a probe.
- Readiness: a service reports its bring-up on a second endpoint, its readiness endpoint, apart
  from the one clients call. It sends `READY` there only after it holds a first draw that is not
  all zero and, for a draw of at least `MAXIMUM_BYTES` bytes, not one value repeated (`readiness`
  decides). Otherwise it sends
  `BRING_UP_FAILED | step` (`bring_up_failure`), where the step is `STEP_NO_FIRST_BYTES` (0x10, the
  device never answered), `STEP_FIRST_ALL_ZERO` (0x11, every byte was zero) or
  `STEP_FIRST_ALL_SAME` (0x12, at least `MAXIMUM_BYTES` bytes, every one equal and nonzero). Steps
  0x01 to 0x0f belong to each backend, the driver for one kind of random-number device. Treating an
  all-zero first draw as a dead device is wrong with probability 2^-64 for an 8-byte draw down to
  2^-2048 for a 256-byte one. For eight equal bytes it is 255 in 2^64 or less. Both errors are the
  safe way round.
- `INTERFACE_ID` (12) is this interface's number in a program's `INTERFACES` note, the section of
  a program's binary that lists the interfaces it speaks. No other interface takes it, and a format
  change is a new version, never a new number. The project that links these crates keeps the table
  of numbers and holds it to that rule.
- `VERSION` (1) is the protocol's version: one number for the operation set and the reply shape. A
  break bumps it. A new opcode does not. A new field in the request word must be a new opcode or a
  version bump, because a service refuses any request with a reserved bit set. A program declares
  the pair of `INTERFACE_ID` and `VERSION` in its `INTERFACES` note.
- A service builds its reply with `reply`: the count, then the delivered bytes little-endian in the
  low bytes of the word, with zeros above them.

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

`fill_with(out, call)` runs a stricter form of that loop, given a closure that performs the
`CALL`: it refuses any reply whose count differs from the one it asked for. It returns
`Err(Refusal::NoService)` when a reply is not a count, and `Err(Refusal::ShortReply { asked,
delivered })` when a reply carries a different count than the request asked for, a larger one
included. A dry device causes the second, so a caller can tell a missing grant from an empty
source. `Refusal` is `#[non_exhaustive]`, so a `match` on it needs a wildcard arm.

## What is proven

Six Kani harnesses in `src/lib.rs` prove these properties, each within the stated bounds:

- A request word reads back as built: for any opcode that fits the top byte and any byte count,
  `operation` returns the opcode. `requested_bytes` returns the count, clamped to
  `MAXIMUM_BYTES`.
- A copy writes only the delivered bytes: for any count, any reply word and any slice of up to 12
  bytes, `copy_delivered_bytes` writes the least of the count, `MAXIMUM_BYTES` and the slice length.
  It writes them in little-endian order and leaves every other byte alone.
- A fill succeeds exactly when every reply is full: for any slice of up to 17 bytes and any
  replies, `fill_with` asks for the remainder clamped to `MAXIMUM_BYTES` each time. It answers `Ok`
  exactly when every reply delivers the bytes it asked for, and then holds those bytes in order.
  Otherwise it refuses with `NoService` exactly when the reply was not a count, and with
  `ShortReply` naming the asked and delivered counts exactly when it was.
- A reply reads back as built: for any bytes up to 12, `reply` answers the count of the least of
  their number and `MAXIMUM_BYTES`. `delivered_bytes` accepts that count, `copy_delivered_bytes`
  returns the bytes in order, and every byte of the word above them is zero.
- Readiness refuses a repeated first draw: for any first draw of up to 9 bytes, `readiness` names
  `STEP_FIRST_ALL_SAME` when the draw holds at least `MAXIMUM_BYTES` bytes of one nonzero value.
  It answers `READY` for every draw that is neither empty, zero nor such a repeat.
- Readiness is `READY` exactly when a byte of the first draw is nonzero: for any first draw of up
  to 4 bytes, `readiness` answers `READY` when at least one byte is nonzero. Otherwise it names `STEP_NO_FIRST_BYTES`
  for an empty draw or `STEP_FIRST_ALL_ZERO` for a draw of zeros.

Every harness carries a falsification patch in `falsifications/`, a small defect that turns the
harness red. Unit tests and doctests cover the rest, among them that no byte count collides with a
kernel error.

## Building and testing

```sh
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none
cargo build --lib --target aarch64-unknown-none-softfloat
cargo build --lib --target riscv64imac-unknown-none-elf
```

The crate has no features and no dependencies, so a plain `cargo test` runs every unit test and
doctest.

### Running the proofs

Install Kani first: run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. Setup downloads a model checker and the
Rust toolchain Kani is pinned to, so it needs network access. Then run every harness:

```sh
cargo kani
```

The six harnesses take one to two minutes and peak near 2.2 gigabytes of memory, as
`/usr/bin/time -v cargo kani` measured on a shared 80-core x86_64 host. Almost all of the time is
`a_fill_succeeds_exactly_when_every_reply_is_full`; each other harness takes seconds. To run one harness, give its full path, because
`--exact` refuses a bare name:

```sh
cargo kani --harness verification::a_reply_reads_back_as_built --exact
```

A passing run prints lines that look alarming and mean nothing. They are the `use of an unstable
feature` warning for `register_tool`, the `Found the following unsupported constructs` warning, and
many `aborting path on assume(false)` lines, which are paths Kani discards on purpose. The answer
is the last line, `Complete - 6 successfully verified harnesses, 0 failures, 6 total.`, and
`cargo kani` exits 0.

### Running one falsification patch

Each file in `falsifications/` applies a small defect that turns its harness red. Apply one,
confirm the proof fails, then undo it with the same patch reversed:

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.readiness_is_reported_exactly_when_a_first_byte_is_nonzero.patch
cargo kani --harness verification::readiness_is_reported_exactly_when_a_first_byte_is_nonzero --exact
patch -R -p1 --no-backup-if-mismatch < falsifications/verification.readiness_is_reported_exactly_when_a_first_byte_is_nonzero.patch
```

The failing run prints `Failed Checks: assertion failed: word == READY` and
`VERIFICATION:- FAILED`. It ends with `Complete - 0 successfully verified harnesses, 1 failures, 1
total.`, and `cargo kani` exits 1. Each patch's opening lines name the check it expects to fail.

## BUGS

- A client cannot tell an unknown operation from a dry source: both are answered `NO_ENTROPY`, and
  the reply has no room for a third answer.
- `readiness` reads an all-zero first draw as a dead device: wrong with probability 2^-64 to
  2^-2048 by source, and the safe way round. It reads a first draw of eight or more equal bytes the
  same way: wrong with probability at most 255 in 2^64 for eight bytes from a uniform source. It
  does not judge a shorter draw for repeats.
- Continuous health testing of the source is not part of this contract; the checks are bring-up
  only.
- No request carries `VERSION`, and the service answers no query for the versions it serves, so a
  client built against another version is not refused at bind.

## Minimum Rust version

Rust 1.88. To check it, install the toolchain and its bare-metal targets, then run the tests and
all three bare-metal builds with `+1.88`:

```sh
rustup toolchain install 1.88 --profile minimal
rustup target add --toolchain 1.88 x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo +1.88 test
cargo +1.88 build --lib --target x86_64-unknown-none
cargo +1.88 build --lib --target aarch64-unknown-none-softfloat
cargo +1.88 build --lib --target riscv64imac-unknown-none-elf
```

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
