# Rust `std` on the native ABI

*(Milestone 27. The first wall an application hits on nife was "no std": you could write a
`no_std` binary against `crates/user_mode_runtime`, and nothing else. This milestone makes ordinary Rust,
`Vec` and `String` and `println!` and `Instant`, compile and run on the capability ABI. See
DECISIONS §22 for the decision and why; notes/abi.md for the ABI it binds to.)*

The shape is Hermit's, not Redox's. Hermit implements std's platform layer directly on a
non-POSIX unikernel ABI; Redox writes a POSIX C library (relibc) first and puts std on top of that.
We took the native road: no errno, no fd table, no `open`, no `fork` under our `sys` backend,
because the OS lacks them and std does not need them for a workload that stays off files and
sockets. That is why the native ABI came first, per DECISIONS §14 (the project's direction) and §15 (the native ABI): std widens "runs real workloads" from hand-built `no_std` binaries to most of
crates.io, without smuggling in the POSIX assumptions the ABI deliberately excludes.

## What a std program is given

A std program is an ordinary nife ELF (notes/abi.md §3): entered at `_start`, linked at
`0x6000_0000` (notes/address-space-map.md), capability table populated by its parent. std's runtime contract needs two things, and the ABI's
out-of-band convention (notes/abi.md §4) grants them at fixed slots:

- slot 0: an untyped budget. The global allocator draws heap pages from it lazily via
  `untyped::MAP`, one page per invoke, at `0x4000_0000`: the untyped-backed heap the
  `allocator_exerciser` workload proved (`crates/user_mode_heap` algorithm, host-tested), restated
  because std cannot depend on an out-of-tree crate.
- slot 1: an endpoint with WRITE. `stdout` and `stderr` SEND here, 16 bytes per message (w0 =
  byte count, w1|w2 = the bytes, little-endian). std's own `LineWriter` batches user writes; the
  receiver reassembles.

Three more slots exist, and a program holds each only if it was *given* the thing behind it
(milestone 27 phase two, the `std::net` and `std::fs` bindings below):

- slot 2: a `Stack` endpoint with WRITE. `std::net` speaks net_stack's socket contract over it.
- slot 3: an untyped budget the net PAL mints each socket's shared frame from.
- slot 4: an FS-service endpoint with WRITE, which *is* a directory capability, plus the page it
  shares with the FS server mapped at `0x1100_0000`. `std::fs` speaks the §27 file contract over it.
- slot 5: a `PageFrame` capability naming the clock page, with `READ`, plus a read-only mapping of it
  at `0x1200_0000`. `SystemTime::now()` is the offset it finds there plus the ambient counter
  (milestone 51, §43).
- **slot 6: the entropy service's request endpoint**, with WRITE (milestone 56, §44). It means "you
  may obtain randomness" and names no device; no mapping comes with it, because randomness is
  obtained by asking, not reading. `std::random::SystemRng` is a `CALL` on it.

A program never touches the slots it does not use. The absence of slots 2 and 3 is what "no
ambient network" feels like from inside a process, and the absence of slot 4 is "no ambient
filesystem": each returns `Unsupported` because there is no capability to reach, not because the
code was compiled out. A program can hold
one and not the other, so slots do not fill contiguously; section 4 of notes/abi.md records how the kernel
places slot 4 while leaving 2 and 3 empty, and why the gap matters.

## The PAL surface, and what each piece binds to

The backend lives in `patches/std-nife/overlay/std/src/sys/` and is materialized into a patched
std by `cargo xtask std-src`. Each file binds one std concept to the ABI:

| std concept | nife binding |
|---|---|
| `GlobalAlloc` | `untyped::MAP` from slot 0, grow-on-demand (`sys/alloc/nife`) |
| `stdout` / `stderr` | `endpoint::SEND` on slot 1 (`sys/stdio/nife.rs`) |
| `stdin` | `RECEIVE` on slot 30 when the line redirected or piped a stream into the program; otherwise the console's line, slot 29, for a program granted it; otherwise end of input (`sys/stdio/nife.rs`) |
| `Instant`, `SystemTime` | the virtual counter, `CNTVCT_EL0` / `rdtime` (`sys/time/nife.rs`) |
| `panic!` | print, then `brk`/`ebreak`: a fault the kernel attributes. No unwinding. |
| `thread::spawn` | a TCB joined to the program's own process, slots 10 to 12 ([threads](std/threads.md)) |
| `net` (`TcpStream`, outbound `UdpSocket`) | net_stack's socket contract on slots 2/3 (`sys/net/connection/nife.rs`), or `Unsupported` when not granted |
| `fs` (`File`, `metadata`, `read`/`write`) | the FS service's file contract on slot 4 (`sys/fs/nife.rs`), or `Unsupported` when no directory was granted |
| `std::random::SystemRng` | the entropy service's endpoint on slot 6 (`sys/random/nife.rs`), or a **panic** when not granted |
| `HashMap` seed | the same service when granted; splitmix64 from the counter when not, and labeled |
| `std::env::consts::OS` | `"nife"` (patched into `env_consts.rs`) |
| `std::env::var` / `vars` / `set_var` | a **process-local table** (`sys/env/nife.rs`), seeded only with `TZ`/`LANG`/`TERM` from a granted inert-configuration page (slot 7, milestone 47, DECISIONS §111) if one exists; otherwise empty at start |
| `std::env::temp_dir` / `split_paths` / `join_paths` | `TMPDIR` or `/`, and a `:`-separated list (`sys/paths/nife.rs`) |
| `std::env::current_dir` | `/`, the root of this process's own namespace (milestone 47); `Unsupported` when it holds no directory. `current_exe` and `chdir` refuse, `home_dir` is `None` |
| `std::process::id` | `0`, because this system issues no process identifier (`sys/process/nife.rs`); everything else in `std::process` refuses |

The syscall glue (`sys/pal/nife/rt.rs`) deliberately twins `crates/user_mode_runtime`'s
`svc`/`ecall` wrappers, because std cannot depend on the crate. The ABI constants are not restated: `abi.rs` is generated verbatim from `crates/abi` by `std-src`, so the numbers cannot
drift. Likewise `user_mode_heap.rs` from `crates/user_mode_heap`: the host-tested heap algorithm is
the only heap algorithm. So too `netproto.rs` from `crates/socket_contract/src/lib.rs`, and `fsproto.rs` from
`crates/filesystem_contract`. Every wire format the PAL speaks has exactly one definition, and it
lives with the server that answers it.
A crate root may keep a module in its own file (`pub mod fs;`), as `filesystem_contract` has since
milestone 845 (the filesystem-protocol crate root is split along its seams). It is copied with that
file inlined as `pub mod fs { ... }`, so std still builds one file.
The slot numbers and page addresses themselves (`rt::FS_DIR_SLOT`, `rt::CLOCK_PAGE` and the rest)
come the same way, as `runtimeproto.rs` from `crates/std_runtime_contract`, since milestone 595
(provisional) made the progenitor a loader of `std` programs too.

## The toolchain: build-std against a patched rust-src

There is no crate to adopt; the deliverable IS the PAL, plus the machinery to build it.
`-Zbuild-std` compiles std from the sysroot of the rustc it invokes, so a patched std means a
patched sysroot. `cargo xtask std-src`
builds one:

1. Hardlink-clone the real nightly (`cp -al` of `bin` and `lib`), which costs almost no disk. rustc resolves *this* directory as its sysroot (it derives the
   sysroot from the location of `librustc_driver`; a symlink farm resolves back to the real
   toolchain, as measured).
2. Replace the `src` subtree with a real copy (independent inodes), so patching it never
   touches the shared rustup toolchain.
3. Patch that copy: drop in the overlay PAL files, generate `abi.rs`/`user_mode_heap.rs`, and insert a
   `target_os = "nife"` arm into std's `cfg_select!` dispatchers (pal, alloc, stdio, random,
   thread, time, io/error, thread_local storage and guard) plus `env_consts` and the
   `restricted_std` chain in std's `build.rs`.
