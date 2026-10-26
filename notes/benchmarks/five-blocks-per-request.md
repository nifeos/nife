# The fixed term: five blocks per request

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the 208 us identified, what option 2
costs, and why 4 KiB was the only transfer size the system had, with the dates, tables and
corrections behind them. Name: ratified 2026-09-24 (calef), who refused `the-fixed-term` for this
file; [the naming record](README.md) says why.*

These sections continue [the record-level sweep](record-level-sweep.md) of milestone 138 (close the read gap), all dated
2026-08-18 unless marked.

### The 208 us, identified: five blocks, and they are the same five every time

The sweep left the fixed term attributed rather than measured, as "RedoxFS re-reading its own
metadata, about 5.3 block reads". calef asked whether that is inherent to RedoxFS's design or the
absence of a cache any store would need. It is the second. The measurement is a count rather than a
time, so it is not close.

Every 4 KiB read of an ordinary file makes exactly five single-block reads below the record, and they
are the same five block numbers on every request. Not 5.3 on average, and not a different five each
time: five, and the same five, in every phase measured.

They are one call, `Transaction::read_tree_and_addr`, which `Server::read` reaches once per request
through `read_node`:

| # | block | what it is |
|---|---|---|
| 1 | `header.tree` | the node tree's L3 root, fixed for the whole filesystem until something commits |
| 2 | L2 | one block per 16.7 M node ids |
| 3 | L1 | one block per 65,536 node ids |
| 4 | L0 | one block per 256 node ids |
| 5 | the node | the file's own `Node`, one block, one per file |

`TREE_LIST_SHIFT` is 8 (`vendor/redoxfs/src/tree.rs`), so the fanout is 256 per level. Every file
whose node id falls in the same 256 shares the first four blocks. In the fixture below, two 1 MiB
files and a `motd` share blocks 1 through 4 and differ only in block 5.

#### How it was counted

A `Disk` implementation over an in-memory image logs every `read_at`. It was built as a temporary
probe in `redoxfs_server`'s host tests and reverted before this note was committed (reproduction
below). `BlockDisk` splits a `Disk` call into whole-block transfers, one `filesystem_contract::blk`
request each, exactly as `IpcDisk` does on device. So `ceil(len / 4096)` per call is the number of
block-server round trips the request costs on the machine. The block numbers differ on device (a
different image, behind a partition offset); the counts and the repetition do not.

Fixture: a 32 MiB image, `FileSystem::create`, two 1 MiB incompressible files and one inline `motd`,
reopened through `Server::open`, then 256 requests of 4 KiB per phase, logged per request.

| phase | single-block reads per request | distinct such blocks | record read |
|---|---|---|---|
| 1 MiB file, 256 sequential 4 KiB reads | **5.00** | **5** | 1 call, 32 blocks |
| 1 MiB file, 256 random 4 KiB reads | **5.00** | **5** | 1 call, 32 blocks |
| a second 1 MiB file, 256 sequential | **5.00** | **5** | 1 call, 32 blocks |
| alternating between the two files | **5.00** | **6** (4 shared) | 1 call, 32 blocks |
| `motd`, 64 reads, inline, no record | **5.00** | **5** | none |

99.6% of those reads were of a block already read in the same phase (1,275 of 1,280). Zero writes
happened during any read phase, so nothing was invalidating anything.

It does not move with the record level, which makes it the fixed term rather than part of the slope.
Re-run at `RECORD_LEVEL` 1 and 2: 5.00 per request, 5 distinct blocks, unchanged, with the record read
falling to 2 and 4 blocks as the model says.

Level 0 is the exception, and it is the residual the sweep already found. There the record read is
itself one block, so the probe's classifier folds it in. The figure is 6.50 per request sequential and
6.53 random. That decomposes as five tree blocks, one record, and 0.5 indirect pointer blocks. A node
holds 128 direct record pointers and a 1 MiB file at level 0 is 256 records, so half of its reads
need an indirect block first. That is the 5% level-0 residual in the sweep's fit, measured directly.
It is a property of the small record, not of the walk.

#### What a perfect cache removes

