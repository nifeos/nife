# designware_ethernet

The logic of a driver for the Synopsys DesignWare Ethernet QoS controller (dwmac 4.x and 5.x
descriptor format), with the glue for the StarFive JH7110 and its Motorcomm YT8531 PHY. It holds
the register map, the descriptor formats, the DMA region's layout, the receive and transmit rings,
the controller and PHY bring-up sequences, MDIO, and a probe that measures whether device DMA is
coherent with the CPU's caches. It is `no_std` and needs no allocator.

It dereferences no pointer and touches no hardware. You supply register access through the `Hw`
trait, the DMA region through `Region`, and the tail-pointer writes and barrier through `Tails`;
the tests run all of it against a simulated controller. It does not support the older 3.x GMAC.

## What is proven

Two Kani harnesses, in `src/proofs.rs`:

- `a_delivered_frame_always_fits_the_buffer_and_the_room`: for every value a device can write
  into word 3 of a receive descriptor and every caller buffer size, a frame that `rx_verdict`
  accepts fits the caller's buffer, is at most `MAX_FRAME` bytes, and with its CRC fits the
  receive buffer.
- `every_ring_offset_is_inside_the_region`: for every index, each descriptor and buffer offset
  lies inside its own part of the DMA region.

Rerun them with `cargo kani`. Each harness has a falsification record at
`falsifications/<module.path>.<harness>.patch`; applying it with `patch -p3` from the crate root
makes that harness fail at its own assertion, which shows the proof checks something.

The PHY link decode, MDIO sequencing, the PHY configuration writes, the coherence verdict and
controller bring-up have unit tests against the simulator but no proof.

## Usage

`DataPlane` runs the rings over a DMA region and tail-pointer writes you supply. Here the region
is ordinary memory and the tails go nowhere, so nothing answers the frame:

```rust
use designware_ethernet::{DataPlane, Region, Tails, layout};

struct Memory(Vec<u8>);
impl Region for Memory {
    fn read_u64(&self, off: u64) -> u64 {
        let mut b = [0; 8];
        self.read_bytes(off, &mut b);
        u64::from_le_bytes(b)
    }
    fn write_u64(&mut self, off: u64, v: u64) {
        self.write_bytes(off, &v.to_le_bytes());
    }
    fn read_bytes(&self, off: u64, out: &mut [u8]) {
        out.copy_from_slice(&self.0[off as usize..][..out.len()]);
    }
    fn write_bytes(&mut self, off: u64, src: &[u8]) {
        self.0[off as usize..][..src.len()].copy_from_slice(src);
    }
}

struct NoDevice;
impl Tails for NoDevice {
    fn set_rx_tail(&mut self, _addr: u32) {}
    fn set_tx_tail(&mut self, _addr: u32) {}
    fn barrier(&mut self) {}
}

let mut region = Memory(vec![0; layout::BYTES as usize]);
let mut tails = NoDevice;
let mut plane = DataPlane::new(0x4000_0000); // the region's physical address
plane.start(&mut region, &mut tails); // posts the receive buffers
plane.transmit(&mut region, &mut tails, &[0xff; 60]).unwrap();
let mut buf = [0u8; 1514];
assert_eq!(plane.receive(&mut region, &mut tails, &mut buf), None);
```

## Limitations

- The tests run only against the simulator, not against a device.
- One receive and one transmit queue, polled: no interrupts, offloads, jumbo frames or multicast.
- The crate reads no device tree. The `jh7110` module names the compatible strings and node names
  to look for, and `Discovered::from_node`, `mainline_phy` and `vendor_phy` decode the properties
  of a node you found.
- On the JH7110 only `gmac0` (`16030000`) is wired; `gmac1`'s clocks have no plan.
- The API may still change while the driver built on it is finished.
- Minimum Rust version: 1.88.

## License

You may use this crate under the MIT license (LICENSE-MIT) or the Apache License, version 2.0
(LICENSE-APACHE), at your option. Parts are adapted from OpenBSD's `dwqe` and `ytphy` drivers under
the ISC license, whose notice is in `src/lib.rs`.
