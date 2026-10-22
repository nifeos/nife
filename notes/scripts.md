# The `script/` entry points

Every command you need to work on this repo lives in `script/`, one short file each, with the
same names GitHub's [Scripts to Rule Them All](https://github.com/github/scripts-to-rule-them-all)
pattern uses. The idea is muscle memory: clone any repo that follows the pattern, run
`script/setup`, then `script/test`, and you are working, without learning which one uses
`cargo xtask`, `make` or `npm`.

## The commands

One table per group. Each links to an appendix holding the full row: flags, gates and history.

### Build, test and gate

Full rows: [build-test-and-gate.md](scripts/build-test-and-gate.md).

| script | what it does |
|---|---|
| `script/bootstrap` | Install the pinned toolchain and QEMU. Idempotent. `NIFE_SKIP_LINT_TOOLS=1` skips the lint-only tools (cargo-machete, typos). |
| `script/setup` | First run after a clone: `bootstrap`, then build. |
| `script/update` | After a pull: `bootstrap`, then rebuild. |
| `script/test` | The suite: host-logic crates, then the kernel under QEMU. The gate. `--arch`, `--no-host`, `--cpu`, `--hvf`, `--test` narrow it. |
| `script/verify` | The Kani proofs over the pure-logic crates. |
| `script/verify-riscv64` | The `kernel` row for riscv64, under Kani patched from `patches/`. Provisional name. |
| `script/bench` | icount microbenchmarks; `--check` fails on drift from the baseline. |
| `script/icount` | Boot a `--features icount` kernel and assert the two timing claims a wall clock cannot make. |
| `script/fmt` | Format with the pinned rustfmt; `--check` is the CI gate. |
| `script/lint` | Clippy on every ISA and feature build, plus the non-clippy checks sharing its job. It names every failing section. |
| `script/coverage` | Host-crate coverage, gated at 80% per file. |
| `script/ci-build` | The one list of checks a pull request must pass. CI runs its rows. |
| `script/qemu-check` | Is the QEMU on PATH the pinned one, with the devices the suite needs? |
| `script/ci-qemu` | Build the pinned QEMU on Linux, where apt cannot. |
| `script/drift` | Does a toolchain still build us? |
| `script/toolchain-bump` | Raise the pinned nightly, with every gate as evidence. |
| `script/kani-install` | Install the stock Kani that `.kani-version` pins. The only place that does. |
| `script/kani-bump` | Raise `.kani-version` to the newest stable Kani, and nothing else. |
| `script/server` | Boot the OS in QEMU: the milestone tour, then the shell. |
| `script/console` | Boot straight to the interactive shell at EL0. |

### The tree's records and indexes

Full rows: [records-and-indexes.md](scripts/records-and-indexes.md).

| script | what it does |
|---|---|
| `script/decisions` | Index `design/decisions/`; `--check` is gated in `lint`; `--renumber-from <n>` shifts a collided run up by `--by <k>`, keyed on the filename, per §194 (sessions interleave rather than serialize). |
| `script/roadmap` | Index the milestones; `--ready`, `--unclaimed`, `--outstanding`, `--proposed`. |
| `script/journeys` | Progress against the end-to-end user journeys. |
| `script/citations` | Does each `§N` or `milestone N` gloss match what it cites? `--ratchet` gates added lines. |
| `script/fatal-risks` | Does `design/fatal-risks/README.md` agree with the records it cites? |
| `script/catch-up` | What changed since you last looked. |
| `script/apropos <word>` | Search every document in the repository. |
| `script/names` | Who named this, when, and what was refused. |
| `script/metrics` | The weekly measures behind `notes/project-metrics.md`. |

### How the project measures itself

Full rows: [process-measures.md](scripts/process-measures.md).

| script | what it does |
|---|---|
| `script/rule-violations` | Total the strikes against each documented rule. |
| `script/audits` | Is an audit due? |
| `script/cadence-check` | Which scheduled workflows have stopped producing a result? |
| `script/stranger-test` | Hand the repository to a fresh process and record what it could not work out. |

