# Review of model-checking/kani#4913

*Name: provisional. Written 2026-09-30 (UTC) by the lane `lane/kani-upstream`. The parent note is
[kani-upstream.md](../kani-upstream.md).*

This lane reviewed [kani#4913](https://github.com/model-checking/kani/pull/4913) as a hostile-but-fair
upstream reviewer would, and records the findings here. The pull request is our own: head `6760749`,
base `c1a1cf6`, opened 2026-09-30 01:18 (UTC). The review is therefore a rehearsal for the real one,
and its findings are ours to fix before a maintainer makes them.

Evidence used, all fetched without cloning kani. The pull request diff and body came through `gh`;
the touched driver and build files at the head SHA were fetched raw; so were CBMC's
`src/util/config.cpp` at tag `cbmc-6.11.0`, the RISC-V psABI `riscv-cc.adoc` at master, and
`.github/workflows/kani.yml`.

## Blocking

Nothing found. The two highest-risk areas both check out, verified rather than trusted:

- The machine model matches CBMC and the psABI on every field (details under "What holds up").
- The flag threading is complete: no `env!("TARGET")` remains anywhere in `kani-driver` at the head
  SHA, checked by fetching every file that could hold one, not by memory.

## Should-fix

1. The posted body still contains the literal token `SUITE_RESULTS` (Testing section, sentence
   listing the compiletest suites). It reads as unfinished, and it stands where the evidence should
   be. Our own record (`kani-upstream.md`, "Tests, on patagonia") says `script-based-pre` had one
   local failure (`verify_std_cmd`, undiagnosed) and `coverage` never produced a result. Fill the
   token honestly or drop it, and say which suites were clean.
2. The two rejections and the missing-library error have no test. The pull request adds the
   `--concrete-playback` conflict (`kani-driver/src/args/mod.rs:875-884`), the `verify-std`
   rejection (`kani-driver/src/args/std_args.rs:43-50`), and the bail naming the build command
   (`kani-driver/src/session.rs:316-331`). The new script test covers only the `-Z` gate
   (`tests/script-based-pre/target_riscv64/target_riscv64.sh:13`). Each check is one more
   `[TEST]` block in that script; Kani tests its other argument conflicts, and these are new
   surface.
3. `docs/src/reference/experimental/target.md` omits the goto-cc arch-symbol overwrite from its
   Limitations list (file lines 46-54). The body discloses it with measurements: after linking,
   `__CPROVER_architecture_*` are the host's, and `char_is_unsigned` flips from the written value.
   That bounds what a cross-target proof claims, and a user who reads only the docs page will not
   find it. Add a bullet and link the offered issue.

## Nice-to-have

4. The body says the flag "replaces `env!(\"TARGET\")` in ... the single-file `rustc` call". Nothing
   was replaced there: the old single-file path never read or passed a target, and the diff adds
   `--target` only when the flag is given (`kani-driver/src/call_single_file.rs:67-70` at head).
   The next sentence in the body states the truth, so this is wording, not substance.
5. `tests/script-based-pre/target_riscv64/riscv64.rs:16` asserts `size_of::<usize>() == 8`. Every
   host Kani runs on is 64-bit, so this cannot discriminate. The doc comment above it (lines 10-11)
   says "not for every host", which the first two assertions satisfy and this one does not.
   Harmless as a sanity check; misleading as evidence of cross-compilation.
6. `verify-std` rejects `--target` even when the triple is the host's (`std_args.rs:43`), while the
   `--concrete-playback` gate allows the host triple (`args/mod.rs:877`). Both defensible; worth
   one comment line each, or align them.
7. `cargo build-dev --skip-libs --lib-target T` silently ignores the target
   (`tools/build-kani/src/main.rs:29-33` at head). A warning would save a confusing missing-library
   error later.

## Questions for maintainers

8. CI cost. `scripts/kani-regression.sh:33-35` adds a riscv64 library build, and
   `.github/workflows/kani.yml:22-24` runs that script on five runners, including `macos-15-intel`.
   The body pre-offers moving the build to one job. The maintainer call is whether five duplicated
   builds buy enough matrix coverage; the test does exercise x86_64 and aarch64 hosts either way.
9. Coverage with `--target` is accepted but untested. `cov_session.rs` now threads the
   verification target into both metadata calls (diff lines 389-401), and nothing rejects the
   combination, but no test runs it. Intended, or worth a rejection until tested?
10. Cross-target builds with proc-macro or build-script dependencies are untested. Reading the
    mechanism: `cargo_config_args` sets `host.rustflags` (`call_cargo.rs:505-512`), which keeps host
    crates native, and `pass_rustc_args(..., PassTo::AllCrates)` carries the target sysroot in
    `CARGO_ENCODED_RUSTFLAGS` (`kani-driver/src/util.rs:215-237`). That says proc macros still
    compile for the host, as they do today. The sample crate has no dependencies. A test with one
    proc-macro dependency would pin this.
11. The `-Z unstable-options` gate and the `targets/<TRIPLE>/lib/` layout are already offered as
    negotiable in the body. Nothing to add from this side.

## What holds up

The machine model (`compiler_interface.rs:929-971` at head) was checked field by field against two
primary sources, both fetched for this review. CBMC 6.11.0's `set_arch_spec_riscv64`
(`config.cpp:437-463` plus `set_LP64` at 47-61): LP64 widths, `char_is_unsigned=true`,
`wchar_t` signed and 32 bits, `long_double_width` 16 bytes, little-endian, `NULL_is_zero`. The
psABI master `riscv-cc.adoc`: the LP64 table matches every width, line 908 says "`char` is
unsigned", line 1050 says "`wchar_t` is signed", and `long double` is binary128. Every value in the
new arm agrees with both. The code cites both sources at the arm, which is the part a reviewer
would actually praise.

The threading: `cargo_build` uses the verification target for the cargo invocation, metadata
(`--filter-platform`), and library folder (`call_cargo.rs:143-181` at head). `cargo_build_std` and
playback stay on `HOST_TARGET`, correctly, since both are host activities. Both `cov_session` sites
use it; single-file passes `--target` only when given. With no flag, every path passes the same
string `env!("TARGET")` produced before.

The layout: build and driver agree on `targets/<TRIPLE>/lib/` (`tools/build-kani/src/sysroot.rs:68-70`
and `session.rs:316-331`); the host `lib/` path is unchanged; playback and `no_core` libraries stay
host-built; no packaging or setup file is touched, verified against the pull request's file list.
The missing-library error names `cargo build-dev --lib-target <TRIPLE>`, which is the exact command.

The gating: `-Z unstable-options` fires on the flag alone; both rejections behave as documented.

The test proves real target facts: `c_char::MIN == 0` differs on an x86_64 host, `target_feature =
"d"` differs on every host, and the failing overflow check forces CBMC to actually solve a riscv64
program. The host-run case proving the harnesses are compiled out is the right negative control,
and the expected-file format matches the framework's ordered-subsequence comparison.

## Could not verify from the diff

- The "about two minutes" cost of the added library build. No timing evidence exists in the diff,
  and the Intel macOS runner will differ from the Apple Silicon measurement.
- The 0.67.0 out-of-tree kernel claim. Out of tree, not reproducible from the pull request.
- CI. At review time only the Auto Label check had run (checked 2026-09-30, UTC); the regression
  matrix, which is what actually exercises the new test, had not reported.
- The psABI was checked at master, which moves; CBMC at the `cbmc-6.11.0` tag, which is pinned but
  only for this review's fetch, not by anything in the diff.
- "Release bundles and `cargo kani setup` are unaffected" is verified by absence of changes, not by
  building a bundle.
