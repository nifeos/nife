# Milestone 138 (close the read gap), steps 4 and 2, and the comparisons not yet run

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the 16-block blk contract, the metadata
cache, and the two controlled comparisons, with the dates, tables and corrections behind them. Name:
ratified 2026-09-24 (calef), who refused `milestone-138-steps-4-and-2` for this file; [the naming
record](README.md) says why.*

## Step 4 taken: the blk contract carries 16 blocks, and the win is smaller than the block count predicts (milestone 138, 2026-08-19)

Step 3's own residual pointed here ([steps 1 and 3](read-path-record-and-request-size.md)). After it,
`fs_seq_read` was 74% single-block trips through `filesystem_contract::blk`, one per filesystem
block, against a ~100 MiB/s ceiling notes/fs-server.md's `BUGS` section had already named.

`filesystem_contract::blk::TRANSFER_BLOCKS` goes from an unwritten 1 to 16. The region the FS server
and the block server share is 64 KiB of contiguous pages. `IpcDisk` batches contiguous whole-block
runs into one blk `CALL` (up to 16 blocks), and the block server issues one virtio descriptor for the
whole batch instead of one per block. The crash injector and the write-verify diagnostic keep the
pre-step-4 path, one block per `CALL`, unconditionally. Neither tolerates a request the device
completes as one indivisible unit; `redoxfs_server`'s `IpcDisk::write_at` carries the argument.

### Before and after, ten interleaved rounds each, on a shared and noisy machine

`sh bench/blk-transfer-sweep.sh 10 1 16`. The machine was not quiet: `uptime`'s one-minute load sat
at 15 to 21 throughout, with several other lanes building and testing, the tree's normal condition
per `AGENTS.md`. Every earlier sweep ran at 3.6 to 9. The `fs_read` control says the numbers are
still usable: 8 of 10 rounds at each point landed within 6% of the 203,000 to 208,000 ns quiet
baseline the earlier sweeps established. The figures below are the median of all 10 rounds at each
point. That is milestone 38's discipline (discard a loaded round rather than average it) adapted to
a run where almost every round is close and one or two are not. The file transfer size stays at
`fs::TRANSFER_PAGES = 16` (64 KiB, step 3's setting) for both points; only `blk::TRANSFER_BLOCKS`
varies.

| phase | 1 block per blk CALL | 16 blocks per blk CALL | speedup |
|---|---|---|---|
| `fs_seq_write` | 1,544,228 ns | **1,335,376 ns** | **1.16x** |
| `fs_rand_write` | 2,101,313 | **1,539,488** | **1.37x** |
| `fs_seq_read` | 811,724 | **527,420** | **1.54x** |
| `fs_rand_read` | 848,706 | **546,541** | **1.55x** |
| `fs_record_read` | 843,448 | **545,040** | **1.55x** |

### Why this is nowhere near 16x

The naive model says sixteen round trips become one, so the win should be close to 16x. It is not,
because steps 1 and 3 already changed what is inside the batch.

`fs::TRANSFER_PAGES` is 16 (64 KiB per file-level request), but `RECORD_LEVEL` is 1 (step 1): an
8 KiB record, two blocks. RedoxFS's engine (`Transaction::read_node`/`read_node_inner`,
`vendor/redoxfs/src/transaction.rs`) walks the tree once per file-level `Server::read`/`write` call.
That walk is `read_tree`, five single-block reads: the L3/L2/L1/L0 spine and the target node. The
engine then loops over the records that one call's transfer spans. It reads each record's body with
its own `Disk::read_at` call, sized to the record.

A 64 KiB request therefore spans 8 records. Step 4 batches each record's own body (2 blocks, one call
instead of two). It cannot batch across records, because each record's body arrives through a
separate `Disk::read_at` the engine issues on its own. Per record that is 5 metadata reads
(unbatchable, one call each, untouched by step 4) plus 1 data call, which was 2. The read call count
per 64 KiB request falls from 8 x 7 = 56 to 8 x 6 = 48. That call-count ratio of 1.17x lines up with
`fs_seq_write`'s measured 1.16x almost exactly.

Reads show a larger ratio (~1.55x) than writes (~1.16 to 1.37x). A read's cost is almost entirely blk
calls, so the same eight eliminated round trips are a larger fraction of it. A write also pays the
unbatched transaction commit, which dilutes the same absolute saving.

