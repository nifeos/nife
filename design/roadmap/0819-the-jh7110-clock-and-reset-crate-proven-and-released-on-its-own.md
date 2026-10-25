---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 819. `jh7110_clock_and_reset`: JH7110 clock and reset logic, proven, then released on its own

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
- Fork 4: *"the release workflow refuses to publish unless the proof job is green."* His word:
  *"Yes"*. The other release rules are forks 1 to 3 and 6, applied in *Release* below.
- The cut, 2026-10-25 (UTC), choosing option A of the lane crates-stand-alone: *"A seems like the
  obvious choice"*. The crates take plain values, every device-tree walk moves to nife's side, and
  nife's process layout leaves the device crates. He accepted the refinement with *"Yes"*: what
  each of the JH7110's two trees calls a block stays in the crate as data and small decoders, and
  only the walk moves.

## What exists, checked 2026-10-07 (UTC)

- `crates/jh7110_clock_and_reset`: 863 code lines (1,577 with comments), written by milestone 220
  (this kernel drives no clock or reset controller, and the first real device will need one), which
  is BUILT. It holds the JH7110 clock and reset generator's register offsets and bit fields, the
  bring-up plans the TRNG, the SD slots and the PMIC's I2C bus need, and the device-tree queries
  that find the controllers.
- **No Kani harnesses and no falsification patches.** #1806's note counts it with the JH7110 pair as
  publish ours, but fork 4 refuses a release without green proofs. So this block starts by writing
  them. The note's
  "publish with the DesignWare pair" also predates the one-milestone-per-crate ruling.
- Class B in #1806's note `notes/releasable-crates-2026-10-07.md`. **The cut** is codebase rule 2
  (a driver gets what it needs passed in), and the crate now depends on nothing. It holds each
  tree's spellings (`STG_WINDOW`, `AON_WINDOW`, `SYS_WINDOW`, `PMIC_COMPATIBLES`) and decoders over
  raw property values (`Found::from_regions`, `PmicBus::from_bus`, `DevicePlan::from_specifiers`,
  which take a phandle lookup from the caller). The walks (`discover`, `discover_aon`,
  `discover_sys`, `pmic_bus`) are `machine_discovery::jh7110::clock_and_reset`'s, with their
  fixtures, and `jh7110_watchdog` builds its own `DevicePlan`.
- Its consumers, all in `kernel/src/`: `drivers/jh7110_clock_and_reset.rs` (the volatile shell),
  `designware_ethernet.rs`, `user/designware_mobile_storage_service.rs`, `user/entropy_service.rs`,
  `reboot.rs`, `memory.rs`, `lib.rs`, `storage_bench.rs` and `network_bench.rs`, plus
  `system_tests/src/user/entropy_tests.rs`.
- The crate name is ratified. The decoders and the newly public spellings carry provisional
  `Name:` blocks (in [names.md](0819-the-jh7110-clock-and-reset-crate-proven-and-released-on-its-own/names.md)),
  and their names become permanent with the crate. `discover_aon` and `discover_sys` left the crate
  with the walk, so their provisional names no longer bind it.
- crates.io answers 404 for the name (checked 2026-10-07).

**Reuse:** the alternatives were surveyed in #1806's `other-implementations.md` appendix (looked up 2026-10-07),
and none is taken, because calef ruled this crate "publish ours". The only Rust JH7110 crates found,
weathered-steel's `jh71xx-pac` 0.11.1 and `jh71xx-hal` 0.7.2 (Codeberg, 2025-03), are register maps
under GPL-3.0-only, which nife cannot take under its license. No Rust crate for the clock and reset
logic was found. Linux's `clk-starfive-jh7110-*` drivers stay the reference and are read, never
copied. No upstream offer goes anywhere: calef ruled on #1806 that nife contributes only to
projects nife depends on.

## What is proven