4. Link it as this worktree's own toolchain, `nife-dev-<directory>-<hash>` (`rustup toolchain
   link`; the next section says why the name is per worktree).

`cargo xtask std-exerciser` then builds the `std_exerciser` demo for both custom targets against it,
setting `RUSTUP_TOOLCHAIN=<farm>` rather than `+<name>`. The cargo proxy that launched xtask already
exports `RUSTUP_TOOLCHAIN=nightly`, which would override a `+` selector
and silently build std from the *unpatched* sysroot.

`std-src` is idempotent: a stamp of all inputs (the toolchain version, the ABI/heap crates, the
target specs, every overlay file, and a patch-logic version) guards the rebuild, so only a PAL
change forces std to recompile.

### Each worktree has its own toolchain name

The farm is per worktree, and since 2026-10-10 so is its name: `std-src` links
`nife-dev-<directory>-<hash of the worktree's path>` (provisional; calef has not ruled on it) and
prints it. `rustup toolchain link` writes one symlink per name for the whole account, so a name
that two worktrees shared was a name they fought over. Until that day every worktree linked the
same `nife-dev`, and each `script/test` took it from whichever lane had it last.

- 2026-07-31: a pruned worktree left `nife-dev` dangling, and unrelated builds failed far from the
  cause with `override toolchain 'nife-dev' is not installed`.
