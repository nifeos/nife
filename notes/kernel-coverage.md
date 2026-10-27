# Kernel coverage under QEMU

*Name: provisional, minted 2026-10-27 (UTC) by the lane for milestone 781 (measure kernel and
service coverage under QEMU).*

`script/coverage` sees only the host crates. `script/coverage --kernel` builds the kernel's
unit-test image with `-C instrument-coverage`, boots it under QEMU, and reads the counters the
kernel wrote to the host as it exited. The report has the regions `cargo llvm-cov` reports for the
host crates, so `helpers/error_paths.py` reads it unchanged. It is a report and never a gate, and
no CI job runs it. The roadmap block is
[0781](../design/roadmap/0781-measure-kernel-and-service-coverage-under-qemu.md); it holds the
architect's ruling of 2026-10-27 (UTC) and what remains.

## How it works

1. `cargo xtask coverage` prepares the same userspace archive and test disks the aarch64 leg of
   `script/test` uses, then builds `kernel`'s test image with `RUSTFLAGS="-C instrument-coverage
   -Zno-profiler-runtime"` and `--features coverage`, in `target/kernel-coverage/`. A target
   directory of its own keeps the ordinary build cache valid. The explicit `--target` keeps build
   scripts and procedural macros uninstrumented, and the userspace archive builds before the flags
   exist, so no user program carries counters.
2. The `coverage` feature links `minicov` and compiles `kernel/src/coverage.rs`. When the image
   exits, `arch::semihosting::exit` calls `coverage::dump`, which hands the counters to `minicov`
   with a sink that writes through semihosting `SYS_OPEN`, `SYS_WRITE` and `SYS_CLOSE`. The dump runs
   on the failure paths too. The dump needs no allocator and no filesystem. The host path comes from
   `NIFE_COVERAGE_PROFILE`, which `kernel/build.rs` bakes in.
3. `kernel/link-aarch64.ld` places the profiling sections. The runtime finds the counters and the
   per-function records by their `__start_` and `__stop_` symbols, which the linker defines only for
   orphan sections, so the script defines them inside `.data`. The function names get an output
   section of their own, `__llvm_prf_names`, because `llvm-cov` finds names by that section name. The
   runtime is C compiled position-independent, so its global offset table goes into `.rodata`
   instead of an unmapped orphan page. In a build without the instrument these are all empty, and
   the flat image is byte for byte the one the old script linked (checked 2026-10-27 with `cmp`).
4. `llvm-profdata merge` and `llvm-cov show -format=html` and `export -format=lcov` produce the
   reports in `target/kernel-coverage/html/` and `lcov.info`. `helpers/error_paths.py` reads the HTML.
   A file the report measures counts once, in the measured table.

The boot runs under `helpers/qemu-bounded.sh` with a 2,400 second limit. One run takes about four minutes
after the userspace builds.

## The dependency: minicov 0.3.9

Audited 2026-10-27 (UTC) for §46 (thin primitives or whole subsystems; we write everything in
between) and the kernel's bar of 2026-10-25.

| Fact | Value |
|---|---|
| Version, published | 0.3.9, 2026-08-14 (more than two weeks before this pin); `=0.3.9` in `kernel/Cargo.toml` |
| License | MIT or Apache-2.0 (the crate writes `Apache-2.0/MIT`); `cargo deny` passes |
| Author and upkeep | Amanieu d'Antras (Rust libs team), `github.com/Amanieu/minicov`; nine releases since 2023-03, the last two on 2025-12-05 and 2026-08-14; about 23 million downloads |
| What it is | 349 lines of Rust, plus ten C files (1,847 lines) of LLVM's `compiler-rt/lib/profile`, taken from llvm-project 23.1.0-rc3 with `c/llvm.diff` removing the libc dependence |
| Normal dependencies | none; `default-features = false` drops its `alloc` feature |
| Build dependencies | `cc` 1.4.0, `walkdir` 2.5.0; transitively `shlex`, `find-msvc-tools`, `same-file`, `winapi-util`, `windows-sys`, `windows-link` |
| New in `Cargo.lock` | `minicov`, `walkdir`, `same-file`, `winapi-util`; the rest were already locked |
| C build step | yes: `build.rs` runs `clang` (hard-coded name) over the ten files for the target |

