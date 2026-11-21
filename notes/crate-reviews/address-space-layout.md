# Review package: `address_space_layout`

*Name: provisional. The `address_space_layout` package of [crate-reviews.md](../crate-reviews.md),
in its own file.*

The architect cleared this package on 2026-11-21 (UTC), option A of the milestone's decision, for
the crate's move to its own repository at step 3 of [milestone 1017 (the contract crates leave in
this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md). The review that
raised the findings is [milestone 1076 (`address_space_layout`: what an outside maintainer would
fix before it leaves)](../../design/roadmap/1076-address-space-map-outside-review.md), which holds
the full item text. This package describes the crate as `main` leaves it.

## In one paragraph

The one table every program of a process agrees on: where its image, stack, heap and shared windows
sit in the low half of its address space. Eight bands tile the first 2 GiB, the same on aarch64,
riscv64 and x86_64. `const fn` checks (`pair_page`, `runtime_window`, `service_window` and the
whole-window `window`) stop the build when a page or a window sits outside its band, and
`check_image` tells a loader why an image does not fit. The crate is `no_std`, has no dependency
and no `unsafe` (`#![forbid(unsafe_code)]`), and eleven Kani harnesses prove the layout. The kernel,
the program builder, the runtime, the linker script test and every fixture that shares a page read
it. No text in the crate points back into nife.

## Identity

| item | value |
|---|---|
| crate | `address_space_layout`, ratified (decision N1, ruled 2026-11-14 UTC) |
| identity values | `INTERFACE_ID` 2 and `VERSION` 1, from row 2 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table. No magic word: the crate has no wire form. `VERSION` and `INTERFACE_ID` still carry provisional `Name:` blocks (see the names table) |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, with `LICENSE-MIT` and `LICENSE-APACHE` in the crate directory |
| `publish` | `false`; a separate ruling holds every first publish until the last outside-review milestone closes |
| size | `src/lib.rs` 682 lines and `src/verification.rs` 234; 19 tracked files in all |
| dependencies | none |
| mutation | the crate's `.github/mutants-exempts` names the one survivor, `check_image`'s `>` to `>=`, with its equivalence reason, and `.cargo/mutants.toml` leaves the `cfg(kani)` module out; on the staged copy 20 mutants are caught, 26 unviable, and the template's gate passes |

## Who in nife depends on it

Twenty-eight manifests name the crate (twenty-seven dependents, plus the root `Cargo.toml`):

| kind | where |
|---|---|
| path dependency, normal | `components`, `kernel`, `fixtures`, `jig`, `xtask`, `redoxfs_server`, `c_seam`, `capability_witness_contract`, `confined_system_call_fuzzer_contract`, `current_cpu_contract`, `driver_process_contract`, `free`, `hot_swap_contract`, `initial_ramdisk_writer`, `machine_statistics_contract`, `memory_region_heap`, `process_builder`, `slabtop`, `soak_page`, `startup_page_contract`, `supervision_contract`, `system_initializer`, `user_mode_runtime`, `video_terminal` |
| path dependency, development | `page_tables`, `std_runtime_contract`, `system_tests` |
| prose and tables | `packages/contracts.package.toml`, `helpers/trust_boundary_crates.tsv`, `crates/externalization-list.txt`, `helpers/interface_declarations_check.py` |

The linker script test now lives in `user_mode_runtime` (`src/linker_script.rs`), which parses its
own `link.ld` against `address_space_layout::IMAGE_BASE_VIRTUAL_ADDRESS`. No test in the crate reads a file outside
it, and no `include_str!` remains. The crate has no fuzz target.

## What was run

All of these ran on this lane's tree, at the commit the pull request carries.

| check | result |
|---|---|
| `script/crate-standalone address_space_layout` | one failure, `names`: 20 names are not ratified (`VERSION` and `INTERFACE_ID` provisional; 18 public items carry no block). Passing: the build on 1.88 for the host and the three bare-metal targets, Kani 11 of 11 harnesses verified, and all eleven falsification patches turn exactly their own harness red |
| `script/crate-move --dry-run address_space_layout` | clean: every patch applies with no offset or fuzz, the template's prose, README, naming and history checks pass, and no file mentions nife |
| `cargo kani` and the template's `falsify.py` on a git checkout of the staged crate | 11 of 11 verified; all eleven records hold |
| `cargo mutants` and the template's `mutants_gate.py` on the staged crate | 20 caught, 26 unviable, one survivor, exempted with its reason |
| `cargo test -p address_space_layout` | 10 unit tests and 3 doctests pass |
| `cargo xtask std-src` with every manifest pointing at a local git repository of the staged crate | the farm copied `src/lib.rs` from Cargo's git checkout |

Not run, and so not claimed: the stranger test, the template's workflows, `cargo package`, and
`cargo-semver-checks`, since a first release has no baseline. The template's `msrv` job confirms
the 1.88 floor.

## Public API

