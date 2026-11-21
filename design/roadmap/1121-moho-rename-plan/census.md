# The census

An appendix to [the milestone](../1121-moho-rename-plan.md).

`git grep -c -E <pattern>` at `nifeos/nife@97d16ae8b` and `nifeos/basalt@2c437ca`, summed by file
kind. It counts lines, not occurrences. Each line takes its kind from the path, in this order:
`vendor/`, `patches/std-nife/` (the overlay), `xtask/`, `.github/`, `targets/`, any `build.rs`,
`helpers/` and `script/`, then `*.md`, `Cargo.toml` and `Cargo.lock`, `*.rs`, other `*.toml`, and
everything else.

## nife

| spelling | pattern | lines | files | by kind (lines / files) |
|---|---|---|---|---|
| `abi::` | `(^\|[^A-Za-z0-9_])abi::` | 2,190 | 450 | Rust 1,741 / 232; prose 325 / 178; overlay 41 / 7; xtask 7 / 2; helpers 5 / 3; Cargo 1 / 1; other 70 / 27 |
| `use abi`, `extern crate abi` | `(use\|extern crate) abi\b` | 85 | 66 | Rust 81 / 63; prose 3 / 2; helpers 1 / 1 |
| a dependency on `abi` | `^abi *= *\{\|package *= *"abi"\|^name *= *"abi"` | 26 | 26 | Cargo manifests only |
| `crates/abi` | `crates/abi(/\|\b)` | 370 | 128 | prose 155 / 101; helpers 77 / 8; other 122 / 5; Cargo 6 / 6; xtask 5 / 3; Rust 2 / 2; CI 1 / 1; overlay 1 / 1; other toml 1 / 1 |
| `target_os = "nife"` | `target_os *==? *"nife"` | 48 | 24 | vendor 36 / 15; prose 6 / 5; xtask 4 / 2; helpers 2 / 2 |
| `"os": "nife"` | `"os": *"nife"` | 10 | 10 | target specifications 3 / 3; prose 6 / 6; helpers 1 / 1 |
| the triples | `unknown-nife` | 160 | 82 | prose 70 / 43; xtask 37 / 10; helpers 19 / 7; other 14 / 6; Rust 9 / 7; Cargo 8 / 7; other toml 3 / 2 |
| the `std` port | `std-nife` | 120 | 90 | prose 87 / 70; other 15 / 4; xtask 5 / 4; CI 3 / 2; Cargo 3 / 3; Rust 3 / 3; helpers 3 / 3; other toml 1 / 1 |
| the C macro | `__nife__` | 193 | 14 | the generated relibc headers (`unistd.h` 110, `signal.h` 22, `open-flags.h` 19, `mman.h` 13, `time.h` 10, `fcntl.h` 7), the cbindgen files, `c_library/README.md` and `helpers/c-library-cflags.sh` |
| a build script reading the OS | `CARGO_CFG_TARGET_OS` | 0 | 0 | none |
| `std::env::consts::OS` | `consts::OS` | 5 | 5 | `std_exerciser`, the `system_tests` check of its output, xtask, `notes/std.md` |
| kept: the toolchain link | `nife-dev` | 135 | 70 | prose 94 / 50; xtask 18 / 4; helpers 14 / 8 |
| kept: the farm directory | `nife-farm` | 31 | 23 | prose 10 / 9; helpers 9 / 6; other 7 / 4; CI 3 / 2; xtask 2 / 2 |

The union of the spellings that move covers 731 files.

What the kinds hold:

- "other" under `abi::` is bench patches, the falsification patches, a dependency census TSV and
  the mutation CSVs. Under `crates/abi` it is `design/module-block-baseline.txt` (104 lines), the
  naming-rule baseline (1) and two mutation CSVs (7 each).
- 27 falsification patches, under `system_tests/falsifications/` and
  `crates/capability_plan/falsifications/`, name the old spellings in their context lines. A patch
  whose context no longer matches does not apply, so the step that renames its context regenerates
  it.
