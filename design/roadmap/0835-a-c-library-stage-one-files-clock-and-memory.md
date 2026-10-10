---
status: BUILT
raised: 2026-10-08
built: 2026-10-10
milestone_dependencies: none
decision_dependencies: 31, 265
machine_requirements: none
specific_machine: none
needs_person: no
---
# 835. A C library, stage 1: files, clock and memory

*(Minted 2026-10-08 (UTC) by lane/c-library from calef's ruling the same day; number provisional
until the merge queue lands it. The file name is a lane's coinage.)*

calef chose relibc for nife's C library on 2026-10-08 (UTC), expecting "we end up building our own
with a similar approach". §265 (a C library started from relibc) records the ruling.
notes/c-library.md is the evaluation this stage is shaped by: take relibc's OS-neutral code and its
`Pal` seam once, into an in-tree crate nife owns, and write the nife platform layer in Rust. This
stage is the smallest library that runs a real C benchmark unmodified. It supersedes part of
milestone 478 (tier three: full POSIX behind the foreign-language seam).

## First consumers

Numbered as PR #1854 mints them, provisionally: ioping (834), SQLite's speedtest1 (831) and STREAM
(832). Each needs files or memory and a clock, and none needs threads, sockets or a second process.
STREAM's number means little until the userspace targets stop being softfloat (that is milestone 534
(the soft-float targets could now be flipped)); it can still run first, as the library's simplest
workload.

Reuse: relibc (MIT), chosen by calef on 2026-10-08 over musl and picolibc/newlib; its OS-neutral
header code and `Pal` seam are seeded, its Redox runtime, dynamic linker and dlmalloc are not
(notes/c-library.md has the measurement). The platform layer reuses the protocol crates `std` speaks
and `crates/user_mode_heap`.

## What it builds

- The seed: relibc's header modules a stage-1 consumer reaches, copied from a recorded relibc
  commit with its MIT notice and a provenance line per file. Nothing from `redox-rt`, `ld_so`,
  `platform/redox` or `platform/linux`. Each crate dependency kept is named in the `Reuse:` line
  with a reason, under §46 (thin primitives or whole subsystems).
- The platform layer, in Rust, against the protocol crates `std` already speaks:
  - files: `openat`, `close`, `read`, `write`, `pread`, `pwrite`, `lseek`, `fstat`, `fstatat`,
    `ftruncate`, `fsync`, `fdatasync`, `unlinkat`, `mkdirat`, `renameat`, `getdents`, `getcwd`, on
    slot 4's directory grant through `filesystem_protocol`, with a userspace descriptor table;
  - clock: `clock_gettime`, `gettimeofday`, `clock_getres`, `nanosleep`, from slot 5's clock page
    and the counter;
  - memory: anonymous `mmap` and `munmap` from slot 0's untyped budget, `malloc` on
    `user_mode_heap`, as rule 4 of §31 (the foreign-language seam) has it; `brk` refused;
  - process: `exit`, `getrandom` on slot 6, `uname`, stdout and stderr on slot 1, and fixed answers
    for the identity calls, each in `BUGS`.
- `fork` returns `ENOSYS`. No C translation unit contains a syscall instruction, and `syscall()` is
  not built (§31 rule 1 as amended by §265).
- All three architectures (§19 (architectural parity is a tenet)), proven by the same test.

## What this stage decides in its own block

Where the crate lives, and its name (provisional until calef rules). Then whether cbindgen generates
the headers or they are written by hand, how `user/build.rs`'s clang finds them, and the startup
path (`crt0`, static TLS for one thread) that replaces relibc's dynamic linker.

## Done when

ioping and SQLite's speedtest1 build from their unmodified upstream sources against this library
and run to completion on nife under QEMU on all three architectures, and a host test proves the
platform layer's `errno` mapping. STREAM building and running counts too; its number waits on 534.

## What is built (2026-10-10, lane `milestone/835-a-c-library-stage-one-files-clock-and-memory`)

Two unmodified C programs run in the suite on aarch64, riscv64 and x86_64 under QEMU
(`system_tests/src/user/c_program_tests.rs`, three tests), each skipping when its helper has not
been run, as `rg`'s do:

- SQLite 3.50.4's `speedtest1` (`helpers/build-speedtest1.sh`; 266,378 lines of C), with
  `--testset main --size 1 --verify`, once in memory and once on a file in a directory granted
  alone (`--vfs unix-none`). All 32 tests run, and on each architecture both runs print
  `Verification Hash: 111130 1e792c9db61996c477b8ab5ce2d690052e8dae74824a430a`, the hash of every
  value SQLite read back, which is what the same program prints on macOS.
