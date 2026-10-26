# Filesystem throughput against ext4 and APFS

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds milestone 38's measurement, the three
systems, and what is not apples to apples, with the dates, tables and corrections behind them. Name:
ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## Filesystem throughput, and what is honestly comparable (milestone 38 (filesystem throughput), 2026-08-18)

DECISIONS Condition 2 of §34 (RedoxFS is the primary filesystem) asks: does the userspace-server architecture cost throughput once the
device dominates? The FS server had a per-request number since milestone 32 (a real filesystem) built it, `fs_read` at
~204 us. It had no MB/s figure at all, so the objection a microkernel skeptic presses had never met a
measurement.

The answer is no, and it is not close. The confined-server tax is about a microsecond per request
(`relay_rtt`, in [the service-path appendix](service-path.md)). A 4 KiB file operation through this
stack costs one and a half to three and a half milliseconds. The architecture is three and a half
orders of magnitude below the thing under measurement.

A sharper answer replaces it, and it decomposes onto one constant. Every 4 KiB read of an ordinary
file costs 32 block reads, because RedoxFS stores a file in 128 KiB records and reads the record
whole. One block read through our confined block server costs ~46 us, the same as Linux's on the
same device at the same tier. So our block path is at parity, and the entire gap is one design
constant in the store we vendored.

*Correction, 2026-08-18: the ~46 us (46.2 us) per block is an average that charges the per-request
metadata walk to the blocks. [The record-level sweep](record-level-sweep.md) measured the marginal
cost at 39.0 us per block plus a fixed ~208 us per request. The parity claim survives.*

### What one transfer is, and why that is most of the story

A `filesystem_contract` `READ` or `WRITE` moves its payload through the one page the client shares
with the server. So 4096 bytes is the protocol's ceiling per request. Every figure below is a request
rate wearing throughput's clothes, and a system whose client may pass a 64 KiB buffer is answering an
easier question. The ext4 table below prices that, the way bench/host/pipe_throughput.rs prices
`pipe_16` against `pipe_64k`.

*Correction, 2026-08-19: milestone 138 (close the read gap) step 3 raised the request to 64 KiB
(`fs::TRANSFER_PAGES` = 16). See [steps 1 and 3](read-path-record-and-request-size.md). The one-page
ceiling describes the build measured here.*

The workload is 256 transfers of 4 KiB, so 1 MiB per phase, over one file, in four phases:
sequential write (which also creates the file), sequential read, random read, random write. Two more
phases exist to decompose the others. Offsets in the random phases come from a fixed-seed xorshift64,
page-aligned and inside the file, so the sequence is identical on every run and every system. The
phase size is small for a throughput benchmark; the 16 MiB fixture image bounds it.

Two properties of RedoxFS had to be defeated before any of this measured anything, and the first
draft of the bench fell into both:

- `Transaction::write_node` compares before it writes, so a write whose bytes match what is already
  there does nothing. A benchmark sending one constant page reported random writes as fast as random
  reads, because none of them were writes.
- Records are 128 KiB and lz4-compressed, so 32 identical pages inside one record compress to almost
  nothing. An incompressible page is not enough; each page has to differ from the last.

So every write carries a freshly generated, incompressible page. That costs 512 stores inside the
timed window, because a client that writes data has to produce it. It has its own row,
`fs_payload_fill`, so a reader can take it back out: 820 ns per page, 0.03% of a write. Both host
benchmarks generate their payload the same way.

### The three systems, and which two are at the same tier

| | what runs | tier |
|---|---|---|
| **nife** | `cargo xtask bench --release --real --smp`; the FS server on RedoxFS over blk IPC | QEMU-HVF, `virt`, `-cpu host`, `-m 256M`, `-smp 4`, `virtio-blk-device` on a raw host image |
| **Linux / ext4** | `bench/host/run_linux_fs.sh`, a static PID 1 in a one-file initramfs | **the same**: same machine model, same `-cpu host`, same memory, same core count, same device model, same raw image file, same QEMU default `cache=writeback` |
| **macOS / APFS** | `bench/host/macos_fs.rs`, natively | **not matched**: no virtualization, no virtio, the real NVMe. A reference ceiling, not a competitor |

The first two differ in the filesystem and in nothing underneath it. The third is here for the
reason milestone 25 (cross-OS performance comparison) kept native macOS in the primitive comparison, with the same warning: it says
what the hardware can do with nothing in the way. Any ratio against it measures the tier as much as
the filesystem.

Getting Linux there was most of the work, because the obvious way does not work. Alpine's `virt`
kernel builds neither ext4 nor `virtio_blk` in; both live in `modloop-virt`, a squashfs. So
`run_linux_fs.sh` lifts seven modules out of that squashfs (with podman, since macOS has neither
`mke2fs` nor `unsquashfs`). The bench loads them itself with `finit_module` before it mounts
anything. Two failed boots, one answering ENODEV and one ENOENT, taught that.

### nife: the four phases, and the two that decompose them

Median of the four rounds that passed the noise control (below), all on 2026-08-18, aarch64 release
under HVF on four harts. Run-to-run spread is in the fourth column.

