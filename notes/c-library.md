# A C library for nife: relibc, measured

Written 2026-10-08 (UTC) by lane/c-library, for §265 (a C library started from relibc) and
milestone 835 (a C library, stage 1: files, clock and memory). The file name is this lane's
coinage and provisional.

calef chose relibc on 2026-10-08 (UTC), over keeping the refusal in milestone 478 (tier three: full
POSIX behind the foreign-language seam), musl, and picolibc/newlib: *"B, although I suspect we end
up building our own with a similar approach. We have different constraints than they do."* This note
measures relibc against those constraints before the first stage is shaped, so the shape follows
from the data rather than from the expectation.

The short answer: **take relibc's architecture and its OS-neutral code once, then own the result.**
Do not track upstream as a vendored engine the way `vendor/redoxfs` is tracked. Three findings
decide it, and the table below gives the rest.

## What was read

relibc at `893a3b9133ac2fb3089f71b02d5b61d145d97968` (committed 2026-10-07), a shallow clone of
`gitlab.redox-os.org/redox-os/relibc` in `scratchpad/c-library/relibc`, not in the tree. Counts are
`wc -l` over `.rs` files, comments included. Submodules (`openlibm`, `dlmalloc-rs`) were not
fetched and are not counted.

| Part | Lines | What it is |
|---|---|---|
| `src/` total | 62,424 | the library |
| `src/header/` | 38,648 | 142 header modules: `stdio`, `string`, `stdlib`, `time`, `pthread`, `math`... |
| `src/platform/` | 9,944 | the `Pal` traits and both implementations |
| `src/platform/redox/` | 6,682 | the Redox platform layer |
| `src/platform/linux/` | 1,252 | the Linux platform layer |
| `redox-rt/` | 7,485 | Redox's userspace runtime: fork, signals, process management |
| `src/ld_so/` | 4,016 | the dynamic linker and TLS setup |
| `src/pthread/`, `src/sync/` | 1,947 | its own pthreads, mutexes and condition variables on a futex |

License: MIT (`LICENSE`, "Copyright (c) 2018 Redox OS"), compatible with this tree's MIT/Apache-2.0.
Toolchain: nightly pinned to `nightly-2026-05-24`, built as a `staticlib` by a Makefile that sets
`CC` per target triple (`aarch64-unknown-redox`, `riscv64gc-unknown-redox`, `x86_64-unknown-redox`
and Linux equivalents). Headers are generated from the Rust by cbindgen, one `cbindgen.toml` per
header (140 of them). It pulls about thirty crates, among them `posix-regex`, `chrono-tz`, five
password-hashing crates for `crypt()`, `rand`, a gitlab fork of `object` and two Redox crates from
git.

## How the platform layer is split

Every OS call goes through a `Pal` trait in `src/platform/pal/`: 100 required methods and 23
defaulted in `Pal` itself, plus `PalSignal` (12), `PalSocket` (15), `PalEpoll` (3) and `PalPtrace`
(1). `Sys` is the implementing type, chosen by `cfg(target_os)`. The header modules call
`Sys::openat`, `Sys::clock_gettime` and the rest, so `printf`, `strtod`, `qsort` and the string
functions never learn which OS they are on.

The split is good and it leaks. 149 `cfg(target_os)` lines sit outside `src/platform/`, across about
40 files, and 15 files outside the platform layer name `redox_rt` or the Redox syscall crate
directly (`start.rs`, `ld_so/tcb.rs`, `pthread/mod.rs`, `header/signal/redox.rs` and others). A
third platform has to add a branch at each.

The two implementations' sizes are the measurement that matters most. Linux needs 1,252 lines,
because Linux already is POSIX and the layer is a syscall table. Redox needs 6,682 plus redox-rt's
7,485, about 14,000. Redox is a microkernel that is not POSIX, and relibc emulates the gap in
userspace: fork by copying an address space through the process manager, signals delivered by a
userspace trampoline, a current directory held as a file descriptor. **nife is further from POSIX
than Redox is**, so its platform layer will be the Redox kind, not the Linux kind.

## The constraints, compared

Each row is a place nife's constraints differ from Redox's, and which way it argues.

