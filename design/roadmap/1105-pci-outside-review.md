---
status: NOT-STARTED
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
enumeration leaves the kernel) says the crate "moves with the server"; the decision below asks
whether it leaves first.

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
(unverified), and the two `include_bytes!` reach into `crates/device_tree_blob/tests/fixtures/` and
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
`patch -p1 -F0` accepts it. The other seven regenerate cleanly, so the cause is in the
regeneration for a hunk that ends on a blank line (unverified). The `Name:` blocks, patch paths
and record positions belong to the move script and are not items here.
Fix: a manifest with `license = "MIT OR Apache-2.0"`, `rust-version = "1.88"` (the crate uses
`as_chunks`, stable since 1.88, and edition 2024), `repository`, the two license files, and no
workspace inheritance. Write a README in the template's section order (EXAMPLES, "What is proven",
Building and testing, BUGS, Minimum Rust version, License) from the two existing documentation
tests. Replace every tree reference in a doc comment with the specification it stands for. Cite the
PCI Local Bus Specification 3.0 for the header, capability list, MSI and MSI-X; PCI Express Base for ECAM;
virtio 1.2 section 4.1.4 for the vendor capabilities; the device tree specification for `ranges`.
Remove the four lines that name the architect. Before the repository's first CI run, fix
`script/crate-move`'s patch regeneration, or hand-write a patch for that one harness.
Effort: M. Door: two-way.

### P1

The P1 items are in [the appendix](1105-pci-outside-review/before-the-tag.md).

### P2: later

The P2 items are in [the appendix](1105-pci-outside-review/later-items.md).

## Decisions for the architect

No one-way door is open. Everything else above the maintainer rules as provisional.

### The package name, because `pci` is taken on crates.io

In force since 2026-11-14 (UTC): the package name is `pci_configuration_space`, its directory is
`crates/pci_configuration_space`, and the repository, when it exists, carries the same name. Another
author owns `pci` on crates.io with a 0.0.1 release, nine years dormant. The short name `pci` stays
ratified under §154 (the acronym test is whether the phrase is spoken), and the `Name:` note in
`crates/pci_configuration_space/src/lib.rs` states both. The noun names what the crate decodes, it
passes every naming gate, and nobody else holds it.

Consumers import the crate under its package name, as the tree did for `glob` becoming `byte_glob`.
Every manifest names `pci_configuration_space` with no `package =` rename, and every path reads
`pci_configuration_space::`. The short name `pci` is no longer an import.

### Not a decision here, but held for the architect's awareness

Milestone 1064 says the crate "moves with the server", and the server waits on milestones 317,
1062 and 1068. The crate has no dependency on any of them, the kernel and `driver_process_contract`
both link it today, and a bus server will link it the same way. Moving it first is the shape the
contract order already uses for every leaf, and reversing it costs one copy back. The maintainer
rules it; this review recommends moving first, after items 1 through 8.

## Done when

The checks are in [the appendix](1105-pci-outside-review/done-when.md).

## BUGS

- Item 3's size claim comes from reading the arithmetic, not from a device that answers a
  non-contiguous mask; the fix is cheap enough to land with its proof rather than after a
  reproduction.
- The cause of the one corrupt regenerated patch (item 2) is in `script/crate-move` or in the
  patch's shape, and this review did not find which. Seven of eight regenerate cleanly.
- The review ran the template's name gates from the template's head; the release the move pins
  (`v7` today) runs fewer of them, so some of item 5 becomes red only at a later template bump.
- No review package exists for this crate. This block reads as if one did; a package lane
  may still help so the index at `notes/crate-reviews.md` has a row.
- The crates.io figures are a point reading and will drift.

## Index row

An outside review of `pci` before it leaves. It finds two blockers (the fixture tests that reach
into nife, and the staged tree's red gates). It finds six items to land before the tag (BAR sizing
that trusts the mask, BAR probing on any header, two name lists, documentation for a stranger, a
missing MSI proof), and seven for later. It asks one decision: the package name, because another author holds `pci` on crates.io.
