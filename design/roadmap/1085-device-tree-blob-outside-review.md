---
status: NOT-STARTED
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
the names already ratified at the item (`device_tree_blob`, `DeviceTreeBlob`, `MAXIMUM_TOTALSIZE`,
`initial_ramdisk`), the provisional names recorded there (`parent_prop_compatible`,
`boot_arguments`), and the bounds notes/device-tree.md records. One ratified name meets a later
rule (item D4). A ruling on record has nife working toward Google's `dtoolkit` as its device-tree
dependency. The switch itself is a separate decision. Until that switch, this crate is what boots
every nife machine. So the correctness items below (P1-1 to P1-4) are worth landing in the kernel's
path whether or not the crate ever publishes.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its naming and standalone helpers,
cargo-mutants, Kani, cargo-fuzz and the crate's own `Blob` builder in `tests/hostile.rs`. No new
tool.

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

Each item: what is wrong, where, why it matters, the fix, effort (S/M/L), and the door. A name,
layout or dependency that the move publishes is a one-way door; everything else is two-way.

### P0: the new repository's required checks are red on day one

**P0-1. The manifest and the files do not stand alone.** `crates/device_tree_blob/Cargo.toml` lines 1 to 9.
`standalone.py` reports: no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, `license.workspace =
true` where the field must read `MIT OR Apache-2.0`, no `rust-version`, `publish = false`, and
`lints.workspace = true`, which inherits a workspace that is not there. The template's `check /
standalone` fails on each. The fix is the scaffold's: inline the license field and the lints
(`unsafe_op_in_unsafe_fn`, `missing_docs`). Add `rust-version` (the crate uses `let` chains and
`is_multiple_of`, so 1.88 is the floor; verify with `cargo +1.88 test`). Add `description`,
`repository`, `keywords`, `categories` and `readme`, and copy both license files. Drop the allowlist
comment and `publish = false` in the release that publishes. The template's README order (`#
device_tree_blob`, `## EXAMPLES`, `## What is proven`, `## BUGS`, `## Minimum Rust version`, `##
License`) is the README's shape. Effort S. Two-way.

**P0-2. The documentation cites nife, and once quotes the architect.** `src/lib.rs` lines 1 to 32 (the
crate's opening paragraph points at `notes/boot-protocol.md`, `notes/device-tree.md` and a decision
section, then carries a 22-line name-provenance block naming the architect three times).
`src/lib.rs` lines 1020 to 1022 (a quotation of the architect's ruling, which no record may carry).
`src/lib.rs` line 105, `:542`, `:1093-1095` (three more `Name:` blocks). `src/lib.rs` line 50, `:81`, `:1017`,
`:1234` and `tests/hostile.rs` line 6, `:184`, `:874`, `:912`, `:1057`, `:1099`, `:1128` (milestone
numbers, `kernel/src/memory.rs`, `notes/fuzzing.md`). All four files under `falsifications/` (each
cites a milestone). `standalone.py` counts 19 milestone citations, 13 paths into the tree, six
section citations, five provenance blocks and four lines naming the architect, and fails on each
class. A stranger cannot follow any of them, and the externalized-crate rule is that a crate
repository does not mention nife. The fix: say in the crate's own words what each citation was
standing for. The 2 MiB ceiling is Linux's `MAX_FDT_SIZE`. The saturating `end` exists because a
`pub` type with `pub` fields lets anyone build it. The one-cell rule follows Linux's
`of_n_addr_cells`. Then move the provenance to the move commit's message, and delete the quotation.
Effort M. Two-way.

**P0-3. The naming gates refuse 29 names.** `spelled_out.py` refuses the binding `dtb` 28 times:
`src/lib.rs` line 181 and 27 bindings across the three test files (the table replaces `dtb` with
`device_tree_blob`; `tree` or `blob` reads better at a binding). `british_spelling.py` refuses
`tests/hostile.rs` line 825 (the British form of "honors"). Both gates run at a ceiling of zero in the template. Effort S.
Two-way.

### P1: land before the tag

**P1-1. `node_reg` does not stop at the first match; its documentation and its siblings say it
does.** `src/lib.rs` lines 320 to 327 says "the `reg` regions of the first node whose name starts with
`prefix`", and the comment at `:369-376` says "either way, stop looking", but the code clears
`target_at` and keeps matching. Probe: a root with `uart@1` (no `reg`) then `uart@2` (`reg`) gives
`node_reg(b"uart@")` one region, `uart@2`'s, while `node_prop(b"uart@", b"reg")` on the same blob
gives `Ok(None)`, and `node_reg_compatible` documents "zero regions for a match with no `reg`"
(`:430`). Two lookups with the same contract answer differently for the same tree, which is the kind
of difference that hides until a board writes a disabled first instance. The callers in the tree
pass `plic@`, `serial@10000000` and `smmuv3@`. On every fixture the first match carries `reg`, so this review found no caller that leans on the fall-through (unverified beyond the fixtures). The fix is to return
`Ok(0)` at the matched node's `END_NODE`, as `node_prop` returns `Ok(None)`, and a hostile test of
the shape above. Effort S. Two-way.

**P1-2. `reserved_memory_regions` matches a `reserved-memory` node at any depth and decodes its
children with the root's widths.** `src/lib.rs` line 952 matches the name wherever it first appears;
`:973-982` reads cell counts at depth 1 only, and `:932` documents "each depth-3 child's `reg`".
Probe: a root declaring 1/1 with `/soc` declaring 2/2 and `/soc/reserved-memory/fw@80000000`
carrying a 2/2 `reg` yields two regions, `{0, 2 GiB}` and `{0, 256 KiB}`, neither of which the tree
names. The kernel reserves what this returns before it has a frame allocator; a wrong answer here is
RAM the machine never gets. Linux resolves the path `/reserved-memory` and reads its children only
(the memory scan it shares the file with reads root children only, which this review verified; the reserved-memory scan stays unverified). The fix: match at depth 2 only, as `memory_regions` and
`initial_ramdisk` do, and a hostile test of the nested shape. Effort S. Two-way.

**P1-3. Offset arithmetic has no check on a target whose `usize` is 32 bits.** Every walker adds a
blob-supplied `u32` to a cursor without a check. The sums are `at + align4(len)` (`src/lib.rs` line 285,
`:382`, `:481`, `:604`, `:670`, `:756`, `:820`, `:906`, `:969`, `:1056`, `:1127`). Next come
`self.off_strings + name_off` (`:287`, `:384`, `:483`, `:606`, `:672`, `:758`, `:823`, `:909`,
`:971`, `:1059`, `:1131`) and `value_at + len` (`:500`, `:591`, `:615`, `:675`, `:742`, `:808`,
`:914`, `:1135`). Last come `at + i * 4` (`:1212`) and `align4` itself (`:1227`, which overflows
above `usize::MAX - 3`). With 64-bit `usize` none can overflow, because `len` and `name_off` are
`u32` and `at` is below `isize::MAX`, so the kernel is sound on its three targets. On a 32-bit
target a `len` of `0xffff_fffd` is an arithmetic overflow: a panic under overflow checks, a wrapped
cursor without them (the wrapped `get` still fails as `Truncated`, so no out-of-bounds read follows
either way). A published `no_std` parser that nothing stops a 32-bit kernel from taking should not
panic on a hostile byte. `be32` and `be64` gained `checked_add` for this reason and proved total, and the walkers above them did not. Inferred from reading; the review host has no 32-bit target.
The fix is one of two: route every cursor step through a
`checked_add(...).ok_or(Error::Truncated)?`, or state `#[cfg(not(target_pointer_width = "64"))]
compile_error!(...)` and say so in the README. The first is the honest one for a crate that claims
`no_std`. The second is cheaper and closes the door on 32-bit users. Effort M for the first, S for
the second. Two-way.

**P1-4. The walk is never confined to the structure block.** `from_bytes` (`src/lib.rs` lines 162 to 193)
reads the magic, `totalsize`, `last_comp_version` and three offsets, and never reads `version`
(offset 20), `size_dt_strings` (32) or `size_dt_struct` (36). A walker runs from `off_struct` until
`FDT_END`, an unknown token or the end of the blob. A structure block that lacks `FDT_END` is
therefore walked into the strings block. The probe replaced `FDT_END` with `FDT_NOP` and
`memory_regions` answered `Err(BadToken(0x2361_6464))`, which is the ASCII of `#add` read as a
token. The answer is an error, so nothing is unsound, but it is the wrong error, and a blob whose
strings block happens to spell a valid token sequence reads as structure. libfdt's
`fdt_check_full` and Linux refuse this; notes/device-tree.md records that this tree filed the same
missing check against `dtoolkit`. The fix: read `size_dt_struct`, refuse `off_struct +
size_dt_struct > totalsize` at `from_bytes`, and stop every walker at that end with `Truncated`. For
a `version` below 17 fall back to `off_strings` as the end, as libfdt does. Effort S. Two-way.

