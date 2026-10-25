---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 814. `non_volatile_memory_express`: proven NVMe queue logic, released on its own

*(From calef's rulings on pull request #1806; the title and slug are drafts. The FAT crate's block,
milestone 813 (a proven FAT crate released on its own), is the shape this follows.)*

calef's rulings, quoted from the maintainer comments on #1806 of 2026-10-07 (UTC):

- Policy: *"Each publish-ours crate (NVMe, e1000e, the DesignWare pair, the JH7110 pair, xHCI, and
  `portable_executable` once it carries proofs) is published when ready (proofs passing, API
  stable), each through its own milestone at its review."* His words: *"Since the consumer would be
  other hobby OSes, I think it makes sense to publish them. That's a way to engage with the broader
  community of os developers. I can't think of who else might want these crates again if nife fails
  a nice legacy is a bunch of hobby oses that use its parts."*
- Crates: *"one milestone per crate, minted now, each keeping its tree name (all free on crates.io
  as of today) [...] Names are ratified now and permanent on first publication."* His words: *"Yes,
  one per crate"*.
- The release rules are forks 1 to 4 and 6 of the same pull request, applied in *Release* below.

## What exists, checked 2026-10-07 (UTC)

- `crates/non_volatile_memory_express`: 733 code lines (1,335 with comments, one file), class A in
  #1806's note `notes/releasable-crates-2026-10-07.md`, so it depends on no other nife crate and
  needs no cut.
- Eight Kani harnesses, each with a falsification patch in `falsifications/`. Submission pushes and
  completion pops stay in bounds, and the phase flips only at the wrap. Doorbells never collide.
  The PRP pair is total and page-disciplined, and the identify parse is total and bounds its shift.
  The spawn handoff round-trips and keeps its doorbells inside the mapped page. A transfer command
  is only built for a block the namespace has.
- Its consumers: `kernel/src/non_volatile_memory_express.rs` (the volatile shell),
  `kernel/src/user/non_volatile_memory_express_service.rs`, `kernel/src/user/install_service.rs`,
  `kernel/src/disk_throughput.rs`, `components/src/non_volatile_memory_express.rs` (the EL0
  driver) and `system_tests/src/user/non_volatile_memory_express_tests.rs`.
- The `Handoff` types encode nife's spawn ABI rather than NVMe. Whether they stay in the published
  crate or move to the driver is the lane's call, recorded in its pull request, since a stranger's
  kernel has no use for them.
- crates.io answers 404 for the name (checked 2026-10-07).

**Reuse:** the alternatives were surveyed in #1806's `other-implementations.md` appendix (looked up 2026-10-07),
and none is taken, because calef ruled this crate "publish ours". rcore-os's `nvme-driver` 0.8.2
(MIT, 4.9k downloads, one reverse dependency, no proofs found) lives in the `tgoskits` monorepo.
lihanrui2913's `nvme` is a 0.0.0 from 2025-04. Redox's `nvmed` is a daemon, not a library, and its
GitHub mirror is archived. SPDK (C) stays the reference for behavior. None is a sans-IO layer a
model checker can reach, and the proofs are the point. No upstream offer goes anywhere: calef ruled
on #1806 that nife contributes only to projects nife depends on.

## What is proven

Each row is a property this crate must prove before it publishes. The harness cell names the Kani
harness that proves it, or says `none yet`; every named harness carries a falsification patch that
makes it fail at its own assertion. Each patch is at
`crates/non_volatile_memory_express/falsifications/`. Measured 2026-10-11 (UTC): 8 of 8 proven.

| property | harness |
|---|---|
| Submission queue pushes stay in bounds | `verification::sq_push_stays_in_bounds` |
| Completion pops stay in bounds, and the phase flips only at the wrap | `verification::cq_pop_stays_in_bounds_and_flips_only_at_the_wrap` |
| Distinct doorbells never collide | `verification::distinct_doorbells_never_collide` |
| The PRP pair is total and page-disciplined | `verification::prp_pair_is_total_and_page_disciplined` |
| The identify-namespace parse is total and bounds its shift | `verification::identify_parse_is_total_and_bounds_the_shift` |
| The spawn handoff round-trips | `verification::the_spawn_handoff_round_trips` |
| An accepted handoff keeps its doorbells inside the mapped page | `verification::an_accepted_handoff_keeps_its_doorbells_inside_the_mapped_page` |
| A transfer command is built only for a block the namespace has | `verification::a_transfer_command_is_only_built_for_a_block_the_namespace_has` |

