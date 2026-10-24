# What a directory walk costs on nife

*(Milestone 121 (`ripgrep` on nife: enumeration as a capability, and what the walk costs). Measured
2026-09-26 by lane `milestone/121-ripgrep`. The page's stem and the crate's name,
`walk_pricing`, are provisional; calef names things.)*

Every `read_dir` on nife is a message to a filesystem server, and every path is resolved one
component at a time. A recursive walk is the workload that prices that bet most directly. This page
records what the walk costs, split into the parts that cost it, so the question milestone 122 (a
directory handle `std` can hold) left open has numbers: does a multi-component resolve belong in
the contract?

## The instrument

`crates/walk_pricing` is plain `std::fs` code with no nife in it. `std_exerciser` runs it through a
granted directory; `cargo run --release -p walk_pricing --example host` runs the same function on a
host. The tree is `filesystem_contract::fixture::walk`, staged into the RedoxFS test image by the
same `stage` function a host uses:

| region | shape | the slope it gives |
|---|---|---|
| `chain` | a file at each of nine levels, 2 to 10 components below the grant | per path component |
| `wide`, `narrow` | 128 files against 1, each listed with `file_type` | per directory entry |
| `sizes` | 4 KiB and 256 KiB files | per KiB read |
| everything | a recursive walk reading every file: 153 entries, 141 files, 333,984 bytes, 366 components | the total |

Each slope point is the median of three timed runs after one untimed one, and the whole walk is one
warm run. The table below was taken at five runs for every figure, including the whole walk; the
count is three because the CI job that runs this on three emulated architectures is at its time
limit.

The grant is `ENUMERATE | READ | DESCEND` over `walk/`, behind an `fs_subtree_caretaker`. That is
the capability the confined `rg pattern src/` will hold, and it adds one relay hop to every message.

## The numbers

nife: `script/test --hvf --test a_walk_through`, which is the aarch64 kernel on the physical M-series
core under Hypervisor.framework. The kernel is a debug build (the suite has no release mode) and
userspace is release. Two runs on a quiet machine:

| | run 1 | run 2 | macOS, APFS, native |
|---|---:|---:|---:|
| per component | 15.0 us | 51.3 us | about 0 (-0.04 to 0.15 us) |
| read at depth 2 | 427 us | 246 us | 10.3 us |
| per entry listed | 85.5 us | 42.8 us | 0.29 us |
| per KiB read | 11.8 us | 6.2 us | 0.03 us |
| whole walk | 42.4 ms | 41.6 ms | 1.85 ms |

A third run shared the machine with another lane's riscv64 QEMU and moved every figure by up to 5x
(a negative per-component slope, a 77.7 ms walk). It is discarded, and it is the reason nothing
below leans on a single slope.

Under TCG in CI the same test prints figures on all three architectures; they are the emulator's
time and are printed, not read.

## What they say

The whole walk is stable and the slopes are not. Two quiet runs agree on the total to 2% and
disagree on every slope by up to 3.4x. Five samples remove a stray preemption and nothing more.
Three runs of the trimmed instrument, taken later with the machine at a load average of 24 from
other lanes' gates, put the whole walk at 144 to 445 ms and are discarded for the same reason as
run 3.

A path component is not where the walk's time goes. At the larger of the two slopes, the 366
components the walk resolves cost about 19 ms of 42; at the smaller, about 5.5 ms. Either way the
fixed cost of an operation dwarfs a component: one small read two components down costs 250 to
430 us, against 15 to 51 us per extra component. A multi-component resolve would save something
between an eighth and a half of this walk, and the spread is the honest answer.

Listing is the largest per-unit cost, and the mechanism is known. 43 to 85 us per entry, for a
listing whose `file_type` is free on the client. The time is in the server: `read_dir` in
`redoxfs_server` reads every child's node to set the `IS_DIR` bit, because a RedoxFS directory entry
does not store the child's type. That is one tree read per entry, and 128 children overflow the
server's 64-block metadata cache. The `BUGS` section of that function records it.

Against macOS the walk is about 23x slower, and most of the gap is not the microkernel. The
comparison is not apples to apples in four ways, each large:

- a debug kernel, on an IPC path `notes/benchmarks.md` measures at about 6.7x slower unoptimized;
- a caretaker hop on every message, which a Unix process does not pay;
- no page cache and no dentry cache on nife, where macOS answers a second walk from memory;
- a virtualized tier (HVF, virtio-blk behind a host file) against native NVMe.