- The 15 packages that depend on `abi`: `components`, `capability_plan`,
  `confined_system_call_fuzzer_contract`, `memory_region_heap`, `pgrep`, `pmap`, `process_builder`,
  `ps`, `system_initializer`, `top`, `user_mode_runtime`, `fixtures`, `jig`, `kernel` and
  `system_tests`. Ten `Cargo.lock` files name the package.
- The overlay has 30 files named `nife`, and 96 lines in 25 files with the word. Twelve of its
  module paths end in `nife` or `nife.rs`, one each under `alloc`, `args`, `env`, `fs` (both),
  `pal`, `paths`, `process`, `random`, `stdio`, `thread` and `time`.
- The overlay reaches the crate as `abi::`, a module of `std` that the farm generates verbatim from
  the crate as `sys/pal/nife/abi.rs`. It moves with the crate in step 3 and with the overlay's
  directory in step 4.
- `xtask/src/farm` spells `nife` most in `patch.rs` (49 lines), `aborts.rs` (30), `tests.rs` (22)
  and `copied.rs` (16). `generate.rs` names `TALC_IN_STD = "crate::sys::alloc::nife::talc::"`.
- `vendor/relibc` is a seed nife owns, not a pin. Its edits carry `nife:` markers, and its platform
  module is `c_library/src/platform/nife.rs`, included by path.
- Two gates key on the crate's path: rule 1 of `helpers/architect-label-rules.py` tests
  `crates/abi/`, and `helpers/format_values_check.py` sets `ABI_CRATE = 'crates/abi/'`. Both pair
  renames through git, so they keep working once the constant changes with the path.

## The prose, by status

Markdown lines naming any spelling that moves, by where they live. The rename checklist moves the
live ones and keeps the closed ones, except that a path changes everywhere.

| where | lines | files | moves |
|---|---|---|---|
| `notes/` | 263 | 138 | yes |
| roadmap blocks, BUILT | 145 | 72 | paths only |
| roadmap blocks, PARTIAL | 44 | 27 | yes |
| roadmap blocks, NOT-STARTED | 39 | 24 | yes |
| roadmap proposals | 9 | 3 | yes |
| roadmap blocks, REFUSED | 1 | 1 | paths only |
| `design/decisions/` | 81 | 48 | paths only |
| audit reports | 49 | 15 | paths only |
| crate READMEs | 1 | 1 | yes |
| elsewhere | 7 | 4 | by status |

## basalt

| spelling | lines | where |
|---|---|---|
| `abi::`, `crates/abi`, `target_os`, `std-nife`, `nife-dev`, `__nife__` | 0 | nowhere |
| `unknown-nife` | 7 in 3 files | `.github/workflows/packages.yml` (2), `helpers/packages.py` (the `TRIPLES` list and the architecture map), and the command in the falsification note of `patches/ripgrep-ignore-walk.patch` |
| `nife-farm` | 1 | `helpers/packages.py`, the `RUSTUP_TOOLCHAIN` it sets |

basalt pins nife at one commit in `pins.toml`, and nife pins basalt in `xtask/basalt-pin`. Asset
names carry the package, version and architecture, so no published name carries a triple.

## Names checked outside the tree

On 2026-11-21:

- crates.io: `moho_abi`, `moho-abi` and `moho_sys` free. `moho` belongs to an unrelated game library
  (0.0.9, created 2017).
- rustc: `rustc --print target-list` on the pinned nightly-2026-11-20 lists no target with `moho`
  or `nife`. stable rustc's check-cfg expects 50 `target_os` values. `moho` is not one, and
  `#[cfg(target_os = "moho")]` draws `unexpected_cfgs`.
- LLVM: `OSType` in `llvm/include/llvm/TargetParser/Triple.h` on main has no `Moho` or `Nife`. The
  target specifications name LLVM triples without an OS (`aarch64-unknown-none`, `riscv64`,
  `x86_64-unknown-none-elf`), so LLVM never sees the name.
- Nobody searched the Linux sources.
