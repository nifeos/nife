# extensible_host_controller_interface

Everything a Universal Serial Bus (USB) host controller (xHCI) driver computes, with nothing it touches: register offsets
and field decode, Transfer Request Block (TRB) construction and decode, the producer and event ring
cycle discipline, slot and endpoint contexts, port status and the port debounce. The caller performs
every volatile access and owns the direct memory access (DMA) memory; this crate supplies the
arithmetic, so it runs in `no_std`, tests on the host and suits a model checker. It does not touch
hardware, allocate, or drive a controller on its own.

## EXAMPLES

```rust
use extensible_host_controller_interface::{Capabilities, ProducerRing, TransferRequestBlock};

// QEMU's qemu-xhci: 0x40 bytes of capability registers, 64 slots, 16 interrupters and 8 ports
// (4 USB 2 and 4 USB 3, the defaults), runtime at 0x1000, doorbells at 0x2000. Port 1's status
// register and the command ring's doorbell:
let caps = Capabilities::decode(0x0100_0040, 0x0800_1040, 0, 0x0000_7000, 0x2000, 0x1000);
assert_eq!((caps.port_status_and_control(1), caps.doorbell(0)), (0x440, 0x2000));

// A four-entry command ring: three usable slots, then a link TRB back to the first.
let mut ring = ProducerRing::new(0x1000, 4);
let a = ring.enqueue(TransferRequestBlock::enable_slot());
assert_eq!((a.index, a.transfer_request_block.is_cycle(), a.link), (0, true, None));
```

The caller writes each placed TRB (and the link, when `link` is `Some`) to DMA memory at
`ring.entry_physical_address(index)`, then rings the doorbell.

## What is proven

Seven [Kani](https://github.com/model-checking/kani) harnesses, in `src/verification.rs`:

- `a_producer_ring_stays_in_bounds`: from every reachable state, a producer ring never places a TRB
  on its link slot or past its end, and places the link exactly on the last slot.
- `the_event_ring_stays_in_bounds_and_flips_only_at_the_wrap`: from every reachable state, consuming
  an event leaves the dequeue index inside the segment, flips the cycle bit exactly when the index
  wraps to zero, and keeps the `ERDP` address inside the segment. The segment is assumed not to run
  past the end of the 64-bit address space.
- `the_capability_walk_ends_and_stays_inside_the_limit`: for any start, limit and id, and any dword
  the controller returns at each read, `find_extended` makes at most 64 reads, each at a nonzero
  offset whose whole dword is inside `limit` and past the previous read, and returns only an offset
  it read whose id matched.
- `a_device_context_index_is_one_to_thirty_one_and_distinct`: for every endpoint address, the device
  context index is from 1 to 31, endpoint 0 is index 1 in both directions, two different endpoints
  never share an index, and the offsets of every index lie inside a 32- or 64-byte-per-context
  device or input context.
- `the_slot_and_endpoint_contexts_keep_each_field_in_its_own_bits`: for every input, a slot context
  and an endpoint context read back each field from exactly the bits the specification gives it,
  the dequeue pointer is reassembled from its two dwords, and the reserved dwords stay zero.
- `a_port_write_never_acknowledges_what_nobody_asked_to`: for any port status register value,
  `write_value` writes the preserved bits plus exactly the bits asked for, so writing nothing writes
  no change bit, no Port Enabled bit and no reset; the speed id is the four bits it comes from.
- `the_interrupt_interval_is_the_exponent_the_specification_asks_for`: for any speed and
  `bInterval`, a full- or low-speed interval is the largest exponent within the device's period,
  between 3 and 10, and any other is the device's exponent less one; the default control packet is
  known for speeds 1 to 5 and no others.

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
checker, so it needs network access. CI uses version 0.68.0, and another version may verify
differently.

Rerun them with `cargo kani` from the crate's directory. Each harness has a falsification patch at
`falsifications/<module.path>.<harness>.patch`; apply it with `patch -p3` and the harness fails at
its own assertion, which shows the proof checks something real. Revert the patch afterward.

Not yet proven: event decoding.

## BUGS

- Hubs are not modeled (route string 0), so a device behind a hub cannot be addressed. Only devices
  on the root ports work.
- One page size: the DMA layout assumes 4 KiB pages (`PAGESIZE` bit 0).
- No streams, isochronous or bulk transfers: the crate builds the TRBs a boot keyboard needs and
  nothing more.

The reference is the xHCI 1.2 specification, chapters 4 to 7.

## Minimum Rust version

1.88.

## License

Licensed under either of the MIT license ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
