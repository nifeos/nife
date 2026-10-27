# `ripgrep` on nife: what a stranger's program actually hits

*(Milestone 121 (ripgrep), and `design/fatal-risks/README.md` risk 1's decisive experiment. First run
2026-08-31 against `ripgrep` 14.1.1 from crates.io; the search, the refusal and the measurement
2026-10-07 (UTC) by lane `lane/121-ripgrep`.)*

Risk 1 says the platform *"can run hand-written Rust and nothing else, so every piece of software
anyone wants has to be rewritten."* It is the most dangerous of the nine because no amount of kernel
work fixes "nothing runs here". `ripgrep` was chosen as the falsifier because it is not a toy: forty
transitive crates, a filesystem walk, gitignore semantics, memory maps, and threads.

## The answer, in one paragraph

Unmodified `ripgrep` searches a directory it was granted, on all three architectures, with no
change to its own source. One dependency carries one patch, `ignore`'s parallel walker (below). Told `rg --threads 1 --no-mmap --stats 'walk entry'` over milestone 121's priced
tree, it finds the 138 matches in 141 files that are there, reads all 333,984 bytes, and prints
3,813 bytes that are byte for byte what the same `rg` prints on macOS. Through a grant that lacks
`ENUMERATE` it says the capability does not carry the right, searches nothing, and prints no match.
On the same HVF machine the search takes about 1.5 times what it takes on Linux. It needs no
`--no-mmap`, and serially it fits in the 256-page heap the harness gives it with room to spare.
Without `--threads 1` it walks on one thread per core, and the kernel counts them (below).

```
`rg --threads 1 --no-mmap --stats 'walk entry'` printed 3813 bytes and spent 65 heap pages
    | 138 matches
    | 138 matched lines
    | 138 files contained matches
    | 141 files searched
    | 3644 bytes printed
    | 333984 bytes searched
```

## How it runs, and where it does not yet

Through the kernel harness, not the prompt. `system_tests/src/user/ripgrep_tests.rs` spawns `rg`
with `fs_service::start_std_narrowed`. It puts a heap budget at slot 0, an output endpoint at slot
1, a caretaker narrowing `walk/` to the rights under test at slot 4, and the argv at slot 8. The
argv is built from a line typed as it would be at the prompt, by `grant_plan::argv`, the function
`swish` uses. It is placed as the progenitor places it: a `READ` frame mapped read-only at `ARGS_PAGE`
(milestone 205 (how a foreign program is told what to do)). The image loads at its own link address
since milestone 206 (a program image has under 896 KiB) moved the ceiling to 496 MiB.

`rg` reaches that archive only after `cargo xtask basalt-fetch`, which takes the program member of
basalt's published package, accepted under the digest basalt's recipe records at the commit `xtask/basalt-pin` pins (milestone
810 (`ripgrep` is packaged in basalt and installed with `jig`)). CI's `rg-threads` and
`swish-check` rows run it. It is never part of the base image: calef ruled on 2026-10-07 (UTC) that
`rg` is installed with `jig` from a package in nifeos/basalt, and `swish-check` types `jig install
ripgrep` then `rg --version` against the same package.

