---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 820. `extensible_host_controller_interface`: proven xHCI logic, released on its own

*(From calef's rulings on pull request #1806; the title and slug are drafts. The FAT crate's block,
milestone 813 (a proven FAT crate released on its own), is the shape this follows.)*

calef's rulings, quoted from the maintainer comments on #1806 of 2026-10-07 (UTC):

- Policy: *"Each publish-ours crate (NVMe, e1000e, the DesignWare pair, the JH7110 pair, xHCI, and
  `portable_executable` once it carries proofs) is published when ready (proofs passing, API
  stable), each through its own milestone at its review."* His words: *"Since the consumer would be
  other hobby OSes, I think it makes sense to publish them. [...] a nice legacy is a bunch of hobby
  oses that use its parts."*
- Crates: *"one milestone per crate, minted now, each keeping its tree name (all free on crates.io
  as of today): [...] `extensible_host_controller_interface` (no offer to rust-osdev's dormant
  `xhci`, per today's only-our-dependencies rule) [...] Names are ratified now and permanent on
  first publication."* His words: *"Yes, one per crate"*.
- That rule: *"nife contributes upstream only to projects nife depends on [...] For projects nife
  does not use, nife makes no offer and maintains no proofs there; our proofs stay public under our
  license for anyone to take."* His word: *"Yes"*.
- The release rules are forks 1 to 4 and 6 of the same pull request, applied in *Release* below.

## What exists, checked 2026-10-07 (UTC)

- `crates/extensible_host_controller_interface`: 1,191 code lines (1,817 with comments), written by
  milestone 242 (USB host and HID, because on commodity hardware the keyboard is not a UART), which
  is PARTIAL. It is everything a USB host controller driver computes and nothing it touches:
  capability and operational registers, device and endpoint contexts, the rings, port status and
  HID boot reports. Class A in #1806's note `notes/releasable-crates-2026-10-07.md`, so it depends
  on no other nife crate and needs no cut.
- Three Kani harnesses, each with a falsification patch: the register window never maps a withheld
  page, a producer ring stays in bounds, and the handoff round-trips.
- Its consumers: `kernel/src/extensible_host_controller_interface.rs` (finds and confines the
  controller), `kernel/src/user.rs`, `kernel/src/user/usb_keyboard_service.rs` and
  `components/src/usb_keyboard_driver.rs` (the EL0 driver).
- The `Handoff` types encode nife's spawn ABI rather than xHCI. Whether they stay in the published
  crate or move to the driver is the lane's call, recorded in its pull request.
- crates.io answers 404 for the name (checked 2026-10-07).

**Reuse:** the alternatives were surveyed in #1806's `other-implementations.md` appendix (looked up 2026-10-07),
and none is taken. rust-osdev's `xhci` 0.9.2 (2023-07, MIT OR Apache-2.0, 108k downloads, no proofs)
has the right scope and has been dormant since 2024-09. The first proposal on #1806 was to ask
rust-osdev to revive it, and calef ruled no offer, since nife does not depend on it. rcore-os's
`crab-usb` 0.12.1 (Apache-2.0, 28k) lives in the `tgoskits` monorepo with no proofs found. Redox's
`xhcid` is a daemon whose GitHub mirror is archived. The xHCI 1.2 specification is the reference.

## What is proven

Each row is a property this crate must prove before it publishes. The harness cell names the Kani
harness that proves it, or says `none yet`; every named harness carries a falsification patch that
makes it fail at its own assertion. Each patch is at
`crates/extensible_host_controller_interface/falsifications/`. Measured 2026-10-11 (UTC): 3 of 5
proven.

| property | harness |
|---|---|
| The register window never maps a withheld page | `verification::the_window_never_maps_a_withheld_page` |
| A producer ring stays in bounds | `verification::a_producer_ring_stays_in_bounds` |
| The handoff round-trips | `verification::the_handoff_round_trips` |
| The event ring stays in bounds and flips its cycle bit only at the wrap (`EventRing::advance`) | none yet |
| The extended-capability walk (`regs::find_extended`) ends within its hop bound and inside `limit` for any register contents | none yet |

The two new rows are this lane's reading of the hostile surface: the controller writes the event
ring and the capability list. Not proven: event decoding (`Event::decode`), the context builders
and port decoders (`context`, `port`), and the report words (`report`).

## Release, by #1806's rulings

1. The name is settled before anything leaves (857's Fork 5 as calef amended it on 2026-10-11 UTC).
   `script/names extensible_host_controller_interface` reports it ratified, crates.io answers 404
   for it (or it is already ours), and `nifeos/extensible_host_controller_interface` does not exist
   yet. `script/crate-standalone` checks the first.
2. In the tree, `script/verify` proves the harnesses and `Cargo.toml` says `publish = false`.
3. The package is 0.1.0 with its own semver, states `rust-version` (#1806's note built it on stable
   1.94.1), and ships its harnesses and falsification patches. `cargo package --list` shows them.
4. Ready means proofs passing and the API stable. §235 (the OS is built and updated from packages,
   and the tree divides by what releases together) has milestone 610 (the interface's stability is
   measured weekly)'s per-package co-change column, and the soak (step 5) is how the second is
   measured. When the soak exits, calef adds it to the lint-checked publish allowlist. The
   `nifeos/extensible_host_controller_interface` repository is created when the crate moves, on the
   shared CI template (milestone 887), through reusable workflows: Kani through
   `kani-github-action`, lint, release. Its release workflow refuses to publish unless the proof job
   is green. No harness here reads `target_arch`, so stock Kani proves them. The patched Kani of
   §218 (carry a Kani patch so riscv64 is proved, and send it upstream) is carried only if one comes
   to need riscv64. Milestone 589 (Kani can prove riscv64 from the hosts we already have) built it.
5. Soak, as its own repository (calef, 2026-10-17 UTC, replying "Agrred" (sic) to the maintainer's
   proposal; §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)).
   Milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) is the pilot and
   soaks first. Before publication the crate is moved, not copied: it leaves this tree, and nife
   depends on `nifeos/extensible_host_controller_interface` by `rev`. A copy left behind does not
   count. The soak ends on evidence, not a duration, when all five hold:
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