The absolute savings agree with the per-block marginal cost measured twice before (35.9 to 39.1 us).
Eight round trips at ~37 us each predict ~296 us saved. `fs_seq_read`, `fs_rand_read` and
`fs_record_read` each saved 284,304 to 302,165 ns, within 6%.

The finding, as a sentence a reader can act on: the record size bounds step 4's batching, not the
file-level request size, because RedoxFS only ever asks its `Disk` for one record's worth of bytes
at a time. Step 1 chose an 8 KiB record to keep lz4 compression, and that caps what step 4 alone can
batch per record at two blocks. Most of what remains (5 of 6 to 7 calls per record) is the tree walk,
which is step 2's target.

### Crash consistency, re-run at the new geometry

`redoxfs_server/tests/crash_consistency.rs`, unchanged pass: 0 silently wrong. It could not have
changed, for the reason step 3's re-run gave: the host-side crash model drives
`BlockDisk<Recording>` directly, below `IpcDisk`'s batching. The device-level crash injector
(`redoxfs_server/src/bin/redoxfs_server.rs`'s `inject` module) is on `IpcDisk`'s own path. That is
why `write_at` keeps an unconditional one-block-per-`CALL` fallback whenever the injector might be
armed. The fallback is byte-for-byte the code the earlier steps exercised, so the crash test's
coverage of the real device is unchanged by this step, not merely re-run.

### BUGS

- The machine was busy, `uptime` 15 to 21 throughout. The `fs_read` control and the model's
  internal consistency (a predicted ~296 us saving against a measured 284 to 302 us across three
  independent phases) are the evidence of a real signal. It is not the quiet single-tenant machine
  the record-level and transfer-size sweeps had.
- `blk::TRANSFER_BLOCKS` was set equal to `fs::TRANSFER_PAGES` (16) for symmetry with step 3, not
  because 16 is where step 4's curve bends. Nobody has swept it independently the way
  `bench/record-level-sweep.sh` swept the record level. `sh bench/blk-transfer-sweep.sh` takes any
  list of block counts and would answer this in one run.
- A larger record level would let step 4 batch more per record, and nobody has re-measured that
  trade now that step 4 exists. `RECORD_LEVEL` is still 1 for lz4's sake (step 1's reasoning); this
  step does not revisit it.

## Step 2 taken: a 64-block metadata cache, and it is the biggest single number in this milestone (milestone 138, 2026-08-19)

Step 4 said where the residual now lives. Of every 6 to 7 blk calls per record, 5 are
`Transaction::read_tree_and_addr`'s tree walk. It runs fresh on every `Server::read`/`write`/...
call, even when the call just before resolved the identical node. That
was first identified on 2026-08-18 ([the fixed term](five-blocks-per-request.md), and notes/fs-server.md's
"the same five blocks every time") and not addressed until now.

`redoxfs_server::CachedDisk` (`redoxfs_server/src/block.rs`) wraps `IpcDisk` in a small direct-mapped,
write-through cache of single-block reads. It has 64 slots (`CACHE_SLOTS`,
`redoxfs_server/src/bin/redoxfs_server.rs`), about 257 KiB. Only a `buffer.len() == BLOCK`
`Disk::read_at` consults it; a record body, already step 4's batched call, bypasses it. A write
updates or invalidates the written block's slot, and only after the inner disk confirms the write
landed.

That is enough without a generation counter. RedoxFS's copy-on-write allocator never rewrites a live
address in place, so a cached address's content can change only through that same write path. Six
host tests (`redoxfs_server/src/block/tests.rs`) check hit and miss, write-through freshness, short-write
invalidation, slot-collision safety and multi-block bypass, in milliseconds with no emulator.

### Before and after, eight interleaved rounds each

`sh bench/cache-slots-sweep.sh 8 1 64`, on the same shared, noisy machine as step 4 (`uptime` load
15 to 19). Capacity 1 is not quite "off", since the same block asked for twice in a row still hits.
But the tree walk touches five different blocks per call, so a one-slot cache thrashes across a
single walk and rarely survives to the next; the sweep's own doc says so. `fs::TRANSFER_PAGES`
(64 KiB) and `blk::TRANSFER_BLOCKS` (16) are at their step-3 and step-4 settings for both points. This
isolates the cache's marginal contribution on top of everything already shipped.

