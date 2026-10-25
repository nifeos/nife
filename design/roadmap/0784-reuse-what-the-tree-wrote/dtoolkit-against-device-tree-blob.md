# `device_tree_blob` against Google's `dtoolkit`

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md), for the
`device_tree_blob` row of [the rest of the tree](the-rest-of-the-tree.md) and of the publish table in
[the FAT options](fat-adopt-or-build.md). Name provisional.

784 recorded "contribute to `dtoolkit`" with no technical comparison. calef asked for one on
2026-10-25 (UTC). This page measures our crate against `dtoolkit` 0.4.1 (MIT or Apache-2.0,
`no_std`, no `alloc` for reading). I measured on 2026-10-25 at base `e865d8838`, on a Linux x86_64
build host shared with other lanes, with the pinned nightly of that date, and added no dependency.
[dtoolkit-probe.sh](dtoolkit-probe.sh) generates every probe outside the tree, and its mode names
the section it feeds. [The reports](dtoolkit-upstream-reports.md) hold the three bugs this page
sent upstream, filed as google/dtoolkit#101, #102 and #103.

## What a candidate has to do here

Ten packages depend on the crate: the kernel, `machine_discovery`, `pci` (its tests),
`uefi_loader`, the five JH7110 device crates, and the fuzz target. They make twelve kinds of query.
Four read fixed places: RAM (`memory_regions`), the reservation block, `/reserved-memory` and the
initial ramdisk in `/chosen`. The rest search: a node's `reg` by name prefix or by `compatible`, a
property by prefix, by `compatible`, on the parent of a `compatible` match, by phandle, or inherited
from an ancestor, and one property on every `cpu@`. Nothing in the kernel's path has an allocator.

`dtoolkit` validates the whole blob in `Fdt::new`, then offers the root, each node's children and
properties, lookup by path, `reg` decoded with the parent's cells, and `memory()`,
`reserved_memory()`, `chosen()` and `cpus()`. It has no search by `compatible` below one level, no
phandle lookup, no parent link and no whole-tree iterator. So the probe's adapter writes the walk
itself: a depth-first walk with a stack of child iterators, 32 deep, plus our policy (which node
counts as a match, zero-size pairs skipped, a wrapping pair refused). The adapter is 308 code lines.
Our crate is 745, proofs aside, so a switch would retire about 440 lines net.

`Fdt::memory()` returns the first `/memory` node only; a board that writes two banks as two nodes
needs the adapter's own loop over the root's children. `Fdt::new` refuses a buffer longer than the
header's `totalsize`, so the adapter trims first, as ours does.

## Fit

Both run the kernel's boot queries in a freestanding binary with no global allocator, built for the
three nife targets (`dtoolkit-probe.sh fit`).

| | links with no allocator | text + rodata, aarch64 | riscv64 | x86_64 | parsed type |
|---|---|---|---|---|---|
| ours | yes | 27,288 B | 23,258 B | 26,579 B | 40 B |
| `dtoolkit` and adapter | yes | 45,964 B | 42,248 B | 51,623 B | 16 B (`Fdt`); a node is 32 B |

A cold build of that binary for riscv64 takes 0.9 seconds with ours. With `dtoolkit` it takes 19.7
seconds on one job and 7.2 seconds on eighty, almost all of it the two proc macros and `syn`. The
host was busy with fuzzing, so read these as an order of magnitude.

| | crates new to `Cargo.lock` | crates added to the kernel's graph | `unsafe` in the crate | in its dependencies |
|---|---|---|---|---|
| ours | 0 | 0 | 2 blocks, 1 `unsafe fn` (`from_ptr`) | none |
| `dtoolkit` | 3: `dtoolkit`, `zerocopy`, `zerocopy-derive` | 10, among them two proc macros (`zerocopy-derive`, `thiserror-impl`) and both `syn` 2 and `syn` 3 | 8 blocks, 4 `unsafe fn`; `#![deny(unsafe_code)]` with each use marked `#[expect]`; 3 blocks on the read path, all in `from_raw` | `zerocopy` 302 blocks, 58 `unsafe fn`, 111 `unsafe impl`; `zerocopy-derive` emits `unsafe impl`s into `dtoolkit` |

