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

Raised by a read-only review of `device_tree_blob` as a stranger to this tree would meet
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

One line per finding, in the review's order. The P0 and P1 ids are in [the
appendix](1085-device-tree-blob-outside-review/before-the-tag.md), the P2 ids in [the later
items](1085-device-tree-blob-outside-review/later-items.md), and D1 to D6 under [Ruled
decisions](#ruled-decisions). The crate lives in `nifeos/device_tree_blob`.

- P0-1, the manifest and files do not stand alone: Done,
  [#3833](https://github.com/nifeos/nife/pull/3833). The manifest carries its license, lints and
  `rust-version` and reads `publish = false`; the repository has a README in the template's order
  and both license files.
- P0-2, the documentation cites nife: Done, [#3612](https://github.com/nifeos/nife/pull/3612) and
  [#3833](https://github.com/nifeos/nife/pull/3833). The prose says in the crate's own words what
  each citation stood for, and the move commit stripped the `Name:` blocks.
- P0-3, 29 refused names: Done, [#3612](https://github.com/nifeos/nife/pull/3612). The binding is
  `tree`, and the spelling is American.
- P1-1, `node_register_regions` does not stop at the first match: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-2, reserved memory matched at any depth: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-3, unchecked cursor arithmetic on 32-bit targets: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559). Every cursor step is a checked add.
- P1-4, the walk is not confined to the structure block: Done,
  [#3559](https://github.com/nifeos/nife/pull/3559).
- P1-5, the fuzz target does not move with the crate: Done,
  [#3833](https://github.com/nifeos/nife/pull/3833). The target and its corpus live in the crate's
  `fuzz/`, and its weekly caller keeps `public-crash-inputs: false`.
- P2-1, `Error::Truncated` names five faults: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612). `BadValue` and `BadStructure` exist, `Error`
  has `Display` and `core::error::Error`, and the enum is `#[non_exhaustive]`.
- P2-2, a cell count above two reads as its low 64 bits: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-3, `memory_regions` keys on the name and counts a disabled node: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-4, one token loop written nine times: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
  A private `Tokens` walk carries every walker.
- P2-5, the proofs cover the leaves only: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-6, the reservation block gets fewer checks than `reg`: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-7, `from_pointer` reads 40 bytes while its contract speaks of 8: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-8, `from_bytes` does not say what it trims: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-9, one example, and it does not run: Done, [#3612](https://github.com/nifeos/nife/pull/3612)
  and [#3833](https://github.com/nifeos/nife/pull/3833). The doctests run, and the README's `##
  EXAMPLES` shows the same code.
- P2-10, two depth counters: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-11, `MAXIMUM_DEPTH` is five local constants: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612). One public `MAXIMUM_DEPTH`, 64, bounds every
  lookup.
- P2-12, a trailing partial `reg` pair drops silently: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-13, private names that need a decoder: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
- P2-14, `cfg_attr(not(test), no_std)`: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
- D1, `prop` spelled out as `property`: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
- D2, the needle parameter is `compatible`: Done, [#3612](https://github.com/nifeos/nife/pull/3612).
- D3, `reg` spelled out as the register regions: Done,
  [#3783](https://github.com/nifeos/nife/pull/3783).
- D4, the total size limit carries its unit: Done,
  [#3783](https://github.com/nifeos/nife/pull/3783). The limit is `MAXIMUM_TOTAL_SIZE_BYTES`.
- D5, the reservation block is `memory_reservations`: Done,
  [#3612](https://github.com/nifeos/nife/pull/3612).
- D6, `from_pointer` stays the one `unsafe` function: Done,
  [#3783](https://github.com/nifeos/nife/pull/3783).
- The pointer handle, `phandle` spelled out: Done,
  [#3783](https://github.com/nifeos/nife/pull/3783). The lookup is `pointer_handle_property`.
- F-size, `Region::size` is a length with no unit: Done,
  [#3833](https://github.com/nifeos/nife/pull/3833). The field is `size_bytes`.
- F-dtoolkit, the switch to `dtoolkit`: Waiting on a `dtoolkit` release that carries
  [google/dtoolkit #111](https://github.com/google/dtoolkit/pull/111) and
  [#112](https://github.com/google/dtoolkit/pull/112), then the switch's own ruling, as
  [ruled](#take-dtoolkit-once-a-release-carries-the-fixes). Both are open upstream, and the latest
  release, 0.5.0, carries neither.
- F-package, the review package: Done, [#3797](https://github.com/nifeos/nife/pull/3797).
- F-clearance, review clearance: Done, [#3830](https://github.com/nifeos/nife/pull/3830), as
  [ruled](#clear-the-device_tree_blob-package-for-the-move).
- F-move, the repository: Done, [#3833](https://github.com/nifeos/nife/pull/3833). The repository is
  `nifeos/device_tree_blob`, with `publish = false`, no tag and no publication, and nife pins a
  commit of its `main`.
- BUGS-239, milestone 239 gives the depth bound as a `MAX_DEPTH` of 16: Done,
  [#3954](https://github.com/nifeos/nife/pull/3954). It names `MAXIMUM_DEPTH`, 64.

## Ruled decisions

### Spell `prop` out as `property` (D1)

Ruled 2026-11-19 (UTC): option A. The lookups spell `property` out, with no alias for the old
`prop` spelling: `node_property`, `node_properties`, `node_property_compatible`,
`parent_property_compatible`, `node_property_inherited` and `pointer_handle_property` (below). The
six names stand as built. The naming rules spell out a word that needs a decoder, and the
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
reservation block, stands as built. `reserved_memory_regions` keeps its name for the
`/reserved-memory` node's children, and the old `reserved_regions` has no alias. Two lookups read
two sources, and the kernel calls both, so the names say which is which.

Door: one-way at publish. Prior art: the `fdt` crate's `memory_reservations()`, libfdt's
`fdt_get_mem_rsv`, and `dtoolkit`'s `reserved_memory()` for the node. Example:
`tree.memory_reservations(&mut out)` beside `tree.reserved_memory_regions(&mut out)`.

### Keep `from_pointer` as the crate's one `unsafe` function (D6)

Ruled 2026-11-19 (UTC): option A. `DeviceTreeBlob::from_pointer` stays the crate's one `unsafe`
function, and the weekly public fuzz opt-in stays off. The tree's own fuzz host keeps the target.
The constructor spells `ptr` out and carries `#[doc(alias = "from_ptr")]`, a search aid
for the standard library's spelling and not an old nife spelling. It reads the header through a raw
pointer, checks the magic, caps the size at 2 MiB and builds the slice. Sixteen call sites outside
the crate use it: nine in `kernel/`, four in `uefi_loader/`, two in `machine_discovery` and one in
a board-check tool. Because of it, the template refuses the public fuzz opt-in (P1-5). The one
place that knows how to trust a firmware pointer is the parser.

Door: one-way at publish, since an `unsafe` constructor is a published contract. Prior art:
`fdt::Fdt::from_ptr` and `dtoolkit::Fdt::from_raw` are `unsafe` constructors in the parser crate.
nife differs in nothing here. Example: `unsafe { DeviceTreeBlob::from_pointer(pointer) }?` stays
one line in the kernel.

### Take `dtoolkit` once a release carries the fixes

Ruled 2026-11-19 (UTC): option A. nife waits for a `dtoolkit` release that carries
[google/dtoolkit #111](https://github.com/google/dtoolkit/pull/111) and
[#112](https://github.com/google/dtoolkit/pull/112), then rules the switch against the dependency
bar. Until then `device_tree_blob` boots every nife machine. An earlier ruling of 2026-11-11 (UTC)
has nife contribute five gaps to Google's `dtoolkit` and switch once a release ships them; the
switch is its own ruling. Upstream merged [#108](https://github.com/google/dtoolkit/pull/108). As
of 2026-11-19, #111 and #112 are open with changes requested; the answers went up on 2026-11-16
and 2026-11-17, and both wait on the upstream maintainer. Release 0.5.0 (2026-11-03), the latest,
carries neither.

Door: one-way, a third-party dependency on the boot path. Prior art: Linux, U-Boot and FreeBSD each
vendor libfdt (`scripts/dtc/libfdt`, `sys/contrib/libfdt`; recalled, unverified) rather than keep
their own parser. nife differs by holding the dependency to its bar of proofs and three
architectures. Example: after the switch, the kernel's `DeviceTreeBlob::from_pointer` becomes
`dtoolkit::Fdt::from_raw`, and P0-1, P1-5 and the P2-9 README lapse.

### Write the review package now; the move is not held for `dtoolkit`

Ruled 2026-11-19 (UTC): option A. A lane writes the `device_tree_blob` review package under
`notes/crate-reviews/` now. The move does not wait for the `dtoolkit` switch, which may take a long
time or never come. The architect gives the clearance, the go to create the repository, once the package
exists.

Door: two-way until the repository exists. Prior art: `file_allocation_table` and `e1000e` cleared
from a written package. Example: F-package and P0-1 each get a lane, and if the switch later comes
first, the move items lapse with no loss to the kernel, which keeps the fixes either way.

### Clear the `device_tree_blob` package for the move

Ruled 2026-11-20 (UTC): option A. The review package is cleared. The repository
`nifeos/device_tree_blob` is a go, with `publish = false`, no tag and no publish. The move does not
wait for the `dtoolkit` switch. The move lane runs the stranger test, the template's workflows and the
first weekly mutation run in the new repository.

Door: two-way until the repository exists. Example: the move lane creates the repository and lands P0-1,
P1-5 and the renames there, as the package lists them.

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

There are none.

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

- **Outstanding.** The one line under [Finding status](#finding-status) that is not Done: F-dtoolkit.

## BUGS

- P1-3 came from reading, not from a build for a 32-bit target. The review host has none, and the fix is cheap enough to land without one.
- The mutation baseline's single missed mutant was not identified; a census inside its window
  names it.
- The Linux reserved-memory scan (`fdt_scan_reserved_mem`) was not fetched; P1-2's claim about it
  is from memory and marked unverified. The review fetched and read the memory scan and `of_fdt_device_is_available`.
- The review ran the template's helpers from a checkout of the template, not from a crate
  repository with its own ceilings. A repository created from the scaffold starts at a ceiling of
  zero, which is what the counts above assume.
- `abbreviations.py` reports `formatter`, `core::fmt`'s word.

## Index row

An outside review of `device_tree_blob` before it leaves. Three day-one gate failures come first
(the manifest, the nife-only prose, 29 refused names). Five items come before the tag. They are a
prefix lookup that contradicts its siblings, a reserved-memory match at any depth, unchecked cursor
arithmetic on 32-bit targets, a walk that is not confined to the structure block, and a fuzz target
that does not move. Fourteen smaller items follow, and the architect ruled all eight decisions.
