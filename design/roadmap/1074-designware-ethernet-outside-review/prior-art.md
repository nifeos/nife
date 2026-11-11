# Prior art for the designware_ethernet crate

An appendix to [the milestone](../1074-designware-ethernet-outside-review.md).

How the crate sits against what a driver author already knows. Recent downloads are the last 90
days at the time of the review; figures marked "not fetched" come from reading those sources
earlier and were not checked for this review.

| source | kind | rings and polling | coherence | what a reader coming from it will miss here |
|---|---|---|---|---|
| Linux `stmmac` (`dwmac4`) | kernel driver, the reference for this silicon | interrupts and NAPI with a per-poll budget; ring sizes in the hundreds (not fetched) | assumed from the device tree (`dma-noncoherent` or its absence) and the architecture's DMA API | interrupts, more than one queue, offloads, jumbo frames, multicast, phylink, the MDIO clock range read from the clock framework |
| OpenBSD `dwqe` and `ytphy` | the crate's source | interrupt driven, ring tail written per fill; 30,000 by 10 µs reset poll, 2,000 by 10 µs MDIO poll, which the crate keeps | none: the bus_dma layer syncs | the receive buffer size inherited from reset (the crate writes it), jumbo frames (the crate refuses them) |
| FreeBSD `eqos`, NetBSD `eqos`, FreeBSD `mcommphy` | the cross-checks the crate names | interrupt driven | bus_dma | same as OpenBSD |
| U-Boot `dwc_eth_qos` | boot-time driver, the source of the receive buffer size field | polled, a handful of descriptors | flushes and invalidates around every descriptor | nothing: closest in shape, but written for one short session |
| `smoltcp` 0.14.0, 2.82M recent / 10.1M all-time | the stack this crate feeds | `phy::Device` with `RxToken` and `TxToken` | the device's business | a token that hands the frame to a closure instead of a copy into the caller's buffer (item 13) |
| `stm32-eth` 0.8.2, 17.3k / 41.9k | Rust driver for the older DesignWare MAC (3.x descriptors) on STM32 | polled or interrupt, static rings, `smoltcp` integration | none needed on those cores | `smoltcp` `Device` implemented in the crate; the descriptor format differs |
| `embassy-stm32` 0.6.0, 191k / 822k | async HAL with an Ethernet driver for the same STM32 MACs | async with interrupts | none needed | async, interrupt waker |
| `dwmac-my` 0.2.0, 9 / 35 | Rust DWMAC driver for the StarFive VisionFive 2, the same board | polled | its ArceOS HAL flushes descriptors through the L2 cache (from `notes/designware-ethernet.md`, not fetched) | a whole driver, not logic behind traits |

Where this crate differs: it dereferences no pointer and touches no hardware; register access, the
DMA region and the two tail pointers come through three traits, so every sequence runs on a host
against a simulated controller. It measures DMA coherence once per bring-up with a MAC loopback and
names the direction of any failure, where every other driver here assumes or always flushes. It
bounds every wait. It writes the receive buffer size that OpenBSD inherits. It refuses jumbo frames
in the MAC rather than dropping them from the ring. Six machine-checked proofs ship with the patch
that makes each fail. What it costs: one queue each way, polled, no interrupts, no offloads, no
multicast, a copy per frame, and the smaller surface in the last column.

A concrete example: a receive descriptor whose word 3 reads first, last and a packet length of
`0x7fff`. `receive_verdict` judges it `Drop`, and the harness proves that for every word the device
can write. A driver that trusted the field would copy 32,763 bytes out of a 2,048-byte buffer.
