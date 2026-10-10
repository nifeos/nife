# nife's C library

*Milestone 835 (a C library, stage 1: files, clock and memory), from §265 (a C library started from
relibc, whose Rust platform layer holds the capabilities). Written 2026-10-10 (UTC) by lane
`milestone/835-a-c-library-stage-one-files-clock-and-memory`. The crate's name, `c_library`, is
provisional; §265 leaves it to calef.*

This is how a C program written for POSIX runs on nife without being changed. Two such programs run
in the suite on all three architectures:

- **SQLite 3.50.4's `speedtest1`** (266,378 lines of C), in memory and on a file. Both runs print
  `Verification Hash: 111130 1e792c9db61996c477b8ab5ce2d690052e8dae74824a430a`, the hash of every
  value SQLite read back, which is what the same program prints on macOS.
- **ioping 1.3**, timing five 4 KiB reads of a file it creates in its granted directory.

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
| `src/platform/allocator.rs` | `malloc` and its family on `std`'s allocator, which is `crates/user_mode_heap` |
| `src/start.rs` | how a `std` program becomes a C program: `argv`, `environ`, `main`, `exit` |
| `src/bin/c_program.rs` | the `std` program a C program is linked into |
| `src/long_double.rs` | the one `long double` symbol x86_64 needs (see BUGS) |
| `build.rs` | links the C program's archive, and compiles relibc's one C file |

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

### What each call family does

| family | on nife |
|---|---|
| files | a process-local table from `int` to a `std::fs::File`, a directory or a standard stream. `open`, `read`, `write`, `pread`, `pwrite`, `lseek`, `fstat`, `stat`, `fsync`, `ftruncate`, `unlink`, `mkdir`, `rmdir`, `rename`, `access`, `getcwd` |
| clock | `CLOCK_REALTIME` from the clock page (`SystemTime`), `CLOCK_MONOTONIC` from the counter (`Instant`), `nanosleep` real |
| memory | `malloc` on `std`'s heap; anonymous `mmap` from the same heap; `brk` refused |
| process | `exit`, `atexit`, `getenv`, `uname` (`nife`); `getpid` 0 and the user and group calls 65534 (`nobody`), because nife issues no process identifier and attaches no identity to a process; `fork` and `exec*` refused (§264 (`fork` is declined for good)) |
| signals | `sigaction` records, `sigprocmask` masks, and `raise` (or `kill` of this process) delivers synchronously; nothing else ever sends one |

An error from `std` becomes an `errno` by meaning: a missing name is `ENOENT`, a grant without the
right is `EACCES`, no capability at all is `ENOSYS`. `crates/c_library_errno` holds the table.

## EXAMPLES

Build SQLite's `speedtest1` and ioping for all three architectures, then run them in the suite:

```
$ helpers/build-speedtest1.sh
build-speedtest1: compiling SQLite 3.50.4 for aarch64-unknown-nife
build-c-program: .../target/speedtest1/aarch64-unknown-nife/speedtest1 ( 1519664 bytes)
...
$ helpers/build-ioping.sh
...
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

## BUGS

- **One thread.** The descriptor table, `errno` and the signal table are single cells in `static`s,
  sound only because a stage-1 process has one thread. Stage 2, milestone 836 (a C library, stage
  2: threads), replaces them with locks.
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
  positioned read; atomic only because there is one thread.
- **No `dup`, `pipe`, `readdir`, symbolic links or hard links.** Each answers `ENOSYS` or, for
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
- **Nothing runs this in CI yet.** The two programs are fetched from their projects' servers, so
  the tests skip unless their `helpers/build-*.sh` ran, as `rg`'s did. calef ruled on #1896 that CI
  fetches them; that is milestone 869 (SQLite and ioping run in CI, and the C headers cannot
  drift), which also fails CI when the committed headers differ from what cbindgen makes.
