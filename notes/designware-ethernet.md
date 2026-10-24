# radon's Ethernet: the JH7110's DesignWare controller

*(Milestone 53 (the board's own peripherals: network and storage on real silicon), its network
half. Every name on this page is provisional: the crate `designware_ethernet`, the kernel module
of the same name, `designware_ethernet_service`, `designware_ethernet_transport`, and this page's
stem. An architect names things.)*

radon's two ports are Synopsys DesignWare Ethernet QoS controllers, `snps,dwmac-5.20`, at
`0x1603_0000` and `0x1604_0000`, each behind a Motorcomm YT8531 PHY. This driver brings up the
first, which is the one U-Boot netboots radon over, and hands it to `net_stack` the way milestone
494 (a driver for the network card a PC actually has) hands it an `e1000e`. It has leased an
address and moved 100 MiB on radon (Results, below), and the booted system brings the port up.
The second port is proposed in pull request #2470 (radon's second Ethernet port, `gmac1`).

## How it is split

| piece | where | what it does |
|---|---|---|
| everything computed | `crates/designware_ethernet` | register map, descriptors, the DMA region's layout, the handoff, ring logic, MDIO, the YT8531's board settings, the controller sequences, the coherence probe and its verdict, the device-tree query. Host-tested against a simulated controller; two Kani harnesses |
| the clocks | `crates/jh7110_clock_and_reset` | the AON domain, a parent-select step, and `gmac0`'s two plans, walked by the existing `drivers/jh7110_clock_and_reset.rs` |
| control plane | `kernel/src/designware_ethernet.rs` | clocks and resets, RGMII select, PHY, reset, the coherence probe, rings, link, start |
| what the process is handed | `kernel/src/user/designware_ethernet_service.rs` | the DMA region and the controller's DMA page; not the MAC page |
| data plane | `components/src/designware_ethernet_transport.rs`, inside `net_stack` | posts receive buffers, copies frames, moves the two tail pointers, polls |
| the bench boot | `kernel/src/network_bench.rs`'s `radon` arm | the bring-up's whole account, then a lease and a measured transfer |

Reuse: adapted from OpenBSD's `dwqe` (`sys/dev/ic/dwqe.c`, `dwqereg.h`, `sys/dev/fdt/if_dwqe_fdt.c`
at `59ec3ab4d107`, 2026-07-31) and `ytphy` (`sys/dev/mii/ytphy.c` at `9d7a05f92003`), ISC licensed,
with the notice carried in the crate root. FreeBSD's `if_eqos_starfive.c` and `mcommphy.c`
(BSD-2-Clause) also support this chip and were read as an independent cross-check; OpenBSD's was
adapted because its glue handles `starfive,tx-use-rgmii-clk`, radon's v1.3B transmit clock
arrangement, and FreeBSD's does not. Linux's stmmac and `dwmac-starfive.c`, U-Boot's `dwc_eth_qos*.c`, both JH7110 device
trees and the vendor U-Boot's board code are GPL and were read only for facts about the hardware.

## Is radon's DMA coherent? Yes, by measurement

Measured on radon, 2026-10-20 (UTC), over three boots: `Coherent` every time. The line was
`coherence : Coherent (rx des3 0x30010084, tx des3 0x30000000, channel status 0x00000445, payload
Matches)` in all three logs under `bench/radon-2026-10-20/`, including the two whose link never
came up (the probe runs in MAC loopback, before the link wait). The device read descriptors and a
payload the CPU had only cached, and the CPU read the device's writes over lines it held cached,
byte for byte. This is what milestone 655 (DMA on a non-coherent RISC-V machine)'s claim that
every riscv64 machine nife has run on keeps DMA coherent rests on for radon.

What the sources said before the measurement, read 2026-10-06 (UTC), all of which it agrees with
except the one practice that disagreed:

- Mainline Linux's `arch/riscv/Kconfig` selects `ARCH_DMA_DEFAULT_COHERENT`, so a RISC-V device
  is coherent unless its tree node or an ancestor says `dma-noncoherent`.
- Mainline's `arch/riscv/boot/dts/starfive/jh7110.dtsi` says `dma-noncoherent` nowhere. Its
  predecessor's `jh7100.dtsi` says it on `/soc`, which is the JH7100's well-known non-coherence
  written where the kernel acts on it.
- The vendor U-Boot's `jh7110.dtsi` (the tree radon actually hands over) says it nowhere either,
  and marks its USB controller `dma-coherent`.