| phase | 1 slot (~off) | 64 slots | speedup |
|---|---|---|---|
| `fs_read` (control; repeated inline read) | 210,490 ns | **9,474 ns** | **22.2x** |
| `fs_seq_write` | 1,387,898 | **936,583** | **1.48x** |
| `fs_rand_write` | 1,487,736 | **1,087,904** | **1.37x** |
| `fs_seq_read` | 514,419 | **329,168** | **1.56x** |
| `fs_rand_read` | 542,605 | **331,732** | **1.64x** |
| `fs_record_read` | 560,088 | **341,504** | **1.64x** |

`fs_read`'s 22.2x needs its own explanation, because it is the control this milestone used to prove
there was no cache. `motd` is 69 bytes and lives inline in its node, so reading it needs no record
read: the tree walk is the whole request. With the cache warm, every read after the first answers
from memory. `fs_read` collapses toward the bare IPC and server floor, estimated at ~13 us in
[steps 1 and 3](read-path-record-and-request-size.md). 9,474 ns beats that estimate, plausibly because the
cache lookup is cheaper than a `CALL`, plus whatever margin the estimate carried.

This retires the "no cache anywhere" claim that milestone 38 (filesystem throughput) demonstrated and that
[filesystem throughput](filesystem-throughput.md) and notes/fs-server.md stated as an architectural
property. It was true of the build measured then and is not true of the build the tree ships now. The correction
in place in notes/fs-server.md says what still holds: a different file's first
access, or any file's first access in a fresh session, is still fully uncached.

### The combined effect of all four steps, against milestone 38's original baseline

Multiplying the per-step ratios would compound noise from four days and machine states, so the honest
total is one head-to-head. For `fs_seq_read`, milestone 38's 1,509,270 ns per 4 KiB request
(2.68 MiB/s) against this run's 329,168 ns per 64 KiB request (189.9 MiB/s) is 70.9x. That is all
four steps combined, on one machine, in MiB/s because the transfer size itself changed at step 3.
Most of the gap this milestone set out to close has closed on this metric. The remaining gap to
buffered Linux (7,141 MiB/s) is the page-cache gap this milestone was never scoped to close; see
"What is out of scope, deliberately" in
[`design/roadmap/0138-file-io-throughput.md`](../../design/roadmap/0138-file-io-throughput.md).

### Crash consistency, re-run at the new geometry

`redoxfs_server/tests/crash_consistency.rs`, unchanged pass: 0 silently wrong. The reason is the one
steps 3 and 4 gave: the host-side model drives `BlockDisk<Recording>` below `CachedDisk`, so the cache
is not on its path. At the device level the property that matters is an argument, not a re-run. The
recovery mount of milestone 37 (prove RedoxFS's crash consistency) is a fresh process, so it builds
a fresh, cold `CachedDisk` and can never observe what the killed process's cache held. A cache that
survived its process's death would be the hazard; one that cannot outlive its process is not.

### What was not measured, and is the first thing to run next

The two caches were not swept against each other. `blk::TRANSFER_BLOCKS` and `CACHE_SLOTS` were each
swept alone, at the other's shipped value. Whether a smaller blk batch with a larger cache, or the
reverse, reaches the same total for less memory or DMA region is open. `bench/cache-slots-sweep.sh`
and `bench/blk-transfer-sweep.sh` run together would answer it, and nobody has run them together.

64 slots was chosen against the tree spine's size (five blocks, times roughly twelve for several open
handles) and against milestone 37's smaller crash-test heap budget. It was not chosen from a sweep of
the capacity. `sh bench/cache-slots-sweep.sh N 1 4 16 64 256` would show where the curve bends; the
two points here are the shipped value and an approximate floor.

### BUGS

- The machine was busy here too, `uptime` 15 to 19 throughout. The `fs_read` control's absolute
  value (9,474 to 15,107 ns across 8 rounds at 64 slots) is small enough that a scheduling hiccup
  moves it proportionally more than a millisecond-scale phase. The headline 22.2x is a median of 8
  rounds for that reason.
- The cache's size follows this milestone's test fixtures, not a real deployment's node count.
  [The fixed term](five-blocks-per-request.md) puts a 65,536-node filesystem's full spine at 259 blocks. 64
  slots keeps one open file's working set hot. It thrashes if enough distinct files are open at once
  to collide across the tree's shared upper levels, and nobody has measured that case.
- A collision evicts silently. Two block numbers that hash to the same slot (`block % 64`) take
  turns in the cache. Correctness does not depend on the hit rate, since a miss is always safe. But
  a workload alternating between two colliding blocks gets none of this step's benefit, and no
  instrumentation would show a reader why.

