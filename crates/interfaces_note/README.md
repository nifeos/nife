# interfaces_note

[![check](https://github.com/nifeos/interfaces_note/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/interfaces_note/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/interfaces_note/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/interfaces_note/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/interfaces_note#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/interfaces_note/blob/main/Cargo.toml)

The note that declares which interface versions a program was built against. A program carries one
Executable and Linkable Format (ELF) note listing each interface it links as a pair of an id and a
version. The host that spawns the program decodes the note and refuses a program that declares a
version of an interface the host serves at another version. `no_std`, no allocator, no dependencies.

The note is a compatibility check and not an authority check. A program with no note declares
nothing and is spawned unchecked.

## The note

The note has type `4` (`INTERFACES`). Its descriptor is little-endian: a version word
(`INTERFACES_VERSION`, 1), a count word, then `count` records of `INTERFACE_RECORD_BYTES` (12)
bytes. Each record is an interface id (`u64`, strictly ascending) and that interface's version
(`u32`). One list has one encoding. A count past `MAXIMUM_INTERFACES` (64) is refused. This
crate's own interface id is 21 (`INTERFACE_ID`).

- Write with `encode_interfaces`, which is `const` and refuses an unsorted or repeated list, so in
  a constant that is a compile error.
- Declare with `carry_interfaces!`, one line at module scope. It places an `InterfacesNote` in the
  manifest note section.
- Read with `decode_interfaces`, which returns `Interfaces` or an `Error`
  (`NoVersion`, `UnknownVersion`, `WrongLength`, `BadField`) and never panics.
- Check with `Interfaces::check_served`. A matching version, an absent entry and an absent note all
  pass. Any other version returns a `Refusal` that names the declared and the served version.

## EXAMPLES

```rust
use interfaces_note::{
    DeclaredInterface, Refusal, decode_interfaces, descriptor_bytes, encode_interfaces,
};

const LIST: [DeclaredInterface; 2] = [
    DeclaredInterface { id: 4, version: 7 },
    DeclaredInterface { id: 9, version: 1 },
];
let bytes: [u8; descriptor_bytes(2)] = encode_interfaces(&LIST);

let list = decode_interfaces(&bytes).unwrap();
assert_eq!(list.version_of(4), Some(7));
assert_eq!(list.check_served(4, 7), Ok(()));
assert_eq!(list.check_served(5, 3), Ok(())); // no entry, nothing to refuse
assert_eq!(
    list.check_served(4, 8),
    Err(Refusal { interface: 4, declared: 7, served: 8 }),
);
```

## What a stranger needs to place and read the note

- Every word is little-endian, whatever the target's byte order. An ELF note's own header is in the
  file's byte order, so a big-endian file would carry a header this crate does not write.
- The linker must keep the section. The link script has to `KEEP` the input section the macro names
  into an output section inside a `PT_NOTE` segment, and that segment has to lie in a readable
  `PT_LOAD`. A link script that drops it leaves the program with no note.
- The note's owner string names the operating system that reads it, and the crate repeats it as a
  constant so that it depends on nothing.

## What is proven

Three Kani harnesses in `src/lib.rs` prove the codec. Each has a falsification patch in
`falsifications/` that breaks the code under proof and turns the harness red.

| Harness | Property |
|---|---|
| `interface_verification::an_encoded_interface_list_decodes_back_to_itself` | a list round-trips |
| `interface_verification::a_decoded_interface_list_encodes_back_to_its_bytes` | one list has one encoding |
| `interface_verification::decode_interfaces_never_panics` | any bytes give a list or an `Error` |

Unit tests cover the whole-note layout and the served-version check.

## Building and testing

```sh
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none
cargo build --lib --target aarch64-unknown-none-softfloat
cargo build --lib --target riscv64imac-unknown-none-elf
```

### Running the proofs

Install Kani first: run `cargo install --locked kani-verifier --version 0.68.0`, then `cargo kani
setup`. Setup downloads a model checker and the Rust toolchain that Kani pins, so it needs
network access. Then run every harness:

```sh
cargo kani
```

A passing run prints warnings that mean nothing, among them `use of an unstable feature` for
`register_tool`. The answer is the last line, `Complete - 3 successfully verified harnesses, 0
failures, 3 total.`

### Running one falsification patch

Apply the patch, confirm the proof fails, then undo it with the same patch reversed:

```sh
patch -p1 --no-backup-if-mismatch < falsifications/interface_verification.decode_interfaces_never_panics.patch
cargo kani --harness interface_verification::decode_interfaces_never_panics --exact
patch -R -p1 --no-backup-if-mismatch < falsifications/interface_verification.decode_interfaces_never_panics.patch
```

The failing run prints `VERIFICATION:- FAILED` and `cargo kani` exits 1.

## Using it

The project has not published this crate, and publishing is on hold. No release tag exists yet.
Depend on it by git, pinned to a commit of this repository's `main` and never to a branch:

```toml
[dependencies]
interfaces_note = { git = "https://github.com/nifeos/interfaces_note", rev = "<full commit hash>" }
```

Once a pre-release tag exists, use `tag = "<that tag>"` in place of `rev`.

## BUGS

- A program names its interfaces by hand in `carry_interfaces!`, although each version comes from
  the interface crate's constant. Deriving the list from the dependency graph at package build is
  not built.
- A program with no note declares nothing and is spawned unchecked.

## Minimum Rust version

Rust 1.88. To check it, install the toolchain and run the tests with `+1.88`:

```sh
rustup toolchain install 1.88 --profile minimal
cargo +1.88 test
```

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
