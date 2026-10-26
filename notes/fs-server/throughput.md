# Throughput, and where the milliseconds actually go

An appendix to [notes/fs-server.md](../fs-server.md). It holds milestone 38 (filesystem throughput,
and the comparison): what a read costs and why, the cache that was not there and then was, the
transfer unit, and the throughput BUGS.

## The per-request number, and the record behind it

The per-request number has been here since milestone 32 (a real filesystem: RedoxFS behind a
capability FS server) built this server: `fs_read`, ~204 us. There was no MB/s figure at all, which
is what milestone 38 was for. The four phases now live in `fs_test_client`'s throughput role and
`kernel/src/bench.rs`'s `fs_throughput`. The cross-OS side is in notes/benchmarks.md. Two results
belong here, next to the server they are about.

Every 4 KiB read of an ordinary file fetches the whole record, whatever offset it asked for. At
milestone 38's 128 KiB record that was 32 blocks for 4 KiB of use. `fs_read` reads `motd`, 69 bytes,
which RedoxFS keeps *inline in the node*. That is the cheapest read this server can serve, at
~207 us, and it is a tree walk with no record at the end of it. A 4 KiB read of an ordinary file was
~1.51 ms. Dividing by 32 gave 46.2 us per 4 KiB block through the block server. That constant then
explained every other figure: an inline read is 4.5 blocks, a sequential write is 55, a random write
is 74.

Two things in that paragraph no longer hold, and it is kept because the reasoning is why. The
46.2 us was an *average* that charged the per-request tree walk to the blocks. The sweep of milestone 138
(close the read gap: a 4 KiB request must stop moving 128 KiB) separated the two with six points and
a slope. The marginal cost of a block is 39.0 us, with a separate ~208 us walk per request. And the
record is no longer 128 KiB. Step 1 took it to 8 KiB, so a 4 KiB read is now ~284 us and fetches two
blocks. See notes/benchmarks.md.

## The mechanism is one level below where it looks

Milestone 38's bench found that out by predicting the wrong thing and measuring. `read_node_inner`
asks for the record at `BlockLevel::for_bytes(offset_within_record + len)`. That is level 0 at the
start of a record and level 5 at the end, so a read at a record boundary should have been cheap. It
is not. `read_record` reads the block the pointer stores, and only then checks whether that is as
large as the level requested. A fully written record is stored at level 5. The bench keeps the phase
that refuted the prediction (`fs_record_read`) for exactly that reason.

None of this is the isolation. `relay_rtt` prices one confined intermediary at about a microsecond,
three orders of magnitude below a read. 46.2 us per block is the same as Linux gets on the same
virtio device at the same tier (notes/benchmarks.md has that comparison).

## There was no cache, and then there was

There was no cache anywhere in this path, and the throughput numbers are how we finally proved it.
`redoxfs_server`'s `IpcDisk` was a bare `Disk` with no `DiskCache` wrapped around it, so a re-read
went back to the device. That had been stated and never demonstrated. Milestone 38 demonstrated it by
measuring sequential, random and record-aligned reads of the same file, and getting the same
per-request cost to within 3% (1.51, 1.48 and 1.48 ms). A path with a cache or a readahead cannot do
that. It also finally retired a comment in `fs_test_client` that had claimed `fs_read` was a warm
measurement.

Milestone 138's step 2 wraps `IpcDisk` in `CachedDisk`
(`redoxfs_server::CachedDisk`, `redoxfs_server/src/block.rs`). It is a small direct-mapped
write-through cache of single-block reads: 64 slots, ~257 KiB. It answers exactly the repetition
milestone 38 found and this section names above. That is `Transaction::read_tree_and_addr`'s
five-block tree walk, answered from memory when the walk resolves the same node it last resolved. It
does not cache a record body. Any `Disk::read_at` wider than one block bypasses it entirely, still
batched instead per step 4. It never serves stale bytes. A write updates or invalidates the relevant
slot only after the inner disk confirms the write landed. RedoxFS's copy-on-write allocator means a
live address's content can only ever change through that same write path.
`redoxfs_server::CachedDisk`'s own doc argues the invariant in full. Six host tests, including a
stale-read regression, check it in milliseconds with no emulator.

What this means for the "no cache" claim above, stated exactly rather than softened. It was true of
the build milestone 38 measured, and it is not true of the build this tree ships today. A repeated
`fs_read` now answers from the cache after the first request in a session. (`motd` is stored inline
in its node, so its entire content rides on the tree walk, with no separate record read at all.)
That is the case a cold-vs-warm distinction was previously impossible to make: 210,490 ns median to
9,474, 22.2x, eight interleaved rounds at each point (`sh bench/cache-slots-sweep.sh 8 1 64`).
`bench/cache-slots-sweep.sh` is the instrument that isolates it. Its own doc explains why capacity 1
is "approximately off" rather than a true zero.

Sequential, random and record-aligned reads of a different file, or the first access of any file in
a fresh session, still pay the uncached cost this section describes. Every earlier number in this
file and in notes/benchmarks.md was taken against the uncached build. Each is a fact about that
build, not a fact this section now retracts.

## The transfer unit: one page until milestone 138 step 3, sixteen since

A `READ` or `WRITE` moves its bytes through the region the client and the FS server share, and that
region was one page. Nothing in the wire word ever required that: `fs::req` has carried a 40-bit
length since milestone 32. So the multi-page transfer milestone 38's `BUGS` entry called for turned
out to be one constant, `filesystem_contract::fs::TRANSFER_PAGES`. There was no new opcode, reply
word, descriptor or changed field.

