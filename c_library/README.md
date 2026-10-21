# nife's C library

*Milestone 835 (a C library, stage 1: files, clock and memory), from §265 (a C library started from
relibc, whose Rust platform layer holds the capabilities). Written 2026-10-10 (UTC) by lane
`milestone/835-a-c-library-stage-one-files-clock-and-memory`; threads added 2026-10-12 (UTC) by
milestone 836 (a C library, stage 2: threads). The crate's name, `c_library`, is provisional; §265
leaves it to calef.*

This is how a C program written for POSIX runs on nife without being changed. Three such programs
run in the suite on all three architectures:

- **SQLite 3.50.4's `speedtest1`** (266,378 lines of C), in memory and on a file. Both runs print
  `Verification Hash: 111130 1e792c9db61996c477b8ab5ce2d690052e8dae74824a430a`, the hash of every
  value SQLite read back, which is what the same program prints on macOS.
- **ioping 1.3**, timing five 4 KiB reads of a file it creates in its granted directory. basalt
  builds it as a package, and `cargo xtask basalt-fetch` fetches that package (milestone 906 (ioping is packaged in basalt)).
- **hackbench** (rt-tests 2.11) in its thread mode: up to fourteen `pthread`s passing messages
  over pipes and socket pairs.

Neither source is touched. What differs from a Linux build is on the command line.

## How it is put together

Two halves, in two places, so a reader and a gate can tell them apart.

**relibc's code, in [`vendor/relibc/`](../vendor/relibc/).** Its header modules (`stdio`, `string`,
`stdlib`, `time`, `unistd`, `signal` and the rest, 29,419 lines over 152 files) were seeded once
from relibc at a recorded commit, and nife owns them from then on: no upstream tracking, fixes
ported by hand. They reach the operating system only through relibc's `Pal` trait. Every edit nife
made says `nife:` where it is. The C headers (`vendor/relibc/include/`) are generated from those
modules by cbindgen, as relibc does, and committed. [`vendor/README.md`](../vendor/README.md) lists
what was taken, what was left behind and why.

**nife's code, here.** Every gate the tree runs reads these files:

| file | what it is |
|---|---|
| `src/platform/nife.rs` | the platform layer: `Pal` and `PalSignal` implemented on nife's Rust `std` |
| `src/platform/allocator.rs` | `malloc` and its family on `std`'s allocator, which is `talc` over the process's untyped budget |
| `src/start.rs` | how a `std` program becomes a C program: `argv`, `environ`, `main`, `exit` |
| `src/pthread.rs` | POSIX threads on `std`'s threads: create, join, detach, exit, self |
| `src/platform/futex.rs` | relibc's wait-on-address pair, on `std`'s thread parking |
| `src/platform/stream.rs` | pipes, `socketpair` and `poll`, inside one process |
| `c/thread_start.c` | where a new thread's C begins, so `pthread_exit` can `longjmp` back |
| `src/bin/c_program.rs` | the `std` program a C program is linked into |
| `src/long_double.rs` | the one `long double` symbol x86_64 needs (see BUGS) |
| `build.rs` | links the C program's archive, and compiles relibc's C file and nife's |

The `std` errno mapping is its own crate, [`crates/c_library_errno`](../crates/c_library_errno),
so it is tested on the host.

### Why the platform layer is built on `std`

§265 asks that the C library "speaks the same protocol crates `std` speaks". Building it on `std`
is the strongest form of that: there is one client of the file contract, one of the clock page and
one heap, and a C program reaches each through it. `open` is `std::fs::OpenOptions`, `pread` is a
seek and a read on a `std::fs::File`, `clock_gettime(CLOCK_REALTIME)` is `SystemTime::now()`,
`nanosleep` is `std::thread::sleep`, `malloc` is `std::alloc::alloc`. No C translation unit makes a
syscall, and neither does this crate (§31 (the foreign-language seam) rule 1 as amended by §265).
The cost is that `std` is linked into every C program; `speedtest1` is 1.4 to 2.4 MB per
architecture, inside the 496 MiB an image may have.

