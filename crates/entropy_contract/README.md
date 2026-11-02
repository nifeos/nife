# entropy_contract

The words a client and an entropy service exchange to move random bytes, and the rules for reading
what comes back. `no_std`, no dependencies, no system calls of its own: the caller supplies the
`CALL`. It was written for the nife kernel's entropy service, its clients and its standard library,
which share this one definition so they cannot drift.

The service holds the random-number device. A client holds an endpoint that names no device, sends
a request word naming a byte count, and gets the bytes back in the reply words. No byte is
transformed on the way, so the claim stays "these are the device's bytes".

## The contract

A `CALL` carries one request word and returns two reply words.

- Request: the opcode in bits 63:56 (`OPERATION_SHIFT` is 56), the byte count in bits 7:0. There is
  one operation, `GET` (1). Build a request with `req`, read it with `operation` and `want`.
- Reply word 1: the count of valid bytes in word 2, from 0 to `MAXIMUM_BYTES` (8). A count of 0 is
  `NO_ENTROPY`: the service has none to give, and it never pads, repeats an earlier answer or
  substitutes a pseudo-random stand-in. A caller that cannot be given randomness must find out.
- Reply word 2: those bytes, little-endian, in the low bytes. The rest of the word carries no
  entropy and must not be read.
- Asking for more than `MAXIMUM_BYTES` is answered with `MAXIMUM_BYTES`, not refused (`want` clamps),
  so the caller loops.
- A reply word 1 above `MAXIMUM_BYTES` did not come from the service. The kernel reports a failed
  `CALL` as a small negative number (-1 to -8), which is an enormous unsigned value, so `delivered`
  returns `None` for it and a caller with no entropy capability finds out without a probe.
- Readiness: a service sends `READY` on its readiness endpoint only after it holds bytes that are not
  all zero (`readiness` decides). Otherwise it sends `DEAD | step` (`bringup_failure`), where the
  step is `STEP_NO_FIRST_BYTES` (0x10, the device never answered) or `STEP_FIRST_ALL_ZERO` (0x11,
  every byte was zero). Steps 0x01 to 0x0f belong to each backend. Treating an all-zero first draw
  as a dead device is wrong with probability 2^-64 to 2^-2048 by source, the safe way round.
- `VERSION` (1) is the protocol's version: one number for the operation set and the reply shape. A
  break bumps it; an addition (a new opcode, an optional field) does not.

Continuous health testing of the source is not part of this contract; the check is bring-up only.

## Usage

```rust
use entropy_contract::{GET, MAXIMUM_BYTES, delivered, req, take, want};

// Stands in for the service: it answers with a count and a word of bytes.
fn service(w0: u64) -> (u64, u64) {
    (want(w0), 0x0807_0605_0403_0201)
}

let mut key = [0u8; 20];
let mut filled = 0;
while filled < key.len() {
    let (r0, r1) = service(req(GET, (key.len() - filled) as u64));
    let n = delivered(r0).expect("the service answered");
    assert_ne!(n, 0, "a zero count is NO_ENTROPY: fail, do not retry");
    filled += take(n, r1, &mut key[filled..]);
}
assert_eq!(MAXIMUM_BYTES, 8);
```

`fill_with(out, call)` does that loop for you, given a closure that performs the `CALL`. It returns
`None` if any reply is not a count or delivers fewer bytes than asked.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The minimum supported Rust version is 1.88. There are no Kani harnesses; the properties above are
covered by unit tests and doctests, among them that no byte count collides with a kernel error.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