**The C toolchain.** The step needs a `clang` with the target's backend. CI already installs one:
`script/bootstrap` installs Ubuntu's clang for the C library (milestone 835), and it carries the
AArch64, RISC-V and X86 backends. On a Mac, Apple's clang has no RISC-V backend, and
`helpers/c-library-cc.sh` already documents the Homebrew LLVM that does. Only a build with the
`coverage` feature runs the step. Nothing in `script/test`, `script/ci-build` or a shipping image does.

**The kernel's bar.**

1. `no_std`: builds and links for `aarch64-unknown-none-softfloat` and runs. riscv64 and x86_64 are
   not built yet: `kernel/src/lib.rs` refuses the feature there with a `compile_error!`, and the
   milestone lists them as later pull requests. Each needs its own exit channel for the file.
2. Parsing input: it parses none. It serializes the counters.
3. Graph: one crate compiled into the kernel (`minicov`), against a cap of three. The build-time
   crates are the list above, which `Cargo.lock` pins and `deny.toml` gates.
4. `unsafe`: Rust, four blocks (`check_version`, `coverage_enabled`, `reset_coverage`,
   `module_signature`), two `unsafe fn` (`capture_coverage`, `merge_coverage`), and the `extern "C"`
   callbacks the C side calls. The kernel reaches `capture_coverage` and, through it,
   `check_version`. The C is LLVM's own, built for freestanding use. What checks it upstream: LLVM's
   test suite for the profile runtime, and the runtime version assertion in `check_version`, which
   panics when the compiler and the runtime disagree about the raw profile format. No claim in nife
   rests on any of it, because only the test image links it.
5. Upgradable: a bump is a deliberate commit. The run itself tells a mismatch: a compiler whose
   profile format the runtime does not know fails the assertion at exit.

The dependency is behind the `coverage` feature, so `cargo tree -e normal -p kernel` for the three
targets stays as `notes/kernel-dependency-unsafe.md` lists it, and `helpers/kernel_dependency_unsafe.py`
needs no row for it.

## First measurement

Run 2026-10-27 (UTC) at the commit that introduced this note, aarch64, the 141 tests of the kernel's
own image, all passing instrumented. Source files compiled into that image only (`kernel/src` on
aarch64), counted by `helpers/error_paths.py`:

| | paths | reached | not reached | unmapped |
|---|---:|---:|---:|---:|
| Result family | 449 | 44 | 394 | 11 |
| Option family | 828 | 277 | 531 | 20 |
| cleanup paths (either family) | 70 | 4 | 66 | 0 |

So the unit-test image reaches 44 of the kernel's 449 Result-family error paths (10%) and 277 of its
828 Option-family ones (33%). Four of its 70 cleanup paths run. The kernel's riscv64 and x86_64
files, and the features this image does not build, stay in the unmeasured table.

The crates the kernel links are in the same report (`device_tree_blob`, `paging`, `capability` and
others), measured by this image alone. They have host tests that this image does not run, so read
their rows as what the kernel image reaches, not as their coverage.

## BUGS

- This is the unit-test image. The whole-system image (`system_tests/`) boots the services and
  exercises the syscall surface, and it would raise every number above. The milestone lists it first.
- The instrumented image is slower. Tests that time themselves or measure stack depth saw different
  numbers (the IPC stack-depth floors rose by about 130 bytes) and still passed, but a future
  margin could fail here and not in the plain image. The report says what the tests reach, not
  whether they pass.
- Counters are plain integers, bumped without atomics on all four cores. A count can lose an
  increment; whether a region was reached stays the same.
- `helpers/error_paths.py` is a source-level reading. An unmapped path (31 here) is one the report
  carries no region for, such as an `.ok_or(e)` on a reached line.