### Boards and boot checks

Full rows: [boards-and-boot-checks.md](scripts/boards-and-boot-checks.md).

| script | what it does |
|---|---|
| `script/board-console` | Read a real board's serial console and say how far the boot got. |
| `script/board-image` | Build the VisionFive 2 microSD payload. |
| `script/card-check` | Does this card's kernel vouch for its archive? |
| `script/board-netboot` | Serve `target/board` over TFTP so radon boots without a card. |
| `script/netboot-rehearsal` | Boot nife the way xenon will, on patagonia, with nothing plugged in. |
| `script/swish-check` | `console`'s gating twin: type at the prompt and check what came back. On aarch64 and riscv64 it also launches `graphical_terminal` with a virtio-gpu and reads the screen (the old `--graphical` flags, folded in 2026-10-03). |
| `script/boot-check` | Boot the default kernel on all three architectures to a green verdict. |

### Load, concurrency and reliability

Full rows: [load-and-reliability.md](scripts/load-and-reliability.md).

| script | what it does |
|---|---|
| `script/cpu-matrix` | The riscv64 suite against every QEMU CPU model in the matrix. |
| `script/repeat-under-load` | Run the suite N times under a measured load. |
| `script/runner-container` | Boot the suite repeatedly in an approximation of the CI runner. |
| `script/soak-test` | A sustained multicore workload under QEMU, judged as a board is. |
| `script/job-mix` | Rehearse the multi-tasking workload sweep under QEMU. |
| `script/interleaving-check` | The atomic protocols under loom, every interleaving. |
| `script/ci-log-baseline` | Per-check attribution for failed CI jobs, before the logs expire. |

### Analysis, proofs and the supply chain

Full rows: [analysis-and-supply-chain.md](scripts/analysis-and-supply-chain.md).

| script | what it does |
|---|---|
| `script/vendor-verify` | Each `vendor/*.pin` tree is the published tarball plus its patch. |
| `script/vendor-watch` | What upstream has done since each pin. |
| `script/supply-chain` | cargo-deny over each workspace, then `vendor-verify`. |
| `script/fuzz` | Coverage-guided fuzzing of the byte parsers. |
| `script/undefined-behavior-check` | The host-logic tests under Miri. |
| `script/stack-frame-check` | One kernel function's frame, gated at the 4096-byte guard page. |
| `script/stack-depth-check` | How deep a kernel thread stack can get, by walking the call graph. |
| `script/build-is-reproducible` | The same commit builds the same bytes, from any path. |
| `script/fastpath-footprint` | An upper bound on the IPC fastpath's instruction footprint. |
| `script/boot-file-size-check` | The release `BOOTX64.EFI` against a 16 MiB budget, a quarter of the 64 MiB install slot; prints the loader, kernel and archive breakdown. |
| `script/image-permissions` | The shipped kernel images obey W^X. |
| `script/crate-probes` | Build fifty crates.io crates against the patched `std`. |
| `script/crypto-probes` | Which TLS crypto providers build for nife's targets. |
| `script/mutation` | Mutation testing over the host crates. A report, not a gate. |
| `script/mutation-census` | One committed row per crate, per mutation census. |
| `script/falsifications` | Can each Kani harness be made to fail? |
| `script/crate-standalone <crate>` | Would this crate stand on its own outside nife? Provisional name. |

`fmt`, `lint`, `coverage`, `supply-chain`, `fuzz`, `miri`, and `mutants` are not part of the canonical
set; they exist so the CI format, clippy, coverage, supply-chain, fuzz, miri, and weekly mutation
jobs are one-liners. `coverage` measures only the pure-logic host crates(`abi`, `capability`,
`nifefs`, `device_tree_blob`, `elf`, `frames`, `paging`, `pci`, ...). The kernel and user crates run
under QEMU, out of reach of host instrumentation, which is the same reason DECISIONS
§7 keeps the testable logic in host crates in the first place. It installs its own tool rather than
leaning on `bootstrap`, so the CI test job (which runs `bootstrap`) never compiles a coverage tool
it does not use.

