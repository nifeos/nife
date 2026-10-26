# extensible_host_controller_interface

Everything a USB host controller (xHCI) driver computes, with nothing it touches: register offsets
and field decode, Transfer Request Block (TRB) construction and decode, the producer and event ring
cycle discipline, slot and endpoint contexts, port status and the port debounce. The caller performs every volatile access and owns the DMA memory; this crate supplies the
arithmetic, so it runs in `no_std`, tests on the host and suits a model checker. It does not touch
hardware, allocate, or drive a controller on its own.

## What is proven

One [Kani](https://github.com/model-checking/kani) harness, in `src/verification.rs`:

- `a_producer_ring_stays_in_bounds`: from every reachable state, a producer ring never places a TRB
  on its link slot or past its end, and places the link exactly on the last slot.

Rerun it with `cargo kani` from the crate's directory. The harness has a falsification patch at
`falsifications/<module.path>.<harness>.patch`; apply it with `patch -p3` and the harness fails at
its own assertion, which shows the proof checks something real. Revert the patch afterward.

Not yet proven: the event ring's bounds and cycle flip, the extended-capability walk, event
decoding, and the context and port decoders.

## Usage

```rust
use extensible_host_controller_interface::{Capabilities, ProducerRing, TransferRequestBlock};

// QEMU's qemu-xhci: 0x40 bytes of capability registers, 64 slots, 4 ports, runtime at 0x1000,
// doorbells at 0x2000. Port 1's status register and the command ring's doorbell:
let caps = Capabilities::decode(0x0100_0040, 0x0400_0840, 0, 0x0000_7000, 0x2000, 0x1000);
assert_eq!((caps.portsc(1), caps.doorbell(0)), (0x440, 0x2000));

// A four-entry command ring: three usable slots, then a link TRB back to the first.
let mut ring = ProducerRing::new(0x1000, 4);
let a = ring.enqueue(TransferRequestBlock::enable_slot());
assert_eq!((a.index, a.trb.cycle(), a.link), (0, true, None));
```

The caller writes each placed TRB (and the link, when `link` is `Some`) to DMA memory at
`ring.address_of(index)`, then rings the doorbell.

## Limitations

- Hubs are not modeled (route string 0), so a device behind a hub cannot be addressed. Only devices
  on the root ports work.
- One page size: the DMA layout assumes 4 KiB pages (`PAGESIZE` bit 0).
- No streams, isochronous or bulk transfers: the crate builds the TRBs a boot keyboard needs and
  nothing more.

The reference is the xHCI 1.2 specification, chapters 4 and 6.

## Minimum Rust version

1.88.

## License

Licensed under either of the MIT license ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
