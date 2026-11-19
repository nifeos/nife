# Review package: `byte_glob`

*Name: provisional, minted by lane 1092-byte-glob-package. The `byte_glob` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to clear the crate for its move to its own repository, step 5 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` plus this branch leaves it. The milestone that produced it is
[1092 (what an outside maintainer would fix before the crate leaves)](../../design/roadmap/1092-glob-outside-review.md).

## In one paragraph

A glob matcher over bytes for one file name. A pattern and a name go in and a `bool` comes out, as
`fnmatch(3)` in the C locale answers it. The function is total on any two byte strings: it has no
error case, no allocation, no `unsafe` (the crate says `forbid(unsafe_code)`) and no dependency.
Its cost has a bound a caller computes from the two lengths before matching. Six Kani harnesses
prove its corners over every pattern and name of up to three bytes. A leading `.` in the name needs
a literal `.` in the pattern by default, because the shell's `rm *` hands a child authority over
every name it matched, and the reading that grants less is the default. The crate points back into
nife in comments and a manifest comment only, and the move rewrites those. The shell, `pgrep` and
the grant planner call it.

## Identity

| item | value |
|---|---|
| crate | `byte_glob`, ratified 2026-11-14 (UTC): the standard term glob, with the bytes it matches said in the name. crates.io holds `glob` for another crate, which is why the qualifier exists |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest |
| `publish` | `false`; milestone 1017 keeps moved crates unpublished until their own release milestone |
| dependencies | none, at run time or in development |
| size | `src/lib.rs` 419 lines, `src/tests.rs` 488, `src/verification.rs` 174 (`cfg(kani)`) |

## Public API

The seven names are the architect's to ratify, below. Everything else the crate exports is the
enum's two variants.

| kind | item |
|---|---|
| fn | `matches(pattern: &[u8], name: &[u8]) -> bool` |
| fn | `matches_with(pattern: &[u8], name: &[u8], dot: LeadingDot) -> bool` |
| fn | `has_magic(pattern: &[u8]) -> bool` |
| fn | `literal<'a>(pattern: &[u8], out: &'a mut [u8]) -> Option<&'a [u8]>` |
| fn | `match_steps(pattern: &[u8], name: &[u8], dot: LeadingDot) -> (bool, usize)` |
| const fn | `cost_bound(pattern_bytes: usize, name_bytes: usize) -> usize` |
| enum | `LeadingDot { Special, Ordinary }`, deriving `Clone`, `Copy`, `PartialEq`, `Eq`, `Debug` |

No type from another crate appears in a signature, and no nife type, capability or handoff does.
`script/crate-review-packet byte_glob` found no foreign type and no `unsafe` site.

What a stranger may find surprising:

- `/` is an ordinary byte, so `matches(b"*.txt", b"dir/notes.txt")` is `true`. A caller that
  matches a path splits it first.
- `.` and `..` are ordinary names to the matcher, so `.*` matches both. The layer that lists a
  directory decides whether to offer them. No listing in this tree offers either name.
- A malformed pattern means itself: an unterminated `[` is a literal `[`.
- `**`, zsh qualifiers and POSIX classes are out by decision, and the README says so.
- `match_steps` returns a tuple. Its second value has no named type, and the README's BUGS says
  so. A named return is a two-way door before release.

## Names

The move strips every `Name:` block, so the architect's ratification here is the only record the
names get. The reading of each against `design/naming.md`:

| name | kind | reading | status |
|---|---|---|---|
| `matches` | fn, `bool` | a third-person verb phrase, as the question rule asks | as built |
| `matches_with` | fn, `bool` | the same, with `_with` for the policy argument, as std uses it | as built |
| `has_magic` | fn, `bool` | `has_` before a thing held; "magic" is the word shell manuals and Python's `glob` use for an unquoted wildcard | as built |
| `literal` | fn, `Option` | a noun for what comes back, the pattern's one literal name | as built |
| `match_steps` | fn, `(bool, usize)` | the steps of a match | as built |
| `cost_bound` | const fn | a bound on the cost; the parameters carry the unit `_bytes` | as built |
| `LeadingDot` | enum | says which dot the policy is about; the maintainer ruled the spelling provisionally on 2026-11-14 (UTC), and the rename is built with no `doc(alias)` of the old spelling | renamed |

`LeadingDot::Special` and `LeadingDot::Ordinary` keep their spelling: they are the two answers.
Prior art names the same policy by position: `FNM_PERIOD` in fnmatch, `require_literal_leading_dot`
in rust-lang/glob, `dotglob` in bash and `GLOB_PERIOD` in glibc's `glob(3)`. At a call site,
`matches_with(b"*", b".config", LeadingDot::Ordinary)` reads without the documentation.

`script/names byte_glob` lists the crate name as ratified, the enum with a provisional block that
records the maintainer's ruling, and the six functions with no block. The module names `tests` and
`verification` carry provisional blocks. `helpers/name_release.py --draft byte_glob` agrees.

## What was run

Every result below comes from this branch.

