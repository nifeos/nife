# A directory walk against Linux on the same machine

*(Milestone 606 (a directory walk costs what it does on Linux). Measured 2026-09-26 by lane
`milestone/606-walk-cost`. The number, the page's stem and every new name here are provisional;
the integrator mints the number and calef names things.)*

calef, 2026-09-26: "20x slower is unacceptable." This page is what the walk cost, where the time
went, what was fixed inside the file-service contract, and the wire changes calef ruled on to
close the rest, each with its measured number.

The walk is the one milestone 121 (`ripgrep` on nife: enumeration as a capability) priced:
`walk_pricing::walk` over `filesystem_contract::fixture::walk`, 153 entries and 141 files, read
whole. [What a directory walk costs](walk-pricing.md) describes the
tree and the instrument.

## The numbers

Every figure is a warm walk, the median of five in one boot, on the same M-series host. nife is
`script/bench --real --release`: the release kernel under HVF, with `std_exerciser` holding the tree
behind an `fs_subtree_caretaker`. That grant is the one a confined `rg pattern src/` will hold.
Linux is `bench/host/run_linux_walk.sh`: the Alpine `virt` kernel on the same `virt,accel=hvf`
machine, running the same walker as PID 1.

| | whole walk | against Linux |
|---|---:|---:|
| nife before this milestone | 27.0 to 29.5 ms | about 68x |
| nife now, inside the current contract | 0.76 to 0.82 ms | about 1.9x |
| Linux (Alpine 3.20 `virt`), tmpfs, same machine | 0.41 to 0.42 ms | 1x |
| macOS, APFS, native (for reference only) | 1.7 ms | |

Three boots each, on a machine with a load average of 4 to 9 from other lanes. Under that load a
run sometimes lands on an efficiency core and reads about 1.55x slower across every figure at
once. Those runs are visible as a separate mode and are left out of the ranges above.

The split by `std::fs` call, from the same runs:

| | list (13 dirs) | open (141) | read (141) | close (141) |
|---|---:|---:|---:|---:|
| nife before | 7.6 ms | 11.5 ms | 8.2 ms | 0.18 ms |
| nife now | 89 us | 308 us | 333 us | 82 us |
| Linux | 81 us | 103 us | 172 us | 34 us |

Listing is now at parity. The rest is per request.

## Where the 28 ms went

The device, not the kernel. The debug kernel under the kernel suite took 41 ms and the release
kernel 28 ms, so optimization bought a third. The FS server's cache held 64 blocks and the
walk's metadata alone is 207. `redoxfs_server/examples/walk_replay.rs` replays the walk's
requests against the server core on the host and counts what reaches the disk: 415 device reads
per warm walk, each about 37 us through the block server under HVF. What was not device time was
RedoxFS answering every lookup by walking its node tree from the root, five blocks deep, hashing
each block. Replayed on the host with no device at all, that was 6.4 ms of server time per walk.

## What was fixed inside the contract

None of these changes a message.

1. The FS server remembers listings, names it found, and small files, until something changes
   the image (`redoxfs_server/src/memo.rs`). Every mutating verb goes through `change`, which
   forgets. A warm walk now reads nothing from the device, and `tests/walk_cost.rs` asserts that.
2. The block cache grew from 64 slots to 256. Slots are allocated as they fill and the header
   ring is never cached, so the crash test's 2 MiB servers still fit. After a namespace change
   forgets the memo, the next walk reads the device 2 times instead of 372.
3. `OPEN` and `CLOSE` stopped opening a RedoxFS transaction when the memo answered. A transaction
   clones the engine's allocator, and that clone was 80% of the server's remaining time.
4. The `std` PAL copies the shared page with `memcpy` instead of a volatile byte loop.

Server time per warm walk, replayed on the host: 6.4 ms before, 43 us after.

## Where the remaining 0.8 ms goes

Round trips. The walk sends 1,259 requests (`walk_model` counts them exactly): 225 `OPENDIR`,
26 `READDIR`, 141 `OPEN`, 141 `FSTAT`, 360 `READ` and 366 `CLOSE`. Each costs about 0.63 us, of
which the FS server's own work is 34 ns. The rest is two IPC round trips (client to caretaker,
caretaker to server) and the PAL. Linux answers each of the same calls with one trap and a
dentry or page cache hit.

