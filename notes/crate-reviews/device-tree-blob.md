# Review package: `device_tree_blob`

*Name: provisional. The `device_tree_blob` package of [crate-reviews.md](../crate-reviews.md), in its
own file because the packages together outgrow the page.*

The architect reads this to ratify the crate's names and to give the go to create
`nifeos/device_tree_blob`, as item 4 of
[milestone 857 (crates released on their own)](../../design/roadmap/0857-crates-released-on-their-own.md).
It describes the crate as `main` leaves it. The findings behind it are in
[milestone 1085 (what an outside maintainer would fix before it leaves)](../../design/roadmap/1085-device-tree-blob-outside-review.md);
this file does not repeat them.

## In one paragraph

A flattened device tree parser: bytes in, regions and property values out. The device tree blob is
the machine describing itself, handed over by firmware (a bootloader, a UEFI loader or QEMU), and
the crate reads it as untrusted input. It checks every offset, bounds every walk, and answers a
malformed blob with an `Error`, never a panic. It reads the memory banks, the memory reservation
block, the `/reserved-memory` children, the register regions of a node found by name prefix or by
`compatible` string, any property of a node, the parent's `compatible`, the initial ramdisk and the
command line. The crate is `no_std` and has no run-time dependency. It has `unsafe` in one function:
`DeviceTreeBlob::from_pointer`, which reads eight bytes through a raw pointer before it validates
anything. Nine Kani harnesses prove the leaf readers and the token step, each with a falsification
patch. This crate boots every nife machine today. Nothing in its code points back into nife.

## Identity

| item | value |
|---|---|
| crate | `device_tree_blob`, ratified 2026-09-18 and performed 2026-09-19 ("device tree blob" is a phrase people say, so `dtb` is refused) |
| version | `0.1.0`; no tag and no repository yet |
| edition, `rust-version` | 2024; the manifest has no `rust-version` yet, and the move writes 1.88 (the copy outside the tree builds and passes on 1.88) |
| license | `license.workspace = true`; the move writes `MIT OR Apache-2.0` and both license texts. The crate adapts no source: libfdt, Linux and the Devicetree Specification are named as prior art and for constants (`MAX_FDT_SIZE`, `FDT_MAX_NCELLS`, `FDT_MAX_DEPTH`), and no text is copied |
| `publish` | `false`, and stays so in the new repository: no tag and no publish until the architect says so |
| dependencies | none at run time or in development |
| size | `src/lib.rs` of 1,476 lines (library, doctests and Kani harnesses), `tests/hostile.rs` of 1,889 lines (46 tests over hand-built blobs), two QEMU fixture suites of 275 and 271 lines, five `.dtb` fixtures and one `.dts`, nine falsification patches. No README and no license file yet |
| format read | the flattened format of chapter 5 of the Devicetree Specification, release 0.4, versions up to 17 |
| bounds | `MAXIMUM_TOTAL_SIZE_BYTES` 2 MiB, `MAXIMUM_DEPTH` 64, `MAXIMUM_CELLS` 4; `notes/device-tree.md` records them |

## Who in nife depends on it

Each by a path dependency with `version = "0.1.0"`, except the tool and the fuzz crate, which use a
bare path:

