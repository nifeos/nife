# Building, testing and gating

*An appendix to [`notes/scripts.md`](../scripts.md), the front door to `script/`. It holds the full
table rows for the commands that build, test, format and gate the tree, the paragraph gathering what
those rows cite, and the counted-claims section. A reader who only needs to run a command should not
have to open it. The directory `notes/scripts/` and this file's stem are provisional names; naming
is calef's.*

| script | what it does |
|---|---|
| `script/bootstrap` | Install every dependency: the pinned Rust toolchain (via rustup, from `rust-toolchain.toml`) and QEMU. Idempotent: it checks first and installs only what is missing. |
| `script/setup` | First run after a clone: `bootstrap`, then build. |
| `script/update` | After pulling new code: `bootstrap` (the pinned toolchain can change), then rebuild. |
| `script/claim <branch-name> (--debt-paydown \| --new-work) [--worktree <dir>] [--title <text>] [--no-claim]` | Cut a lane's branch and worktree, refusing a name that would fail `script/lint`'s check 4 (a near-miss on `milestone/N-slug`) before either exists, instead of after the worktree is built and the work is written. Shares the shape rule with that check through `helpers/branch-name-check.sh` rather than a second copy. `--worktree` overrides the default `<worktree root>/<slug>` (`nife-worktrees` beside the main checkout); `--title` sets the draft pull request's title and the empty commit's message (default: the branch name). Also makes AGENTS.md §90 (the claim is a draft pull request; the status flip is a gate)'s claim (an empty commit, a push, a draft pull request) unless `--no-claim` stops it at the branch and worktree. Refuses to run without exactly one of `--debt-paydown` (the draft gets that label) or `--new-work` (calef's ruling R1 on #1894, 2026-10-10 UTC; [debt-paydown.md](../debt-paydown.md)), and `helpers/claim-selftest.sh` holds it to that. **Name provisional.** |
| `script/test` | Host-logic crates, then the kernel under QEMU on **both** ISAs. The gate. |
| `script/verify` | The machine-checked proofs (Kani) over the pure-logic crates. Not in `bootstrap`: Kani pulls its own toolchain and a CBMC backend, so it is installed only where it is used. |
| `script/bench` | icount microbenchmarks; `--check` fails on >10% drift from `bench/baseline-aarch64.txt`, `--save` rewrites it, `--real` runs under HVF for magnitudes, `--extra-features <name>` (with `--real` only) builds an extra kernel feature alongside `bench` (E3's padded-fastpath latency comparison, milestone 134). |
| `script/icount` | The instruction-count instrument (milestone 78): boots a `--features icount` kernel under `-icount shift=0,sleep=off` on both ISAs and asserts the two timing claims a wall clock cannot make, because a slow handler and a descheduled emulator look identical from inside the guest. `--arch` narrows it to one leg. Not in `test` (see notes/instruction-clock.md: `-icount` gives every vCPU one shared virtual clock, which is an argument about a boot mode rather than about a command); it IS a `local` row in `ci-build`, and CI runs it beside `bench --check`. This cell read "not in `test` or `gates`" for a month after milestone 62 put it in the set a developer runs. |
| `script/qemu-check` | Is the QEMU on PATH the one `.qemu-version` pins, and does it carry the devices the suite needs? **Fails** on a missing device (that would gut a test silently), **warns** on a version mismatch (Homebrew cannot install an arbitrary older QEMU, and an unfollowable rule is worse than none). Called by `bootstrap` and by `ci-qemu`. |
| `script/ci-qemu` | CI only, Linux only: build the pinned QEMU into a cacheable prefix, because Ubuntu 24.04's 8.2 has no `riscv-iommu-pci` and apt cannot go newer. |
| `script/drift [nightly-YYYY-MM-DD]` | Does a toolchain still build us? Bare-metal build plus the host-logic tests. With no argument it checks the pin, which makes it a fast health check; given a nightly it checks that one, which is what the daily `toolchain drift` workflow does with the newest. |
| `script/toolchain-bump [YYYY-MM-DD]` | Raise the pinned nightly, with evidence: install, rebuild the std farm from scratch, run every gate. Restores the old pin if anything fails, because a half-applied toolchain bump is worse than none. Run it when the daily `toolchain drift` workflow goes red. |
| `script/test` | Run the suite: the host-logic crates in milliseconds, then the kernel under QEMU. The fast inner loop; assumes `setup` has run. `--arch aarch64\|riscv64` runs one ISA leg instead of both (the default is still both, so the parity gate cannot be weakened by forgetting it); `--cpu <model>` picks the emulated CPU (notes/cpu-models.md); `--hvf` runs the aarch64 kernel leg on the physical Apple Silicon core instead of under TCG (aarch64 only, `-cpu host` mandatory, and it skips the host-logic crates because no accelerator exists on that path; notes/hvf-leg.md); `--test <substring>` runs only the kernel tests whose path contains it (milestone 210, see *Running one kernel test* in [`notes/scripts.md`](../scripts.md)). |
| `script/ci-build` | **The one enumeration of the checks a pull request must pass** (milestone 286). With no arguments it provisions (`script/bootstrap`, idempotent) and runs the `local` tier in the table's order, cheapest first, so a formatting slip costs twenty seconds rather than ten minutes; that path absorbed `script/gates`, which was a second copy of the same list and had drifted from it in three separate places. `script/ci-build <check>...` runs named checks and nothing else, which is how `ci.yml` fans them into parallel jobs, and is why a check cannot be added to CI without the local set learning about it. `--list` prints the table: `name`, `tier`, and the command. The `ci` tier is what only a runner waits for (the CPU matrix, coverage, fuzzing, the bench tripwire, the supply-chain audit, the two stack instruments, the fastpath footprint), nameable here but never run by the no-argument path, because a gate people skip is not a gate. Kani (`script/verify`) and the falsification sweep are absent on purpose: they are `verify.yml`, a different workflow. The `hvf` check is the aarch64 suite again on the physical Apple Silicon core (milestone 81); it lives here rather than in a workflow because GitHub's hosted macOS arm64 runners are VMs without nested virtualization, and where the host cannot supply it the check **skips loudly**, naming the reason and saying that nothing in the run touched a physical core, so a Linux transcript cannot be read as silicon coverage. About 16 s (measured; notes/hvf-leg.md). Never writes; use `script/fmt` to format. **The tier names and what `no arguments` means are provisional** pending calef (milestone 440, `design/roadmap/0440-what-no-arguments-means.md`). |
| `script/server` | Boot the OS in QEMU (the milestone tour, then the shell). An OS is the thing you *start*, so it is `server`. |
| `script/console` | Boot straight to the interactive shell at EL0. For this project the console is literally a shell running as an unprivileged process. |
| `script/fmt` | Format the tree with the pinned rustfmt; `--check` reports instead of writing (the CI gate). |
| `script/lint` | Run clippy across the workspace with warnings denied (a CI gate), on both ISAs and in each boot-mode feature build, plus the non-clippy checks that share its job: broken intra-doc links, conflict markers, the roadmap status vocabulary, relative markdown links plus the notes/README.md index, DECISIONS numbering, that every `script/` has an entry here, that no file carries a module-wide `#![allow(dead_code)]` (DECISIONS §38), and the naming conventions a machine can check (no `-d` names, none of the rejected Unix vocabulary, one spelling for contract crates, a recognized branch prefix; design/naming.md). Milestone 68 added three more: **dependency direction** (nothing under `crates/` may depend on a binary, which would still build while leaving the host tests and Kani), **unused dependencies** via `cargo-machete` (DECISIONS §46), and **spelling** via `typos`. Milestone 94 added one more: a **`TODO`/`FIXME` marker in code names the milestone that owns it** (`TODO(milestone N):`, and the block has to exist), because a marker with no home is identified work resting where nobody will look for it. Markdown is exempt, since prose explaining the convention has to spell the shape it forbids, and a note may quote a marker that was resolved milestones ago. Milestone 113 added a fourteenth clippy configuration: the **proof harnesses**, compiled with `--cfg kani` against the shim in `helpers/kani-lint-shim/`, because `cfg(kani)` is set by the model checker and by nothing else and so those modules had never been linted at all (26 warnings on the first run; notes/unsafe-obligations.md). Lint SELECTION is not here: it lives in `Cargo.toml`'s `[workspace.lints]`, with `clippy.toml` and `_typos.toml` holding the two allowlists. See DECISIONS §61 for why three candidate lints were measured and dropped. |
| `script/coverage` | Coverage for the host-logic crates, gated on an 80%-per-file line floor (a CI gate). Every run also reports the **minimum** per-file coverage, the distribution across bands, and how many files a floor of 85 or of 90 would newly fail, and writes those to `target/llvm-cov/floor.txt` for `script/metrics --coverage-min-from`: the dashboard used to plot only the aggregate, which cannot move when one file slides under the floor. Installs cargo-llvm-cov on first run. |
| `script/preflight-queue [--dry-run \| --act] [--no-hvf]` | **Replay the merge queue on this machine before a group build does.** Walks `mergeQueue(branch: "main")` in order, then the armed-but-unqueued pull requests, merging each onto the green entries ahead of it in a reusable detached worktree (`preflight` in the worktree root), and runs `script/ci-build fmt` and `lint`, `cargo test -p documentation`, host tests for the crates it touches, one `script/test --arch aarch64` if it touches code (CI's own documentation-only predicate, read out of `ci.yml`), and `script/falsifications --affected-since` when no solver is running. A conflict or a red is a finding. The HVF leg on `main`'s tip runs first and is reported, never blocking; a red `main` baseline means it acts on nothing and exits 3. `--dry-run` is the default; `--act` comments and dequeues. Prompted by three pull requests on 2026-09-24 that were green alone and red on top of the queue; when to run it is in notes/merge-queue.md. **Name provisional.** |

The table's numbers are glossed here rather than inline. The reason is that the `script/lint` row is
one of the longest markdown lines in the repository and `documentation::render::LINE_MAX` is sized
against the longest. The decisions cited are DECISIONS §38 (a suppression carries a reason), §46
(thin primitives or whole subsystems) and §61 (a lint adopted on evidence). The milestones are
milestone 68 (code-quality gates), milestone 94 (the untracked-work sweep) and milestone 113 (the
proofs' unsafe code is ungated).

## Counted claims, one of `script/lint`'s checks

Milestone 125 added a check that does not fit the table above, because what it gates is the prose
rather than the code. A number carrying a `<!--count:NAME-->` marker is re-derived from the tree on
every build, and `script/lint` fails on a mismatch, naming both values and the line. Three registry
entries so far (`kani-harnesses`, `harness-crates`, `sh-scripts`); an unmarked number stays
unchecked, which is the ratchet working as designed. See [counted-claims.md](../counted-claims.md) for
how to add one, and for the honest limits.

This section is prose and not a row in that table on purpose, and the reason is worth knowing before
you edit either. The longest line in the repository's markdown is
1928 bytes <!--count:longest-markdown-line-->, and `manual`'s renderer sizes `LINE_MAX` at 2048
against exactly that measurement. The rows in the table above are the next three longest and sit within about a
hundred bytes of it. (This sentence named `script/lint`'s row as the longest. That stopped being
true without anything noticing, because the marker vouches for the NUMBER and nothing vouches for
which line carries it.) Extending one of those rows by a sentence overflows the buffer, and the way
you find out is a `manual` render test failing while pointing at text three hundred lines further
down the file.

## What this file's numbers cite

Glossed here because the split made this a file of its own, and `script/citations` asks each
file to say once what a number cites. Each gloss is the record's own title.

- milestone 62 (Tests that assert on time: make a red run mean something)
- milestone 78 (The load-sensitive assertions, and the three that measure the wrong thing)
- milestone 81 (An HVF leg: the test suite on the physical core)
- milestone 125 (A number in the prose is a claim, and nothing re-derives it)
- milestone 134 (The register of measures: every number this kernel owes itself)
- milestone 210 (No kernel test can be run by name, so one falsification costs a whole suite)
- milestone 286 (One enumeration of the checks that gate a pull request)
- milestone 440 (What `script/ci-build` with no arguments should mean)
