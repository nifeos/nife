# block_roster

The page that answers "what block devices are attached": a small, fixed-width, pointer-free layout
that one party writes once, at wiring time, and that other programs then map read-only and read.
`no_std`, no dependencies. The writer and the readers share this one definition, so they cannot
drift.

This is a shared page, not a message protocol. There is no request, no reply and no server. Holding
a roster tells a program that a device exists; it does not let the program touch it. Which program
gets which device's endpoint is decided elsewhere, so a program without the page has nowhere to
look.

## The contract

The page is little-endian:

| bytes | field |
|---|---|
| `0..8` | `MAGIC`, the ASCII text `blkrosv1` read as a little-endian `u64` |
| `8..12` | count: how many entries follow |
| `12..16` | capacity: how many entries this page has room for |
| `16..` | entries, `ENTRY_BYTES` (8) each |

An entry is a `u32` ordinal (which device of this transport, counting from 0) and a `u32` transport,
one of `TRANSPORT_VIRTIO_MEMORY_MAPPED_IO` (0), `TRANSPORT_VIRTIO_PCI` (1) and
`TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS` (2).

- The roster carries no capacity in bytes. Size is learned by asking the device.
- `Roster::read` returns `None` when the page is shorter than the header, when the magic is wrong
  (a zeroed frame is not an empty roster) or when the count runs past what the page holds. A
  reader never indexes past its own mapping.
- A number the reader does not know as a transport has no name (`Device::transport_name` returns
  `None`); the reader decides what to do with it.
- The page has one version, `VERSION` (1), which is also the digits of the magic. A layout break
  bumps both in one change, and an old reader then refuses the page. `INTERFACE_ID` (5) names this
  interface when a program declares which interfaces it links.

## Usage

```rust
use block_roster::{Device, Roster, TRANSPORT_VIRTIO_PCI};

// The writer lays the roster into a page it owns.
let mut page = [0u8; 4096];
block_roster::write(&mut page, &[Device { ordinal: 0, transport: TRANSPORT_VIRTIO_PCI }]).unwrap();

// A reader holds the mapped page and borrows from it.
let roster = Roster::read(&page).unwrap();
assert_eq!(roster.len(), 1);
assert_eq!(roster.get(0).unwrap().transport_name(), Some("virtio-pci"));
assert_eq!(roster.count_on(TRANSPORT_VIRTIO_PCI), 1);
```

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The minimum supported Rust version is 1.88. Two Kani harnesses in `src/lib.rs` prove that `Roster::read` is total on any bytes
and that `write` then `read` returns the same devices in order; run them with `cargo kani`.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
