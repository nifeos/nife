# The RedoxFS filesystem server (milestone 32 phase 2)

A real copy-on-write filesystem we did not write, RedoxFS, running confined as a userspace
component and served over a capability-shaped contract. This is the flagship userspace-reuse
story the prior-art survey predicted (notes/prior-art.md, notes/redoxfs-audit.md): the kernel
confines a serious component it knows nothing about, and the thing milestone 31's per-file grants
point at (they now exist; see the caretaker section below).

The written contract lives with its code in `crates/filesystem_contract`, the way the terminal contract lives
in `line_editor::proto` (notes/terminal-contract.md). This note is the design around it.

The evidence behind several sections is in [`fs-server/`](fs-server/README.md), one appendix per
topic, and each section links the one it summarizes.

## Three processes, two protocols

```text
  disk ──virtio──►┌──────────────┐──blk IPC──►┌───────────┐──file IPC──► client
                  │ block server │            │ FS server │
                  └──────────────┘◄───────────└───────────┘◄──────────── (holds a directory cap)
                       owns the DMA             owns RedoxFS +
                       confinement              its own heap
```

Nobody names anyone else (endpoint-only naming, notes/ipc-naming.md). The FS server holds "an
endpoint I read and write blocks on"; the client holds "an endpoint that opens and reads files
under the one directory this endpoint is bound to." Rewire the endpoints and neither side can tell,
which is milestone 23's hot-swap claim in component form.

