# e1000e

The logic of a driver for Intel's e1000e network controllers, the 82574L that QEMU's
`-device e1000e` presents and the Sunrise Point I219, with no register access of its own. It holds
the register offsets and bits, the descriptor formats, the DMA region's layout, the whole receive
and transmit ring logic behind two small traits (`Region` and `Tails`), and the I219 bring-up
sequences ported from FreeBSD's Intel shared code. You supply the volatile register accesses and
the DMA memory; everything computed from them is here, `no_std`, and tested on the host against a
simulated device. It was written for the nife kernel.

## What is proven

Four Kani harnesses cover the input a device or a caller controls:

- `proofs::a_delivered_frame_always_fits_the_buffer_and_the_room`: for every status word a device
  can write and every caller buffer size, a frame that `rx_verdict` delivers fits both the receive
  buffer and the caller's space.
- `proofs::the_handoff_round_trips_and_accepts_only_what_pack_can_make`: `Handoff::unpack` accepts
  only a page-aligned, nonzero DMA base and a unicast MAC address, and what it accepts packs and
  unpacks to itself.
- `proofs::every_ring_offset_is_inside_the_region`: for any index, every descriptor and buffer
  offset lies inside its part of the DMA region.
- `pch::phy::proofs::a_paged_access_selects_the_offsets_own_page_and_register`: every paged PHY
  access names a PHY address that exists, the offset's own register and the offset's own page.

Rerun them with `cargo kani`. Each harness has a falsification record at
`falsifications/<module.path>.<harness>.patch`; applying it with `patch -p3` from the crate root
makes that harness fail at its own assertion, which shows the proof checks something.

The NVM reads, the reset, ULP and PHY workaround sequences, and transmit descriptor status are
tested against the simulated device but not model checked.

## Usage

Claim a device, then judge each receive descriptor's status word before trusting its length:

```rust
use e1000e::{RX_STATUS_DD, RX_STATUS_EOP, RxVerdict, VENDOR_INTEL, is_supported, model, rx_verdict};

assert!(is_supported(VENDOR_INTEL, 0x10d3));
assert_eq!(model(0x10d3), Some("82574L (QEMU's e1000e)"));

let word = |len: u64, status: u8| len | (status as u64) << 32;
assert_eq!(rx_verdict(word(60, RX_STATUS_DD | RX_STATUS_EOP), 1514), RxVerdict::Frame(60));
assert_eq!(rx_verdict(word(60, 0), 1514), RxVerdict::NotYet);
assert_eq!(rx_verdict(word(0xffff, RX_STATUS_DD | RX_STATUS_EOP), 1514), RxVerdict::Drop);
```

For the rings, implement `Region` over the DMA memory and `Tails` over the queue registers, then
drive a `DataPlane` with `start`, `receive` and `transmit`.

## Limitations

- One receive and one transmit queue, polled. No interrupts, checksum offload, segmentation offload
  or jumbo frames.
- Only the 82574L and the Sunrise Point I219 rows of `DEVICE_IDS` are claimed. Later PCH
  generations need workarounds this crate does not carry.
- The I219's link-up half is not ported, so the PHY is not reconfigured when link comes up. A link
  that comes up and then loses frames points there first. Flow control is not configured.
- The I219 port has run on real hardware once, with no cable; otherwise it is proven only against
  the simulator, which is this crate's reading of FreeBSD.
- Legacy descriptors on the I219 are an assumption from the 8254x lineage.
- `Handoff` encodes one kernel's spawn arguments and may leave the crate. The API may still change
  before 1.0.

The minimum supported Rust version is 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option. The
files under `src/pch/` adapt FreeBSD's Intel shared code and also carry Intel's BSD-3-Clause
notice, which a binary redistribution must reproduce.