By its path it runs too (milestone 595 (the shell runs a `std` program)). basalt's build links a
manifest note into `rg` (`cargo xtask foreign-note`), and `script/swish-check` copies the
package's `rg` to its disk as `installed/rg` without its symbol table. Typed `/installed/rg needle
docs ../hay/secret.txt`, it finds the needle in the granted `docs` and cannot open the path outside
it, on all three architectures. The lines skip, and say so, only where nothing was fetched.

## Parity (DECISIONS §19)

| Target | ELF | the search | the refusal | the control |
|---|---|---|---|---|
| `aarch64-unknown-nife` | 5.0 MB | 138 / 141 / 333,984 | refused, 0 searched | found |
| `riscv64-unknown-nife` | 11.5 MB | the same | the same | the same |
| `x86_64-unknown-nife` | 4.4 MB | the same | the same | the same |

Run 2026-10-07 with `script/test --arch <arch> --test ripgrep`, debug kernel under TCG, with `rg`
built from the same 14.1.1 source and flags basalt's recipe uses. All eight tests in the file pass on each. x86_64's OVMF leg has no
RedoxFS disk and skips them, as it skips the walk tests.

The riscv64 ELF is larger from file padding and section alignment rather than code (`.text` 1.23
MiB against aarch64's 1.37). `PT_RISCV_ATTRIBUTES` at virtual address 0 is not a `PT_LOAD`, so the
loader ignores it, as it does for `std_exerciser`.

## Building it

basalt builds it: `recipes/ripgrep.toml` and `helpers/packages.py` in nifeos/basalt download the
published crate and build every target in one pass against a pinned nife, with no vendored copy
and no fork. `ripgrep` is untouched. One dependency is patched: `ignore` 0.4.23, fetched from
crates.io by its checksum and given basalt's `patches/ripgrep-ignore-walk.patch`, about ten lines. Milestone 812 (`std::thread::spawn` runs real
threads in one address space) gave `rg` threads, so it walks in parallel, and `ignore`'s parallel
walker answered "unsupported platform" for every path on a target that is neither `unix` nor
`windows`. calef ruled the patch on pull request #1892; BUGS has its upkeep. What else differs from
a Linux build is on the command line: the target spec (`panic=abort`), `-Zbuild-std` against the
patched `std` (notes/std.md), the link script, and `-Copt-level=s -Cstrip=debuginfo`. That last is
because ripgrep's own release profile sets
`debug = 1` and a 25 MB ELF rides into RAM in the initrd. About 25 seconds a target from cold.

Every crate compiles, including the four milestone 64 (enough `std` to run somebody else's crate) flagged (`ignore`, `crossbeam-deque`,
`memmap2`, `walkdir`). Two resolve to non-Unix implementations without being asked: `memmap2`
compiles its `stub.rs`, and `std::sys::args` compiles nife's own `args/nife.rs`.

## Threads: it asks, and since milestone 812 it gets more than one

`ripgrep` does not assume parallelism, it asks. `crates/core/flags/hiargs.rs` sets its default
thread count to `available_parallelism().map_or(1, |n| n.get()).min(12)`. Until milestone 812
(`std::thread::spawn` runs real threads in one address space) nife's PAL answered `Ok(1)`, so `rg`
took its serial walker and never called `thread::spawn`. Since
2026-10-10 (UTC) the answer is the online core count, and `rg` takes `ignore`'s parallel walker:
`std::thread::scope` and one worker per thread, each walking and searching, while the main thread
waits.

Measured 2026-10-12 (UTC) by `system_tests/src/user/ripgrep_threads_tests.rs` (lane
milestone/121-rg-threads), with QEMU at `-smp 4` on each architecture. The test runs `rg` as a
process, built the way the progenitor builds a `std` job, and reads the kernel's count of threads
that process ever started (`started` on the process page, `sched::process_threads_started`). Nothing
in `rg` is asked to say how many threads it ran.

| `-smp 4`, TCG | `rg --no-mmap --stats 'walk entry'` | `rg --threads 4 ...` | heap pages |
|---|---:|---:|---:|
| aarch64 | 5 threads started | 5 | 269 |
| riscv64 | 5 | 5 | 267 |
| `x86_64` (PVH leg) | 5 | 5 | 269 |

Five is the main thread and four workers, asserted exactly; a serial walk reads 1. Both lines print the fixture's 138 match lines, each once and in whatever
order the workers finished, and the test holds them to `fixture::walk::rg_match_path`'s list. The
host's `rg`, run over the same staged tree with the same two lines, is held to the same list by
`walk_pricing`'s `the_hosts_parallel_rg_prints_the_fixture_lines`, so nife and macOS agree byte for
byte once both are sorted.

Four workers spend 269 heap pages against the serial walk's 65, about 51 pages a worker. At the 256
pages `ripgrep_tests` gives the serial walk, the fourth spawn was refused for want of a page for its
thread control block, so the threaded tests give `rg` 512. CI's `rg-threads` rows run them on
every pull request (`script/ci-build`).

The first full `x86_64` suite hung the four-thread test past its 90 s ceiling at two cores. The PAL
held a spinlock on its one file page across each `CALL`, so idle workers spun while the holder
waited on the caretaker. It is `std`'s futex mutex now (`sys/fs/nife/page.rs`), and the test takes
about 0.5 s there. The comparison with Linux below stays serial on both sides.

## The refusal, through `rg`

```
rg: ./: IO error for operation on ./: this directory capability does not carry the right that verb needs
rg: No files were searched, which means ripgrep probably applied a filter you didn't expect.
...
0 files searched
```

The words after `./:` are the PAL's, for the FS server's `EPERM`. §47 (a directory capability
carries six rights) chose a refusal over an empty listing. It survives the caretaker, std's
`read_dir` and `ignore`'s walker to reach a person as an error. The control holds the same
`READ | DESCEND` grant and is told `rg 'walk entry' narrow/n000`, and finds its line, so the
refusal is about enumeration alone.

## The walk at the prompt

`script/swish-check` stages a copy of `crates/` at `/search/crates` on the first boot when `rg` was
fetched. The second boot inherits the disk, installs `rg` with `jig install ripgrep` from the index
basalt publishes, and types this in `/search` by the bare name (milestone 121's exit 4). `caps rg
BUGS crates` first shows the grant: `crates` alone, read-only, which is `ENUMERATE | READ |
DESCEND` and nothing else.

```
$ rg BUGS crates
  crates/abi/src/lib.rs:    /// # BUGS