| bench | ns per 4 KiB | MiB/s | spread | = block reads |
|---|---|---|---|---|
| `fs_seq_write` | 2,566,304 | **1.52** | 15% | 55.5 |
| `fs_seq_read` | 1,509,270 | **2.59** | 4.5% | 32.6 |
| `fs_rand_read` | 1,484,338 | **2.63** | 2.2% | 32.1 |
| `fs_rand_write` | 3,427,674 | **1.14** | 6.5% | 74.1 |
| `fs_record_read` (decomposer) | 1,479,768 | 2.64 | 3.5% | 32.0 |
| `fs_read` (control; a 69-byte inline read) | 206,902 | n/a | 0.7% | 4.5 |
| `fs_payload_fill` (the client's own page fill) | 820 | 4,764 | 3.4% | 0 |

The last column is arithmetic on one measured constant: 46.2 us per 4 KiB block through the confined
block server. That is `fs_record_read` divided by the 32 blocks a 128 KiB record holds. Everything
else falls out of it and nothing was fitted:

- A read is 32 blocks, flat. Sequential, random and record-aligned reads agree to within 3%. That is
  also the proof that this path has no cache and no readahead: `IpcDisk` is a bare `Disk` with no
  `DiskCache` around it. *(True of the 2026-08-18 build. Milestone 138 step 2, on 2026-08-19, added a
  64-slot metadata cache, `CachedDisk`; see [steps 4 and 2](read-path-block-contract-and-metadata-cache.md).)*
- `fs_record_read` was added to show the opposite, and refuted itself. The prediction was that a read
  at the start of a record fetches one block where a read at the end fetches 32, since
  `read_node_inner` asks for `BlockLevel::for_bytes(offset_in_record + len)`. It measures the same as
  the others because `read_record` reads the block the pointer stores and only then checks the
  requested level. A fully written record sits at level 5, so every read fetches all 128 KiB.
- An inline read is 4.5 blocks: the tree walk with no record at the end. `motd` is 69 bytes and lives
  inside its node.
- A sequential write is 55 blocks and a random write is 74. A write is a read-modify-write of a
  copy-on-write record. The random case reads 32, writes 32, and spends the rest on the tree and the
  allocator. The sequential case is cheaper because it grows the record from level 0.

### Linux and ext4, at the same tier

Five rounds on a quiet machine. The host figures swing by two to five times run to run while nife's
move by a few percent. So a median of five is a pessimistic reading of Linux, and the minimum is the
fair estimate of what the machine can do. Both are printed.

| variant | seq write | seq read | rand read | rand write |
|---|---|---|---|---|
| **buffered** (what a program gets) | 2,068 / 1,812 | 547 / 449 | 530 / 352 | 2,105 / 1,412 |
| **`O_DIRECT`** (no page cache) | 63,688 / 44,717 | 91,694 / 27,149 | 48,120 / 26,580 | 50,481 / 28,636 |
| **`O_DIRECT` + `O_DSYNC`** (durable per write) | 284,264 / 262,118 | 63,406 / 32,830 | 41,499 / 34,676 | 97,274 / 74,516 |
| **raw `/dev/vda`, `O_DIRECT`** (no filesystem) | 42,104 / 33,691 | 53,296 / 38,730 | 47,890 / 45,307 | 49,426 / 47,467 |

ns per 4 KiB, median / minimum. The variant that prices our protocol's page limit is the same ext4
with the same flags and a unit a real program would use, in ns per 64 KiB transfer:

| variant | seq write | seq read | rand read | rand write |
|---|---|---|---|---|
| **`O_DIRECT`, 64 KiB unit** | 66,737 / 50,009 | 103,411 / 44,473 | 69,073 / 50,648 | 69,522 / 66,483 |

A 64 KiB request costs about what a 4 KiB one does, so sixteen times the payload arrives for the same
price: 600 to 900 MiB/s against 40 to 80. That is the prize a multi-page transfer on
`filesystem_contract` would chase.

One ordering artifact moves a number. The `rawdev` rows run last, after the 64 KiB variant has
written 16 MiB, so the host is still flushing when they start. They are an upper bound on the device
floor rather than a clean reading. The cleanest floor available is `O_DIRECT` sequential read at
27 us.

### macOS and APFS, natively, which is a different tier

| variant | seq write | seq read | rand read | rand write |
|---|---|---|---|---|
| **buffered** | 3,992 / 3,645 | 1,768 / 466 | 1,223 / 338 | 5,739 / 1,859 |
| **`F_NOCACHE`** | 46,460 / 10,319 | 65,843 / 25,898 | 57,128 / 48,210 | 27,241 / 20,454 |
| **`F_NOCACHE` + `F_FULLFSYNC`** | 3,458,029 / 3,092,685 | 57,277 / 37,153 | 71,118 / 51,681 | 2,845,194 / 2,326,291 |

ns per 4 KiB, median / minimum, five rounds. `F_FULLFSYNC` is macOS's real barrier (`fsync` there
does not flush the drive cache). Asking for one on every 4 KiB write costs three milliseconds on an
NVMe SSD. That lands in the same range as our sequential write, and it is not a win: we issue no
device flush at all. The two do different work, and the coincidence is of magnitudes, not of
guarantees.

