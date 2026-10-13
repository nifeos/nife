---
status: BUILT
raised: 2026-07-30
built: 2026-08-18
---
# 38. Filesystem throughput, and the comparison (DECISIONS §34, condition 2; extends 21/25)

Built 2026-08-18, aarch64 under HVF. Four phases (sequential and random read and
write) through the confined FS server, against ext4 on Linux at a matched virtualization tier and
APFS on macOS natively, plus the raw virtio-blk floor both filesystems stand on. The instrument is
`kernel/src/bench.rs`'s `fs_throughput` and `fs_test_client`'s throughput role; the comparisons are
`bench/host/linux_fs.rs` (with `run_linux_fs.sh`) and `bench/host/macos_fs.rs`. Numbers, method and
caveats: notes/benchmarks.md; the two findings that belong to the server rather than to the
comparison are in notes/fs-server.md, including a `BUGS` entry on write amplification.

What it answers, which is the reason it existed. The confined-server tax is about a microsecond
per request (`relay_rtt`) against a per-request cost of 1.5 to 3.4 milliseconds, so the
architecture is 0.07% of the measurement and "userspace servers are too slow" is refuted for this
workload by three and a half orders of magnitude. Two measured results replace it. Our **confined
userspace block server is at parity with Linux**: 46.2 us per 4 KiB block, against 39 to 53 us for
Linux's own raw reads on the same virtio device at the same tier. And every 4 KiB file request
moves 128 KiB, because RedoxFS reads and rewrites a whole record, which is 32x amplification and is
the entire remaining gap. That belongs to the vendored store rather than to anything this project
designed, and it is a `BUGS` entry in notes/fs-server.md with the two shapes a fix could take.

In brief. Sequential and random read/write throughput through the confined FS server, against ext4 on Linux and APFS on macOS at a matched virtualization tier, the way milestone 25 did the primitives. Requires deciding what is honestly comparable: our reads are device-latency-dominated (`fs_read` is ~204 us/read under HVF, and `relay_rtt` puts the isolation tax a thousand times below that), so the interesting question is whether the userspace-server architecture costs throughput once the device dominates, which is a claim a microkernel skeptic will press

Why it matters. **"primary filesystem" invites a comparison we cannot currently make.** We have the per-request numbers and the isolation tax, and no MB/s figure at all. Milestone 21's rule is measure rather than argue, and 25 already established that the honest way to do this is EL0-measured against real systems rather than self-reported. This is where the "userspace servers are too slow" objection gets an answer or a concession

## Follow-on

- **Milestone 138.** The 32x write amplification this milestone measured, where every 4 KiB file
  request moved 128 KiB because RedoxFS reads and rewrites a whole record. Its step 1 takes the
  record to 8 KiB and its step 3 takes the client's transfer unit to 64 KiB.
- **Recorded.** `notes/fs-server.md`: a fixed per-request cost of about 204 us that neither the
  record size nor the transfer unit touches. It is device latency under HVF rather than anything the
  architecture adds, and the note carries the share it accounts for before and after 138.

## Index row

"primary filesystem" invites a comparison we cannot currently make
