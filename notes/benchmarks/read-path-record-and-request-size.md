# Milestone 138 (close the read gap), steps 1 and 3

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the 8 KiB record and the 64 KiB file
request, measured before and after, with the dates, tables and corrections behind them. Name:
ratified 2026-09-24 (calef), who refused `milestone-138-steps-1-and-3` for this file; [the naming
record](README.md) says why.*

## Step 1 taken: the record is 8 KiB, and 72% of a read is now the part it does not touch (milestone 138, 2026-08-18)

[The record-level sweep](record-level-sweep.md) measured. This is the step that shipped, the first of
milestone 138's four. `RECORD_LEVEL` in the vendored engine goes from 5 to 1. A file this build
creates stores 8 KiB records instead of 128 KiB, and a 4 KiB request stops moving 32 blocks to serve
one.

### Before and after, on a machine that was actually quiet

Six interleaved passes over levels 5, 1 and 0 (`sh bench/record-level-sweep.sh 1 5 1 0`, six times),
on the same harness and the same six phases as milestone 38 (filesystem throughput) and the sweep. The sweep's normalization
is not needed here. The `fs_read` control measured 202,246, 203,389 and 202,352 ns at levels 5, 1 and
0, a spread of 0.6%, so these are raw means of six rounds rather than ratios. Load ran 3.8 to 6.6,
inside milestone 38's own 4 to 9.

ns per 4 KiB, mean of six, MiB/s in brackets:

| phase | before (level 5) | after (level 1) | speedup | level 0, for comparison |
|---|---|---|---|---|
| `fs_seq_read` | 1,458,124 (2.68) | **283,974 (13.76)** | **5.13x** | 261,310 (14.95) |
| `fs_rand_read` | 1,453,688 (2.69) | **282,773 (13.81)** | **5.14x** | 260,966 (14.97) |
| `fs_record_read` | 1,448,318 (2.70) | **281,014 (13.90)** | **5.15x** | 260,582 (14.99) |
| `fs_seq_write` | 2,399,611 (1.63) | **796,930 (4.90)** | **3.01x** | 803,598 (4.86) |
| `fs_rand_write` | 3,289,133 (1.19) | **904,858 (4.32)** | **3.63x** | 872,925 (4.47) |

The model reproduced independently. Fitting `cost = a + b x 2^level` through levels 1 and 5 gives a
fixed term of 205.7 us and a per-block term of 39.1 us for a sequential read. The earlier sweep gave
207.7 and 39.0, from six points on a loaded machine. The write terms land at 690.1/53.4 and 745.9/79.5
against 672.2/53.7 and 769.2/80.0. Nothing needed tuning; they are two runs on two different days.

Level 0 is still above the line and by the same amount: +6.7 to +8.1% across the five phases, against
the sweep's +4.6 to +8.1%. That is the indirect-pointer cost the sweep identified, arriving twice.

### Why level 1 and not level 0, verified rather than inherited

The sweep recommended level 1. Measured here, the trade it named holds:

- Reads are 8.7% slower at level 1 than at level 0 (283,974 against 261,310). The sweep said 8%.
- Sequential writes are marginally faster at level 1 (796,930 against 803,598, 0.8%), and random
  writes 3.7% slower. The sweep saw sequential writes 2.7% faster the same way.
- Level 1 keeps lz4 and level 0 does not, because RedoxFS compresses a record only when it is larger
  than one block. The sweep measured the space cost at +19% at level 1 against +38% at level 0, on
  text. That figure was not re-measured here.

So level 1 buys back half the space for 8.7% of the speed. The 8.7% is not the compression. It is
the second block, at 39.1 us, which the two-term model predicts to within a percent without knowing
anything about lz4.

### The residual: what step 1 leaves behind

Milestone 138 asks every step to report the per-request cost no record level removes. The sum of
those answers whether this architecture has a disk-read liability it cannot overcome. After step 1:

| | total | fixed term | record term |
|---|---|---|---|
| 4 KiB sequential read | 283,974 ns | **205,698 (72%)** | 78,277 (28%) |
| 4 KiB sequential write | 796,930 ns | **690,085 (87%)** | 106,845 (13%) |
| 4 KiB random write | 904,858 ns | **745,907 (82%)** | 158,952 (18%) |

Before this step the record term was 86% of a read (32 blocks at 39.1 us) and the fixed term 14%.
After it the fixed term is 72% and the record 28%. Step 1 did not shrink the residual at all; it made
the residual the whole problem.