The kernel's graph today holds four crates from outside the tree and no proc macro: `spin` on every
target, `aarch64-cpu` and `tock-registers` on aarch64, and `uuid` on x86_64 (`cargo tree -e normal`
per target). Counts
are textual over each crate's `src/`, tests included. Every license passes `deny.toml`: `zerocopy` is
BSD-2-Clause or Apache-2.0 or MIT. The minimum Rust is 1.88; nife pins a nightly, so that costs
nothing.

`from_raw` in 0.4.1 builds a slice of the header's `totalsize` before it checks the magic, with no
ceiling: the 4 GiB read our `MAX_TOTALSIZE` exists to refuse. Unreleased `main` adds
`from_raw_bounded` (google/dtoolkit#90), which checks magic and length first. Either way the
kernel's 2 MiB ceiling stays a check of ours.

## Agreement on the tree's trees

`dtoolkit-probe.sh agree` takes every `.dtb` in the tree, 40 files. Five QEMU dumps are
`device_tree_blob`'s own fixtures. `machine_discovery` holds three more QEMU dumps and 13 trees:
JH7110 modeled on vendor and mainline sources, TH1520, the PLIC excerpt captured from radon's
U-Boot, and hand-written shapes. The five device crates hold 19 JH7110 excerpts, radon's PMIC bus
and watchdog among them. For each, the probe collects every node name, name stem, `compatible`
string, property name and phandle the tree holds, and adds every literal the callers pass. It then
asks both parsers every query over every needle: 307,947 queries.

On 38 of the 40, every QEMU dump and every JH7110 tree among them, the two agree on every query.
The 20 disagreements are on two hand-written fixtures, `many-harts` and `partial-psci`, whose `cpu@` nodes carry a `reg` shorter
than one pair. Ours returns no region, as Linux's reader truncates a partial pair; `dtoolkit`
refuses the property. No panic on either side.

Two of our own bugs showed up first, and this branch fixes both:

- An empty `compatible` needle matched the first node with any `compatible`, because splitting a
  NUL-terminated value leaves an empty last piece. It caused 379 disagreements; no caller passes an
  empty needle, but the fuzz target does.
- A `#address-cells` or `#size-cells` that is not four bytes read as four, padding included:
  `many-harts`'s two-byte declaration became 65,536 cells. Now the parser ignores a malformed
  declaration and the inherited width stands, as Linux does.

Each has a host test in `crates/device_tree_blob/tests/hostile.rs` that fails without its fix.

The kernel's boot queries, per boot's worth, best of 7 rounds of 200 (`dtoolkit-probe.sh bench`):

| | `qemu-aarch64-virt` | `qemu-riscv64-virt` | QEMU GICv3 | radon's PLIC excerpt | JH7110 |
|---|---|---|---|---|---|
| ours | 34 µs | 20 µs | 41 µs | 9 µs | 12 µs |
| `dtoolkit` and adapter | 260 µs | 156 µs | 285 µs | 65 µs | 82 µs |

A boot asks once, so this decides nothing on its own. The gap comes from validating the whole tree
first and from how a child iterator finds the next sibling (below).

## Where they differ: hostile blobs

The 47 distinct blobs our host tests build (`hostile.rs` and the QEMU tests, recorded as
`from_bytes` sees them) went through both, then six variants of the riscv64 QEMU dump. Neither
parser panicked on any. Nine blobs both refuse and 29 both read identically. The rest:

| case | ours | `dtoolkit` |
|---|---|---|
| a property name of 32 or more characters, such as `snps,dis-u2-freeclk-exists-quirk` | read | the whole tree refused (`InvalidPropertyName`) |
| a property name with a space or a byte that is not UTF-8 | read | the whole tree refused |
| a property after a child node, or before the root | read | the whole tree refused |
| a header whose blocks overlap it | read | refused |
| a bus that declares no `#address-cells` | the parent's width, as Linux | the specification's default of 2 and 1: the child's `reg` refused, or decoded to a different region with no error |
| `reg` shorter than one pair | no region | refused |
| a match 40 levels deep | found by `node_prop` | the adapter stops at 32 |
| an empty strings block ending the blob | refused | read |

The first row is the one that matters. The Devicetree Specification says names are 1 to 31
characters, and `dtc` compiles longer ones. `snps,dis-u2-freeclk-exists-quirk` is 32, and Linux's own
`rk3399-base.dtsi`, `rk3588-base.dtsi`, `rk3328.dtsi`, `exynos2200.dtsi` and the RISC-V
`spacemit/k1.dtsi` carry it. A kernel on `dtoolkit` booted on one of those boards would find no RAM.
The variants with 31, 32 and 33 characters show the edge exactly.

The bus row is a real difference of reading. The specification says the cell counts are not
inherited; Linux's `of_n_addr_cells` walks up to the nearest ancestor that says. Every tree we boot
declares both, so neither choice has met a board.

The depth row is the adapter's, not `dtoolkit`'s: with no parent link and no flat walk, a caller
without a heap has to bound its stack. Ours walks a flat token stream and keeps no stack for that
query.

## Hostile shapes by size

`dtoolkit-probe.sh deep` builds a chain of nested nodes and a flat tree of the same size, up to
Linux's 2 MiB ceiling. It times `Fdt::new`, a walk of every node on `dtoolkit`'s own API (a heap
stack, so no depth limit), and ours walking every token:

| tree | bytes | `Fdt::new` | `dtoolkit` walk | ours |
|---|---|---|---|---|
| chain, 1,000 deep | 12,076 | 0.6 ms | 27 ms | 11 µs |
| chain, 10,000 deep | 120,076 | 4.6 ms | 0.87 s | 74 µs |
| chain, 50,000 deep | 600,076 | 6.5 ms | 18 s | 0.38 ms |
| chain, 174,000 deep | 2,088,076 | 15 ms | 237 s | 1.3 ms |
| flat, 174,000 wide | 2,088,076 | 15 ms | 12 ms | 1.2 ms |

The walk is quadratic in depth. A child iterator finds the next sibling by walking the whole
subtree of the child before it, so a visit costs the sum of every node's subtree. A valid blob
inside Linux's limit holds a walk for four minutes. The adapter's 32-deep stack bounds the
factor at 32, which is why the agreement runs above finish in milliseconds.

`Display` for a node panics, "Formatting argument out of range", on a tree nested 16,384 levels
below the root, a 196 KiB blob that `Fdt::new` accepts: it indents with `{:indent$}`, and the
width passes `u16::MAX`. 16,383 levels print. The kernel would not call `Display`, but a tool on
the host would.

## Fuzzing

Three cargo-fuzz targets ran at once for 20 minutes each, eight forked workers apiece, seeded with
the 91 blobs above and a dictionary of the format's tokens and common names
(`dtoolkit-probe.sh fuzz`). `ours` is the tree's `device_tree_blob_walk`. `dtoolkit_nife` is that
target call for call with the adapter in place of our crate. `dtoolkit_parse` is `dtoolkit`'s own
target from its repository.

| target | executions | coverage (edges) | corpus | panics | hangs over 10 s | out of memory |
|---|---|---|---|---|---|---|
| `ours` | 24.0M | 1,043 | 1,139 | 0 | 0 | 0 |
| `dtoolkit_nife` | 15.1M | 845 | 679 | 0 | 0 | 0 |
| `dtoolkit_parse` | 36.5M | 771 | 660 | 0 | 0 | 0 |

Neither crate failed. The quadratic walk never surfaced as a hang: a fuzzer's inputs stay a few
kilobytes, too small to nest deep enough, and it took the hand-built chains above to show it. Coverage counts different code, so it compares nothing across rows.

## Proofs

Our crate's four harnesses prove the leaf readers: `be32` and `be64` are total, an in-bounds `be32`
reads big-endian, and `align4` rounds up to a multiple of four. All four verify in 10 seconds. The
same properties asked of `dtoolkit`'s nearest public functions (`dtoolkit-probe.sh kani`, Kani
0.68, 15 minutes per harness):

