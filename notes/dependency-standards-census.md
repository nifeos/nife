# nife's dependencies, held to nife's own standards

*Name: provisional, minted 2026-10-19 (UTC) by maintainer/dependency-standards-census.*

Measured 2026-10-18 and 2026-10-19 (UTC) at nife `6a71434e4` (the branch point of this note's pull
request). calef asked on 2026-10-18 whether nife should review its dependencies against its
internal standards and then derive its dependency standards from what it finds. This note is the
review. The questions it raises for an architect, one per standard, are in
[dependency-standards-census/proposed-decision.md](dependency-standards-census/proposed-decision.md).

The short answer: the standards that describe a crate's risk transfer to dependencies, and the ones
that describe its owner's engineering process do not. No shipped dependency meets the confidence bar
pull request #2379 proposes. One runs Kani, none runs mutation testing, and none has a BUGS section.
Most of them pass the risk checks, and the ones that fail, fail for reasons nife would want to know.

## What was measured

### Which dependencies ship

A crate ships when a build that produces part of a nife image links it. `tools/classify.py` replays
each such build's own cargo flags through `cargo tree -e normal`, so feature resolution matches what
the build compiles:

| build | flags copied from | third-party crates |
|---|---|---|
| `kernel` | `xtask/src/archive.rs`, the three `*-none` targets | 3 |
| `components` (every program in the archive) | `build_programs` | 23 |
| `uefi_loader` | `xtask/src/uefi.rs`, `--features uefi` | 0 |
| `redoxfs_server` | `xtask/src/disk.rs`, `--no-default-features --features el0` | 23 |
| `jig` (with `pinned_tls_client` and `cryptography_provider`) | `xtask/src/farm/exerciser.rs`, the TLS graph's config | 68 |
| `c_library` | `helpers/build-c-program.sh` | 1 |

That is 99 crate versions (89 crates) in [dependencies.tsv](dependency-standards-census/dependencies.tsv).
Two vendored trees ship as well: `vendor/redoxfs` (RedoxFS 0.9.1 plus a recorded patch) and
`vendor/relibc`. Test images add 6 more (rayon and crossbeam, through `std_exerciser`). Build
scripts and proc macros add 4 (`autocfg`, `rustc_version`, `semver`, `version_check`). Host tooling,
fuzz harnesses and dev-dependencies account for the rest: 97 versions of 90 crates, which this note
counts and does not measure.

The TLS graph is the bulk of it. `jig` links 68 of the 99, nearly all of them RustCrypto and
rustls. The kernel links three: `aarch64-cpu`, `spin` 0.12.3 and `tock-registers`. The Rust
standard library itself ships too, through `-Zbuild-std`, and is outside this census.

The candidates under milestone 784 (reuse what the tree wrote) were measured the same way, each with
its dependency closure. Item (e) is `r-efi` 7.1.0. Item (b), held for 705, is `vte` 0.15.0 with
`arrayvec`. Item (f) is `plist` 1.10.1 with 13 crates, and item (f2) is `pulldown-cmark` 0.13.4 with
`bitflags` and `unicase`. The terminal lane has named no others.

### How each standard was measured

Every number below is measured, at the version nife locks. Where nife has a helper for a standard,
the helper ran unchanged on the crate's published source:

- Unsafe density, `unsafe impl Send`/`Sync` and `static mut`: `helpers/rust_source.py`'s
  `unsafe_census`, over the package minus `tests/`, `benches/`, `examples/`, `fuzz/` and `build.rs`.
- Function length: `helpers/function_length.py`'s `measure`, over every `.rs` file, as §283 (a function is held to 100 code lines by a lint ratchet) scopes
  nife's own.
- Comment blocks: `helpers/comment_block_ratchet.py`'s `blocks_over`, the ceiling of §267 (a
  comment states the constraint as it is now). File length: physical lines, as §266 (a Rust source
  file stays under 2,000 lines, held by a ratchet that only falls) counts them.