### How a C program starts

The program is a `std` program whose Rust `main` is one line, `c_library::start::run(nife_c_main)`.
`std`'s `_start` sets up `std` (heap, arguments from nife's argument page, environment) and calls
that `main`, which builds a C `argv` and `environ` and calls the C program's `main`, renamed
`nife_c_main` in its object by `llvm-objcopy` so the two `main`s do not collide. When it returns,
`exit` runs the `atexit` handlers, flushes the C streams and ends the process through
`std::process::exit`. relibc's `crt0`, its dynamic linker and its TLS setup are not seeded.

### How a thread starts and ends

`pthread_create` spawns a `std` thread with the stack size asked for, holding an `Arc` of the
thread's record (`src/pthread.rs`); the `pthread_t` is a second count of the same `Arc`, which
`pthread_join` or `pthread_detach` gives back. The new thread enters `c/thread_start.c`, which
records a `setjmp` and calls the start routine. Returning from it, or `pthread_exit` (which runs
the cleanup handlers and `longjmp`s back), ends in the same place: key destructors run, the return
value is stored, and `std` ends the thread and frees its stack. relibc's `clone`-based thread
module and its dynamic linker's thread control block are not seeded.

A thread's `errno` is `std`'s `thread_local!`, not relibc's `#[thread_local]`. nife's thread
pointer names `std`'s own per-thread block, so an ELF thread-local would be addressed through it
as if it were something else (BUGS).

### What each call family does

| family | on nife |
|---|---|
| files | a process-local table from `int` to a `std::fs::File`, a directory or a standard stream. `open`, `read`, `write`, `pread`, `pwrite`, `lseek`, `fstat`, `stat`, `fsync`, `ftruncate`, `unlink`, `mkdir`, `rmdir`, `rename`, `access`, `getcwd` |
| clock | `CLOCK_REALTIME` from the clock page (`SystemTime`), `CLOCK_MONOTONIC` from the counter (`Instant`), `nanosleep` real |
| memory | `malloc` on `std`'s heap; anonymous `mmap` from the same heap; `brk` refused |
| process | `exit`, `atexit`, `getenv`, `uname` (`nife`); `getpid` 0 and the user and group calls 65534 (`nobody`), because nife issues no process identifier and attaches no identity to a process; `fork` and `exec*` refused (§264 (`fork` is declined for good)) |
| signals | `sigaction` records, `sigprocmask` masks, and `raise` (or `kill` of this process) delivers synchronously; nothing else ever sends one |
| threads | `pthread_create` is `std::thread::Builder::spawn`; mutexes, condition variables, read-write locks, barriers, semaphores and `pthread_once` are relibc's, waiting through `futex.rs`; keys, `errno` and the locale per thread |
| pipes and sockets | `pipe`, `pipe2` and `socketpair(AF_UNIX, SOCK_STREAM)` are byte queues in the process; `poll` and `select` wait on any of them; every other socket call is `ENOSYS` until stage 3, and so is `getaddrinfo` |
| resources | `getrusage` answers zeros (BUGS); `getpriority` answers nice 0, and `setpriority` takes only 0 |
| refused by design | `dlopen` finds nothing (a program is one static image), `shmget` and its family answer `ENOSYS`, and `syslog` writes to standard error only under `LOG_PERROR` or `LOG_CONS` |

An error from `std` becomes an `errno` by meaning: a missing name is `ENOENT`, a grant without the
right is `EACCES`, no capability at all is `ENOSYS`. `crates/c_library_errno` holds the table.

## EXAMPLES

Fetch basalt's packages of SQLite's `speedtest1` and ioping (milestones 897 (SQLite is packaged in
basalt) and 906 (ioping is packaged in basalt)), then run them in the suite:

