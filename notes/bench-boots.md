# The bench boots: what each prints, and why it prints it

*Name: provisional. calef names notes.*

A bench boot is a kernel feature that replaces the hand-over to the progenitor with one experiment
on a real machine, prints what it found and halts. Each prefixes every line with its own word, so a
photograph of the screen and a serial capture are read the same way. Each runs the measured-boot
chain itself through `trust::require_program`, because it replaces the progenitor that would have:
the archive's table must be the one the kernel image vouches for, and each program's bytes must hash
to its entry. That is also what keeps `TRUST_ROOT` in a release image, and so what `uefi_loader`'s
seal finds. Which machine to take an evening on is [bench-runbook.md](bench-runbook.md).

## `disk_throughput`: fatal risk 6 on xenon

`kernel/src/disk_throughput.rs`, for risk 6 of `design/fatal-risks/README.md` and
[risk-6-bench-evening.md](risk-6-bench-evening.md). The experiment is one real, non-virtio device on
real silicon, confined, at throughput. Two things have to be true on the night that no code can make
true: the VT-d unit this kernel programs must own the NVMe's requester id, and the namespace's LBA
size must give `blocks_per` in `1..=8`. Either failing could arrive as something that looked like a
result, a throughput number from a device nothing was translating or a test that said "skipped". So
the boot prints both verdicts before it measures, and one verdict line whose first word cannot read
as a pass unless both held. Every line starts `disk-throughput:`.

1. `preflight 1/2 dmar scope : PASS|FAIL`, with which unit owns the controller and how.
2. `preflight 2/2 lba size   : PASS|FAIL`, with the LBA size and `blocks_per`.
3. `ipc floor`, `write`, `flush` and `read`, each with its count and its time.
4. `verdict CONFINED-AT-RATE | UNCONFINED | SKIPPED | FAILED`, and nothing after it.

The client is the kernel's boot thread, playing a client process. Every block is one `CALL` into the
EL0 server, which builds one NVMe command, rings the doorbell, polls the completion's phase tag and
replies. So a figure is one command in flight, 4096 bytes per command, polled, plus two context
switches per block. That is a lower bound on the device and an honest measure of this driver, not
comparable to `fio` at queue depth 32. The `ipc floor` line, the same path with no device on it,
lets whoever quotes the figure subtract the IPC share. The write phase stamps each block's first and
last words with its number, and the read phase clobbers the stamp first, so a read that never
reached the device fails rather than passing on what the write left. A debug build says `DEBUG` on
its line, because its figures understate the driver.

BUGS:

- It writes: `WINDOW_BLOCKS` blocks from 1 MiB into the namespace are overwritten and read back.
  On any machine but a wiped xenon it destroys data, which is why no gate builds the feature and why
  the verdict names the window it wrote.
- The client is in the kernel, so the figure leaves out the cost of a client that is itself a
  process. The server, which risk 6 is about, is the same process either way.
- Sequential only, one pass, no warm-up and no repeats. A second boot is the repeat.

## `network_bench`: the NIC on xenon, and the port on radon

`kernel/src/network_bench.rs`, for exit criterion 2 of milestone 494 (a driver for the network card
a PC actually has). It asks for the PCI survey line naming the card, a DHCP lease from the house
router, and a measured transfer from a host on the LAN, photographed. The tour prints the survey.
This boot runs the other two through exactly the code the QEMU gates prove (`e1000e_service`,
`net_stack`, `socket_test_client`) and prints them in the order they can fail, every line
`network-bench:`.

1. `nic`: the device id and model, its requester id, whether the link came up, the MAC address.
2. `phy`: the PHY's identifier, read over MDIO at bring-up (`0x01410cb0` under QEMU; on an I219 the
   bring-up's own `e1000e:` lines come first).
3. `preflight dmar scope : PASS|FAIL`, which IOMMU unit owns the NIC, risk 6's condition applied to
   this device.
4. `dhcp`: the leased address, after a line saying it is waiting, so a boot with no lease ends on
   that line rather than on silence.
5. `transfer`: bytes, time and rate from the peer the image was built for.
6. `verdict LEASED-AND-MEASURED | UNCONFINED | NO-PEER | FAILED`, then `done, halting.`

