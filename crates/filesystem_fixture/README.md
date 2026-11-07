# filesystem_fixture

Test data for a filesystem stack, in one place so the programs that touch it agree: the names and
contents of the files a test image ships, the verdict words of the witnesses that probe the stack
from inside a guest, and the shapes of the walk and throughput benchmarks. `no_std`. Its one
dependency is `filesystem_contract`, whose protocols it is written against. It was written for the
nife filesystem stack's QEMU tests, and the host build that writes the image, the in-guest client
that reads and writes it and the test that asserts the client's report all read the same constants.

It is not a wire contract and nothing that boots depends on it. The protocol words (`READY`,
`DESCENT_REFUSED`, the `rm` status convention) are in `filesystem_contract`.

## The contract

- Image contents: `MOTD_NAME` and `MOTD`, `SCRATCH_NAME` and `SCRATCH_INIT`, `WRITE_PATTERN`, and
  the `tree`, `walk`, `blank` and `crash` modules, which name the directories and files a test
  image carries and the disk each test starts from.
- Verdict words and bitmaps: `SUCCESS`, `VERDICT` and the witnesses `escape`, `dirscape`, `twodir`,
  `twotrees`, `navscape`, `globscape`, `attrs`, `durability` and the shared-frame report words. A
  witness reports a bitmap and the test asserts an exact set, so a witness that reached nothing and
  one that reached everything both fail. Each bit is a single bit and no two share one.
- Benchmarks: `throughput` fixes the transfer unit (`filesystem_contract::fs::TRANSFER_MAXIMUM`),
  the bytes per phase and the phase tags, and `walk` fixes the tree a walk is priced over.
- Names ride in grants, so every file and directory name here fits `filesystem_contract::grant::fits`;
  the crate's tests check that against the contract, and check the witnesses' bits for overlap.

## EXAMPLES

```rust
use filesystem_contract::grant;
use filesystem_fixture::{MOTD, MOTD_NAME, escape};

// The host build writes this file into the image, and the test expects these bytes back.
assert!(grant::fits(MOTD_NAME.as_bytes()));
assert!(MOTD.len() > 8);
// Each witness bit is a single bit.
assert_eq!(escape::SECOND_FILE.count_ones(), 1);
```

## What is proven

Nothing is machine-checked: there are no Kani harnesses. The crate's tests check every name against
`filesystem_contract::grant::fits` and the witnesses' bits for overlap. The tests that check the
staged glob names against a glob matcher live with the matcher.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

## BUGS

- While `filesystem_contract` is a path dependency this crate builds only beside it. The two leave
  together.
- Much of the crate describes the QEMU test runs of the stack it was written for (the witnesses, the
  benchmark phases), not a filesystem in general. A third party gets the file names and the shapes as examples.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
