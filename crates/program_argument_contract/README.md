# program_argument_contract

[![check](https://github.com/nifeos/program_argument_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/program_argument_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/program_argument_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/program_argument_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/program_argument_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/program_argument_contract/blob/main/Cargo.toml)

One read-only 4 KiB page that carries a program's argv as length-prefixed bytes. Whoever spawns the
program writes it with `PageBuilder`, and the program's runtime reads it with `ArgumentPage`. `no_std`,
no dependencies, so the writer and the reader share one definition and cannot drift on the layout.

The bytes carry no authority. The access a program holds decides what it may read or write, never
what its arguments say, so nothing checks the page's bytes for meaning.

## The contract

- The page is `PAGE_BYTES` (4096) bytes: a header of `HEADER_BYTES` (16) and then records. The header is
  `MAGIC` (eight bytes), a little-endian `u32` count of arguments and a little-endian `u32` total
  of record bytes. Each record is a little-endian `u32` length and then that many bytes. Any byte
  may appear in an argument, which is why the layout is length-prefixed and not delimited.
- The first argument is `argv[0]`, because programs such as `ripgrep` skip it and `clap` treats it
  as the program's name.
- The records have room for `MAXIMUM_RECORDS_BYTES` (4,080) bytes, each argument's `record_bytes`
  (its four-byte prefix and its bytes) included. When a line does not fit, `PageBuilder` refuses all
  of it with `PushRefused::TooLong` and truncates nothing.
- A page that does not parse reads as no arguments: a wrong magic, a zeroed frame, a count that
  disagrees with the records, or a length that overruns the page. The reader never returns a partial
  argv, because a program that sees half its arguments would act on a line nobody typed.
- `PageBuilder::new` takes a slice and refuses, with `WrongPageLength`, any whose length is not `PAGE_BYTES`.
- `ArgumentPage::from_virtual_address` is `unsafe` and names a mapped page; `ArgumentPage::parse`
  takes any slice, and is the safe reader.
- The identity is `VERSION` 1 and `INTERFACE_ID` 18, with the layout above.

## EXAMPLES

```rust
use program_argument_contract::{ArgumentPage, PAGE_BYTES, PageBuilder};

let mut page = [0u8; PAGE_BYTES];
let mut builder = PageBuilder::new(&mut page).unwrap();
for argument in [&b"rg"[..], b"ab\xffcd", b"src"] {
    builder.push(argument).unwrap();
}
assert_eq!(builder.finish(), 3);

let arguments: Vec<&[u8]> = ArgumentPage::parse(&page).iter().collect();
assert_eq!(arguments, [&b"rg"[..], b"ab\xffcd", b"src"]);
```

## What is proven

Two Kani harnesses cover the parser, over a symbolic 48-byte page that carries the magic.
`parse_never_panics_and_iterates_len_items` proves that `parse` never panics and that the iterator
yields exactly `len()` arguments, with `is_empty` agreeing.
`a_parsed_page_holds_exactly_what_its_header_states` proves that a page which parses holds exactly
`count` arguments whose four-byte prefixes and bytes add up to `total`. Each has a falsification
patch in `falsifications/` that turns it red.
`the_builder_and_the_parser_round_trip_two_arguments` proves that two arguments of three and two
symbolic bytes, written over a 4,096-byte page of stale bytes, read back exactly as `PageBuilder`
wrote them. Its falsification patch is in `falsifications/` too. Each harness verifies in under a
minute. No proof covers the caller's promise that the address names a mapped page,
because Kani cannot see a mapping.

## Building and testing

```sh
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

## BUGS

- Damage to two header fields at once can read as a shorter argv. A `count` and a `total` that
  both describe a prefix of the records parse as exactly that prefix, since the layout has no
  checksum. No single-field damage yields a partial argv. The page has one writer and maps
  read-only, so this is a property of the format under corruption and not an attack surface.
- `from_virtual_address` reads through a `usize` pointer, so on a target narrower than 64 bits an
  address above 4 GiB would read a different page. Its `# Safety` section makes a fitting address
  the caller's promise. No target built here is that narrow.
- A secret on a command line is plain bytes on this page.
- This crate is one file, `src/lib.rs`, because a standard library port copies it by
  name. A test checks that `src/` holds no other file.
- `PageBuilder::new` takes a slice of exactly 4,096 bytes, so a proof over the builder needs the
  whole page. With symbolic argument lengths the solver ran past 11 GiB and failed, so the
  round-trip proof fixes the lengths at three and two bytes and leaves the bytes symbolic. The
  tests cover other lengths, the empty argument included.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