### Running the agent fleet

Full rows: the appendices above.

| script | what it does |
|---|---|
| `script/lint-fleet` | The fleet tooling's checks, apart from `script/lint`; CI runs both. |
| `script/claim <branch-name>` | Cut a lane's branch, worktree and draft pull request; sets `nife.fleet`. Push with `script/push`. `--debt-paydown` or `--new-work` ([debt-paydown.md](debt-paydown.md)). |
| `script/preflight-queue` | Replay the merge queue locally before a group build does. |
| `script/architect-queue`, `script/record-ruling` | calef's queue; rulings. |
| `script/nanny` | Work the merge queue by delegating the rebases. |
| `script/redo-rate` | How often delegated work has to be done again. |
| `script/effort` | Machine effort spent per ISO week, from agent session records. |
| `script/launch-pass --reviewer glm\|fable --commit <sha>` | Start one outsider pass, pinned to a commit and a model; `--complete` sends its one completion round ([outsider-passes.md](scripts/outsider-passes.md)). |
| `script/booted-rows` | An outsider pass's booted rows against the brief's floor; `--short-only` for the completion round ([outsider-passes.md](scripts/outsider-passes.md)). |

## They are thin wrappers, on purpose

The scripts do almost nothing themselves. `script/test` is `cargo xtask test`; `script/server`
is `cargo xtask run`; `script/console` is `cargo xtask shell`. `cargo xtask` is still the
engine and still the place the real build logic lives (and it exposes more than the scripts do:
`gdb`, `objdump`, `image`, `std-aborts`). The scripts add a normalized interface on top, duplicating
nothing, and `cargo xtask …` still works.

## `script/` and `helpers/`

`script/` is the front door. `helpers/` is the drawer behind it: cargo runners, `qemu-bounded.sh`,
and modules the scripts import. You do not run those by hand. `bootstrap` installs system packages,
and on Linux it also builds the pinned QEMU. [helpers-and-bootstrap.md](scripts/helpers-and-bootstrap.md)
has the reasons, the exceptions and the 2026-09-23 rename from `scripts/`.

## A piped gate reports the pipe's status, not the gate's

`script/lint | tail -30; echo $?` prints `tail`'s exit code, so a red gate can read green. Write it
as a redirect, and read `$?` before anything else touches it:

```console
$ script/lint > /tmp/lint.txt 2>&1; echo "exit=$?"
```

`set -o pipefail` also fixes it, and `script/` entry points use it. Nothing can gate this.
[piped-gate.md](scripts/piped-gate.md) has the redirect to use instead, and why it stays rung
four.

## CI leverages them

Every job in `.github/workflows/ci.yml` names one check out of `script/ci-build`'s table,
from milestone 286 (one enumeration of the checks that gate a pull request). The format job runs
`script/ci-build fmt`, the lint job `script/ci-build lint`, the suite jobs `script/ci-build test-aarch64-riscv64` and `test-x86_64`, the swish job `script/ci-build swish-check`, the bench job
`script/ci-build bench` and `script/ci-build icount`, and so on. CI runs the commands a
developer does, from one list, and a job without a row is the defect that list prevents. `verify.yml` is the exception and says so: Kani is sharded
across jobs with its own scope predicate, and the table does not claim it.

The lint job is named `lint` (calef, 2026-10-08 UTC). It runs every check in `script/lint`, then
`script/lint-fleet`. The three-step rename of a required check, with the maintainer's ruleset
commands and the step still outstanding, is in [lint-job-rename.md](scripts/lint-job-rename.md).

## The versioned hooks