| property | ours | `dtoolkit` |
|---|---|---|
| P1, P2: the 4- and 8-byte readers are total on any slice | proved | proved, 3 s (`from_property_value`) |
| P3: an accepted 4-byte read is big-endian | proved | proved, 3 s |
| P4: padding rounds up to a multiple of four | proved | proved, 1 s (core's `next_multiple_of`) |
| any 16-byte `reg` in a valid tree decodes to those bytes | timed out | timed out |
| `from_bytes` or `Fdt::new` on any 64 bytes never panics | header only: proved, 4 s | timed out |
| a walker over any 64-byte blob the parser accepted never panics | timed out | timed out |

So all four hold on `dtoolkit`, and they hold for the reason they hold on ours: they are small
functions. Neither crate's whole parse fits in CBMC at this size and bound; validating up front does
not make `dtoolkit` easier to prove than our walkers. Our `from_bytes` checks only the header, which
is why it alone finishes. Proving either parse needs restructuring: a bounded loop body proved once, with the
iteration argued separately. That is the freedom §46 keeps by having us write the crates Kani proves.

## Record and upkeep

Read from crates.io and GitHub on 2026-10-25.

- Releases: eight since the first, on 2025-12-15, the latest 0.4.1 on 2026-09-25. 0.2.0,
  0.3.0 and 0.4.0 each broke the API. `main` holds four more breaking changes, among them the removal
  of the `Node` trait's generic associated types (#85) and of `ToCellInt` (#87). The adapter builds
  against 0.4.0 and 0.4.1, and fails with 5 errors on 0.2.0 and 0.2.1, 4 on 0.3.0 and 2 on `main`
  (`dtoolkit-probe.sh churn`).
- People: two crates.io owners, Mateusz Maćkowski and Andrew Walbran, both at Google. Of 94 pull
  requests, Maćkowski opened 74. Outside contributors have opened four, all merged, median three
  days: two from Arm (the fuzz target and a header-validation fix) and two from one other person.
  Two issues ever, both Arm's fuzzer finding panics, both closed by 0.2.1. In practice the project
  has one maintainer.
- Use: 4,986 downloads, 1,423 in the last 90 days; 20 stars.
- Checks: CI runs tests on stable, nightly and 1.88, Clippy, Miri, coverage, minimal versions and
  every feature combination. The repository has one cargo-fuzz target, `parse`, and CI does not run
  it. No Kani.
- Advisories: none for `dtoolkit`, `zerocopy-derive` or `thiserror` in RUSTSEC (advisory-db at
  `21d909e`, 2026-10-24). 0.7.31 and later carry the fix for `zerocopy`'s RUSTSEC-2023-0074.
- Contribution terms: Google's Contributor License Agreement, signed once by a person or an
  employer, and Conventional Commits. CONTRIBUTING states no policy on AI-written work, and allows
  `unsafe` only with a written reason.

## What it means for 784's "contribute"

The test is calef's of 2026-10-22: release our crate only where no maintained upstream crate
deserves our proofs, and contribute where one does. `dtoolkit` deserves them. Google maintains it and
Arm uses it. It agrees with ours on every query over every QEMU and JH7110 tree, it survived the
same fuzzing, and our four properties hold on its code. Its whole parse is no easier to
prove than ours, so the proofs do not favor either crate. 857 should therefore not publish
`device_tree_blob`, and "contribute" stands.

What contributing means changes. Today `dtoolkit` cannot replace ours in the kernel. It refuses real
Linux trees over a name's length and walks a deep tree in quadratic time. It would add ten crates
and two proc macros to a kernel graph that holds four and no proc macro, and it breaks its API every minor release. §46 (thin primitives or whole subsystems; we write everything in between) rule 3
keeps the kernel's parser ours regardless, so none of this argues for a switch; it says what to
send. Three bug reports, now filed ([the reports](dtoolkit-upstream-reports.md)): the name-length
refusal, the quadratic walk, and `Display`'s panic. Then the four harnesses above, which hold on its
code, the hostile corpus as fuzz seeds, and a CI job for its fuzz target. Each of those is code, so
each needs the CLA signed first.

If calef later wants the kernel's parser gone, the gate is a release that reads long names and
walks in linear time, after the 0.5 API settles. Even then the policy stays ours, about 300 lines.
That would be its own ruling under §46. This page proposes none.