- OpenBSD's riscv64 `simplebus.c` honors both properties per node, and its `dwqe` drives this
  controller with no JH7110-specific cache maintenance.
- One data point leans the other way, and is recorded rather than smoothed over. `dwmac-my`, a
  Rust driver for this board on crates.io (0.2.0), comments "jh7110 does not need to invalidate
  dcache" on its receive path, yet its ArceOS HAL (`elliott10/arceos` commit `2360a3dce275`)
  flushes descriptors through the L2 cache's `flush64` register (`0x0201_0200`) before handing
  them to the device. Whether that flush was needed or defensive is not stated; U-Boot flushes
  around every DMA on every board as a matter of course, which is a plausible origin.
- The interconnect itself was not read: StarFive's JH7110 technical reference was not consulted,
  and a claim about the SiFive coherence manager's front port would be recall. Marked as such.

Sources agreeing on what a tree says are not the bus, and one practice disagrees. So the bring-up
measures it, once, before any descriptor is trusted (`designware_ethernet::coherence`). It puts the
MAC in loopback and uses ordinary cached stores, with only the `fence` the data plane already uses.
It posts the receive ring and fills receive buffer 0 with a poison byte. Then it reads that buffer
back, so a cache the device does not snoop is holding the poison. It queues one 128-byte frame and
waits up to 100 ms. Last, it compares the CPU's view of the ring with the controller's own status
register, read over MMIO:

| verdict | what it means | driver does |
|---|---|---|
| `Coherent` | the device read what the CPU only cached, and the CPU read what the device wrote over cached lines, byte for byte | proceeds |
| `DeviceCannotSeeCpuWrites` | the device reported descriptors unavailable without completing them | refuses the port |
| `CpuCannotSeeDeviceWrites` | the device says received; the CPU still sees the descriptor as the device's | refuses the port |
| `ReceivedPayloadStale` / `TransmittedPayloadStale` | descriptors agree and data does not | refuses the port |
| `BusError` | the DMA hit an address the interconnect refused | refuses the port |
| `Inconclusive` | the loopback never ran | proceeds, and says so on every line that depends on it |

The simulation models two kinds of non-coherent cache: a write-back cache the device does not
snoop, and a write-through cache it does not invalidate. The host tests show the probe names the
right direction for each, and reads a coherent machine as `Coherent`.

### What that means for milestone 655, as a proposal

655 waits on milestone 89 (Scaleway EM-RV1: a second RISC-V implementation, rented) because the
TH1520 is the first non-coherent machine in reach. radon reads `Coherent`, so radon cannot stand
in for the TH1520: there is nothing to maintain, and 655 keeps its dependency on 89. The
`flush64` flush in the ArceOS HAL above was not needed on this board. What radon gives 655 is the
instrument. The probe tells the two directions of non-coherence apart, and the TH1520's
`snps,dwmac-3.70a` is the older GMAC generation, so its descriptors differ, but the
loopback-and-compare method carries over unchanged.