Two things that look like levers are not:

- Dropping the size hint. `std::fs::read` asks `metadata()` for a size hint, which is one
  `FSTAT`. Without it `read_to_end` probes with a 32-byte read first. For the fixture's 16-byte
  files that saves a request; for any file over 32 bytes it costs one back. It would flatter this
  benchmark and slow a real source tree.
- Caching directory handles in the PAL. A handle held across calls keeps naming the directory
  after a rename, where a path should name what is there now. That is a semantic change, not an
  optimization.

## The wire changes: A and B built, D ruled

Each option changes something two programs agree on, so each was calef's call. All three were
prototyped and measured first; the branches were not kept, because the numbers were the finding.

| | requests | with the caretaker | no caretaker |
|---|---:|---:|---:|
| inside the old contract | 1,259 | 0.79 ms | 0.57 to 0.59 ms (prototype) |
| A: a path in one `OPEN` or `OPENDIR`, built | 833 | 0.58 to 0.61 ms | 0.39 ms (prototype) |
| A and B: `OPEN` also returns the size, built | 692 | 0.515 to 0.53 ms | 0.35 to 0.36 ms (prototype) |
| Linux | | 0.41 to 0.42 ms | |

The built rows were measured on 2026-09-27 on branch `milestone/606-open-path`, three `--smp`
release boots each, in one session with the old-contract row and Linux (load average 7 to 12). A
and B together leave nife at about 1.26x Linux.

**A, ruled 2026-09-27 07:04Z and built.** `OPEN` and `OPENDIR` resolve a relative path on the
server, exactly as the hop-by-hop walk would. Every directory on the way is a descent asking for
`DESCEND` plus the request: it needs `DESCEND` on its parent, is narrowed, and is `EPERM` if it
comes up short. `OPEN`'s second word carries that request, so a file opened for reading still
carries only `READ`. Every component is checked before any is resolved. An older server answers
`EINVAL`, and the PAL then walks hop by hop and stops asking. `fs_nameset_caretaker` checks the
first component of a path against its set. `filesystem_contract::fs::OPEN` and `OPENDIR` hold the
rule.

**B, ruled 07:06Z in its second form and built.** `OPEN`'s reply carries the file's size in its
second word, and 0 means "not given". The PAL keeps it as `File::open_size_hint`, and the std
overlay routes the buffer sizing of `fs::read`, `fs::read_to_string` and `read_to_end` through it
(`std_patch_size_hint` in `xtask/src/farm.rs`). `metadata()` still always asks the server. calef's
reason: a stale hint costs at most one resize, and a stale `metadata()` lies to the program.

**D, ruled 07:17Z as option 2, on per-filesystem terms.** Caretakers stay the default for every
filesystem. D is a fast path only for a memory-safe server that resolves every path through one
shared scope crate, proven with Kani, which owns `..`, symbolic links, hard links and mount
crossings. It is for subtree grants only, and the progenitor chooses per mount, from what the
filesystem's package declares, between a badge and a caretaker. The section below is the costing
that ruling was made on and what was built from it, after milestone 599 (a frame per filesystem
client channel) landed badged endpoints under §230 (badged endpoint capabilities).

## D in detail: the server narrows, keyed on the badge

### Is the premise true?

Half. The server can tell which endpoint a request came through, since milestone 599 (a frame per
filesystem client channel): `rendezvous::BADGE` mints a badged copy of an endpoint capability, and
`RECEIVE_CAP` returns the badge in x3. So the premise that it cannot is false.

The other half, that the caretaker is where confinement is enforced, is only partly true. The rights arithmetic is already the server's, with three Kani harnesses in `filesystem_contract` (`attenuate_never_widens`,
`a_grandchild_is_bounded_by_the_root`, `a_root_carries_nothing_undefined`). What the subtree
caretaker adds is handle scoping. The server's handle table is one per server, and handle 0 is the
image root with every right. The caretaker keeps its own 64-slot table, so a client can only name
the handles its own caretaker minted, and its `ROOT` means the granted directory. D moves that
table into the server.

