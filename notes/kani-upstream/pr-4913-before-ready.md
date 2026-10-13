# model-checking/kani#4913: what stands between the draft and "ready for review"

*Name: provisional. The parent note is [kani-upstream.md](../kani-upstream.md). Nothing here is
posted. calef reads and posts every upstream word; nothing here was pushed to calef/kani or written
on GitHub outside nifeos/nife.*

## The premise was stale

The lane was briefed to draft the upstream half of milestone 589 (Kani can prove riscv64 from the
hosts we already have) as a `-Z` flag against
[#2402](https://github.com/model-checking/kani/issues/2402). That draft already exists and is open:
[#4913](https://github.com/model-checking/kani/pull/4913), from calef/kani `riscv64-target`, built
by `lane/kani-upstream` and recorded in [kani-upstream.md](../kani-upstream.md). Milestone 589's
block still said "a separate lane holds it", which is how the brief came to ask for it again. So
this lane did not write a second patch. It read where #4913 stands, tested its head, and drafted
what is left before it can leave draft.

Read through `gh` on 2026-10-03 (UTC):

| | state |
|---|---|
| #4913 | open, **draft**, `MERGEABLE`, head `2d0a623`, 20 files, +411 −36, base Kani `main` at `1640445da` (current) |
| review | tautschnig, 2026-09-30: five inline comments. calef answered all five on 2026-10-03, and each is in code at `2d0a623` |
| upstream CI | never ran. Six workflows (Kani CI, Format Check, Cargo Deny, Benchmarks, Release Bundle, Std Verification) are `action_required` at `2d0a623`: a first-time contributor's runs wait for a maintainer to approve them. Only `Auto Label` and `Kani Extra` (`pull_request_target`) ran. So the regression matrix that runs `target_riscv64` has not run anywhere but patagonia |
| #2402 | open, last activity 2024-10-02, two comments. The comment drafted in [issue-2402-comment.md](issue-2402-comment.md) is not posted; #4913's "Towards #2402" puts a cross-reference in its timeline either way |
| follow-up | the `kani_lib.c` link-order pull request promised on #4913: drafted in [kani-lib-link-order.md](kani-lib-link-order.md), not opened |

## Tested here, at `2d0a623`

On patagonia, in a scratch clone of #4913's head (push URL disabled), with CBMC 6.11.0 (Kani's pin)
unpacked from the cached Homebrew bottle into the session scratchpad. Nothing installed.

| run | result |
|---|---|
| `cargo build-dev --lib-target riscv64gc-unknown-linux-gnu -- -j 4`, niced, load about 10 | exit 0, 328 s wall, 1.74 GB peak resident |
| compiletest `script-based-pre`, `target_riscv64` alone | passed, 5.3 s |
| compiletest `script-based-pre`, whole suite | 91 passed, 1 ignored, 498 s. `verify_std_cmd` passed |
| compiletest `ui` | 150 passed, 3 failed: all three need the `kissat` solver, which was missing. With kissat 4.0.4 unpacked the same way, the three passed, so 153 |
| compiletest `cargo-ui` | 30 passed |
| with the patch below applied: `cargo build-dev --lib-target riscv64gc-unknown-linux-gnu` | exit 0 |
| with the patch: `target_riscv64` | passed, 5.7 s, after deleting its compiletest stamp (the stamp does not see a change to `riscv64.rs`, so the first rerun was silently skipped as up to date) |
| with the patch: `cargo build-dev --skip-libs --lib-target riscv64gc-unknown-linux-gnu` | refused by clap: "the argument '--skip-libs' cannot be used with '--lib-target <TRIPLE>'" |
| with the patch: `kani-fmt.sh --check`, `cargo clippy -p build-kani -- -D warnings`, `cargo test -p build-kani` | clean, clean, 0 tests |

Not run: `cargo-kani`, `expected`, `coverage`, `cargo-coverage`, the `kani` suite and firecracker.

## Still open at the head, from the September rehearsal review

[pr-4913-review.md](pr-4913-review.md) listed findings before a maintainer saw the pull request.
Should-fix 1 to 3 are done. Of the rest, checked against `2d0a623`:

- Fixed in the drafted patch below: nice-to-have 5 (the `usize` assertion holds on every host, so
  it shows nothing) and 7 (`--skip-libs --lib-target T` silently ignored T; it is now a clap
  conflict, rejected at parse time).
- Found by this lane, also in the patch. The docs page says "every target Kani accepts has the
  same C type widths as every supported host". That is false on the host the pull request was
  tested on: macOS arm64 has a 64-bit `long double` and riscv64 a 128-bit one, measured by
  milestone 635 (riscv64 proofs check against the riscv64 model). The patch names the difference
  and says where it matters.
- Left as a question for review, not changed: nice-to-have 6 (`verify-std` refuses even the
  host triple, playback allows it), question 9 (`--target` with coverage is accepted and untested)
  and question 10 (a crate with a proc-macro dependency is untested). Each is a maintainer's
  preference, and none is wrong today.

## The drafted patch

[pr-4913-followups.patch](pr-4913-followups.patch): one commit in `git format-patch` form, for
calef to apply to calef/kani `riscv64-target` with `git am` if he agrees. 3 files, 5 lines added and
5 removed:

- `tools/build-kani/src/parser.rs`: `--lib-target` `conflicts_with = "skip_libs"`.
- `tests/script-based-pre/target_riscv64/riscv64.rs`: drop the `usize` assertion. The expected
  transcript does not change, since it counts harnesses, not checks.
- `docs/src/reference/experimental/target.md`: the `long double` correction above.

## Edits to the posted body

The body on #4913 is the one in [pull-request.md](pull-request.md) with the September fixes. Four
places are now stale or wrong. Replacement text, ready to paste:

1. **Testing, first bullet.** Delete "and (as of the follow-up commit) the exit-code discipline for
   expected-failure blocks". No such commit exists: `target_riscv64.sh` runs under `set +e`, ends
   in `exit 0`, and passes on its transcript alone. The sentence reads as a claim about exit codes
   the test does not check.
2. **Testing, local runs bullet.** Replace from "In `script-based-pre`" to the end of that sentence
   with: "On 2026-10-03, rebased on `main` at `1640445da`, `script-based-pre` passed (91,
   1 ignored), `ui` passed (153) and `cargo-ui` passed (30). The `verify_std_cmd` failure I mentioned earlier did
   not recur. I have not run the `coverage` suite."
3. **Testing, CI bullet.** Append: "None of the regression workflows have run on this PR yet; they
   are waiting for approval to run for a first-time contributor."
4. **Disclosure.** `https://github.com/crickertech/nife` becomes `https://github.com/nifeos/nife`.
   The `crickertech` organization is now `nifeos`, and the old link redirects for now.

The heading "Something I noticed and did not change" now holds the corrected cause and the
follow-up offer, which is current. It can stay.

## calef's checklist before posting

Every claim below rests on memory, inference, or a measurement this lane did not repeat.

- [ ] Approving the CI runs is a maintainer's action. Asking for it is the one thing that moves
      #4913 today. "Waiting for approval to run for a first-time contributor" is inferred from the
      `action_required` conclusion and GitHub's general behavior, recalled, not read in Kani's
      settings.
- [ ] Mark ready, or ask first? Leaving draft is what invites a full review. The patch above and
      the body edits are what this lane would want done before that, but the order is yours.
- [ ] Body, "about two minutes" for the riscv64 library build per CI job: measured on patagonia
      under load, never on a runner. This lane's cold build of everything, host and riscv64
      libraries together, took 328 s at load 10; the riscv64 share was not separated.
- [ ] Body, "every target `check_target` accepts is a host triple on some CI runner": true only of
      x86_64 and aarch64 Linux and macOS. riscv64 is accepted and is no runner's host; its libraries
      exist in CI only because `kani-regression.sh` builds them. The sentence's conclusion (no
      transcript-stable test for the missing-library error) still holds, since a riscv64 build is
      present in every regression job.