```
$ cargo xtask basalt-fetch
basalt-fetch: ripgrep-14.1.1-aarch64, ..., sqlite-3.50.4-x86_64 accepted from .../target/basalt
$ script/test --arch aarch64 --test c_program_tests
test system_tests::user::c_program_tests::unmodified_ioping_times_reads_in_a_granted_directory ...
4 KiB <<< . (  0 B): request=1 time=25.5 ms (warmup)
...
4 requests completed in 2.14 ms, 16 KiB read, 1.87 k iops, 7.29 MiB/s
test system_tests::user::c_program_tests::unmodified_sqlite_speedtest1_runs_in_memory ...
...
Verification Hash: 111130 1e792c9db61996c477b8ab5ce2d690052e8dae74824a430a
test result: ok. 3 passed
```

hackbench's thread mode the same way, from basalt's package (milestone 907 (hackbench is packaged in
basalt)):

```
$ cargo xtask basalt-fetch
$ script/test --arch riscv64 --test c_thread_tests
test system_tests::user::c_thread_tests::unmodified_hackbench_threads_over_pipes ...
Running in threaded mode with 2 groups using 6 file descriptors each (== 12 tasks)
Each sender will pass 100 messages of 100 bytes
Time: 0.050
```

The library's own contract program fetches nothing:

```
$ helpers/build-c-library-contract.sh
$ script/test --arch x86_64 --test c_library_keeps
test system_tests::user::c_thread_tests::the_c_library_keeps_the_pieces_fio_needed ...
ok basename
...
ok select is woken by another thread
...
contract: 0 failed
```

Those latencies are QEMU's, under HVF on an Apple laptop, and are not a benchmark: milestones 831
(SQLite's speedtest1 on nife and Linux) and 834 (ioping on nife and Linux) take the numbers, on
silicon, beside Linux.

Build any other C program the same way. Compile each file with the library's flags, then link:

```
$ CC="$(helpers/c-library-cc.sh)"
$ "$CC" $(helpers/c-library-cflags.sh riscv64-unknown-nife) -O2 -c hello.c -o hello.o
$ helpers/build-c-program.sh hello riscv64-unknown-nife hello.o
build-c-program: .../target/hello/riscv64-unknown-nife/hello (...)
```

`helpers/c-library-cflags.sh` says what each flag is for. The ones that matter most are the
soft-float ABI on each target, which must match the Rust target the program links with.

After changing a seeded module's exported types, regenerate the headers (cbindgen 0.29.0 on
`PATH`):

```
$ helpers/c-library-headers.sh stdio
```

## Decisions this stage made

§265 left these to milestone 835's block. All are reversible.

- **Where the crate lives**: `c_library/` for nife's code, `vendor/relibc/` for the seed, so the
  gates read nife's code and treat the seed as somebody else's, as they do `vendor/redoxfs`.
- **How the headers are made**: generated once by cbindgen, as relibc generates them, and
  committed. No build runs cbindgen; `helpers/c-library-headers.sh` is how to remake them.
- **How clang finds them**: `-isystem vendor/relibc/include` with `-nostdlibinc`, from
  `helpers/c-library-cflags.sh`, plus `-D__nife__`, which the headers' `target_os = "nife"` arms
  test. No compiler defines it, because none has a nife target.
- **The startup path**: `std`'s, as above, rather than relibc's `crt0` and dynamic linker.
- **The ABI values**: Linux's generic ones (open flags, clock ids, `errno`, `struct stat`), by
  extending each of relibc's Linux `cfg` arms to nife, so a program's compile-time assumptions
  match the commonest platform.
