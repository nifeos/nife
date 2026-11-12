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

Raised by a read-only review of `crates/pci` as a stranger to this tree would meet it, on a fresh
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
Where: `crates/pci/Cargo.toml` lines 12 to 15; `crates/pci/tests/qemu_virt_dtb.rs` lines 17 to 18, 52 and 147 to 148.
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
Where: `crates/pci/Cargo.toml` (whole file), `crates/pci/src/lib.rs` lines 4, 11 to 13, 38, 51 to 53, 79, 146 to 148, 170, 514, 563 to 566, 638 and 655 to 656, and the regenerated
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

**3. `read_bars` turns any writable-bits mask into a size, and the size is a power of two only when
the mask is contiguous.**
Where: `crates/pci/src/lib.rs` lines 401 to 408 and 411 to 419.
Why: the specification says the writable bits of a memory BAR are the high bits above the size,
and the crate computes `size = !mask + 1` on that assumption. A function that answers the probe
with a non-contiguous mask, or a 64-bit BAR whose upper dword reads back with no writable bit, gets
a `Bar` whose `size` is not a power of two and may be near `u64::MAX`. Every consumer then trusts
that size: `place_bars` aligns to it and checks the window against it, and the capability resolver
checks `offset + length <= bar.size` against it. The proof `read_bars_is_total_for_any_device`
proves no panic in the decoder, which is true, and says nothing about the shape of the answer.
Linux and OpenBSD both take the lowest set bit of the masked value as the size, which tolerates a
non-contiguous mask; the stricter answer is to refuse one.
Fix: compute the size from the lowest set bit of the mask (both BSDs' `PCI_MAPREG_MEM_SIZE` shape).
Return `None` for a 64-bit BAR whose upper mask is not all-ones above the size (the 64-bit type says
the upper dword is address, so a hard-wired zero there is a malformed function). Prefer refusing a
non-contiguous mask over tolerating it, since loosening later is compatible and tightening is not. Add a proof `a_decoded_bar_size_is_a_power_of_two_of_at_least_sixteen` with its falsification
patch, and unit tests for a non-contiguous mask and for an upper half that reads zero.
Effort: S. Door: two-way.

**4. `read_bars` probes six slots on any header, and a 64-bit type in the sixth slot probes a
seventh.**
Where: `crates/pci/src/lib.rs` lines 373 to 425 (the loop), `:394-399` (the upper half at `off + 4`).
Why: on a type-1 header, slots 2 through 5 are the bus numbers, the I/O base and limit and the
memory base and limit. The write-ones probe rewrites them and restores them, which is a transient
misrouting of everything behind the bridge. The doc comment says "a type-0 header" and every caller
in the tree happens to pass an endpoint found by vendor id or class, but nothing in the signature or
the code says so, and `Function.header_type` is available to say it. Separately, a 64-bit type bit
in slot 5 makes the probe read and write offset 0x28, the CardBus CIS pointer, one past the BAR
slots. The test `a_64_bit_bar_in_the_first_slot_keeps_its_high_half` asserts the probe never touches
it, but only for a fixture whose slot 5 is not 64-bit. `pci_types` avoids both by typing
the endpoint header separately.
Fix: take the header type (or the `Function`) and return `[None; 6]` for anything but a type-0
header, documented as the precondition it is. Treat a 64-bit type in slot 5 as malformed and return
`None` for it without probing 0x28. Extend the proof in item 3 with "no read or write beyond `BAR0
+ 20`" and a test that puts a 64-bit type in slot 5.
Effort: S. Door: two-way.

**5. Names the template's gates refuse at a ceiling of zero.**
Where and what, each a rename:
- `mem32_window` (`crates/pci/src/lib.rs` line 751): `mem` is a spelled-out word. Recommend
  `memory_window_32_bit`, or `non_prefetchable_memory_window`, since the function also refuses the
  prefetchable entry.
- `CLASS_NVME` (`:74`): `nvme` is a spelled-out word. Recommend
  `CLASS_NON_VOLATILE_MEMORY_EXPRESS`, matching the kernel module and the crate of that name.
- `Bar.size` (`:361`), `VirtioCapability.length` (`:454`), `MsixCapability.table_size` (`:528`): a
  size carries its unit and a count names the things. Recommend `size_bytes`, `length_bytes` and
  `table_entries`. `length` mirrors `struct virtio_pci_cap`'s field; keeping it needs the kept form
  (the source named in the first line of its doc comment, plus `#[doc(alias)]`), which it lacks.
- `MsiCapability::address_low` (`:583`), `MsiTarget.address` (`:660`), the locals `pci_addr` and
  `cpu_addr` (`:768-769`): an address is virtual or physical, or it carries a qualifier from the
  list. The MSI value is the specification's "Message Address"; recommend the kept form on the
  field and the method (`message_address_low`), and `pci_address`, `cpu_address` for the locals.
- The test `the_mem32_entry_is_found_among_its_neighbours` (`src/tests.rs` line 624): a British
  spelling; `neighbors`. The test `an_empty_slot_costs_one_config_read` (`:390`): `configuration`.
  The file `tests/qemu_virt_dtb.rs` and its `dtb` bindings: `device_tree_blob`, moot after item 1.
Why: `check / clippy`'s naming step at the template's head runs these checks with no baseline, so
the first pull request in the new repository is red until they land, and a rename after the first
tag is a minor bump.
Fix: one rename lane across the crate, `kernel/src/pci.rs`, `kernel/src/memory.rs` and
`crates/driver_process_contract`, with the architect's ratification of each spelling.
Effort: M. Door: two-way until the first tag.

**6. Names outside the gates that the naming rules still rule against.**
Where and what, with the recommendation:
- `VIRTIO_BLK_MODERN`, `VIRTIO_BLK_TRANSITIONAL` (`:95, :98`): the acronyms appendix names `blk`
  as a word that fails the spoken test. `VIRTIO_BLOCK_MODERN`, `VIRTIO_BLOCK_TRANSITIONAL`, which
  also matches `VIRTIO_TYPE_BLOCK` four lines below.
- `VIRTIO_RNG_MODERN`, `VIRTIO_RNG_TRANSITIONAL` (`:125-126`) beside `VIRTIO_TYPE_ENTROPY`
  (`:134`): one device, two spellings. `VIRTIO_ENTROPY_MODERN`, `VIRTIO_ENTROPY_TRANSITIONAL`;
  the virtio specification's own name for device type 4 is the entropy device.
- `VIRTIO_NET_MODERN`, `VIRTIO_NET_TRANSITIONAL`, `VIRTIO_TYPE_NET` (`:103-104, :132`):
  `network`.
- `CMD_MEMORY_SPACE`, `CMD_BUS_MASTER` (`:83, :88`): `cmd` is an abbreviation with an expansion
  everyone says. `COMMAND_MEMORY_SPACE_ENABLE`, `COMMAND_BUS_MASTER_ENABLE`, the specification's
  bit names.
- `Bdf { bus, dev, func }` (`:22-26`): `dev` is in the abbreviations table (`device`), and
  `func` abbreviates `function`. `Bdf { bus, device, function }`, as `pci_types::PciAddress` and
  `virtio-drivers`' `DeviceFunction` both spell them. `Bdf` itself reads as a word aloud and stays,
  with `#[doc(alias = "bus device function")]`.
- `VirtioCapability.cfg_type` (`:451`): `cfg` abbreviates `configuration`. The field mirrors
  `struct virtio_pci_cap`; either `configuration_type` with the alias `cfg_type`, or the kept form.
  `notify_off_multiplier` (`:455`) is the same case and the same choice; `off` for `offset` has no
  owner but the specification.
- `CLASS_XHCI` (`:80`): the tree spells xHCI out everywhere else (`extensible_host_controller_interface`
  module and review package). `CLASS_EXTENSIBLE_HOST_CONTROLLER_INTERFACE`, or the kept form with
  the alias `xHCI`.
- `pba_bar`, `pba_offset` (`:534, :538`): "pending bit array" reads as words aloud. `pending_bit_array_bar`,
  `pending_bit_array_offset`.
- `VIRTIO_CAPABILITY_ISR` (`:437`): "interrupt status" reads as words aloud; `VIRTIO_CAPABILITY_INTERRUPT_STATUS`
  with the alias `ISR`.
- `Bar.is_64`, `MsiCapability.is_64` (`:363, :578`): `is_64_bit`; the bare number does not say what
  is 64.
- `VENDOR_ID`, `COMMAND`, `CLASS_REVISION`, `HEADER_TYPE`, `BAR0`, `CAPABILITIES_POINTER`,
  `INTERRUPT_PIN`, `BUS_NUMBERS`, `MESSAGE_CONTROL` (`:45-54, :68, :624`): register offsets end
  `_REGISTER` under the registers rule. `COMMAND` without the suffix is also the dword that holds
  Status, which the capability walks read it for.
- `Bar.base` (`:359`): it is the BAR's bus address, a physical address on both `virt` boards and
  under no IOMMU translation this crate knows of. `base_physical_address`, or the kept form naming
  the specification's "Base Address".
- Locals `orig`, `orig_hi` (`:382, :396`): `original`, `original_high`.
- `MsiCapability`, `MsixCapability`, `MSI_*`, `MSIX_*`, `msi_capability`, `msix_capability`,
  `MsiTarget`, `ecam_offset`, `intx_interrupt_number`: MSI, MSI-X, ECAM and INTx read as
  words aloud and stay, each with `#[doc(alias)]` for its expansion where the item lacks one.
Why: names are the architect's call before the first release, and the abbreviation report (83
components in `src/lib.rs`) lands in every pull request's job summary until the list shrinks.
Fix: the same rename lane as item 5, one ratification list.
Effort: M with item 5. Door: two-way until the first tag.

