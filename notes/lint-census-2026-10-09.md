# The lint census, 2026-10-09 (provisional name)

Measured 2026-10-09 (UTC), read-only, at base `2d59ddde1`, on nightly-2026-10-09 (clippy 0.1.101).
A maintainer session asked for five proposed milestones on panics, casts, `static mut`,
suppressions and function length, each with numbers taken first. This note holds those numbers
once, so the five proposals in `design/roadmap/proposals/` cite it rather than repeat it. The
scripts are in [lint-census-2026-10-09/](lint-census-2026-10-09/) and run from the repository root.

## Scope, and why the first counts were wrong

The request arrived with grep counts: 814 `.unwrap()`/`.expect()` in `kernel/src`, about 5,000
integer casts and about 90 `static mut`. Those count test code, comments and `&'static mut`. The
census counts the Rust that runs on nife and is not test code. It starts from `git ls-files '*.rs'`
and drops `vendor/`, then the unsafe census's host-only list (`helpers/rust_source.py`'s
`HOST_ONLY` and `HOST_TOOL_CRATES`). Test code goes too: `system_tests/`, `fixtures/`, `bench/`,
the three `*_exerciser/` workloads, `build.rs`, any `tests/` or `benches/` directory, and every item
under a `#[cfg]` that is false with `test`, `kani`, `loom` and the test-only features off
([scope.py](lint-census-2026-10-09/scope.py)). That leaves 408 files, 132 of them under
`kernel/src/`.

Two instruments, because they answer different questions:

- Text counts ([census.py](lint-census-2026-10-09/census.py)) strip comments and literals with
  `helpers/rust_source.py`'s stripper, then the test items, then match a pattern.
- Lint hits ([clippy-runs.sh](lint-census-2026-10-09/clippy-runs.sh),
  [clippy_counts.py](lint-census-2026-10-09/clippy_counts.py)) run clippy without `--all-targets`
  over five configurations: the host workspace, the kernel on each of the three architectures, and
  `redoxfs_server`. A site compiled three times counts once. Default features only, so the
  kernel's build modes are outside the count. The five passes took 14 seconds with dependencies
  already built.

## Panics

| measure | in scope | kernel |
|---|---|---|
| `.unwrap()` / `.expect(` (text) | 61 / 529 | 44 / 485 |
| `panic!` / `unreachable!` / `todo!` / `unimplemented!` (text) | 69 / 5 / 0 / 1 | 50 / 4 / 0 / 1 |
| `unwrap_used` / `expect_used` (lint) | 58 / 438 | 41 / 418 |
| `panic` / `unreachable` / `unimplemented` (lint) | 59 / 3 / 1 | 44 / 3 / 1 |
| `indexing_slicing` (lint) | 2,282 | 282 |

The 814 became 459 lint hits in the kernel once tests left. Of the 418 `expect_used`, 251 are in
the boot-time service builders (`kernel/src/user/*_service.rs`), 70 in `user.rs` and 41 in
`sched.rs`. A random sample of 25 kernel panic sites read as boot construction or a stated
invariant ("no scheduler", "unhandled x86_64 exception"); none was on a system call argument path.

The 32 crates this census reads as parsing bytes from a less-trusted party carry 24 panic hits
between them. Their exposure is indexing: 719 `indexing_slicing` hits.

| group | crates | indexing | arithmetic | panics |
|---|---|---|---|---|
| disk and image formats | 8 | 247 | 220 | 16 |
| network | 5 | 61 | 61 | 0 |
| device-supplied | 10 | 289 | 461 | 7 |
| peer-process contracts | 9 | 122 | 143 | 1 |

`fuzz/` has 10 targets. They reach 12 of the 32 crates directly; the other 20 have none.

## Casts and arithmetic

`as <integer>` appears 3,262 times in scope, 945 in the kernel. Clippy's view is narrower:

| lint | in scope | kernel |
|---|---|---|
| `cast_possible_truncation` | 844 | 271 |
| `cast_possible_wrap` | 155 | 30 |
| `cast_sign_loss` | 185 | 3 |
| `arithmetic_side_effects` | 3,218 | 648 |

340 of the 844 truncation hits are `u64` or `i64` to `usize`, the class that removed this lint in
§61 (a lint is adopted on evidence from this tree). Clippy still has no setting for it.

## `static mut`

52 declarations in all tracked Rust, 42 in scope. 27 are in `components/`, 3 in `uefi_loader`, 2
each in `redoxfs_server` and `crates/loaded_image_check`, and 8 in the kernel. Of the kernel's 8,
five are under `arch/x86_64/` (`IDT`, `TSS`, `GDT`, `INSTALLED_PORT_GRANT`, `BENCH_IOMAP`) and three
are portable (`screen.rs`'s `PIXELS` and `SCRATCH`, `user/fs_service.rs`'s `SERVER_MAPS`). The
aarch64 and riscv64 layers have none. `static_mut_refs` fires nowhere: it is deny-by-default in
edition 2024, checked with a two-line program on this toolchain.

## Suppressions

| measure | in scope | kernel |
|---|---|---|
| `allow(...)` attributes (text) | 457 | 354 |
| of them `dead_code` | 344 | 328 |
| of them bare `#[allow(dead_code)]`, no `cfg_attr` | 48 | 38 |
| of them `clippy::` | 28 | 11 |
| of them `unused*` | 7 | 6 |
| `allow_attributes_without_reason` (lint) | 380 | 294 |
| `#[expect(...)]` attributes | 1 | 1 |

`TODO`, `FIXME` or `XXX` appears on 7 lines of tracked Rust, and none is a marker. At the base, four
of them pointed at a TODO that no longer existed; 1c7849691 points them at where the work went. The 3
`#[ignore]` attributes all carry a reason.

## Function length and complexity

`too_many_lines` counts code lines, not blank or comment lines. Over 5,064 functions in scope:

| | p50 | p90 | p95 | p99 | max | over 100 | over 150 | over 200 |
|---|---|---|---|---|---|---|---|---|
| in scope | 5 | 26 | 41 | 89 | 977 | 39 | 14 | 10 |
| kernel (1,619) | 5 | 30 | 45 | 86 | 977 | 8 | 3 | 3 |

The longest is `kernel_main` (977). `cognitive_complexity` has p99 10 and max 80, with 9 functions
over clippy's default of 25. `excessive_nesting`, run on the host and aarch64 configurations only,
finds 243 blocks in 56 files at a threshold of 4 and 10 blocks in 4 files at 6.

## BUGS

- The test strip is a lexer, not a parser. A `#[cfg]` on a statement inside a function ends at the
  next `;`, `,` or brace at depth zero, which is right for the shapes in this tree and was not
  proven for every shape.
- The 32-crate parser list is this census's reading, not a ruling. It is the first open question
  in the panic proposal.
- Lint counts cover default features. A site compiled only under a kernel build mode is missing.