Five block reads at the measured marginal 39.0 us is 195 us against the fitted 208 us intercept, 94%
of it. The remaining ~13 us is the file-IPC round trip and the server's own work, which no cache
touches. `fs_read` says the same from the other direction: it reads an inline `motd`, does exactly
these five reads and nothing else, and costs 203 to 208 us.

The cache is not a large object. Four of the five blocks are the tree spine, and every file shares
them. A filesystem with 65,536 nodes has a spine of 1 + 1 + 1 + 256 = 259 blocks, about 1 MiB. The
fifth block is the node, one per open handle, which a server holding handles could keep without a
cache at all.

*Correction, 2026-08-19: milestone 138 step 2 built that cache, a 64-slot `CachedDisk`, and
`fs_read` fell from ~210 us to ~9.5 us. See [steps 4 and 2](read-path-block-contract-and-metadata-cache.md).*

So option 3 gets nothing here either, now for a measured reason. The walk is structural in one narrow
sense: the format fixes the depth at four levels plus the node, and a store with a shallower
id-to-node map would do fewer reads. That is not what makes it cost 195 us. It costs 195 us because
the same five blocks are fetched off the device 256 times in a row. Every store that maps an id to a
node has a path from a root to that node, which it would also fetch. Replacing RedoxFS buys a rewrite
and arrives needing the identical cache. **Nothing measured here is evidence that RedoxFS is the
problem.**

This count does not settle the half a count cannot reach. It says a cache removes 94% of the fixed
term on this workload, not that a cache is cheap to build. A cache in this server has coherency and
confinement questions of its own, and milestone 138 puts it out of scope on purpose. This is an
argument about which milestone owns the 208 us, not a design for one.

#### Reproducing it

The probe was a `Disk` recorder in `redoxfs_server`'s `mod tests`, plus a driver that clears the log
per request and histograms block number against read count. It is not in the tree: it is a
one-question instrument, and the tree already carries the two facts it produced. The whole of it is
`read_tree_and_addr`'s five `read_block` calls. A reader who wants the result without the probe can
read `vendor/redoxfs/src/transaction.rs:498` and count. `git log` for this section has the probe in
its message.

Conditions: host measurement only, no emulator, so no QEMU ran and nothing competed with another
lane. Load average 2.4 at the start. Load matters less here than anywhere else in these notes: every
number in this section is a count of block reads, and a count does not move with load. The one time
in it, 39.0 us per block, comes from the sweep and carries that sweep's conditions.

### What option 2 costs, and why level 1 beats level 0

Not going all the way down avoids two of the three costs milestone 138 named.

Compression is given up at level 0 and only at level 0. RedoxFS compresses a record when its stored
level is above zero (`write_node_inner_records`: `if decomp_level.0 > 0`). So a one-block record is
never compressed and an 8 KiB record still is. Level 1 keeps lz4 and reads 8% slower than level 0,
against the 5.6x either of them buys. It writes sequentially faster (769,266 ns against 790,317).

More records means more block pointers, and the sweep shows it in the level 0 residual. It grows with
the file. An 8 MiB Time Machine band file is 64 records at level 5, every one of them direct. At level
1 it is 1,024 records, of which 87% need an indirect block read: one more 39.0 us round trip on a
280 us request, about 14%. Level 1 halves the number of records against level 0 for the same reason
it keeps compression.

Copy-on-write means a write reads its record first, and that cost falls with the level. It is the
80.0 us per block slope, and at level 0 there is one block to read instead of 32. It is a cost of the
large record, not of the small one.

The space cost is the one this sweep can put a number on. The same 560 KiB of documentation was
imported into a fresh 16 MiB image and counted as non-zero 4 KiB blocks:

| record level | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|---|
| blocks used | 200 | 172 | 165 | 162 | 160 | 145 |
| against level 5 | **+38%** | +19% | +14% | +12% | +10% | 1x |

That is compression and metadata together, on text, the payload most favorable to lz4. Level 0 gives
up both and pays 38%. Level 1 keeps compression and pays 19%, and the 19% is the pointers rather than
the entropy. An incompressible payload would show only the pointer half. So a backup workload should
expect something closer to the 10% at level 4 than to the 38%, and this sweep did not measure that
case.