**P1-5. The fuzz target does not move with the crate, and the template will not run one.**
`fuzz/fuzz_targets/device_tree_blob_walk.rs` lives in nife's fuzz workspace and exercises every
public lookup; it is the target that found the out-of-bounds index `tests/hostile.rs` lines 141 to 179
closes. The template's weekly fuzz job refuses the crash-input opt-in for a crate whose shipped
source holds `unsafe` (`unsafe_scan.py` reports `src/lib.rs` line 139, `:142`, `:155`), so after the move
the crate gets fuzzed only where someone runs it by hand. The fix: carry a `fuzz/` directory with the
target and its dictionary into the new repository so the README's one command runs it. Keep the
crate's weekly caller at `public-crash-inputs: false`, and let the tree's own fuzz host keep the
target in its rotation. D6 decides whether the `unsafe` stays. Effort S. Two-way.

### P2: later

The P2 items are in [the appendix](1085-device-tree-blob-outside-review/later-items.md).

## Decisions for the architect

Names and a surface the move publishes. Each states the background first, then a recommendation,
prior art, and an example. All six are two-way doors while the crate is unreleased, and the
maintainer has decided them, provisionally; the decision follows the six.

D1. `prop` in six public names. Background: `node_prop`, `node_props`, `node_prop_compatible`,
`parent_prop_compatible`, `node_prop_inherited` and `phandle_prop` abbreviate "property", which is a
word people say. design/naming.md refuses an abbreviation that needs a decoder and spells out every
word in a name this tree authors. The Devicetree Specification's own word is "property"; `prop` is
libfdt's C spelling (`fdt_getprop`). Recommendation: `node_property`, `node_properties`,
`node_property_compatible`, `parent_property_compatible`, `node_property_inherited`,
`phandle_property`, with no alias for the old spelling (the crate has no release). Prior art: the
`fdt` crate and `dtoolkit` both spell `property()` and `properties()`; libfdt abbreviates. Example:
`tree.node_property(b"chosen", b"stdout-path")`. Effort S; the eight packages in the tree that
depend on the crate change with it.

