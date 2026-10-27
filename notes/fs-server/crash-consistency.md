# Crash consistency, measured

An appendix to [notes/fs-server.md](../fs-server.md). It holds milestone 37 (prove RedoxFS's crash
consistency): the property, the host injector and its results, the controls, the durability half
milestone 55 closed, and the device-level test.

## From a claim to a measurement

"RedoxFS is copy-on-write, so crash consistency is designed in" is a description of somebody else's
design document. DECISIONS §34 (RedoxFS is
the primary filesystem, on three conditions) made it a condition rather than a claim for exactly that
reason. It is now a measurement.

The property, stated so it can fail. A workload is operations, each acknowledged only after the
engine commits it. Call the filesystem after the first `p` of them `S(p)`. For every point at which
the device could stop, a fresh mount recovers exactly `S(p)` for some `p`. `p` never goes backwards
as the cut advances. And where nothing is lost, `p` is the whole workload. That is prefix
consistency, and "an acknowledged write is wholly present or wholly absent" is a consequence of it.

## How the injector works, and why it is not an approximation

The seam is `BlockIo`, the trait the FS server reaches its disk through. On device it is `IpcDisk`
calling the block server; on the host it is a `Vec`. `redoxfs_server/src/crash.rs` runs the workload
once, against a recorder that applies every block write and appends it to an ordered log. That log
is what the platter was asked to do. So the disk after a failure at point `i` is the pristine image
plus the first `i` entries, optionally with one of them truncated.

For a device that does not reorder, that is not a model of a crash; it *is* the crash. It costs a
`memcpy` per fault point instead of a re-run of the engine. It also makes every fault point start
from a byte-identical image. That matters here more than anywhere: a crash harness that leaves state
behind between runs produces the exact class of false result
[stack-sizing.md](stack-sizing.md) spends 60 lines on.

| injection | fault points | result at the 8 KiB record | result at the 128 KiB record |
|---|---|---|---|
| power cut, at every write | 134 | all prefix-consistent | all prefix-consistent |
| power cut, last write torn, 4 offsets | 536 | all prefix-consistent | all prefix-consistent |
| a lying device (drop or tear one write, keep persisting after) | 268 | 196 recovered, 72 refused, **0 silently wrong** | 185 recovered, 83 refused, **0 silently wrong** |

Both columns were re-run on 2026-08-18, when milestone 138 (close the read gap: a 4 KiB request must
stop moving 128 KiB) took the record level from 5 to 1. A store's safety claim measured at one
geometry is not automatically true at another. The property is unchanged and the fault-point count
is identical. What moves is *where* the lying device is caught, and it moves the harmless way. Eleven
cases go from "refused at a read" to "recovered", which is what a smaller record predicts: a dropped
write damages less. 0 silently wrong in both.

A correction this re-run forced, and it is not about the record level. Milestone 37's counts were
93, 372 and 186, with 112 recovered and 74 refused, and the workload has grown since: the same suite at the *old* record level counts 134 fault points today, not 93. Nothing
was checking them, because they live in prose, and the tests assert the property rather than the
count. That is rung four working as badly as rung four works. The property is gated, and the
arithmetic describing it is not.

## The honest limit, and the half milestone 55 closed

The third row is the honest limit. RedoxFS's `Disk` trait has no flush and no barrier, so ordering is
the device's job. A device that acknowledges a write it never persists can leave a valid commit
pointing at a block that never landed. What is guaranteed is that this is never *silent*. Every
`BlockPtr` carries a seahash of the block it names, checked on every read.

Half of that gap was ours, and it is closed, 2026-08-18, by the durability half of milestone 55
(Time Machine). Our block server used to issue no `VIRTIO_BLK_T_FLUSH`, so the last acknowledged
write's durability was the device's word rather than ours. It now issues one.
`filesystem_contract::blk::FLUSH` (op 4) is a real device flush the block server waits on.
`filesystem_contract::fs::SYNC` (op 19) is the file-service verb that asks for it. A device that
never offered `VIRTIO_BLK_F_FLUSH` gets `EOPNOTSUPP` all the way up, rather than a quiet success.
That is the part that matters: the old problem was less the missing flush than that nothing above it
could tell.

The half that is still the device's is still the device's, and it is what the third row measures:
ordering *between* writes, which nothing on this contract expresses. A `SYNC` says "everything
acknowledged so far is durable now". It does not say "these two writes landed in this order", and
RedoxFS's `Disk` still has no barrier to say it with. A device that lies about its own flush is
likewise outside anything we can check.