Not proven: the `Cap` register decode beyond the doorbell stride the handoff harness bounds, the
status fields of `Completion::from_dwords`, and anything in time (reset, enable, timeouts), which
lives in the driver's volatile shell. Two rows prove nife's spawn handoff. If `Handoff` moves to the
driver, they move with it and the table loses two rows.

## Release, by #1806's rulings

1. The name is settled before anything leaves (857's Fork 5 as calef amended it on 2026-10-11 UTC).
   `script/names non_volatile_memory_express` reports it ratified, crates.io answers 404 for it (or
   it is already ours), and `nifeos/non_volatile_memory_express` does not exist yet.
   `script/crate-standalone` checks the first.
2. In the tree, `script/verify` proves the harnesses and `Cargo.toml` says `publish = false`.
3. The package is 0.1.0 with its own semver, states `rust-version` (#1806's note built it on stable
   1.94.1), and ships its harnesses and falsification patches. `cargo package --list` shows them.
4. Ready means proofs passing and the API stable. §235 (the OS is built and updated from packages,
   and the tree divides by what releases together) has milestone 610 (the interface's stability is
   measured weekly)'s per-package co-change column, and the soak (step 5) is how the second is
   measured. When the soak exits, calef adds it to the lint-checked publish allowlist. The
   `nifeos/non_volatile_memory_express` repository is created when the crate moves, on the shared CI
   template (milestone 887), through reusable workflows: Kani through `kani-github-action`, lint,
   release. Its release workflow refuses to publish unless the proof job is green. No harness here
   reads `target_arch`, so stock Kani proves them. The patched Kani of §218 (carry a Kani patch so
   riscv64 is proved, and send it upstream) is carried only if one comes to need riscv64. Milestone
   589 (Kani can prove riscv64 from the hosts we already have) built it.
5. Soak, as its own repository (calef, 2026-10-17 UTC, replying "Agrred" (sic) to the maintainer's
   proposal; §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)).
   Milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) is the pilot and
   soaks first. Before publication the crate is moved, not copied: it leaves this tree, and nife
   depends on `nifeos/non_volatile_memory_express` by `rev`. A copy left behind does not count. The
   soak ends on evidence, not a duration, when all five hold:
   1. nife builds and passes CI against the repository at a pinned commit, with no path dependency
      left.
   2. The shared template's weekly run (milestone 887) is green on the crate.
   3. A release dry run is green.
   4. The stranger test builds and proves the crate from its README alone.
   5. The API has gone a set period with no change `cargo-semver-checks` calls breaking. The period
      is not yet set.
6. crates.io trusted publishing from that workflow, owned by a `nifeos` team, with no stored token.
7. calef approves this publication, as he does every one.
8. nife consumes the crates.io release, pinned with its checksum in `Cargo.lock`. A `[patch]` to
   a checkout is allowed only during development, and a lint fails one that reaches `main`. The
   soak's git dependency goes.

## Parity

Pure logic, tested on the host. This lane built it on 2026-10-07 with the pinned nightly for
`aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none` and the
host, all exit 0. The release workflow builds the same four.

## Standalone, measured 2026-10-11 (UTC)

`script/crate-standalone non_volatile_memory_express` fails today, at base `ae6b91e79`. The manifest
inherits `license` and `lints` from nife's workspace and states no `rust-version`, and there is no
README, `LICENSE-MIT` or `LICENSE-APACHE`. Those hold for all nine crates. Builds and tests on
stable for the host and the three bare-metal targets. Stock Kani proves all eight harnesses in the
copy, and each falsification patch, applied with `-p3`, turns its harness red. It fails on manifest
and files only, plus 25 lines citing this tree: 12 `§N`, 8 `milestone N`, 2 tree paths, 2 naming
calef and 1 provenance block.

`script/names` reports the name `non_volatile_memory_express` ratified (checked 2026-10-25).

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn handoff
  or nife error enum in a public signature. A reviewer judges this; no script can. Known today:
  `Handoff` encodes nife's spawn ABI. Whether it leaves the public API is the lane's call above.

## Exit