Nothing yet. The crate has no Kani harnesses today, so writing all four is this milestone's scope,
not a step before it. Each row is a property this crate must prove before it publishes. The harness
cell names the Kani harness that proves it, or says `none yet`; every named harness carries a
falsification patch that makes it fail at its own assertion. Each patch sits at
`crates/jh7110_clock_and_reset/falsifications/<module.path>.<harness>.patch`, as §134 (a harness
carries a machine-replayable falsification record) and `script/falsifications` require.

| property | harness |
|---|---|
| `with_parent` changes only the mux field: for every word and every parent, every bit outside `CLOCK_MUX_MASK` is unchanged and an over-wide parent never spills into the enable bit | none yet |
| Every offset a domain answers is one of its own words: `Domain::clock_offset` and `Domain::reset_bit` return none past the domain's `clocks` or `resets` count, and otherwise a word-aligned offset inside its range | none yet |
| Every plan ungates its clocks before it releases its reset: fixed or built from caller-supplied specifiers, a plan holds only in-bounds ids, never more than `MAX_PMIC_BUS_STEPS`, with every `EnableClock` ahead of every `DeassertReset` | none yet |
| The status decoders (`is_deasserted`, `is_clock_enabled`, `clock_divider`) are total and match their definitions for every word | none yet |

Not proven, by design: that a plan's order is the one the silicon needs, which only a board shows,
and the device-tree queries, which the cut moves out of the crate.

## Release, by #1806's rulings

1. The name is settled before anything leaves (857's Fork 5 as calef amended it on 2026-10-11 UTC).
   `script/names jh7110_clock_and_reset` reports it ratified, crates.io answers 404 for it (or it is
   already ours), and `nifeos/jh7110_clock_and_reset` does not exist yet. `script/crate-standalone`
   checks the first.
2. Write the four proofs above with their falsification patches, then make the cut. `cargo tree -p
   jh7110_clock_and_reset` then lists no nife crate.
3. In the tree, `script/verify` proves the harnesses and `Cargo.toml` says `publish = false`.
4. The package is 0.1.0 with its own semver, states `rust-version` (#1806's note built it on stable
   1.94.1), and ships its harnesses and falsification patches. `cargo package --list` shows them.
5. Ready means proofs passing and the API stable. §235 (the OS is built and updated from packages,
   and the tree divides by what releases together) has milestone 610 (the interface's stability is
   measured weekly)'s per-package co-change column, and the soak (step 6) is how the second is
   measured. When the soak exits, calef adds it to the lint-checked publish allowlist. The
   `nifeos/jh7110_clock_and_reset` repository is created when the crate moves, on the shared CI
   template (milestone 887), through reusable workflows: Kani through `kani-github-action`, lint,
   release. Its release workflow refuses to publish unless the proof job is green. Nothing here
   reads `target_arch`, so stock Kani serves. The patched Kani of §218 (carry a Kani patch so
   riscv64 is proved, and send it upstream) is carried only if a harness comes to need riscv64.
   Milestone 589 (Kani can prove riscv64 from the hosts we already have) built it.
6. Soak, as its own repository (calef, 2026-10-17 UTC, replying "Agrred" (sic) to the maintainer's
   proposal; §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)).
   Milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) is the pilot and
   soaks first. Before publication the crate is moved, not copied: it leaves this tree, and nife
   depends on `nifeos/jh7110_clock_and_reset` by `rev`. A copy left behind does not count. The soak
   ends on evidence, not a duration, when all five hold:
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
host, all exit 0. The release workflow builds the same four. The silicon is riscv64 only; the crate
is not.

## Standalone, measured 2026-10-11 (UTC)

`script/crate-standalone jh7110_clock_and_reset` fails today, at base `ae6b91e79`. The manifest
inherits `license` and `lints` from nife's workspace and states no `rust-version`, and there is no
README, `LICENSE-MIT` or `LICENSE-APACHE`. Those hold for all nine crates. It does not build alone:
`device_tree_blob` is a path dependency outside the copy, so neither the build nor Kani ran, and it
has no harness to run. A test reads a fixture from outside the crate. Beyond manifest and files, 32
lines cite this tree: 22 `milestone N`, 4 tree paths, 4 provenance blocks and 2 naming calef.