That residual is already counted. Of the read's 206 us, about 195 us is five single-block reads of
the same five blocks on every request, which is step 2's target (see [the fixed term](five-blocks-per-request.md)
and `design/roadmap/0138-file-io-throughput.md`). The remaining ~13 us is the file-IPC round trip and
the server's own work. Nothing in milestone 138's four steps removes it, and it puts a fully cached
4 KiB read at about 300 MiB/s.

The write residual is larger and has a different owner. 690 us on a sequential write is the
transaction: allocate, rewrite the node, commit to the header ring, on every 4 KiB request. Step 2's
block cache does not touch it, because those are writes. Step 3's multi-page transfer does, by the
most of anything on the list, because it amortizes one transaction over sixteen pages. After step 1
the write path's fixed cost is 87% of a write and the largest unaddressed term in the measurement.

### What step 2 looks like now, against measured numbers rather than the model

Milestone 138's table modeled step 2 as "on its own worth 15%; with a small record it is 4.7x". With
step 1 measured, that can be restated against real numbers:

- A 4 KiB read is 283,974 ns, of which 205,698 is the fixed term and ~195,000 of that is the five
  repeated block reads. If a cache removed all five, a read would be about 89,000 ns. That is 3.2x
  again, and 16x against where milestone 138 started. It is smaller than the block's 4.7x because the
  block's model used level 0's numbers and this shipped at level 1.
- The same cache before step 1 would have taken a read from 1,458,124 to 1,263,000, which is 15%,
  exactly as the block said. The two steps are multiplicative and step 1 is what makes step 2 worth
  doing, as the block predicted.
- It does not help the write path, whose 690 us residual is a transaction rather than a set of reads.
  Nothing on milestone 138's list addresses that except step 3.

*(Step 3 later re-priced step 2 again, downward on bulk reads; see below. Step 2's measured result is
in [steps 4 and 2](read-path-block-contract-and-metadata-cache.md).)*

### BUGS

- The space cost of level 1 was not re-measured for this step. The +19% is the sweep's, from counting
  non-zero blocks in a fresh image after importing 560 KiB of text, the payload most favorable to
  lz4. An incompressible payload would show only the pointer half. A backup workload is the
  incompressible case and nobody has measured it.
- `fs_record_read` reads at multiples of 128 KiB and the record is now 8 KiB. That is still a record
  boundary, the only property the phase needs, and keeping the constant keeps every figure the phase
  has produced comparable across the sweep. It is no longer named after the record size. Before this
  step `filesystem_contract` called the mismatch "the one soft spot in this module" and nothing checked
  it; `redoxfs_server` now asserts at compile time that the two divide.
- Only levels 5, 1 and 0 were measured here. The six-point sweep establishes the line; this run
  confirms two points on it and one off it, and would not have caught a non-linearity at 2, 3 or 4.
- These are the same 4 KiB transfers milestone 38 chose. Step 3 changes the transfer unit, and then
  none of the ratios here survive as ratios. The sweep showed that at a 64 KiB request every level
  from 0 to 4 costs the same, so step 1's 5.13x describes the contract as it is today, not a
  permanent property of the store.

## Step 3 taken: a file request carries 64 KiB, and the read path lands on the block contract (milestone 138, 2026-08-19)

Taken before step 2, deliberately. Step 1 measured a write's fixed term at 690 us per 4 KiB, 87% of
the request, and step 2's read cache does not touch a write. Only this step does, and a backup is
writes.

`filesystem_contract::fs::TRANSFER_PAGES` goes from an unwritten 1 to 16. The region a client and the
FS server share is 64 KiB of contiguous pages, and a `READ` or `WRITE` may carry all of it in one
request. Nothing in the packed request word changed: the length field has been 40 bits since
milestone 32 (a real filesystem), and the page was always what bounded a transfer.

### Before and after, on a machine that was quiet again

Six interleaved rounds at each point (`sh bench/transfer-size-sweep.sh 6 1 16`), on milestone 38's
harness and the same six phases. The `fs_read` control measured 203,976 ns at one page and 203,326 at
sixteen, a spread of 0.3%, so these are raw means of six rounds with no normalization. Load ran 3.6 to
5.2, inside milestone 38's own 4 to 9. The benchmark holds bytes moved constant (1 MiB per phase)
rather than the transfer count, so both points move the same file.

| phase | 4 KiB per request | 64 KiB per request | speedup |
|---|---|---|---|
| `fs_seq_write` | 732,541 ns (5.33 MiB/s) | **1,461,394 ns (42.77)** | **8.02x** |
| `fs_rand_write` | 899,257 (4.34) | **1,991,506 (31.38)** | **7.22x** |
| `fs_seq_read` | 275,860 (14.16) | **778,354 (80.30)** | **5.67x** |
| `fs_rand_read` | 281,685 (13.87) | **830,341 (75.27)** | **5.43x** |
| `fs_record_read` | 282,167 (13.84) | **840,962 (74.32)** | **5.37x** |