| check | result |
|---|---|
| `cargo test -p byte_glob` | 18 host tests and 2 doctests pass |
| `cargo test -p pgrep -p grant_plan` | pass; the dependents compile and pass against `LeadingDot` |
| `cargo clippy --all-targets -- -D warnings` on `byte_glob`, and `cargo clippy` over `byte_glob`, `pgrep`, `grant_plan` and `swish` | no warning |
| `cargo doc` with `RUSTDOCFLAGS="-D warnings"` | no warning |
| `cargo build --lib` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and `riscv64imac-unknown-none-elf` | all three build |
| MSRV: a copy of the crate outside the tree, `cargo +1.88 test` and the bare-metal build | 18 tests and 2 doctests pass; builds |
| Miri, `cargo +nightly miri test --lib` | 17 pass, 1 ignored (the exhaustive worst-case scan is too slow under Miri); the crate has no `unsafe`, so this is a cross-check |
| `cargo kani` in `crates/byte_glob` | 6 of 6 harnesses verified, 0 failures (638 s of verification time, on a loaded host). The queue's prove shard runs them again on `merge_group` |
| falsification sweep, `script/falsifications --sweep byte_glob` | not run here: it needs a clean tree and a Kani run per patch, and the host was loaded. All six patches apply (checked with `git apply --check`); the `Dot` rule patch was rewrapped after rustfmt changed its line |
| `script/crate-standalone byte_glob` | not finished locally, for the host load named above. The dry run of `script/crate-move` covers the same staging and passes the host tests and bare-metal builds |
| `script/crate-move --dry-run --into <scratch> byte_glob` | stages at template `v8`; the host tests and the three bare-metal builds pass on 1.88; 5 of 6 patches regenerate for the move. Nine findings for the move lane, listed below. The sixth patch failed to apply after the rename and was fixed on this branch; its own commit says why |
| `script/crate-review-packet byte_glob` | public API and dependents as above; six harnesses pair with six patches |
| `script/names byte_glob`, `python3 helpers/name_release.py --draft byte_glob` | as under Names |
| `script/lint` | the full run on this branch; the pre-push hook runs it again and refuses a push on a failure |

Not run, and so not claimed:

- Mutation testing. The recorded results are the baseline run's 21 survivors, all dispositioned (14
  killed, 7 confirmed hangs, none deferred), and the first weekly census at 100% caught. No census
  ran here, since heavy runs belong in the 08:00 to 13:00 UTC window. The template's weekly job
  runs it in the new repository.
- The stranger test, which belongs to the new repository (`docs/stranger-test.md`).
- Fuzzing. No target exists (item P2-4 of milestone 1092, what an outside maintainer would fix before
  the crate leaves). The crate has no `unsafe`, so the opt-in rule allows one later.
- `cargo-semver-checks` (a first release has no baseline), the template's workflows and the
  publish rehearsal. No repository exists yet.

## Proofs

Six Kani harnesses in `src/verification.rs`, each with a falsification patch that turns it red.
They quantify over every byte value of patterns and names up to three bytes, not over length.
Matching is total under both dot rules. The step count never exceeds `cost_bound`. A pattern with
no magic matches exactly itself, and `*` and `?` keep their base meanings. The dot rule touches only
names that start with a dot, and a negated class is the exact complement of its class. The
length-independent claims are host tests: an exhaustive cross-check against a naive matcher, and a
100,000-byte adversarial run held under the bound. The proof domain and its reasons are in
`docs/proofs.md` once the move writes it, and today in `notes/glob/proofs.md`.

## Who in nife depends on it

Three crates, each by a path dependency with `version = "0.1.0"`:

- `crates/swish`, whose word expansion asks `has_magic` before anything else.
- `crates/grant_plan`, whose directory offers match with the default dot rule, so `*` does not
  grant a dotfile. Its integration test `tests/fixture_agreement.rs` holds the fixture check of
  the matcher's cases.
- `crates/pgrep`, which matches state names with `LeadingDot::Ordinary` and records why at the
  call site.

The move's own dry run lists the other files that name the crate: `.github/workflows/verify.yml`,
`crates/compositor/src/tests.rs`, `crates/externalization-list.txt`,
`crates/grant_plan/src/line.rs` and `parsing.rs`, `crates/swish/src/words.rs`,
`helpers/interface_declarations_check.py`, `helpers/interface_stability.py`, `helpers/kani-peaks.tsv`,
`helpers/template_report_baseline.txt`, `helpers/trust_boundary_crates.tsv`, the verify selftests,
`helpers/verify_times.py`, `packages/contracts.package.toml`, `packages/homes.toml` and
`script/verify`.

The crate refers back into nife in two places the move handles: the comments in `src/` that cite
this tree (17 path lines, one section citation, one milestone citation, found by the packet) and
the manifest's `publish = false` comment, which cites the allowlist.

## What still blocks the move

| item | owner |
|---|---|
| Ratify the seven names and clear the review. | architect |
| Replace the citations of this tree in `src/` with sentences, and point the README and `docs/proofs.md` at the crate's own text. The dry run's `standalone` and `citations` findings are these. | move lane |
| Give the manifest a `repository` field and a `publish = false` comment in the crate's own words. | move lane |
| Shorten the README under the template's prose caps and fix its two passives, which the dry run reports. | move lane |
| Rewrite the falsification paths, regenerate the hunks and replay all six patches in the new repository. | move lane |
| Carry the repository's `.cargo/mutants.toml` and add the `byte_glob` row to `.cargo/mutants-baseline.txt` when the weekly job first runs. | move lane |
| Convert `grant_plan`, `pgrep` and `swish` to `git` and `rev` with `version = "0.1.0"`, and rewrite the files above. | move lane |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository. | move lane |

## Open questions

1. Whether `match_steps` returns a named type (`Outcome`). Recommendation: keep the tuple for the
   first tag and record it in the README's BUGS, as built. The maintainer can decide it.

## Recommendation

Ratify the seven names as built, with the `LeadingDot` spelling the maintainer ruled provisionally, and clear `byte_glob` for its repository once the prove shard is green. The crate is small, total, has no dependency and no `unsafe`, and carries the strongest proof set of its size in the tree. The move lane's list above is mechanical. Publishing stays held.

Ready for review.