crates/abi/src/lib.rs:    /// # BUGS
...
```

The gate holds no list of matches. `cargo xtask basalt-fetch` also fetches ripgrep's own 14.1.1
build for the host, pinned by SHA-256, and staging runs it over the same copy (`--no-config --no-ignore-parent`, so the host sees what the
guest sees) into `target/ripgrep-walk-expected.txt`. The answer must hold every line of that, as
often, and nothing else, in any order, since RedoxFS and APFS list a directory differently. On
2026-10-07 that was 349 lines from 136 files, matched on all three architectures, in 14.9 s
(aarch64, HVF), 40.8 s (riscv64) and 345.8 s (`x86_64`, both TCG). The bounds and their table are
beside `RG_WALK_SECONDS` in `xtask/src/swish_check/launch.rs`.

The walk exposed three faults, each fixed in the same change, among them swish-check's image
growing to 32 MiB (`SWISH_CHECK_DISK_MIB`); [`walk-faults.md`](ripgrep-on-nife/walk-faults.md) has
all three.

## The heap and `--no-mmap`, decided by measurement

- The 256-page heap is enough. Every `rg` run retyped 65 pages of its budget, on all three
  architectures, whether it searched 141 files, one, or none. std's heap grows geometrically (8, 8,
  16, 32), so `rg`'s heap peaked between 128 and 256 KiB. The next doubling fits and the one
  after it reaches the budget. Most of that
  is startup (argument parsing, the regex compile), not the walk: the one-file control spends the
  same. A tree with very large directories costs more, since `read_dir` reads a listing whole.
- `--no-mmap` need not be a default. The control's line omits it and names one file, which is
  exactly where `ripgrep` tries a memory map. `memmap2`'s stub fails, `grep-searcher` reads instead,
  and the line is found. The flag stays on the measured line only so Linux cannot map either.

## The search against Linux

`rg_search` in `script/bench --real --release --smp` (release kernel under HVF, four cores) runs the
line above six times through the caretaker grant and keeps the median of the last five.
`bench/host/run_linux_rg.sh` boots the Alpine `virt` kernel on the same `virt,accel=hvf` machine,
`-cpu host -m 256M -smp 4`, with `walk_pricing`'s `rg_host` example as PID 1. It stages the same
tree, splits the same line with `grant_plan::each_word`, and runs a static Linux `rg` built from
the same source with the same flags, six times, keeping the last five.

| 2026-10-07, M-series host | `rg`'s own clock, median of 5 | spawn to exit |
|---|---:|---:|
| nife, four boots | 1.24 to 1.33 ms | 1.91 to 2.07 ms |
| Linux (Alpine 3.20 `virt`), four boots | 0.82 to 0.87 ms | 1.23 to 1.44 ms (one boot) |
| macOS, APFS, native, reference only | 4.5 to 9.0 ms | 7.5 to 14 ms |
| `fs_walk` on the same nife boots, for calibration | 0.52 to 0.54 ms | |

About 1.5x Linux. Read it with these caveats:

- The figure is ripgrep's own `--stats` time, from after argument parsing to after the last file,
  so process creation and image loading are out of it on both sides. Spawn to exit includes them
  and is less comparable: nife's is the kernel harness spawning a caretaker and `rg`; Linux's is
  `fork` and `exec` from PID 1.
- Warm on both sides. The first run is untimed and fills nife's FS server memo; Linux's tree is
  on tmpfs.
- Not the same allocator. Linux's `rg` drops upstream's musl-only `jemalloc`, which needs a C
  cross-compiler this machine lacks, and uses musl's malloc. Upstream added `jemalloc` because
  musl's is slow for `ripgrep`, so Linux's figure is if anything pessimistic.
- The machine was loaded. Load averages ran 14 to 76 from other lanes. Two further boots on
  each side landed in a slower mode across every figure at once (nife's `fs_walk` read 0.87 ms
  instead of 0.52, Linux 1.7 ms) and are left out, as notes/walk-cost.md leaves out the same mode.
  Inside that mode the ratio was 1.46.
- Wall time only. The block asked for instruction counts too, and this does not take them.
- One tree, one microkernel. This prices this system's file service, not microkernels.

The walk alone is about 0.1 ms slower than Linux's (`fs_walk` 0.52 ms against 0.41 ms in
notes/walk-cost.md), so most of the 0.45 ms gap is in what `rg` does beyond `walk_pricing`'s walk.
Not measured yet: each directory costs `ripgrep` failed opens for `.gitignore`, `.ignore` and
`.rgignore`, and each is a round trip through the caretaker. The bound grant, which skips the
caretaker, made the walk 0.35 ms, and nothing here runs `rg` through it.

```
cargo xtask basalt-fetch              # basalt's rg for all three targets, and the host's reference
script/test --arch aarch64 --test ripgrep
script/bench --real --release --smp   # rg_search, beside fs_walk
sh bench/host/run_linux_rg.sh         # needs rustup target add aarch64-unknown-linux-musl
```

## BUGS

- **`ignore` carries nife's own patch**: basalt's `patches/ripgrep-ignore-walk.patch`, the parallel
  walker on a target that is neither `unix` nor `windows` (milestone 812, #1892). nife maintains it until
  upstream fixes the bug by any route, and re-checks it whenever ripgrep or `ignore` is bumped. The
  upstream issue: (link pending). Re-checked 2026-10-12 (UTC), lane milestone/121-rg-threads: the
  patch does not exist because threads were missing, and it cannot go now that they are here. The
  parallel walker builds each entry with `DirEntryRaw::from_entry_os` and `from_path`, which
  `ignore` implements for `unix` and `windows` only and stubs as "unsupported platform" everywhere
  else; the serial walker uses `walkdir` and never meets them. `ignore` 0.4.33, the newest on
  crates.io that day, still has the stubs. So the parallel walk needs the patch, `ripgrep`'s own
  source is still unmodified, and its dependency tree is not.
- Spawning `rg` at the prompt is slow, and where the time goes is unmeasured. Lines that stop at
  argument parsing took 3.8 to 30.1 s on aarch64 under HVF, and the `caps` line that hashes the
  2.8 MiB image up to 38.7 s on `x86_64`, with other lanes loading the host (2026-10-07). The
  image passes through the shell's staging and the progenitor's hash; which dominates is open.
  These lines get a 90 s bound of their own (`RG_LINE_SECONDS`) rather than the shared 30 s.
- The walk costs 345.8 to 432.1 s on `x86_64` under TCG, against 14.9 to 54.1 s on aarch64 under
  HVF. Why is unmeasured; the screen path through TCG is the suspect `SWISH_CHECK_X86_LINE_SECONDS`
  already documents.
- The walk's peak memory at the prompt is unmeasured, since the shell reports no per-job heap. It
  finished inside the `std` heap on all three legs. The host's `rg` with `--threads 1 --no-mmap`
  over the same copy peaks at a 1.42 MiB footprint (2.7 MiB resident) on macOS.

- **The kernel suite's `rg` tests run in CI only in the `rg-threads` rows**, at four cores, with
  the threaded walk. The `test` job fetches no `rg`, so they skip there. CI's `swish-check` rows
  type `rg`'s lines, the walk included, on all three legs, and refuse to skip them. The timings
  above are from one machine on one evening.
- **The gap to Linux is not attributed.** The failed ignore-file opens are a guess, not a
  measurement; a request count like the one milestone 606 (a directory walk costs what it does on Linux) took
  with `walk_model` would settle it.
- **Gitignore semantics are untested.** The fixture holds no ignore files, and neither does the
  prompt's corpus (whatever `crates/` holds when the gate runs), so `ignore`'s matching has not
  been observed here, only its walking.
- **The threaded walk is proved in the kernel harness, not at the prompt.** `swish-check`'s walk
  runs with no thread flag, so on aarch64 and riscv64 at four cores it walks in parallel, but
  nothing there counts its threads, and the `x86_64` leg boots OVMF with one core. The harness is
  where the kernel's count can be read.
- **Threaded memory grows with cores.** About 51 heap pages a worker, and `ripgrep` takes up to 12
  workers, so on a twelve-core machine the default line would want about 680 pages. The prompt's
  `std` region is 384 (`grant_plan::STD_REGION_PAGES`). Unmeasured above four cores.
- **One version, one program.** `ripgrep` 14.1.1 is evidence about this platform, not a survey of
  crates.io.
- **The harness's `rg` is not installed by `jig`.** The kernel harness and the by-path lines run the
  package's program member, taken out on the host with `package_index::accept`, the installer's
  own decision. Only `swish-check`'s `jig install ripgrep` takes the installer's path.
- **The harness's argv page is the kernel's, not the progenitor's.** It writes the same bytes with
  the same function and maps them the same way, but the path a command line takes from `swish`
  through the progenitor is milestone 205's tests, not these.
