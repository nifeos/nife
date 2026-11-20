# device_tree_blob

[![check](https://github.com/nifeos/device_tree_blob/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/device_tree_blob/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/device_tree_blob/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/device_tree_blob/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/device_tree_blob#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/device_tree_blob/blob/main/Cargo.toml)

A flattened device tree parser: bytes in, memory regions, register regions and property values out.
The device tree blob is the machine describing itself, handed over by firmware (a bootloader, a UEFI
loader or QEMU). The crate reads it as untrusted input. It checks every offset and bounds every
walk, and a malformed blob is an `Error`, never a panic. It is `no_std`, has no allocator and no
dependencies, and reads the flattened format of chapter 5 of the Devicetree Specification
(release 0.4), versions up to 17.

## EXAMPLES

```rust
use device_tree_blob::{DeviceTreeBlob, Region};

// A tree dumped from QEMU's riscv64 `virt` machine, kept in this repository.
let blob = include_bytes!("tests/fixtures/qemu-riscv64-virt.dtb");
let tree = DeviceTreeBlob::from_bytes(blob)?;

let mut ram = [Region { start: 0, size_bytes: 0 }; 4];
let banks = tree.memory_regions(&mut ram)?;
assert_eq!(&ram[..banks], [Region { start: 0x8000_0000, size_bytes: 0x800_0000 }]);

let mut rtc = [Region { start: 0, size_bytes: 0 }; 1];
let found = tree.node_register_regions_compatible(b"google,goldfish-rtc", &mut rtc)?;
assert_eq!(&rtc[..found], [Region { start: 0x10_1000, size_bytes: 0x1000 }]);
# Ok::<(), device_tree_blob::Error>(())
```

The same code runs as a doctest on `memory_regions` and `node_register_regions_compatible`.

## What it reads

| To read | Call |
|---|---|
| The memory banks | `memory_regions` |
| The memory reservation block | `memory_reservations` |
| The children of `/reserved-memory` | `reserved_memory_regions` |
| The register regions of a node, by name prefix or by `compatible` | `node_register_regions`, `node_register_regions_compatible` |
| One property of a node | `node_property`, `node_property_inherited`, `node_property_compatible`, `parent_property_compatible`, `pointer_handle_property`, `node_properties` |
| The `/chosen` node | `initial_ramdisk`, `boot_arguments` |

Build a tree with `DeviceTreeBlob::from_bytes`, or with the crate's one `unsafe` function,
`DeviceTreeBlob::from_pointer`, when firmware hands over only an address.

A few behaviors a stranger may not expect:

- Every lookup walks the structure block from its first token and holds nothing between calls.
- Output goes into a caller's `&mut [Region]` and the count comes back, because the crate has no
  allocator. A slice too short is `Error::TooManyRegions`, never a silent truncation.
- Cell widths come from the nearest ancestor that declares them, as Linux does, and not from the
  specification's default of 2 and 1. A reader that follows the specification returns a different
  region for a tree that is silent.
- `node_register_regions` matches a name prefix (`b"serial"` matches `serial@10000000`) and stops at
  the first match. `node_register_regions_compatible` is the strict form.
- `from_bytes` accepts a buffer longer than the blob and trims it to `totalsize`. `from_pointer`
  reads eight bytes through the pointer before it validates anything, then trusts the header's
  length up to `MAXIMUM_TOTAL_SIZE_BYTES`, 2 MiB.
- A tree nested deeper than `MAXIMUM_DEPTH` (64) is `Error::TooDeep`, and a `reg` under more than
  `MAXIMUM_CELLS` (4) cells is `Error::BadValue`.

## What is proven

Kani harnesses in `src/lib.rs` prove the leaf readers and the token step:

- A read at any offset returns a value or `Truncated`, never a panic, and an in-bounds read equals
  the big-endian decode.
- The alignment helper rounds up to a multiple of four, and the cursor skip never wraps, on a 64-bit
  or a 32-bit `usize`.
- One token step is total and advances, so a walk ends.
- A cell value wider than 64 bits is `BadValue`, and the region decoder never writes past its
  output.
- A buffer of up to 48 bytes gives a tree or an error, and the tree lies inside the buffer.

Each harness has a falsification patch in `falsifications/` that breaks the code under proof and
turns the harness red. The walkers are not proved. The hostile tests in `tests/hostile.rs` pin the
behavior of each lookup on hand-built blobs, and a fuzz target covers the walkers.

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
setup`. Setup downloads a model checker and the Rust toolchain Kani is pinned to, so it needs
network access. Then run every harness:

```sh
cargo kani
```

KANI_TIMING

A passing run prints warnings that mean nothing, among them `use of an unstable feature` for
`register_tool`, `Found the following unsupported constructs`, and lines about unreachable code.
The answer is the last line, `Complete - 9 successfully verified harnesses, 0 failures, 9 total.`
To run one harness, give its full path:

```sh
cargo kani --harness verification::be32_is_total --exact
```

### Running one falsification patch

Apply the patch, confirm the proof fails, then undo it with the same patch reversed:

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.be32_is_total.patch
cargo kani --harness verification::be32_is_total --exact
patch -R -p1 --no-backup-if-mismatch < falsifications/verification.be32_is_total.patch
```

The failing run prints `VERIFICATION:- FAILED` and `cargo kani` exits 1.

## Using it

The project has not published this crate, and publishing is on hold. No release tag exists yet.
Depend on it by git, pinned to a commit of this repository's `main` and never to a branch:

```toml
[dependencies]
device_tree_blob = { git = "https://github.com/nifeos/device_tree_blob", rev = "<full commit hash>" }
```

Once a pre-release tag exists, use `tag = "<that tag>"` in place of `rev`.

## Fuzzing

Weekly fuzzing of the crate's public inputs is off, because `DeviceTreeBlob::from_pointer` is
`unsafe`. The repository holds one fuzz target, `fuzz/fuzz_targets/device_tree_blob_walk.rs`, which
feeds arbitrary bytes through every lookup.

## BUGS

- `from_pointer` reads eight bytes through a raw pointer before it validates anything. That is the
  crate's one `unsafe` function, and the caller must hand it a pointer to a blob.
- No 32-bit target has been built. The cursor arithmetic is checked and proved for a 32-bit `usize`,
  but nobody has run it there.
- The walkers are not proved.

## Minimum Rust version

Rust 1.88. To check it, install the toolchain and run the tests with `+1.88`:

```sh
rustup toolchain install 1.88 --profile minimal
cargo +1.88 test
```

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