- 2026-08-18: lane `55-durability` relinked mid-run and lane `64-more`'s `std_exerciser` built
  against 55's farm, caught only by a person reading the `Compiling std` path. `std_src` then
  learned to relink loudly on the warm path, which made the theft visible and did not stop it.
- 2026-09-30: three sites of evidence in one evening. Main-soak's `std-aborts` aborted on dep-info
  naming another worktree's farm; the 1377 lane built the unpatched std twice under
  `CARGO_TARGET_DIR=$PWD/target`; `rustc --print sysroot` in a fresh worktree named another
  worktree's farm. Every build this tree owns has named the farm by path since, because a path
  this checkout computed and checked is the only selector with one owner.
- 2026-10-10: three false alarms in a day from lanes relinking each other, though no result was
  wrong, since the builds already used paths. A name nothing built through still churned.

So nothing in the tree builds through a name. The per-worktree link is for a person typing
`cargo +<name>`, and it cannot be taken: no other worktree computes it. `std-src` also removes
every `nife-dev-*` link whose farm is gone, which is what a pruned worktree leaves, so nobody has
to remember to. The workaround from the lane for milestone 57 (partitioning a real drive) still
works: symlink a worktree's `target/nife-farm` at the main checkout's farm once `cargo xtask
std-stamp` shows the stamps match, and `std_src()` early-returns instead of rebuilding a second copy.

**The legacy `nife-dev` link** is still written by lanes cut before 2026-10-10 until they rebase,
and nothing here reads, writes or deletes it. Once no open lane predates the change, the maintainer
removes it once with `rustup toolchain uninstall nife-dev`.

`std-exerciser` also pins `CARGO_TARGET_DIR` to `std_exerciser/target` and prints the override.
An export used to separate a build from its evidence: the sweep judged the previous run's
dep-info, so a wrong-sysroot build passed green. Both shapes were reproduced host-side.

### The target specs

`targets/{aarch64,riscv64,x86_64}-unknown-nife.json`, built with `-Zbuild-std` and
`-Zjson-target-spec`. x86_64 joined at milestone 184.
The load-bearing fields:

- `"os": "nife"` selects our `sys` backend through every dispatcher.
- `"panic-strategy": "abort"` means unwinding machinery is never linked; `panic!` prints and faults.
- `"singlethread": false` since milestone 812 (`std::thread::spawn` runs real threads in one
  address space): futex locks, key-based thread-locals through the thread pointer, and real
  threads. [The threads appendix](std/threads.md) has the PAL.
