# Review package: `memory_region_heap`

*Name: provisional, minted by lane 1100-rulings-package. The `memory_region_heap` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect cleared this package on 2026-11-20 (UTC) for the crate's move to its own repository, in step 9 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` leaves it. The outside review behind it is
[milestone 1100 (`memory_region_heap`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1100-memory-region-heap-outside-review.md).
The ruling of 2026-11-19 (UTC) ratified the names. The clearance of 2026-11-20 (UTC) approves
`nifeos/memory_region_heap` with `publish = false`; there is no tag and no publish.

## In one paragraph

A `GlobalAlloc` paid from the process's own memory region. The region is the budget of pages the
process was handed. The heap maps pages out of it one at a time, hands them to `talc` (5.1.1, the
`Manual` source), and lives at one virtual range the program picks. It maps nothing until the first
allocation, then grows geometrically: as many pages again as it holds, at least eight, never past
the cap. It returns null at the cap, when the region has no page left, and after any other refusal;
only the last stops growth for good. It never shrinks. The page supply is a trait, `PageSource`, so
the growth policy runs on a host with a fake source. The crate is `no_std`, has `unsafe` in the
`Sync` and `GlobalAlloc` impls, and three Kani harnesses prove the growth arithmetic.

## Identity

| item | value |
|---|---|
| crate | `memory_region_heap`, ratified 2026-11-14 (UTC) |
| version | `0.1.0`; no tag and no repository yet |
| edition | 2024 |
| `rust-version` | none yet; the manifest inherits `license` and `lints` from the workspace. The move lane writes both. The crate builds on 1.88 (below) |
| license | none of its own yet. The move lane adds `MIT OR Apache-2.0` with both texts, as ruled for every crate that leaves |
| `publish` | `false`; the allowlist lint holds the exception. No publish until the review milestones are delivered |
| size | one `src/lib.rs` of 1,160 lines (library, host tests, Kani harnesses), three falsification patches, and an unpublished `fuzz` crate with seeds |
| dependencies | `abi`, `address_space_layout` and `user_mode_runtime` by path, each `version = "0.1.0"`; `talc = "5.1.1"` without default features |

## Who in nife depends on it

`components`, `fixtures` and `redoxfs_server` (optional there) depend on it by path. Programs
declare it as the global allocator. The standard library's allocator overlay,
`patches/std-nife/overlay/std/src/sys/alloc/nife/heap.rs`, copies the growth policy (`MINIMUM_GROW_PAGES`
and the shortfall reckoning) instead of calling the crate, and so does
`redoxfs_server/src/bin/second_mount.rs`. Item 8 of the milestone keeps that open: one copy of the
policy is still to come. `packages/runtime.package.toml` lists the crate, and
`crates/externalization-list.txt` names it for step 9. The crate leaves on its own clearance, with `subtree_scope` or alone, and
before `redoxfs`, which shares its `talc` and keeps its own track.

Four things point back into nife from the crate. The three path dependencies become `git` and
`rev` pins once `abi`, `address_space_layout` and `user_mode_runtime` have repositories, which
steps 2 and 3 of milestone 1017 produce first. The rest are citations of nife milestones and paths in the
crate's text and fuzz crate, and the `Cargo.toml` inheritance. `script/crate-standalone` and
`script/crate-move --dry-run` list each (below).

## Public API

Every public name reads ratified: decision A ratified seven on 2026-11-14 (UTC), and the ruling of
2026-11-19 (UTC) ratified the thirteen that items 5 to 13 added.

| item | kind | signature or value | status |
|---|---|---|---|
| `memory_region_heap` | crate | | ratified 2026-11-14 |
| `DEFAULT_BASE_VIRTUAL_ADDRESS` | constant | `u64`, `address_space_layout::HEAP.start` | ratified 2026-11-14 |
| `MemoryRegionHeap<S: PageSource = MemoryRegionPages>` | struct | a `GlobalAlloc`; `Sync` by `unsafe impl` | ratified 2026-11-14 |
| `MemoryRegionHeap::new` | const fn | `() -> Self`, for the default source | ratified 2026-11-14 |
| `MemoryRegionHeap::initialize` | method | `(&self, slot, base, maximum_bytes) -> Result<(), WiringRefusal>` | ratified 2026-11-14 |
| `MemoryRegionHeap::with_source` | const fn | `(source: S) -> Self` | ratified 2026-11-19 |
| `MemoryRegionHeap::initialize_source` | method | `(&self, base: u64, maximum_bytes: u64) -> Result<(), WiringRefusal>` | ratified 2026-11-19 |
| `MemoryRegionHeap::is_initialized` | method | `(&self) -> bool` | ratified 2026-11-19 |
| `MemoryRegionHeap::commit_all` | method | `(&self) -> Result<(), Short>` | ratified 2026-11-14 |
| `MemoryRegionHeap::committed_bytes` | method | `(&self) -> u64` | ratified 2026-11-14 |
| `MemoryRegionHeap::maximum_bytes` | method | `(&self) -> u64` | ratified 2026-11-19 |
| `MemoryRegionHeap::base_virtual_address` | method | `(&self) -> u64` | ratified 2026-11-19 |
| `MemoryRegionHeap::last_refusal` | method | `(&self) -> Option<Refusal>` | ratified 2026-11-19 |
| `PageSource` | trait | the seam; one method | ratified 2026-11-19 |
| `PageSource::map_page` | method | `(&mut self, virtual_address: u64) -> Result<(), Refusal>` | ratified 2026-11-19 |
| `MemoryRegionPages` | struct | the default source: `MAP` on a memory region in one table slot | ratified 2026-11-19 |
| `Refusal` | struct | `pub code: i64`, the nonzero value the source answered with | ratified 2026-11-19 |
| `Refusal::is_budget_spent` | method | `(self) -> bool` | ratified 2026-11-19 |
| `Short` | struct | `pub uncommitted_bytes: u64` | ratified 2026-11-19 |
| `WiringRefusal` | enum | `AlreadyWired` and the other refusals of a wiring call | ratified 2026-11-19 |

The fuzz crate's names (the modules `src`, `policy` and `sequence`, and the function `run`) stay
provisional. The crate never publishes.

What a stranger may find surprising:

- `MemoryRegionHeap::new` is a `const fn` for a `static`; the heap does nothing until `initialize`.
- The program picks the base and the cap. `initialize` refuses a cap under one page. The smallest useful cap
  is one page, which serves about 2,500 bytes of allocations after `talc`'s 1,575 bytes of
  bookkeeping.
- A second `initialize` changes nothing and says so with `WiringRefusal::AlreadyWired`.
- Null from `alloc` means one of three refusals; `last_refusal` tells a person which.
- Growth is monotonic. The heap never calls `talc`'s `truncate`, and the memory-region interface has no
  call that returns one page.
- The allocator does not catch a panic, since `GlobalAlloc` has no way to report one.

## Proofs and tests

Three Kani harnesses, each with a falsification patch under `falsifications/`.

| harness | what it proves | falsification |
|---|---|---|
| `verification::the_bytes_a_grow_asks_for_never_overflow` | the byte count a grow asks for never overflows | its harness fails with the patch |
| `verification::a_grow_never_commits_past_the_maximum` | committed bytes stay at or under the maximum | its harness fails with the patch |
| `verification::a_grow_at_the_maximum_is_a_refusal` | a grow at the maximum is a refusal and commits nothing | its harness fails with the patch |

Sixteen host tests drive the policy through a fake `PageSource`. They cover the doubling from eight pages, the
first heap fitting in one page, a second `initialize`, a grow of zero bytes, the refusals, zero-size
and over-aligned layouts, and the accessors. The crate disables doctests, because every
bare-metal path ends in a syscall.

The fuzz crate holds a libFuzzer target, committed seeds and a
test that replays the seeds, which is the check that would catch the `talc` defect of upstream pull
request 57 coming back. No proof covers the `unsafe impl GlobalAlloc` or a caller's promise that
the slot holds a memory region.

## What was run

On the branch's base, `main` at `cc4ee3ad9`, on 2026-11-19 (UTC).

| check | result |
|---|---|
| `cargo test -p memory_region_heap` | 16 host tests pass, no doctest |
| `cargo clippy -p memory_region_heap --all-targets -- -D warnings` | no warning |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p memory_region_heap --no-deps` | no warning |
| `cargo build -p memory_region_heap --lib` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and `riscv64imac-unknown-none-elf` | all three build |
| a copy of the crate outside the tree, 1.88, with the three path dependencies as absolute paths, `rust-version` and license written in, and the workspace `[patch]` for `talc` | `cargo +1.88 test` passes 16 tests; `cargo +1.88 build --lib --target x86_64-unknown-none` builds. The one warning is `kani` as an unknown `cfg`, which the nife workspace's lint table declares and the copy's did not |
| `cargo miri test -p memory_region_heap` | 16 pass under Miri |
| `cargo miri test` on the fuzz crate's `seeds` test, with `-Zmiri-permissive-provenance -Zmiri-disable-isolation` | passes |
| `cargo test` on the fuzz crate's `seeds` test | passes |
| `cargo kani` on the copy | 3 of 3 harnesses verify, each in about one second |
| each falsification patch applied to a fresh copy, its harness run | all 3 fail, as they must |
| `helpers/name_release.py --draft memory_region_heap` | every item reads ratified except the fuzz crate's four |
| `script/crate-standalone memory_region_heap` | 14 failures, all the move lane's work: four unratified fuzz names, the three path dependencies, the inherited `license` and `lints`, no `rust-version`, no README, no license files, 5 milestone citations and 7 nife paths. The build and proofs steps skipped because the copy cannot resolve the path dependencies; the runs above stand in for them |
| `script/crate-move --dry-run --into <empty dir> memory_region_heap` | stages at template `v8`; Name blocks removed from 5 files; 3 patches regenerated for `-p1` and apply exactly; 8 findings for the move lane: no README, the lockfile step fails on the inherited `license`, no license files, prose caps on the staged README scaffold, `va` in the fuzz crate's policy copy, 5 milestone citations, 7 nife paths and 9 lines that mention nife, all in the fuzz crate and the falsification patch comments |

Not run, and so not claimed:

- Mutation testing. The census runs from about 08:00 to 13:00 UTC, and this lane ran at about 22:00
  UTC. The census groups the crate with two others, so no per-crate figure exists.
- `cargo-semver-checks` (a first release has no baseline), the stranger test, and the template's
  workflows, which run in the repository after the move.
- The QEMU suite. The kernel-side behavior of `MAP` on a memory region stays in `system_tests`.
- A fuzz campaign. The seeds replay; a long run is the fuzz host's.

## The `talc` position

Ruled 2026-11-14 (UTC), option A: outside nife the crate depends on crates.io `talc = "5.1.1"`
unpatched. nife's root `[patch.crates-io]` keeps building the vendored copy with upstream pull
request 57 for every nife image. The move lane writes this README sentence. The unpatched package
lacks two fit-check fixes. They matter only for a free gap within one chunk of the top of the
address space, and a 64-bit user process never maps one. When a `talc` release carries the fix, the
crate bumps and the sentence goes. The fork question exists only once the monthly
`script/vendor-watch` report passes in silence; it is not live and this package does not raise it.

## What still blocks the move

The move is step 9 of milestone 1017, after the contract crates it links, on this crate's own clearance, with `subtree_scope` or alone.

| item | owner |
|---|---|
| Convert `abi`, `address_space_layout` and `user_mode_runtime` from path dependencies to `git` and `rev` pins with `version`, once their repositories exist. | move lane |
| Write the README (what pays, where it lives, how it grows, the minimum cap, what null means, the `talc` sentence and monotonic growth), both license files, `license`, `rust-version`, `description` and `repository`; drop the inherited `lints`. | move lane |
| Remove nife citations from the fuzz crate, the falsification patch comments and the manifest comments; rename `va` in the fuzz crate's policy copy. | move lane |
| One copy of the growth policy: the std overlay and `second_mount.rs` copy `MINIMUM_GROW_PAGES = 8` today. | a later lane, item 8 of the milestone |
| The files that name the crate (listed by `script/crate-move`): the three manifests, `helpers/template_report_baseline.txt`, `helpers/lint_ratchet.py`, `helpers/trust_boundary_crates.tsv`, `packages/runtime.package.toml`, `crates/externalization-list.txt` and others. | move lane |
| The first template run, the stranger test and the first weekly mutation run. | move lane, in the new repository |

## Open questions

None for the maintainer. The package is cleared (2026-11-20 UTC, no tag, no publish), and the
clearance approves the repository, `nifeos/memory_region_heap`, with `publish = false`. The move, in
step 9 of milestone 1017, waits for `abi`, `address_space_layout` and `user_mode_runtime` to get
repositories. It leaves with `subtree_scope` or alone, decided at the move, and before `redoxfs`.