The number is one TCP stream into `net_stack` through a 16 KiB window, received one `RECEIVE` per
chunk, with a polled NIC watched while frames flow (`socket_contract::polled_wait`). It measures
this stack as built, not the I219 or radon's port: the window bounds the rate to 16 KiB per round
trip. It is what a package download gets today.

A riscv64 machine whose tree names a JH7110 Ethernet port runs the same boot over
`designware_ethernet_service`, with the bring-up's own account first. That account is the clock and
reset words before and after, the syscon select, the controller and PHY identities, where the
station address came from, and both reset times. Then come the coherence probe's verdict with the
words it was judged on, and the link. Its preflight line is `coherence` where xenon's is `dmar
scope`; each is the one condition that decides whether the numbers after it mean anything there.
[designware-ethernet.md](designware-ethernet.md) has the runbook.

BUGS:

- The DHCP wait has no bound. `net_stack` retries DISCOVER forever, and the kernel blocks on its
  report, so a boot with no lease ends on the `waiting` line, which is the diagnosis.
- The peer is baked into the image (`NIFE_NETWORK_BENCH_PEER`), because the boot has no way to be
  told one. A different peer is a rebuild.

## `storage_bench`: radon's SD/MMC controller

`kernel/src/storage_bench.rs`, riscv64 only. Nothing emulates radon's SD/MMC controller, so a bench
boot is the driver's first meeting with the part, and it has to say everything at once. For the
microSD slot (slot 1) and then the eMMC socket (slot 0), every line `storage-bench:`:

1. The slot as the tree describes it.
2. Its clocks and reset as found, walked through `SDIO_BRING_UP`, every step idempotent.
3. The controller's identity (`VERID`, `HCON`, the FIFO, `CDETECT`) and the five registers the
   firmware's driver last wrote, read before anything is written.
4. The read-only probe (`designware_mobile_storage::bench::read_only`): identification, the CID, the
   capacity, sector 0's partition table, the first partition's boot sector, and a timed 8 MiB
   sequential read from the start of that partition.
5. Only on the microSD slot, and only in a build made with `NIFE_STORAGE_BENCH_WRITE=scratch`, the
   scratch write test: up to eight sectors before the first partition, read, overwritten with a
   pattern, read back, restored, read back.
6. The same reads through the IDMAC, and in a write build the write test.
7. On the microSD slot, the same card through the EL0 block server, called the way the FS server
   calls one. In a read-only build the window is the first partition: block 0 must be the boot
   sector, `WRITE` must be `EROFS` and a block past the window `EINVAL`. In a write build it is the
   scratch range, and one block is read, overwritten, read back and restored.

Then one verdict line about the microSD slot, and a halt. The design of the driver is
[designware-mobile-storage.md](designware-mobile-storage.md).

BUGS:

- The booted system has no block device on radon yet. This boot proves the EL0 server on a window
  it picks for the test; which window the booted system serves is an architect's call, in milestone
  53's block.
- It re-initializes the card U-Boot booted from. `CMD0` puts the card back to idle, which is
  harmless (the next power-on does the same) and means the boot cannot hand the card back to
  anything that expected it selected, so it halts.
- The eMMC socket is probed read-only, always. Whether radon has a module fitted is a bench fact the
  boot reports, and nothing writes to one.

## `xhci_port_probe`: what an xHCI's ports read, and when

`kernel/src/xhci_port_probe.rs`, an instrument for one question about xenon: the driver's scan
reported `0 of 26 ports in use` with a keyboard and the boot stick plugged in. Was nothing
connected, or was the scan early? It prints `xhci-probe:` lines at two moments.

1. Before the firmware handoff: the controller as the firmware left it (`USBCMD`, `USBSTS`,
   `HCCPARAMS1`, `USBLEGSUP`, `USBLEGCTLSTS`), which ports are USB 2 and which USB 3 (the Supported
   Protocol capabilities), and every port's `PORTSC`.
2. After the driver's bring-up report, which follows its reset and scan: every port's `PORTSC` at
   once, then each port whose value moved, at fixed offsets out to five seconds, with `USBSTS` and
   interrupter 0's `IMAN` and `ERDP` beside them.

It only reads. A `PORTSC` read has no side effect, so the driver running beside it sees the
controller it would have seen anyway. The boot stalls about five seconds in the second step. The
driver itself is [usb.md](usb.md).
