# Steps 1 and 3: the 8 KiB record and the 64 KiB request

This appendix belongs to [milestone 138 (close the read gap)](../0138-file-io-throughput.md). It was
moved out of that block on 2026-10-12 (UTC) under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md), and the block is complete
without it. It holds the measurement and reasoning behind the first two steps built. The tables
behind both are in
[`notes/benchmarks/read-path-record-and-request-size.md`](../../../notes/benchmarks/read-path-record-and-request-size.md).

## Step 1, built and measured: the record is 8 KiB (2026-08-18)

5.13x on a 4 KiB read and 3.01x on a 4 KiB write, measured on [milestone 38 (filesystem
throughput)](../0038-filesystem-throughput.md)'s harness over six interleaved passes at levels 5, 1
and 0. The machine was quiet enough that the `fs_read` control varied 0.6% across every level, so
no normalization was needed. A sequential read went from 1,458,124 ns to 283,974; a sequential
write from 2,399,611 to 796,930.

### Level 1 rather than level 0, verified rather than inherited

The sweep recommended level 1 and this run checked the trade it named. Level 0 reads 8.7% faster
and gives up lz4 entirely, because RedoxFS compresses a record only when it is larger than one
block. It pays roughly twice the space overhead for that (+38% against +19% on text). Sequential
writes are marginally *faster* at level 1. The 8.7% is not the compression. It is the second block
at 39.1 us, which the two-term model predicts without knowing lz4 exists.

### The one-way door is not there any more

The block priced step 1 as irreversible: lowering `RECORD_LEVEL` makes every record stored at a
higher level answer `ENOENT`. That is free today only because nothing is stored. It is free
*because* of the timing, and the timing is not a property anyone can hold on to.

So the change splits the constant instead. `RECORD_LEVEL` is the level a new file is created at (now
1). A new `RECORD_LEVEL_MAX` is the largest level this build can read (still upstream's 5), which is
what the two `BlockTrait::empty` guards compare against. Nothing at any level from 0 to 5 becomes
unreadable, and a future change of the created level cannot orphan what this one wrote. The guards
now compare against a maximum, which is half of what a genuine per-file level would need. It cost
one constant. See vendor/README.md divergence 5.

### The residual

This is what the step was asked to report. It did not shrink at all, and that is the finding. A
read's fixed term is 205,698 ns, unchanged, and it went from 14% of a request to 72% of one. A
write's is 690,085 ns and 87%. Of the read's 206 us, ~195 is step 2's five repeated block reads.
The other ~13 us is the IPC round trip and the server's own work, which is the number this
milestone's four steps never touch. The write residual is the transaction (allocate, rewrite the
node, commit to the header ring, per 4 KiB request). Nothing on the list except step 3 addresses it.
After step 1 it is the largest unaddressed term in the whole measurement.

### What step 2 was worth after step 1

Priced against measurement rather than the model. A read is 283,974 ns; a cache that removed all
five repeated block reads would take it to about 89,000 ns. That is 3.2x again and 16x against
where milestone 138 started. The block modeled 4.7x, and the difference is that the model was built
on level 0's numbers while this shipped at level 1. The block's other claim survives intact and is
now checked: the same cache *before* step 1 would have been worth 15%, exactly as predicted. So the
two are multiplicative and step 1 is what makes step 2 worth building.

### Crash consistency

Re-run at the new geometry, because a safety claim measured at one record size is not automatically
true at another. Same fault-point count, same properties, 0 silently wrong at both levels. Eleven
lying-device cases move from "refused at a read" to "recovered", which is what a smaller record
predicts. It also turned up a stale record: notes/fs-server.md's counts were [milestone 37 (RedoxFS
crash consistency)](../0037-redoxfs-crash-consistency.md)'s and the workload has grown since. So the
table there was wrong before this lane touched it. Corrected in place.

## Step 3, built and measured: a request carries 64 KiB (2026-08-19)

Taken before step 2, deliberately, because step 1's residual said to. A write's fixed term was
690 us per 4 KiB, 87% of the request, and step 2's read cache does not touch a write. A backup is
writes.

8.02x on a sequential write and 5.67x on a read, measured on milestone 38's harness over six
interleaved rounds at each transfer size (`sh bench/transfer-size-sweep.sh 6 1 16`). The machine's
`fs_read` control varied 0.3% between the two points, so no normalization was needed.
`fs_seq_write` went from 5.33 MiB/s to 42.77; `fs_seq_read` from 14.16 to 80.30; random write from
4.34 to 31.38. The benchmark holds bytes moved constant at 1 MiB per phase rather than the transfer
count, so both points move the same file.

