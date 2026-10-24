# The host programs under bench/host

*Name: provisional (lane comments/929-rest-01, 2026-10-24 UTC). An appendix to
[`notes/benchmarks.md`](../benchmarks.md). It holds why each program in `bench/host/` is shaped the way
it is. The numbers they produce, with their dates and tiers, are in
[cross-os-primitives.md](cross-os-primitives.md) and [filesystem-throughput.md](filesystem-throughput.md).*

Every native program here builds with a bare `rustc -O`, not Cargo, so it does not fight the aarch64
target the workspace sets. Native numbers are statistical, because a shared desktop runs underneath
them. Take the median of a few runs rather than one reading.

## The primitive suite (milestone 25)

### `null_syscall.rs`

It makes a raw `getpid` through the syscall gate. libc's `getpid` caches the pid, so it would turn
the call into a userspace read that never traps. The raw call is the real crossing into the kernel.

### `ipc_rtt.rs`

Two processes are made with `fork`, so the round trip crosses an address-space boundary, as nife's
does between two EL0 processes. One iteration is four syscalls and two context switches.

It calls libc's `read` and `write` directly, which is what a C program and lmbench use. The generic
`syscall` gate was about 200 ns slower on the macOS host when the program was written (2026-07-25,
native), and would distort the count. `rustc` warns about a "suspicious runtime symbol" for those two declarations. The warning is
benign: the declarations match libc exactly.

### `ctx_switch.rs`

No stock OS has a call that yields to one named process, so a switch cannot be timed directly.
lmbench isolates it by subtraction. A two-process pipe round trip is two switches plus two pipe
passes. A self-pipe pass, one process writing a byte and reading it straight back, is a pipe pass
with no switch. One switch is then `round_trip / 2 - self_pipe_pass`.

The subtraction carries a known bias. The blocking `read` in the round trip is a little heavier than
the non-blocking one in the self-pipe pass, so the result comes out slightly high. That is why lmbench
and these notes treat it as a derived number rather than a measured one.

### `mmap.rs`

Neither side's bare call equals the other's. A bare `mmap` is lazy on both Linux and macOS: it makes
the mapping record and installs no page-table entry. Our `MAP_INTO` writes one entry eagerly. What the
two share is the trap that installs an entry, so that is what is timed. The program maps a large
anonymous region and writes one byte per page, and each first touch faults into the kernel.

Two asymmetries remain. The trigger differs, a fault here and an explicit syscall in `MAP_INTO`. And
the host allocates and zeroes a fresh page where `map_el0` aliases one existing frame, so the host
figure carries a zeroing cost that ours does not. The warm-up run uses a separate region, because a
page that has faulted once stays resident and a second touch would time a hit.

### `spawn.rs` and `linux_all.rs`

`spawn.rs` times `fork` plus `_exit` plus `waitpid`. Its caveat, that `fork` duplicates a heavier
object than nife builds, is in [cross-os-primitives.md](cross-os-primitives.md).

`linux_all.rs` is the five metrics in one static binary, booted as PID 1 under QEMU-HVF by
`bench/host/run_linux.sh`. That puts Linux on the same core and tier as nife.

## Pipe throughput (milestone 50): `pipe_throughput.rs`

A round trip measures latency. This measures bytes moving one way, which is what `a | b` is when `a`
has a lot to say.

nife's pipe is an endpoint and nothing else. It moves sixteen bytes per rendezvous, and the producer
cannot run ahead because there is no buffer to run ahead into. A Unix pipe has a 64 KiB buffer, so its
producer writes until the buffer fills and only then blocks. Comparing our lockstep against 64 KiB
writes would measure the buffer. Comparing it only against sixteen-byte writes would pretend nobody
uses a bigger write. Both are printed:

- `pipe_16` writes sixteen bytes, our message size. Its gap to our number is the cost of our
  mechanism at their granularity, the honest head-to-head.
- `pipe_64k` writes 64 KiB, as a real Unix program does. Its gap to `pipe_16` is what batching buys
  on Unix, the prize a buffering component would chase.

## Filesystem throughput (milestone 38): `linux_fs.rs` and `macos_fs.rs`

[filesystem-throughput.md](filesystem-throughput.md) has the tiers, the tables and the list of what
is not apples to apples. This section holds the reasons behind the programs' variants.

nife's FS server holds no data cache. A record body is always a virtio round trip, batched since
milestone 138 step 4 but never answered from memory. Every write is its own RedoxFS transaction that
commits before the reply. It does hold a small metadata cache since step 2 (`CachedDisk`, 64 blocks,
about 257 KiB, as of 2026-08-22). That answers a repeated read of one file's tree spine from memory, so `fs_read`, a
repeated read of one small inline file, is not comparable to a cold read. The four phases are
dominated by record bodies, which the metadata cache never touches, so the comparison holds for them.
It would not hold for a workload that mostly reopens hot metadata.

Linux by default is the opposite on both counts. The page cache absorbs reads, writeback absorbs
writes, and a 1 MiB file fits in the cache whole. A single ext4 number would answer a question nobody
asked, so each phase runs in several variants:

- Buffered is what a Linux program actually gets, and the number to quote for how fast Linux is.
- `O_DIRECT` has no page cache. It is the closest thing Linux has to what nife's path does on every
  request, and the one variant that is apples to apples.
- `O_DIRECT | O_DSYNC` makes each write durable before it returns. nife sits between this and plain
  `O_DIRECT`: it commits the filesystem's state per request but issues no device flush unless asked.
  `O_DIRECT` is the analogue for where the bytes are, and adding `O_DSYNC` is the analogue for the
  metadata commit. Quoting only one of them would be picking the answer.
- The 64 KiB row prices the unit. The 4 KiB phases were the protocol's one-page ceiling in the build
  measured; a Linux program would take 64 KiB without thinking. It is the same split as `pipe_16`
  against `pipe_64k`.
- The raw device row runs the same phases with no filesystem, the floor both filesystems stand on.
  It says how much of either one's cost is the filesystem and how much is virtio under HVF.

Both Linux variants still enjoy the host's page cache under QEMU, because the drive is attached
`cache=writeback`, QEMU's default and what the nife runner uses. That is matched rather than removed,
which is the point of running at the same tier.

After the write phase, the program calls `sync` and sleeps 250 ms before the read phases. Linux has
background work after a write returns, even under `O_DSYNC`, and a read phase that ran into it would
time the previous phase. nife gets no such pause and needs none. The asymmetry favors Linux, which is
the direction to err in. Module loading failures are reported and not fatal: `crc32c_generic` is
built in on some configurations and answers `EEXIST`, and the mount that follows proves the ext4 load.

`macos_fs.rs` runs natively on APFS and the real NVMe device, so it is a reference ceiling, not a
competitor. `F_NOCACHE` is macOS's nearest thing to `O_DIRECT`, but the bytes still go to the
device's own cache and nothing is flushed. `F_FULLFSYNC` after every write is macOS's real barrier,
since `fsync` there does not flush the drive cache. It is stronger than what nife's FS server does per
write, and it is printed so the strongest and weakest readings are both on the page. Its buffer is
page aligned although `F_NOCACHE` does not demand it, so the comparison does not turn on one side's
alignment.
