---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 818. `jh7110_entropy`: proven JH7110 TRNG logic, released on its own

*(From calef's rulings on pull request #1806; the title and slug are drafts. The FAT crate's block,
milestone 813 (a proven FAT crate released on its own), is the shape this follows.)*

calef's rulings, quoted from the maintainer comments on #1806 of 2026-10-07 (UTC):

- Policy: *"Each publish-ours crate (NVMe, e1000e, the DesignWare pair, the JH7110 pair, xHCI, and
  `portable_executable` once it carries proofs) is published when ready (proofs passing, API
  stable), each through its own milestone at its review."* His words: *"Since the consumer would be
  other hobby OSes, I think it makes sense to publish them. [...] a nice legacy is a bunch of hobby
  oses that use its parts."*
- Crates: *"one milestone per crate, minted now, each keeping its tree name (all free on crates.io
  as of today) [...] Names are ratified now and permanent on first publication."* His words: *"Yes,
  one per crate"*.
- The release rules are forks 1 to 4 and 6 of the same pull request, applied in *Release* below.

## What exists, checked 2026-10-07 (UTC)

- `crates/jh7110_entropy`: 516 code lines (1,247 with comments), written by milestone 159 (a real
  hardware entropy source: the JH7110's TRNG), which is BUILT. It holds the StarFive JH7110 true
  random number generator's register layout, the decision whether a generation attempt succeeded,
  failed or is still running, and the device-tree query that finds it.
- Four Kani harnesses, each with a falsification patch: a lockup bit is never overridden, ready
  requires `RAND_RDY` and carries the words untouched, neither bit set is always not ready, and an
  unseeded core never yields bytes.
- Class B in #1806's note `notes/releasable-crates-2026-10-07.md`. **The cut**, which is codebase
  rule 2 (a driver gets what it needs passed in): `discover` takes a `DeviceTreeBlob`; the caller
  passes the register region instead, and the query moves to the kernel side, which already depends
  on `device_tree_blob`. The tests also `include_bytes!` a fixture from
  `../../device_tree_blob/tests/fixtures/`, outside the package.
- Its consumers: `kernel/src/user/entropy_service.rs`, `kernel/src/memory.rs`, `kernel/src/lib.rs`
  and `components/src/jh7110_entropy.rs`.
- crates.io answers 404 for the name (checked 2026-10-07).

**Reuse:** the alternatives were surveyed in #1806's `other-implementations.md` appendix (looked up 2026-10-07),
and none is taken, because calef ruled this crate "publish ours". The only Rust JH7110 crates found,
weathered-steel's `jh71xx-pac` 0.11.1 and `jh71xx-hal` 0.7.2 (Codeberg, 2025-03), are register maps
under GPL-3.0-only, which nife cannot take under its license and which carry no TRNG logic. No Rust
crate for the TRNG was found. Linux's `jh7110-trng` driver stays the reference and is read, never
copied. No upstream offer goes anywhere: calef ruled on #1806 that nife contributes only to
projects nife depends on.

## What is proven

Each row is a property this crate must prove before it publishes. The harness cell names the Kani
harness that proves it, or says `none yet`; every named harness carries a falsification patch that
makes it fail at its own assertion. Each patch is at `crates/jh7110_entropy/falsifications/`.
Measured 2026-10-11 (UTC): 4 of 4 proven.

| property | harness |
|---|---|
| A lockup bit is never overridden | `verification::a_lockup_bit_is_never_overridden` |
| Ready requires `RAND_RDY` and carries the words untouched | `verification::ready_requires_rand_rdy_and_carries_the_words_untouched` |
| Neither bit set is always not ready | `verification::neither_bit_set_is_always_not_ready` |
| An unseeded core never yields bytes | `verification::an_unseeded_core_never_yields_bytes` |

Not proven: the quality of the randomness, which is a property of the silicon that no proof
reaches; `Pool` and `assemble`, which copy words; and the device-tree query, which the cut moves
out.

## Release, by #1806's rulings

1. The name is settled before anything leaves (857's Fork 5 as calef amended it on 2026-10-11 UTC).
   `script/names jh7110_entropy` reports it ratified, crates.io answers 404 for it (or it is already
   ours), and `nifeos/jh7110_entropy` does not exist yet. `script/crate-standalone` checks the
   first.
2. Make the cut above. `cargo tree -p jh7110_entropy` then lists no nife crate, and the tests read
   only fixtures inside the package.
3. In the tree, `script/verify` proves the harnesses and `Cargo.toml` says `publish = false`.
4. The package is 0.1.0 with its own semver, states `rust-version` (#1806's note built it on stable
   1.94.1), and ships its harnesses and falsification patches. `cargo package --list` shows them.
5. Ready means proofs passing and the API stable. §235 (the OS is built and updated from packages,
   and the tree divides by what releases together) has milestone 610 (the interface's stability is
   measured weekly)'s per-package co-change column, and the soak (step 6) is how the second is
   measured. When the soak exits, calef adds it to the lint-checked publish allowlist. The
   `nifeos/jh7110_entropy` repository is created when the crate moves, on the shared CI template
   (milestone 887), through reusable workflows: Kani through `kani-github-action`, lint, release.
   Its release workflow refuses to publish unless the proof job is green. No harness here reads
   `target_arch`, so stock Kani proves them. The patched Kani of §218 (carry a Kani patch so riscv64
   is proved, and send it upstream) is carried only if one comes to need riscv64. Milestone 589
   (Kani can prove riscv64 from the hosts we already have) built it.
6. Soak, as its own repository (calef, 2026-10-17 UTC, replying "Agrred" (sic) to the maintainer's
   proposal; §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)).
   This crate is the pilot and soaks first: it is the smallest crate with a real cut, and the cut is
   where refactor lessons show up. Before publication the crate is moved, not copied: it leaves this
   tree, and nife depends on `nifeos/jh7110_entropy` by `rev`. A copy left behind does not count.
   The soak ends on evidence, not a duration, when all five hold:
   1. nife builds and passes CI against the repository at a pinned commit, with no path dependency
      left.
   2. The shared template's weekly run (milestone 887) is green on the crate.
   3. A release dry run is green.
   4. The stranger test builds and proves the crate from its README alone.
   5. The API has gone a set period with no change `cargo-semver-checks` calls breaking. The period
      is not yet set.
7. crates.io trusted publishing from that workflow, owned by a `nifeos` team, with no stored token.
8. calef approves this publication, as he does every one.
9. nife consumes the crates.io release, pinned with its checksum in `Cargo.lock`. A `[patch]` to
   a checkout is allowed only during development, and a lint fails one that reaches `main`. The
   soak's git dependency goes.

## Parity

Pure logic, tested on the host. This lane built it on 2026-10-07 with the pinned nightly for
`aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none` and the
host, all exit 0. The release workflow builds the same four. The silicon is riscv64 only; the
crate is not.

## Standalone, measured 2026-10-11 (UTC)

`script/crate-standalone jh7110_entropy` fails today, at base `ae6b91e79`. The manifest inherits
`license` and `lints` from nife's workspace and states no `rust-version`, and there is no README,
`LICENSE-MIT` or `LICENSE-APACHE`. Those hold for all nine crates. It does not build alone:
`device_tree_blob` is a path dependency outside the copy, so neither the build nor Kani ran. A test
reads a fixture from `../../device_tree_blob/tests/fixtures/`, outside the crate. Beyond manifest
and files, 26 lines cite this tree: 14 `milestone N`, 5 tree paths, 3 `§N`, 2 naming calef and 2
provenance blocks.

`script/names` reports the name `jh7110_entropy` ratified (checked 2026-10-25).

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn handoff
  or nife error enum in a public signature. A reviewer judges this; no script can. Known today:
  `discover` takes a `DeviceTreeBlob`; the cut makes the caller pass the register region.

## Exit

A check proves it. In nife, `cargo tree --locked -i jh7110_entropy` resolves to `v0.1.x` from
crates.io with a checksum in `Cargo.lock`, `crates/jh7110_entropy` no longer exists, and the tree
builds for all three targets. In `nifeos/jh7110_entropy`, the run that published 0.1.0 shows the
Kani job green ahead of the publish job, and every harness's falsification patch makes its proof
fail. The five soak criteria of release step 6 hold before it publishes.

Two checks of the shared template (milestone 887) are this crate's, because it publishes first.
calef, 2026-10-22 (UTC): *"A: 887 is BUILT once a v7 dry-run release on the canary passes the gate
and waits in crates-io for my approval; the first-publish and semver-baseline checks move to 818."*

- First publish: 0.1.0 publishes through the scoped token (`CRATES_IO_FIRST_PUBLISH_TOKEN` on the
  `crates-io` environment, reaching the called release job), after the gate and calef's approval.
  calef then binds the trusted publisher and deletes the secret.
- Semver baseline: a pull request that breaks the API against the published 0.1.0 gets
  `needs-architect` from the template's labeler.

The name check of release step 1 passes first, so the repository, the package and every doc link
carry a ratified name. Before the move, `script/crate-standalone jh7110_entropy` exits 0 in nife,
and every row of *What is proven* names a harness that stock Kani proves in the copy and whose
falsification patch makes it fail. No crate publishes without Kani harnesses covering its
hostile-input and state-machine surface, each carrying a falsification patch that makes it fail at
its own assertion, replayable in the crate's own repository with stock Kani.

## What the crate no longer cites

The crate's source cites nothing of this tree, so this section keeps the links it carried. Its
`Name:` blocks are in [names.md](0818-the-jh7110-entropy-crate-released-on-its-own/names.md).

- The crate doc: the sources behind its `[trm]`, `[driver]`, `[netbsd]` and `[binding]` labels,
  with the bring-up order and health testing, are `notes/entropy/jh7110-trng.md`; the driver
  program is `components/src/jh7110_entropy.rs`.
- `IE_GLBL_EN`: the bench run that showed polling works with `IE` never written is
  `target/board/radon-2026-09-04-clock-and-first-entropy.log`, and the driver program says why it
  polls.
- `COMPATIBLE_VENDOR`, `Discovered`: the vendor node and its evidence are the note's "The vendor
  spelling" section, found by milestone 239 (radon's device tree does not describe the TRNG, so a
  working driver never runs).