### The wire change is a constant

That is the finding about the contract. `fs_proto::fs::TRANSFER_PAGES` is 16 instead of 1, so the
region a client and the FS server share is sixteen contiguous pages and a `READ` or `WRITE` may
carry all of it. Nothing in the packed request word changed. `fs::request` has carried a 40-bit length
since [milestone 32 (a real filesystem)](../0032-redoxfs-fs-server.md), so the packing was
never what limited a transfer to 4096 bytes. The page was. No new opcode, no second reply word, no
descriptor, no changed field, and the shape a reader of `fs_proto` meets is the one already there.

### A single-page client is untouched

This is a property rather than a courtesy. The serve loop clamps twice, and which clamp a verb gets
is the whole compatibility story. A length the client chooses (`READ`, `WRITE`) may use the whole
channel. A length the server chooses (`READDIR`, the attribute verbs, a rename's two names) stays
inside the one page every client has always mapped. A `READDIR` that filled 64 KiB would land in a
single-page client's unmapped second page. `swish`, the three caretakers, the sinks and the `std`
PAL are unmodified and cannot tell this happened.

### What was refused

Milestone 138 authorizes the step and not a shape. A new opcode or a `READV`-shaped scatter list:
unnecessary, since the length field already fits, and a new concept on this contract. A frame
capability granted per request, which is the `mmap`-shaped answer: that is the frontier the block
names, not a transfer size. A negotiated channel size at bind time: a new verb, where every other
agreement between a client and this wiring (the endpoint slot, the VA, the role number) is already a
compile-time constant both sides carry.

### The residual changed owner rather than shrinking

Step 1 reported that the fixed per-request term became the whole problem. Step 3 inverts step 1's
table exactly:

| 4 KiB sequential read | total | fixed term | block term |
|---|---|---|---|
| before step 3 | 275,860 ns | 204,076 (74%) | 71,784 (26%) |
| after, per 64 KiB request | 778,354 ns | 204,076 (26%) | 574,278 (74%) |

A read is now dominated by sixteen single-block trips through `fs_proto::blk`, which is step 4.
`fs_seq_read` measures 80.30 MiB/s against the ~100 MiB/s ceiling that contract imposes. So step 4
stopped being a limitation recorded for later and became the binding constraint.
notes/fs-server.md's `BUGS` entry says so and cites its own promotion trigger.

The write residual moved, and by the most of anything in this milestone. The 690 us transaction is
charged once per request, so it went from 690 us per 4 KiB to 43. That is where the 8.02x comes
from, and it is what step 1 predicted a larger transfer would do.

The two-term model was fitted a second time, from a different variable. Varying the transfer size
gives a fixed term of 204,076 ns and a per-block term of 35.9 us. Step 1 fitted 205,698 and 39.1 by
varying the record level on a different day. The fixed term agrees to 0.8%, and nothing was tuned
to make it.

### Crash consistency, step 3

Re-run and unchanged: 134 fault points, 13 commits, 0 silently wrong, identical to step 1's. It is
honest to say why it could not have changed. The injector drives `Server` on the host, below the
wire, and its workload already contained a 160 KiB single-transaction write. What step 3 changed is
that a client can now cause one; the engine-level case was always covered.

### What step 2 was worth after step 3

Re-priced for the second time. On a 64 KiB read the metadata cache is worth about 1.33x. That is not
the 3.2x step 1 measured and not the block's modeled 4.7x, because the five repeated block reads are
~195 us against a 778 us request instead of a 284 us one. On a 4 KiB read it is still 3.2x. On
writes it is still worth nothing. Steps 2 and 3 target the same term, which the block's
"multiplicative" note did not anticipate. So step 2's value became a function of which request size
the workload uses: a third for [milestone 55 (Time Machine)](../0055-time-machine.md)'s backup,
three times for a small-file workload. (Step 2 then measured more than this, for a reason the model
missed; see [batching-and-cache.md](batching-and-cache.md).)

### What step 3 did not do, and it is one command

The record level was not re-swept at 64 KiB. The 2026-08-18 sweep found that with a multi-page
transfer, levels 4 and 0 cost the identical 837 us per 64 KiB. That predicts step 1's 5.13x does
not survive as a ratio at this transfer size. `sh bench/record-level-sweep.sh 3 0 1 5` with
`TRANSFER_PAGES` at 16 settles it. Until somebody runs it, the level shipped in step 1 is chosen on
4 KiB evidence for a system that no longer moves 4 KiB by default. That run is now
[milestone 329 (the 5.13x record-level number)](../0329-a-record-level-chosen-at-the-wrong-size.md).
