# Both directions: block writes, kill-mid-write, and the receive queue

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/dma.md](../dma.md), moved there on 2026-10-11 (UTC) under §212 (a prose
budget). It records why the validator needed no change for the
write direction (milestone 32 (a real filesystem) phase 1) or the receive queue (milestone 30 (the
network stack as a confined component)), and what the kill-mid-write test found.

## The write direction (milestone 32 phase 1)

The write-capable block path needed no kernel change. It is worth recording why that is a
property and not luck: the validator bounds *addresses*, never directions. A blk read marks the
data descriptor device-writable (the device fills the buffer). A blk write leaves the flag clear
(the device consumes the buffer). Either way `validate_and_shadow` checks the same thing, that
`addr..addr+len` stays inside the driver's region. So both hazards are closed by one check. A read
descriptor aimed at kernel memory would let the device *overwrite* the kernel. A write descriptor
aimed there would let it *exfiltrate* kernel memory to disk. Neither address ever reaches the
device. The roadmap's claim that the transport "already speaks both directions" was verified
against the code and is accurate.

Three things were new. The driver's write verb is `crates/virtio`'s `write_block`, one flag away
from the read. QEMU's disks are attached writable, one image file per transport, because QEMU
write-locks a file once any attachment can write (see the runners). And there are the tests. They
run the same matrix as the read path: both ISAs, both transports (mmio and PCIe). Each does a full
pattern round trip through a wiped buffer. A re-check of the superblock and directory catches a
write that strayed off its own block.

Kill-mid-write. Errors here eat filesystems, so the teardown case is tested on both ISAs. A
driver submits a validated write and dies (panic, kill, reap) without collecting the completion or
acknowledging the interrupt. Then a fresh driver resets the same device and must complete its own
round trip. Three facts make the abandoned request harmless, and all three are load-bearing:

- The dead driver's DMA frame is never reclaimed. A `Spawn`-mapped page goes through
  `map_physical`, whose contract is "Drop leaves it alone". So the in-flight completion (the used
  ring entry, the status byte) lands in a frame the allocator never re-issued. This is currently a
  deliberate leak, one frame per spawned driver. The caveat for whoever builds DMA-region
  reclaim later is this. Freeing that frame on thread death would hand the device a landing zone the
  allocator may already have re-issued to someone else. That is a use-after-free performed by
  hardware. Reclaim must quiesce the device first (device reset, then confirm no requests are
  outstanding), and nothing enforces that today except this note and the test.
- The kernel's transport state carries nothing in-flight. `Device` tracks `last_avail`, a
  count of validated submissions, not completions. So an uncollected completion leaves nothing
  dangling, and the next operator of the physical device resets it (status 0) and programs its own
  registration's rings from scratch.
- The completion is the used ring, not the interrupt. This one was missing at first, and the
  test caught it. The kill-mid-write case failed intermittently with `report_code(0xE3)` ("woke, but
  the device did not complete the request"), roughly one run in two. The abandoned write's
  completion still raises the device's interrupt line, and the kernel turns that into a pending
  signal on the interrupt's routing endpoint. That endpoint is shared across every driver of the
  same physical line. So the *survivor's* first `WAIT` consumed the dead driver's stale completion
  signal, and then found its own request not yet on the used ring. The read path never noticed
  because it only ever expects one completion. The fix is the correct virtio discipline anyway. A
  driver treats the used ring advancing, not a single wakeup, as the completion. So
  `complete_block` (`crates/virtio`) now loops WAIT/ack until `used.idx` moves, discarding a
  wakeup that was really someone else's. Every real completion also raises the line, so the loop
  always makes progress. This hardens the read path too: coalesced and spurious interrupts were
  always possible and were only tolerated by luck before.

The riscv half of this test also exposed that the riscv trap path could not kill a
faulting user thread at all. It stepped over a U-mode `ebreak` and kernel-panicked on any other
U-mode fault, behind a comment stale since milestone 20 (a portable HAL). Fixed alongside; see
notes/riscv-parity-scope.md.

## Multiple queues, and the receive direction (milestone 30)

The disk uses one virtqueue. A NIC uses two: receive (queue 0) and transmit (queue 1). virtio-net is
the reason the confinement grew a second queue, and receive is the reason it is worth stating what
"a second direction" does and does not mean.

The plumbing that is new. Each device now carries a per-queue last-validated index and a
per-queue ring block. `setup_queue(id, num, queue)` and `notify(id, queue)` take a queue number. Queue `q`'s
descriptor table, available ring, and used ring sit at `q * RING_BLOCK` (0x200). That holds in both
the driver's DMA region and the one kernel-private shadow frame (two queues fit in 0x400 of a 4 KiB
frame, asserted at compile time). Queue 0's offsets are exactly the old single-queue layout, so the
disk driver did not change. Its data buffers already start at 0x200, which is queue 1's block, and a
disk has no queue 1. The `Virtio` capability's methods grew a queue argument rather than gaining new
methods. The disk passes queue 0 and its ABI is byte-identical (DECISIONS §23 (multi-queue DMA confinement)).

The validation that is NOT new, and why that is the honest finding. `validate_and_shadow` did not
change. It bounds the *address* of every descriptor, `addr..addr+len` inside the driver's region,
whichever way the device moves the bytes. Receive is where the *device writes into* driver memory
(the driver posts an empty buffer, the device fills it with a packet). That is exactly the shape
milestone 32's block write already leaned on: the validator "bounds addresses, never directions." A
receive descriptor aimed at kernel memory would let the device *overwrite* the kernel with an inbound
packet. A transmit descriptor aimed there would let it *exfiltrate* kernel memory onto the wire. Both
are refused by the one in-region check, before the device is ever rung. So the "second direction" is
proved, not implemented. `the_validator_refuses_an_rx_descriptor_that_escapes_the_region` sets the
device-writable flag and asserts the refusal is about the address, not the flag.
`a_second_queue_validates_on_its_own_block` proves queue 1 validates on its own block without
touching queue 0's shadow. Both run on both ISAs.

The used ring for the receive queue needs no extra confinement either. The device writes the
received length into `used.ring[i].len` and the packet bytes into the descriptor buffer. Both landing
zones are the driver's own in-region memory (the used ring is placed by the kernel; the buffer is
address-checked). The device never writes an *address* anywhere the driver chose, which is the whole
invariant.