- `# Safety` on every `unsafe fn`: `script/lint`'s check, copied into `tools/measure.py`.
- SAFETY comments: a text check, because clippy's `undocumented_unsafe_blocks` does not fire on a
  dependency. On nife's own kernel, crates and components, where clippy holds the rule at zero
  misses, it finds 97 to 99 percent. Read its numbers as a floor with a miss rate of about 2 percent.
- Tests: each package's own suite, unchanged, from its published `.crate`, on stable 1.94.1 on
  aarch64 macOS, in a scratch workspace (`tools/runtests.sh`). Toolchain pins in a package were
  removed so no old compiler was installed.
- Miri: `cargo miri test` on nightly-2026-10-18 for 17 crates chosen for their unsafe code
  (`tools/runmiri.sh`). Mutation: cargo-mutants on one crate (`tools/runmutants.sh`, results below).
- Upstream practice: each repository's default branch, read through the GitHub API (CI files,
  `fuzz/`, OSS-Fuzz projects, contributors, archive state).
- Registry and history: the crates.io API, the RustSec advisory database at `999ec89`, and the
  lockfiles of rust-lang/rust, Chromium, Android's `android-crates-io` and Fuchsia's `rust_crates`.
  Also the cargo-vet audit sets of Google, Mozilla, Bytecode Alliance, ISRG and Zcash.

The joined data is [census.tsv](dependency-standards-census/census.tsv), one row per crate version.
[pass-fail.tsv](dependency-standards-census/pass-fail.tsv) applies each standard below to each row.
The raw tables beside them are `registry.tsv`, `upstream-practices.tsv`, `package-tests.tsv` and
`miri-sample.tsv`.

## Results

Rows are crate versions. nife's own value is from its gates at the same commit.

| standard | nife's own code | shipped (99) | vendored (2) | candidates (20) |
|---|---|---|---|---|
| unsafe blocks per 10,000 code lines at or under nife's ceiling of 60 | 57 | 65 | 0 | 15 |
| no unsafe at all | no | 44 | 0 | 7 |
| a SAFETY comment on every unsafe block | all | 52 | 0 | 10 |
| a `# Safety` section on every `unsafe fn` | all | 72 | 0 | 14 |
| no file over 2,000 lines (§266) | 0 over | 84 | 0 | 11 |
| no function over 100 code lines (§283) | 0 over | 77 | 0 | 10 |
| no comment block over 40 lines (§267) | 0 over | 33 | 1 | 5 |
| the package's own tests build and pass | yes | 76 | not run | 15 |
| upstream CI runs the tests | yes | 93 | n/a | 18 |
| upstream CI runs Miri | weekly | 31 | n/a | 9 |
| fuzz targets upstream, or an OSS-Fuzz project | 10 targets | 9 | n/a | 5 |
| Kani proofs | 247 harnesses | 1 | 1 | 0 |
| any formal method (Kani, fiat-crypto, Verus) | Kani | 5 | 1 | 0 |
| mutation testing in upstream CI | per pull request | 0 | n/a | 0 |
| semver checks in upstream CI | none | 9 | n/a | 1 |
| a `rust-version` in the package | none | 69 | 0 | 19 |
| a BUGS or known-issues heading in the README | practice | 0 | 0 | 0 |
| no RustSec advisory affects the version | yes | 98 | 2 | 20 |
| no vulnerability advisory, ever | n/a | 79 | 2 | 16 |
| a cargo-vet audit reaches the exact version (five sets combined) | n/a | 23 | 0 | 2 |
| in rustc, Chromium, Android or Fuchsia | n/a | 79 | 0 | 18 |
| two or more crates.io owners | n/a | 83 | n/a | 9 |
| a release in the last 24 months | n/a | 92 | n/a | 20 |
| repository not archived | n/a | 95 | 2 | 20 |

