---
status: BUILT
raised: 2026-10-04
built: 2026-10-04
milestone_dependencies: none
decision_dependencies: 247
machine_requirements: none
specific_machine: none
needs_person: no
---
# 755. basalt v0 pins nife and runs its gate

*(Title and slug are drafts.)* From calef's ruling of 2026-10-04 (UTC), recorded in §247
(the split begins with basalt holding nife, and `procps` moves first). basalt v0 is the first step
of the repository split. The ruling includes the go-ahead for a lane's first write to
`nifeos/basalt`. The reasoning and the measurements are in
[notes/the-first-split.md](../../notes/the-first-split.md), section 3.

## What was built

Built 2026-10-04 (UTC) in `nifeos/basalt` pull request #1, merged as `f5bb505a5`. Nothing in this repository changed. Every name below is
provisional and calef's to name.

- `pins.toml`, the manifest: one TOML table per component, `[nife]` with `repository` (a
  `https://github.com/` URL) and `commit` (a full 40-character id; a branch or tag is refused).
  `helpers/pins.py` is its one reader and writer (`get <component>`, `set <component> <commit>`).
- The workflow `gate` (`.github/workflows/gate.yml`, job `nife`) checks out the pinned commit and
  runs that commit's own `script/ci-build test`, the row nife's `ci.yml` job `test` runs. That is the
  host crates, then each architecture's kernel and programs built and the system test suite booted
  under QEMU on aarch64, riscv64 and x86_64. QEMU comes from the pinned commit's `script/ci-qemu`, the
  toolchain from its `script/bootstrap`. One job on `ubuntu-24.04-arm`, 75-minute ceiling,
  a branch's older runs cancelled by its newer one. It keeps each architecture's system-test kernel and the three
  program archives (`initrd*.img`) as a 14-day artifact.
- The workflow `pin bump` (`.github/workflows/pin-bump.yml`, job `propose`): daily at 06:17 UTC,
  it reads nife's `main` head, rewrites the pin on the force-pushed branch `pin/nife` and opens or
  updates one pull request. On a pull request touching it or the helper it runs as a dry run.

**One job, not a matrix per architecture.** `script/ci-build test` runs the three legs in one
process; a matrix would run the provisioning (QEMU, toolchain, cache) and the host pass three times
to buy a status per leg. The same answer at equal effort: basalt then depends on one command name in
nife, and the legs are whatever the pinned commit says they are.

**A pull request, not a push, for the bump.** A pin change is then one reviewable, revertible commit
with the gate run on it. A nife commit that fails the gate is a red pull request, not a red `main`. nife is public, so no token reads it.

## What proves it

| run | pin | result | job wall time | of which | billable |
|---|---|---|---|---|---|
| [37224826206](https://github.com/nifeos/basalt/actions/runs/37224826206) | `e2ee49717` (nife `main` at the pin) | green, all three architectures | 15 min 25 s | QEMU built cold 4 min 18 s, bootstrap 1 min 21 s, `ci-build test` 9 min 14 s | 0 |
| [37225450461](https://github.com/nifeos/basalt/actions/runs/37225450461) | `e7c4f0219` (known bad) | red, as predicted | 6 min 32 s | QEMU built cold 4 min 12 s, bootstrap 1 min 23 s, `ci-build test` 36 s | 0 |

- **The red pin is deterministic.** `e7c4f0219` (624's block header carries its own number and
  branch) is on `main` through a merge commit. Its host test
  `xtask scanout::tests::the_scanout_check_rejects_text_that_is_one_letter_wrong` fails an assertion
  on a fixed input (the same test, in `xtask/src/scanout.rs`): in all six jobs of nife run
  36808261077, three of three times on a Mac, and in basalt's run above. `12890ddd9` fixed it.
- **The first red pin was withdrawn.** `7f9bcb511` failed nife run 37212608311 on the frame ledger
  (26,670 frames kept against 26,668), but that count read 26,667 to 26,670 on the same base, so it
  need not fail every run. Its basalt run, 37224836786, was cancelled. The maintainer caught it.
- The pin bump's App path works end to end. calef installed `nife-smelter` on basalt and set
  `AUTOMATION_APP_ID` and `AUTOMATION_APP_KEY` on 2026-10-04 (19:05 and 19:10 UTC). Run
  37227310530, dispatched from the lane's branch, minted the token, pushed `pin/nife` and opened
  basalt pull request #3 as `nife-smelter[bot]`. The gate started on it by itself (run
  37227325610, event `pull_request`).
- Corrected: the first green run kept the program archives but no kernel. The artifact globs
  named `debug/deps/`, and this nightly's cargo puts a test binary under
  `debug/build/<package>/<hash>/out/`. The gate now finds it by name, and run 37227454682 kept all
  six files: three kernels and three program archives.
- Warm, that same run took 10 min 2 s: QEMU came from the cache, and `ci-build test` took 9 min 32 s.
- Cost. Runner minutes 16 and 7, billed at nothing: GitHub-hosted runners, the arm64 ones
  included, are free on a public repository. A warm run is about 10 minutes, close to nife's
  own `test` job in merge-group run 37223076104 (11 min 34 s). The Rust cache missed on both runs
  and `ci-build test` still took 9 min 14 s against nife's warm 9 min 28 s; the cache is not what
  makes this check slow.
- No citation crosses. basalt's files name no milestone and no section; the reservation's
  README line that cited milestone 120 (the rename: the OS becomes `nife`) was removed, so §201
  (one roadmap until a citation has to cross)'s revisit is not triggered.

## BUGS

- The system gate runs a second time per pin bump. That is the price of the gate living in basalt
  before a split needs it there.
- The gate is one job, so a failing architecture hides the legs after it.
- Until a gate run on basalt's `main` saves the QEMU cache, every pull request builds QEMU from
  source (about 4 minutes). A cache saved by a pull request is visible to that pull request only,
  and a red run saves none.
- A red that fails before the images are built (a host test, a compile error) keeps no artifact.
- basalt's files carry their own BUGS in its README, including the scheduled-workflow suspension
  after 60 days without repository activity.

## Follow-on

- **Milestone 777.** Milestone 777 (basalt's gate blocks a merge, and a green pin bump merges itself). `design/roadmap/0777-basalts-gate-blocks-a-merge.md`: a ruleset requiring the
  gate, and a green pin bump that merges itself. Today the gate reports and blocks nothing.

## Index row

The first step of the repository split, ruled by calef 2026-10-04. basalt, the distribution
repository, pins nife at a commit in `pins.toml` and runs that commit's own `script/ci-build test`
on all three architectures. It is green on a current pin and red on a known-bad one, and a daily job
proposes the next pin as a pull request.