- `interpret`, `Pool` and the `an_unseeded_core_never_yields_bytes` harness and patch: milestone
  159 (a real hardware entropy source: the JH7110's TRNG) added the `stat` parameter and the pool.
- `assemble`: no conditioning here for the reason §44 (entropy is a capability, `std::random` improves
  transparently, and the refusal is loud) gives `components/src/entropy.rs`.
- `WORD_BYTES` and `take`'s clamp: the same 8 as `entropy_contract::MAX_BYTES`, and
  `entropy_contract::want` clamps first in the driver.
- The tests: milestone 326 (nobody has been assigned to turn a mutation score upward) found the
  survivors the transcription, clear-mask and deadline tests kill.
- The harness `ready_requires_rand_rdy_and_carries_the_words_untouched` and its patch: milestone
  211 (a harness that states its property through the function under test cannot see that
  function break) rewrote the harness after it verified blind. The module's proofs serve §14 (the project's
  direction: a verified-Rust capability microkernel).

## BUGS

- Steps 5 and 6 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team,
  and on milestone 610's per-package co-change column.
- The TRNG needs its clocks and reset released first, which is `jh7110_clock_and_reset`'s plan
  (milestone 819 (JH7110 clock and reset logic, proven, then released on its own)). A stranger using
  this crate alone must do that bring-up some other way; the crate's documentation says so before it
  is published.
- The device-tree fixtures (`tests/fixtures/*.dts`) are excerpts of the mainline and vendor JH7110
  trees. Their licenses were not checked here; record each before `cargo package` ships them.
- `needs_person` is `yes` because the exit includes a publication only calef can approve. Everything
  before step 7 can start now.

## Index row

nife's StarFive JH7110 true random number generator logic, with four Kani proofs and their
falsification patches, cut free of nife's device tree. It is published on crates.io as
`jh7110_entropy` 0.1 from its own `nifeos` repository, and nife consumes that release.