- **The block server** is a role of the virtio driver (`crates/virtio`'s `run_blk_server`, dispatched by `crates/virtio`). It
  brings the RedoxFS disk up, then serves read/write/size over blk IPC forever. The device
  confinement is unchanged from any driver (the kernel owns the transport and validates every DMA
  descriptor, notes/dma.md), so a serving block driver is as confined as a reading one.
- **The FS server** (`redoxfs_server/`, its own workspace because it links the vendored engine) runs the
  no_std RedoxFS core behind a `Disk` trait implemented over blk IPC, allocating everything from its
  own untyped budget through the milestone-27 `GlobalAlloc`. It serves file IPC to clients.
- **The client** (`fixtures/src/fs_test_client/main.rs`) is the program a milestone-31 shell will be: it holds only
  a directory capability and opens files by name under it.

The kernel wires all three (`kernel/src/user/fs_service.rs`), handing each a `Spawn` literal that
is its entire authority. The kernel never sees a filesystem operation, an opcode, or a byte of file
data.

## The contract is capability-shaped from birth

- The endpoint IS the directory capability. A client reaches the FS server only by holding an
  endpoint the server reads on, and that endpoint is bound, in the server, to one directory node
  (the image root, in phase 2). `Server::open_file` resolves every name in an `OPEN` *under that
  bound directory*: no absolute path, no `..` escape, no global namespace. A client with no such
  endpoint can open nothing, and the refusal is "you hold no such capability", not a permission
  check. Milestone 31 binds new endpoints to subdirectories or single files and hands them out as
  grant expressions; the server code already carries the bound-directory seam.
- A handle is a server-minted token. A successful `OPEN` returns a small integer the server
  issued and validates against this session's table (`Server::node`). Forging one is meaningless:
  the server honors only the handles it minted, in exactly one place.
- Open-by-path exists only inside the server. The client sends a name; the server resolves it.
  There is no path walking on the wire and no way to designate a file the granted directory does not
  contain.

## The error boundary, mapped exactly once

RedoxFS speaks its own error type everywhere (`syscall::error::Error`, redox_syscall's errno). The
sans-IO core (`redoxfs_server::Server`) and the `Disk` impl both return `syscall::error::Result`,
unmapped. The translation to the wire happens in one place, the FS server's serve loop
(`redoxfs_server.rs::serve`), via `filesystem_contract::reply_err`, which is just the negated errno. The client
inverts it with `filesystem_contract::reply_errno`. Keeping the core in RedoxFS's vocabulary is what makes the
rule enforceable: there is no ABI type below the boundary to leak. The blk IPC has the same
convention (a negative reply is a negated errno), and the `Disk` impl maps a negative blk reply to
`Error::new(EIO)`, which is the trait's own vocabulary, not an ABI leak.

## The block server, and how it completes a request

A mount is hundreds of block reads, because RedoxFS scans a 256-entry header ring. Two choices keep
that inside the watchdog. Each virtio request moves whole 4 KiB blocks, up to sixteen since
milestone 138 (close the read gap: a 4 KiB request must stop moving 128 KiB), DMA'd straight into the
region shared with the FS server. And the block server waits on the completion interrupt rather than
polling; a note that once said otherwise was overruled by the machine.

One hazard is real: QEMU's `virt` assigns virtio-mmio slots in reverse command-line order, so the
runners put the nifefs disk last to keep it at slot 0. Getting this backwards silently hands the
phase-1 tests the wrong disk. [`fs-server/block-server.md`](fs-server/block-server.md) has the
details and the correction.

## The FS server's stack is sized by measurement, because guessing it cost a day

RedoxFS recurses, 8 KiB a frame, so the FS server gets 96 extra stack pages. A 32-page grant ran out
528 bytes short when `CREATE` and `TRUNCATE` arrived. The downstream instruments hid it for a
day: the client blocked forever, the heartbeat saw other spinners, and the budget ceiling reported
the budget rather than the cost. Now the kernel poisons the stack and measures the high-water
(about 128 to 136 KiB of a 388 KiB grant), and a test fails under a quarter left. A client of a dead
server still blocks forever. [`fs-server/stack-sizing.md`](fs-server/stack-sizing.md) has the
crash, the table and what is still open.

## Crash consistency, measured (milestone 37)

The property is prefix consistency: a fresh mount after a crash at any point recovers exactly the
first `p` acknowledged operations, and `p` never goes backwards. A host injector replays the
recorded write log to every fault point. Every power cut and every torn last write is
prefix-consistent. A lying device that drops or tears one write is caught or recovered and never
silently wrong, because every block pointer carries a seahash. Our block server now issues a real
flush (`fs::SYNC`), and ordering between writes stays the device's job. A device-level test tears a
real virtio write inside a real FS-server process and recovers it with a second one.
[`fs-server/crash-consistency.md`](fs-server/crash-consistency.md) has the tables, the controls and
the corrections.

## Never create on-device

The std-gated core APIs are exactly creation (`FileSystem::create`, uuid v4, getrandom). The FS
server only ever OPENS an image; entropy never becomes a *server* dependency. Test images are made
host-side by `tools/redoxfs_host` with the same pinned engine that serves them (roadmap §32 port
plan item 4). The host tool's `mkfs` was also fixed to start from an empty file: `DiskFile::create`
opens without truncating, so `mkfs` over an existing image left stale blocks past the new write and
produced an image that failed to open. Removing the file first makes it idempotent, which the test
flow relies on (it regenerates the image every run).

## `mkfs` on the target (milestone 57 (partitioning and formatting a real drive, and extended attributes))

The *server* never creates. A separate program in the same package, `mkfs` (provisional name), does,
on the target, holding only a block endpoint and an entropy endpoint. The engine needed one
divergence: the disk UUID is a parameter (`create_reserved_with_uuid`), because a `no_std` engine has
no randomness, so no randomness enters vendored code. It formats a partition, refuses one that is not
4096-aligned at both ends, and stamps every timestamp zero because it holds no clock.
[`fs-server/mkfs-on-target.md`](fs-server/mkfs-on-target.md) has the first attempt that did not
work and why.

## What is proven

Proven end to end, on both ISAs (the parity gate, DECISIONS §19 (architectural parity is a tenet)),
is the whole chain. A block server drives a host-made RedoxFS image over DMA. An FS server mounts
it over blk IPC and serves from its own heap. A client opens the shipped `motd` through a granted
directory capability and reads it back byte for byte. The engine mounts, the contract holds, the confinement holds. The sans-IO core is host-tested
for both read and write against a `DiskMemory` image, so the filesystem *logic* is proven on both
paths independently of any device.

The write path is proven on device too. A write that seemed to loop for three rounds of
investigation was never a filesystem bug: a write shorter than the file does not truncate it, and a
test compared a whole-file read across boots. A test now pins that semantics.
[`fs-server/write-path-history.md`](fs-server/write-path-history.md) has the three rounds, the
salvaged branch, and the two hypotheses that died.

## The write path is complete: `CREATE`, `TRUNCATE`, and one rule that was ours

`CREATE` (opcode 6) resolves a name under the bound directory and makes it, returning a handle.
`EEXIST` if the name is already there, and nothing is modified: create is create, not
create-or-open. A caller that wants either has to ask for both and say which it got, because the
alternative silently makes a partly-working write look like a working one, which is exactly the
failure §27 records.

`TRUNCATE` (opcode 7) sets a file's size in both directions: growing extends with zeroes,
shrinking discards. The shrink is the point. The new size rides in the *second word*, not the length
field, because the length field is what the serve loop clamps to the shared region and it would have
silently capped every truncate at the region's size. That was 4096 bytes when this was written and is
64 KiB now (milestone 138 step 3), which changes the number and not the argument. A size is an
offset-shaped quantity, and does not belong in a field bounded by how much a request can carry.

Adding `CREATE` surfaced a rule that had been true by accident. RedoxFS's `check_name` rejects `:`,
over-long names, and duplicates; `/`, `.` and `..` pass straight through. Nothing walked paths, so
nothing escaped, and the "one component, no `..`" invariant held by the *absence of a walker* rather
than by a check. With `CREATE` a client could write one: `create_file("../escape")` made a directory
entry literally named that. Still not a traversal, and still a landmine, because the moment anything
does walk paths (a per-directory grant, the host tool, the image mounted through Redox's FUSE driver)
that entry means something it was never allowed to mean. `check_component` now enforces it at our
boundary, deliberately there and not patched into the vendored engine: it is a rule of *this*
contract, not a bug in RedoxFS, whose other callers may name entries whatever they like.

## Extended attributes are a layer on top, not a change to the engine (milestone 57)

The same loop serves `GETXATTR`, `SETXATTR`, `LISTXATTR` and `REMOVEXATTR` over the same endpoint,
and RedoxFS knows nothing about any of them. The server keeps one blob per node in a reserved
directory of the image, keyed on the node's `TreePtr` id. Three things that belong in this note
rather than in [xattr.md](xattr.md), because they are facts about *this* server:

- The transaction guarantee is what makes it safe. Every mutation runs inside one `fs.tx`, so an
  attribute and the file it is on reach the platter in one commit. That was the check DECISIONS §34
  required before the layer was viable, and it is the same property `RENAME`'s crash atomicity rests
  on.
- The purge asks the engine. `remove_node` answers `Some(id)` exactly when a node's last link
  went, so `unlink` and `rmdir` take the attributes away on `Some` and never have to decide for
  themselves. `rename_node` is the exception: it removes a replaced destination and discards the
  freed id, so `Server::rename` notices that one for itself.
- `check_component` and `read_dir` enforce the reserved name at our boundary, for the same reason
  the one-component rule is ours. It is a rule of *this* contract, not a bug in RedoxFS, whose other
  callers may name entries whatever they like.

## A per-file grant: the caretaker between the directory and the program

Milestone 31's `run wc report.txt` grants one file, and the unit of authority here is a *directory*.
`components/src/fs_file_caretaker.rs` is the difference: a caretaker process that holds the directory
capability, opens the granted name once, and serves this same contract on its own endpoint with a
namespace of exactly one name and a direction it cannot widen. The design, the three refusals, and
the two attacker witnesses that prove it are written up in
[grant-expression.md](grant-expression.md); the part that belongs here is why it is a process:

This server receives on one endpoint. Serving a second, narrower one would need a receive over a
*set* of endpoints, which the kernel does not offer, and the way to add it is to badge endpoint
capabilities (seL4's answer). That is a design fork, recorded rather than taken. The caretaker needs
nothing new: it is an ordinary client of this contract above, and an ordinary server of it below. The
"bound directory" seam this note has always advertised for milestone 31 turned out to be used from
the *other* side: the caretaker binds nothing new here, it just never asks for more than one name.

## The verb table (milestone 61 (the caretakers: one verb table))

`filesystem_contract::verb::TABLE` holds one row per opcode, and a `const assert!` makes adding an
opcode without a row a compile error. So a new verb reaches the three caretakers or fails the build.
The four extended-attribute verbs had silently been missing from all three before it. The table
shares the dispatch, never the attenuation. Its `Operand::Name` versus `Operand::Payload` split is
what keeps a name-set capability from escaping its set.
[`fs-server/verb-table.md`](fs-server/verb-table.md) has the fields.

## `STATFS` (milestone 54 (a network file service a Mac can actually mount))

Op 18 answers how much room the store behind a capability has: the allocation unit, the volume size
and the free count, as a record whose length is its version. It demands no right, so a write-only
grant can ask whether its next write fits. It answers about the whole image, never a subtree, and
free space is a forecast. [`fs-server/statfs.md`](fs-server/statfs.md) has the wire and the choices.

## Throughput (milestone 38 (filesystem throughput, and the comparison))

A request costs a fixed ~204 us tree walk plus 39 us per block. The record is now 8 KiB and the
transfer unit 64 KiB, which took a 4 KiB read from 1,458 us to 284 us and sequential throughput to
80.30 MiB/s read and 42.77 MiB/s write. A small write-through cache now answers the repeated tree
walk, so a warm `fs_read` is 22x faster than a cold one. Every earlier number was taken against the
uncached build. A client may still ask for more than it mapped and fault on its own page.
[`fs-server/throughput.md`](fs-server/throughput.md) has the mechanism, the measurements and the
throughput BUGS.

## For later milestones

- **31 (capability shell)** is done as a mechanism: per-file grants exist, proven on both ISAs by
  a read-only and a writable attacker. What is left is the interactive boot wiring an FS service so
  the shell holds a directory to narrow; see grant-expression.md.
- **27 (`std::fs`)** is **done**: the PAL binds `File` to this contract, and the endpoint's bound
  directory becomes the thing `File::open`'s path resolves under, so a path that would leave it is
  refused rather than served. The std program holds the endpoint at slot 4 of the std slot convention
  and nothing else that names a filesystem. A program handed a *narrowed* endpoint instead needs no
  std change at all: the one granted name opens and every other is `NotFound`. See notes/std/fs.md and
  notes/abi.md §4.
- 23 (live replacement) gets its hardest state-handoff case here: an FS server with open handles
  and in-flight writes is the "serialise-old / absorb-new" problem the console swap never had. It
  also owns the open item above: a client of a dead server blocks forever, and §26's fault endpoint
  is the mechanism that turns that into a message a supervisor can act on.
