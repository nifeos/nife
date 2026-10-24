# The block server, and how it completes a request

An appendix to [notes/fs-server.md](../fs-server.md). It holds why the block server moves whole
blocks, how it waits for a completion, and the disk-order hazard in the QEMU runners.

RedoxFS reads a lot at open. It scans a 256-entry header ring for crash consistency
(`redoxfs::HEADER_RING`), so a mount is hundreds of block reads before it serves anything. Two
choices keep that inside the test's watchdog.

## One request moves whole blocks

A whole filesystem block goes in each virtio request, and since milestone 138 (close the read gap:
a 4 KiB request must stop moving 128 KiB) step 4, up to sixteen of them in one. The block server's
DMA region is `1 + blk::TRANSFER_BLOCKS` contiguous pages. Page 0 holds the rings, request header and
status. The rest is the data buffer, which IS the region shared with the FS server: one page, one
block, through step 3; sixteen pages, up to sixteen blocks, since. So one request moves one or more
whole blocks, and the device DMAs them straight into the FS server's region as a single descriptor,
with no per-sector loop and no copy. The driver roles of milestone 9 (a virtio-blk driver at EL0,
and an interrupt becomes a message) still transfer one 512-byte sector at a time; this role does not.
This is what keeps the mount's read count in the low hundreds rather than thousands. The block
server's other clients, `disk_surveyor` and `disk_partitioner`, map only the first data page and
send only single-block requests, so the region's growth leaves them unchanged.

## It waits on the completion interrupt

This is the milestone-9 discipline (`complete_blk`, `crates/virtio`). The kernel turns the device's
completion IRQ into a message on the block server's `Irq` endpoint. The server WAITs for it, quiets
the device, ACKs the line, and lets `used.idx` decide when the completion is really its own. A
wakeup can be stale or coalesced. It is the same loop the read driver uses.

This is a correction (fix/irq-delivery, 2026-07-29). The earlier note here claimed interrupt-driven
completion "overran the watchdog" and forced a poll of the used ring. It does not. Booted with the
WAIT path, the redoxfs_server test passes on both ISAs at the 4-core SMP boot (aarch64 141 tests,
riscv64 84 tests). The mount's hundreds of WAIT-driven completions all land well inside the 60 s
watchdog. The completion IRQ reaches the block server's endpoint exactly as it reaches any
milestone-9 driver's. The whole-block reads above are what keep the reschedule count affordable.

The prior "hangs on the first read" report did not reproduce; the machine overruled the note. QEMU
still completes synchronously inside the `QUEUE_NOTIFY` write (notes/dma.md). So the interrupt is
already pending by the time the server WAITs. The kernel's pending-signal count makes that WAIT return at
once, rather than block on an event already over. (The text this moved from credited that count to a
lettered decision 9a. No decision file by that number exists, so the citation is dropped here, on
2026-10-11 (UTC).)

## The disk order is a real hazard

QEMU's `virt` assigns virtio-mmio devices to slots in reverse command-line order, and the kernel
finds block devices by ascending slot. So the runners place the nifefs disk LAST on the command line,
to keep it at slot 0; the phase-1 driver tests use `find_block_device`. That leaves the RedoxFS disk
at the next slot for `find_block_device_n(1)`. Getting this backwards silently hands the phase-1
tests the wrong disk, and the runner comments say so.