**7. The documentation assumes this tree, and 26 public items have none.**
Where: `crates/pci/src/lib.rs` line 18 (`#![allow(missing_docs)]` against a 24-item worklist), the
undocumented fields of `Bdf`, `BridgeBuses`, `Function` and `VirtioCapability`, and the constants at
`:45-54, :95, :104, :126, :133-136`. Doc comments at `:4, :38, :79, :131, :514, :566, :638,
:656, :711-713` cite `notes/pcie.md`, `kernel/src/iommu.rs`, `kernel/src/virtio.rs`,
`design/roadmap/0215-...`, `arch::x86_64::interrupt_controller::initialize_io_apic`,
`tests/qemu_virt_dtb.rs`, a bench machine, and milestones 16b, 29, 30, 56, 215, 242 and 320.
Why: a stranger cannot follow one of these references, and the template's `check / standalone`
counts them. The crate's two documentation tests are good; nothing else introduces the crate.
Fix: document every public item (what it is, which register and bit, which specification section).
Replace each tree reference with the specification or with a sentence that stands on its own. Drop
the `allow`. Write the README from item 2 so its EXAMPLES section is the
`enumerate` doctest and its "What is proven" section is the eight harness names in plain words.
Effort: M. Door: two-way.

**8. The MSI capability walk has no proof, and nothing proves the ceiling.**
Where: `crates/pci/src/lib.rs` lines 598 to 619 (`msi_capability`), `:256-303` (`walk`).
Why: `msix_capability` and `virtio_capabilities` each prove termination on any device;
`msi_capability` is the same loop written a third time, and neither harness covers it. A future
edit that drops its hop bound goes unnoticed. One fixture test holds the ceiling property
(`walk` never reads a bus at or above `ceiling`), and the doc comment argues it. A proof would need
the device loop unrolled 32 times per bus and may not finish (unverified), which is the kind of
limit the proofs module should state.
Fix: add `the_msi_walk_terminates_on_any_device` with a falsification patch; factor the three
capability walks over one private `capabilities(bdf, read32, &mut dyn FnMut(offset, head) -> bool)`
so one proof covers all three; try a ceiling proof at `ceiling <= 2` and record the result either
way in the module's doc comment.
Effort: S. Door: two-way.

