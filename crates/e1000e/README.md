# e1000e

The logic of a driver for Intel's e1000e network controllers, the 82574L that QEMU's
`-device e1000e` presents and the Sunrise Point I219, with no register access of its own. It holds
the register offsets and bits, the descriptor formats, the DMA region's layout, the whole receive
and transmit ring logic behind two small traits (`Region` and `Tails`), and the I219 bring-up
sequences ported from FreeBSD's Intel shared code. You supply the volatile register accesses and
the DMA memory; everything computed from them is here, `no_std`, and tested on the host against a
simulated device. It was written for the nife kernel.

## What is proven

Three Kani harnesses cover the input a device or a caller controls:

- `proofs::a_delivered_frame_always_fits_the_buffer_and_the_room`: for every status word a device
  can write and every caller buffer size, a frame that `receive_verdict` delivers fits both the
  receive buffer and the caller's space.
- `proofs::every_ring_offset_is_inside_the_region`: for any index, every descriptor and buffer
  offset lies inside its part of the DMA region.
- `pch::physical_layer_device::proofs::a_paged_access_selects_the_offsets_own_page_and_register`:
  every paged PHY access names a PHY address that exists, the offset's own register and the offset's
  own page.

Rerun them with `cargo kani`. Each harness has a falsification record at
`falsifications/<module.path>.<harness>.patch`; applying it with `patch -p3` from the crate root
makes that harness fail at its own assertion, which shows the proof checks something.

The NVM reads, the reset, ULP and PHY workaround sequences, and transmit descriptor status are
tested against the simulated device but not model checked.

## Usage

Claim a device, then judge each receive descriptor's status word before trusting its length:

```rust
use e1000e::{
    RECEIVE_STATUS_DD, RECEIVE_STATUS_EOP, ReceiveVerdict, VENDOR_INTEL, is_supported, model,
    receive_verdict,
};

assert!(is_supported(VENDOR_INTEL, 0x10d3));
assert_eq!(model(0x10d3), Some("82574L (QEMU's e1000e)"));

let word = |len: u64, status: u8| len | (status as u64) << 32;
let done = RECEIVE_STATUS_DD | RECEIVE_STATUS_EOP;
assert_eq!(receive_verdict(word(60, done), 1514), ReceiveVerdict::Frame(60));
assert_eq!(receive_verdict(word(60, 0), 1514), ReceiveVerdict::NotYet);
assert_eq!(receive_verdict(word(0xffff, done), 1514), ReceiveVerdict::Drop);
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
- The API may still change before 1.0.

The minimum supported Rust version is 1.88.

## Sources

Offsets, bits, descriptor layouts and device ids were read, not recalled, on 2026-10-04 (UTC). The
sources were Linux's `drivers/net/ethernet/intel/e1000e/regs.h`, `defines.h` and `hw.h`. They were
cross-checked the same day against FreeBSD's BSD-licensed `sys/dev/e1000/`: `e1000_regs.h`, and
`e1000_api.c`'s SPT class, which holds every I219 row of `DEVICE_IDS` and two Comet Lake ids
(`0x0d53`, `0x0d55`) the crate does not claim. No Linux code was copied.

QEMU's `hw/net/e1000e_core.c` and `hw/net/e1000x_common.c` say what the emulated 82574L does with
them. QEMU uses legacy
descriptors unless `RFCTL.EXTEN` is set, and it receives only with `RCTL.EN`, link up and bus
mastering.

Intel's own I219 datasheet was not read. Where the I219 is believed to differ from the 82574L, the
item says so.

The I219 needs a port rather than a reading of registers. It is a MAC inside the platform
controller hub, talking to a separate PHY that it shares with the Management Engine, and it
carries workarounds that only field exposure finds. FreeBSD's Intel shared code (BSD-3-Clause) and its
`if_em.c` and `if_em.h` (BSD-2-Clause) are the only sources read for `pch`; Linux's `e1000e` is GPL and was not consulted for any of it. Each
item in `pch` names the FreeBSD function it follows. Everything was read from `sys/dev/e1000/` on
FreeBSD's `main`: `ulp`, `flush` and `reset` on 2026-10-04, the rest on 2026-10-05. The last
commit touching `e1000_ich8lan.c` was then 46cf612d (2026-08-30).

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option. The
files under `src/pch/` adapt FreeBSD's Intel shared code and `if_em.c` and also carry Intel's BSD-3-Clause
notice and the BSD-2-Clause notice of `if_em.c` and `if_em.h` (Intel, Nicole Graziano, Kevin Bowling),
which a binary redistribution must reproduce. Their SPDX expression is `BSD-2-Clause AND BSD-3-Clause`.