- softfloat (aarch64 `-neon`, riscv `lp64`, x86_64 `-mmx,-sse...,+soft-float` with
  `"rustc-abi": "softfloat"`) matches EL0/U-mode/ring 3 with no FP save area, the same choice the
  `no_std` programs make. On x86_64 this is a correctness requirement, not a preference:
  `kernel/src/arch/x86_64/` saves no FPU or SSE state on a context switch. A std program that let
  LLVM emit SSE would have its `xmm` registers overwritten by whichever thread ran next.

The reasoning of milestone 184 (extend the `std` port to x86_64) for each field, the probe showing no
SSE leaked in, and the entry stub that fixes the stack offset are in [the x86_64 spec
appendix](std/x86-64-target.md).

## `std::net` and `std::fs`

Both are clients of a frozen contract. Their wire constants are generated verbatim by `std-src`
(`netproto.rs` and `fsproto.rs`), so they cannot drift from the server's.

- `std::net` binds `TcpStream`, `TcpListener` and outbound `UdpSocket` to net_stack's socket
  contract on slots 2 and 3, under §25 (socket identity). Each socket gets its own shared frame; a listener gets
  none. Errors map by meaning, not by errno: a refused connect is `ConnectionRefused`, and a port
  outside the listen grant is `PermissionDenied`. The full mapping is in
  [the net appendix](std/net.md).
- `std::fs` binds `File` and the namespace verbs to the FS server's file contract on slot 4,
  under §27 (the filesystem service). A path resolves under the one granted directory, so `/motd`
  and `motd` are one file. Any `..` is refused with `InvalidFilename`, because no capability
  designates what is above the grant. A nested path is a chain of `OPENDIR` descents
  since milestone 122 (a directory handle `std` can hold). The mapping is in [the fs appendix](std/fs.md), the walk and `std::fs::Dir` are in
  [the descent appendix](std/fs-descent.md), and file times are in
  [their own appendix](std/file-times.md).

A program granted neither gets `Unsupported` from both, because there is no capability to reach.

## What still ends a nife process