- [ ] Body, "Kani's runners are x86_64 and arm64" (in [kani-upstream.md](../kani-upstream.md), the
      reason for one pull request): read in `kani.yml` in September. Not re-read.
- [ ] Body, "Out of tree, a patch equivalent to this one against 0.67.0 compiled a whole `no_std`
      kernel crate": true of nife's carried patch, which uses an environment variable, not
      `--target`. "Equivalent" is a fair word for the model and the threading, not for the interface.
- [ ] Docs and body, the RISC-V psABI facts (`char` unsigned, `wchar_t` signed, binary128 `long
      double`): checked against psABI master on 2026-09-30, which moves. CBMC's side is pinned at
      `cbmc-6.11.0`.
- [ ] Apply the patch or not. If not, findings 5 and 7 and the `long double` sentence go to review
      as they are; none blocks correctness.
- [ ] Post the #2402 comment, or let the cross-reference stand. The comment describes #4913 as it is
      now, except that it says "64-bit little-endian targets" and Kani accepts only riscv64gc Linux
      beyond the old hosts.

## What is left after posting

- Review latency, unknown. Upstream's next move is a maintainer approving the runs.
- The `kani_lib.c` follow-up, which waits on calef's reading of its draft.
- nife drops `patches/kani-0.67.0-riscv64-target.patch` when its Kani pin reaches a release
  containing #4913 (and the link-order fix, which milestone 635 carries as the patch's second
  commit), per §218 (carry a Kani patch so riscv64 is proved).