What a party maps, and the one rule that makes old clients safe. The FS server maps the whole channel,
because it serves whatever length a request asks for. A client maps as much as it intends to use.
`swish`, the three caretakers, the sinks and the `std` PAL still map exactly one page, and still ask
for one page. They are untouched by the step and cannot tell it happened. That is a property rather
than a convention, and the serve loop is where it is enforced:

- The server clamps a length the client chooses (`READ`, `WRITE`) to `fs::TRANSFER_MAX`. A client
  that asks for one page gets one page, and a client that maps sixteen may ask for sixteen.
- The server clamps a length it chooses (`READDIR`, the four attribute verbs, a rename's two names)
  to one page, as it always did. This is the half that matters. A `READDIR` that filled 64 KiB would
  write into a single-page client's *unmapped* second page, and no client asked for it.

What is not checked, marked because it is rung four of CLAUDE.md's ladder and not rung one: nothing
stops a client asking for more than it mapped. It gets a data abort on its own second page, after the
server has already done the work. It is the same agreement a client already has with its wiring about
where the region lives. Every FS client hardcodes its `FILE_VA`, and `kernel/src/user/fs_service.rs`
carries a comment saying it must match. This is one number wider, rather than a new category. Making
it unrepresentable needs the channel size to travel on the wire, which is a new concept on this
contract, and was deliberately not taken.

The kernel wiring allocates the channel as a contiguous run of frames (`file_channel`). So both
halves of the agreement still pass it around as one physical base address, and each mapping site is a
loop rather than a signature change. The block server's DMA region was already wired this way.

## BUGS

- A 4 KiB request no longer moves 128 KiB, and the transfer unit is no longer 4 KiB. This entry used
  to record a 32x amplification. The client's transfer unit was one page, because that is what a
  `filesystem_contract` request could carry, and RedoxFS's record was 128 KiB. So a read fetched 32
  blocks, and a write read 32, changed one and wrote 32 back. Both halves are gone.

  Milestone 138 step 1 took the record to 8 KiB (`RECORD_LEVEL` 1, vendor/README.md divergence 5).
  It measured 5.13x on a 4 KiB read and 3.01x on a 4 KiB write: 1,458 us to 284 us, and 2,400 us to
  797 us. Step 3 then took the transfer unit to 64 KiB (`filesystem_contract::fs::TRANSFER_PAGES`,
  above). It measured 5.67x on a read and 8.02x on a sequential write against the same harness:
  80.30 MiB/s and 42.77, from 14.16 and 5.33.

  What survives is a fixed ~204 us per request that neither touches. Step 3 moved it from 74% of a
  read to 26%, because the payload it is charged against is sixteen times larger. That term is
  identified rather than attributed: five single-block reads, the *same* five block numbers on every
  request (the node tree's L3 root, L2, L1, L0, then the file's own `Node`). 99.6% of them are a block
  already read, which is 195 us of it. It is the absence of a cache rather than a property of RedoxFS,
  and that is what rules out replacing the store. The write's ~690 us transaction is the same story,
  and is where step 3's biggest number comes from. It is charged once per request, so it went from
  690 us per 4 KiB to 43. notes/benchmarks.md has both before-and-afters, the two-term model fitted
  twice from two different variables, and the residual.
- Closed 2026-08-19 (milestone 138 step 4). This entry recorded that the block contract had the
  one-page limit the file contract used to have, and had become the binding constraint rather than
  the wall behind one. `IpcDisk::read_at` chunked every record into one `filesystem_contract::blk`
  request per 4 KiB block, so a 64 KiB read was 16 device round trips rather than one.
  `blk::TRANSFER_BLOCKS` (16, mirroring `fs::TRANSFER_PAGES`) and batched `IpcDisk` calls close it. A
  `Disk::read_at`/`write_at` call now moves up to 16 contiguous blocks in one blk `CALL` and one
  virtio descriptor.

  It measured 1.16x to 1.55x across the throughput phases (notes/benchmarks.md), well below the naive
  16x. RedoxFS's own per-record tree walk (five unbatchable single-block reads) and an 8 KiB record
  (step 1, two blocks) together bound what one call can batch to a fraction of a 64 KiB request. The
  write-through of what remains, mostly that tree walk, is milestone 138's step 2,
  `redoxfs_server::CachedDisk`, also built. The crash injector keeps its own one-block-per-`CALL` path
  unconditionally, since a batched virtio request cannot be torn mid-batch the way the injector needs.
  So this closure does not touch the property milestone 37 (prove RedoxFS's crash
  consistency) checks.
- A write whose bytes match what is already there is not a write. `Transaction::write_node` compares
  before it does anything, so rewriting a block with identical contents costs a read and no write at
  all. That is a sensible store optimization and a trap for anyone measuring: a benchmark that sends
  one constant page repeatedly measures the comparison. It also means a client cannot use a rewrite to
  force an allocation.
- RedoxFS compresses records with lz4 when the record is larger than one block, which is still always
  here. Milestone 138 chose an 8 KiB record (two blocks) rather than a one-block record partly to keep
  it. Payload entropy therefore changes throughput: an all-zero or repetitive file writes and reads
  several times faster than an incompressible one. Every number above and in notes/benchmarks.md uses
  an incompressible payload, which is the conservative choice and the one a backup workload resembles.