- **Dependencies**: relibc's own (about thirty crates) are replaced except `libm`, which `math.h`
  needs (calef, #1896); that is relibc's opt-in Rust `math` module rather than its default,
  openlibm. The rest were small enough to write: musl's `rand` for the `rand` crates, the in-tree
  `crates/calendar` for `chrono`, four-word bit sets for `cbitset`, a `Vec` for `arrayvec`, a
  marker trait for `plain`, and no logging.

Stage 2 (milestone 836) added these, also reversible:

- **Threads are `std`'s**, not relibc's `clone` and control block, for the reason the platform layer
  is on `std`. `bitflags` is a two-line struct.
- **The futex is a parking lot on `std::thread::park`**, rather than a new `std` export of the
  kernel's `WAIT` and `WAKE`, so `std` gains no public API (BUGS: a second crossing).
- **Pipes and socket pairs are queues in the process**, needing no kernel object while both ends
  are in one process.

fio (milestone 833 (the same storage benchmark on nife and Linux)) added these on 2026-10-12 (UTC),
also reversible: `sys/resource.h` without its limits, `libgen.h`, `select` and `pselect` on the
platform layer's `poll`, `O_SYNC` and `O_DSYNC` (a write is synced before it returns), and the
honest refusals in `dlfcn.h`, `syslog.h`, `netdb.h` and `sys/shm.h`, so a program that only links
them builds. `notes/fio-on-nife.md` has what fio needed and why. `fixtures/c/c_library_contract.c`
checks each, built by `helpers/build-c-library-contract.sh` and run by `c_thread_tests`.

## BUGS

- **C's own thread-locals do not link.** A `__thread` or `_Thread_local` variable stops the link
  with `has an STT_TLS symbol but doesn't have a PT_TLS segment`: nife's thread pointer names
  `std`'s block, and nothing lays out an ELF one beside it. It is
  milestone 912 (C thread-locals on nife's thread pointer). fio does not need it: its `configure`
  finds `__thread` missing and uses `pthread_getspecific` instead.
- **`getrusage` answers zeros.** The kernel keeps each thread's on-CPU time (milestone 282 (a
  thread's CPU time, and the `top` it makes possible), §150 (how does a thread's CPU time reach
  userspace?)), but a program reads it only through `SURVEY` on a capability it does not hold, and
  `std` has no way to it. Elapsed time is not passed off as CPU time, so fio prints `usr=0.00%,
  sys=0.00%`, which here means "not measured". The other fields are zero for the same reason, except
  `RUSAGE_CHILDREN`'s, which are true: there are none.
- **No resource limits.** `getrlimit`, `setrlimit` and the `RLIMIT_*` constants are not declared,
  because nife keeps no per-process limit to report; the bounds a process meets are its granted
  memory and 16 threads.
- **`select` leaves `timeout` as it was given**, which POSIX allows; Linux writes back the time
  left, so a program that reuses it without resetting it waits longer here.
- **`O_DIRECT` asks for nothing the server is told.** The library keeps the flag on the
  descriptor and `fcntl(F_SETFL)` may set or clear it, as on Linux, but the file contract has no way
  to ask a server not to cache, so whether a read skips a cache is the server's. `redoxfs_server`
  caches, and there the flag changes nothing; the disk tree (`/disk`, milestone 834 (ioping on nife and Linux)) has no cache,
  so there every read reaches the device with or without it.
- **`O_DSYNC` costs what `O_SYNC` does.** The file contract has one sync, `fsync`'s, so a data-only
  sync also waits for the metadata.
- **No log service.** `syslog` drops a message unless `openlog` asked for `LOG_PERROR` or
  `LOG_CONS`, and then writes it to standard error with `%m` unexpanded.