The ns column is per request, and a request is sixteen times larger on the right, so the MiB/s is what
compares. Per 4 KiB of payload a sequential write went from 732,541 ns to 91,337.

### The two-term model reproduced from a different sweep

The record-level sweep fitted `cost = fixed + blocks x per_block` by varying the record size. This one
varies the transfer size, which changes the same two terms through a different variable and could not
be tuned to agree. A 4 KiB read at record level 1 fetches 2 blocks and a 64 KiB read fetches 16, so:

- `275,860 = F + 2B` and `778,354 = F + 16B` give B = 35.9 us and F = 204,076 ns.
- Step 1's fit, from six record levels on a different day, gave 39.1 us and 205,698 ns.

The fixed term agrees to 0.8%, with nothing fitted to make it so. It is the strongest evidence here
that the two-term model is the real shape of a request.

### The residual changed owner

Step 1's residual became the whole problem. Step 3's residual changed owner:

| 4 KiB sequential read | total | fixed term | block term |
|---|---|---|---|
| before (one page) | 275,860 ns | **204,076 (74%)** | 71,784 (26%) |
| after (sixteen pages, per request) | 778,354 ns | 204,076 (**26%**) | **574,278 (74%)** |

That is step 1's table read backwards. A read is now dominated by sixteen single-block trips through
`filesystem_contract::blk`. That is the block contract's one-page limit, which notes/fs-server.md's
BUGS section has recorded as a ~100 MiB/s ceiling since milestone 38. `fs_seq_read` now measures
80.30 MiB/s, 80% of that ceiling.

So nothing left on milestone 138's list moves a bulk read much. The next read win is the block
contract. That is step 4, which was a `BUGS` entry rather than a milestone when this was written.

The write residual moved most. The 690 us transaction (allocate, rewrite the node, commit to the
header ring) costs once per request, so it went from 690 us per 4 KiB to 43 us per 4 KiB. A
sequential write is 8.02x, the largest number in this milestone, as step 1 predicted for the
transaction term under a larger transfer.

### What step 2 is worth now, re-priced a second time

Step 1 re-priced the metadata cache from the block's modeled 4.7x to a measured 3.2x. Step 3
re-prices it downward, because the two steps target the same term:

- On a 64 KiB read it is worth about 1.33x. The five repeated block reads are ~195 us per request,
  and against a 778 us request that is 25% rather than 69%.
- On a 4 KiB read it is still worth about 3.2x, since nothing about that request changed.
- On writes it is still worth nothing, for step 1's reason.

The block's table said steps 1 and 2 were multiplicative and neither worth much alone. It did not say
that step 3 would take most of what step 2 was going to get on the bulk path, and it does. Step 2's
value is now a function of the workload's request size. For the backup of milestone 55 (Time Machine), which reads and
writes in 64 KiB units over SMB, step 2 is worth a third. For a small-file or metadata-heavy workload
it is worth three times. It is still worth building and no longer the headline.

### What was not measured, and is the first thing to run next

The record level was not re-swept at 64 KiB. [The record-level sweep](record-level-sweep.md)
(2026-08-18) found that with a multi-page transfer, levels 4 and 0 cost the identical 837 us per
64 KiB. That predicts step 1's 5.13x does not survive as a ratio at this transfer size. This run holds
the level at 1 and varies only the transfer, so it cannot confirm or refute that. The experiment is
one command: `sh bench/record-level-sweep.sh 3 0 1 5` with `TRANSFER_PAGES` at 16.

### BUGS

- Sixteen timed requests per phase. Bytes moved stay at 1 MiB, which the fixture image bounds, so
  a 64 KiB point is 16 iterations where a 4 KiB point is 256. Each is long enough that counter
  resolution is not the issue, but the sample is small. Standard deviations are 0.5% at one page and
  0.7 to 4.5% at sixteen, and `fs_seq_write` is the noisy one.
- `fs_payload_fill` grew sixteenfold in absolute terms and is unchanged per byte (4,969 MiB/s against
  4,692). It is inside the write phases' timed window, so subtract it: 0.9% of a 64 KiB write, the
  same fraction it was of a 4 KiB one.
- These are still not apples to apples with a buffered Linux read. 64 KiB is the buffer size milestone
  38's ext4 comparison used, so the transfer-size half of the mismatch is now closed and the page-cache
  half is not. Buffered ext4 remains three orders of magnitude away, for a structural reason.
- The write comparison is `O_DSYNC`-shaped, unchanged. Every `filesystem_contract` write still commits
  a RedoxFS transaction before it replies; what changed is how much payload one commit covers.