A check proves it. In nife, `cargo tree --locked -i non_volatile_memory_express` resolves to
`v0.1.x` from crates.io with a checksum in `Cargo.lock`, `crates/non_volatile_memory_express` no
longer exists, and the tree builds for all three targets. In `nifeos/non_volatile_memory_express`,
the run that published 0.1.0 shows the Kani job green ahead of the publish job, and every harness's
falsification patch makes its proof fail. The five soak criteria of release step 5 hold before it
publishes.

The name check of release step 1 passes first, so the repository, the package and every doc link
carry a ratified name. Before the move, `script/crate-standalone non_volatile_memory_express` exits
0 in nife, and every row of *What is proven* names a harness that stock Kani proves in the copy and
whose falsification patch makes it fail. No crate publishes without Kani harnesses covering its
hostile-input and state-machine surface, each carrying a falsification patch that makes it fail at
its own assertion, replayable in the crate's own repository with stock Kani.

## What the crate no longer cites

The crate's source cites nothing of this tree, so this section keeps the links it carried. Its
`Name:` blocks are in [names.md](0814-the-nvme-queue-crate-released-on-its-own/names.md).

- `src/lib.rs`, the crate doc: the design is in notes/non-volatile-memory-express.md, and the
  protocol walk-through is its section "How the protocol works, in one sitting".
- `src/lib.rs`, `Command::flush`: the silent yes a flush must never give is the one milestone 55
  (Time Machine: SMB3 with Apple's extensions, and mDNS)'s verb exists to forbid.
- `src/lib.rs`, `lba_format_shift`: the bench rehearsal belongs to milestone 261 (the NVMe driver
  leaves the kernel, on the machine that can finally confine it). The format condition is the second
  night-of condition of its fatal risk 6, for the Micron drive.
- `src/lib.rs`, the admin and data plane banner: the split is milestone 261's and §86 (whether an
  NVMe driver can leave the kernel, and what capability would let it)'s option 2a. Its history is
  notes/non-volatile-memory-express.md, "The driver left the kernel, 2026-09-17".
- `src/lib.rs`, `MAX_DSTRD`: the one page of BAR0 is §86's option 2a, and the bound's bug is in
  the BUGS of notes/non-volatile-memory-express.md.
- `src/lib.rs`, `CLIENT_WINDOWS`: the windows came from milestone 834 (ioping on nife and Linux).
  calef ruled "Per-client window" on nifeos/nife#2371, 2026-10-18 23:06 UTC. A frame per client
  channel is milestone 599 (a frame per filesystem client channel)'s pattern.
- `src/lib.rs`, `can_write`: calef ruled the disk tree a read-only view on 2026-10-18 at
  21:36 UTC.
- `src/lib.rs`, `client_window`: the corruption is `redoxfs_server`'s lesson from milestone 599.
- `src/lib.rs`, `ESCAPE_MAGIC`: its caller is milestone 261's Outstanding item, under milestone
  202 (every confinement test is a ritual until somebody breaks the confinement and watches it
  fail)'s convention.
- `src/lib.rs`, the `verification` module: `script/verify` runs the proofs, and
  notes/verification.md describes them.
- `src/lib.rs`, `the_spawn_handoff_round_trips`: the handoff is milestone 261's, and the split is
  §86's option 2a.
- `src/lib.rs`, `a_transfer_command_is_only_built_for_a_block_the_namespace_has`: the data plane
  that leans on this bounds check is §86's option 2a.
- `Cargo.toml`, the manifest comment: the crate is a pure-logic crate under §7 (testing: QEMU
  harness + host-testable crates, from commit one).
- `Cargo.toml`, the `confinement_attackers` feature: calef ratified the feature name on 2026-10-05
  (#1647). `components/confinement_attackers` (the server that reads `ESCAPE_MAGIC`) and the
  kernel's `system_tests` (the wiring that writes it) turn it on, and nothing a shipped image is
  built from does. The `documentation` build also compiles the crate with it.

## BUGS

- Steps 4 and 5 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team,
  and on milestone 610's per-package co-change column.
- `needs_person` is `yes` because the exit includes a publication only calef can approve. Everything
  before step 6 can start now.

## Index row

nife's NVMe queue arithmetic, with eight Kani proofs and their falsification patches, published on
crates.io as `non_volatile_memory_express` 0.1 from its own `nifeos` repository. Another hobby OS
can then drive NVMe on proven queue logic, and nife consumes the same release.
