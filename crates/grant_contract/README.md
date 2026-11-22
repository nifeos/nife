# grant_contract

[![check](https://github.com/nifeos/grant_contract/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/grant_contract/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/grant_contract/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/grant_contract/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/grant_contract#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/grant_contract/blob/main/Cargo.toml)

What a program declares it can receive, as types, in a capability system where a program holds
only what its spawner hands it. A `Manifest` names each grant a program accepts: an integer
argument, memory, one file, one directory, its words, its output and input streams, and the
services its spawner endows it with rather than the command line.

The capability slot numbers (`DOMAIN_SLOT`, `ENTROPY_SLOT`, `NETWORK_SLOT` and their neighbors) say
where a program finds each of those services. A spawner endows from a manifest, a command-line
planner checks a line against one, and the program reads its slots from the same numbers, so none
of the three keeps a copy that could drift.

`narrow_by_verb` narrows a manifest to what one verb of a program needs, so `list` and `install`
can be one program that holds different authority per line. `no_std`, no allocation, no input or
output. Its one dependency is `program_argument_contract`, the page a program reads its verb from.

## EXAMPLES

A package manager whose `install` verb holds the network and whose `list` verb does not:

```rust
use grant_contract::*;

let package_manager = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Forbidden,
    file: FileGrantDeclaration::Forbidden,
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: NO_SHORT_OPTIONS,
    output: OutputDeclaration::Bytes,
    input: InputDeclaration::Forbidden,
    reports: true,
    interruptible: false,
    clock: false,
    domain: false,
    configuration: false,
    entropy: false,
    network: true,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: true,
    indexes: IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    startup: StartupContract::Std,
};
assert!(package_manager.is_well_formed());

let verbs = [
    VerbGrant { verb: "list", authorities: ManifestAuthorities::CATALOG },
    VerbGrant {
        verb: "install",
        authorities: ManifestAuthorities::CATALOG.union(ManifestAuthorities::NETWORK),
    },
];
let list = narrow_by_verb(&package_manager, &verbs, Some(b"list"));
assert!(list.catalog && !list.network);
let install = narrow_by_verb(&package_manager, &verbs, Some(b"install"));
assert!(install.catalog && install.network);

// A word that is no verb in the table keeps none of the verb-chosen fields.
let unknown = narrow_by_verb(&package_manager, &verbs, Some(b"remove"));
assert!(!unknown.catalog && !unknown.network);

// The program finds its network client at the slot its spawner placed it in.
assert_eq!(NETWORK_SLOT, 10);
```

## What is proven

No Kani harness covers this crate yet. Host tests cover the manifest's rules (an option set, a
well-formed directory grant) and the narrowing by verb.

## Building and testing

```sh
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

## BUGS

- The slot numbers are one layout, fixed in this crate. A system that places capabilities
  elsewhere needs its own copy, and its programs cannot share a binary with this one's.
- `WordGrant::rights` and `IndexStoreDeclaration::rights` spell the filesystem rights as numbers
  (enumerate 1, read 2, write 4, create 8, descend 32), because this crate does not depend on a
  filesystem contract. A consumer that links both pins the two together in its own tests.
- A `Manifest` has no default, so every declaration spells out every field, as the example
  shows. That is deliberate: a new field is a compile error at each program, never a silent grant.

## Minimum Rust version

Rust 1.88. Check it with `rustup toolchain install 1.88 --profile minimal`, then `cargo +1.88 test`.

## License

Licensed under either of the MIT license (`LICENSE-MIT`) or the Apache License, Version 2.0
(`LICENSE-APACHE`), at your option.