- **`sys/user.h` does not compile**: it includes relibc's `arch/*/user.h`, which were not seeded.
- **16 threads a process**, the main thread included (`std`'s limit). One more `pthread_create`
  answers `EAGAIN`.
- **Every stack comes out of the heap**, which on nife is memory already committed. relibc's
  default stack is 1 MiB, so a program given an ordinary `std` heap (256 pages) cannot start one
  thread with default attributes; hackbench asks for 64 KiB and its test gives it 1,024 pages.
  A heap that runs out mid-run leaves hackbench waiting on threads that never started, which is
  hackbench's own failure path.
- **Cancellation is deferred, and acted on only at `pthread_testcancel`.** No signal can interrupt
  a thread, so a blocking call is not a cancellation point. A caller-supplied stack is refused.
- **Scheduling parameters change nothing.** nife's scheduler has one class: every priority range
  is 0, and `sched_setscheduler` answers `EPERM`. `sched_getcpu` and CPU affinity are not built.
- **A futex wake crosses twice**: `futex.rs` parks through `std`, which waits on a word of its own
  in the kernel, rather than on the program's word directly. A timed wait burns a core, as `std`'s
  do until the kernel's `WAIT` takes a deadline.
- **File calls take turns.** Every call on a file holds the descriptor table's lock throughout,
  which is also what makes `pread` and `pwrite` (seek, transfer, seek back) atomic. A pipe or
  socket call lets it go before it waits.
- **Pipes and socket pairs cannot leave the process**, a write with no reader answers `EPIPE`
  without `SIGPIPE`, and `poll` is woken by any pipe's change, not only the ones it watches
  (`src/platform/stream.rs`).
- **No `syscall()`**, by §265 and calef's ruling on #2009 (2026-10-12 UTC), so a program that
  calls the kernel by Linux number, as schbench does, does not build.
- **No wall clock without a clock grant.** `time`, `gettimeofday` and `clock_gettime(CLOCK_REALTIME)`
  call `SystemTime::now()`, which stops a process holding no clock with a message naming the
  missing grant, as `std` does, rather than answer 1970. A C program that never asks is unaffected.
  The C answer would be an error, and that needs `std` to say whether it holds a clock without
  panicking, which it cannot yet.
- **Local time is UTC.** nife has no time-zone database, so `localtime` is `gmtime`, `TZ` is not
  read, and `calendar` limits years to 0 to 9999 (`EOVERFLOW` outside).
- **No file locks.** `fcntl`'s record locks answer `ENOLCK`, because the file contract has none.
  SQLite runs with `--vfs unix-none`, its VFS for a platform without locks, which is safe only with
  one process on the file.
- **Inode numbers are path hashes.** `st_ino` is an FNV-1a hash of the path, which SQLite keys its
  per-file state on. A renamed file gets a new number, which a real inode would not.
- **`pread` and `pwrite` move the offset and move it back**, because the file contract has no
  positioned read; atomic because the descriptor table's lock is held throughout.
- **No `dup`, `readdir`, symbolic links or hard links.** Each answers `ENOSYS` or, for
  `readlink`, `EINVAL`. `readdir` is the first a consumer will want; `std::fs::read_dir` can carry it.
- **`mmap` of a file is refused** (`ENODEV`): the file contract offers no shared page. Anonymous
  memory works. `mprotect` is refused.
- **Signals never arrive from outside.** A handler runs only when the program raises its own
  signal. `sigsuspend` and an untimed `sigtimedwait` refuse rather than wait forever.
- **`getrandom` answers `ENOSYS`**, because `std::random` panics when the entropy service was not
  granted and a C program must get an error instead.
- **No `long double` on x86_64.** The x87 is off in a nife process, and clang then refuses the type
  outright, so no C program for x86_64 can use it. On aarch64 and riscv64 `%Lf` reaches relibc's
  conversion and is untested.
- **`isatty` is always false**, there being no terminal interface.
- **Static C destructors do not run**: `exit` does not walk `.fini_array`.
- **`wcwidth` is 1 for every printable character**, which is wrong for East Asian wide characters
  and combining marks; relibc's `unicode-width` was not taken.
- **The CPU-time clock is the monotonic one**, because nife accounts no CPU time to a process.
- **Nothing runs this in CI.** The two programs are fetched from their projects' servers, so the
  tests skip unless their `helpers/build-*.sh` ran. calef ruled on #1896 that foreign programs are
  tested in basalt's package build, not in nife
  (`design/roadmap/0897-sqlite-is-packaged-in-basalt.md`). Milestone 869 (the C library's
  headers cannot drift, and its contract is tested in nife) gives the library its own tests in CI.