The dangerous std call is not one that returns `Unsupported`, but one that compiles and then kills
the process. Milestone 64 (enough `std` to run somebody else's crate) found five on
2026-08-18: `env::vars`, `env::temp_dir`, `env::split_paths`, `process::id` and `process::exit`. All
five now answer or exit cleanly. A gap list built from `Unsupported` answers could never show them,
so a check now greps the `sys/` sources the compiler built for bodies that end a process. Each hit is compared against `ABORTS_ACCEPTED` in `xtask/src/farm.rs`, which
carries a reason per entry.

```sh
cargo xtask std-aborts       # on its own, against whatever the farm last built
script/test                  # runs it too: `std-exerciser` ends with it
```

The five calls, how the check works and its own BUGS are in
[the std-aborts appendix](std/std-aborts.md).

## EXAMPLES

Build the patched toolchain and the demo, then check that this worktree's farm matches the main
checkout's before borrowing it:

```sh
cargo xtask std-src           # patch rust-src and link it as this worktree's nife-dev-<dir>-<hash>
cargo xtask std-exerciser     # build std_exerciser for the nife targets against it
cargo xtask std-stamp         # run in both checkouts; equal output means the farms match
```

Find this worktree's toolchain name and use it by hand:

```sh
rustup toolchain list -v | grep "$(pwd)/target/nife-farm"
cargo +nife-dev-nife-1a2b3c4d build ...   # the name the line above printed
```

What a program granted a directory sees, as `std_exerciser` runs it:

```rust
let text = std::fs::read_to_string("/motd")?;            // the same file as "motd"
let err = std::fs::File::open("../motd").unwrap_err();
assert_eq!(err.kind(), std::io::ErrorKind::InvalidFilename); // nothing is above the grant
std::fs::write("f", b"x")?;                                 // CREATE or TRUNCATE, then WRITE
```

And a clean abort sweep:

```
$ cargo xtask std-aborts
std-aborts: 26 process-ending bodies across 79 compiled std sources, all accounted for
```

## The proof

`std_exerciser` is an ordinary Rust program with no `no_std` and no `unsafe`: one binary with three
behaviors, chosen by the authority it was granted. With a directory it walks the `std::fs`
surface; with the network it runs a UDP DNS query and a TCP echo; with neither it runs `Vec`,
`String`, `HashMap` and `Instant` and checks that `fs` and `net` refuse. Three kernel tests spawn it
and compare its output byte for byte on both ISAs. What each branch asserts is in
[the proof appendix](std/the-proof.md).

## BUGS

One line each. The full entry is in
[the caveats appendix](std/caveats.md) unless another link is given.

- A spawned thread's stack has no guard page, and a timed wait spins on `yield`. See
  [the threads appendix](std/threads.md).
- `std::process::exit` exits cleanly, but the kernel drops the code. A supervisor can tell exit from
  crash and cannot tell `exit(0)` from `exit(1)`. §301 (exit status and output) decided how it travels; milestone 172 (a capability-native subprocess primitive) builds it.
- `SystemTime::now()` and `std::random` panic when the process holds no clock or entropy grant,
  because neither call has an error channel. `HashMap`'s seed is the one caller that degrades.
- The environment holds only what a grant seeded (`TZ`, `LANG`, `TERM`) plus what the program sets.
  `current_exe` and `chdir` refuse, `home_dir` is `None`, and `process::id()` is `0`.
- `net` has no peer address on `accept` (it reads `0.0.0.0:0`), no non-blocking mode or timeouts, no
  DNS through `lookup_host`, and no IPv6. A listener's backlog is one connection. A socket's local
  port follows its id, so fast churn can reuse a port slirp has not cleared
  ([the net appendix](std/net.md)).
- `fs` has no `canonicalize`, links, permissions, locks or `File::duplicate`. A revoked FS endpoint
  reads as `PermissionDenied`, because the wire's errno space overlaps the kernel's invoke errors
  ([the fs appendix](std/fs.md)). The rights discipline is tested only under a full-rights grant
  ([the descent appendix](std/fs-descent.md)).
- A file written on nife reads as early 1970, since the FS server holds no clock. A write never moves
  a real timestamp, times are whole seconds, and the handle-shaped time calls refuse
  ([file times](std/file-times.md)).
- stdout and stderr share one endpoint, so they interleave by 16-byte chunk.
- The `std-src` patches are string-anchored to the pinned nightly. A bump that reshapes a dispatcher
  fails with "anchor not found", which is the intended tripwire.
- The legacy `nife-dev` link stays wherever the last pre-2026-10-10 lane put it until the
  maintainer removes it (above). Nothing in the tree reads it; a person typing `+nife-dev` gets
  whichever lane that was.
- `std-aborts` covers `sys/` only, and proves a body reachable, never a call. A stale or foreign
  build under `std_exerciser/target` (left from before a nightly bump) surfaces as a defect or
  an abort, or compiles the nightly's unpatched std even with `RUSTUP_TOOLCHAIN` set (the farm's
  hard-linked `rustc`, since fixed, [caveats](std/caveats.md)). The fix is `rm -rf
  std_exerciser/target` ([appendix](std/std-aborts.md#bugs)). An
  exported `CARGO_TARGET_DIR` used to blind the check; `std-exerciser` now pins and prints it.

## Appendices

| appendix | what it holds |
|---|---|
| [x86-64-target](std/x86-64-target.md) | the x86_64 spec field by field, and the SSE probe |
| [net](std/net.md) | `std::net` over the socket contract |
| [fs](std/fs.md) | `std::fs` over the file contract, and what stays `Unsupported` |
| [file-times](std/file-times.md) | `modified` and `set_times`, with their own BUGS |
| [fs-descent](std/fs-descent.md) | nested paths, `std::fs::Dir`, and the write-path correction |
| [caveats](std/caveats.md) | the full caveat list behind BUGS above |
| [std-aborts](std/std-aborts.md) | the calls that ended a process, and the check that finds them |
| [allocation](std/allocation.md) | the heap under threads: a cache per thread, the allocators surveyed, the measurement |
| [the-proof](std/the-proof.md) | what `std_exerciser` asserts under each grant |