### Where the milliseconds go

The confined-server tax is about a microsecond. `relay_rtt` measures the exact topology this path
uses, a client through a confined intermediary to a backend, at ~980 icount ticks. Against a 1.5 ms
read that is 0.07%. This is the number the "userspace servers are too slow" objection is about. It
cuts both ways: no tuning of the IPC path moves any figure in these tables.

The block path is at parity with Linux. Our 46.2 us per 4 KiB block goes through a userspace block
server that owns the DMA and answers over IPC. Linux's own 4 KiB reads at this tier are 38.7 to
53.3 us raw and 27 to 92 us through ext4. A confined userspace block driver is not costing us the
device. This milestone was least confident of that result in advance.

The store's geometry is 32x, and it is all of the rest. A 4 KiB read fetches 128 KiB, and everything
in the nife table is that constant times a small integer. It belongs to the store we vendored rather
than to anything this project designed. notes/fs-server.md records it as a `BUGS` entry beside the
server.

### The numbers a skeptic will quote, at 4 KiB

| | nife | ext4 `O_DIRECT` | ext4 buffered | raw virtio |
|---|---|---|---|---|
| sequential read | 1.51 ms | 92 us (min 27) | 0.55 us | 53 us |
| sequential write | 2.57 ms | 64 us (min 45) | 2.1 us | 42 us |

Against buffered Linux we are about three orders of magnitude behind on reads. That is the honest
number for "how fast is a program's file IO". It is the page cache, and it is real: a system with no
cache anywhere reads a hot file at device speed.

### What is not apples to apples

The map bench's tie and the spawn bench's "lighter object than a Unix process" are the model: a
figure quoted without its caveats is worth less than no figure.

1. The 4 KiB unit is ours by constraint and theirs by choice. A `filesystem_contract` request
   cannot carry more than a page. The 64 KiB row prices that, at about sixteen times. *(Step 3's 64 KiB
   request lifts this limit; see [steps 1 and 3](read-path-record-and-request-size.md).)*
2. We have no cache and they have several. No `DiskCache`, no readahead, no metadata cache; the
   identical cost of our sequential, random and record-aligned reads is the proof. `O_DIRECT` and
   `F_NOCACHE` remove the page cache on the other side, but not the in-kernel metadata caching that
   lets ext4 map a block without reading one. *(Step 2's `CachedDisk` now supplies the metadata
   cache; see [steps 4 and 2](read-path-block-contract-and-metadata-cache.md).)*
3. Our write is between Linux's two. Every `filesystem_contract` write goes through a RedoxFS
   transaction that commits to the header ring before the reply. So the filesystem's own state is
   durable per request, the way `O_DSYNC` makes ext4's. But nothing issues a `VIRTIO_BLK_T_FLUSH`
   unless a client asks (`filesystem_contract::fs::SYNC`, milestone 55 (Time Machine)), so the bytes
   sit where `O_DIRECT` alone leaves them. Both rows appear in the output and neither is the
   comparison.
4. The copy counts differ, in our favor. A completed read lands in the page the client already
   shares with the server, so the bytes can be used in place; buffered Linux copies into the caller's
   buffer. `O_DIRECT` closes most of that gap by DMA-ing into the user buffer, one more reason it is
   the row to read.
5. macOS is not at this tier at all, per its table.
6. 1 MiB per phase is small, bounded by the fixture image. It makes the per-request costs clear
   and says nothing about behavior at scale.
7. The machine was shared, per the next section.

### The noise floor, and how a round earns its place

Other agent lanes share this machine, and it was not quiet for most of the day. Load averages
ran between 15 and 70, including a lane deliberately running eight stress processes for its own
measurement. So each system carries a control:

| system | control | its quiet value |
|---|---|---|
| nife | `fs_read` | ~204 us, recorded 2026-07-29, before this milestone existed |
| Linux | `rawdev_seq_read` | 38.7 us, the best observed |
| macOS | `nocache_seq_read` | 25.9 us, the best observed |

The nife figures are the median of the four rounds (of five) whose `fs_read` landed within 2% of that
pre-existing quiet value, on a machine at load 4 to 9. An earlier five-round series, taken hours apart
at load 14 to 18, agrees with them to within 6% on every phase. That is the evidence the control
selects for a real condition rather than for luck. The host figures were taken on the same quiet
machine and are reported as median and minimum because they are the noisier of the two.

A round that failed its control is discarded, not averaged in. That is selection toward the unloaded
machine, stated rather than smoothed away. The alternative on a shared machine is a number nobody can
reproduce.

### One tooling bug found on the way

`helpers/qemu-bounded.sh` killed its watchdog subshell when the guest finished on its own and left the
`sleep` inside it running. An orphaned `sleep` holds the write end of the pipe it inherited. So
every bounded run whose output goes through a pipe blocked for the full bound, however quickly the
guest exited. The Linux comparison boots a guest that powers itself off in about fifteen seconds,
and each round took five minutes. Fixed by killing the watchdog's children first (`pkill -P`). Every
caller that pipes a bounded run was paying this.
