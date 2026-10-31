---
status: PROPOSED
raised: 2026-10-30
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The cross-repository pair is tested where the pin is

Raised 2026-10-30 (UTC) by the lane that landed milestone 956 (the linked interfaces carry checkable versions) step 1, after a one-line magic change broke nife's `rg-threads` and `swish-check` rows. The failure was not in the change. It was in where nife verifies a cross-repository pair.

Reuse: basalt already checks out nife at its pinned commit and boots nife's `swish-check` rows against the packages it built. nife already holds the `rg-threads` test body. This proposal moves a check between two existing gates and adds no dependency.

## What the tree does today

nife's `rg-threads` and `swish-check` rows do not build `rg`. `cargo xtask basalt-fetch` fetches basalt's published package at the release `xtask/basalt-pin` names, which is `rolling-2026-10-17-155eba5`. basalt built that package against the nife commit in its `pins.toml`, which is `942328b8`. So a row that proves `rg` on nife runs a binary built from an older nife.

The build graph is acyclic. basalt depends on nife. What is circular is verification: to prove the current tree, nife runs a program built from a past one. The lag is not minutes. On 2026-10-30 nife's `main` was `f943439c`, its pin was thirteen days old, and basalt's `main` was `d0bff76` pinning nife `942328b8`.

## The defect

A pull request that changes a wire format on the writer side cannot go green in one step. basalt rebuilds the reader that must accept the new bytes only after the change merges, and basalt refuses a pin at an unmerged branch. Milestone 956 step 1 hit this: re-minting `program_argument_contract`'s magic from `nifeargv` to `nifargv1` made the rebuilt shell write pages that basalt's prebuilt `rg` refused.

The same shape covers every foreign program basalt delivers: `rg`, `ioping`, `hackbench` and `sqlite` all read nife contracts. A reader-first rollout escapes the cycle, because a reader that accepts both magics is backward compatible. But that is a rule somebody must remember, not a mechanism, and step 1 shows it is easy to forget.

## The change

Move the foreign-program verification to basalt, where the pair is explicit and the pin is the contract.

- basalt's `packages` workflow already checks out nife at `pins.toml`'s commit and boots nife's `swish-check` rows with `NIFE_BASALT_REPOSITORY`. It runs the `rg`-dependent tests, `rg-threads` among them, against the packages it just built at that commit. The pair under test is then the pin's pair, and basalt owns it.
- nife stops fetching basalt's programs for its own `rg-threads` and `swish-check` rows. nife keeps the argument contract's reader and writer tests in `program_argument_contract`, and a small in-tree program that reads an argv page, so a contract change still meets a reader in the same tree.

nife's gate then builds everything it verifies. A contract change meets its real reader in basalt's gate, at a known commit, with no lag hidden in nife's.

## What it costs

The `rg`-dependent rows move, and they do not disappear. basalt's gate grows by the `rg-threads` runtime, about four to five minutes on the arm64 pool, and it gains a step that runs nife's system test at the pin. nife's gate loses the ripgrep threading row and keeps argv coverage through the fixture. The measure to take before and after is wall time on each gate and the number of rows that need a basalt artifact.

## What it does not do

It does not re-mint the argv magic. `nifeargv` stays unnumbered until this lands. After it, the digits are a one-line change in `program_argument_contract` with no cross-repository dance and no red window. It also does not repair the second-address `backup` job in basalt, which fails until its deploy key exists; that is separate.

When this lands, the `# BUGS` entry in `xtask/src/basalt.rs` that records the break cannot land in one pull request comes out.

## Alternatives considered

The reader-first rollout alone: it works, and it is the escape step 1 used. It leaves the cycle in place and makes a discipline load-bearing, so it loses as the only answer.

A lint that refuses a writer-side magic change when a foreign reader exists: cheaper, and useful beside this. It still leaves nife proving a tree against a binary it did not build.

Building `rg` in nife's CI from crates.io: refused. It duplicates basalt's recipe, which then drifts from the one basalt publishes, and nife deliberately builds no foreign program.