### P2: later

The P2 items are in [the appendix](1105-pci-outside-review/later-items.md).

## Decisions for the architect

One one-way door is open. Everything else above the maintainer rules as provisional.

### The package name, because `pci` is taken on crates.io

Background. A crate on crates.io has one name, and `pci` belongs to a 0.0.1 release from another
author, nine years dormant, with 61 downloads in the last 90 days. crates.io transfers a name only
with its owner's consent, and the publishing hold means nothing publishes soon. But creating the repository
sets its name and the manifest's `package.name`, the naming rules say
a directory holding a package carries exactly the package's name, and every consumer's `use pci::`
line follows the package name. Changing it after `nifeos/pci` exists and the kernel pins it is a
rename across three manifests and two repositories.

Recommendation. Name the package `pci_configuration_space`, create the repository under that name,
and keep `pci` only as the kernel's local alias (`pci = { package = "pci_configuration_space", ... }`)
if you want the shorter call sites. The name is the noun for what the crate decodes, it passes
every gate (`configuration` is the spelled-out form), and it is free. The crate name `pci` stays
ratified for the module in this tree until the move; nothing in §154 reopens, since `pci` keeps
its spelling inside the longer name.

Alternatives. Ask the owner of `pci` for the name: a reversible step with an unknown answer and a
repository that waits to exist until the answer comes. Publish never, and keep `pci` as a git-only
dependency: this is the current state under the hold, and it leaves the repository name wrong the
day the hold lifts. `pci_decode`: a verb, which the crate rule refuses.

Prior art. `pci_types` and `pci-ids` both took qualified names; `pci_types`'s README gives no
reason, and whether that reason applied to the `pci` already taken remains unverified. Linux's and the BSDs' trees name the
layer `pci` because they own their namespaces.

Concrete example. `crates/driver_process_contract/Cargo.toml` would read
`pci_configuration_space = { version = "0.1.0", git = "https://github.com/nifeos/pci_configuration_space", rev = "<commit>" }`
and the import `use pci_configuration_space::{Bdf, virtio_capabilities}`, or with the alias,
`pci = { package = "pci_configuration_space", ... }` and the call sites unchanged.

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
