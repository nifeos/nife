This PR adds an unstable `--target <TRIPLE>` option, so Kani can verify a crate for a target other than the host, and adds `riscv64gc-unknown-linux-gnu` as a target Kani has a machine model for.

Towards #2402. It covers the part of that issue that needs no new machine-model work: 64-bit little-endian targets. The 16-bit and big-endian cases asked for there are left for later (see "Not in this PR").

### Why

CBMC does not care which machine it runs on; the machine model is data Kani writes into the goto program. What stops Kani verifying for another target today is that four places are keyed to `env!("TARGET")`: the target passed to cargo, the sysroot `build-kani` builds, `check_target`'s allowlist, and `new_machine_model`. Our use is a kernel with an `arch/riscv64` tree whose code only compiles for riscv64 (`cfg(target_arch)`, `core::arch::riscv64`), which we want to prove from x86_64 and Apple Silicon machines, including GitHub-hosted runners. CBMC 6.11's `set_arch_spec_riscv64` already exists, so CBMC needs no change.

### What changes

- **`kani-compiler`**: a `riscv64` machine model (RISC-V psABI LP64D: unsigned `char`, signed 32-bit `wchar_t`, 128-bit `long double`, matching CBMC's `set_arch_spec_riscv64`), `riscv64-unknown-linux-gnu` in `check_target`, and the `riscv64gc` target features in `target_config`. Without `d` there, rustc warns on every crate that the LP64D ABI needs it, and says the warning will become an error.
- **`kani-driver`**: `--target <TRIPLE>`, gated on `-Z unstable-options`, for both `kani` and `cargo kani`. It replaces `env!("TARGET")` in the cargo invocation, `cargo metadata --filter-platform`, the coverage paths and the single-file `rustc` call. When it is not given, nothing changes: the host triple is used exactly as before, and single-file runs do not pass `--target` to `rustc` at all. `--concrete-playback` with a non-host `--target` is rejected (the test runs on the host), and so is `verify-std --target` (its `no_core` library is host-only).
- **Libraries for more than one target**: `cargo build-dev --lib-target <TRIPLE>` (repeatable) also builds the verification libraries for that triple into `target/kani/targets/<TRIPLE>/lib/`, which is a complete sysroot of its own. The host keeps `lib/` exactly as it is, so release bundles and `cargo kani setup` are unaffected. The driver looks for a non-host target's libraries there, and if they are missing it says which command builds them. The rustup target does not need to be installed, since `-Z build-std` builds from `rust-src`.
- Docs: a page under Experimental features.

### Testing

- New `tests/script-based-pre/target_riscv64`. It runs `kani` on a single file and `cargo kani` on a crate, both for riscv64. The harnesses sit behind `#[cfg(target_arch = "riscv64")]`, so the test also shows that the host run finds none. They check target facts that differ from an x86_64 host (`c_char` is unsigned, `target_feature = "d"`), and there is one failing overflow check, so CBMC has to find something. The crate's harness stubs a function that uses `rdcycle` inline assembly, which does not compile for any other target. The test also covers the error when `-Z unstable-options` is missing.
- `scripts/kani-regression.sh` now builds with `--lib-target riscv64gc-unknown-linux-gnu` so this test runs in CI. That adds one library build to each of the five `regression` matrix jobs; on an Apple Silicon Mac it took about two minutes, under heavy load from other work. I can move it to a single separate job if you would rather keep it off the main regression path.
- Local runs on aarch64 macOS with CBMC 6.11.0: `cargo test -p kani-driver -p kani_metadata -p build-kani`, `./scripts/kani-fmt.sh --check`, and the compiletest suites `ui`, `cargo-ui`, `cargo-kani`, `script-based-pre`, `coverage`, `cargo-coverage` and `expected`, SUITE_RESULTS. I did not run the full `kani` suite or firecracker locally.
- Out of tree, a patch equivalent to this one against 0.67.0 compiled a whole `no_std` kernel crate for riscv64 and verified its harnesses, stubs included.

### Something I noticed and did not change

`link_goto_binary` compiles `kani_lib.c` with `goto-cc`, which configures itself for the host. After linking, the `__CPROVER_architecture_*` symbols are the host's rather than the ones Kani wrote. With `--target riscv64gc-unknown-linux-gnu` on an Apple Silicon Mac, `architecture_arch` is `"riscv64"` in the `.symtab.out` and `"arm64"` in the linked `.out`, and `char_is_unsigned` goes from 1 to 0. The same thing happens without `--target`: on macOS the aarch64 model's `char_is_unsigned = true` is replaced by goto-cc's 0. For Rust code this looks harmless, because the goto program spells out its own widths and signedness, and for the targets `check_target` accepts, pointer width and endianness match every supported host. It is the #2086 problem (`goto-cc` needing `-m32`) in general form, though. `goto-cc`'s `-march` table has no riscv64 entry, so it cannot be fixed from Kani's side alone. I'm happy to open an issue for it.

### Not in this PR

- 32-bit targets (`goto-cc -m32`, plus the 64-bit assumptions @celinval mentioned in #2086), 16-bit `usize`, and big-endian targets, all of which #2402 asks for.
- Non-host libraries in release bundles and in `cargo kani setup`.
- A dedicated `-Z` feature name. This uses `-Z unstable-options`, like other experimental options. If you would prefer a named feature, or an RFC first, I'll do that.

### Disclosure

This change was written by an AI coding agent (Claude Code) working under my direction, for [nife](https://github.com/nifeos/nife), a capability microkernel that uses Kani for its kernel proofs. I'm responsible for it and will follow up on review.

By submitting this pull request, I confirm that my contribution is made under the terms of the Apache 2.0 and MIT licenses.