`.githooks/` holds hooks the repository owns, wired by `script/setup` with
`git config core.hooksPath .githooks`, not files copied into `.git/hooks`, which is neither
versioned nor shared. A lane's worktree shares the main checkout's `.git`, so `core.hooksPath`
covers every worktree at once, the case that motivated the first hook.

- `pre-push` runs `script/fmt --check` (~0.7 s) and refuses the push if rustfmt would change
  a file, because CI's `rustfmt` is a required check and a runner ten minutes later is the slowest
  way to learn about a wrapped line. Every lane on 2026-08-15 and
  -16 paid that tax at least once. `git push --no-verify` bypasses it, deliberately: pushing a
  work-in-progress branch for safekeeping is a legitimate reason, and the hook is a courtesy to
  the queue rather than a rule about what may exist on a branch.
- It then runs `script/lint --no-cargo` (every check that does not invoke cargo; clippy stays in
  CI). A push that changes no files, a claim, skips both. See
  [queue-ejection.md](queue-ejection.md).
- With `nife.fleet` set (by `script/claim`) it runs `helpers/pre-push-fleet.sh`. Without it, no
  `gh` or network.

An existing clone installs it by rerunning `script/setup`, or by hand with the config line above.

### BUGS

- The hook is opt-in per clone. A contributor who never runs `script/setup` never has it, and
  nothing detects that; CI stays the authority.
- It checks the whole tree, not the pushed range. A tree unformatted anywhere fails CI anyway.
- Clippy and the checks that call cargo wait for CI (`script/lint --no-cargo --list` names them).

## Two test images per architecture, and where a new test goes

Since milestone 609 (the system tests leave the kernel crate), each architecture leg of
`script/test` boots two images, one after the other. `cargo test -p kernel` is the kernel's own
unit tests, beside the code they test. `cargo test -p system_tests` is the whole-system suite: the
same kernel, linked as a library from `system_tests/`, with the tests that spawn and drive real
programs. The scanout and network checks watch the second image, because that is where the
display, compositor and network tests live. The kernel crate links no service or fixture crate;
`script/lint` check 13 keeps it that way.

**A new service test is a userspace program**, unless it has to observe kernel internals (calef,
2026-09-27). A test that does goes in `system_tests/src/user/`, and its doc comment says which
internals it observes and why no syscall could show them. Existing tests that observe nothing
internal move to userspace over time; the backlog is in
[system-tests-and-the-kernel-crate.md](system-tests-and-the-kernel-crate.md).

## Running one kernel test (`script/test --test`)

Milestone 210. A host crate's test is a function a harness calls, so `cargo test <name>` has always
worked there. A kernel test runs inside a booted kernel under QEMU, and until this flag
`kernel/src/testing.rs`'s `runner` took no filter, so seeing one meant running all 312.

```
script/test --arch aarch64 --test frames_are_zeroed
```

The substring is matched against the test's full path (`core::any::type_name` of the
`#[test_case]` function), which is the same shape `cargo test <name>` matches, so a module name
selects a module's worth and a full path selects exactly one.

### What it costs, measured

The block that minted this guessed that "the boot is most of the four minutes". It is not. Timed
on patagonia, aarch64,
`cargo xtask test --arch aarch64`:

| | |
|---|---|
| QEMU start to `running 312 tests` (objcopy, QEMU, the whole kernel boot) | **0.50 s** |
| the 312 tests themselves | **53.1 s** |
| the whole `--arch aarch64` run, host crates and builds included | **174 s** |
| the same run with `--test <one test>`, warm | **8.6 s** |

So the boot is about 1% of the QEMU leg, and the flag is worth more than the block expected. What
is left in the 8.6 s is the fixture work `test` does before
any leg (the userspace archive, the `std` exerciser, and five disk images), not the boot.

[one-kernel-test.md](scripts/one-kernel-test.md) has how the filter reaches the kernel, what it
switches off, and `--repeat N`.

### EXAMPLES