So the ratio is an upper bound on what the design costs, not a measurement of it.

## What this does not decide

The contract question stays open, with better inputs. The data argues for looking at the listing
before the resolve: a type in the directory entry, or a server-side type cache, would attack the
largest slope. That is a format and contract choice, so it is recorded rather than made. A
walker holding `std::fs::Dir` handles would pay one hop per directory instead of one per component;
`walkdir` and `ignore` walk by path, so that is a change to the PAL or to them.

## What `price` separates

Every `read_dir` on nife is a message to a filesystem server, and every path is resolved one
component per message: an `OPENDIR` per hop, then the verb, then a `CLOSE` per hop. The instrument
is ordinary `std::fs` code, so the same function runs on nife through a granted directory and on a
host through a path. That is the only way a comparison runs the same logic on both sides.

One number for a walk hides which half of the system is expensive, so `price` measures four
things, each a slope over one variable with the others held still:

- Per component: reading the one small file at each level of a chain of directories, from one
  component below the grant to ten. The slope is what one more name in a path costs.
- Per entry: listing a directory of 128 files against listing one of 1, each with its
  `file_type`, which is free on nife because `READDIR` carries it.
- Per byte: reading a 4 KiB file against a 256 KiB one. This is what the search half of a search
  tool pays, and it exercises the block path rather than the name path.
- The whole walk: a recursive `read_dir` and read of every file, in the shape `walkdir` and
  `ignore` walk in. It counts the path components it resolved, so the three slopes can be checked
  against the total rather than trusted.

Each slope point is the median of `REPS` timed runs after one untimed one. The whole walk is the
median of `WHOLE_REPS`, warm, after the walk that counted, and each of those runs is also split by
the `std::fs` call that spent the time (`Split`).

## What the instrument does not see

- A median of three removes a stray preemption and nothing else. The figures are magnitudes and
  directions; a published comparison wants more runs and a spread.
- The untimed run primes whatever caches each side has: the FS server's metadata cache on nife,
  the dentry and page caches on a Unix host. So these are the costs of a second walk. A first walk
  over a cold disk is a different number and is not taken here.
- The per-entry slope is inside one `READDIR` page. 128 four-byte names fit in one; a directory of
  more than about 680 such names would add a message per page, which this does not price.
- It walks with paths, as `walkdir` and `ignore` do, which on nife means every open re-resolves
  every component from the grant. A walker holding `std::fs::Dir` handles would pay one hop per
  directory instead, and nothing here measures that shape.
- The host example's matched-tier Linux run, `bench/host/run_linux_walk.sh` (milestone 606 (a
  directory walk costs what it does on Linux)), builds it static for musl and boots it as PID 1
  under QEMU-HVF on the machine nife's bench boot uses. The tree lives on the initramfs rather than
  ext4 on virtio. For a warm walk that does not matter; for a cold one it does, and
  notes/walk-cost.md says so.

## Reproduce

```text
script/test --hvf --test a_walk_through           # nife, aarch64, the physical core
cargo run --release -p walk_pricing --example host # the same walk on this host
```

The HVF leg needs `cargo xtask std-src` behind it, which builds the worktree's own std farm (until
2026-10-10 it also relinked the machine-wide `nife-dev` toolchain).

## Since then

Milestone 606 (a directory walk costs what it does on Linux) took these figures as its starting
point. The walk is now in the release bench boot as `fs_walk`, beside a Linux figure on the same
machine, and a warm walk on the release kernel went from 28 ms to about 0.8 ms.
[A directory walk against Linux on the same machine](walk-cost.md) has the numbers. The two
`BUGS` entries this page carried about a debug kernel and a missing Linux figure are closed there.

## BUGS

- The table above is from the debug kernel under the kernel suite, before milestone 606. It is
  kept as the record of where the work started, not as the current cost.
- Three samples per slope point now, five when the table was taken, and the table shows five is
  not enough for a slope. The totals are the figures to quote, and they want a quiet machine: the
  same walk measured 42 ms idle and up to 445 ms beside other lanes' emulators.
- `rg` itself has not walked. This prices the walk `ripgrep`'s `ignore` crate performs, with the
  same `std::fs` calls, but not `ripgrep`. That run waits on arguments: milestone 205 (how a
  foreign program is told what to do).
