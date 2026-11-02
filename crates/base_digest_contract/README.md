# base_digest_contract

The page that carries the SHA-256 digest of a base set's measurement table from a kernel to its
first process. `no_std`, no dependencies, no `unsafe`. The kernel holds the digest from its loader,
writes the page once before the first process exists, and hands it over as a read-only capability.
The first process maps the page where it chooses, reads the digest, and checks the table against it
before it reads a line of the table.

## The contract

- The page content is `PAGE_CONTENT_BYTES` (40) bytes: `MAGIC` (the ASCII text `BASEDIG1`) and then
  the 32 bytes of the digest. The rest of the frame is zeroes. `VERSION` (1) is the digit the magic
  carries; a layout break bumps both in one change.
- `build_page(&digest)` makes the content. `read_page(bytes)` returns the digest or a `Refusal`: a
  buffer shorter than the content, a page without the magic, another layout version, or an all-zero
  digest. `Refusal::reason()` is one fixed line for a console.
- `is_plausible(&digest)` is false for an all-zero digest, a frame nobody wrote.
- `INTERFACE_ID` (22) names this interface in a program's `INTERFACES` note.
- The page has no fixed address. A capability names it, and only its one reader maps it.

## Usage

```rust
use base_digest_contract::{Refusal, build_page, read_page};

let digest = [7u8; 32];
let page = build_page(&digest);
assert_eq!(read_page(&page), Ok(digest));
assert_eq!(read_page(&[0u8; 40]), Err(Refusal::NotThisPage));
```

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The minimum supported Rust version is 1.88. There are no Kani harnesses.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
