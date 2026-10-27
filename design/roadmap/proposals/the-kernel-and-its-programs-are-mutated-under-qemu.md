---
status: PROPOSED
raised: 2026-10-27
milestone_dependencies: 517, 742, 924
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# Mutation testing reaches the kernel and its programs under QEMU, a diff and a slice at a time

Raised 2026-10-27 (UTC) by `lane/mutation-gaps-plan`, the second of the three mutation-testing
gaps calef ruled closed that day. Nothing mutates the kernel or the user-mode programs. Kani proofs
and QEMU tests cover them, and nothing asks whether those would notice a wrong line.

This builds what milestone 517 (what fraction of survivor growth arrives on lines a pull request
touched) priced and called the affordable half: a diff-scoped kernel check. Its appendix
`kernel-pricing.md` refused a kernel census at about 500 runner-hours. This proposal keeps that
refusal and replaces it with a rolling census. The gate of milestone 924 (a survivor on a pull
request's own lines cannot merge untriaged) passes a kernel pull request with "no mutants: outside
the corpus", and this is what retires that summary.

## What sits outside the corpus

`cargo mutants --no-config --list`, run on 2026-10-27 (UTC):

| package | mutants |
|---|---|
| `kernel` | 10,461 |
| `components` | 5,663 |
| `system_initializer` | 1,029 |
| `virtio` | 567 |
| `user_mode_runtime` | 452 |
| `hot_swap_contract`, `supervision_protocol`, `memory_region_heap` | 346 |
| total | 18,518 |

The host census covers 20,974, so the unmutated half of the tree is about as large as the mutated
half. In the kernel, `arch/` holds 3,879 (x86_64 2,066, riscv64 988, aarch64 825). The kernel side
of the programs' wiring in `user/` holds 1,476. The confinement core holds about 1,800: `sched/`,
`syscall/`, `revoke`, `cap`, `memory_region` and their neighbors. Tests, benches and self-tests
hold about 965, and those leave the count by exclusion.

## What one mutant costs

- Rebuilding the kernel's test binary after a one-line mutation took 3.5 seconds on an 80-core
  Linux host, and the system tests' binary 8.9 seconds. A four-core CI runner takes longer, and
  part 1 measures it there.
- In merge-group run 40537138863 (2026-10-27), the kernel suite ran 141 tests in 8 seconds on
  aarch64 and 152 in 10 seconds on x86_64. The system tests ran 388 in 133 seconds on x86_64.

A kernel mutant therefore costs about 20 seconds against the kernel suite and about 150 against
both. `components` has no tests of its own, so every one of its mutants pays the system tests.

## The options

**A. A full kernel census under QEMU every week.** 10,461 mutants at 20 seconds is 58 runner-hours
per architecture before any system test, and about 440 with them. §19 (architectural parity is a
tenet) would triple it. Refused on cost, as milestone 517 refused it.

**B. QEMU, scoped two ways.** A diff-scoped check mutates only the lines a kernel or `components`
pull request changed. A rolling census mutates one slice of the kernel a week under a fixed budget,
so the whole kernel passes through it every few weeks. Both run in two tiers. The kernel suite runs
first, and only its survivors pay for the system tests. An `arch/` file runs on its own
architecture. Arch-independent code runs on one, the cheapest, which part 1 picks by measuring.
Survivors go into the same ledger under the rule of milestone 85 (mutation testing over the host
crates) and under 924's key.

**C. Move decisions out to host crates.** This is the tree's existing practice: `capability`,
`paging`, `timetable`, `memory_regions`, `inter_process_communication` and `generational_table`
all left the kernel, and the host census mutates each at 2.35 seconds per mutant. Milestone 278
(two modules fenced out by a `Cargo.toml`) is one such move for `user_mode_runtime`. It is the most
elegant answer for pure logic. It cannot reach the glue, the arch code or the wiring, though, and
each move is a restructuring rather than a measurement.

**D. Kani harnesses as the test.** Milestone 741 (does a standing proof notice a regression) ran
this: the kernel's five aarch64 and `syscall` harnesses reach 80 mutants and kill 51. That measures
the proofs, not the tests, and covers under 1% of the kernel. Milestone 776 (a weekly check that
every proof can still fail) owns its weekly form.

**The recommendation is B**, with C continuing as ordinary practice and D left to 776. B is the
only option that reaches the glue, `arch/` and the programs, and it would win at equal cost. Its
cost is runner time on shared runners, which is the question for calef.

## How a kernel mutant runs

cargo-mutants 27.1.0's `--test-tool` takes `cargo` or `nextest` and nothing else. The system
tests need `cargo xtask test`'s scaffolding: the initrd, the RedoxFS and GPT images, the NVMe disk
and the `NIFE_*` device variables. Milestone 517 found that a bare `cargo test` in a cargo-mutants
build directory fails its unmutated baseline for that reason.

`script/falsifications` already replays a kernel test's record the way a mutant needs. It applies a
patch, boots, and reads the named failure, the replay that milestone 742 (every test is falsified as
routine) built. So cargo-mutants generates the mutants as patches, `cargo mutants --list --diff`,
and the falsification driver runs them. Generation stays with the tool that names mutants the ledger
already keys on, and execution stays with the driver that already knows how to boot this tree.

## What done means

- Every kernel pull request runs the diff-scoped check, and its summary reports mutants rather than
  "outside the corpus". It is a required check once it has run green for four weeks; adding it to
  the required set is calef's settings change.
- Every kernel module and every `components` program appears in the census record no more than one
  rotation old. A rotation is the time one pass takes at the agreed budget.
- Every survivor the rolling census finds has a row under the ledger's rule, the same as a host
  survivor.
- Fatal risk 3's caveat that the verdict does not speak for the kernel narrows to whatever B still
  cannot reach, stated with numbers.

## How the work splits

1. The driver and the measurement. Feed `--list --diff` patches to the falsification driver for
   one module on one architecture, and record the per-mutant cost on a CI runner for each tier. One
   lane, and every number below is an estimate until it lands.
2. The kernel's exclusions: test, bench and self-test modules, each with its reason, in the
   kernel's own section of `.cargo/mutants.toml`.
3. The diff-scoped check, advisory, sharded so each job stays under 20 minutes. At about 150
   seconds per mutant and two jobs per runner, one job holds about 16 mutants. A larger diff takes
   more shards, never a longer job. One lane.
4. The rolling census, one workflow with a slice list and a cap on concurrent jobs. The slices go
   in order of what a wrong line costs: `syscall/`, `revoke`, `cap`, `memory_region`, `sched/`,
   then `arch/` per architecture, then `user/`, then `components` against the system tests. One
   lane.
5. Triage, one lane per slice as each lands, under the companion proposals' rules for accepted and
   hanging survivors.

## What holds the gain

- The diff-scoped check, once required, holds new kernel code to 924's rule.
- `script/cadence-check` fails when a slice in the census record is older than one rotation.
- The census-wide check from `a-survivor-accepted-once-is-not-accepted-forever.md` covers the
  kernel's rows like any other crate's.

## Cost, estimated until part 1 measures it

At 20 seconds per mutant, the kernel's first tier is 58 runner-hours a pass. If half its mutants
survive that tier, the second adds about 190. `components` is about 236 at 150 seconds each. At a
budget equal to today's host census, about 14 runner-hours a week, the kernel's first tier passes
in about four weeks. The full rotation, kernel and programs, takes several months. A larger budget
shortens it in proportion.

GitHub-hosted runners cost this public repository no money. The limit is the organization's shared
concurrency. Milestone 741's first scaled run queued 40 jobs on those runners, and the maintainer
cancelled it. And `gating-ci-rents-more-concurrency-before-it-owns-a-runner.md` is the open proposal
about that limit. The rolling census therefore runs at a capped `max-parallel`, outside the hours
the merge queue is busy. The diff-scoped check on pull requests runs on hosted runners only, never
on owned hardware, since pull requests from forks never run there.

Reuse: cargo-mutants generates every mutant (`--list --diff`), `script/falsifications` runs each
one under QEMU, and `helpers/mutation_diff_gate.py` and the triage ledger judge the result. The
one piece written new is the workflow that feeds the first into the second.

## Names

The rolling census, its slices and the rotation. Name: provisional, minted by this proposal on
2026-10-27 (UTC). An architect names things.

## Index row

18,518 mutants in the kernel, `components` and the crates that need a kernel never run, about as
many as the host census covers. Proposed: a diff-scoped check on kernel pull requests and a
rolling census a slice a week, both under QEMU through the falsification driver, with host
extraction and Kani kept for what each already does.
