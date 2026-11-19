---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 857
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1085. `device_tree_blob`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/device_tree_blob` as a stranger to this tree would meet
it, on the assumption that the crate leaves for its own repository under the shared template
(milestone 857 (crates released on their own), item 4). It changes nothing; it lists what the
lanes that move the crate change, and what the kernel should take from the list whether or not the
crate moves.

The crate has no review package under `notes/crate-reviews/`, so the baseline this review takes is
the names already ratified at the item (`device_tree_blob`, `DeviceTreeBlob`, `initial_ramdisk`),
the provisional name recorded there (`boot_arguments`), and the bounds notes/device-tree.md
records. A ruling on record has nife working toward Google's `dtoolkit` as its device-tree
dependency. The switch itself is a separate decision. Until that switch, this crate is what boots
every nife machine. So the correctness items below (P1-1 to P1-4) are worth landing in the kernel's
path whether or not the crate ever publishes.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its naming and standalone helpers,
cargo-mutants, Kani, cargo-fuzz and the crate's own `Blob` builder in `tests/hostile.rs`. No new
tool.

## Finding status

The ids are the review's own; P0 and P1 are in [the
appendix](1085-device-tree-blob-outside-review/before-the-tag.md), P2 in [the later
items](1085-device-tree-blob-outside-review/later-items.md), D1 to D5 under [Ruled decisions](#ruled-decisions), and D6 under [Open
decisions](#open-decisions).

- P0-1, the manifest and files do not stand alone: Open. The manifest still inherits the workspace's
  license and lints and reads `publish = false`; the README and both license files are missing. The
  move writes them, and the item lapses if nife takes `dtoolkit` first.
- P0-2, the documentation cites nife: Done, [#3612](https://github.com/nifeos/nife/pull/3612). Only
  `Name:` blocks cite it, and the move strips them.
- P0-3, 29 refused names: Done, [#3612](https://github.com/nifeos/nife/pull/3612). The spelled-out
  and British-spelling checks refuse none over `src/` and `tests/`.
- P1-1, `node_reg` does not stop at the first match: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-2, reserved memory matched at any depth: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-3, unchecked cursor arithmetic on 32-bit targets: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-4, the walk is not confined to the structure block: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-5, the fuzz target does not move with the crate: Open. The target is still in nife's `fuzz/`;
  the move stages it in the new repository with `public-crash-inputs: false`.
- P2-1 to P2-8, the error's shape, cells above two, `memory_regions` and a disabled node, the token
  loop, the proofs, the reservation overflow, `from_ptr`'s read and `from_bytes`'s trim: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-9, one example, and it does not run: Open. [#3612](https://github.com/nifeos/nife/pull/3612)
  added the running doctests; the README's `## EXAMPLES` waits for P0-1.
- P2-10 to P2-13, the depth counters, `MAXIMUM_DEPTH`, a trailing partial `reg` pair and private
  names: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-14, `cfg_attr(not(test), no_std)`: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
