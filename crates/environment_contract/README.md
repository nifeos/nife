# environment_contract

[![check](https://github.com/nifeos/environment_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/environment_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/environment_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/environment_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/environment_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/environment_contract/blob/main/Cargo.toml)

The read-only page that carries a process's time zone, locale and terminal type, and the rules for
writing and reading it. `no_std`, no dependencies, no system calls. The program that starts a
process writes the page, and the process's start-up code reads it. They share this one definition,
so they cannot drift.

A value reaches the page only as a member of a closed list. The writer refuses anything else, and
the reader answers only a member, so a page some other writer produced cannot hand a program an
arbitrary string such as a password.

## The page

The page is 107 bytes (`PAGE_CONTENT_BYTES`). It starts with the 8-byte magic `ENVCONF1`. Then each
of `TZ`, `LANG` and `TERM` takes one length byte and a fixed field of 40, 24 and 32 bytes.

- Write with `PageBuilder`. Each setter (`time_zone`, `locale`, `terminal_type`) returns
  `Err(Refused)` for a value outside its list. `build` returns the page's bytes. A key never set
  stays zero.
- Read with `ConfigurationPage`. `ConfigurationPage::new` is `unsafe` because it takes a raw
  address. Each accessor returns `None` for a zeroed or foreign frame, for a key never set, for a
  length byte past its field and for a value outside its list. No input makes it panic.
- `INTERFACE_ID` is 13, `VERSION` is 1 and `MAGIC` is `ENVCONF1`. A format change is a new
  version, never a new number. A fourth key is a new version.

## EXAMPLES

```rust
use environment_contract::{ConfigurationPage, PageBuilder, Refused};

let page = PageBuilder::new()
    .time_zone("America/Los_Angeles").unwrap()
    .locale("en_US.UTF-8").unwrap()
    .terminal_type("xterm-256color").unwrap()
    .build();

// SAFETY: `page` is a live buffer of exactly `PAGE_CONTENT_BYTES`, alive for this block.
let read = unsafe { ConfigurationPage::new(page.as_ptr() as u64) };
assert_eq!(read.time_zone(), Some("America/Los_Angeles"));
assert_eq!(read.locale(), Some("en_US.UTF-8"));
assert_eq!(read.terminal_type(), Some("xterm-256color"));

assert_eq!(
    PageBuilder::new().time_zone("definitely-not-a-timezone"),
    Err(Refused::UnknownTimeZone),
);
```

## The lists

The lists are `domain::KNOWN_TIME_ZONES`, `KNOWN_LOCALES` and `KNOWN_TERMINAL_TYPES`. They are
short on purpose. They are not the full IANA, glibc or terminfo databases, and compiling those in
would be a dependency decision. A constant assertion fails the build when a member outgrows its
field.

| To do this | Change |
|---|---|
| Add a zone, locale or terminal type | A pull request that adds the member to its list and names the real IANA, glibc or terminfo entry. It must fit the field's maximum. |
| Add a fourth key | A new `VERSION` and a new field. |

## What is proven

One Kani harness in `src/verification.rs` proves that `field_bytes`, which every accessor reads
through, never panics for any length byte. A length byte past the field reads as absent. The
harness covers the bound that once panicked: a length byte of 255 on the last field. Its
falsification patch in `falsifications/` turns it red.

| Harness | Property | Falsification patch |
|---|---|---|
| `verification::field_bytes_never_panics_for_any_length_byte` | a length byte past its field reads as absent and never panics | `verification.field_bytes_never_panics_for_any_length_byte.patch` |

Unit tests and doctests cover the round trip and domain membership, not a proof.

## Building and testing

```sh
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none
cargo build --lib --target aarch64-unknown-none-softfloat
cargo build --lib --target riscv64imac-unknown-none-elf
```

### Running the proof

Install Kani first: run `cargo install --locked kani-verifier --version 0.68.0`, then `cargo kani
setup`. Setup downloads a model checker and the Rust toolchain that Kani pins, so it needs
network access. Then run the harness:

```sh
cargo kani
```

A passing run prints warnings that mean nothing, among them `use of an unstable feature` for
`register_tool`. The answer is the last line, `Complete - 1 successfully verified harnesses, 0
failures, 1 total.`

### Running the falsification patch

Apply the patch, confirm the proof fails, then undo it with the same patch reversed:

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.field_bytes_never_panics_for_any_length_byte.patch
cargo kani --harness verification::field_bytes_never_panics_for_any_length_byte --exact
patch -R -p1 --no-backup-if-mismatch < falsifications/verification.field_bytes_never_panics_for_any_length_byte.patch
```

The failing run prints `VERIFICATION:- FAILED` and `cargo kani` exits 1.

## Using it

The project has not published this crate, and publishing is on hold. No release tag exists yet.
Depend on it by git, pinned to a commit of this repository's `main` and never to a branch:

```toml
[dependencies]
environment_contract = { git = "https://github.com/nifeos/environment_contract", rev = "<full commit hash>" }
```

Once a pre-release tag exists, use `tag = "<that tag>"` in place of `rev`.

## Fuzzing

Weekly fuzzing is off for this crate, because the reader has `unsafe` code in one place:
`ConfigurationPage::new` takes a raw address.

## BUGS

- The only reader is `unsafe`. A safe reader over borrowed bytes is not built yet.
- The lists are short on purpose. `KNOWN_LOCALES` spells the codeset `UTF-8`, as glibc's `LANG`
  does, while `locale -a` prints `utf8`. Nobody has verified whether `setlocale` treats the two alike.

## Minimum Rust version

Rust 1.88. To check it, install the toolchain and run the tests with `+1.88`:

```sh
rustup toolchain install 1.88 --profile minimal
cargo +1.88 test
```

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
