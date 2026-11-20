# pci_configuration_space

[![check](https://github.com/nifeos/pci_configuration_space/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/pci_configuration_space/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/pci_configuration_space/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/pci_configuration_space/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/pci_configuration_space#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/pci_configuration_space/blob/main/Cargo.toml)

PCI and PCI Express configuration-space decoding, with none of the memory-mapped input and output.
The crate addresses a function in the enhanced configuration access mechanism (ECAM) window, walks
a machine's buses for functions (following PCI-to-PCI bridges), sizes a function's base address
registers, and decodes its capability lists: MSI, MSI-X and the virtio PCI vendor capabilities. It
reads and writes through closures on `(Bdf, offset)`, so a kernel passes volatile accessors into
the mapped window and a test passes an array. `no_std`, no dependencies, `forbid(unsafe_code)`.

A device is untrusted input. Every function here is total: a device that answers every read with
arbitrary bytes leaves the caller with a bounded, well-formed answer or `None`. A capability list
is a linked list the device supplies, so each walk stops after 64 entries; the bus walk visits
each bus at most once, so a bridge that names its own bus or a loop terminates.

## EXAMPLES

```rust
use pci_configuration_space::{Bdf, VENDOR_ID_REGISTER, VIRTUAL_IO_VENDOR, enumerate};

// A configuration space with exactly one function: a modern virtio block device at 00:01.0.
// Every other slot reads 0xffff, the bus's way of saying nobody is home.
let mut read32 = |bdf: Bdf, offset: u64| -> u32 {
    if bdf == (Bdf { bus: 0, device: 1, function: 0 }) && offset == VENDOR_ID_REGISTER {
        (0x1042 << 16) | VIRTUAL_IO_VENDOR as u32
    } else if offset == VENDOR_ID_REGISTER {
        0xffff_ffff
    } else {
        0
    }
};

let mut found = Vec::new();
enumerate(1, &mut read32, &mut |bdf, vendor, device| found.push((bdf, vendor, device)));
assert_eq!(found.len(), 1);
assert_eq!(found[0].0.ecam_offset(), 1 << 15);
```

## What is proven

Ten Kani harnesses (`cargo kani`) cover every input: the ECAM offset stays inside the 256-bus
window; `intx_interrupt_number` is total; `read_bars` is total and every register it returns has
a power-of-two size and an end that does not overflow; a size is exactly the boundary between the
bits a device ignores and the bits it decodes; the three capability walks terminate on any device
(the shared walk visits at most 64 entries) and an MSI-X table size is in `1..=2048`; the bus
queue never runs past its array on any sequence of bridges; the ranges parse is total. Each has a
patch under `falsifications/` that makes it fail. The scan loops of `walk` over 256 buses are
bounded by literals and not proved; a harness of `walk` on a two-bus machine ran over twelve
CPU-minutes without finishing and was not kept.

## What it decodes

- `Bdf`: bus, device and function, with `ecam_offset` (4 KiB per function, `bus:8 | device:5 |
  function:3 | offset:12`) and `requester_id` (the 16-bit identifier a function stamps on its
  memory transactions, which an input and output memory management unit keys a device by). A
  device or function number wider than its field is masked, so it never names another page.
- `walk` and `enumerate`: every function reachable from bus 0, with its ids, class code, header
  type, and for a bridge the bus numbers behind it. A caller states the bus number it can read, and
  a bridge naming a bus at or above it is reported but not entered.
- `read_bars`: the memory base address registers of a header, sized by the write-all-ones probe and
  restored afterwards. A register whose writable bits are not contiguous, whose base plus size
  overflows, or whose 64-bit upper half would fall outside the header is refused with a
  `BarRefusal` rather than sized.
- `virtual_io_capabilities`, `msi_capability`, `msix_capability`: the capability list, decoded.
- `intx_interrupt_number`: the legacy interrupt swizzle of QEMU's `virt` boards.
- `memory_window_32_bit`: the 32-bit non-prefetchable window from a PCI host bridge's `ranges`
  property.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
cargo kani                                       # the proofs
```

## BUGS

- Only memory base address registers are decoded. An input and output BAR, which x86 legacy
  devices carry, is skipped.
- `intx_interrupt_number` is the swizzle of a function directly on bus 0. A function behind a
  bridge has its pin rotated by each bridge first, which this crate does not do; a board with
  its own routing needs the device tree's `interrupt-map`.
- The crate does not size or assign a bridge's own windows.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
