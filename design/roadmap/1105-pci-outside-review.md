---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1034
decision_dependencies: 46, 154, 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1105. `pci`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/pci_configuration_space` as a stranger to this tree would meet it, on a fresh
shallow clone of `main` and a checkout of `nifeos/crate-workflows`. It changes nothing; it lists
what the next lanes change, and the one decision the architect must take before a repository
exists.

The review found no review package for this crate under `notes/crate-reviews/`, unlike every other
crate that has moved or been cleared. Its accepted baseline is therefore what the tree records at
the names themselves: the crate name `pci` (ratified under §154 (the acronym test is whether the
phrase is spoken)), `intx_interrupt_number` (ratified), and `virtio` as a proper name. Everything
else in the public surface carries a `provisional` block, and this review treats those names as
open. It does not reopen the two ratified names, the closure-over-`(bdf, offset)` shape that §18
(the PCIe transport) chose over `pci_types`, or the pure-logic split that leaves every volatile
access in the kernel.

Milestone 1034 (a runbook for moving a crate out of nife) owns the move itself. Milestone 1064 (PCI
enumeration leaves the kernel) says the crate "moves with the server"; the crate leaves first, as
the held item below records.

Reuse: `script/crate-move`, `script/crate-standalone`, the template `nifeos/crate-workflows` and
its name gates, Kani, cargo-mutants and the crate's own fake configuration spaces (`FakeCfg`,
`BridgedCfg`). No new tool.

## Goal

A stranger who finds this crate can tell in one screen what it decodes and what it refuses to
decode. The stranger can build, test, prove and falsify it from the README alone. The stranger can
also trust that a function answering every configuration read with garbage leaves the caller with a
bounded, well-formed answer or `None`, never with a size or an offset the caller must second-guess. Every public name
says what it is in words the naming rules accept, and the template's gates are green for the
crate's own reasons.

## Finding status

- P0-1, fixture tests that reach into nife: Done, [#3807](https://github.com/nifeos/nife/pull/3807).
  The five witness tests moved to `crates/machine_discovery/tests/pci_host_bridge_windows.rs`, the
  crate inlines the swizzle cells, and the dev-dependency is gone.
- P0-2, the staged tree's red gates: Done, [#3807](https://github.com/nifeos/nife/pull/3807)
  ([#3521](https://github.com/nifeos/nife/pull/3521) fixed the patch regeneration earlier). The
  manifest states its license, lints and `rust-version` as literals, the README and both license
  files exist, doc comments cite specifications, and all ten patches regenerate against the new source and apply.
- P1-3, BAR size trusts the mask: Done, [#3519](https://github.com/nifeos/nife/pull/3519).
- P1-4, BAR probing on any header and past slot 5: Done,
  [#3519](https://github.com/nifeos/nife/pull/3519).
- P1-5, names the template's gates refuse: Done, [#3807](https://github.com/nifeos/nife/pull/3807).
  The crate, the kernel, `driver_process_contract`, `jh7110_pci_express`
  and `machine_discovery` carry the new spellings. Every one is provisional, and the review
  package asks the architect to ratify them.
- P1-6, names the naming rules refuse: Done, [#3807](https://github.com/nifeos/nife/pull/3807). The
  same renames and the same provisional status as P1-5.
- P1-7, documentation that assumes the tree: Done, [#3807](https://github.com/nifeos/nife/pull/3807).
  Every public item has a doc comment that cites a specification, and `allow(missing_docs)` is gone.
- P1-8, the MSI walk and the ceiling: Done, [#3807](https://github.com/nifeos/nife/pull/3807). The
  three capability walks are one shared walk with one bound. A Kani harness of `walk` on a two-bus
  machine ran over twelve CPU-minutes without a result and is not kept; the scan loops stay bounded
  by literals and the bus queue, the part a device controls, stays proved.
- P2, `virtio_capabilities` ignores `cap_len`: Done,
  [#3567](https://github.com/nifeos/nife/pull/3567).
- P2, `intx_interrupt_number` precondition: Open. The doc comment states no bus-0 precondition,
  `intx_pin_through_bridge` does not exist, and `kernel/src/pci.rs` has no BUGS entry for it.
- P2, `Bdf` fields unmasked: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2, `#![forbid(unsafe_code)]`: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2, fuzz targets: Open. No target covers `mem32_window` or the walks; the opt-in waits for the
  repository.