| Constraint | Redox | nife | relibc's shape | Argues for |
|---|---|---|---|---|
| Who makes syscalls | relibc, from Rust, via `redox_syscall` | the libc's Rust platform layer only, per §31 (the foreign-language seam) as amended by §265 | all OS calls already go through `Pal` in Rust | adapting: this is relibc's design and the reason calef chose it |
| Path namespace | an ambient per-process namespace of schemes; absolute paths resolve through it (`platform/redox/path.rs`, 474 lines) | no ambient root; `/` is the one directory granted on slot 4, `..` above it refused (notes/std.md) | path logic is inside the Redox layer | adapting: a nife `openat` resolves under the grant, as `std::fs` already does |
| File descriptors | kernel file descriptors to scheme handles | none in the kernel; a file is a server-side handle over an endpoint plus a shared page | `int` descriptors everywhere above `Pal` | adapting: the nife layer keeps a userspace table of `int` to (endpoint, handle, page) |
| fork | emulated in userspace (`redox_rt::proc::fork_impl`) | declined permanently (the fork ruling, PR #1856) | `Pal::fork` is required, and `popen`, `system` and `forkpty` call `fork` from generic code | owning: the generic layer assumes fork, and those three have to be rewritten on `posix_spawn` |
| Spawning | `Pal::spawn` exists beside fork | spawn is the supported primitive, milestone 172 (a capability-native subprocess primitive) | `posix_spawn` calls `Sys::spawn` directly, not fork plus exec | adapting: the seam is already where nife needs it |
| Signals | full POSIX signals, emulated in redox-rt (1,162 lines plus per-arch assembly) | none; a fault is an event to the supervisor | `PalSignal`, and `header/signal/redox.rs` outside the layer | owning: nife's answer is a narrow `raise`/`abort` and refusing the rest, a different design rather than a port |
| Process and user identity | pids, uids, process groups, sessions | no process identifier (`std::process::id` is 0); identity is attribution, not authority, per milestone 49 (users, login, and attribution) | about twenty `get*id`/`set*id` methods | adapting: fixed answers in the layer, each in its `BUGS` |
| Memory | `mmap` and `brk` | `untyped::MAP` from the process's own budget, per §22 (Rust `std` on the native ABI) and §31 rule 4 | dlmalloc over `Pal::mmap` | owning the allocator: notes/std.md says `crates/user_mode_heap` is the only heap algorithm, and §31 rule 4 already ties C's heap to it |
| Thread-local storage | static TLS set up by `ld_so/tcb.rs`, with a Redox-specific `OsSpecific` block in the TCB | userspace targets are `singlethread: true`; nothing context-switches `TPIDR_EL0` | TLS and the TCB live in the dynamic linker | owning: a static-only TLS block is a few hundred lines, and the linker it lives in is not wanted |
| Threads | relibc's own pthreads on `rlct_clone` and a futex | milestone 812 (`std::thread::spawn` runs real threads in one address space) builds them, and PR #1856 rules that it will; no futex exists yet | `Pal::futex_wait`/`futex_wake` underneath everything in `src/sync/` | adapting, if 812 gives a wait-on-address primitive; this is a question for 812, not a reason to rewrite `src/sync/` |
| Dynamic linking | `ld.so` and `dlopen` | static ELF only (notes/abi.md, section 3) | `src/ld_so/`, 4,016 lines, entangled with startup and TLS | owning: dropped, and startup rewritten without it |
| Floating point | hard-float targets | userspace is softfloat on all three ISAs (`targets/*.json`); the kernel saves FP state since milestone 447 (a thread's vector registers are its own), but the targets have not flipped, which is milestone 534 (the soft-float targets could now be flipped) | assumes hard float; `libm` and `openlibm` | neither: STREAM's number waits on 534 whichever library runs it |
| Dependencies | whatever Redox wants | each one a ruling under §46 (thin primitives or whole subsystems) | about thirty crates, two from git | owning: stage 1 takes only what it uses, each named in its `Reuse:` line |
| Upstream | Redox's project | agent-written | Redox does not accept LLM-generated contributions (redox-os/redox `CONTRIBUTING.md`, policy adopted March 2026) | owning: a `platform/nife` can never go upstream, so tracking upstream means rebasing a private fork forever |
| Architectures | aarch64, riscv64gc, x86_64, i686 | aarch64, riscv64imac, x86_64, and parity is a gate under §19 (architectural parity is a tenet) | per-arch code in redox-rt and `ld_so` | adapting the generic code, which is arch-neutral; the arch code goes with redox-rt |

Ten rows argue for adapting the OS-neutral code and the `Pal` seam. Seven argue for owning what
sits under and around it. None argues for writing `printf` again.

## The three findings that decide it

**The upstream route is closed.** Redox's project-wide contribution policy refuses LLM-generated
work, and every lane here is an agent. A `platform/nife` that upstream will not take is a permanent
private fork, and the vendored-engine discipline (`vendor/README.md`: the published package plus
exactly this patch, re-verified by `script/vendor-verify`) is priced for a divergence of a few
lines. redoxfs carries 299. This one would be the whole platform layer, startup, TLS, and edits at
149 `cfg` sites, and it would grow with every Redox commit to those files. Fixes can still flow the
other way: under the upstream-hardening rule, a bug found in relibc's generic code becomes a bug
report, which the policy does not forbid.

**The generic layer is not as generic as the trait suggests.** fork is called from three generic
functions; signals reach outside the layer; startup, TLS and pthreads all name Redox crates. Those
are exactly where nife's constraints bite hardest, so they are the files a fork would rewrite
anyway.

**The valuable part is large, OS-neutral and finished.** About 38,000 lines of headers, of which
most (`stdio`, `string`, `stdlib`, `ctype`, `time` formatting, `math` glue, `inttypes`, `locale`)
call nothing but `Pal`. That is the part musl or newlib would also have given us, and relibc gives
it in Rust, behind a trait, which is what lets the C make no syscalls.

## Recommendation: own it, with relibc's architecture, seeded from relibc's code

Neither "adapt" (track upstream with a platform layer added) nor a clean-room rewrite.

1. Take relibc's OS-neutral code once, at a recorded commit, with its MIT notice and a provenance
   line in each file it seeds, into an in-tree crate that nife owns. Keep the `Pal` trait as the
   seam, and cut it down to what the stages need.
2. Leave behind `redox-rt`, `platform/redox`, `platform/linux`, `ld_so`, dlmalloc and every
   dependency a stage does not use.
3. Write the nife platform layer in Rust against the protocol crates `std` already speaks
   (`filesystem_protocol`, `socket_protocol`, `clock_protocol`, `std_runtime_protocol`,
   `user_mode_heap`), so the C library and `std` cannot disagree about a wire format.
4. Rewrite `popen`, `system` and `forkpty` on `posix_spawn`, or refuse them; `fork`
   returns `ENOSYS` and says why in `BUGS`.
5. Watch upstream's generic code by reading, the way `script/vendor-watch` reads redoxfs, and port
   fixes by hand with a commit naming the upstream one.

That is calef's expectation made concrete: relibc's approach, nife's library. Would this still win
if a clean rewrite cost the same? Yes. The seeded code carries years of conformance fixes a rewrite
would have to rediscover, and CLAUDE.md rule 6 asks for adapted code first outside the crates Kani
proves. Would vendoring still lose if it cost the same? Also yes, because the divergence would be
the larger part of the code, and a pin whose patch is bigger than its source is a fork that says it
is not one.

## What stage 1 needs from a nife platform layer

Files, clock and memory, for ioping (834), SQLite's speedtest1 (831) and STREAM
(832), all on PR #1854. The `Pal` subset, from relibc's trait:

- **Files**: `openat`, `close`, `read`, `write`, `pread`, `pwrite`, `lseek`, `fstat`, `fstatat`,
  `ftruncate`, `fsync`, `fdatasync`, `unlinkat`, `mkdirat`, `renameat`, `getdents`, `getcwd`, over
  slot 4's directory grant and the §27 (the filesystem service) file contract. SQLite's `fcntl`
  locks become a no-op behind `SQLITE_THREADSAFE=0` and the `unix-none` locking style, which is a
  real limit and goes in `BUGS`. ioping's `O_DIRECT` is a question for the file server, not for the
  library.
- **Clock**: `clock_gettime` and `gettimeofday` from the clock page on slot 5 and the counter,
  `nanosleep` on the kernel's sleep. `clock_getres` answers the counter's frequency.
- **Memory**: `mmap` of anonymous memory and `munmap` from slot 0's untyped budget; `brk` refused.
  `malloc` is `user_mode_heap`, as §31 rule 4 already has it.
- Process: `exit`, `getpid` (a fixed answer), `getrandom` from slot 6, `uname`, `stdout` and
  `stderr` on slot 1.

What stage 1 must decide in its own block: where the crate lives, its name (an architect's call),
how clang finds the generated headers, and whether cbindgen is taken or the headers are written by
hand (a §46 question).

## What stage 1 found (2026-10-10, UTC)

Milestone 835 (a C library, stage 1: files, clock and memory) built the shape recommended above.
Three findings against this note's predictions:

- The platform layer was the Linux kind in size, not the Redox kind. This note expected nife's
  layer to be near Redox's 14,000 lines. Built on nife's own Rust `std` rather than on the protocol
  crates, it is 1,311 lines (`c_library/src/platform/nife.rs`), because `std` already holds every
  client nife needs: files, clock, heap and standard streams.
- Of relibc's thirty-odd crates, one survived. `libm`, for `math.h`. The rest were replaced by
  a few lines each or by what the tree already had (`crates/calendar` for `chrono`).
- The soft-float ABI is where C and Rust disagree. C on a soft-float target passes a `double`
  vararg in a general register, and Rust's `va_arg::<f64>` on aarch64 and x86_64 reads the
  floating-point save area. relibc's `printf` printed every `double` as 0 until it read the bits as
  a `u64`. SQLite's `speedtest1` did not notice, because SQLite formats numbers itself; ioping did.

## BUGS

- Measured from a shallow clone. Submodules were not fetched, so dlmalloc's and openlibm's sizes are
  missing, and nothing here was compiled. The first compile is stage 1's.
- The Redox contribution policy was read in `redox-os/redox`'s `CONTRIBUTING.md`; relibc's own
  `CONTRIBUTING.md` does not repeat it. Reading the project-wide policy as covering relibc is this
  note's inference.
- The futex row assumes milestone 812 will provide a wait-on-address primitive. If it does not,
  `src/sync/` is rewritten too and that row moves to "owning".
