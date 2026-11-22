# Review package: `abi`

*Name: provisional. The `abi` package of [crate-reviews.md](../crate-reviews.md), in its own file
because the packages together outgrow the page.*

This package is for the architect's clearance of the crate's move to its own repository, the last
contract crate of step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as the branch of pull request 3942 leaves it. The outside review behind it is
[milestone 1075 (`abi`: what an outside maintainer would fix before the move)](../../design/roadmap/1075-abi-outside-review.md),
whose finding status says what each item needs.

## In one paragraph

The numbers a program and a kernel agree on: five system call numbers, the method numbers of every
capability object, the rights bits and the error codes. It also holds a few message-word
conventions: the fault rendezvous slot, the console's register stride and the cycle counter's
meaning. It is `no_std`, has no
dependency and no `unsafe`, and holds only constants, two small enums and a handful of `const fn`
decoders. Fifteen crates in this tree depend on it, the kernel and the runtime among them, and the
`std` port carries a generated copy. The architect ruled its published name `moho_abi` in §323 (the
binary interface, and the names of the layers), ruling 2 of 2026-11-14 (UTC). The rename belongs to
milestone 1121 (the Moho rename: the order in which the binary interface takes its name). It has not
landed, so the crate is still `abi` here. No file in the crate names nife, a path in this tree or a private machine.

## Identity

| item | value |
|---|---|
| crate | `abi` today, ruled `moho_abi` (§323 ruling 2) |
| version | `0.1.0`; no tag and no repository yet |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0` in the manifest |
| `publish` | `false`; the manifest comment says the line stays until the release that publishes |
| size | 25 source files, 2,216 lines with the tests, a README of 88 lines |
| identity values | `ABI_VERSION` 1, `INTERFACE_ID` 1 |
| the version rule | a changed or removed public number bumps `ABI_VERSION`; a new number owes nothing. A test counts every public `u64` and `u32` against the pinned lists, and a second test holds every pinned line to `pinned-at-version-1.txt` |

## Who in nife depends on it

Fourteen manifests depend on it by a path dependency: `kernel`, `user_mode_runtime`,
`components`, `fixtures`, `jig`, `system_initializer`, `process_builder`, `capability_plan`,
`confined_system_call_fuzzer_contract`, `memory_region_heap`, `pgrep`, `pmap`, `ps` and `top`.
`system_tests` takes it as a development dependency. The `std` overlay reads a copy that the farm generates from the crate's sources. Two
cleared packages wait for this crate's repository: `user_mode_runtime` and `memory_region_heap`.

## Public API

| kind | items |
|---|---|
| root | `ABI_VERSION`, `INTERFACE_ID`, the five `SYSTEM_CALL_*` numbers, `CAPABILITY_TABLE_SLOTS`, `Error` and its fifteen variants |
| method modules | `address_space`, `memory_region`, `notification`, `page_frame`, `rendezvous`, `reply`, `thread_control_block`, `timer`, `interrupt`, `port_range`, `reboot`, `process`, `virtual_io`, `console` (one reserved number) |
| conventions | `address_wait` (flags of the address wait), `fault` (the fault rendezvous and its events), `uart` (`RegisterShape`), `cycle_counter` (`CycleMeaning`), `rights`, `object_type`, `survey`, `usage` |

The register convention (which register carries the system call number and each argument on
aarch64, riscv64 and `x86_64`) is a table at the crate root.

## Proofs, tests and what was run

The crate has no Kani harness and no falsification patch. Its tests pin every number: 25 host tests
and 4 doctests.

On the branch of pull request 3942, on stable 1.99.1:

| check | result |
|---|---|
| `cargo test -p abi` | 25 host tests and 4 doctests pass |
| `cargo clippy -p abi --all-targets -- -D warnings` | no warning |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p abi --no-deps` | no warning |
| `cargo build -p abi --lib --target <t>` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` | all three build |
| `cargo check -p pmap -p ps -p memory_region_heap -p user_mode_runtime` | builds |
| the template's helpers at `nifeos/crate-workflows` `1b9abe9`, run by hand over the crate | prose caps, passive voice, history phrases, acronyms, comment blocks, file and function length, spelled-out words, unit symbols and British spelling pass. `register_names` refuses six names (below) |
| `python3 helpers/name_release.py --draft abi` | 146 rows: 19 ratified, 127 provisional, none without a block |
| `script/crate-review-packet abi` | no dependency, no `unsafe`, no reference back into nife |

Not run, and so not claimed:

- Miri: the crate has no `unsafe`.
- Mutation. The census belongs in the 08:00 to 13:00 UTC window, and the first weekly run in the
  new repository lists survivors (item 20).
- `script/crate-move abi --dry-run`, the stranger test and `cargo-semver-checks`, which has no
  baseline before a first release.
- The kernel boot rows, which run in the merge queue.

## Names

Nineteen names read ratified: the crate, the modules `interrupt` and `object_type`, the five system
call numbers, `MAP_READ_ONLY`, `MAP_READ_WRITE`, `FAULT_RENDEZVOUS_SLOT`, `interrupt::ACKNOWLEDGE`,
`REPLY_DELIVERED`, `SUSPEND`, `RESUME`, `READ_REGISTER`, `WRITE_REGISTER` and the two `tests`
modules. Every other public name reads `Name: provisional.` The ruled rename to `moho_abi` changes
the crate's name and no item's.

Four names come from the outside review's items, set provisionally by the maintainer:

- `address_wait::is_admitted`: a function that answers yes or no is named like
  a question (item 12).
- `impl TryFrom<i64> for Error`, which replaces `Error::from_ret` and hands a non-error word back
  (item 12).
- `RegisterShape::new(register_shift, access_width_bytes)`, with the device tree binding's
  `reg-shift` and `reg-io-width` as aliases (item 16).
- `CapabilitySlot` is gone: nothing named it (item 15).

Six names failed the template's rule that a size carries its unit. The architect ratified their
replacements on 2026-11-22 (UTC), and the rename pass of pull request 3998 carries them across the
kernel, the runtime, the `std` overlay and the system tests. No number changes.

| name in the code | what it is | ratified |
|---|---|---|
| `address_wait::SIZE_U8`, `SIZE_U16`, `SIZE_U32`, `SIZE_U64` | the width of the waited word, in the flags word | `WIDTH_8_BITS`, `WIDTH_16_BITS`, `WIDTH_32_BITS`, `WIDTH_64_BITS` |
| `address_wait::SIZE_MASK` | the two bits that hold the width | `WIDTH_MASK` |
| `usage::SIZE` | the record of the pages a region holds in total | `SIZE_PAGES` |

## What the move still needs

| item | owner |
|---|---|
| The rename to `moho_abi` | milestone 1121, step 3 |
| The six ratified size names above | pull request 3998 |
| The README's badge row, with the repository's name (item 9) | the move commit |
| Convert the fifteen manifests and the `std` farm's copy to the pinned repository | the move lane's nife-side pull request |
| One `mutants-exempts` line or one test per survivor of the first weekly run (item 20) | the move lane, in the new repository |
| The proofs job with no harness, and the bounded `counter_ticks_for` harness attempt (item 21) | the move lane, after the repository's first runs |

## Clearance

Rule in force, ruled 2026-11-22 (UTC), option A of milestone 1075's open decision: the review
clears once step 3 of milestone 1121 (the `moho_abi` rename) lands. That clearance is the go to
create the repository with `publish = false`, no tag and no publish. The six names above read
ratified as of the same ruling.
