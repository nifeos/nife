# Upstreaming the riscv64 target to Kani

*Name: provisional (`notes/kani-upstream.md` and the `notes/kani-upstream/` appendix). Written
2026-09-24 (UTC) by the lane `lane/kani-upstream`.*

calef ruled on 2026-09-25 (UTC) that nife carries a patch letting Kani verify riscv64 code, and
that the same change goes upstream against
[model-checking/kani#2402](https://github.com/model-checking/kani/issues/2402). Another lane carries
the patch. This note covers the upstream half: what was built, why it has this shape, and the two
texts calef posts under his own account. The research behind it is the proposal
`design/roadmap/proposals/kani-can-target-riscv64-from-the-hosts-we-have.md` (pull request #1280).

## Where it is

- Pull request: [model-checking/kani#4913](https://github.com/model-checking/kani/pull/4913),
  opened as a draft 2026-09-30 (UTC) from [calef/kani
  `riscv64-target`](https://github.com/calef/kani/tree/riscv64-target).
- On 2026-10-03 (UTC) the branch was rebased onto Kani `main` at `1640445da`, and the first review
  was folded in. "Review, 2026-09-30" below has what changed.
- The pull request body as first posted: [kani-upstream/pull-request-4913-body.md](kani-upstream/pull-request-4913-body.md).
- Comment for #2402: [kani-upstream/issue-2402-comment.md](kani-upstream/issue-2402-comment.md).

## What Kani asks of a contributor

Read at `de756c936`, not recalled:

- No DCO and no CLA. No workflow checks `Signed-off-by` or a CLA. `CONTRIBUTING.md` and the pull
  request template ask for one sentence in the body: "By submitting this pull request, I confirm
  that my contribution is made under the terms of the Apache 2.0 and MIT licenses." That sentence is
  the attestation. It is already the last line of the body, and it is calef's to make.
- **Discuss significant work in an issue first.** #2402 is that issue: open since 2023-04-23, with a
  second use case added by a zerocopy maintainer and no objection from anyone.
- **The RFC process** applies to one-way doors and to changes with a significant design component.
  It also asks that a new feature be reachable only behind `-Z`. An option behind `-Z
  unstable-options` is not a one-way door. The multi-target sysroot layout could be read as design,
  so the body offers to write an RFC if a reviewer asks for one.
- Kani's `AGENTS.md` has guidance for AI assistants, so an agent-written pull request is
  expected there. The body still says it was written by an agent under calef's direction, as nife's
  `**Lane:**` line does here.
- Pull requests are squash-merged, so the branch is one commit.

## The shape, and why

**One pull request: an unstable `--target <TRIPLE>` flag, a sysroot that holds libraries for more
than one target, and the riscv64 machine model.**

- A smaller opening pull request with only the machine-model table was considered. It lost because
  nothing in Kani's CI could exercise it. A riscv64 model is reachable only from a riscv64 host or
  through a target flag, and Kani's runners are x86_64 and arm64. The RFC process asks for "a
  testable end-to-end flow" in every pull request. The flag and the model test each other, so they
  go together.
- The flag alone, tested with x86_64 to aarch64, was the other split. It would work, but it would
  send the riscv64 reason for the change in a second pull request, and that reason is the case that
  makes the first one worth reviewing.
- **Multi-target libraries.** The host's `lib/` is untouched. `cargo build-dev --lib-target T` puts
  T's libraries in `targets/T/lib/`, which is a sysroot of its own, so the driver's `LibConfig`
  needed no change. Release bundles and `cargo kani setup` are unchanged and still host-only. That
  is listed under "Not in this PR", along with 32-bit, 16-bit and big-endian targets.
- The gate is `-Z unstable-options`, as Kani uses for other experimental options, rather than a new
  named `-Z` feature. A feature name would be a naming decision for Kani's maintainers, so the body
  offers one.

Diff: 20 files, 366 lines added and 33 removed. The prototype patch (46 lines over 4 files, environment variable)
was the starting point. It is not what went upstream: the environment variable is gone, and so is
the `KANI_TARGET` read at `build-kani`'s compile time.

## Tests, on patagonia

Kani built with `cargo build-dev --lib-target riscv64gc-unknown-linux-gnu`, two jobs, niced, 20
minutes wall under a load average above 100 from other lanes. CBMC 6.11.0 (Kani's pin) was
extracted from the Homebrew bottle into the session scratchpad. Nothing was installed machine-wide.

| run | result |
|---|---|
| `cargo test -p kani-driver`, `-p kani_metadata`, `-p build-kani` | 104, 2+2, 0 passed; no failures |
| `./scripts/kani-fmt.sh --check` | clean |
| new `script-based-pre/target_riscv64` | passed (7.3 s). A `git clean` then deleted the untracked test before commit; it was rewritten from the lane's transcript, and compiletest's stamp skipped the rerun |
| suite `ui` | 152 passed |
| suite `cargo-ui` | 30 passed |
| suite `cargo-kani` | 71 passed |
| suite `expected` | 478 passed, 16 ignored |
| suite `cargo-coverage` | 2 passed |
| suite `script-based-pre` | 75 passed, 1 failed (`verify_std_cmd`), not yet diagnosed |
| suite `coverage` | failed: the harness could not find `kani-cov`, which the regression script builds first and this run did not. Not a result |

Not run: the `kani`, `firecracker`, `prusti`, `smack` and `kani-fixme` suites.

## Found on the way

The `__CPROVER_architecture_*` symbols Kani writes do not survive the link. Measured with
`goto-instrument --show-symbol-table`: for riscv64, `architecture_arch` is `"riscv64"` in the
`.symtab.out` and `"arm64"` in the linked `.out`, and `char_is_unsigned` goes from 1 to 0. A plain
host run on patagonia shows the same `char_is_unsigned` flip, so this predates the change. For
Rust it looks harmless, because the goto program carries explicit widths, and pointer width and
endianness match for every accepted target.

The cause first recorded here was wrong. It said `goto-cc` configures itself for the host when it
links. tautschnig showed in review that when `goto-cc` links goto binaries, the first input's
architecture symbols win, and `link_goto_binary` passes `kani_lib.c` as C source, which `goto-cc`
compiles with the host's configuration. Re-measured on patagonia 2026-10-03 (UTC) with CBMC
6.11.0: the Rust symbol table linked alone, or followed by a precompiled `kani_lib.c` object, keeps
`"riscv64"`; the object first, or the C source anywhere, gives `"arm64"`. So Kani can fix it by
precompiling `kani_lib.c` and linking it last, with no `goto-cc` change. That fix is offered as a
follow-up to #4913, because it changes the linked model for every host run, not only `--target`.

For nife this bounds what a riscv64 proof claims. Rust-level widths and `cfg` are riscv64's, but
CBMC's C-library models run with the host's C configuration. Nothing in `kernel/` calls C.

## Review, 2026-09-30

tautschnig (a Kani maintainer) left five inline comments on #4913. What became of each, as of
2026-10-03 (UTC):

| comment | outcome |
|---|---|
| remove `shim/kani`, a local wrapper | already gone in `6580581`, pushed after the review was written |
| `check_target`: match the OS and riscv64gc's feature string, since `riscv64a23-unknown-linux-gnu` and `riscv64-wrs-vxworks` share its LLVM target | taken. Checked first: both share `riscv64-unknown-linux-gnu`, a23 has features `+rva23u64` and vxworks has os `vxworks`. After the change a23 is rejected, and the error names the triple as well as the LLVM target |
| restrict the Apple prefix checks to macOS | taken; `aarch64-apple-ios` is now rejected, and the host still builds |
| the docs give the wrong cause for the architecture overwrite | taken; see "Found on the way". The fix itself waits on calef's reply |
| rewrap an over-long comment | taken |

Checks after the change, on patagonia with CBMC 6.11.0: `cargo build-dev --lib-target
riscv64gc-unknown-linux-gnu`, `kani-fmt.sh --check`, clippy with `-D warnings` on the three touched
crates, the `kani-driver`, `build-kani` and `kani_metadata` unit tests, and the
`target_riscv64` test all pass. So do the compiletest suites `ui` (153), `cargo-ui` (30) and
`script-based-pre` (91 passed, 1 ignored), which means `verify_std_cmd` now passes as well.

The patch nife carries, `patches/kani-0.67.0-riscv64-target.patch`, has the same loose
`check_target`. It does not affect nife's proofs, because `script/verify-riscv64` only ever passes
`riscv64gc-unknown-linux-gnu`. So the patch is left as it is, and the gap ends when the patch is
dropped for an upstream release. The reason first given, that editing the patch rebuilds the cached
Kani in CI, lapsed on 2026-10-03 when milestone 635 edited it anyway. The gap is still harmless to
nife, so it stays a limitation.

## Where this lane stopped

The pull request is open and the review is answered in code. Waiting on calef:

1. The `kani_lib.c` link fix, promised on #4913 as a follow-up pull request. Milestone 635
   (riscv64 proofs check against the riscv64 model) carries it in nife's patch
   ([its block](../design/roadmap/0635-riscv64-proofs-check-against-the-riscv64-model.md)) and gates it in `script/verify-riscv64`. The upstream pull request is drafted in
   [kani-upstream/kani-lib-link-order.md](kani-upstream/kani-lib-link-order.md), and nobody posts it
   before calef has read it. In CI the old link gave `arm64`/`linux`, so `arch` was the only field
   Kani wrote that changed there; no verdict changed for the seven harnesses that exist.
2. The pull request body's "Something I noticed" paragraph, if it still gives the old cause.
3. Still open from before: the `coverage` suite was not rerun. `verify_std_cmd`, undiagnosed in
   September, passed on 2026-10-03 (UTC), so that failure was the loaded machine or a baseline
   since fixed.