The confidence bar in #2379 asks for Kani with every harness falsified, a mutation baseline, semver
checks, a tested minimum Rust version and a stranger build. Zero shipped crates meet it. zerocopy
comes closest, with Kani and semver checks in CI, and has no mutation testing or falsification
record.

## What the numbers say

### Code-shape standards measure what a crate does

Unsafe density fails 34 crates, and the failures sort by job rather than by care. Block-cipher and
MAC crates use SIMD intrinsics (`aes`, `polyval`, `poly1305`, `chacha20`). Containers and cells
manage uninitialized memory (`heapless`, `smallvec`, `once_cell`, `generic-array`, `hybrid-array`).
Locks and register crates touch atomics and hardware (`spin`, `tock-registers`). Entropy crates
call the platform (`getrandom`). nife's own code sits at 57 only because the ceiling's denominator
leaves `kernel/src/arch/` out. Counted with `arch/` included, nife's kernel directory is at 154.

Function and file length fail on two shapes. Protocol state machines are one: smoltcp's
`Socket::process` is 494 code lines, and its `tcp.rs` is 9,875 lines. rustls has 14 functions over
100. Generated or table-driven code is the other: `typenum` (4 files over 2,000),
`libc`, `p384`'s fiat-crypto field arithmetic, `num-bigint-dig`'s 1,352-line function. The
comment-block rule fails on 66 crates. That is mostly crate-level rustdoc, a front page that nife's
rule was not written for.

Holding dependencies to these limits would remove the TLS stack, the network stack and most of the
cryptography, with no evidence that any of them is worse for it.

### Process standards are nearly absent upstream

Mutation testing appears in no shipped crate's CI. Kani appears in one (zerocopy). Fiat-crypto
generated arithmetic covers `p384` and one `curve25519-dalek` backend. Its README also cites an
outside Verus verification of 280 functions at 4.1.3, the version nife ships. Nine crates fuzz
upstream, and three have their own OSS-Fuzz project (`httparse`, `lz4_flex`, `rustls`). None has a
BUGS section. These are nife's practices, and the ecosystem has not adopted them.

### Running the checks ourselves finds no defect

Tests: 76 of 99 packages build and pass their own suite as published. The 23 that do not are
packaging, not code:

- Fixture files the package leaves out: `rustls`, `rustls-webpki`, `xts-mode`, `ed25519-dalek`'s
  test vectors.
- Dev-dependency or feature mismatches: `bitflags` 1.3.2, `spin` 0.9.9, `digest` 0.11.3.
- Lints that newer compilers deny: `lazy_static`, `rand_core`.
- Doctests that do not compile on a macOS host: `aarch64-cpu`, `generic-array`.
- Examples that need a feature the default build lacks: `smoltcp`.
- Packages whose suite runs zero tests: 11.

Miri: all 17 sampled crates ran with no undefined behavior reported. That covers both `spin`
versions, `heapless`, `smallvec`, `httparse`, `lz4_flex`, `once_cell`, `zeroize`, `generic-array`,
`hybrid-array`, `inout`, `block-buffer`, `tock-registers`, `endian-num`, and the candidates `vte`,
`arrayvec` and `r-efi`. `inout`'s package has no tests, so its clean run proves nothing. Miri
checks only the paths a package's tests reach.

Mutation: one crate, partly. cargo-mutants 27.1.0 reached 342 of `httparse`'s 387 mutants before the
run was stopped. Of the 241 viable ones scored, the package's tests caught 179 (176 failed, 3 timed
out) and missed 62, so 74 percent were killed. nife's own census kills 96.0 percent. The sample is one
crate, the run is incomplete, and the published package carries only part of upstream's tests, so
read it as a single data point. The other four planned crates (`vte`, `base64ct`, `managed`,
`subtle`) were not run.

### The risk checks find things worth knowing

- One shipped version is affected by an advisory: `rsa` 0.9.10 and RUSTSEC-2023-0071. `deny.toml`
  already ignores it with its reason.
