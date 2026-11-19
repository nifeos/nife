# P0 and P1: before the tag

An appendix to [the milestone](../1085-device-tree-blob-outside-review.md).

## P0: the new repository's required checks are red on day one

### P0-1. The manifest and the files do not stand alone

`crates/device_tree_blob/Cargo.toml` lines 1 to 9.
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

### P0-2. The documentation cites nife, and once quotes the architect

DONE, except the `Name:` blocks, which the move commit strips. `src/lib.rs` lines 1 to 32 (the
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

### P0-3. The naming gates refuse 29 names

DONE (the binding is `tree`, and "honor" is spelled the American way). `spelled_out.py` refuses the binding `dtb` 28 times:
`src/lib.rs` line 181 and 27 bindings across the three test files (the table replaces `dtb` with
`device_tree_blob`; `tree` or `blob` reads better at a binding). `british_spelling.py` refuses
`tests/hostile.rs` line 825 (the British form of "honors"). Both gates run at a ceiling of zero in the template. Effort S.
Two-way.

## P1: land before the tag

### P1-1. `node_reg` does not stop at the first match; its documentation and its siblings say it does

DONE (`node_reg_stops_at_the_first_matching_node_even_without_a_reg`). `src/lib.rs` lines 320 to 327 says "the `reg` regions of the first node whose name starts with
`prefix`", and the comment at `:369-376` says "either way, stop looking", but the code clears
`target_at` and keeps matching. Probe: a root with `uart@1` (no `reg`) then `uart@2` (`reg`) gives
`node_reg(b"uart@")` one region, `uart@2`'s, while `node_prop(b"uart@", b"reg")` on the same blob
gives `Ok(None)`, and `node_reg_compatible` documents "zero regions for a match with no `reg`"
(`:430`). Two lookups with the same contract answer differently for the same tree, which is the kind
of difference that hides until a board writes a disabled first instance. The callers in the tree
pass `plic@`, `serial@10000000` and `smmuv3@`. On every fixture the first match carries `reg`, so this review found no caller that leans on the fall-through (unverified beyond the fixtures). The fix is to return
`Ok(0)` at the matched node's `END_NODE`, as `node_prop` returns `Ok(None)`, and a hostile test of
the shape above. Effort S. Two-way.

### P1-2. `reserved_memory_regions` matches a `reserved-memory` node at any depth and decodes its children with the root's widths

DONE (`a_reserved_memory_node_below_the_root_is_not_the_reservation_list`). `src/lib.rs` line 952 matches the name wherever it first appears;
`:973-982` reads cell counts at depth 1 only, and `:932` documents "each depth-3 child's `reg`".
Probe: a root declaring 1/1 with `/soc` declaring 2/2 and `/soc/reserved-memory/fw@80000000`
carrying a 2/2 `reg` yields two regions, `{0, 2 GiB}` and `{0, 256 KiB}`, neither of which the tree
names. The kernel reserves what this returns before it has a frame allocator; a wrong answer here is
RAM the machine never gets. Linux resolves the path `/reserved-memory` and reads its children only
(the memory scan it shares the file with reads root children only, which this review verified; the reserved-memory scan stays unverified). The fix: match at depth 2 only, as `memory_regions` and
`initial_ramdisk` do, and a hostile test of the nested shape. Effort S. Two-way.

### P1-3. Offset arithmetic has no check on a target whose `usize` is 32 bits

DONE (the first of the two fixes: every cursor step is checked; `a_property_length_past_the_blob_is_truncated_in_every_lookup`, the helper tests in `src/lib.rs`, and the `skip_padded_never_wraps` harness with its falsification patch). The wrap itself needs a 32-bit `usize`, so no blob test on a 64-bit host fails before the fix. Every walker adds a
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

### P1-4. The walk is never confined to the structure block

DONE (`a_structure_block_without_its_end_token_is_truncated_not_read_into_the_strings`, `the_declared_structure_size_bounds_the_walk`, `a_blob_older_than_version_17_ends_its_structure_at_the_strings`). `from_bytes` (`src/lib.rs` lines 162 to 193)
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

### P1-5. The fuzz target does not move with the crate, and the template will not run one

`fuzz/fuzz_targets/device_tree_blob_walk.rs` lives in nife's fuzz workspace and exercises every
public lookup; it is the target that found the out-of-bounds index `tests/hostile.rs` lines 141 to 179
closes. The template's weekly fuzz job refuses the crash-input opt-in for a crate whose shipped
source holds `unsafe` (`unsafe_scan.py` reports `src/lib.rs` line 139, `:142`, `:155`), so after the move
the crate gets fuzzed only where someone runs it by hand. The fix: carry a `fuzz/` directory with the
target and its dictionary into the new repository so the README's one command runs it. Keep the
crate's weekly caller at `public-crash-inputs: false`, and let the tree's own fuzz host keep the
target in its rotation. D6 decides whether the `unsafe` stays. Effort S. Two-way.