- P2, stale patch headers: Done, [#3807](https://github.com/nifeos/nife/pull/3807). All ten patches
  regenerate against the renamed source.
- P2, the proof table: Done, [#3807](https://github.com/nifeos/nife/pull/3807).
  `notes/verification/parser-harnesses.md` lists all ten harnesses.
- P2, mutation survivors: Open. The baseline row carries counts only; the first run in the
  repository exempts the 11 equivalent survivors.
- Decision, the package name: Done, [#3588](https://github.com/nifeos/nife/pull/3588) (ruled
  2026-11-14 UTC).
- Decision, the crate moves before the bus server: Done,
  [#3589](https://github.com/nifeos/nife/pull/3589) (maintainer, provisional, 2026-11-14 UTC;
  nothing to build).
- Review package: Done, [#3807](https://github.com/nifeos/nife/pull/3807):
  [`pci-configuration-space.md`](../../notes/crate-reviews/pci-configuration-space.md) and its row.
- Review package clearance: Done (2026-11-20 UTC). The architect cleared the package and approved
  `nifeos/pci_configuration_space` with `publish = false`, no tag and no publish.
- Follow-on, creating `nifeos/pci_configuration_space`: Open. It waits on the virtio to `virtual_io`
  sweep (draft #3854), because the crate has `VirtioCapability` and virtio constants.

## Open decisions

### Clearing the review, which approves the repository

The crate decodes a PCI function's configuration space: its header, its base address registers,
its capability list and the host bridge window from the device tree. The kernel and
`driver_process_contract` link it today, and a bus server outside the kernel will link it later.
Moving it into its own repository, `nifeos/pci_configuration_space`, is what lets those three
consumers pin one released copy. Creating a public repository is an outward step, and the
standing rule is that clearing a crate's review is what approves it.

Every other crate that has moved or passed clearance had a review package under
`notes/crate-reviews/`: a short record of the names, the identity and the gates, written for the
clearance. This crate's package is
[`pci-configuration-space.md`](../../notes/crate-reviews/pci-configuration-space.md). The maintainer
took option A provisionally on 2026-11-19 (UTC), a two-way door the architect may revisit. The
architect cleared the package on 2026-11-20 (UTC), which approves the repository. The architect ruled the names on
2026-11-20 (UTC), the names decision taken as option B: the whole table ratified as built, with one change, the struct `Bdf`
becomes `BusDeviceFunction` and the specification spellings ride as aliases. The move waits on the
virtio to `virtual_io` sweep (draft #3854) and on the P2 move steps above.

Recommendation: A, taken provisionally.

- A. A lane writes the review package once P0-1, P0-2 and P1-5 to P1-8 land; clearing that
  package approves the repository. This is the option in force.
- B. Clear on this milestone alone, without a package, once the same items land.
- C. Keep the crate in nife until the bus server of milestone 1064 exists, and clear both
  together.

Door: one-way. Others can see, fork, pin or cite a public repository the moment it exists.
Renaming or deleting it later breaks whoever acted on it.

Prior art: the rust-osdev `pci_types` crate lives in its own repository apart from any kernel,
which is the shape A gives. Redox keeps PCI decoding inside its `pcid` daemon, which is the shape C
keeps. nife differs from both by keeping every volatile access in the kernel and only pure decoding
in the crate. In this tree, `designware_ethernet` and `e1000e` each moved after a cleared package.

Blocks: the move, and with it any tag and the first publish.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1105-pci-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1105-pci-outside-review/prior-art.md).

## The items

Each item: what is wrong, where, why it matters, the fix, effort (S under a day, M a few days, L
more), and the door. A name is two-way until the crate's first tagged release and one-way after.

### P0

**1. The fixture tests read two files outside the crate and link `device_tree_blob` by path, so the
crate cannot leave.**
Where: `crates/pci_configuration_space/Cargo.toml` lines 12 to 15; `crates/pci_configuration_space/tests/qemu_virt_dtb.rs` lines 17 to 18, 52 and 147 to 148.
Why: the rule a crate leaves under is that every crate it links has left or leaves with it, and no
file in it names a nife path. `device_tree_blob` has no move plan the review could find
(unverified), and the two `include_bytes!` reach into `crates/machine_discovery/tests/fixtures/` and
`crates/machine_discovery/tests/fixtures/`. The five tests are also kernel witnesses, not crate
tests: they pin `PCI_ECAM_BASE`, `PCI_BAR_BASE` and `PCI_INTERRUPT_NUMBER_BASE` (32 and 35) from
`kernel/src/arch/*/mmu.rs`, and the reads `memory::initialize` performs, none of which a stranger
to nife can read.
Fix: move the five tests to the kernel's host tests or beside `device_tree_blob`'s fixtures, where
the constants they witness live; drop the dev-dependency. The crate keeps what is its own: the
`mem32_window` tests over inlined cells (already in `src/tests.rs`) and a swizzle test over the
sixteen `interrupt-map` entries inlined as cells with their source stated in the comment. The
alternative, copying the two trees (5 KiB and 7.5 KiB) into the crate and pinning
`device_tree_blob` by `rev` once it has a repository, keeps the crate waiting on a move nobody has
scheduled.
Effort: S. Door: two-way.

**2. The template's gates are red on the staged tree for reasons `script/crate-move` does not
remove.**
Where: `crates/pci_configuration_space/Cargo.toml` (whole file), `crates/pci_configuration_space/src/lib.rs` lines 4, 11 to 13, 38, 51 to 53, 79, 146 to 148, 170, 514, 563 to 566, 638 and 655 to 656, and the regenerated
`falsifications/verification.a_fresh_bus_queue_holds_the_root_and_nothing_else.patch`.
Why: `check / standalone` fails on no README, no license files, `license.workspace`,
`lints.workspace`, no `rust-version`, `publish = false`, 5 section citations, 7 paths into nife and
14 milestone citations in doc comments. `proofs / falsify` fails because the regenerated patch for
`a_fresh_bus_queue_holds_the_root_and_nothing_else` is one context line short: its hunk header
says seven old lines and the body holds six, so `git apply --check` reports a corrupt patch while
`patch -p1 -F0` accepts it. The other seven regenerate cleanly. The `Name:` blocks, patch paths
and record positions belong to the move script and are not items here.
Fix: a manifest with `license = "MIT OR Apache-2.0"`, `rust-version = "1.88"` (the crate uses
`as_chunks`, stable since 1.88, and edition 2024), `repository`, the two license files, and no
workspace inheritance. Write a README in the template's section order (EXAMPLES, "What is proven",
Building and testing, BUGS, Minimum Rust version, License) from the two existing documentation
tests. Replace every tree reference in a doc comment with the specification it stands for. Cite the
PCI Local Bus Specification 3.0 for the header, capability list, MSI and MSI-X; PCI Express Base for ECAM;
virtio 1.2 section 4.1.4 for the vendor capabilities; the device tree specification for `ranges`.
Remove the four lines that name the architect. `script/crate-move` keeps a hunk's trailing blank
context line and checks each regenerated patch with `git apply --check`, so a fresh dry run shows
whether every patch applies.
Effort: M. Door: two-way.

### P1

The P1 items are in [the appendix](1105-pci-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1105-pci-outside-review/later-items.md).

## Decisions for the architect

Both decisions here have rulings, and each states the rule in force. Clearing the review is under
"Open decisions" above; everything else the maintainer rules as provisional.

### The package name, because another author holds `pci` on crates.io

In force since 2026-11-14 (UTC): the package name is `pci_configuration_space`, its directory is
`crates/pci_configuration_space`, and the repository, when it exists, carries the same name. Another
author owns `pci` on crates.io with a 0.0.1 release, nine years dormant. The short name `pci` stays
ratified, and the `Name:` note in
`crates/pci_configuration_space/src/lib.rs` states both. The noun names what the crate decodes, it
passes every naming gate, and nobody else holds it.

Consumers import the crate under its package name, as the tree did for `glob` becoming `byte_glob`.
Every manifest names `pci_configuration_space` with no `package =` rename, and every path reads
`pci_configuration_space::`. The short name `pci` is no longer an import.

### When the crate moves: before the bus server

Milestone 1064 says the crate "moves with the server", and the server waits on milestones 317,
1062 and 1068. The crate has no dependency on any of them, the kernel and `driver_process_contract`
both link it today, and a bus server will link it the same way. Moving it first is the shape the
contract order already uses for every leaf, and reversing it costs one copy back. This review
recommends moving first, after items 1 through 8.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may
revisit. The crate moves first, after items 1 through 8, as every leaf of milestone 1017 (the order the contract crates leave in) does, and
the bus server links it from its repository when it comes. The crate's manifest has no
`[dependencies]` table, only a test dependency on `device_tree_blob`, so it waits on none of
milestones 317, 1062 and 1068. The move still waits on the package name above.

## Done when

The checks are in [the appendix](1105-pci-outside-review/done-when.md).

## Follow-on

- **Outstanding.** Every finding marked Open under "Finding status" above (the `intx_interrupt_number`
  precondition, the fuzz targets and the mutation survivors), each with what remains, checked
  against the crate's manifest, `src/lib.rs`, `tests/` and `falsifications/`.
- **Outstanding.** The architect clears the review package, whose names are ratified, which
  approves the repository: the decision under "Open decisions" above.

## BUGS

- Item 3's size claim comes from reading the arithmetic, not from a device that answers a
  non-contiguous mask; the fix is cheap enough to land with its proof rather than after a
  reproduction.
- The review ran the template's name gates from the template's head; the release the move pins
  (`v7` today) runs fewer of them, so some of item 5 becomes red only at a later template bump.
- The crates.io figures are a point reading and will drift.

## Index row

An outside review of `pci` before it leaves. It finds two blockers (the fixture tests that reach
into nife, and the staged tree's red gates). It finds six items to land before the tag (BAR sizing
that trusts the mask, BAR probing on any header, two name lists, documentation for a stranger, a
missing MSI proof), and seven for later. Both blockers and the six items have landed, with every
renamed name provisional, and the review package exists. The package name, because another author
holds `pci` on crates.io, has a ruling; clearing the review, which approves the repository, stays
open.