### What was built, and what it costs

No kernel change. The kernel pieces all came with milestone 599: `BADGE`, the badge on
`RECEIVE_CAP`, and `SEND_CAP` to hand the badged copy over. Built on branch `milestone/606-scope`:

1. `crates/subtree_scope` (name provisional): `walk` resolves every path, `landing` decides the
   last step, and `admit` and `Bindings` decide what a badge reaches. Four Kani harnesses prove
   it, and `script/verify`'s table has the row. A step is one name; a walk never widens, never goes
   through a symbolic link, never crosses a mount; a bound badge reaches its root and what it
   minted; a badge once bound is never open again.
2. Two control verbs, `BIND` and `UNBIND` (`filesystem_contract::fs`, numbers 64 and 65,
   provisional). They sit outside the verb table on purpose, so no caretaker forwards them. Only a
   caller whose own badge is unbound may use them. The badge still names the client's window
   (§230), and a grant is bound to that same index.
3. `redoxfs_server`, the first eligible server: every path goes through `subtree_scope::walk`, every
   handle through `admit`, and every handle a bound badge mints is recorded as its.
4. The kernel harness (`fs_service::start_std_bound`) and two tests on device. The walk through a
   bound grant visits exactly the fixture. A bound badge cannot open a name outside its subtree, use a handle it did not mint, bind, close its root, or reach anything after `UNBIND`.

Measured 2026-09-27 on the release kernel, three `--smp` boots in one session: `fs_walk_bound` is
0.347 to 0.355 ms, against 0.508 to 0.521 ms through the caretaker and Linux's 0.41 to 0.42 ms.
That is about 0.85x Linux. It also saves a process per grant.

### The hard cases, as one crate decides them

`crates/subtree_scope` owns `..`, symbolic links, hard links and mount crossings, so no server
decides them. `..` and `.` are never names, so a path can only go down. A symbolic link is never
followed, on the way or at the end; a server that wants links followed inside a grant is asking for
a policy the crate does not have. A mount point is never crossed, so a grant is one filesystem.

A hard link is a second name for a file, and a file reached by a name inside the grant is inside the
grant. That is the Unix rule, and it is the right one for a subtree grant, whose authority is over
names. A directory has no hard links in any eligible filesystem. A server is eligible only when it
is memory-safe and routes every path and every handle through the crate.

A badge is a window and a grant at once. Under §230 (badged endpoint capabilities) a badge is the
index of the client's staging window, and the crate binds a grant to that same index. So a revoked
badge must not be handed to a new client while its old holder is alive. The progenitor's window pool
releases a window at reap, after the holder is dead, and nothing in the crate can check that. The
proof is bounded: paths up to `PROOF_PATH` bytes, and binding tables of `PROOF_BADGES` badges over
`PROOF_OPERATIONS` operations. The functions have no loops whose behavior changes past those bounds,
which is the argument that the bound is enough.

### The trust tradeoff

Lines of code outside tests, blank lines and comments, counted 2026-09-26:

| | code lines | shares an address space with |
|---|---:|---|
| `fs_subtree_caretaker` | 111 | nothing; no heap, one stack page |
| `fs_nameset_caretaker` | 170 | nothing |
| `redoxfs_server`: serve loop (`bin/redoxfs_server.rs`) | 341 | the engine below |
| `redoxfs_server`: core and memo (`lib.rs`, `memo.rs`) | 836 | the engine below |
| vendored RedoxFS engine | about 8,900 | the server |

What a bug in each exposes:

- A handle-scoping bug in a caretaker lets its client use a server handle it did not mint. The
  first one it would find is handle 0, the image root with every right. So the blast radius is the
  whole image. That is also the blast radius of the same bug in the server under D.
- The difference is what else can reach the scoping state. Under D the handle table sits beside
  the RedoxFS engine, so a memory-safety bug in parsing a hostile image could corrupt it. That
  buys an attacker little: a compromised engine already holds the whole image, with or without a
  caretaker in front. The caretaker protects against clients, not against the server.