## The controls

There are three, and the strongest needs no tampering at all. The lying-device sweep produces 72
images the filesystem refuses (2 at the mount, 70 at a read), so the injector is demonstrably
destroying things. Then `only_this_generation` blanks every header slot but one, taking the ring's
history away, and 133 of 134 fault points stop mounting. That isolates the fallback as the
mechanism. *(Re-measured 2026-09-24 with
`cargo test --release --test crash_consistency -- --nocapture` in `redoxfs_server/`. These had been
the 74 and 92 of 93 from milestone 37.)* Then, with no mount at all, a commit torn at 2048 bytes
fails `Header::valid()`, while the previous generation's slot stays valid and stays older.

A fourth control turned up on its own. The harness's first version treated any failed `open_file` as
"the name is absent". A dropped write to a directory's tree block makes that lookup answer `EIO`. So
nine fault points reported filesystems that never existed, empty root and all. It looked like a
serious RedoxFS bug for about ten minutes, and it was a test bug. `ENOENT` is the only error that
means absence. The engine refusing to guess at a block whose checksum does not match is the property
working. An instrument that can produce a false positive, and did, is an instrument connected to
something.

## Two mechanisms this note had named wrong

`cleanup: true` is not the header-ring replay. The ring scan is unconditional in `FileSystem::open`:
read all 256 slots, keep the newest whose seahash checks out, ignore the rest. `cleanup` adds a
tidy-up on top (release unused nodes, commit). The server passes it because a mount should not leak,
not because recovery depends on it.

And what the scan keeps is not "the newest consistent generation" in any sense the engine computes.
It is the newest generation whose *header* still hashes. That suffices only because a commit's blocks
are all written before it.

## The device-level half, and why it has a disk of its own

The host sweep is exhaustive and the device test is one crash, because they answer different
questions. The device test's question is whether the property survives the real stack: a real virtio
write torn in half, a real FS-server process dying inside its own transaction, and a real second
process recovering the disk it left behind.

The injector lives at `IpcDisk` and is armed by the `Spawn` literal. `arg0` is which `WRITE` request
to die in, `arg1` is block writes to allow first, and `arg2` is bytes of the last one that reach the
platter. It is the Spawn literal rather than a build flag, because the thing that crashes has to be
the FS server the gate otherwise runs, not a lookalike. `arg0 == 0` makes it inert, which is every
boot but this one. The tear is a read-modify-write with half the new contents laid over the old,
which is what a drive leaves when the rail collapses mid-block. `arg1` is one, because one is the
count that cannot miss. A write transaction always issues at least one block write, and a larger
count is a server that never dies and a test that hangs.

The recovery is a second FS-server process on the same block server and the same block page, with
its own file endpoint and its own stack. It carries nothing from the process that died. That it can
do this at all is endpoint-only naming doing its job: the block server never learns its client died
and was replaced, because it never knew who its client was. Its readiness sentinel is the consistency
result, because `Server::open` refuses an image it cannot make sense of.

The disk is dedicated and regenerated every run. This test deliberately leaves a filesystem
half-written. Doing that to the shared fixture would make every other FS test's result depend on
whether this one ran first, which is the order-coupled gate this note already spends a section on.
`NIFE_KEEP_REDOXFS` deliberately does not apply to it. The cross-boot case is interesting for the
shared disk, and is nothing but noise for this one.

The assertion is the property, not an outcome. Either payload passes. What fails is a mixture, a
length nobody wrote, or the pre-boot contents, which would mean an acknowledged write had vanished.
Pinning it to "payload A" would be pinning a detail of when RedoxFS happens to write its commit, and
the claim is not about that. Both legs currently report A, 66 bytes, whole. The gate re-reads the
image afterwards with the host tool and the pinned engine, which is the half a cache cannot fake.

Two ceilings moved for it, and both are receipts rather than tuning. `MAX_DEVICES` went to 27 for the
second block server, the fourth such bump and another argument for the missing unregister. And the
crash servers' untyped budget is 2 MiB rather than 8. An untyped is *reserved*, not merely capped,
and three 8 MiB reservations do not fit in this machine's 128 MiB. The first symptom of that was
`init` failing to get its own budget several tests later, which is a long way from the cause.