| kind | items |
|---|---|
| identity | `VERSION`, `INTERFACE_ID` |
| page size | `PAGE_BYTES` (4096) |
| type | `Band { start, end }` with `new`, `bytes`, `contains`, `holds` |
| bands | `NULL_GUARD`, `PAIR_PAGES`, `RUNTIME_WINDOWS`, `HEAP`, `SERVICE_WINDOWS`, `IMAGE`, `STACK`, `PROCESS_PAGES`, and `BANDS: &[(&str, Band)]` in address order |
| addresses | `CODE_MODEL_CEILING_VIRTUAL_ADDRESS`, `IMAGE_BASE_VIRTUAL_ADDRESS`, `STACK_TOP_VIRTUAL_ADDRESS`, `STACK_TOP_PAGE_VIRTUAL_ADDRESS`, `CURRENT_CPU_PAGE_VIRTUAL_ADDRESS`, `INITIAL_RAMDISK_VIRTUAL_ADDRESS` |
| count | `MAXIMUM_STACK_PAGES` |
| cross-check | `LOW_HALF_SPLIT_SHIFTS`, each architecture's low-half split shift |
| checked placement | `window(band, start_virtual_address, bytes)`, `pair_page`, `runtime_window`, `service_window` |
| image check | `check_image(start, end)`, `ImagePlacement::{TooLarge { image_end_virtual_address, stack_base_virtual_address }, OutsideBand { image_start_virtual_address, image_end_virtual_address }}` |

No nife type appears in a signature. What a stranger may find surprising:

- Every band and fixed window sits at one address in every process, so there is no address-space
  layout randomization. The crate root and the README say so, and what the design buys.
- `Band`'s fields are public, so a caller who skips `Band::new` can build an inverted band and
  `bytes` then underflows.
- `window` checks a window against its band, not against another window in the same band; the
  crate that places two windows asserts that they are disjoint.
- `LOW_HALF_SPLIT_SHIFTS` restates the paging code's split shifts; a `page_tables` test ties the
  two.

## Proofs

Eleven Kani harnesses, each with a falsification patch under `falsifications/`, state the layout.

- Every address below 2 GiB lies in exactly one band, and the map lies in the user half of all
  three architectures.
- `BANDS` is in address order.
- `pair_page`, `runtime_window` and `service_window` each accept every page of their band and
  refuse every address their documentation forbids.
- `window` accepts every page-aligned, non-empty window its band holds. It refuses a malformed or
  overrunning one.
- `check_image` places, refuses or calls outside exactly as documented.

The harnesses name no unproved assumption. Two further facts hold at compile time and not by
proof: the bands tile `[0, 2 GiB)` with no gap, and the stack and process pages share one 2 MiB
page-table window.

## Rulings in force

- The crate is `address_space_layout`, in code and as the package name, with no alias of the
  earlier spelling (decision N1, option A, ruled 2026-11-14 UTC).
- The band numbers and `MAXIMUM_STACK_PAGES` come from §171 (where a program image starts),
  option D, which ratified them.
- The review clearance of 2026-11-21 UTC ratifies the whole-window check `window`. The three
  one-page functions wrap it.
- The review is cleared (option A, ruled 2026-11-21 UTC): the crate moves to
  `nifeos/address_space_layout` with `publish = false`, no tag and no publish, and the names of
  items 7, 9, 15 and 17 are ratified as proposed.

## Names

| item | name | state |
|---|---|---|
| 7 | `PAGE_BYTES`; `IMAGE_BASE_VIRTUAL_ADDRESS`, `STACK_TOP_VIRTUAL_ADDRESS`, `STACK_TOP_PAGE_VIRTUAL_ADDRESS`, `CURRENT_CPU_PAGE_VIRTUAL_ADDRESS`, `CODE_MODEL_CEILING_VIRTUAL_ADDRESS`; the `ImagePlacement` fields `image_start_virtual_address`, `image_end_virtual_address`, `stack_base_virtual_address` | ratified 2026-11-21 (UTC), renamed at every call site |
| 15 | `Band::new` | ratified 2026-11-21 (UTC) |
| 17 | public `LOW_HALF_SPLIT_SHIFTS` | ratified 2026-11-21 (UTC) |
| 9 | `window` | ratified 2026-11-21 (UTC) |
| - | `VERSION`, `INTERFACE_ID` | provisional |
| - | `INITIAL_RAMDISK_VIRTUAL_ADDRESS`, `MAXIMUM_STACK_PAGES` | ratified |
| - | `Band`, the eight band constants, `BANDS`, `bytes`, `contains`, `holds`, `pair_page`, `runtime_window`, `service_window`, `check_image`, `ImagePlacement` | no block; they pass the template's naming checks |

## What still blocks the move

| item | owner |
|---|---|
| The repository, its secrets and the first push of the staged crate | move lane |
| Item 13: the template's `register_names.py` reads `address` in a harness function name as an address. Until nifeos/crate-workflows fixes it, the crate's `check / standalone` is red on its first run | maintainer, in another repository |
| The pins: all 27 dependents move to `address_space_layout = { version = "0.1.0", git = "https://github.com/nifeos/address_space_layout", rev = "<landed commit>" }` in one pull request, preferably through one root `[workspace.dependencies]` entry, with `helpers/interface_declarations_check.py` reading the pinned checkout | move lane |
| Item 18 and the milestone 1017 table: its blocker row for this crate names a test that has since moved | the 1017 lane |

The clearance kept the three one-page functions as wrappers and `BANDS`' band names as strings, as
built.

## Reproducer

```
script/crate-review-packet address_space_layout
script/crate-standalone address_space_layout
script/crate-move --dry-run --into <directory> address_space_layout
cargo test -p address_space_layout
cargo clippy -p address_space_layout --all-targets -- -D warnings
script/names address_space_layout
python3 helpers/name_release.py --draft address_space_layout
```
