# The record-level sweep

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds milestone 138's sweep of the RedoxFS
record size and the two-term model it found, with the dates, tables and corrections behind them.
Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## The record level, swept: what a 128 KiB record actually costs (milestone 138 (close the read gap), 2026-08-18)

Milestone 38 (filesystem throughput) ended on one term: every 4 KiB file request moves 128 KiB, because RedoxFS stores a file
in 128 KiB records and reads a record whole (see [filesystem throughput](filesystem-throughput.md)).
Milestone 138 lists three ways to fix that. None of them can be argued until somebody measures
throughput against the record level, and nobody had. This is that sweep.

Taking the record from 128 KiB down to one block makes a 4 KiB read 5.6 times faster and a 4 KiB
write 3.0 to 3.8 times faster. It does not deliver the 32x, because the 32x was never all of the
cost. A request also pays a fixed ~208 us that no record level touches. Once the record is one block,
that fixed cost is 80% of what remains.

### How it was measured, on a machine that was not quiet

`bench/record-level-sweep.sh` runs on milestone 38's existing harness: the same six phases of
`fs_test_client`'s throughput role, the same 256 transfers of 4 KiB per phase, the same fixed-seed
offsets, the same fresh incompressible payload per write. Every figure here is comparable to
[milestone 38's table](filesystem-throughput.md).

Nothing can ask for a level, so the script sets one. It edits `RECORD_LEVEL` in
`vendor/redoxfs/src/lib.rs`, rebuilds, runs, and puts the constant back. It refuses to start if that
file is already dirty, so it cannot restore over an edit it did not make. `cargo xtask bench
--release --real --smp` regenerates the RedoxFS image on every run, so each point is a whole
filesystem built at that level. This sweep ran against a committed value of 5, and it measures and
does not decide. *(Step 1 of milestone 138 (close the read gap) sets `RECORD_LEVEL` to 1; see
[steps 1 and 3](read-path-record-and-request-size.md).)*

The machine was busy, so the control does more here than select rounds. Twenty passes ran over the
six levels, interleaved (one pass sweeps 0 through 5, then the next). The host sat at load averages of
5.5 to 21 with three other lanes gating. That is outside the load 4 to 9 where milestone 38 took its
figures. Keeping only rounds that pass a 2% control would have left one or two per level.

So the primary figure below is a ratio: each phase divided by the same round's `fs_read`. `fs_read`
reads `motd`, which is 69 bytes and lives inline in its node. It fetches no record, so its cost is
independent of the level swept. That is a property of the code (`read_node_inner` returns from the
inline branch before it reads `record_level()`). It is visible in the data too: `fs_read` measures
203 to 208 us at every level on every quiet round. Dividing by it cancels whatever the host was doing
to the guest that second.

Two runs at an ordinary load show the normalization invents nothing. Before the sweep, at load 6.9
and 7.3, single runs at level 5 and level 0 gave a sequential read of 1,466,327 ns and 257,893 ns.
The ratio method over twenty passes at loads from 5.5 to 21 gives 1,453,963 and 258,582: 0.8% and
0.3% apart. The raw minimums sit beside the normalized figures. Where the two disagree, the
disagreement is the noise.

### The sweep

ns per 4 KiB, with MiB/s in brackets. Level 5 is the tree's shipped value, so that row is milestone
38's table measured again by a different method on the same day; it agrees to within 6%.

| record level | record | seq read | rand read | record read | seq write | rand write |
|---|---|---|---|---|---|---|
| **0** | 4 KiB | 258,582 (15.1) | 261,918 (14.9) | 262,883 (14.9) | 790,317 (4.9) | 881,395 (4.4) |
| **1** | 8 KiB | 280,262 (13.9) | 281,607 (13.9) | 279,648 (14.0) | 769,266 (5.1) | 911,009 (4.3) |
| **2** | 16 KiB | 355,002 (11.0) | 359,628 (10.9) | 359,342 (10.9) | 870,776 (4.5) | 1,079,208 (3.6) |
| **3** | 32 KiB | 517,314 (7.6) | 521,923 (7.5) | 518,700 (7.5) | 1,058,267 (3.7) | 1,398,535 (2.8) |
| **4** | 64 KiB | 836,690 (4.7) | 839,899 (4.7) | 840,427 (4.6) | 1,520,484 (2.6) | 2,056,411 (1.9) |
| **5** | 128 KiB | 1,453,963 (2.7) | 1,458,916 (2.7) | 1,458,735 (2.7) | 2,408,470 (1.6) | 3,331,724 (1.2) |

The raw minimum of every round at each level, with no normalization, in ns:

| record level | seq read | rand read | record read | seq write | rand write |
|---|---|---|---|---|---|
| 0 | 256,296 | 251,037 | 251,311 | 767,722 | 868,865 |
| 1 | 268,126 | 268,806 | 264,532 | 720,203 | 879,846 |
| 2 | 338,488 | 333,764 | 336,788 | 805,264 | 1,018,608 |
| 3 | 506,940 | 512,699 | 513,431 | 1,043,946 | 1,367,542 |
| 4 | 810,257 | 830,180 | 823,514 | 1,505,937 | 2,016,396 |
| 5 | 1,445,499 | 1,462,345 | 1,399,981 | 2,357,163 | 3,260,817 |

### One straight line fits all six points

The three read phases measure the same thing at every level: milestone 38's no-cache finding holds at
every record size. Fitting `cost = a + b x 2^level` by least squares:

| phase | `a`, fixed per request | `b`, per 4 KiB block | residual at levels 0..5 |
|---|---|---|---|
| `fs_seq_read` | 207,679 ns | 38,980 ns | +4.6 -1.9 -2.4 -0.4 +0.6 -0.1 % |
| `fs_rand_read` | 210,769 ns | 39,036 ns | +4.6 -2.6 -2.0 -0.2 +0.5 -0.1 % |
| `fs_record_read` | 209,832 ns | 39,059 ns | +5.3 -3.0 -1.9 -0.7 +0.7 -0.1 % |
| `fs_seq_write` | 672,199 ns | 53,720 ns | +8.1 -1.3 -1.9 -4.1 -0.7 +0.7 % |
| `fs_rand_write` | 769,202 ns | 80,049 ns | +3.6 -2.0 -0.9 -0.8 +0.3 +0.0 % |

Read a request as two terms: about 208 us the record level does not touch, plus 39.0 us for every
4 KiB block the record holds. At level 5 the second term is 32 blocks and swamps the first. At level
0 it is one block, and the first term is 80% of the total.

That corrects a constant milestone 38 quoted. Its 46.2 us per block came from dividing one
measurement by 32, so it charged the per-request metadata walk to the blocks. It is an average, not a
marginal cost. The marginal cost of a block through the confined block server is 39.0 us, and the
per-request metadata walk is a separate 208 us, about 5.3 blocks at that price. The sweep can
separate the two because it has six points and a slope. The parity claim survives and gets slightly
stronger: 39.0 us against Linux's 38.7 to 53.3 us for a raw 4 KiB virtio read at this tier.

The writes decompose the same way. A random write's slope is 80.0 us per block, 2.05 times the read
slope: the record is read and then written, as copy-on-write says. A sequential write's slope is
lower, 53.7 us, because a growing record doubles its stored level rather than rewriting a full one.
That is the mechanism behind milestone 38's "a sequential write is 55 blocks and a random write is
74". Both write intercepts are far larger than the read's, 672 us and 769 us against 208. That is the
transaction, which allocates, rewrites the node and commits to the header ring on every request. No
record level touches it either.

The one place the fit is visibly wrong is level 0. Its reads land about 5% above the line in all
three read phases. That is the metadata cost of small records. A node carries 128 direct record
pointers, so at level 0 they address the first 512 KiB of a file. The throughput file is 1 MiB, so
half of its reads need an indirect block read first. At level 5 the whole file is eight direct
pointers and there is no indirection.

### What this says about milestone 138's three options

| | read | seq write | rand write | against today |
|---|---|---|---|---|
| **today**: 4 KiB request, 128 KiB record | 2.7 MiB/s | 1.6 | 1.2 | 1x |
| **option 2 alone**: 4 KiB request, one-block record | **15.1** | 4.9 | 4.4 | 5.6x read, 3.0x write |
| **option 1 alone**: 64 KiB request, 128 KiB record | **43** | 26 | | 16x |
| **options 1 and 2**: 64 KiB request, 64 KiB record | **75** | 41 | | 28x |
| the block path's own ceiling | ~100 | | | 37x |
| ext4 `O_DIRECT` at a 64 KiB unit, same tier | ~940 | ~940 | ~900 | |

Only the first two rows are measurements. Rows three and four are the measured cost of one request
that fetches one record of that size. That is what a 64 KiB request would cost if the contract could
carry one. The derivation rests on something milestone 38 measured: `read_record` reads the block
the pointer stores and only then checks the level asked for. So the bytes a request asks for do not
change what the store fetches. `fs_payload_fill`, which paints a page in 820 ns, bounds the cost of
moving 64 KiB rather than 4 KiB into the client's pages: sixteen pages is 13 us against 837, under
2%.

- Option 2, a record level matched to the transfer unit: 5.6x on reads and 3.0x on writes, measured.
  It is the only one of the three that needs no agreement between two programs. Its costs are in
  [the five-blocks-per-request appendix](five-blocks-per-request.md), and the largest of them is not on milestone 138's
  list.
- Option 1, a multi-page transfer on the file contract: 16x on its own, more than option 2 buys. It
  amortizes both terms of the model over sixteen times the payload, not only the record term. It is a
  wire change.
- Both: 28x, the combination worth wanting. Once a request carries 64 KiB, level 4 and level 0 cost
  the same 837 us for that 64 KiB. The fixed cost is per request, and the block count is identical
  either way. With a multi-page transfer the record level stops mattering for aligned bulk IO. Option
  2 then collapses into "do not fetch more than the request asked for", which every level from 0 to 4
  satisfies.
- Option 3, replace the store: this sweep gives it nothing. The 32x behind that blame is a parameter
  the format already carries. What survives once that parameter drops is 208 us of RedoxFS
  re-reading its own metadata from the device on every request, about 5.3 block reads. That is the
  absence of a cache, which milestone 138 puts out of scope on purpose. Swapping the store to acquire
  one is a strictly larger change than adding one. **Nothing measured here is evidence that RedoxFS
  is the problem.**

The wall behind all three is new. `IpcDisk::read_at` chunks every record into one
`filesystem_contract::blk` request per 4 KiB block, because the block contract shares exactly one
page with the block server, the same limit the file contract has. A 128 KiB record read is therefore
32 device round trips rather than one 128 KiB transfer. So 39.0 us is the price of a round trip, not
of the bytes. At that price no request size and no record level can exceed about 100 MiB/s. Linux
moves 64 KiB through one virtio request for 67 us at this same tier. The remaining order of magnitude
lives there, one layer below the one milestone 138 is about.

### BUGS

- Nothing here was measured at a level other than the tree's own in a build anyone kept. Every
  point except level 5 came from a throwaway build with the vendored constant edited. The whole
  filesystem in that build used that level. A per-file level, the shape option 2 would actually ship,
  has never been built or measured.
- The only correctness these runs prove is the harness's own. The throughput client checks the bytes
  it read back and the file's length after each phase, and every level passed at every round. But
  `script/test` was not run at any level below 5. A level that measures well is not thereby a level
  the confinement, crash and recovery suites pass at.
- The record-aligned phase stays aligned by luck rather than by design. `fs_record_read` reads at
  multiples of 128 KiB, a multiple of every record size at or below level 5, so it means the same
  thing at every point in this sweep. It would stop meaning it if anyone swept above level 5, and
  nothing checks that; `filesystem_contract::fixture::throughput::RECORD` says so in its own comment.
- The machine was not quiet, and the headline figures are the normalized ones, not raw. The method
  and the raw minimums are above. Both agree with the two runs taken at an ordinary load, which is
  weaker evidence than a quiet machine would have been.
- Option 1 has a derived price, not a measured one, because no request in this system could carry
  more than a page. The assumption it rests on appears where the price does, and milestone 38
  measured it.
- The space figures in [the five-blocks-per-request appendix](five-blocks-per-request.md) count non-zero blocks in a fresh
  image, which is not the allocator's own answer. A block a record has vacated keeps its old bytes, so
  the count would drift upward on a rewritten image. These images were made, imported into once and
  never written again, the case where count and allocation agree. RedoxFS's own free-block count
  (`filesystem_contract::fs::STATFS`) would be exact and needs a guest or a verb the host tool lacks.