`script/names` reports the name `jh7110_clock_and_reset` ratified (checked 2026-10-25). The
device-tree walks, whose names were provisional, left the crate on 2026-10-25.

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn handoff
  or nife error enum in a public signature. A reviewer judges this; no script can. Known today: the
  decoders take raw property bytes and a phandle lookup generic over the caller's error type, and
  their names are provisional.

## Exit

A check proves it. `script/verify` proves the four properties and `script/falsifications` reports
every harness `replayable`. In nife, `cargo tree --locked -i jh7110_clock_and_reset` resolves to
`v0.1.x` from crates.io with a checksum in `Cargo.lock`, `crates/jh7110_clock_and_reset` no longer
exists, and the tree builds for all three targets. In `nifeos/jh7110_clock_and_reset`, the run that
published 0.1.0 shows the Kani job green ahead of the publish job. The five soak criteria of release
step 6 hold before it publishes.

The name check of release step 1 passes first, so the repository, the package and every doc link
carry a ratified name. Before the move, `script/crate-standalone jh7110_clock_and_reset` exits 0 in
nife, and every row of *What is proven* names a harness that stock Kani proves in the copy and whose
falsification patch makes it fail. No crate publishes without Kani harnesses covering its
hostile-input and state-machine surface, each carrying a falsification patch that makes it fail at
its own assertion, replayable in the crate's own repository with stock Kani.

## What the crate no longer cites

The crate's source cites nothing of this tree, so this section keeps the links it carried. Its
`Name:` blocks are in [names.md](0819-the-jh7110-clock-and-reset-crate-proven-and-released-on-its-own/names.md).

- The crate doc: the volatile shell is `kernel/src/drivers/jh7110_clock_and_reset.rs`, and the
  story is `notes/jh7110-clock-and-reset.md`.
- `Step::SelectParent`, `AON`, `GMAC0_SYS_BRING_UP`, `GMAC0_AON_BRING_UP`, `SYSCLK_SDIO0_AHB`,
  `SDIO_BRING_UP` and `Report::resets` cite the JH7110's Ethernet and SD/MMC work, milestone 53
  (the board's own peripherals: network and storage on real silicon).
- `STG_BASE`: why a fallback address is safe is the note's section "The one genuinely dangerous
  thing here, and what stops it".
- `SYS`, `PMIC_BUS_BRING_UP`, `PmicBus`, `IcClkWords`, `COMPATIBLE_SYSCRG` and the direct reset
  write's banner: milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write, and the kernel
  sets that write up). Its option B is the direct write, and its 2026-10-10 bench put radon in its
  outcome table's fourth row.
- `SYSRST_I2C5_APB`: every source for the reset line is in milestone 592's block.
- `StandardMode` and `PMIC_RESET_REG`: the vendor U-Boot tree milestone 592 pinned, and its block's
  reading of the PMIC register.
- `src/tests.rs`: the tests left `lib.rs` on 2026-10-12 (UTC), by lane
  maintainer/near-ceiling-splits, to keep the root under §266 (a Rust source file stays under
  2,000 lines, held by a ratchet that only falls).

## BUGS

- Steps 5 and 6 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team,
  and on milestone 610's per-package co-change column.
- The four properties are this lane's proposal, read from the code, not a ruling. A lane writing
  them may find a better set and says so in its pull request.
- The device-tree fixtures are excerpts of the mainline and vendor JH7110 trees. They moved to
  `crates/machine_discovery/tests/fixtures/` with the walk, so the crate ships none; their licenses
  were not checked.
- `needs_person` is `yes` because the exit includes a publication only calef can approve. Everything
  before step 7 can start now.

## Index row

nife's StarFive JH7110 clock and reset generator logic, which carries no proofs today, gains four
Kani proofs with falsification patches, is cut free of nife's device tree, and is published on
crates.io as `jh7110_clock_and_reset` 0.1 from its own `nifeos` repository.