`script/crate-standalone extensible_host_controller_interface` fails today, at base `ae6b91e79`. The
manifest inherits `license` and `lints` from nife's workspace and states no `rust-version`, and
there is no README, `LICENSE-MIT` or `LICENSE-APACHE`. Those hold for all nine crates. Builds and
tests on stable for the host and the three bare-metal targets. Stock Kani proves all three harnesses
in the copy, and each falsification patch, applied with `-p3`, turns its harness red. It fails on
manifest and files only, plus 12 lines citing this tree: 5 `milestone N`, 3 `§N`, 2 naming calef, 1
tree path and 1 provenance block.

Name: `extensible_host_controller_interface` is ratified today (`script/names`, checked 2026-10-11).

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn
  handoff or nife error enum in a public signature. A reviewer judges this; no script can.
  Known today: `Handoff` and the `report` words are nife's spawn ABI and spawn report, not xHCI.

## Exit

A check proves it. In nife, `cargo tree --locked -i extensible_host_controller_interface` resolves
to `v0.1.x` from crates.io with a checksum in `Cargo.lock`,
`crates/extensible_host_controller_interface` no longer exists, and the tree builds for all three
targets. In `nifeos/extensible_host_controller_interface`, the run that published 0.1.0 shows the
Kani job green ahead of the publish job, and every harness's falsification patch makes its proof
fail. The five soak criteria of release step 5 hold before it publishes.

The name check of release step 1 passes first, so the repository, the package and every doc link
carry a ratified name. Before the move, `script/crate-standalone
extensible_host_controller_interface` exits 0 in nife, and every row of *What is proven* names a
harness that stock Kani proves in the copy and whose falsification patch makes it fail. No crate
publishes without Kani harnesses covering its hostile-input and state-machine surface, each carrying
a falsification patch that makes it fail at its own assertion, replayable in the crate's own
repository with stock Kani.

## BUGS

- Steps 4 and 5 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team,
  and on milestone 610's per-package co-change column.
- Milestone 242 is PARTIAL, and the crate's own `BUGS` says a device behind a hub cannot be
  addressed. A stranger meets that limit first, so the published README states it.
- Three harnesses cover the window, one ring and the handoff. *What is proven* now requires two
  more, on the event ring and the capability walk; whether the context and port decoders want
  proofs before 0.1 is still the lane's question, answered in its pull request.
- `needs_person` is `yes` because the exit includes a publication only calef can approve. Everything
  before step 6 can start now.

## Index row

nife's xHCI host controller logic, three Kani proofs and their falsification patches, published on
crates.io as `extensible_host_controller_interface` 0.1 from its own `nifeos` repository instead
of being offered to rust-osdev's dormant `xhci`, then consumed back by nife from that release.