- D1 to D5 and the pointer handle name: Ruled 2026-11-19, under [Ruled decisions](#ruled-decisions).
- D6: Open. Built as recommended, awaiting ratification; it is a subsection under [Open
  decisions](#open-decisions).
- F-size, `Region::size` is a length with no unit: Open. `register_names` refuses it at `src/lib.rs`
  line 26. It is two-way while unreleased, and the maintainer rules the rename provisionally; the
  new name joins the set for ratification. The same check refuses four test names in
  `tests/hostile.rs`, among them `SIZE_DT_STRUCT_AT` and `address_cells`, and `prose_acronyms` flags
  UEFI, NUL, GIC, PL031, PLIC, PMIC and PMP. The move's gate pass clears these.
- F-dtoolkit, the switch to `dtoolkit`: Open, under [Open
  decisions](#take-dtoolkit-once-a-release-carries-the-fixes).
- F-package, the review package: Open. None exists under `notes/crate-reviews/`; it waits on the
  decision to clear the move.
- F-clearance, review clearance: Open, under [Open
  decisions](#clear-the-move-or-hold-it-for-dtoolkit).
- BUGS-239: Open. Milestone 239 (radon's device tree does not describe the TRNG) gives the depth
  bound as a `MAX_DEPTH` of 16; it is `MAXIMUM_DEPTH`, 64. A lane corrects that milestone.

## Ruled decisions

### Spell `prop` out as `property` (D1)

Ruled 2026-11-19 (UTC): option A. The lookups spell `property` out, with no alias for the old
`prop` spelling: `node_property`, `node_properties`, `node_property_compatible`,
`parent_property_compatible`, `node_property_inherited` and `pointer_handle_property` (below). The
six names are ratified as built. The naming rules spell out a word that needs a decoder, and the
Devicetree Specification says "property"; `prop` is libfdt's C spelling (`fdt_getprop`).

Door: one-way at publish. Prior art: the `fdt` crate and `dtoolkit` spell `property()` and
`properties()`; libfdt abbreviates. nife differs from libfdt only where C's short names were a
constraint. Example: `tree.node_property(b"chosen", b"stdout-path")`.

### Spell `phandle` out as the pointer handle

Ruled 2026-11-19 (UTC): the specification's expansion of `phandle` is "pointer handle", and the
lookup spells it out as `pointer_handle_property(pointer_handle, name)`. It carries
`#[doc(alias = "phandle")]`, a search aid for the specification's term and not an old nife
spelling. Its doc comment opens with the expansion and cites the Devicetree Specification. The ratification is the same as D1's, as built.

### Spell `reg` out as the register regions (D3)

Ruled 2026-11-19 (UTC): option B. The two lookups that decode a node's `reg` spell the term out as
`node_register_regions` and `node_register_regions_compatible`. Each carries `#[doc(alias =
"reg")]`, a search aid for the specification's term and not an old nife spelling, and its doc
comment opens with the expansion ("register regions") and cites the Devicetree Specification. The
old `node_reg` spellings have no alias. The rule matches `pointer_handle_property`.

Door: one-way at publish. Prior art: the `fdt` crate's `NodeProperty::reg()` keeps the
specification's spelling. nife spells the term out and carries the specification's term as a doc
alias. Example: a search for "reg" lands on `node_register_regions`, and
`tree.node_register_regions_compatible(b"arm,pl031", &mut regions)` reads the register regions of
a PL031.

### Add the unit to the total size limit (D4)

Ruled 2026-11-19 (UTC): option A as amended. The limit is `MAXIMUM_TOTAL_SIZE_BYTES`, with the words
separated and the unit last, which matches the existing `total_size()` method. It carries
`#[doc(alias = "MAX_FDT_SIZE")]` and `#[doc(alias = "totalsize")]`. The header field's own spelling,
`totalsize`, stays in doc comments, in backticks. A limit ends with its unit.

Door: one-way at publish. Prior art: Linux's `MAX_FDT_SIZE`, which lends its value; `dtoolkit` has
no ceiling. Example: `if total > MAXIMUM_TOTAL_SIZE_BYTES { return Err(Error::TooLarge(total)) }`.

### Name the reservation block `memory_reservations` (D5)

Ruled 2026-11-19 (UTC): option A. `memory_reservations`, the specification's term for the memory
reservation block, is ratified as built. `reserved_memory_regions` keeps its name for the
`/reserved-memory` node's children, and the old `reserved_regions` has no alias. Two lookups read
two sources, and the kernel calls both, so the names say which is which.

Door: one-way at publish. Prior art: the `fdt` crate's `memory_reservations()`, libfdt's
`fdt_get_mem_rsv`, and `dtoolkit`'s `reserved_memory()` for the node. Example:
`tree.memory_reservations(&mut out)` beside `tree.reserved_memory_regions(&mut out)`.

### Property is spelled out across the crate

The crate spells `property` out in every public item, parameter and doc comment. The one `prop`
left is the private `Blob::prop` test builder in `tests/hostile.rs`, which is not public.

Door: one-way at publish. Prior art: libfdt's `fdt_node_offset_by_phandle` and the `fdt` crate's
`find_phandle` keep the abbreviation; neither spells the specification's expansion. nife differs
because its naming rules spell out a word that needs a decoder, and the alias keeps the term
searchable. Example: `tree.pointer_handle_property(pointer_handle, b"#interrupt-cells")`.

### Name the needle parameter `compatible` (D2)

Ruled 2026-11-19 (UTC): option A. The three lookups that take a needle name the parameter
`compatible: &[u8]`, the property's own name in the Devicetree Specification, ratified as built.
Rustdoc shows parameters, and `compat` needs a decoder.

Door: two-way, since a parameter name is not part of the call. Prior art:
`fdt::Fdt::find_compatible(with: ...)` and `dtoolkit`'s `is_compatible(&self, compatible: &str)`.
Example: `tree.node_reg_compatible(b"arm,pl031", &mut regions)`.

## Open decisions

Code carries the remaining surface decision (D6) as recommended. It is two-way while the crate has
no release and one-way at its first publish. The `dtoolkit` path makes it likely the crate never
publishes, so D6 waits on the last two decisions here.

### Keep `from_ptr` as the crate's one `unsafe` function (D6)

`from_ptr` reads the header through a raw pointer, checks the magic, caps the size at 2 MiB and
builds the slice. Sixteen call sites outside the crate use it: nine in `kernel/`, four in
`uefi_loader/`, two in `machine_discovery` and one in a board-check tool. Because of it, the
template refuses the public fuzz opt-in (P1-5). Recommendation: A, because the one place that knows
how to trust a firmware pointer should be the parser.

- A. Ratify as built: `from_ptr` stays, and the weekly public fuzz opt-in stays off. The tree's own
  fuzz host keeps the target.
- B. Add a safe `declared_size(header: &[u8; 8])`, leave the slice to each caller, forbid `unsafe`
  in the crate and turn the public fuzz job on.

Door: one-way at publish, since an `unsafe` constructor is a published contract. Prior art:
`fdt::Fdt::from_ptr` and `dtoolkit::Fdt::from_raw` are `unsafe` constructors in the parser crate.
nife differs in nothing here. Example: `unsafe { DeviceTreeBlob::from_ptr(pointer) }?` stays one
line in the kernel. Blocks: the tag.

### Take `dtoolkit` once a release carries the fixes

Ruled 2026-11-11 (UTC): nife contributes five gaps to Google's `dtoolkit` and switches once a
release ships them; the switch is its own ruling. Upstream merged [google/dtoolkit
#108](https://github.com/google/dtoolkit/pull/108).
[#111](https://github.com/google/dtoolkit/pull/111) and
[#112](https://github.com/google/dtoolkit/pull/112) have changes requested; the answers went up on
2026-11-16 and 2026-11-17, and both wait on the upstream maintainer. Release 0.5.0 (2026-11-03)
carries neither. Until the switch, `device_tree_blob` boots every nife machine.

Recommendation: A, the restrictive option, which keeps the kernel on the parser it proves today.

- A. Wait for a release that carries #111 and #112, then rule the switch against the dependency bar.
- B. Switch to 0.5.0 now with the two fixes vendored as patches.
- C. Keep `device_tree_blob` and move it as its own crate.

Door: one-way, a third-party dependency on the boot path. Prior art: Linux, U-Boot and FreeBSD each
vendor libfdt (`scripts/dtc/libfdt`, `sys/contrib/libfdt`; recalled, unverified) rather than keep
their own parser. nife differs by holding the dependency to its bar of proofs and three
architectures. Example: under A, the kernel's `DeviceTreeBlob::from_ptr` becomes
`dtoolkit::Fdt::from_raw`, and P0-1, P1-5 and the P2-9 README lapse. Blocks: whether the move
happens at all.

### Clear the move, or hold it for `dtoolkit`

No review package exists, and clearance follows one. Recommendation: B, because every move item
lapses if the switch comes first, and a package written first would go to waste.

- A. Write the package now and clear the move after the ratification of D6.
- B. Hold the package and the move until the `dtoolkit` decision; write the package only if C wins
  there.

Door: two-way until the repository exists. Prior art: `file_allocation_table` and `e1000e` cleared
from a written package. Example: under B, F-package and P0-1 stay open with no lane. Blocks: the
move, F-package, P0-1, P1-5 and P2-9.

## Goal

A stranger who finds `device_tree_blob` can tell in one screen what it reads and what it refuses.
The same stranger can build, test, prove and falsify it from the README alone, meets the same answer
from every lookup for the same shape of tree, and finds no sentence that only a nife contributor can
follow. The new repository's required checks are red only for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1085-device-tree-blob-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1085-device-tree-blob-outside-review/prior-art.md).

## The items

Each item states what is wrong, where, why it matters, the fix, effort (S/M/L), and the door. A
name, layout or dependency that the move publishes is a one-way door; everything else is two-way.
P0 and P1 are in [the appendix](1085-device-tree-blob-outside-review/before-the-tag.md), and P2 is
in [the later items](1085-device-tree-blob-outside-review/later-items.md).

## Done when

The checks are in [the appendix](1085-device-tree-blob-outside-review/done-when.md).

## Follow-on

- **Outstanding.** The open lines under [Finding status](#finding-status): P0-1, P1-5, the README
  half of P2-9, F-size, F-package and BUGS-239, and the three open decisions.

## BUGS

- P1-3 came from reading, not from a build for a 32-bit target. The review host has none, and the fix is cheap enough to land without one.
- The mutation baseline's single missed mutant was not identified; a census inside its window
  names it.
- The Linux reserved-memory scan (`fdt_scan_reserved_mem`) was not fetched; P1-2's claim about it
  is from memory and marked unverified. The review fetched and read the memory scan and `of_fdt_device_is_available`.
- The review ran the template's helpers from a checkout of the template, not from a crate
  repository with its own ceilings. A repository created from the scaffold starts at a ceiling of
  zero, which is what the counts above assume.
- If the ruling to take `dtoolkit` lands before this crate moves, P0-1, P1-5 and the README half
  of P2-9 lapse.
- `abbreviations.py` reports `formatter`, `core::fmt`'s word.

## Index row

An outside review of `device_tree_blob` before it leaves. Three day-one gate failures come first
(the manifest, the nife-only prose, 29 refused names). Five items come before the tag. They are a
prefix lookup that contradicts its siblings, a reserved-memory match at any depth, unchecked cursor
arithmetic on 32-bit targets, a walk that is not confined to the structure block, and a fuzz target
that does not move. Fourteen smaller items follow, and eight decisions went to the architect; D1 to D5 are ruled and three remain open.
