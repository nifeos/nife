# grant_plan

[![check](https://github.com/nifeos/grant_plan/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/grant_plan/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/grant_plan/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/grant_plan/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/grant_plan#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/grant_plan/blob/main/Cargo.toml)

The command-line logic of a capability shell, where naming a resource on the line is what grants
it. `parse` turns a line of bytes into a builtin or a program invocation. `plan_against` places an
invocation's words into the grants a program's manifest declares and answers an `Endowment`, which
is exactly what to hand the child. A line the manifest does not allow gets a typed `Refusal` the
shell prints at the prompt, before anything runs.

The manifest types come from `grant_contract`; this crate knows no program, so the caller resolves
a program's name to its manifest. Patterns on the line expand against a directory listing the
caller supplies, through the `byte_glob` matcher. `no_std`, no allocation, no input or output and
no system call.

## EXAMPLES

A program that reads one file named on the line and takes one option, `-v`:

```rust
use grant_contract::*;
use grant_plan::expansion::Expansion;
use grant_plan::{CommandLine, Refusal, ShellHoldings, parse, plan_against};

const COUNT: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Forbidden,
    file: FileGrantDeclaration::Required { writable: false },
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: ShortOptions::new(b"v"),
    output: OutputDeclaration::Bytes,
    input: InputDeclaration::Forbidden,
    reports: true,
    interruptible: false,
    clock: false,
    domain: false,
    configuration: false,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    startup: StartupContract::Native,
};

// A shell that holds one directory to grant from, standing at its root.
let holdings = ShellHoldings { directory: true, ..ShellHoldings::default() };

let CommandLine::Run(run) = parse(b"count -v notes.txt") else { unreachable!() };
let plan = plan_against(&run, COUNT, &holdings, &Expansion::none()).unwrap();
let file = plan.file.unwrap();
assert_eq!(file.name.as_bytes(), b"notes.txt");
assert!(!file.writable);
assert_ne!(plan.short_options, 0);

// An option the manifest does not declare is refused, not passed along unread.
let CommandLine::Run(run) = parse(b"count -x notes.txt") else { unreachable!() };
assert_eq!(
    plan_against(&run, COUNT, &holdings, &Expansion::none()),
    Err(Refusal::NoSuchOption),
);
```

`cargo run --example plan_a_line` plans a whole line, with its pipes and redirections.

## What is proven

No Kani harness covers this crate yet. Host tests cover the parser, the planner, the expansion of
patterns into name sets, navigation across two directory grants and every refusal.

## Building and testing

```sh
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

## BUGS

- A shell holds at most two directory grants, and a pattern expands to at most eight names in one
  grant. A larger match is refused or batched, never truncated.
- The name and set bounds are spelled as this crate's own constants, so the planner does not link a
  filesystem contract. A consumer that links both pins the two together in its own tests.

## Minimum Rust version

Rust 1.88. Check it with `rustup toolchain install 1.88 --profile minimal`, then `cargo +1.88 test`.

## License

Licensed under either of the MIT license (`LICENSE-MIT`) or the Apache License, Version 2.0
(`LICENSE-APACHE`), at your option.