The proposal, for whoever next edits 655 (a lane edits only its own milestone's block): record
radon as coherent by this measurement, citing `bench/radon-2026-10-20/network-bench-3.log`, and
cite this probe as the method its "measured on the machine, not assumed" sentence asks for.

The same measurement unblocks the storage half's IDMAC data path (notes/designware-mobile-storage.md):
on radon, a DMA descriptor ring needs only the `fence` this driver already uses.

## The confinement, stated plainly

The controller's channel registers sit on its second page, `0x1000` to `0x1fff`: both tail
pointers, both ring bases, and the software reset bit for the whole controller. The JH7110 has no
IOMMU in front of this device. So the process that moves the tails can point the device's DMA
anywhere and reset the MAC; it is as confined as its arithmetic, milestone 261 (the NVMe driver
leaves the kernel)'s position on a machine with no IOMMU. What the split still keeps from it is the
first page: the MAC configuration, the packet filter, the station address and the MDIO bus, so it
cannot reach the PHY, change its address or receive promiscuously.

## calef's bench step on radon

About ten minutes, and nothing on the card changes: radon's card was written once with `--tftp`
(milestone 257 (boot radon over the network)). This boot halts when it is done, so the last step
puts the ordinary image back.

On the Mac radon's UART goes into, in this branch's worktree, with `P` set to its address on the
bench LAN (`P=$(ipconfig getifaddr en0)`). In a second terminal, the peer, which serves 100 MiB per
connection (allow `nc` if macOS asks):

```sh
while true; do head -c 104857600 /dev/zero | nc -l 9494; done
```

In a third, the TFTP server, left running: `script/board-netboot`.

Then build, and capture while you power-cycle radon (its own outlet, or its USB-C lead; never another
outlet, and never a USB hub; which outlet is which is kept off-tree by the maintainers):

```sh
git log -1 --format=%h
NIFE_NETWORK_BENCH_PEER="$P:9494" script/board-image --tftp --extra-features network_bench
mkdir -p bench/radon-$(date -u +%F)
script/board-console --for 3m --until none --log bench/radon-$(date -u +%F)/network-bench.log
```

The Ethernet cable must be in `ethernet@16030000`'s port, the one U-Boot already netboots over.
The U-Boot banner does not say which jack that is; on a boot where U-Boot reports
`ethernet@16030000` timing out and links on `ethernet@16040000` instead, the cable is in the
wrong jack and this boot will end `FAILED: no link`. `nife: payload came from net` near the top
says the new image was served.

radon's card carries a boot script written 2026-09-16 that asks for a server address that is not the
serving Mac's, and for the archive under the name `nife-initrd.img`, where `script/board-image`
writes `nife-initial-ramdisk.img`. Until the card is rewritten, press a key at the autoboot
countdown, type `setenv nife_boot_server <the serving Mac's address>` and `boot`, and serve a copy
of the archive under the old name beside the new one in `target/board`. Then put the ordinary image
back for everyone else's bench work: `script/board-image --tftp` and one more power cycle.

Success looks like this (values vary; the shape does not):

```text
network-bench: nic       : JH7110 port at 0x16030000 (starfive,jh7110-eqos-5.20), clock and syscon windows from the tree
network-bench: syscon    : 0x........ -> 0x........ (gmac0 interface 1 before, 1 after; 1 is RGMII)
network-bench: mac core  : version 0x......52, NN-bit DMA
network-bench: phy       : id 0x4f51e91b, 1800 mV I/O, settings from VendorTree
network-bench: address   : 6c:cf:39:00:47:2c from Tree
network-bench: coherence : Coherent (rx des3 0x30000084, ...)
network-bench: link      : 1000 Mbit/s full duplex, NNNN ms after autonegotiation restarted
network-bench: dhcp      : leased 192.168.8.x
network-bench: transfer  : N bytes in T s = R Mbit/s
network-bench: verdict LEASED-AND-MEASURED
```

The version's low byte (`0x52`, the 5.20 both trees name), the PHY's identifier and the vendor
settings source are expectations from the sources. The DMA width, the I/O voltage and the address
source are predicted by nothing read, so whatever they say is the first record of them. A value
other than expected that still leases is a finding to record, not a failure.

Each way it can end, by the last lines:

| what the log shows | meaning | next |
|---|---|---|
| `verdict LEASED-AND-MEASURED` with `coherence : Coherent` | the driver works, and radon's DMA is coherent by measurement | fill the Results row; a lane flips `PROVEN_ON_SILICON` citing it, and records the coherence result in 655 |
| `INCONCLUSIVE-COHERENCE` | it leased, so DMA works in practice, but the loopback never ran | record it; the probe needs a second method (PHY loopback), a lane |
| `refused at bring-up: NotCoherent(..)` | radon is not coherent, in the direction named | stop: 655's proposal above applies, and this is a finding for fatal risk 6 |
| `refused at bring-up: ClocksNotRunning` or `ResetsHeld` | the clock words did not read back | the `clocks` and `resets` lines say which; a lane |
| `refused at bring-up: Silent { version: .. }` | the controller's window reads as nothing | clocks or resets are not what the plans say; a lane, with the log |
| `refused at bring-up: UnknownPhy { id: .. }` | not a YT8531 at address 0 | record the id; a lane |
| `refused at bring-up: Mdio(Timeout ..)` | the MDIO bus did not answer | a lane, with the log |
| `refused at bring-up: Controller(ResetTimeout)` | the software reset needs the PHY's receive clock and did not get it | check the cable; a lane |
| `FAILED: no link` | the port came up and no link resolved in 5 s | the cable and the port it is in; the `phy` line says the PHY answered |
| leased, then `transfer` slow or `client stopped` | frames move; the TCP path does not | the peer loop and the firewall first, then a lane |
| `dhcp : waiting` as the last line | frames go out and no lease comes back | the transmit clock inversion or the RGMII delays are the first suspects; a lane, with the log |
| no `network-bench:` line at all | a bring-up step hung | the last kernel line says which; power-cycle, put the ordinary image back |

### Results

| date (UTC) | commit | coherence | link | lease | bytes | rate | verdict |
|---|---|---|---|---|---|---|---|
| 2026-10-20 | `290d3ba8b` | `Coherent` | 1000 Mbit/s full duplex, 2830 ms | 192.168.8.200 | 104,857,600 in 362.965 s | 2.311 Mbit/s | `LEASED-AND-MEASURED` |
| 2026-10-22 | `699fcf61a` | `Coherent` | 1000 Mbit/s full duplex, 2880 ms | 192.168.8.200 | 104,857,600 in 58.542 s | 14.329 Mbit/s | `LEASED-AND-MEASURED` |
| 2026-10-22 | `699fcf61a` | `Coherent` | 1000 Mbit/s full duplex, 2880 ms | 192.168.8.200 | 104,857,600 in 58.606 s | 14.313 Mbit/s | `LEASED-AND-MEASURED` |

The row is `bench/radon-2026-10-20/network-bench-3.log`; that directory's `README.md` says how
the boots were run, and why the console was not `script/board-console`. Its two earlier boots had
the cable in `ethernet@16040000`'s jack and ended `FAILED: no link` after 5003 ms, which is the
cabling, not the driver. What the run settled that nothing read had predicted:

- `mac core : version 0x00004152, 40-bit DMA`. The low byte is the expected `0x52`; the 40-bit
  width is the first record of it.
- `phy : id 0x4f51e91b, 1800 mV I/O, settings from Fallback`. The identifier is the YT8531's.
  The tree radon hands nife carries neither the vendor nor the mainline PHY delay properties, so
  the driver used `Settings::FALLBACK_GMAC0`, which leased. The runbook expected `VendorTree`.
- `address : 6c:cf:39:00:47:2c from Tree`: the tree U-Boot hands over carries
  `local-mac-address`.
- The rate is a finding, not a pass: 2.3 Mbit/s on a gigabit link (below).

The 2026-10-22 rows are two boots of one image, `bench/radon-2026-10-22/53-network-rate.log`;
`README-53.md` there says how they ran. Their peer was the TFTP host's wired adapter, where the
2026-10-20 row's was its Wi-Fi, so the two dates do not share a path.

[`the-receive-rate.md`](designware-ethernet/the-receive-rate.md) has why the first row is 2.3
Mbit/s, how QEMU reproduces it, the fix, and what the later rows measured of it.

## BUGS

- **14.3 Mbit/s on a gigabit link.** radon advertises 4 to 5 KiB of its 16 KiB receive window at
  a 2 ms round trip, so the reader on radon drains its buffer slower than the wire fills it.
  [`the-receive-rate.md`](designware-ethernet/the-receive-rate.md) has the measurement; what
  drains it slowly is not yet found.
- **One board.** The Results rows are three boots, on one board, with one cable. Every other test
  runs against the crate's simulation, which is this tree's reading of OpenBSD and the register
  header, not the device; QEMU has no model of this controller.
- **The booted system brings the port up on every radon boot**
  (`designware_ethernet_service::PROVEN_ON_SILICON` is `true`). With no cable in the jack that
  costs the five-second link wait, then `network : NONE`. If a boot ever hangs in the bring-up,
  setting the constant back to `false` leaves the port as U-Boot left it.
- **`gmac0` only.** `gmac1`'s clocks all live in the SYS domain and no plan for them exists; the
  milestone proposed in pull request #2470 is that work.
- The PHY's delays and drive strengths come from `Settings::FALLBACK_GMAC0` on radon, because
  the tree it hands over names neither spelling of them. They leased at 1000 Mbit/s; whether they
  are also behind the slow rate is not excluded.
- **The MAC loopback is the probe's one method.** It ran on radon, so `Inconclusive` has not been
  seen there; a PHY-loopback fallback for a controller where it does not run is not built.
- **Two PHY registers are written that OpenBSD leaves alone**: the YT8531's auto-sleep and its
  receive-clock gating are both turned off, because the controller's software reset needs the
  PHY's receive clock and a board booted with no cable must still reset. The cost is the PHY's idle
  power with no cable.
- **The station address comes from the tree's `local-mac-address`**, which U-Boot's
  `fdt_fixup_ethernet` writes into the tree it boots with; on radon it does (`from Tree`). The
  fallback is the controller's address registers as U-Boot left them, and the `address` line says
  which was used.
- **Polled, one queue each way**, no interrupt, no offloads: `e1000e`'s set of later decisions.
- **The confinement is arithmetic** (above).