- Three repositories are archived, covering four rows. `cfg-if` and `lazy_static` live under the rust-lang
  and rust-lang-nursery organizations, and `spin` 0.9.9's old home is the third. cargo-deny's `unmaintained = "all"` does
  not fire on them because no RustSec advisory says so.
- Seven crates have had no release in 24 months: `byteorder`, `endian-num`, `managed`, `num-traits`,
  `seahash`, `subtle`, `untrusted`. Several of them are finished rather than abandoned.
- 15 crates have one crates.io owner. Four of those are the Rust project's own organization account.
- `managed`, `num-bigint-dig`, `stable_deref_trait` and `untrusted` have no CI that has ever run.
- cargo-vet audits from the five public sets reach the exact version of 23 rows, and 17 within any
  single set. An import would start nife with a quarter of its graph audited.
- 20 rows appear in none of rustc, Chromium, Android or Fuchsia. They include `smoltcp`, `heapless`,
  `curve25519-dalek`, and two of the three kernel dependencies (`aarch64-cpu`, `tock-registers`).

### The vendored trees and the kernel

`vendor/relibc` holds 1,469 unsafe blocks in 22,561 code lines, a density of 651, with SAFETY
comments on about 7 percent. `vendor/redoxfs` is at 144 with none. Both are upstream's code under the
vendoring rule of §46 (thin primitives or whole subsystems; we write everything in between), and nife's own unsafe census leaves `vendor/` out.

The kernel's three dependencies run unconfined. Between them they hold 71 unsafe blocks. The text
check finds SAFETY comments on 34 percent of `spin` 0.12.3's and on none of `aarch64-cpu`'s or
`tock-registers`'s.

### The milestone 784 candidates

| candidate | crates | measured |
|---|---|---|
| `r-efi` 7.1.0 | 1 | 7,185 code lines, almost all type definitions. 3 unsafe blocks, none with a SAFETY comment. Tests and Miri pass. One owner; std, rustc, Chromium and Fuchsia carry 5.x and 6.x, not 7.x. No advisory, no audit. Upstream CI runs no Miri or clippy. |
| `vte` 0.15.0 | 2 | 5 unsafe blocks in vte and 39 in arrayvec, with almost no SAFETY comments. 2 functions over 100 lines. Tests and Miri pass. One commit in 12 months. Both crates are in rustc and Fuchsia. |
| `plist` 1.10.1 | 14 | Adds `time`, `quick-xml`, `serde`, `indexmap` and `hashbrown`. 7 of the 14 come from repositories where one person wrote over 80 percent of the commits. plist's own tests do not build from the package. |
| `pulldown-cmark` 0.13.4 | 3 | 1 unsafe block, 9 functions over 100 lines, 1,257 tests passing. Has its own OSS-Fuzz project. In rustc and Fuchsia. |

`plist` serves a host program (`stick_maker`), so the shipped-set standards apply to it only as far as
a conveyed host program is shipped.

## BUGS

- The SAFETY-comment numbers come from a text check. It is calibrated on nife's own code, not on
  crates whose comment conventions differ (`// Safety:` counts; a comment two statements away does
  not).
- Feature resolution follows each image build's default kernel features. A kernel feature build
  (`icount`, `board`) that adds a dependency would be missed. None does at this commit.
- Upstream practice is read from each default branch, which is often a newer line than nife's lock
  (rustls 0.24, curve25519-dalek 5.0). `seahash` was not measured, because gitlab.redox-os.org
  returned HTTP 502 all session. `opaque-debug` no longer exists on any default branch.
- Package tests ran on the build host, not on nife's targets. A suite that passes on macOS says
  nothing about a `no_std` code path only the targets compile.
- Coverage and clippy were not measured for dependencies. Coverage needs a per-crate build under
  llvm-cov, and clippy's lints are capped for dependencies.
- The Rust standard library ships in every std program and is not measured here.
