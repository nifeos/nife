# Prior art for the e1000e crate

An appendix to [the milestone](../1057-e1000e-outside-review.md).

## Drivers and stacks

| system | what it gives a user | where this crate differs |
|---|---|---|
| Linux `e1000e` | the reference driver: NAPI, interrupt throttling, checksum and segmentation offload, jumbo frames, two queues on the 82574, EEE, PTP, wake-on-LAN, every PCH generation to date | one queue pair, polled, legacy descriptors, two device families; the I219 port reads nothing GPL |
| FreeBSD `em` and the shared code of Intel | the BSD-licensed source of the I219 sequences of this crate, with the link-up half and flow control | ports the attach, reset and init passes; leaves out link-up, flow control, LED init, `GCR`, `WUFC` and `VET` (P1-2, P2-9) |
| QEMU `hw/net/e1000e_core.c` | what the emulated 82574L does with each register; the only device a hosted test can drive | the crate has no QEMU test of its own (P1-8) |
| `ixy.rs` (ixgbe and virtio, user space) | a Rust NIC driver that owns its MMIO and DMA, batches receive and transmit, and measures line rate | this crate owns neither MMIO nor DMA and leaves batching to the caller; it proves its arithmetic instead (P2-4) |
| Redox `e1000d`, Theseus `e1000` | Rust e1000 drivers for older 8254x parts, each bound to the device model of one kernel | this crate claims the e1000e register map and the I219, and binds to nothing |
| smoltcp `phy::Device` | `receive` and `transmit` tokens with a `consume` closure, `capabilities()` with the MTU and checksum policy, no copy into a caller buffer | `DataPlane::receive` copies into the slice of the caller and `transmit` copies from it. An adapter is a few dozen lines: a `Device` whose `RxToken::consume` calls `receive` into a scratch buffer, and whose `TxToken::consume` fills a buffer and then calls `transmit` |
| `embassy-net-driver::Driver` | the same shape plus `link_state`, `hardware_address` and a waker-driven poll | a link-state reader exists only as the `DEVICE_STATUS_LINK_UP` bit, and the MAC address reader is `station_mac_address_from_receive_words`; an adapter would own the polling |

What is unique here: driver logic that is `no_std`, free of dependencies and free of `unsafe`,
whose device-facing arithmetic Kani checks, with each proof arriving falsified. Beside it sits an
I219 bring-up ported from a permissive source, every departure named, with a simulator that replays
it. What a user coming from the table misses most: an example that runs, an adapter to a stack,
interrupts, and a hardware run with a cable.

## Why a register access carries no width in its name

The ruling on P0-1 rests on how existing interfaces split the two kinds of access.

| interface | the width | what it reads |
|---|---|---|
| `core::ptr::read_volatile::<T>` and `write_volatile::<T>` | the type parameter; the name carries none | one location, at the width of its type |
| `designware_ethernet` and `designware_mobile_storage` register-window traits (ratified) | `read` and `write` return and take `u32` | a register, whose width the hardware fixes |
| `byteorder::ReadBytesExt::read_u16`, `read_u32`, `read_u64` | in the name | a byte buffer, where the caller picks the width at each read |
| `DirectMemoryAccessRegion::read_u64` and `write_u64` in this crate | in the name | byte memory the device and the driver share |

A register window reads registers whose width is a fact of the hardware, so the trait signature
states it and the name does not. The width names of byteorder belong to byte-buffer reads, where
the width is a choice, and `DirectMemoryAccessRegion` is that kind of trait. The one 16-bit access
in BAR0 reads one word, so its name says which: `read_flash_status`.