- What D gives up is the caretaker's case in §27 (the filesystem service). There, "it cannot
  reach a second file" is a property of the client's capability space: it holds nothing that
  names the FS server. Under D the client holds a badged endpoint to the server itself, and confinement
  becomes a branch the server is trusted to take. That was calef's reason, and it is the real
  cost.

What verification exists or is feasible:

- Today: the three Kani harnesses on the rights arithmetic, which both designs share. Neither
  caretaker's handle table has a harness.
- Under D the scoping is `subtree_scope`, a host-testable crate, and its Kani harnesses prove that
  a request under badge `b` resolves only handles `b` minted, and `ROOT` only to `b`'s root.
  Nothing stops the same harness being written for the caretaker's table; it has none.

### Does D cover the name-set caretaker too?

It can, at a larger price. `fs_nameset_caretaker` filters every name-taking verb in the granted
directory against a set of names. Under D the server would hold each badge's name set as well and
apply `filesystem_contract::nameset::contains` on those verbs. The set is variable-length, so
`BIND` would carry it through the channel page. That is a second, larger piece of policy moving into
the server, with no walk measurement behind it: glob grants are the shell's and are rarely on a
walk's path. D for subtrees alone is the smaller step.

### Who holds what, and revocation (as built)

- Today: the progenitor holds the FS endpoint; each caretaker holds a badged copy with its window;
  the client holds the caretaker's endpoint. Revoking a grant is ending the caretaker
  (`Holding::release`), which §41 (the endpoint is the broker) describes as taking the endpoint back.
- Under D: the binder holds an unbadged copy and binds one badge per grant; the client holds the
  badged copy. There is no kernel method to pull a capability back out of another
  process's slots, today or under D. So revocation becomes `UNBIND`: the server closes every handle
  the badge minted and answers `EBADF` to it from then on. The client keeps a capability that names
  the server and does nothing. §41's "take it back" turns into "the server stops honoring it",
  which is weaker as a statement even when the effect is the same.

### What else was considered

Keeping the caretaker and making its hop cheaper: the only lever is the IPC path, which milestone
188 (the IPC fastpath) has already been over. The server minting badges itself: `REPLY` carries no
capability, so it would need a kernel change. From memory, not read: seL4 servers identify
clients by badge, and a 9P server keeps fids per connection, which is this per-badge handle table
under another name.

**E, not built. A wider channel for `std`.** `READ` already accepts 64 KiB, but a `std` program
maps one page at `FS_PAGE`, which `std_runtime_contract` fixes, and the caretaker clamps to it.
The walk's three sized files take 80 `READ`s where they could take 6. In a real source tree most
files are over 4 KiB, so this matters more there than here. It changes the loader's mapping, which
the progenitor and the kernel harness share with the PAL.

What is left of D: the progenitor's choice per mount, from what the filesystem's package declares.
It waits on package declarations moving to TOML (#1405) and on the progenitor-stack lane, which is
working in the progenitor now.

## Reproduce

```text
script/bench --real --release --smp          # fs_walk, and the split as probes
sh bench/host/run_linux_walk.sh              # Linux, same machine, same walker
cargo run --release --manifest-path redoxfs_server/Cargo.toml --example walk_replay
```

`--smp` is the leg that builds `std_exerciser` and the FS server, and it takes the machine-wide
`nife-dev` link, as any gate does. `fs_walk` also runs on one hart if its `online_count` gate is
lifted, which is how the ranges above were taken; the figures agree with the fast mode of `--smp`.

## BUGS

- Linux is on tmpfs, not ext4 on virtio. Every timed walk is warm, and a warm walk on Linux is
  answered from the dentry and page caches whichever filesystem is under them. A cold ext4 walk
  is a different number, and `bench/host/run_linux_fs.sh`'s podman-made image is how to take it.
- Warm only, on both sides. A cold nife walk after boot reads the device 207 times.
- The memo forgets everything on any namespace change. `memo.rs` records why that is coarse
  on purpose and what a build that writes while it walks would pay.
- `fs_walk` waits forever on a program that aborts. It drains the program's stdout until the
  end-of-stream marker, which an aborting program never sends; the kernel suite's `drain_sink`
  records the same gap.