D2. `compat` as a public parameter. Background: `node_reg_compatible`, `node_prop_compatible`
and `parent_prop_compatible` take `compat: &[u8]` (`src/lib.rs` line 432, `:527`, `:545`). The naming
rules cover a function's parameters, and rustdoc shows them. Recommendation: `compatible`, the
property's own name. Prior art: `fdt::Fdt::find_compatible(with: &[&str])`; `dtoolkit`'s
`is_compatible(&self, compatible: &str)`. Example: `tree.node_reg_compatible(b"arm,pl031", &mut
regions)` reads the same, and its signature now says what the needle is. Effort S.

D3. `reg` stays, with its alias. Background: `node_reg`, `node_reg_compatible` and the private
`decode_reg` carry `reg`, the property's name in the specification, which a reader searches for by
that spelling. The rule for a specification's term of art is to keep it, open the doc comment
with the expansion and the source, and carry `#[doc(alias)]`. Recommendation: keep `reg`, open each
doc comment with "the `reg` property (Devicetree Specification)" and add
`#[doc(alias = "register regions")]`; the same treatment for `phandle_prop`'s `phandle`. Prior art:
the `fdt` crate's `NodeProperty::reg()` keeps the spelling; libfdt has no `reg` helper. Example: a
search for "register" or "reg" lands on `node_reg`. Effort S.

D4. `MAXIMUM_TOTALSIZE` against the units rule. Background: the name has a ratification. The later rule
says a limit ends with its unit (`MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`), and a ratified name gets a recheck at its release review. `totalsize` names the header field in the
specification, so it keeps its spelling inside the name. Recommendation: `MAXIMUM_TOTALSIZE_BYTES`,
with `#[doc(alias = "MAX_FDT_SIZE")]` for the Linux constant it borrows its value from. Prior art:
Linux `MAX_FDT_SIZE`; `dtoolkit` has no ceiling in the released version. Example: `if total >
MAXIMUM_TOTALSIZE_BYTES { return Err(Error::TooLarge(total)) }`. Effort S.