- ioping 1.3 (`helpers/build-ioping.sh`), five 4 KiB reads of a working file it makes with
  `mkstemp` in its granted directory and removes. It reports four counted requests and 16 KiB read.

The full suite passes on all three architectures with both programs present. The frame ledger
holds: 22,838 frames kept on aarch64 and 22,782 on riscv64, against a budget of 23,764. Each test
ends its caretaker and its clock service (`clock_service::start_in`, from milestone 801 (packages
over the internet)). A host test proves the `errno` mapping
(`crates/c_library_errno`, three tests), and `c_library` checks at compile time that its `errno.h`
values equal the mapping's.

The library is relibc's OS-neutral code seeded into `vendor/relibc/` (29,419 lines, every file
stamped with its provenance) and nife's own platform layer in `c_library/` (1,613 lines), which
implements relibc's `Pal` on nife's Rust `std`. [`c_library/README.md`](../../c_library/README.md)
is the guide, with its `BUGS`; [`vendor/README.md`](../../vendor/README.md) says what was taken and
left.

ioping found what `speedtest1` could not. SQLite formats numbers with its own `printf`, so a C
library whose `printf` lost every `double` argument still produced SQLite's hash. ioping prints
through the library's `printf`, and every size and time it printed was 0: on nife's soft-float
ABI a `double` vararg travels in a general register, and Rust's `va_arg::<f64>` on aarch64 and
x86_64 reads the floating-point save area. The seed now reads the bits as a `u64`, and the ioping
test asserts the figure that exposed it.

## What this block decided

Each was left to this block by §265 (a C library started from relibc, whose Rust platform layer
holds the capabilities), and each is reversible:

- Where the crate lives: nife's code in `c_library/`, the seed in `vendor/relibc/`, which every gate
  already treats as somebody else's code. That also means about 580 seeded `unsafe fn`s carry no
  `# Safety` section and no gate asks; `vendor/README.md` says so.
- The headers: generated once by cbindgen 0.29.0, as relibc generates them, and committed;
  `helpers/c-library-headers.sh` remakes them. No build runs cbindgen.
- How clang finds them: `helpers/c-library-cflags.sh`, which also fixes each target's soft-float
  ABI to match its Rust target (on x86_64 that needs SSE2 and the x87 off too, so clang has no
  `long double` there).
- The startup path: `std`'s `_start`, with the C `main` renamed in its object; no `crt0`.
- The platform layer on `std` itself rather than on the protocol crates, so a C program and a Rust
  program cannot disagree about a wire format.
- The ABI values are Linux's generic ones (relibc's Linux `cfg` arms extended to nife).

## Which fatal risk it serves

Risk 1 (only software written for nife runs on nife): two foreign programs, neither Rust nor
related to `ripgrep`, now run unmodified on all three architectures. One does a database's work,
with results identical to macOS's. They run only where their helpers were run,
never in CI, and under QEMU, not on silicon. The verdict is calef's (§216 (fatal-risk facts are
correctable, and verdicts are the architect's)).

## BUGS

- No signals. `sigaction` records a handler that never fires. Stage-1 consumers are checked for
  depending on one.
- SQLite runs without file locking (`unix-none`), which is safe only with one process on the file.

## Follow-on

- **Milestone 831.** Milestone 831 (SQLite's speedtest1 on nife and Linux, a real database
  workload): the same program against Linux on silicon, now that it runs here.
- **Milestone 834.** Milestone 834 (ioping on nife and Linux, storage latency one request at a
  time): the same program against Linux on xenon, now that it runs here.
- **Milestone 836.** Milestone 836 (a C library, stage 2: threads): the descriptor, `errno` and
  signal tables are single cells sound only for one thread, and stage 2 replaces them with locks.
- **Milestone 868.** Milestone 868 (relibc's seed comes under the unsafe gates): the seed's
  `unsafe fn`s gain `# Safety` sections and the census counts them, as calef ruled on #1896.
- **Milestone 869.** Milestone 869 (SQLite and ioping run in CI, and the C headers cannot drift):
  CI builds and runs both programs, and regenerates the headers and fails on a difference.
- **Recorded.** The rest of this stage's limitations are in `c_library/README.md`.

## Index row

The first C library that runs somebody else's C program unmodified on nife: relibc's OS-neutral
code, seeded once, over a Rust platform layer that holds the process's file, clock and memory
capabilities, so the C itself makes no syscalls.
