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

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The minimum supported Rust version is 1.88. There are no Kani harnesses. While `filesystem_contract`
is a path dependency this crate builds only beside it. The tests that check the staged glob names
against a glob matcher live with the matcher.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