D5. Two names for two sources of reservations. Background: `reserved_regions` reads the memory
reservation block (the fixed-layout list before the structure block) and `reserved_memory_regions`
reads the `/reserved-memory` node's children. A stranger meeting the two names side by side cannot
tell which is which, and the kernel calls both. Recommendation: `memory_reservations` for the block,
the specification's own term ("memory reservation block"), and `reserved_memory_regions` unchanged
for the node. Prior art: the `fdt` crate's `memory_reservations()`; libfdt's `fdt_get_mem_rsv`;
`dtoolkit`'s `reserved_memory()` names the node. Example: `tree.memory_reservations(&mut out)`
beside `tree.reserved_memory_regions(&mut out)`. Effort S.

D6. Whether `from_ptr` stays in the crate. Background: `from_ptr` (`src/lib.rs` lines 139 to 157) is the
crate's only `unsafe`: it reads the header through a raw pointer, checks the magic, caps the size at
2 MiB and builds the slice. Ten call sites in the kernel and the loader use it. Because of it the
template refuses the public fuzz opt-in (P1-5), and the crate cannot carry
`#![forbid(unsafe_code)]`. Moving it out would spread the header-read-and-cap logic to two or more
copies outside the crate, or add a safe `declared_size(header: &[u8; 8])` helper and leave the
`unsafe` slice construction to each caller. Recommendation: keep `from_ptr`, keep the `unsafe` count
at one function, and accept that the weekly public fuzz job does not run. The tree's own fuzz host
keeps the target. The alternative is cleaner for the crate and worse for the system, since the one
place that knows how to trust a firmware pointer should be the parser. Prior art:
`fdt::Fdt::from_ptr` and `dtoolkit::Fdt::from_raw` are both `unsafe` constructors in the parser
crate. Example: `unsafe { DeviceTreeBlob::from_ptr(pointer) }?` stays the kernel's one line. Effort
none if kept.

Decided by the maintainer on 2026-11-14 (UTC), provisional: two-way doors while the crate is
unreleased, which the architect may revisit. The path ruled for `dtoolkit` makes it likely that
this crate is replaced after an upstream release and never published. D1: `node_property`,
`node_properties`, `node_property_compatible`, `parent_property_compatible`,
`node_property_inherited` and `phandle_property`, with no alias for the old spelling. D2: the
parameter `compat` becomes `compatible`. D3: `reg` and `phandle` keep their spelling as the
specification's terms; each doc comment opens with the expansion, and `node_reg` and
`node_reg_compatible` carry `#[doc(alias = "register regions")]`. D4: `MAXIMUM_TOTALSIZE` becomes
`MAXIMUM_TOTALSIZE_BYTES`, with `#[doc(alias = "MAX_FDT_SIZE")]`. D5: `reserved_regions` becomes
`memory_reservations`; `reserved_memory_regions` keeps its name. D6: `from_ptr` stays as the crate's
one `unsafe` function, and the public weekly fuzz opt-in stays off. The count in D6 is low: at
HEAD, 16 call sites outside the crate name `DeviceTreeBlob::from_ptr`, 9 in `kernel/`, 4 in
`uefi_loader/`, 2 in `machine_discovery` and 1 in a board-check tool. The renames are one lane, with
the eight packages that depend on the crate; done-when's last line is the check.

## Done when

The checks are in [the appendix](1085-device-tree-blob-outside-review/done-when.md).

## BUGS

- P1-3 came from reading, not from a build for a 32-bit target. The review host has none, and the fix is cheap enough to land without one.
- The mutation baseline's single missed mutant was not identified; a census inside its window
  names it.
- The Linux reserved-memory scan (`fdt_scan_reserved_mem`) was not fetched; P1-2's claim about it
  is from memory and marked unverified. The review fetched and read the memory scan and `of_fdt_device_is_available`.
- The review ran the template's helpers from a checkout of the template, not from a crate
  repository with its own ceilings. A repository created from the scaffold starts at a ceiling of
  zero, which is what the counts above assume.
- If the ruling to take `dtoolkit` lands before this crate moves, P0 and D1 to D6 lapse. P1-1 to
  P1-4 and P2-2, P2-3 and P2-6 still describe the parser the kernel boots with until then.

## Index row

An outside review of `device_tree_blob` before it leaves. Three day-one gate failures come first
(the manifest, the nife-only prose, 29 refused names). Five items come before the tag. They are a
prefix lookup that contradicts its siblings, a reserved-memory match at any depth, unchecked cursor
arithmetic on 32-bit targets, a walk that is not confined to the structure block, and a fuzz target
that does not move. Fourteen smaller items follow, and six names go to the architect.