## The two controlled comparisons nobody has run (2026-08-19)

Every filesystem comparison in these appendices is uncontrolled. The published pairing is
nife-on-RedoxFS against Linux-on-ext4, where the operating system and the filesystem differ at once,
so either one could explain a gap. That is why milestone 138's answer to *"does this architecture
have a disk-read liability that cannot be overcome"* comes from decomposition rather than from one
number. The block server is at parity with raw virtio, the per-request residual is ~13 us,
and everything else found so far is an implementation choice.

Two comparisons would control it. Both were calef's, on 2026-08-19, and neither has run.

### Linux-on-ext2 against nife-on-ext2

This holds the filesystem constant and leaves the architecture. Its time comes when the ext2 stratum
of milestone 140 (mount a drive this system did not create) exists. That block argues it: the ext2
row isolates the operating system, the nife column isolates the filesystem, and today's diagonal
isolates neither.

Its ceiling: our ext2 would be new against a thirty-year-old one, so the result bounds what the
architecture can cost rather than deciding it.

### Redox-on-RedoxFS against nife-on-RedoxFS

This is the sharper of the two, because of how we got RedoxFS. We vendor it, so this is not an
equivalent implementation; it is the same code. Every difference is ours: the IPC, the scheduler, the
block driver, the shared-page contract. There is no filesystem-maturity caveat, because it is their
filesystem.

It also asks a different question. Linux tells us whether the architecture is viable; Redox tells us
whether we are a good instance of it, against a project about a decade older than this one.

It carries a falsifiable prediction, restated 2026-09-24 against the build that ships step 2.
*(Correction, 2026-09-24: the 2026-08-19 version assumed this system ran no metadata cache. It
predicted Redox faster by roughly the ~195 us the tree walk cost, with `redoxfs::DiskCache` as the
difference. Step 2, above, shipped `CachedDisk` the same day, so that term is gone from this side
and the prediction as written would have confirmed nothing.)*

Both systems now cache the metadata walk, but not the same way, and the prediction has to name
where they differ:

| | `redoxfs::DiskCache` (Redox, expected) | `CachedDisk` (this system) |
|---|---|---|
| size | 16 MiB, `vendor/redoxfs/src/disk/cache.rs` | 64 slots, ~257 KiB |
| policy | FIFO over a `HashMap` | direct-mapped, `block % 64` |
| what it holds | every block read, record bodies included | single-block reads only; a record body bypasses it |

Redox is still only *expected* to run `DiskCache`: it is std-only, and Redox has `std`. That comes
from reading the vendored source, not from Redox's running filesystem service, and the comparison
should confirm it first.

So the prediction splits in three, and each part answers a different question:

1. **One file, first pass.** Neither cache can hold a data block it has not yet read, and both hold
   the walk after the first request. The ~195 us term should appear on neither side. Whatever gap
   remains is the IPC, the scheduler, the block driver and the shared-page contract, which is the
   question this comparison exists for. If Redox is faster here by roughly 195 us again,
   `CachedDisk` is not doing on this path what `fs_read`'s 22.2x says it does. If it is faster by
   substantially more, something is wrong somewhere nobody has looked, which is still the more
   valuable outcome.
2. **The same file read again.** Redox's cache holds the data blocks and this one does not, so Redox
   should win by roughly the device term of every record. That prices a data cache this system chose
   not to build (the page-cache gap milestone 138 (close the read gap) scoped out), not the
   architecture. It has to be reported as such or it will be quoted as the latter.
3. **Several files open at once.** 64 direct-mapped slots collide across the tree's shared upper
   levels, the case step 2's BUGS says nobody has measured. A gap here is cache capacity and policy.
   It is worth running because it is the first workload that would show a reader the collision
   entry above as a number.

Caveats, both directions:

- Pin the same RedoxFS revision. This tree carries five divergences against the vendored engine,
  including step 1's `RECORD_LEVEL_MAX`. Comparing against a different revision measures the
  divergence rather than the operating system.
- Schemes are not capabilities. Redox's IPC has different semantics, so "the difference is ours" is
  not the same as "the difference is implementation quality". Some of it is design, and a report that
  elides that overclaims in whichever direction the number points.
- The cost is the setup, not the measurement: Redox booted at the same tier, same machine model, same
  device, same payload, with the noise control these appendices already use.