- `kernel`: eleven files read the tree at boot (memory, boot, console, screen, the PCI setup, the
  three architectures' timers and interrupt controllers, MMU and ISA setup).
- `uefi_loader`: five files. It builds a tree from firmware or ACPI and reads it back.
- `crates/machine_discovery`: 24 files of source and tests. It is the largest reader, and its
  records carry `Region` in their public types (`machine_discovery::gic`).
- `crates/device_tree_patch` (the writer that edits a blob), `crates/jh7110_watchdog` and
  `crates/pci_configuration_space`.
- `tools/jh7110_entropy_board_check`, and `fuzz/` for three targets (`device_tree_blob_walk`,
  `device_tree_patch_initial_ramdisk`, `machine_description_from_device_tree`).

`DeviceTreeBlob::from_pointer` has sixteen call sites outside the crate: nine in `kernel/`, four in
`uefi_loader/`, two in `machine_discovery` and one in the board-check tool.

Tree files that name the crate: `packages/contracts.package.toml`,
`helpers/template_report_baseline.txt` (three rows: `abbreviations` 12, `prose_acronyms` 7,
`register_names` 1), `.cargo/mutants-baseline.txt` (389 caught, 1 missed, 29 timed out, 25
unviable; the census predates the proofs), `.github/workflows/ci.yml` and
`.github/workflows/kani-reach.yml`. `crates/externalization-list.txt` does not list it.

What references back into nife from the crate: nothing in code or manifest. The `Name:` blocks and
some doc comments cite milestone and decision numbers; the move strips the blocks. The fuzz target
`device_tree_blob_walk` stays in nife's `fuzz/` workspace until the move stages it (P1-5).

## Public API

| kind | items |
|---|---|
| a span | `Region { start: u64, size: u64 }`, `Region::end() -> u64` (saturating) |
| the error | `Error` (`#[non_exhaustive]`): `BadMagic(u32)`, `UnsupportedVersion(u32)`, `Truncated`, `BadValue`, `BadToken(u32)`, `TooManyRegions`, `BadStructure`, `TooDeep`, `TooLarge(usize)`, `RegionOverflow` |
| bounds | `MAXIMUM_TOTAL_SIZE_BYTES`, `MAXIMUM_CELLS`, `MAXIMUM_DEPTH` |
| the tree | `DeviceTreeBlob<'a>` |
| constructors | `unsafe from_pointer(pointer: *const u8)`, `from_bytes(bytes: &'a [u8])`, `total_size()` |
| memory | `memory_regions`, `memory_reservations`, `reserved_memory_regions` |
| register regions | `node_register_regions(prefix, out)`, `node_register_regions_compatible(compatible, out)` |
| properties | `node_property(prefix, name)`, `node_property_inherited`, `node_property_compatible`, `parent_property_compatible`, `pointer_handle_property(pointer_handle, name)`, `node_properties` |
| `/chosen` | `initial_ramdisk`, `boot_arguments` |

No nife type, capability or handoff appears in a public signature.

What a stranger may find surprising:

- Every lookup walks the structure block from its first token and holds nothing between calls. A
  lookup that finds its answer first stops there; one that does not reads the whole tree.
- Output goes into a caller's `&mut [Region]` slice and the count comes back, because the crate has
  no allocator. A slice too short is `Error::TooManyRegions`, never a silent truncation.
- Cell widths come from the nearest ancestor that declares them, as Linux does, and not from the
  specification's default of 2 and 1. A reader that follows the specification returns a different
  region for a tree that is silent. `notes/device-tree.md` records the choice.
- `node_register_regions` matches a name prefix (`b"serial"` matches `serial@10000000`) and stops
  at the first match; `node_register_regions_compatible` is the strict form.
- `from_bytes` accepts a buffer longer than the blob and trims to `totalsize`; `from_pointer`
  trusts the header's length up to 2 MiB.
- `Error` implements `Display` but not `core::error::Error` through a feature; the crate has no
  features.

## Proofs and falsification patches

Nine Kani harnesses, each with a patch under `falsifications/`. Each patch breaks the code under
proof, and its harness must then fail.

| harness | what it proves |
|---|---|
| `be32_is_total`, `be64_is_total` | a read at any offset returns a value or `Truncated`, never panics |
| `be32_reads_big_endian_when_in_bounds` | an in-bounds read equals the big-endian decode |
| `align4_rounds_up_to_a_multiple_of_four` | the alignment helper |
| `skip_padded_never_wraps` | the cursor skip does not wrap on a 64-bit or a 32-bit `usize` |
| `a_token_step_is_total_and_moves_forward` | one token step is total and advances, so a walk ends |
| `cells_refuses_a_value_wider_than_64_bits` | a cell value above two cells is `BadValue` |
| `decode_regions_stays_inside_its_output` | the decoder never writes past `out` |
| `from_bytes_is_total` | a buffer of up to 48 bytes gives a tree or an error, and the tree lies inside the buffer |

The walkers are not proved; the fuzz target covers them in nife, and the hostile suite pins the
behavior of each lookup on hand-built blobs. a lane fixed P1-3 (32-bit arithmetic) from reading, and
nobody built a 32-bit target.

## Names

The rulings of 2026-11-19 (UTC) merged as [#3783](https://github.com/nifeos/nife/pull/3783) and
change nothing in this table except the status column. A name below reads "ratified as built" when
the milestone's ruling covers it, "ratified" when an earlier lane recorded a ruling in its `Name:`
block, and "provisional" when the block still says so or no block exists. The proposal column is for
the architect: ratify as built, unless the row says otherwise. Every name the milestone renamed
carries its ratified block already.

| item | kind | status | proposal |
|---|---|---|---|
| `device_tree_blob` | crate | ratified 2026-09-18 | ratified; the repository takes it |
| `DeviceTreeBlob` | type | ratified 2026-09-18 (crate block) | ratify as built |
| `initial_ramdisk` | method | ratified 2026-10-12 (#1998), refused `initrd` | none |
| `MAXIMUM_TOTAL_SIZE_BYTES` | constant | ratified 2026-11-19 (D4), aliases `MAX_FDT_SIZE` and `totalsize` | none |
| `from_pointer` | `unsafe` constructor | ratified 2026-11-19 (D6), alias `from_ptr` | none |
| `memory_reservations` | method | ratified 2026-11-19 (D5) | none |
| `node_register_regions`, `node_register_regions_compatible` | methods | ratified 2026-11-19 (D3), alias `reg` | none |
| `node_property`, `node_properties`, `node_property_compatible`, `parent_property_compatible`, `node_property_inherited` | methods | ratified 2026-11-19 (D1) | none |
| `pointer_handle_property` | method | ratified 2026-11-19, alias `phandle` | none |
| the `compatible` parameter of three lookups | parameter | ratified 2026-11-19 (D2) | none |
| `MAXIMUM_CELLS` | constant | provisional; its block says so | ratify as built: it is libfdt's `FDT_MAX_NCELLS` spelled out, alias kept |
| `MAXIMUM_DEPTH` | constant | provisional; its block says so | ratify as built: Linux's `FDT_MAX_DEPTH` spelled out, alias kept |
| `boot_arguments` | method | provisional, minted 2026-11-01 (UTC) | ratify as built, for the reason `initial_ramdisk` refused `initrd`. The binding's spelling `bootargs` could carry a doc alias (a search aid, not an old nife spelling) |
| `Region`, `Region::start`, `Region::end` | type, field, method | no block; the milestone's baseline lists the names it ratifies, and `Region` is not among them | ratify as built |
| `Region::size` | field | no block; refused by `register_names` (a length with no unit) | ratify `size_bytes`, so the field carries its unit as every other length does; the maintainer rules the rename provisionally (milestone 1085 F-size), and it touches every caller of `Region` in the tree |
| `Error` and its ten variants | type | no block | ratify as built; each variant names its fault, as libfdt's error codes do |
| `from_bytes`, `total_size` | methods | no block | ratify as built; `total_size` shares `totalsize`'s word split |
| `memory_regions`, `reserved_memory_regions` | methods | no block | ratify as built; the pair with `memory_reservations` says which of three sources each reads |

Private and test names are for the move lane. Milestone 1085 records that `register_names` refuses four test names in
`tests/hostile.rs` (among them `SIZE_DT_STRUCT_AT` and `address_cells`), and `prose_acronyms` flags
UEFI, NUL, GIC, PL031, PLIC, PMIC and PMP in comments. The move's gate pass clears them; its dry run reported `spelled-out words` as passing on the staged copy, so the move lane rechecks the list.

### Which `Name:` blocks still read provisional

Three: `MAXIMUM_CELLS`, `MAXIMUM_DEPTH` and `boot_arguments`. Thirteen read ratified, eleven of them
with the date 2026-11-19. `script/crate-standalone` counts 11 names in all that lack a ratified block: the three
above and the eight public names with no block. Once the architect rules, a later lane rewrites the three, and writes a block
for each name that has none; the move strips every block. This lane changes no crate code.

## What was run

I ran all of it on the branch of this package, which leaves the crate's code as `main` has it, on the
pinned nightly of the tree unless a row says otherwise. The host ran at a load average above 100
because other lanes were running, so the wall times mean little.

| check | result |
|---|---|
| `cargo test -p device_tree_blob` | 1 unit test, 46 hostile tests, 17 aarch64 and 10 riscv64 QEMU fixture tests and 4 doctests pass |
| `cargo clippy -p device_tree_blob --all-targets -- -D warnings` | no warning |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p device_tree_blob --no-deps` | no warning |
| `cargo build -p device_tree_blob --lib --target <t>` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` | all three build |
| the crate's `src/` and `tests/` copied out of the tree with a stand-alone manifest (edition 2024, `rust-version` 1.88), `cargo +1.88 build --lib` for the host and the three targets, then `cargo +1.88 test` | all four builds pass; the same 78 tests and 4 doctests pass on 1.88. The one warning is the unknown `cfg(kani)`, which the template's manifest declares |
| `cargo kani -p device_tree_blob` (Kani 0.68.0, CBMC 6.11.0) | 9 of 9 harnesses verify, 0 failures; 23 minutes of wall time on the loaded host |
| `script/falsifications --sweep device_tree_blob` | the first sweep swept 9: eight turned red and the ninth, `a_token_step_is_total_and_moves_forward`, hit the 900 s limit with no verdict on the loaded host. `script/crate-standalone` ran all nine patches afterward and each fails its harness: 9 of 9, 0 survivors |
| `cargo miri test -p device_tree_blob` | the unit test, the 46 hostile tests (331 s), the 10 riscv64 fixture tests and the 4 doctests pass. In the first full run, `refuses_to_overflow_the_callers_slice` of the aarch64 fixture suite failed once, with an index panic reported at `src/lib.rs` line 1237, whose code has no indexing (`out.get_mut(n)`). The test passed alone, and the whole aarch64 suite passed twice more (17 of 17) with the same sources. I could not reproduce the failure and do not know its cause; the move lane reruns the suite under Miri, and a second failure is a finding |
| `script/crate-standalone device_tree_blob` | 8 failures, all the move's work: `names` (11 names not ratified, first `MAXIMUM_CELLS`), the manifest inherits `license` and `lints` and has no `rust-version`, no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, and one path into the nife tree at `tests/hostile.rs` line 1228. Passes: `build` for the host and the three targets, `proofs` (9 of 9) and all nine falsification patches |
| `script/crate-move device_tree_blob --dry-run --into <scratch>` | stages at template `v8`; the nine patches apply exactly with no offset and no fuzz, the `Name:` blocks are stripped, the fuzz target stages with its six inputs, and the mutation baseline row is carried over. It reports six findings, all for the move lane. Three are the README and license gaps: no README, `standalone` (no license files and the same three manifest items) and `prose caps` (the README that does not exist yet). Three are in the staging: `cargo generate-lockfile` and `cargo fuzz build` fail on the inherited workspace fields, and `citations` finds a nife path at `fuzz/fuzz_targets/device_tree_blob_walk.rs` line 35. Spelled-out words, unit symbols, passive voice, history phrases, function and file length, comment blocks, orphan modules, markdown links, falsification records and `mentions nife` pass. The same nife pull request must remove the target from `fuzz/Cargo.toml`, `script/fuzz` and `notes/fuzzing/targets.md` |

Not run, and so not claimed:

- Mutation. The census belongs in the 08:00 to 13:00 UTC window, and I wrote this package
  outside it, at about 22:00 UTC. The baseline row (389 caught, 1 missed, 29 timed out, 25 unviable)
  predates the fixes of #3559 and #3612, and the missed mutant was never identified. The move
  carries the row into the new repository, and its first weekly run lists survivors.
- The stranger test: building, testing, proving and falsifying the crate from its README alone. The
  README does not exist yet (P0-1), so there is nothing to follow.
- `cargo-semver-checks`: a first release has no baseline.
- The fuzz target. It stays in nife until the move stages it, and the weekly public opt-in stays
  off because of `from_pointer` (D6).
- A build for a 32-bit target. The host has none, and a lane fixed P1-3 from reading.
- The template's workflows, which run in the repository.

## The move lane's work

A lane after the go does these, in the order the milestone lists them:

1. P0-1: write the manifest literals (`license`, `lints`, `rust-version` 1.88, `repository`), the
   README with its `## EXAMPLES` (P2-9's remaining half) and both license files.
2. P1-5: stage `fuzz/fuzz_targets/device_tree_blob_walk.rs` in the new repository with
   `public-crash-inputs: false`.
3. Rewrite the three provisional `Name:` blocks, strip every `Name:` block, rename `Region::size`
   and the four test names, and clear the `prose_acronyms` and `abbreviations` findings in the
   `helpers/template_report_baseline.txt` rows.
4. Convert the dependents' path dependencies (the six crates and two tools above) to `git` and
   `rev` with `version = "0.1.0"`, per §305 (git dependencies on `nifeos/*` only, pinned to a landed commit), and update `packages/contracts.package.toml`,
   `.cargo/mutants-baseline.txt` and the CI references.
5. Run the stranger test, the template's workflows and the first weekly mutation run in the new
   repository.

The new repository carries `publish = false`, no tag and no publish.

## The `dtoolkit` position

Ruled 2026-11-19 (UTC): nife waits for a `dtoolkit` release that carries
[google/dtoolkit #111](https://github.com/google/dtoolkit/pull/111) and
[#112](https://github.com/google/dtoolkit/pull/112), then rules the switch against the dependency
bar as a decision of its own. Both are open with changes requested, and release 0.5.0 of
2026-11-03 carries neither. Upstream merged #108. This lane files nothing upstream. Until the
switch, `device_tree_blob` boots every nife machine, so the move does not wait. If the switch
comes first, P0-1, P1-5 and the README half of P2-9 lapse, and the kernel keeps the fixes either way.
The comparison is
[dtoolkit against device_tree_blob](../../design/roadmap/0784-reuse-what-the-tree-wrote/dtoolkit-against-device-tree-blob.md).

## What still blocks the move

| item | owner |
|---|---|
| The architect ratifies the names above and gives the go to create `nifeos/device_tree_blob`. | architect |
| Rewrite the three provisional blocks and write the missing ones. | a later lane |
| P0-1, P1-5, the `Region::size` rename and the gate pass. | move lane |
| Convert the dependents' path dependencies and the tree's references. | move lane |

## Open questions

1. `Region::size` to `size_bytes`. Recommendation: yes, before the repository publishes the type;
   it is one-way at publish and the maintainer rules the rename provisionally until then.
2. Whether `boot_arguments` carries `#[doc(alias = "bootargs")]`. Recommendation: yes, a search aid
   for the binding's term.

## Recommendation

Ready for review.