### How a level is chosen, and the correction the code forced

The premise milestone 138's block rests on is true and incomplete. `record_level` is a per-node field
in the on-disk format (`vendor/redoxfs/src/node.rs`), and `Node::new` sets it once at creation. Both
data paths honor the node's value rather than the crate constant (`transaction.rs`,
`read_node_inner` and `write_node_inner_records`). Directories get 0 already.

What the block gets wrong is "not a fork of the vendored crate". Three things in the engine put a
per-file level out of reach today:

- `Node::new` takes no level and there is no setter. The constant is the only source.
- `RecordRaw::empty` and `HTreeNode::empty` both refuse a level above `RECORD_LEVEL`, and
  `read_block` allocates its buffer through `T::empty(ptr.addr().level())`. So lowering the constant
  makes every record already stored at a higher level unreadable, with `ENOENT`, on an image that was
  good before.
- Nothing in `filesystem_contract` can name a level, so the FS server would have nothing to pass down
  even if the engine took one.

So option 2 has two shapes, and they are different decisions. Lowering the default is one line and a
one-way door for every existing image. No migration exists, and it costs nothing today only because
every image in this tree is regenerated from source. A genuine per-file choice is what the format
supports and the crate does not. `Node::new` would have to take a level, and the two `empty` guards
would compare against a maximum rather than the default. A creating client would need a way to say
which level it wants. That is a divergence carried in `patches/` plus a contract change, larger than
the block priced.

### The workload question, answered by reading

*The code quoted below changed on 2026-08-19 (milestone 55 (Time Machine)). The section stands because its
reasoning makes the change legible.* `smb_server`'s two `min`s now read `fs::TRANSFER_MAXIMUM` rather than
`filesystem_contract::PAGE`, so a Mac writing a megabyte arrives as 16 requests rather than 256.
Measured through a real SMB client: write 4.8x, read 2.4x, against the 8.02x and 5.67x step 3
measured on the contract itself ([steps 1 and 3](read-path-record-and-request-size.md)). The residual is
now owned by the socket contract's own 4080-byte chunking. The table and reasoning are in
[notes/smb.md](../smb.md)'s throughput section. What follows is the finding as it stood, which is
what made that milestone exist.

Milestone 138 asks whether 4 KiB is the atypical case. A Time Machine backup writes band files, which
are large and sequential, and a 128 KiB record is plausibly right for those.

It is not the atypical case. It is the only case this system has. `user/src/smb_server.rs` chunks
every SMB read and every SMB write into `filesystem_contract::PAGE`-sized requests, in a loop,
because that is what a `filesystem_contract` request carries:

```rust
let want = (out.len() - done).min(filesystem_contract::PAGE);      // read
let chunk = (data.len() - done).min(filesystem_contract::PAGE);    // write
```

A Mac writing a megabyte into a band file therefore arrives at the store as 256 separate 4 KiB
writes. Each is a read-modify-write of a whole 128 KiB record, with nothing between the two to
coalesce them. There is no cache in the FS server, and a RedoxFS transaction's `write_cache` lives
and dies inside one request.

So the reframing inverts. The large record is not right for the customer path and wrong only for the
benchmark. It is wrong for both, for the same reason: the transfer unit is 4 KiB everywhere in this
system while the record is 32 times that. A 128 KiB record starts to make sense the day a request can
carry one, which is option 1.

One qualification: a band file is written sequentially and grown. That is the cheapest thing a
128 KiB record does, because a growing record doubles its stored level instead of rewriting 128 KiB
from the first page. It is already in the numbers, as the gap between the sequential and random write
columns at every level. It is a discount on a bad price, not a case for the price.

Here is what that costs a real backup, at today's setting and the two one decision away. A
100 GB first backup is 17.6 hours of sequential writing at 1.62 MiB/s, 5.8 hours at option 2's 4.94, and 42
minutes at the 41 MiB/s of options 1 and 2 together. Those are the write path alone, with no network,
no SMB, and no second copy of anything.
