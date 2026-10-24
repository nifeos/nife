# The server core's caches and IO: the memo, the block layer, the dispatch and the binary

Name: the file's provisional, minted 2026-10-24 (UTC) by `comments/929-redoxfs-server`; calef names
files, and the directory's own block in [README.md](README.md) covers the stem.

An appendix to [notes/fs-server.md](../fs-server.md), beside [the-core.md](the-core.md), which has
the handle table, names, verbs and the departures from upstream RedoxFS. This one carries the design
behind the memo, the block cache, the wire shapes `Server::handle` decodes and the EL0 binary's IO.

## The memo

`memo.rs` keeps listings, names found one at a time, and small files' bytes until something changes
them. [notes/walk-cost.md](../walk-cost.md) has the measurements and what it bought. Before the memo,
`walk_model` measured 1,259 requests and 6.4 ms of server time per warm walk on an M-series core.
Seahash was 40% of that and block copies most of the rest, because RedoxFS answers each lookup by
walking its node tree from the root. The budgets are sized for the server's 8 MiB heap.

The rule that keeps it correct: the server is its image's only writer, and every verb that changes
the image goes through `change`, which forgets everything, or `change_file`, which forgets one file.
Read-only verbs go through `look`. Forgetting everything on a namespace change is coarse on purpose:
invalidation is one line with nothing to get wrong, and a change is rare next to the reads of a walk.

## The block layer

`BlockIo` is a whole-block transport, and `BlockDisk` bridges RedoxFS's byte-addressed `Disk` calls
to it. The chunking is written once and host-tested because the repeat-write bug hid in chunking that
lived only in the EL0 binary. A short final chunk is read, overlaid and written back whole. That
preserves the bytes past the chunk, as a byte-addressed disk does, and RedoxFS relies on it for
compressed records.

`CachedDisk` is a direct-mapped, write-through cache of single-block reads (milestone 138 (close the
read gap) step 2). An LRU would hit more often at the same size, at the cost of a recency list updated
on every hit. The simpler cache costs nothing measurable while the walk fixture's whole metadata
working set fits. Its other rules:

- A slot's 4 KiB is allocated when a block first lands in it. The ordinary server has 8 MiB of heap
  and a crash-test server 2 MiB, and one slot count serves both because a server pays only for what
  it touched.
- The header ring is never cached. Its 256 blocks are read once at mount and then only written, one
  per commit. Caching them filled every slot they mapped to at mount, and on a 2 MiB server would have
  spent half the heap. The server opens images whose filesystem starts at block 0, so the ring is
  blocks `0..HEADER_RING`.
- Only a one-block read consults the cache. A record body is read once per access, so caching it
  would spend the heap for little repetition.
- A write updates the cache only after the inner write succeeded. A short final chunk is merged below
  this type, so its slot is invalidated instead.
- No generation counter is needed. RedoxFS's copy-on-write allocator never rewrites a live block in
  place, so a cached block goes stale only by being written, and `write_at` is the only way a write
  reaches the platter.

`CACHE_SLOTS` is 256 by measurement, taken after a namespace change forgets the memo. The next walk
of the walk fixture then went to the device 372 times at 64 slots, 212 at 128, 71 at 192 and 2 at
256. The tree's metadata working set is 207 blocks. A larger tree wants more.

`block.rs` also holds `const` assertions tying the wire to the store, because it is the one place that
sees both: `filesystem_contract` depends on nothing and the engine has never heard of it. They are
`const` so that they fire in the EL0 build, where no test runs. They check that the transfer block is
the store's block, that the throughput fixture's `RECORD` is a record boundary, that the created
record level is readable, and that the file channel is a whole number of blocks. A channel ending part
way through a block would make the last chunk of every full-channel request partial, the shape the
repeat-write bug had.

## What the dispatch decodes

`Server::handle` is the only place a RedoxFS error becomes a wire value. Its two clamps are in
[throughput.md](throughput.md): a length the server chooses stays inside one page, and only `READ` and
`WRITE`, whose length the client chooses, may use the whole channel.

The second request word carries whatever does not belong in the clamped length field. `OPEN` puts the
rights each step of a path asks for there (ruling A). `OPENDIR` and `MKDIR` put the rights the child
should have, `READDIR` its cursor, `TRUNCATE` the new size and `SETMTIME_AT` the asserted seconds.
`RENAME` packs the destination's handle and length, and `SETXATTR` the value's length and type code.
A size, a cursor or a time is not a payload length, and the page clamp would silently cap it.

When two payloads share the page (`RENAME`'s two names, `SETXATTR`'s name and value), an overrun is
`EINVAL` rather than a clamp. Clipping a name renames something else, and clipping a value stores
something nobody wrote.

`OPEN`'s reply carries the file's size in its second word (ruling B), so a client reading a whole
file needs no `FSTAT`. `STATFS` is encoded in the dispatch rather than the core, which keeps the
core's verbs free of the wire layout.

`SYNC`'s reply is the block server's own word, a count of completed flushes, so a client can prove each
sync was a fresh round trip. A negative is passed through unmapped, the one exception to mapping
errors once: `EOPNOTSUPP` from a device with no flush and `EIO` from one that refused are different
facts. A sync-only badge (milestone 805 (`reboot` at the prompt)) is decided before any handle is
read, because it names no directory and the sync is of the whole device.

## The EL0 binary

`IpcDisk` batches whole blocks, up to `blk::TRANSFER_BLOCKS` per `CALL` (milestone 138 step 4).
Every call pays a fixed round trip, and the device moves the batch in one virtio descriptor. Batching
does not touch the five tree-spine reads, which the engine issues as separate one-block calls;
`CachedDisk` removes those.

`write_at` takes the one-block-per-call path whenever the crash injector is armed or `VERIFY_WRITES`
is on. The injector lets a set number of block writes through and tears the next, which a batched
virtio request cannot do because the device completes a descriptor whole. The readback needs one call
per block too. Neither costs the ordinary path anything. The injector's arming and purpose are in
[crash-consistency.md](crash-consistency.md).

`VERIFY_WRITES` reads every written block back. It is off because it doubles the write cost, and its
4 KiB scratch block sits on the stack inside a call RedoxFS makes from deep recursion. That can
overflow the server's stack and produce a different failure from the one being chased, so raising the
stack grant goes with turning it on.

A badge at or past `fs::CLIENT_WINDOWS` would point outside the mapped windows. `admit` refuses it
first (milestone 726 (an unknown badge fails closed in subtree_scope)); `window_base` folds it onto
window 0 as a second line, which fails loudly rather than reading unmapped memory.

The heap cap is 8 MiB. RedoxFS's compress buffer is sized by `RECORD_SIZE` (128 KiB), which stays
the ceiling after milestone 138 lowered the created record level, because the buffer must fit any
record this build can rewrite.