```
$ script/test --arch aarch64 --test the_asid_width_supports_the_allocator
--- test filter: the_asid_width_supports_the_allocator (kernel legs only; the host crates have `cargo test`) ---
--- kernel tests, aarch64 (QEMU 11.1.2) ---

running 1 of 312 tests (filter: the_asid_width_supports_the_allocator)

test kernel::arch::aarch64::isa::tests::the_asid_width_supports_the_allocator ... ok

test result: ok. 1 passed
```

A filter that matches nothing fails the run, rather than reporting a green `0 passed`. Both
images run, and the count is taken over the leg, because the test a filter names lives in exactly
one of them. An image with no match says so and exits cleanly, and the leg fails if the two
together selected nothing.

```
$ script/test --arch aarch64 --test no_such_test_anywhere
running 0 of N tests (filter: no_such_test_anywhere)
no test in this image matches the filter `no_such_test_anywhere`; the harness counts the leg
...
running 0 of M tests (filter: no_such_test_anywhere)
no test in this image matches the filter `no_such_test_anywhere`; the harness counts the leg
test: no test on the aarch64 leg matches the filter, in either image (a test only this
architecture lacks? `--test` runs every leg; add `--arch`)
```

A bare `NIFE_TEST_FILTER=x cargo test -p kernel` keeps the older rule and fails on its own, since
nothing is counting across images for it.

### BUGS

- `--test` selects tests, not architectures, and that is deliberate, per DECISIONS §19
  (architectural parity is a tenet). A filter naming an architecture-specific test and no `--arch`
  runs all three legs and fails on the two that do not have it. Failing is the honest outcome,
  because the alternative (skipping a leg with no matches) makes a typo indistinguishable from a
  green run; the message names the fix.
- A filtered run proves nothing about the whole-suite instruments. The frame ledger's
  kept-frames ceiling, the thread peak and the stack high-water are all totals over 312 tests, so a
  one-test run's readings sit far under them and cannot fail. Read a green filtered run as "this
  test passes", never as "the suite would".
- Tests are not independent, and running one alone can fail honestly. A test that only passes
  because an earlier one wired a service will fail on its own. That is a true finding about the
  test, not a defect in the flag.
- The fixture work is not filtered. The 8.6 s above is almost entirely archive and image
  building that happens whether or not the selected test needs a disk. Filtering that too would
  need `test` to know which fixtures a given test wants, which nothing records.

## Appendices

The directory and stems are provisional names; [the directory's README](scripts/README.md) says so.

| Appendix | What it holds |
|---|---|
| [build-test-and-gate.md](scripts/build-test-and-gate.md) | full rows for build, test and gate; what the rows cite; Counted claims, one of `script/lint`'s checks |
| [records-and-indexes.md](scripts/records-and-indexes.md) | full rows for the record and index commands |
| [process-measures.md](scripts/process-measures.md) | full rows for the process measures |
| [boards-and-boot-checks.md](scripts/boards-and-boot-checks.md) | full rows for the board and boot commands |
| [load-and-reliability.md](scripts/load-and-reliability.md) | full rows for the load and reliability commands |
| [analysis-and-supply-chain.md](scripts/analysis-and-supply-chain.md) | full rows for analysis, proofs and the supply chain |
| [helpers-and-bootstrap.md](scripts/helpers-and-bootstrap.md) | Two things that are deliberately the way they are |
| [one-kernel-test.md](scripts/one-kernel-test.md) | How the filter reaches the kernel; What the flag turns off |
| [piped-gate.md](scripts/piped-gate.md) | a piped gate's exit status, and the fix |
| [heavy-job-slots.md](scripts/heavy-job-slots.md) | how many heavy jobs run at once, and nesting |
| [lint-job-rename.md](scripts/lint-job-rename.md) | the lint job, and the three steps that rename a required check |
| [outsider-passes.md](scripts/outsider-passes.md) | `script/launch-pass` and `script/booted-rows`: full rows, EXAMPLES and BUGS |
